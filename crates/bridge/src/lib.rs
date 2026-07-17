use postfiat_crypto_provider::{
    hash_hex, hex_to_bytes, ml_dsa_65_verify_with_context, BRIDGE_WITNESS_SIGNATURE_CONTEXT,
    ML_DSA_65_ALGORITHM,
};
use postfiat_types::{
    pftl_uniswap_non_consumption_proof_hash, pftl_uniswap_return_burn_id_from_fields, BridgeDomain,
    BridgeDomainSpec, BridgeState, BridgeTransfer, BridgeWitnessAttestation,
    BRIDGE_DIRECTION_INBOUND, BRIDGE_DIRECTION_OUTBOUND,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const CRATE_PURPOSE: &str = "bridge simulation state machine";
pub const ROUTE_TRUST_CLASS_CONTROLLED: &str = "CONTROLLED";
pub const ROUTE_TRUST_CLASS_OPTIMISTIC: &str = "OPTIMISTIC";
pub const ROUTE_TRUST_CLASS_TRUSTLESS_FINALITY: &str = "TRUSTLESS_FINALITY";
pub const ROUTE_TRUST_CLASS_BFT_CHECKPOINT: &str = "BFT_CHECKPOINT";
pub const ROUTE_TRUST_CLASS_DISABLED: &str = "DISABLED";
pub const PFTL_UNISWAP_ROUTE_FAMILY_PRIMARY_MINT: &str = "primary_pftl_mint";
pub const PFTL_UNISWAP_ROUTE_FAMILY_SECONDARY_INVENTORY: &str = "secondary_inventory";
pub const PRIMARY_SUBSCRIPTION_ROUNDING_RULE_FLOOR_RESERVE_KEEPS_DUST: &str =
    "floor_nav_atoms_reserve_keeps_dust";
pub const PFTL_UNISWAP_STATUS_MAX_ROWS: usize = 512;

fn default_pftl_uniswap_route_family() -> String {
    PFTL_UNISWAP_ROUTE_FAMILY_PRIMARY_MINT.to_string()
}

fn is_default_pftl_uniswap_route_family(value: &String) -> bool {
    value == PFTL_UNISWAP_ROUTE_FAMILY_PRIMARY_MINT
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PftlUniswapRouteConfig {
    pub schema: String,
    pub route_id: String,
    #[serde(
        default = "default_pftl_uniswap_route_family",
        skip_serializing_if = "is_default_pftl_uniswap_route_family"
    )]
    pub route_family: String,
    pub native_nav_asset_id: String,
    pub settlement_asset_id: String,
    pub wrapped_navcoin_token: String,
    pub handoff_controller: String,
    pub settlement_adapter: String,
    pub verifier_mode: String,
    pub route_trust_class: String,
    pub uniswap_pool_id_or_path: String,
    pub router: String,
    pub failure_behavior: String,
    pub route_supply_cap_atoms: u64,
    pub packet_notional_cap_atoms: u64,
    pub seed_nav_epoch: u64,
    pub seed_usdc_atoms: u64,
    pub seed_wrapped_navcoin_atoms: u64,
    pub lp_recipient: String,
    pub lp_custody_policy: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PftlUniswapOfficialUniswapV4Deployments {
    pub chain_id: u64,
    pub deployments_source_url: String,
    pub deployments_table_hash: String,
    pub checked_at_utc: String,
    pub pool_manager: String,
    pub position_manager: String,
    pub universal_router: String,
    pub permit2: String,
    pub state_view: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PftlUniswapPoolSeedConfig {
    pub pricing_nav_epoch: u64,
    pub pricing_reserve_packet_hash: String,
    pub seed_usdc_atoms: u64,
    pub seed_wrapped_navcoin_atoms: u64,
    pub nav_price_settlement_atoms_per_nav_atom: u64,
    pub tick_lower: i32,
    pub tick_upper: i32,
    pub fee_pips: u32,
    pub lp_recipient: String,
    pub position_recipient: String,
    pub lp_custody_policy: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PftlUniswapLaunchConfig {
    pub schema: String,
    pub route_id: String,
    pub route_config_digest: String,
    pub route_trust_class: String,
    pub native_nav_asset_id: String,
    pub settlement_asset_id: String,
    pub wrapped_navcoin_token: String,
    pub usdc_token: String,
    pub handoff_controller: String,
    pub receipt_verifier: String,
    pub settlement_adapter: String,
    pub official_uniswap: PftlUniswapOfficialUniswapV4Deployments,
    pub uniswap_pool_key_hash: String,
    pub uniswap_pool_id: String,
    pub seed: PftlUniswapPoolSeedConfig,
    pub fork_rehearsal_required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PftlUniswapForkRehearsalEvidence {
    pub schema: String,
    pub rehearsal_id: String,
    pub launch_config_digest: String,
    pub route_config_digest: String,
    pub fork_chain_id: u64,
    pub fork_block_number: u64,
    pub official_uniswap: PftlUniswapOfficialUniswapV4Deployments,
    pub uniswap_pool_key_hash: String,
    pub uniswap_pool_id: String,
    pub seed_export_packet_hash: String,
    pub seed_receipt_root: String,
    pub seed_mint_tx_hash: String,
    pub seed_lp_tx_hash: String,
    pub external_buy_tx_hash: String,
    pub external_sell_tx_hash: String,
    pub mint_only_packet_tx_hash: String,
    pub mint_and_swap_packet_tx_hash: String,
    pub state_view_liquidity_after_seed: u128,
    pub state_view_liquidity_after_buy: u128,
    pub state_view_liquidity_after_sell: u128,
    pub user_buy_usdc_spent_atoms: u64,
    pub user_buy_wrapped_received_atoms: u64,
    pub user_sell_wrapped_spent_atoms: u64,
    pub user_sell_usdc_received_atoms: u64,
    pub canonical_supply_before_external_trades_atoms: u64,
    pub canonical_supply_after_external_trades_atoms: u64,
    pub packet_consumed_without_manual_mint: bool,
    pub min_output_failure_reverted_without_consume: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrimarySubscriptionQuoteInput {
    pub settlement_value_atoms: u64,
    pub nav_price_settlement_atoms_per_nav_atom: u64,
    pub pricing_nav_epoch: u64,
    pub pricing_reserve_packet_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PftlUniswapPrimarySubscriptionRequest {
    pub route_id: String,
    pub source_wallet: String,
    pub settlement_asset_id: String,
    pub subscription_nonce: String,
    pub quote: PrimarySubscriptionQuoteInput,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrimarySubscriptionQuote {
    pub route_family: String,
    pub supply_effect: String,
    pub pricing_source: String,
    pub settlement_reserve_effect: String,
    pub settlement_value_atoms: u64,
    pub requested_settlement_atoms: u64,
    pub accepted_settlement_atoms: u64,
    pub refund_settlement_atoms: u64,
    pub nav_price_settlement_atoms_per_nav_atom: u64,
    pub pricing_nav_epoch: u64,
    pub pricing_reserve_packet_hash: String,
    pub minted_nav_atoms: u64,
    pub dust_settlement_atoms: u64,
    pub rounding_rule: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PftlUniswapMintAndSwapPacket {
    pub schema: String,
    pub route_id: String,
    pub config_digest: String,
    pub source_packet_hash: String,
    pub source_receipt_hash: String,
    pub source_receipt_root: String,
    pub source_wallet: String,
    pub settlement_asset_id: String,
    pub native_nav_asset_id: String,
    pub wrapped_navcoin_token: String,
    pub ethereum_recipient: String,
    pub token_out: String,
    pub settlement_amount_atoms: u64,
    pub mint_amount_atoms: u64,
    pub pricing_nav_epoch: u64,
    pub pricing_reserve_packet_hash: String,
    pub uniswap_pool_id_or_path: String,
    pub swap_path_hash: String,
    pub router: String,
    pub minimum_output_atoms: u64,
    pub deadline_seconds: u64,
    pub nonce: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PftlUniswapExportPacketStatus {
    SourceDebited,
    DestinationConsumed,
    SourceRefunded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PftlUniswapReturnBurnStatus {
    BurnObserved,
    Imported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PftlUniswapExportPacketState {
    pub packet_hash: String,
    pub nonce: String,
    pub source_wallet: String,
    pub ethereum_recipient: String,
    pub amount_atoms: u64,
    pub source_height: u64,
    pub destination_deadline_seconds: u64,
    pub refund_not_before_height: u64,
    pub status: PftlUniswapExportPacketStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PftlUniswapReturnBurnState {
    pub burn_event_hash: String,
    pub ethereum_chain_id: u64,
    pub bridge_controller: String,
    pub wrapped_navcoin_token: String,
    pub native_nav_asset_id: String,
    pub ethereum_sender: String,
    pub pftl_recipient: String,
    pub amount_atoms: u64,
    pub return_nonce: String,
    pub burn_height: u64,
    pub finalized_height: u64,
    pub status: PftlUniswapReturnBurnStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PftlUniswapBridgeLedger {
    pub schema: String,
    pub route_id: String,
    #[serde(
        default = "default_pftl_uniswap_route_family",
        skip_serializing_if = "is_default_pftl_uniswap_route_family"
    )]
    pub route_family: String,
    pub route_config_digest: String,
    pub route_trust_class: String,
    pub native_nav_asset_id: String,
    pub settlement_asset_id: String,
    pub handoff_controller: String,
    pub settlement_adapter: String,
    pub wrapped_navcoin_token: String,
    pub ethereum_chain_id: u64,
    pub route_supply_cap_atoms: u64,
    pub packet_notional_cap_atoms: u64,
    pub latest_finalized_nav_epoch: u64,
    pub return_finality_blocks: u64,
    pub authorized_valid_supply_atoms: u64,
    pub pftl_spendable_supply_atoms: u64,
    #[serde(default)]
    pub native_spendable_balances_atoms: BTreeMap<String, u64>,
    pub ethereum_spendable_supply_atoms: u64,
    pub other_registered_venue_supply_atoms: u64,
    pub outstanding_bridge_claims_atoms: u64,
    pub pending_return_import_claims_atoms: u64,
    pub settlement_reserve_atoms: u64,
    pub primary_subscription_nonces: BTreeMap<String, String>,
    pub export_packets: BTreeMap<String, PftlUniswapExportPacketState>,
    pub export_nonces: BTreeMap<String, String>,
    pub return_burns: BTreeMap<String, PftlUniswapReturnBurnState>,
    pub paused: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PftlUniswapExportDebitRequest {
    pub route_id: String,
    pub packet_hash: String,
    pub nonce: String,
    pub source_wallet: String,
    pub ethereum_recipient: String,
    pub amount_atoms: u64,
    pub source_height: u64,
    pub destination_deadline_seconds: u64,
    pub refund_not_before_height: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PftlUniswapRefundRequest {
    pub packet_hash: String,
    pub current_height: u64,
    pub non_consumption_proof_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PftlUniswapReturnBurnRequest {
    pub burn_event_hash: String,
    pub ethereum_chain_id: u64,
    pub bridge_controller: String,
    pub wrapped_navcoin_token: String,
    pub native_nav_asset_id: String,
    pub ethereum_sender: String,
    pub pftl_recipient: String,
    pub amount_atoms: u64,
    pub return_nonce: String,
    pub burn_height: u64,
    pub finalized_height: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PftlUniswapTransitionReceipt {
    pub schema: String,
    pub transition: String,
    pub route_id: String,
    pub route_config_digest: String,
    pub route_trust_class: String,
    pub settlement_asset_id: Option<String>,
    pub native_nav_asset_id: String,
    pub wrapped_navcoin_token: String,
    pub ethereum_chain_id: u64,
    pub bridge_controller: Option<String>,
    pub packet_hash: Option<String>,
    pub nonce: Option<String>,
    pub return_burn_event_hash: Option<String>,
    pub source_wallet: Option<String>,
    pub ethereum_sender: Option<String>,
    pub ethereum_recipient: Option<String>,
    pub pftl_recipient: Option<String>,
    pub amount_atoms: Option<u64>,
    pub settlement_amount_atoms: Option<u64>,
    pub requested_settlement_atoms: Option<u64>,
    pub accepted_settlement_atoms: Option<u64>,
    pub refund_settlement_atoms: Option<u64>,
    pub minted_nav_atoms: Option<u64>,
    pub nav_price_settlement_atoms_per_nav_atom: Option<u64>,
    pub rounding_rule: Option<String>,
    pub pricing_nav_epoch: Option<u64>,
    pub pricing_reserve_packet_hash: Option<String>,
    pub non_consumption_proof_hash: Option<String>,
    pub source_height: Option<u64>,
    pub destination_deadline_seconds: Option<u64>,
    pub refund_not_before_height: Option<u64>,
    pub burn_height: Option<u64>,
    pub finalized_height: Option<u64>,
    pub state_before_hash: String,
    pub state_after_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PftlUniswapReceiptBatch {
    pub schema: String,
    pub receipt_hashes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PftlUniswapReceiptReplayReport {
    pub schema: String,
    pub route_id: String,
    pub initial_ledger_hash: String,
    pub final_ledger_hash: String,
    pub receipt_root: String,
    pub receipt_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PftlUniswapRoutesStatusReport {
    pub schema: String,
    pub route_count: u64,
    pub routes: Vec<PftlUniswapRouteStatusRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PftlUniswapRouteStatusRow {
    pub route_id: String,
    pub route_family: String,
    pub route_config_digest: String,
    pub route_trust_class: String,
    pub route_live: bool,
    pub paused: bool,
    pub native_nav_asset_id: String,
    pub settlement_asset_id: String,
    pub wrapped_navcoin_token: String,
    pub handoff_controller: String,
    pub settlement_adapter: String,
    pub ethereum_chain_id: u64,
    pub latest_finalized_nav_epoch: u64,
    pub route_supply_cap_atoms: u64,
    pub packet_notional_cap_atoms: u64,
    pub authorized_valid_supply_atoms: u64,
    pub supply_cap_remaining_atoms: u64,
    pub outstanding_bridge_claims_atoms: u64,
    pub pending_return_import_claims_atoms: u64,
    pub primary_subscription_count: u64,
    pub export_packet_count: u64,
    pub outstanding_export_packet_count: u64,
    pub consumed_export_packet_count: u64,
    pub refunded_export_packet_count: u64,
    pub return_burn_count: u64,
    pub pending_return_burn_count: u64,
    pub imported_return_burn_count: u64,
    pub ledger_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PftlUniswapSupplyStatusReport {
    pub schema: String,
    pub route_id: String,
    pub route_config_digest: String,
    pub native_nav_asset_id: String,
    pub settlement_asset_id: String,
    pub wrapped_navcoin_token: String,
    pub native_spendable_balances: Vec<PftlUniswapNativeBalanceRow>,
    pub native_spendable_balance_count: u64,
    pub native_spendable_balance_limit: u64,
    pub native_spendable_balances_truncated: bool,
    pub native_spendable_balance_sum_atoms: u64,
    pub authorized_valid_supply_atoms: u64,
    pub pftl_spendable_supply_atoms: u64,
    pub ethereum_spendable_supply_atoms: u64,
    pub other_registered_venue_supply_atoms: u64,
    pub outstanding_bridge_claims_atoms: u64,
    pub pending_return_import_claims_atoms: u64,
    pub live_supply_sum_atoms: u64,
    pub route_supply_cap_atoms: u64,
    pub supply_cap_remaining_atoms: u64,
    pub packet_notional_cap_atoms: u64,
    pub settlement_reserve_atoms: u64,
    pub invariant_holds: bool,
    pub ledger_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PftlUniswapNativeBalanceRow {
    pub wallet: String,
    pub amount_atoms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PftlUniswapPacketStatusReport {
    pub schema: String,
    pub route_id: String,
    pub route_config_digest: String,
    pub packet_hash: String,
    pub packet: PftlUniswapExportPacketStatusRow,
    pub ledger_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PftlUniswapClaimsStatusReport {
    pub schema: String,
    pub route_id: String,
    pub route_config_digest: String,
    pub ledger_hash: String,
    pub limit: u64,
    pub truncated: bool,
    pub outstanding_bridge_claims_atoms: u64,
    pub pending_return_import_claims_atoms: u64,
    pub export_claim_count: u64,
    pub return_claim_count: u64,
    pub exports: Vec<PftlUniswapExportPacketStatusRow>,
    pub returns: Vec<PftlUniswapReturnBurnStatusRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PftlUniswapExportPacketStatusRow {
    pub packet_hash: String,
    pub nonce: String,
    pub source_wallet: String,
    pub ethereum_recipient: String,
    pub amount_atoms: u64,
    pub source_height: u64,
    pub destination_deadline_seconds: u64,
    pub refund_not_before_height: u64,
    pub status: PftlUniswapExportPacketStatus,
    pub claim_class: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PftlUniswapReturnBurnStatusRow {
    pub burn_event_hash: String,
    pub ethereum_chain_id: u64,
    pub bridge_controller: String,
    pub wrapped_navcoin_token: String,
    pub native_nav_asset_id: String,
    pub ethereum_sender: String,
    pub pftl_recipient: String,
    pub amount_atoms: u64,
    pub return_nonce: String,
    pub burn_height: u64,
    pub finalized_height: u64,
    pub status: PftlUniswapReturnBurnStatus,
    pub claim_class: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeTransferRequest {
    pub domain_id: String,
    pub direction: String,
    pub from: String,
    pub to: String,
    pub asset_id: String,
    pub amount: u64,
    pub witness_id: String,
    pub witness_epoch: u32,
    pub witness_attestation: Option<BridgeWitnessAttestation>,
}

#[derive(Debug, Clone, Copy)]
pub struct BridgeWitnessChainDomain<'a> {
    pub chain_id: &'a str,
    pub genesis_hash: &'a str,
    pub protocol_version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BridgeError {
    code: &'static str,
    message: String,
}

impl BridgeError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn code(&self) -> &'static str {
        self.code
    }
}

impl std::fmt::Display for BridgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for BridgeError {}

#[derive(Debug, Serialize)]
struct TransferSeed<'a> {
    domain_id: &'a str,
    source_chain: &'a str,
    target_chain: &'a str,
    bridge_id: &'a str,
    door_account: &'a str,
    direction: &'a str,
    from: &'a str,
    to: &'a str,
    asset_id: &'a str,
    amount: u64,
    witness_id: &'a str,
    witness_epoch: u32,
    witness_attestation_id: &'a str,
    sequence: u64,
}

#[derive(Debug, Serialize)]
struct WitnessAttestationSeed<'a> {
    chain_id: &'a str,
    genesis_hash: &'a str,
    protocol_version: u32,
    domain_id: &'a str,
    source_chain: &'a str,
    target_chain: &'a str,
    bridge_id: &'a str,
    door_account: &'a str,
    direction: &'a str,
    from: &'a str,
    to: &'a str,
    asset_id: &'a str,
    amount: u64,
    witness_id: &'a str,
    witness_epoch: u32,
    signer: &'a str,
    algorithm_id: &'a str,
    public_key_hex: &'a str,
}

pub fn upsert_domain(
    state: &mut BridgeState,
    domain_id: impl Into<String>,
    name: impl Into<String>,
    inbound_cap: u64,
    outbound_cap: u64,
) -> Result<BridgeDomain, BridgeError> {
    upsert_domain_with_metadata(
        state,
        BridgeDomainSpec::new(domain_id, name, inbound_cap, outbound_cap),
    )
}

pub fn upsert_domain_with_metadata(
    state: &mut BridgeState,
    spec: BridgeDomainSpec,
) -> Result<BridgeDomain, BridgeError> {
    let domain_id = spec.domain_id;
    let name = spec.name;
    let source_chain = default_if_empty(spec.source_chain, &domain_id);
    let target_chain = default_if_empty(spec.target_chain, "postfiat-local");
    let bridge_id = default_if_empty(spec.bridge_id, &domain_id);
    let door_account = default_if_empty(spec.door_account, &format!("door:{domain_id}"));
    let inbound_cap = spec.inbound_cap;
    let outbound_cap = spec.outbound_cap;
    validate_nonempty("domain_id", &domain_id)?;
    validate_nonempty("name", &name)?;
    validate_nonempty("source_chain", &source_chain)?;
    validate_nonempty("target_chain", &target_chain)?;
    validate_nonempty("bridge_id", &bridge_id)?;
    validate_nonempty("door_account", &door_account)?;

    if let Some(domain) = state.domain_mut(&domain_id) {
        if inbound_cap < domain.inbound_used {
            return Err(BridgeError::new(
                "inbound_cap_below_used",
                format!(
                    "inbound cap {inbound_cap} is below used amount {}",
                    domain.inbound_used
                ),
            ));
        }
        if outbound_cap < domain.outbound_used {
            return Err(BridgeError::new(
                "outbound_cap_below_used",
                format!(
                    "outbound cap {outbound_cap} is below used amount {}",
                    domain.outbound_used
                ),
            ));
        }
        domain.name = name;
        domain.source_chain = source_chain;
        domain.target_chain = target_chain;
        domain.bridge_id = bridge_id;
        domain.door_account = door_account;
        domain.inbound_cap = inbound_cap;
        domain.outbound_cap = outbound_cap;
        return Ok(domain.clone());
    }

    let domain = BridgeDomain::with_metadata(BridgeDomainSpec {
        domain_id,
        name,
        source_chain,
        target_chain,
        bridge_id,
        door_account,
        inbound_cap,
        outbound_cap,
    });
    state.domains.push(domain.clone());
    Ok(domain)
}

pub fn set_domain_paused(
    state: &mut BridgeState,
    domain_id: &str,
    paused: bool,
) -> Result<BridgeDomain, BridgeError> {
    let domain = state
        .domain_mut(domain_id)
        .ok_or_else(|| unknown_domain(domain_id))?;
    domain.paused = paused;
    Ok(domain.clone())
}

pub fn apply_simulated_transfer(
    state: &mut BridgeState,
    request: BridgeTransferRequest,
) -> Result<BridgeTransfer, BridgeError> {
    validate_request(&request)?;
    if state.has_witness_replay(
        &request.domain_id,
        request.witness_epoch,
        &request.witness_id,
    ) {
        return Err(BridgeError::new(
            "duplicate_witness",
            format!(
                "witness `{}` already processed in epoch {}",
                request.witness_id, request.witness_epoch
            ),
        ));
    }

    let domain_index = state
        .domains
        .iter()
        .position(|domain| domain.domain_id == request.domain_id)
        .ok_or_else(|| unknown_domain(&request.domain_id))?;
    let sequence = state.transfers.len() as u64 + 1;
    let domain = state.domains[domain_index].clone();
    validate_bridge_domain(&domain)?;
    validate_witness_attestation(&domain, &request)?;
    let transfer_id = transfer_id(&domain, &request, sequence)?;

    {
        let domain = &mut state.domains[domain_index];
        if domain.paused {
            return Err(BridgeError::new(
                "domain_paused",
                format!("bridge domain `{}` is paused", request.domain_id),
            ));
        }

        match request.direction.as_str() {
            BRIDGE_DIRECTION_INBOUND => {
                let next_used = domain
                    .inbound_used
                    .checked_add(request.amount)
                    .ok_or_else(|| BridgeError::new("cap_overflow", "inbound cap overflow"))?;
                if next_used > domain.inbound_cap {
                    return Err(BridgeError::new(
                        "inbound_cap_exceeded",
                        format!(
                            "amount {} exceeds inbound remaining cap {}",
                            request.amount,
                            domain.inbound_cap - domain.inbound_used
                        ),
                    ));
                }
                domain.inbound_used = next_used;
            }
            BRIDGE_DIRECTION_OUTBOUND => {
                let next_used = domain
                    .outbound_used
                    .checked_add(request.amount)
                    .ok_or_else(|| BridgeError::new("cap_overflow", "outbound cap overflow"))?;
                if next_used > domain.outbound_cap {
                    return Err(BridgeError::new(
                        "outbound_cap_exceeded",
                        format!(
                            "amount {} exceeds outbound remaining cap {}",
                            request.amount,
                            domain.outbound_cap - domain.outbound_used
                        ),
                    ));
                }
                domain.outbound_used = next_used;
            }
            _ => unreachable!("direction validated"),
        }
    }

    let transfer = BridgeTransfer {
        transfer_id,
        domain_id: request.domain_id,
        source_chain: domain.source_chain,
        target_chain: domain.target_chain,
        bridge_id: domain.bridge_id,
        door_account: domain.door_account,
        direction: request.direction,
        from: request.from,
        to: request.to,
        asset_id: request.asset_id,
        amount: request.amount,
        witness_id: request.witness_id,
        witness_epoch: request.witness_epoch,
        witness_attestation: request.witness_attestation,
        sequence,
    };
    state.record_witness_replay(
        &transfer.domain_id,
        transfer.witness_epoch,
        &transfer.witness_id,
    );
    state.transfers.push(transfer.clone());
    Ok(transfer)
}

fn validate_request(request: &BridgeTransferRequest) -> Result<(), BridgeError> {
    validate_nonempty("domain_id", &request.domain_id)?;
    validate_nonempty("from", &request.from)?;
    validate_nonempty("to", &request.to)?;
    validate_nonempty("asset_id", &request.asset_id)?;
    validate_nonempty("witness_id", &request.witness_id)?;
    if request.amount == 0 {
        return Err(BridgeError::new(
            "zero_amount",
            "bridge transfer amount must be nonzero",
        ));
    }
    if request.witness_epoch == 0 {
        return Err(BridgeError::new(
            "zero_witness_epoch",
            "bridge witness epoch must be nonzero",
        ));
    }
    if request.direction != BRIDGE_DIRECTION_INBOUND
        && request.direction != BRIDGE_DIRECTION_OUTBOUND
    {
        return Err(BridgeError::new(
            "bad_direction",
            format!(
                "direction must be `{}` or `{}`",
                BRIDGE_DIRECTION_INBOUND, BRIDGE_DIRECTION_OUTBOUND
            ),
        ));
    }
    Ok(())
}

fn validate_witness_attestation(
    domain: &BridgeDomain,
    request: &BridgeTransferRequest,
) -> Result<(), BridgeError> {
    let attestation = request.witness_attestation.as_ref().ok_or_else(|| {
        BridgeError::new(
            "missing_witness_attestation",
            "bridge transfer requires witness attestation evidence",
        )
    })?;
    validate_nonempty(
        "witness_attestation.attestation_id",
        &attestation.attestation_id,
    )?;
    validate_nonempty("witness_attestation.chain_id", &attestation.chain_id)?;
    validate_nonempty(
        "witness_attestation.genesis_hash",
        &attestation.genesis_hash,
    )?;
    if attestation.protocol_version == 0 {
        return Err(BridgeError::new(
            "empty_field",
            "witness_attestation.protocol_version must be nonzero",
        ));
    }
    validate_nonempty("witness_attestation.signer", &attestation.signer)?;
    validate_nonempty(
        "witness_attestation.algorithm_id",
        &attestation.algorithm_id,
    )?;
    validate_nonempty(
        "witness_attestation.public_key_hex",
        &attestation.public_key_hex,
    )?;
    validate_nonempty(
        "witness_attestation.signature_hex",
        &attestation.signature_hex,
    )?;
    if attestation.algorithm_id != ML_DSA_65_ALGORITHM {
        return Err(BridgeError::new(
            "unsupported_witness_algorithm",
            format!(
                "bridge witness algorithm `{}` is not supported",
                attestation.algorithm_id
            ),
        ));
    }
    let chain_domain = BridgeWitnessChainDomain {
        chain_id: &attestation.chain_id,
        genesis_hash: &attestation.genesis_hash,
        protocol_version: attestation.protocol_version,
    };
    let expected_id = bridge_witness_attestation_id(
        chain_domain,
        domain,
        request,
        &attestation.signer,
        &attestation.algorithm_id,
        &attestation.public_key_hex,
    )?;
    if attestation.attestation_id != expected_id {
        return Err(BridgeError::new(
            "bad_witness_attestation",
            "bridge witness attestation id does not match transfer evidence",
        ));
    }
    let message = bridge_witness_attestation_message(
        chain_domain,
        domain,
        request,
        &attestation.signer,
        &attestation.algorithm_id,
        &attestation.public_key_hex,
    )?;
    let public_key = hex_to_bytes(&attestation.public_key_hex).map_err(|error| {
        BridgeError::new(
            "bad_witness_attestation",
            format!("bridge witness public key is invalid: {error}"),
        )
    })?;
    let signature = hex_to_bytes(&attestation.signature_hex).map_err(|error| {
        BridgeError::new(
            "bad_witness_attestation",
            format!("bridge witness signature is invalid: {error}"),
        )
    })?;
    if !ml_dsa_65_verify_with_context(
        &public_key,
        &message,
        &signature,
        BRIDGE_WITNESS_SIGNATURE_CONTEXT,
    ) {
        return Err(BridgeError::new(
            "bad_witness_signature",
            "bridge witness signature does not verify",
        ));
    }
    Ok(())
}

fn validate_nonempty(field: &'static str, value: &str) -> Result<(), BridgeError> {
    if value.trim().is_empty() {
        return Err(BridgeError::new(
            "empty_field",
            format!("{field} must be nonempty"),
        ));
    }
    if value != value.trim() {
        return Err(BridgeError::new(
            "boundary_whitespace",
            format!("{field} must not have boundary whitespace"),
        ));
    }
    if value.chars().any(char::is_control) {
        return Err(BridgeError::new(
            "control_character",
            format!("{field} must not contain control characters"),
        ));
    }
    Ok(())
}

fn validate_bridge_domain(domain: &BridgeDomain) -> Result<(), BridgeError> {
    validate_nonempty("bridge_domain.domain_id", &domain.domain_id)?;
    validate_nonempty("bridge_domain.name", &domain.name)?;
    validate_nonempty("bridge_domain.source_chain", &domain.source_chain)?;
    validate_nonempty("bridge_domain.target_chain", &domain.target_chain)?;
    validate_nonempty("bridge_domain.bridge_id", &domain.bridge_id)?;
    validate_nonempty("bridge_domain.door_account", &domain.door_account)?;
    Ok(())
}

fn validate_chain_domain(chain_domain: &BridgeWitnessChainDomain<'_>) -> Result<(), BridgeError> {
    validate_nonempty("chain_domain.chain_id", chain_domain.chain_id)?;
    validate_nonempty("chain_domain.genesis_hash", chain_domain.genesis_hash)?;
    validate_lower_hex("chain_domain.genesis_hash", chain_domain.genesis_hash, 96)?;
    if chain_domain.protocol_version == 0 {
        return Err(BridgeError::new(
            "empty_field",
            "chain_domain.protocol_version must be nonzero",
        ));
    }
    Ok(())
}

fn validate_lower_hex(
    field: &'static str,
    value: &str,
    expected_len: usize,
) -> Result<(), BridgeError> {
    if value.len() != expected_len
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(BridgeError::new(
            "bad_genesis_hash",
            format!("{field} must be {expected_len} lowercase hex characters"),
        ));
    }
    Ok(())
}

fn validate_tx_hash(field: &'static str, value: &str) -> Result<(), BridgeError> {
    validate_lower_hex(field, value, 64)
}

fn default_if_empty(value: String, fallback: &str) -> String {
    if value.trim().is_empty() {
        fallback.to_string()
    } else {
        value
    }
}

fn unknown_domain(domain_id: &str) -> BridgeError {
    BridgeError::new(
        "unknown_domain",
        format!("bridge domain `{domain_id}` not found"),
    )
}

fn transfer_id(
    domain: &BridgeDomain,
    request: &BridgeTransferRequest,
    sequence: u64,
) -> Result<String, BridgeError> {
    let witness_attestation_id = request
        .witness_attestation
        .as_ref()
        .map(|attestation| attestation.attestation_id.as_str())
        .unwrap_or_default();
    hash_json(
        "postfiat.bridge.transfer.sim.v1",
        &TransferSeed {
            domain_id: &request.domain_id,
            source_chain: &domain.source_chain,
            target_chain: &domain.target_chain,
            bridge_id: &domain.bridge_id,
            door_account: &domain.door_account,
            direction: &request.direction,
            from: &request.from,
            to: &request.to,
            asset_id: &request.asset_id,
            amount: request.amount,
            witness_id: &request.witness_id,
            witness_epoch: request.witness_epoch,
            witness_attestation_id,
            sequence,
        },
    )
}

pub fn bridge_witness_attestation_message(
    chain_domain: BridgeWitnessChainDomain<'_>,
    domain: &BridgeDomain,
    request: &BridgeTransferRequest,
    signer: &str,
    algorithm_id: &str,
    public_key_hex: &str,
) -> Result<Vec<u8>, BridgeError> {
    validate_witness_seed_inputs(
        &chain_domain,
        domain,
        request,
        signer,
        algorithm_id,
        public_key_hex,
    )?;
    let seed = witness_attestation_seed(
        chain_domain,
        domain,
        request,
        signer,
        algorithm_id,
        public_key_hex,
    );
    serde_json::to_vec(&seed).map_err(|error| {
        BridgeError::new(
            "serialization_failed",
            format!("failed to encode witness attestation seed: {error}"),
        )
    })
}

pub fn bridge_witness_attestation_id(
    chain_domain: BridgeWitnessChainDomain<'_>,
    domain: &BridgeDomain,
    request: &BridgeTransferRequest,
    signer: &str,
    algorithm_id: &str,
    public_key_hex: &str,
) -> Result<String, BridgeError> {
    validate_witness_seed_inputs(
        &chain_domain,
        domain,
        request,
        signer,
        algorithm_id,
        public_key_hex,
    )?;
    let seed = witness_attestation_seed(
        chain_domain,
        domain,
        request,
        signer,
        algorithm_id,
        public_key_hex,
    );
    hash_json("postfiat.bridge.witness_attestation.v1", &seed)
}

fn validate_witness_seed_inputs(
    chain_domain: &BridgeWitnessChainDomain<'_>,
    domain: &BridgeDomain,
    request: &BridgeTransferRequest,
    signer: &str,
    algorithm_id: &str,
    public_key_hex: &str,
) -> Result<(), BridgeError> {
    validate_chain_domain(chain_domain)?;
    validate_bridge_domain(domain)?;
    validate_request(request)?;
    validate_nonempty("signer", signer)?;
    validate_nonempty("algorithm_id", algorithm_id)?;
    validate_nonempty("public_key_hex", public_key_hex)?;
    Ok(())
}

fn witness_attestation_seed<'a>(
    chain_domain: BridgeWitnessChainDomain<'a>,
    domain: &'a BridgeDomain,
    request: &'a BridgeTransferRequest,
    signer: &'a str,
    algorithm_id: &'a str,
    public_key_hex: &'a str,
) -> WitnessAttestationSeed<'a> {
    WitnessAttestationSeed {
        chain_id: chain_domain.chain_id,
        genesis_hash: chain_domain.genesis_hash,
        protocol_version: chain_domain.protocol_version,
        domain_id: &request.domain_id,
        source_chain: &domain.source_chain,
        target_chain: &domain.target_chain,
        bridge_id: &domain.bridge_id,
        door_account: &domain.door_account,
        direction: &request.direction,
        from: &request.from,
        to: &request.to,
        asset_id: &request.asset_id,
        amount: request.amount,
        witness_id: &request.witness_id,
        witness_epoch: request.witness_epoch,
        signer,
        algorithm_id,
        public_key_hex,
    }
}

fn hash_json<T: Serialize>(domain: &str, value: &T) -> Result<String, BridgeError> {
    let bytes = serde_json::to_vec(value).map_err(|error| {
        BridgeError::new(
            "serialization_failed",
            format!("failed to encode hash seed: {error}"),
        )
    })?;
    Ok(hash_hex(domain, &bytes))
}

pub fn pftl_uniswap_route_config_digest(
    config: &PftlUniswapRouteConfig,
) -> Result<String, BridgeError> {
    validate_pftl_uniswap_route_config(config)?;
    hash_json("postfiat.pftl_uniswap.route_config.v1", config)
}

pub fn pftl_uniswap_launch_config_digest(
    config: &PftlUniswapLaunchConfig,
) -> Result<String, BridgeError> {
    validate_pftl_uniswap_launch_config(config)?;
    hash_json("postfiat.pftl_uniswap.launch_config.v1", config)
}

pub fn pftl_uniswap_fork_rehearsal_evidence_digest(
    evidence: &PftlUniswapForkRehearsalEvidence,
    launch_config: &PftlUniswapLaunchConfig,
) -> Result<String, BridgeError> {
    validate_pftl_uniswap_fork_rehearsal_evidence(evidence, launch_config)?;
    hash_json("postfiat.pftl_uniswap.fork_rehearsal_evidence.v1", evidence)
}

pub fn validate_pftl_uniswap_launch_config_against_route(
    launch_config: &PftlUniswapLaunchConfig,
    route_config: &PftlUniswapRouteConfig,
) -> Result<(), BridgeError> {
    validate_pftl_uniswap_launch_config(launch_config)?;
    validate_pftl_uniswap_route_config(route_config)?;
    let route_digest = pftl_uniswap_route_config_digest(route_config)?;
    if launch_config.route_config_digest != route_digest
        || launch_config.route_id != route_config.route_id
        || launch_config.route_trust_class != route_config.route_trust_class
        || launch_config.native_nav_asset_id != route_config.native_nav_asset_id
        || launch_config.settlement_asset_id != route_config.settlement_asset_id
        || launch_config.wrapped_navcoin_token != route_config.wrapped_navcoin_token
        || launch_config.handoff_controller != route_config.handoff_controller
        || launch_config.settlement_adapter != route_config.settlement_adapter
        || launch_config.uniswap_pool_id != route_config.uniswap_pool_id_or_path
        || launch_config.seed.pricing_nav_epoch != route_config.seed_nav_epoch
        || launch_config.seed.seed_usdc_atoms != route_config.seed_usdc_atoms
        || launch_config.seed.seed_wrapped_navcoin_atoms != route_config.seed_wrapped_navcoin_atoms
        || launch_config.seed.lp_recipient != route_config.lp_recipient
        || launch_config.seed.lp_custody_policy != route_config.lp_custody_policy
    {
        return Err(BridgeError::new(
            "launch_route_config_mismatch",
            "PFTL-to-Uniswap launch config does not match the route config digest and bound fields",
        ));
    }
    Ok(())
}

pub fn primary_subscription_quote(
    input: PrimarySubscriptionQuoteInput,
) -> Result<PrimarySubscriptionQuote, BridgeError> {
    if input.settlement_value_atoms == 0 {
        return Err(BridgeError::new(
            "zero_subscription_settlement",
            "primary subscription settlement value must be nonzero",
        ));
    }
    if input.nav_price_settlement_atoms_per_nav_atom == 0 {
        return Err(BridgeError::new(
            "zero_nav_price",
            "primary subscription NAV price must be nonzero",
        ));
    }
    if input.pricing_nav_epoch == 0 {
        return Err(BridgeError::new(
            "zero_pricing_nav_epoch",
            "primary subscription pricing NAV epoch must be nonzero",
        ));
    }
    validate_lower_hex(
        "primary_subscription.pricing_reserve_packet_hash",
        &input.pricing_reserve_packet_hash,
        96,
    )?;
    let minted_nav_atoms =
        input.settlement_value_atoms / input.nav_price_settlement_atoms_per_nav_atom;
    if minted_nav_atoms == 0 {
        return Err(BridgeError::new(
            "subscription_mints_zero_nav",
            "primary subscription settlement value is below one NAV atom at the quoted price",
        ));
    }
    let dust_settlement_atoms =
        input.settlement_value_atoms % input.nav_price_settlement_atoms_per_nav_atom;
    Ok(PrimarySubscriptionQuote {
        route_family: "primary_pftl_mint".to_string(),
        supply_effect: "mints_new_native_navcoin_supply".to_string(),
        pricing_source: "finalized_pre_inflow_nav_snapshot".to_string(),
        settlement_reserve_effect: "accepted_settlement_added_after_primary_fill".to_string(),
        settlement_value_atoms: input.settlement_value_atoms,
        requested_settlement_atoms: input.settlement_value_atoms,
        accepted_settlement_atoms: input.settlement_value_atoms,
        refund_settlement_atoms: 0,
        nav_price_settlement_atoms_per_nav_atom: input.nav_price_settlement_atoms_per_nav_atom,
        pricing_nav_epoch: input.pricing_nav_epoch,
        pricing_reserve_packet_hash: input.pricing_reserve_packet_hash,
        minted_nav_atoms,
        dust_settlement_atoms,
        rounding_rule: PRIMARY_SUBSCRIPTION_ROUNDING_RULE_FLOOR_RESERVE_KEEPS_DUST.to_string(),
    })
}

pub fn pftl_uniswap_packet_id(
    packet: &PftlUniswapMintAndSwapPacket,
) -> Result<String, BridgeError> {
    validate_pftl_uniswap_packet(packet)?;
    hash_json("postfiat.pftl_uniswap.mint_and_swap_packet.v1", packet)
}

pub fn validate_pftl_uniswap_packet_against_config(
    packet: &PftlUniswapMintAndSwapPacket,
    config: &PftlUniswapRouteConfig,
) -> Result<(), BridgeError> {
    validate_pftl_uniswap_packet(packet)?;
    let expected_digest = pftl_uniswap_route_config_digest(config)?;
    if packet.config_digest != expected_digest {
        return Err(BridgeError::new(
            "route_config_digest_mismatch",
            "PFTL-to-Uniswap packet config digest does not match route config",
        ));
    }
    if config.route_trust_class == ROUTE_TRUST_CLASS_DISABLED {
        return Err(BridgeError::new(
            "route_disabled",
            "PFTL-to-Uniswap packet cannot execute while route trust class is DISABLED",
        ));
    }
    if packet.route_id != config.route_id
        || packet.settlement_asset_id != config.settlement_asset_id
        || packet.native_nav_asset_id != config.native_nav_asset_id
        || !eq_ignore_ascii_case(&packet.wrapped_navcoin_token, &config.wrapped_navcoin_token)
        || packet.uniswap_pool_id_or_path != config.uniswap_pool_id_or_path
        || !eq_ignore_ascii_case(&packet.router, &config.router)
    {
        return Err(BridgeError::new(
            "route_packet_config_mismatch",
            "PFTL-to-Uniswap packet fields do not match route config",
        ));
    }
    if packet.mint_amount_atoms > config.route_supply_cap_atoms {
        return Err(BridgeError::new(
            "route_supply_cap_exceeded",
            "PFTL-to-Uniswap packet mint amount exceeds route supply cap",
        ));
    }
    if packet.settlement_amount_atoms > config.packet_notional_cap_atoms {
        return Err(BridgeError::new(
            "packet_notional_cap_exceeded",
            "PFTL-to-Uniswap packet settlement amount exceeds packet notional cap",
        ));
    }
    Ok(())
}

pub fn validate_pftl_uniswap_packet_against_launch_config(
    packet: &PftlUniswapMintAndSwapPacket,
    launch_config: &PftlUniswapLaunchConfig,
) -> Result<(), BridgeError> {
    validate_pftl_uniswap_packet(packet)?;
    validate_pftl_uniswap_launch_config(launch_config)?;
    if packet.config_digest != launch_config.route_config_digest {
        return Err(BridgeError::new(
            "route_config_digest_mismatch",
            "PFTL-to-Uniswap packet config digest does not match launch config route digest",
        ));
    }
    if launch_config.route_trust_class == ROUTE_TRUST_CLASS_DISABLED {
        return Err(BridgeError::new(
            "route_disabled",
            "PFTL-to-Uniswap packet cannot execute while launch route trust class is DISABLED",
        ));
    }
    if packet.route_id != launch_config.route_id
        || packet.settlement_asset_id != launch_config.settlement_asset_id
        || packet.native_nav_asset_id != launch_config.native_nav_asset_id
        || !eq_ignore_ascii_case(
            &packet.wrapped_navcoin_token,
            &launch_config.wrapped_navcoin_token,
        )
        || packet.uniswap_pool_id_or_path != launch_config.uniswap_pool_id
        || !eq_ignore_ascii_case(&packet.token_out, &launch_config.usdc_token)
    {
        return Err(BridgeError::new(
            "launch_packet_config_mismatch",
            "PFTL-to-Uniswap packet fields do not match launch config",
        ));
    }
    if packet.pricing_nav_epoch != launch_config.seed.pricing_nav_epoch {
        return Err(BridgeError::new(
            "launch_pricing_nav_epoch_mismatch",
            "PFTL-to-Uniswap packet pricing NAV epoch does not match launch seed epoch",
        ));
    }
    if packet.pricing_reserve_packet_hash != launch_config.seed.pricing_reserve_packet_hash {
        return Err(BridgeError::new(
            "launch_pricing_reserve_packet_mismatch",
            "PFTL-to-Uniswap packet pricing reserve packet hash does not match launch seed reserve packet",
        ));
    }
    Ok(())
}

pub fn pftl_uniswap_bridge_ledger_hash(
    ledger: &PftlUniswapBridgeLedger,
) -> Result<String, BridgeError> {
    validate_pftl_uniswap_bridge_ledger(ledger)?;
    hash_json("postfiat.pftl_uniswap.bridge_ledger.v1", ledger)
}

pub fn pftl_uniswap_transition_receipt_hash(
    receipt: &PftlUniswapTransitionReceipt,
) -> Result<String, BridgeError> {
    validate_pftl_uniswap_transition_receipt(receipt)?;
    hash_json("postfiat.pftl_uniswap.transition_receipt.v1", receipt)
}

pub fn pftl_uniswap_return_burn_id(
    request: &PftlUniswapReturnBurnRequest,
) -> Result<String, BridgeError> {
    validate_pftl_uniswap_return_burn_request(request)?;
    pftl_uniswap_return_burn_id_from_fields(
        request.ethereum_chain_id,
        &request.bridge_controller,
        &request.wrapped_navcoin_token,
        &request.native_nav_asset_id,
        &request.ethereum_sender,
        &request.pftl_recipient,
        request.amount_atoms,
        &request.return_nonce,
        request.burn_height,
    )
    .map_err(|error| BridgeError::new("bad_return_burn_id_preimage", error))
}

pub fn pftl_uniswap_receipt_root(
    receipts: &[PftlUniswapTransitionReceipt],
) -> Result<String, BridgeError> {
    let receipt_hashes = receipts
        .iter()
        .map(pftl_uniswap_transition_receipt_hash)
        .collect::<Result<Vec<_>, _>>()?;
    pftl_uniswap_receipt_root_from_hashes(&receipt_hashes)
}

pub fn pftl_uniswap_receipt_root_from_hashes(
    receipt_hashes: &[String],
) -> Result<String, BridgeError> {
    if receipt_hashes.is_empty() {
        return Err(BridgeError::new(
            "empty_receipt_root",
            "PFTL-to-Uniswap receipt root requires at least one receipt hash",
        ));
    }
    for hash in receipt_hashes {
        validate_lower_hex("receipt_batch.receipt_hash", hash, 96)?;
    }
    let batch = PftlUniswapReceiptBatch {
        schema: "postfiat-pftl-uniswap-receipt-batch-v1".to_string(),
        receipt_hashes: receipt_hashes.to_vec(),
    };
    hash_json("postfiat.pftl_uniswap.receipt_root.v1", &batch)
}

pub fn pftl_uniswap_replay_transition_receipts(
    initial_ledger: &PftlUniswapBridgeLedger,
    receipts: &[PftlUniswapTransitionReceipt],
) -> Result<(PftlUniswapBridgeLedger, PftlUniswapReceiptReplayReport), BridgeError> {
    if receipts.is_empty() {
        return Err(BridgeError::new(
            "empty_receipt_replay",
            "PFTL-to-Uniswap receipt replay requires at least one transition receipt",
        ));
    }
    validate_pftl_uniswap_bridge_ledger(initial_ledger)?;
    let initial_ledger_hash = pftl_uniswap_bridge_ledger_hash(initial_ledger)?;
    let mut replayed = initial_ledger.clone();
    for receipt in receipts {
        validate_pftl_uniswap_transition_receipt(receipt)?;
        ensure_pftl_uniswap_receipt_matches_ledger_static_fields(&replayed, receipt)?;
        let state_before_hash = pftl_uniswap_bridge_ledger_hash(&replayed)?;
        if receipt.state_before_hash != state_before_hash {
            return Err(BridgeError::new(
                "receipt_state_chain_mismatch",
                "transition receipt state_before_hash does not match the replay ledger hash",
            ));
        }
        pftl_uniswap_replay_one_transition_receipt(&mut replayed, receipt)?;
        let state_after_hash = pftl_uniswap_bridge_ledger_hash(&replayed)?;
        if receipt.state_after_hash != state_after_hash {
            return Err(BridgeError::new(
                "receipt_state_chain_mismatch",
                "transition receipt state_after_hash does not match the replay ledger hash",
            ));
        }
    }
    let final_ledger_hash = pftl_uniswap_bridge_ledger_hash(&replayed)?;
    let receipt_root = pftl_uniswap_receipt_root(receipts)?;
    let report = PftlUniswapReceiptReplayReport {
        schema: "postfiat-pftl-uniswap-receipt-replay-report-v1".to_string(),
        route_id: initial_ledger.route_id.clone(),
        initial_ledger_hash,
        final_ledger_hash,
        receipt_root,
        receipt_count: receipts.len() as u64,
    };
    Ok((replayed, report))
}

pub fn pftl_uniswap_verify_transition_receipt_replay(
    initial_ledger: &PftlUniswapBridgeLedger,
    receipts: &[PftlUniswapTransitionReceipt],
    expected_final_ledger: &PftlUniswapBridgeLedger,
) -> Result<PftlUniswapReceiptReplayReport, BridgeError> {
    let (replayed, report) = pftl_uniswap_replay_transition_receipts(initial_ledger, receipts)?;
    validate_pftl_uniswap_bridge_ledger(expected_final_ledger)?;
    let expected_hash = pftl_uniswap_bridge_ledger_hash(expected_final_ledger)?;
    if report.final_ledger_hash != expected_hash || replayed != *expected_final_ledger {
        return Err(BridgeError::new(
            "receipt_replay_final_ledger_mismatch",
            "transition receipt replay did not reproduce the expected final bridge ledger",
        ));
    }
    Ok(report)
}

fn pftl_uniswap_replay_one_transition_receipt(
    ledger: &mut PftlUniswapBridgeLedger,
    receipt: &PftlUniswapTransitionReceipt,
) -> Result<(), BridgeError> {
    let generated = match receipt.transition.as_str() {
        "primary_subscription" => {
            let refund = required_receipt_replay_u64_allow_zero(
                "transition_receipt.refund_settlement_atoms",
                receipt.refund_settlement_atoms,
            )?;
            if refund != 0 {
                return Err(BridgeError::new(
                    "receipt_replay_unsupported_partial_primary",
                    "primary subscription receipt replay currently supports only fully accepted fills",
                ));
            }
            let request = PftlUniswapPrimarySubscriptionRequest {
                route_id: receipt.route_id.clone(),
                source_wallet: required_receipt_replay_string(
                    "transition_receipt.source_wallet",
                    &receipt.source_wallet,
                )?,
                settlement_asset_id: required_receipt_replay_string(
                    "transition_receipt.settlement_asset_id",
                    &receipt.settlement_asset_id,
                )?,
                subscription_nonce: required_receipt_replay_string(
                    "transition_receipt.nonce",
                    &receipt.nonce,
                )?,
                quote: PrimarySubscriptionQuoteInput {
                    settlement_value_atoms: required_receipt_replay_u64(
                        "transition_receipt.requested_settlement_atoms",
                        receipt.requested_settlement_atoms,
                    )?,
                    nav_price_settlement_atoms_per_nav_atom: required_receipt_replay_u64(
                        "transition_receipt.nav_price_settlement_atoms_per_nav_atom",
                        receipt.nav_price_settlement_atoms_per_nav_atom,
                    )?,
                    pricing_nav_epoch: required_receipt_replay_u64(
                        "transition_receipt.pricing_nav_epoch",
                        receipt.pricing_nav_epoch,
                    )?,
                    pricing_reserve_packet_hash: required_receipt_replay_string(
                        "transition_receipt.pricing_reserve_packet_hash",
                        &receipt.pricing_reserve_packet_hash,
                    )?,
                },
            };
            let (_, generated) =
                pftl_uniswap_apply_primary_subscription_with_receipt(ledger, request)?;
            generated
        }
        "export_debit" => {
            let request = PftlUniswapExportDebitRequest {
                route_id: receipt.route_id.clone(),
                packet_hash: required_receipt_replay_string(
                    "transition_receipt.packet_hash",
                    &receipt.packet_hash,
                )?,
                nonce: required_receipt_replay_string("transition_receipt.nonce", &receipt.nonce)?,
                source_wallet: required_receipt_replay_string(
                    "transition_receipt.source_wallet",
                    &receipt.source_wallet,
                )?,
                ethereum_recipient: required_receipt_replay_string(
                    "transition_receipt.ethereum_recipient",
                    &receipt.ethereum_recipient,
                )?,
                amount_atoms: required_receipt_replay_u64(
                    "transition_receipt.amount_atoms",
                    receipt.amount_atoms,
                )?,
                source_height: required_receipt_replay_u64(
                    "transition_receipt.source_height",
                    receipt.source_height,
                )?,
                destination_deadline_seconds: required_receipt_replay_u64(
                    "transition_receipt.destination_deadline_seconds",
                    receipt.destination_deadline_seconds,
                )?,
                refund_not_before_height: required_receipt_replay_u64(
                    "transition_receipt.refund_not_before_height",
                    receipt.refund_not_before_height,
                )?,
            };
            let (_, generated) = pftl_uniswap_export_debit_with_receipt(ledger, request)?;
            generated
        }
        "destination_consumed" => {
            let packet_hash = required_receipt_replay_string(
                "transition_receipt.packet_hash",
                &receipt.packet_hash,
            )?;
            let (_, generated) =
                pftl_uniswap_mark_destination_consumed_with_receipt(ledger, &packet_hash)?;
            generated
        }
        "source_refunded" => {
            let request = PftlUniswapRefundRequest {
                packet_hash: required_receipt_replay_string(
                    "transition_receipt.packet_hash",
                    &receipt.packet_hash,
                )?,
                current_height: required_receipt_replay_u64(
                    "transition_receipt.refund_not_before_height",
                    receipt.refund_not_before_height,
                )?,
                non_consumption_proof_hash: required_receipt_replay_string(
                    "transition_receipt.non_consumption_proof_hash",
                    &receipt.non_consumption_proof_hash,
                )?,
            };
            let (_, generated) = pftl_uniswap_refund_source_with_receipt(ledger, request)?;
            generated
        }
        "return_burn_observed" => {
            let request = PftlUniswapReturnBurnRequest {
                burn_event_hash: required_receipt_replay_string(
                    "transition_receipt.return_burn_event_hash",
                    &receipt.return_burn_event_hash,
                )?,
                ethereum_chain_id: receipt.ethereum_chain_id,
                bridge_controller: required_receipt_replay_string(
                    "transition_receipt.bridge_controller",
                    &receipt.bridge_controller,
                )?,
                wrapped_navcoin_token: receipt.wrapped_navcoin_token.clone(),
                native_nav_asset_id: receipt.native_nav_asset_id.clone(),
                ethereum_sender: required_receipt_replay_string(
                    "transition_receipt.ethereum_sender",
                    &receipt.ethereum_sender,
                )?,
                pftl_recipient: required_receipt_replay_string(
                    "transition_receipt.pftl_recipient",
                    &receipt.pftl_recipient,
                )?,
                amount_atoms: required_receipt_replay_u64(
                    "transition_receipt.amount_atoms",
                    receipt.amount_atoms,
                )?,
                return_nonce: required_receipt_replay_string(
                    "transition_receipt.nonce",
                    &receipt.nonce,
                )?,
                burn_height: required_receipt_replay_u64(
                    "transition_receipt.burn_height",
                    receipt.burn_height,
                )?,
                finalized_height: required_receipt_replay_u64(
                    "transition_receipt.finalized_height",
                    receipt.finalized_height,
                )?,
            };
            let (_, generated) = pftl_uniswap_record_return_burn_with_receipt(ledger, request)?;
            generated
        }
        "return_imported" => {
            let burn_event_hash = required_receipt_replay_string(
                "transition_receipt.return_burn_event_hash",
                &receipt.return_burn_event_hash,
            )?;
            let pftl_recipient = required_receipt_replay_string(
                "transition_receipt.pftl_recipient",
                &receipt.pftl_recipient,
            )?;
            let (_, generated) =
                pftl_uniswap_import_return_with_receipt(ledger, &burn_event_hash, &pftl_recipient)?;
            generated
        }
        _ => {
            return Err(BridgeError::new(
                "bad_transition_receipt_kind",
                "PFTL-to-Uniswap transition receipt kind is not supported",
            ));
        }
    };
    if generated != *receipt {
        return Err(BridgeError::new(
            "receipt_replay_mismatch",
            "replayed transition receipt does not match the recorded receipt",
        ));
    }
    Ok(())
}

fn ensure_pftl_uniswap_receipt_matches_ledger_static_fields(
    ledger: &PftlUniswapBridgeLedger,
    receipt: &PftlUniswapTransitionReceipt,
) -> Result<(), BridgeError> {
    if receipt.route_id != ledger.route_id
        || receipt.route_config_digest != ledger.route_config_digest
        || receipt.route_trust_class != ledger.route_trust_class
        || receipt.native_nav_asset_id != ledger.native_nav_asset_id
        || receipt.ethereum_chain_id != ledger.ethereum_chain_id
        || !eq_ignore_ascii_case(
            &receipt.wrapped_navcoin_token,
            &ledger.wrapped_navcoin_token,
        )
    {
        return Err(BridgeError::new(
            "receipt_route_mismatch",
            "transition receipt route fields do not match the replay ledger",
        ));
    }
    if let Some(settlement_asset_id) = &receipt.settlement_asset_id {
        if settlement_asset_id != &ledger.settlement_asset_id {
            return Err(BridgeError::new(
                "receipt_route_mismatch",
                "transition receipt settlement asset does not match the replay ledger",
            ));
        }
    }
    if let Some(bridge_controller) = &receipt.bridge_controller {
        if !eq_ignore_ascii_case(bridge_controller, &ledger.handoff_controller) {
            return Err(BridgeError::new(
                "receipt_bridge_controller_mismatch",
                "transition receipt bridge controller does not match the replay ledger",
            ));
        }
    }
    Ok(())
}

fn required_receipt_replay_string(
    field: &'static str,
    value: &Option<String>,
) -> Result<String, BridgeError> {
    value.clone().ok_or_else(|| {
        BridgeError::new(
            "missing_receipt_replay_field",
            format!("{field} is required for receipt replay"),
        )
    })
}

fn required_receipt_replay_u64(
    field: &'static str,
    value: Option<u64>,
) -> Result<u64, BridgeError> {
    match value {
        Some(value) if value > 0 => Ok(value),
        Some(_) => Err(BridgeError::new(
            "zero_receipt_replay_field",
            format!("{field} must be nonzero for receipt replay"),
        )),
        None => Err(BridgeError::new(
            "missing_receipt_replay_field",
            format!("{field} is required for receipt replay"),
        )),
    }
}

fn required_receipt_replay_u64_allow_zero(
    field: &'static str,
    value: Option<u64>,
) -> Result<u64, BridgeError> {
    value.ok_or_else(|| {
        BridgeError::new(
            "missing_receipt_replay_field",
            format!("{field} is required for receipt replay"),
        )
    })
}

pub fn pftl_uniswap_bridge_routes_status(
    ledgers: &[PftlUniswapBridgeLedger],
) -> Result<PftlUniswapRoutesStatusReport, BridgeError> {
    if ledgers.len() > PFTL_UNISWAP_STATUS_MAX_ROWS {
        return Err(BridgeError::new(
            "status_row_limit_exceeded",
            "PFTL-to-Uniswap route status request exceeds the status row limit",
        ));
    }
    let mut routes = ledgers
        .iter()
        .map(pftl_uniswap_route_status_row)
        .collect::<Result<Vec<_>, _>>()?;
    routes.sort_by(|left, right| left.route_id.cmp(&right.route_id));
    Ok(PftlUniswapRoutesStatusReport {
        schema: "postfiat-pftl-uniswap-routes-status-v1".to_string(),
        route_count: routes.len() as u64,
        routes,
    })
}

pub fn pftl_uniswap_bridge_supply_status(
    ledger: &PftlUniswapBridgeLedger,
) -> Result<PftlUniswapSupplyStatusReport, BridgeError> {
    validate_pftl_uniswap_bridge_ledger(ledger)?;
    let live_supply_sum_atoms = pftl_uniswap_live_supply_sum(ledger)?;
    let supply_cap_remaining_atoms = checked_sub_atoms(
        "supply_cap_remaining_atoms",
        ledger.route_supply_cap_atoms,
        ledger.authorized_valid_supply_atoms,
    )?;
    let native_spendable_balance_count = ledger.native_spendable_balances_atoms.len();
    let native_spendable_balance_sum_atoms =
        pftl_uniswap_native_spendable_balance_sum(&ledger.native_spendable_balances_atoms)?;
    Ok(PftlUniswapSupplyStatusReport {
        schema: "postfiat-pftl-uniswap-supply-status-v1".to_string(),
        route_id: ledger.route_id.clone(),
        route_config_digest: ledger.route_config_digest.clone(),
        native_nav_asset_id: ledger.native_nav_asset_id.clone(),
        settlement_asset_id: ledger.settlement_asset_id.clone(),
        wrapped_navcoin_token: ledger.wrapped_navcoin_token.clone(),
        native_spendable_balances: pftl_uniswap_native_balance_rows(
            &ledger.native_spendable_balances_atoms,
            PFTL_UNISWAP_STATUS_MAX_ROWS,
        ),
        native_spendable_balance_count: native_spendable_balance_count as u64,
        native_spendable_balance_limit: PFTL_UNISWAP_STATUS_MAX_ROWS as u64,
        native_spendable_balances_truncated: native_spendable_balance_count
            > PFTL_UNISWAP_STATUS_MAX_ROWS,
        native_spendable_balance_sum_atoms,
        authorized_valid_supply_atoms: ledger.authorized_valid_supply_atoms,
        pftl_spendable_supply_atoms: ledger.pftl_spendable_supply_atoms,
        ethereum_spendable_supply_atoms: ledger.ethereum_spendable_supply_atoms,
        other_registered_venue_supply_atoms: ledger.other_registered_venue_supply_atoms,
        outstanding_bridge_claims_atoms: ledger.outstanding_bridge_claims_atoms,
        pending_return_import_claims_atoms: ledger.pending_return_import_claims_atoms,
        live_supply_sum_atoms,
        route_supply_cap_atoms: ledger.route_supply_cap_atoms,
        supply_cap_remaining_atoms,
        packet_notional_cap_atoms: ledger.packet_notional_cap_atoms,
        settlement_reserve_atoms: ledger.settlement_reserve_atoms,
        invariant_holds: live_supply_sum_atoms == ledger.authorized_valid_supply_atoms,
        ledger_hash: pftl_uniswap_bridge_ledger_hash(ledger)?,
    })
}

pub fn pftl_uniswap_bridge_packet_status(
    ledger: &PftlUniswapBridgeLedger,
    packet_hash: &str,
) -> Result<PftlUniswapPacketStatusReport, BridgeError> {
    validate_pftl_uniswap_bridge_ledger(ledger)?;
    validate_lower_hex("navcoin_bridge_packet.packet_hash", packet_hash, 96)?;
    let packet = ledger.export_packets.get(packet_hash).ok_or_else(|| {
        BridgeError::new(
            "unknown_bridge_packet",
            "PFTL-to-Uniswap bridge packet hash is unknown",
        )
    })?;
    Ok(PftlUniswapPacketStatusReport {
        schema: "postfiat-pftl-uniswap-packet-status-v1".to_string(),
        route_id: ledger.route_id.clone(),
        route_config_digest: ledger.route_config_digest.clone(),
        packet_hash: packet_hash.to_string(),
        packet: pftl_uniswap_export_packet_status_row(packet_hash, packet),
        ledger_hash: pftl_uniswap_bridge_ledger_hash(ledger)?,
    })
}

pub fn pftl_uniswap_bridge_claims_status(
    ledger: &PftlUniswapBridgeLedger,
    limit: usize,
    include_terminal: bool,
) -> Result<PftlUniswapClaimsStatusReport, BridgeError> {
    validate_pftl_uniswap_bridge_ledger(ledger)?;
    if limit == 0 || limit > PFTL_UNISWAP_STATUS_MAX_ROWS {
        return Err(BridgeError::new(
            "bad_status_limit",
            "PFTL-to-Uniswap claims status limit must be between 1 and the status row limit",
        ));
    }
    let mut export_rows = ledger
        .export_packets
        .iter()
        .filter(|(_, packet)| {
            include_terminal || packet.status == PftlUniswapExportPacketStatus::SourceDebited
        })
        .map(|(packet_hash, packet)| pftl_uniswap_export_packet_status_row(packet_hash, packet))
        .collect::<Vec<_>>();
    let mut return_rows = ledger
        .return_burns
        .iter()
        .filter(|(_, burn)| {
            include_terminal || burn.status == PftlUniswapReturnBurnStatus::BurnObserved
        })
        .map(|(_, burn)| pftl_uniswap_return_burn_status_row(burn))
        .collect::<Vec<_>>();
    let total_export_count = export_rows.len();
    let total_return_count = return_rows.len();
    let total_rows = export_rows
        .len()
        .checked_add(return_rows.len())
        .ok_or_else(|| {
            BridgeError::new("status_row_overflow", "claims status row count overflow")
        })?;
    let truncated = total_rows > limit;
    if export_rows.len() > limit {
        export_rows.truncate(limit);
        return_rows.clear();
    } else if total_rows > limit {
        let remaining = limit - export_rows.len();
        return_rows.truncate(remaining);
    }
    Ok(PftlUniswapClaimsStatusReport {
        schema: "postfiat-pftl-uniswap-claims-status-v1".to_string(),
        route_id: ledger.route_id.clone(),
        route_config_digest: ledger.route_config_digest.clone(),
        ledger_hash: pftl_uniswap_bridge_ledger_hash(ledger)?,
        limit: limit as u64,
        truncated,
        outstanding_bridge_claims_atoms: ledger.outstanding_bridge_claims_atoms,
        pending_return_import_claims_atoms: ledger.pending_return_import_claims_atoms,
        export_claim_count: total_export_count as u64,
        return_claim_count: total_return_count as u64,
        exports: export_rows,
        returns: return_rows,
    })
}

pub fn pftl_uniswap_bridge_ledger_from_config(
    config: &PftlUniswapRouteConfig,
    ethereum_chain_id: u64,
    latest_finalized_nav_epoch: u64,
    return_finality_blocks: u64,
) -> Result<PftlUniswapBridgeLedger, BridgeError> {
    let route_config_digest = pftl_uniswap_route_config_digest(config)?;
    let ledger = PftlUniswapBridgeLedger {
        schema: "postfiat-pftl-uniswap-bridge-ledger-v1".to_string(),
        route_id: config.route_id.clone(),
        route_family: config.route_family.clone(),
        route_config_digest,
        route_trust_class: config.route_trust_class.clone(),
        native_nav_asset_id: config.native_nav_asset_id.clone(),
        settlement_asset_id: config.settlement_asset_id.clone(),
        handoff_controller: config.handoff_controller.clone(),
        settlement_adapter: config.settlement_adapter.clone(),
        wrapped_navcoin_token: config.wrapped_navcoin_token.clone(),
        ethereum_chain_id,
        route_supply_cap_atoms: config.route_supply_cap_atoms,
        packet_notional_cap_atoms: config.packet_notional_cap_atoms,
        latest_finalized_nav_epoch,
        return_finality_blocks,
        authorized_valid_supply_atoms: 0,
        pftl_spendable_supply_atoms: 0,
        native_spendable_balances_atoms: BTreeMap::new(),
        ethereum_spendable_supply_atoms: 0,
        other_registered_venue_supply_atoms: 0,
        outstanding_bridge_claims_atoms: 0,
        pending_return_import_claims_atoms: 0,
        settlement_reserve_atoms: 0,
        primary_subscription_nonces: BTreeMap::new(),
        export_packets: BTreeMap::new(),
        export_nonces: BTreeMap::new(),
        return_burns: BTreeMap::new(),
        paused: false,
    };
    validate_pftl_uniswap_bridge_ledger(&ledger)?;
    Ok(ledger)
}

pub fn validate_pftl_uniswap_bridge_ledger(
    ledger: &PftlUniswapBridgeLedger,
) -> Result<(), BridgeError> {
    validate_nonempty("bridge_ledger.schema", &ledger.schema)?;
    if ledger.schema != "postfiat-pftl-uniswap-bridge-ledger-v1" {
        return Err(BridgeError::new(
            "bad_bridge_ledger_schema",
            "PFTL-to-Uniswap bridge ledger schema is not supported",
        ));
    }
    validate_nonempty("bridge_ledger.route_id", &ledger.route_id)?;
    validate_lower_hex(
        "bridge_ledger.route_config_digest",
        &ledger.route_config_digest,
        96,
    )?;
    validate_route_trust_class(&ledger.route_trust_class)?;
    validate_route_family(&ledger.route_family)?;
    validate_lower_hex(
        "bridge_ledger.native_nav_asset_id",
        &ledger.native_nav_asset_id,
        96,
    )?;
    validate_lower_hex(
        "bridge_ledger.settlement_asset_id",
        &ledger.settlement_asset_id,
        96,
    )?;
    validate_ethereum_address(
        "bridge_ledger.handoff_controller",
        &ledger.handoff_controller,
    )?;
    validate_ethereum_address(
        "bridge_ledger.settlement_adapter",
        &ledger.settlement_adapter,
    )?;
    validate_ethereum_address(
        "bridge_ledger.wrapped_navcoin_token",
        &ledger.wrapped_navcoin_token,
    )?;
    if ledger.ethereum_chain_id == 0
        || ledger.route_supply_cap_atoms == 0
        || ledger.packet_notional_cap_atoms == 0
        || ledger.latest_finalized_nav_epoch == 0
        || ledger.return_finality_blocks == 0
    {
        return Err(BridgeError::new(
            "zero_bridge_ledger_field",
            "bridge ledger chain id, caps, pricing epoch, and return finality must be nonzero",
        ));
    }
    for (nonce, source_wallet) in &ledger.primary_subscription_nonces {
        validate_lower_hex("bridge_ledger.primary_subscription_nonce", nonce, 64)?;
        validate_nonempty(
            "bridge_ledger.primary_subscription_source_wallet",
            source_wallet,
        )?;
    }
    let native_balance_sum =
        pftl_uniswap_native_spendable_balance_sum(&ledger.native_spendable_balances_atoms)?;
    if native_balance_sum != ledger.pftl_spendable_supply_atoms {
        return Err(BridgeError::new(
            "native_balance_sum_mismatch",
            "bridge ledger native wallet balances must equal PFTL spendable supply",
        ));
    }
    let export_outstanding = ledger
        .export_packets
        .values()
        .try_fold(0_u64, |sum, packet| {
            validate_pftl_uniswap_export_packet_state(packet)?;
            if packet.status == PftlUniswapExportPacketStatus::SourceDebited {
                checked_add_atoms("export_outstanding_atoms", sum, packet.amount_atoms)
            } else {
                Ok(sum)
            }
        })?;
    if export_outstanding != ledger.outstanding_bridge_claims_atoms {
        return Err(BridgeError::new(
            "outstanding_claims_mismatch",
            "bridge ledger outstanding claims do not match source-debited export packets",
        ));
    }
    let pending_return = ledger.return_burns.values().try_fold(0_u64, |sum, burn| {
        validate_pftl_uniswap_return_burn_state(burn)?;
        if burn.status == PftlUniswapReturnBurnStatus::BurnObserved {
            checked_add_atoms("pending_return_import_claims_atoms", sum, burn.amount_atoms)
        } else {
            Ok(sum)
        }
    })?;
    if pending_return != ledger.pending_return_import_claims_atoms {
        return Err(BridgeError::new(
            "pending_return_claims_mismatch",
            "bridge ledger pending return claims do not match observed return burns",
        ));
    }
    let live_supply = checked_add_atoms(
        "bridge_live_supply_atoms",
        ledger.pftl_spendable_supply_atoms,
        ledger.ethereum_spendable_supply_atoms,
    )?;
    let live_supply = checked_add_atoms(
        "bridge_live_supply_atoms",
        live_supply,
        ledger.other_registered_venue_supply_atoms,
    )?;
    let live_supply = checked_add_atoms(
        "bridge_live_supply_atoms",
        live_supply,
        ledger.outstanding_bridge_claims_atoms,
    )?;
    let live_supply = checked_add_atoms(
        "bridge_live_supply_atoms",
        live_supply,
        ledger.pending_return_import_claims_atoms,
    )?;
    if live_supply != ledger.authorized_valid_supply_atoms {
        return Err(BridgeError::new(
            "bridge_supply_invariant_violation",
            "PFTL-to-Uniswap bridge ledger supply terms do not equal authorized valid supply",
        ));
    }
    if ledger.authorized_valid_supply_atoms > ledger.route_supply_cap_atoms {
        return Err(BridgeError::new(
            "route_supply_cap_exceeded",
            "bridge ledger authorized valid supply exceeds route supply cap",
        ));
    }
    Ok(())
}

pub fn pftl_uniswap_apply_primary_subscription_with_receipt(
    ledger: &mut PftlUniswapBridgeLedger,
    request: PftlUniswapPrimarySubscriptionRequest,
) -> Result<(PrimarySubscriptionQuote, PftlUniswapTransitionReceipt), BridgeError> {
    let state_before_hash = pftl_uniswap_bridge_ledger_hash(ledger)?;
    let source_wallet = request.source_wallet.clone();
    let settlement_asset_id = request.settlement_asset_id.clone();
    let subscription_nonce = request.subscription_nonce.clone();
    let requested_settlement_atoms = request.quote.settlement_value_atoms;
    let nav_price_settlement_atoms_per_nav_atom =
        request.quote.nav_price_settlement_atoms_per_nav_atom;
    let pricing_nav_epoch = request.quote.pricing_nav_epoch;
    let pricing_reserve_packet_hash = request.quote.pricing_reserve_packet_hash.clone();
    let quote = pftl_uniswap_apply_primary_subscription(ledger, request)?;
    let state_after_hash = pftl_uniswap_bridge_ledger_hash(ledger)?;
    let mut receipt = pftl_uniswap_base_receipt(
        ledger,
        "primary_subscription",
        state_before_hash,
        state_after_hash,
    );
    receipt.nonce = Some(subscription_nonce);
    receipt.source_wallet = Some(source_wallet);
    receipt.settlement_asset_id = Some(settlement_asset_id);
    receipt.amount_atoms = Some(quote.minted_nav_atoms);
    receipt.settlement_amount_atoms = Some(quote.accepted_settlement_atoms);
    receipt.requested_settlement_atoms = Some(requested_settlement_atoms);
    receipt.accepted_settlement_atoms = Some(quote.accepted_settlement_atoms);
    receipt.refund_settlement_atoms = Some(quote.refund_settlement_atoms);
    receipt.minted_nav_atoms = Some(quote.minted_nav_atoms);
    receipt.nav_price_settlement_atoms_per_nav_atom = Some(nav_price_settlement_atoms_per_nav_atom);
    receipt.rounding_rule = Some(quote.rounding_rule.clone());
    receipt.pricing_nav_epoch = Some(pricing_nav_epoch);
    receipt.pricing_reserve_packet_hash = Some(pricing_reserve_packet_hash);
    validate_pftl_uniswap_transition_receipt(&receipt)?;
    Ok((quote, receipt))
}

pub fn pftl_uniswap_apply_primary_subscription(
    ledger: &mut PftlUniswapBridgeLedger,
    request: PftlUniswapPrimarySubscriptionRequest,
) -> Result<PrimarySubscriptionQuote, BridgeError> {
    validate_pftl_uniswap_bridge_ledger(ledger)?;
    ensure_pftl_uniswap_route_live(ledger)?;
    validate_pftl_uniswap_primary_subscription_request(&request)?;
    if request.route_id != ledger.route_id
        || request.settlement_asset_id != ledger.settlement_asset_id
    {
        return Err(BridgeError::new(
            "primary_subscription_config_mismatch",
            "primary subscription route fields do not match bridge ledger",
        ));
    }
    if ledger.route_family != PFTL_UNISWAP_ROUTE_FAMILY_PRIMARY_MINT {
        return Err(BridgeError::new(
            "primary_subscription_route_family_mismatch",
            "primary subscription requires a primary_pftl_mint route family; secondary inventory routes cannot mint primary supply",
        ));
    }
    if ledger
        .primary_subscription_nonces
        .contains_key(&request.subscription_nonce)
    {
        return Err(BridgeError::new(
            "duplicate_primary_subscription_nonce",
            "primary subscription nonce already exists",
        ));
    }
    if request.quote.pricing_nav_epoch != ledger.latest_finalized_nav_epoch {
        return Err(BridgeError::new(
            "stale_pricing_nav_epoch",
            "primary subscription pricing epoch must equal the latest finalized NAV epoch",
        ));
    }
    let quote = primary_subscription_quote(request.quote)?;
    let authorized_valid_supply_atoms = checked_add_atoms(
        "authorized_valid_supply_atoms",
        ledger.authorized_valid_supply_atoms,
        quote.minted_nav_atoms,
    )?;
    if authorized_valid_supply_atoms > ledger.route_supply_cap_atoms {
        return Err(BridgeError::new(
            "route_supply_cap_exceeded",
            "primary subscription would exceed route supply cap",
        ));
    }
    let mut next = ledger.clone();
    next.primary_subscription_nonces.insert(
        request.subscription_nonce.clone(),
        request.source_wallet.clone(),
    );
    next.authorized_valid_supply_atoms = authorized_valid_supply_atoms;
    next.pftl_spendable_supply_atoms = checked_add_atoms(
        "pftl_spendable_supply_atoms",
        ledger.pftl_spendable_supply_atoms,
        quote.minted_nav_atoms,
    )?;
    pftl_uniswap_credit_native_spendable_balance(
        &mut next.native_spendable_balances_atoms,
        &request.source_wallet,
        quote.minted_nav_atoms,
    )?;
    next.settlement_reserve_atoms = checked_add_atoms(
        "settlement_reserve_atoms",
        ledger.settlement_reserve_atoms,
        quote.accepted_settlement_atoms,
    )?;
    validate_pftl_uniswap_bridge_ledger(&next)?;
    *ledger = next;
    Ok(quote)
}

pub fn pftl_uniswap_export_debit_with_receipt(
    ledger: &mut PftlUniswapBridgeLedger,
    request: PftlUniswapExportDebitRequest,
) -> Result<(PftlUniswapExportPacketState, PftlUniswapTransitionReceipt), BridgeError> {
    let state_before_hash = pftl_uniswap_bridge_ledger_hash(ledger)?;
    let packet = pftl_uniswap_export_debit(ledger, request)?;
    let state_after_hash = pftl_uniswap_bridge_ledger_hash(ledger)?;
    let mut receipt =
        pftl_uniswap_base_receipt(ledger, "export_debit", state_before_hash, state_after_hash);
    receipt.packet_hash = Some(packet.packet_hash.clone());
    receipt.nonce = Some(packet.nonce.clone());
    receipt.source_wallet = Some(packet.source_wallet.clone());
    receipt.ethereum_recipient = Some(packet.ethereum_recipient.clone());
    receipt.amount_atoms = Some(packet.amount_atoms);
    receipt.source_height = Some(packet.source_height);
    receipt.destination_deadline_seconds = Some(packet.destination_deadline_seconds);
    receipt.refund_not_before_height = Some(packet.refund_not_before_height);
    validate_pftl_uniswap_transition_receipt(&receipt)?;
    Ok((packet, receipt))
}

pub fn pftl_uniswap_export_debit(
    ledger: &mut PftlUniswapBridgeLedger,
    request: PftlUniswapExportDebitRequest,
) -> Result<PftlUniswapExportPacketState, BridgeError> {
    validate_pftl_uniswap_bridge_ledger(ledger)?;
    ensure_pftl_uniswap_route_live(ledger)?;
    validate_pftl_uniswap_export_request(&request)?;
    if request.route_id != ledger.route_id {
        return Err(BridgeError::new(
            "route_packet_config_mismatch",
            "export debit route id does not match bridge ledger route",
        ));
    }
    if ledger.export_packets.contains_key(&request.packet_hash) {
        return Err(BridgeError::new(
            "duplicate_export_packet",
            "export packet hash already exists",
        ));
    }
    if ledger.export_nonces.contains_key(&request.nonce) {
        return Err(BridgeError::new(
            "duplicate_export_nonce",
            "export nonce already exists",
        ));
    }
    if request.amount_atoms > ledger.packet_notional_cap_atoms {
        return Err(BridgeError::new(
            "packet_notional_cap_exceeded",
            "export debit amount exceeds packet notional cap",
        ));
    }
    if request.amount_atoms > ledger.pftl_spendable_supply_atoms {
        return Err(BridgeError::new(
            "insufficient_pftl_spendable_supply",
            "export debit amount exceeds PFTL spendable supply",
        ));
    }
    if request.amount_atoms
        > ledger
            .native_spendable_balances_atoms
            .get(&request.source_wallet)
            .copied()
            .unwrap_or(0)
    {
        return Err(BridgeError::new(
            "insufficient_native_wallet_balance",
            "export debit amount exceeds the source wallet native balance",
        ));
    }
    let packet = PftlUniswapExportPacketState {
        packet_hash: request.packet_hash.clone(),
        nonce: request.nonce.clone(),
        source_wallet: request.source_wallet,
        ethereum_recipient: request.ethereum_recipient,
        amount_atoms: request.amount_atoms,
        source_height: request.source_height,
        destination_deadline_seconds: request.destination_deadline_seconds,
        refund_not_before_height: request.refund_not_before_height,
        status: PftlUniswapExportPacketStatus::SourceDebited,
    };
    let mut next = ledger.clone();
    next.pftl_spendable_supply_atoms = checked_sub_atoms(
        "pftl_spendable_supply_atoms",
        ledger.pftl_spendable_supply_atoms,
        packet.amount_atoms,
    )?;
    pftl_uniswap_debit_native_spendable_balance(
        &mut next.native_spendable_balances_atoms,
        &packet.source_wallet,
        packet.amount_atoms,
    )?;
    next.outstanding_bridge_claims_atoms = checked_add_atoms(
        "outstanding_bridge_claims_atoms",
        ledger.outstanding_bridge_claims_atoms,
        packet.amount_atoms,
    )?;
    next.export_nonces
        .insert(packet.nonce.clone(), packet.packet_hash.clone());
    next.export_packets
        .insert(packet.packet_hash.clone(), packet.clone());
    validate_pftl_uniswap_bridge_ledger(&next)?;
    *ledger = next;
    Ok(packet)
}

pub fn pftl_uniswap_mark_destination_consumed_with_receipt(
    ledger: &mut PftlUniswapBridgeLedger,
    packet_hash: &str,
) -> Result<(PftlUniswapExportPacketState, PftlUniswapTransitionReceipt), BridgeError> {
    let state_before_hash = pftl_uniswap_bridge_ledger_hash(ledger)?;
    let packet = pftl_uniswap_mark_destination_consumed(ledger, packet_hash)?;
    let state_after_hash = pftl_uniswap_bridge_ledger_hash(ledger)?;
    let mut receipt = pftl_uniswap_base_receipt(
        ledger,
        "destination_consumed",
        state_before_hash,
        state_after_hash,
    );
    receipt.packet_hash = Some(packet.packet_hash.clone());
    receipt.nonce = Some(packet.nonce.clone());
    receipt.source_wallet = Some(packet.source_wallet.clone());
    receipt.ethereum_recipient = Some(packet.ethereum_recipient.clone());
    receipt.amount_atoms = Some(packet.amount_atoms);
    receipt.source_height = Some(packet.source_height);
    receipt.destination_deadline_seconds = Some(packet.destination_deadline_seconds);
    receipt.refund_not_before_height = Some(packet.refund_not_before_height);
    validate_pftl_uniswap_transition_receipt(&receipt)?;
    Ok((packet, receipt))
}

pub fn pftl_uniswap_mark_destination_consumed(
    ledger: &mut PftlUniswapBridgeLedger,
    packet_hash: &str,
) -> Result<PftlUniswapExportPacketState, BridgeError> {
    validate_pftl_uniswap_bridge_ledger(ledger)?;
    validate_lower_hex("destination_consume.packet_hash", packet_hash, 96)?;
    let packet = ledger
        .export_packets
        .get(packet_hash)
        .cloned()
        .ok_or_else(|| BridgeError::new("unknown_export_packet", "export packet is unknown"))?;
    if packet.status != PftlUniswapExportPacketStatus::SourceDebited {
        return Err(BridgeError::new(
            "export_packet_not_settleable",
            "only source-debited export packets can be consumed on Ethereum",
        ));
    }
    let mut consumed = packet.clone();
    consumed.status = PftlUniswapExportPacketStatus::DestinationConsumed;
    let mut next = ledger.clone();
    next.outstanding_bridge_claims_atoms = checked_sub_atoms(
        "outstanding_bridge_claims_atoms",
        ledger.outstanding_bridge_claims_atoms,
        packet.amount_atoms,
    )?;
    next.ethereum_spendable_supply_atoms = checked_add_atoms(
        "ethereum_spendable_supply_atoms",
        ledger.ethereum_spendable_supply_atoms,
        packet.amount_atoms,
    )?;
    next.export_packets
        .insert(packet_hash.to_string(), consumed.clone());
    validate_pftl_uniswap_bridge_ledger(&next)?;
    *ledger = next;
    Ok(consumed)
}

pub fn pftl_uniswap_refund_source_with_receipt(
    ledger: &mut PftlUniswapBridgeLedger,
    request: PftlUniswapRefundRequest,
) -> Result<(PftlUniswapExportPacketState, PftlUniswapTransitionReceipt), BridgeError> {
    let state_before_hash = pftl_uniswap_bridge_ledger_hash(ledger)?;
    let non_consumption_proof_hash = request.non_consumption_proof_hash.clone();
    let packet = pftl_uniswap_refund_source(ledger, request)?;
    let state_after_hash = pftl_uniswap_bridge_ledger_hash(ledger)?;
    let mut receipt = pftl_uniswap_base_receipt(
        ledger,
        "source_refunded",
        state_before_hash,
        state_after_hash,
    );
    receipt.packet_hash = Some(packet.packet_hash.clone());
    receipt.nonce = Some(packet.nonce.clone());
    receipt.source_wallet = Some(packet.source_wallet.clone());
    receipt.ethereum_recipient = Some(packet.ethereum_recipient.clone());
    receipt.amount_atoms = Some(packet.amount_atoms);
    receipt.source_height = Some(packet.source_height);
    receipt.destination_deadline_seconds = Some(packet.destination_deadline_seconds);
    receipt.refund_not_before_height = Some(packet.refund_not_before_height);
    receipt.non_consumption_proof_hash = Some(non_consumption_proof_hash);
    validate_pftl_uniswap_transition_receipt(&receipt)?;
    Ok((packet, receipt))
}

pub fn pftl_uniswap_refund_source(
    ledger: &mut PftlUniswapBridgeLedger,
    request: PftlUniswapRefundRequest,
) -> Result<PftlUniswapExportPacketState, BridgeError> {
    validate_pftl_uniswap_bridge_ledger(ledger)?;
    validate_lower_hex("refund.packet_hash", &request.packet_hash, 96)?;
    validate_lower_hex(
        "refund.non_consumption_proof_hash",
        &request.non_consumption_proof_hash,
        96,
    )?;
    if request.current_height == 0 {
        return Err(BridgeError::new(
            "zero_refund_height",
            "refund current height must be nonzero",
        ));
    }
    let packet = ledger
        .export_packets
        .get(&request.packet_hash)
        .cloned()
        .ok_or_else(|| BridgeError::new("unknown_export_packet", "export packet is unknown"))?;
    if packet.status != PftlUniswapExportPacketStatus::SourceDebited {
        return Err(BridgeError::new(
            "export_packet_not_refundable",
            "only source-debited export packets can be refunded",
        ));
    }
    let expected_proof_hash = pftl_uniswap_non_consumption_proof_hash(
        &ledger.route_id,
        &request.packet_hash,
        packet.refund_not_before_height,
    )
    .map_err(|error| BridgeError::new("bad_non_consumption_proof_preimage", error))?;
    if request.non_consumption_proof_hash != expected_proof_hash {
        return Err(BridgeError::new(
            "non_consumption_proof_mismatch",
            "refund non-consumption proof hash does not match the bound packet commitment",
        ));
    }
    if request.current_height < packet.refund_not_before_height {
        return Err(BridgeError::new(
            "refund_before_window",
            "export packet cannot be refunded before refund_not_before_height",
        ));
    }
    let mut refunded = packet.clone();
    refunded.status = PftlUniswapExportPacketStatus::SourceRefunded;
    let mut next = ledger.clone();
    next.outstanding_bridge_claims_atoms = checked_sub_atoms(
        "outstanding_bridge_claims_atoms",
        ledger.outstanding_bridge_claims_atoms,
        packet.amount_atoms,
    )?;
    next.pftl_spendable_supply_atoms = checked_add_atoms(
        "pftl_spendable_supply_atoms",
        ledger.pftl_spendable_supply_atoms,
        packet.amount_atoms,
    )?;
    pftl_uniswap_credit_native_spendable_balance(
        &mut next.native_spendable_balances_atoms,
        &packet.source_wallet,
        packet.amount_atoms,
    )?;
    next.export_packets
        .insert(request.packet_hash.clone(), refunded.clone());
    validate_pftl_uniswap_bridge_ledger(&next)?;
    *ledger = next;
    Ok(refunded)
}

pub fn pftl_uniswap_record_return_burn_with_receipt(
    ledger: &mut PftlUniswapBridgeLedger,
    request: PftlUniswapReturnBurnRequest,
) -> Result<(PftlUniswapReturnBurnState, PftlUniswapTransitionReceipt), BridgeError> {
    let state_before_hash = pftl_uniswap_bridge_ledger_hash(ledger)?;
    let burn = pftl_uniswap_record_return_burn(ledger, request)?;
    let state_after_hash = pftl_uniswap_bridge_ledger_hash(ledger)?;
    let mut receipt = pftl_uniswap_base_receipt(
        ledger,
        "return_burn_observed",
        state_before_hash,
        state_after_hash,
    );
    receipt.return_burn_event_hash = Some(burn.burn_event_hash.clone());
    receipt.bridge_controller = Some(burn.bridge_controller.clone());
    receipt.nonce = Some(burn.return_nonce.clone());
    receipt.ethereum_sender = Some(burn.ethereum_sender.clone());
    receipt.pftl_recipient = Some(burn.pftl_recipient.clone());
    receipt.amount_atoms = Some(burn.amount_atoms);
    receipt.burn_height = Some(burn.burn_height);
    receipt.finalized_height = Some(burn.finalized_height);
    validate_pftl_uniswap_transition_receipt(&receipt)?;
    Ok((burn, receipt))
}

pub fn pftl_uniswap_record_return_burn(
    ledger: &mut PftlUniswapBridgeLedger,
    request: PftlUniswapReturnBurnRequest,
) -> Result<PftlUniswapReturnBurnState, BridgeError> {
    validate_pftl_uniswap_bridge_ledger(ledger)?;
    ensure_pftl_uniswap_route_live(ledger)?;
    validate_pftl_uniswap_return_burn_request(&request)?;
    if request.ethereum_chain_id != ledger.ethereum_chain_id {
        return Err(BridgeError::new(
            "wrong_return_chain",
            "return burn Ethereum chain id does not match bridge ledger",
        ));
    }
    if !eq_ignore_ascii_case(&request.bridge_controller, &ledger.handoff_controller) {
        return Err(BridgeError::new(
            "wrong_return_bridge",
            "return burn bridge controller does not match bridge ledger",
        ));
    }
    if !eq_ignore_ascii_case(
        &request.wrapped_navcoin_token,
        &ledger.wrapped_navcoin_token,
    ) {
        return Err(BridgeError::new(
            "wrong_return_token",
            "return burn token does not match wrapped NAVCoin token",
        ));
    }
    if request.native_nav_asset_id != ledger.native_nav_asset_id {
        return Err(BridgeError::new(
            "wrong_return_asset",
            "return burn native asset id does not match bridge ledger",
        ));
    }
    let expected_burn_event_hash = pftl_uniswap_return_burn_id(&request)?;
    if expected_burn_event_hash != request.burn_event_hash {
        return Err(BridgeError::new(
            "return_burn_id_mismatch",
            "return burn event hash does not match the bound Ethereum burn fields",
        ));
    }
    if ledger.return_burns.contains_key(&request.burn_event_hash) {
        return Err(BridgeError::new(
            "duplicate_return_burn",
            "return burn event hash already exists",
        ));
    }
    let required_finalized_height = checked_add_atoms(
        "return_required_finalized_height",
        request.burn_height,
        ledger.return_finality_blocks,
    )?;
    if request.finalized_height < required_finalized_height {
        return Err(BridgeError::new(
            "return_event_below_finality",
            "return burn event is below the configured finality depth",
        ));
    }
    if request.amount_atoms > ledger.ethereum_spendable_supply_atoms {
        return Err(BridgeError::new(
            "insufficient_ethereum_spendable_supply",
            "return burn amount exceeds Ethereum spendable supply",
        ));
    }
    let burn = PftlUniswapReturnBurnState {
        burn_event_hash: request.burn_event_hash,
        ethereum_chain_id: request.ethereum_chain_id,
        bridge_controller: request.bridge_controller,
        wrapped_navcoin_token: request.wrapped_navcoin_token,
        native_nav_asset_id: request.native_nav_asset_id,
        ethereum_sender: request.ethereum_sender,
        pftl_recipient: request.pftl_recipient,
        amount_atoms: request.amount_atoms,
        return_nonce: request.return_nonce,
        burn_height: request.burn_height,
        finalized_height: request.finalized_height,
        status: PftlUniswapReturnBurnStatus::BurnObserved,
    };
    let mut next = ledger.clone();
    next.ethereum_spendable_supply_atoms = checked_sub_atoms(
        "ethereum_spendable_supply_atoms",
        ledger.ethereum_spendable_supply_atoms,
        burn.amount_atoms,
    )?;
    next.pending_return_import_claims_atoms = checked_add_atoms(
        "pending_return_import_claims_atoms",
        ledger.pending_return_import_claims_atoms,
        burn.amount_atoms,
    )?;
    next.return_burns
        .insert(burn.burn_event_hash.clone(), burn.clone());
    validate_pftl_uniswap_bridge_ledger(&next)?;
    *ledger = next;
    Ok(burn)
}

pub fn pftl_uniswap_import_return_with_receipt(
    ledger: &mut PftlUniswapBridgeLedger,
    burn_event_hash: &str,
    pftl_recipient: &str,
) -> Result<(PftlUniswapReturnBurnState, PftlUniswapTransitionReceipt), BridgeError> {
    let state_before_hash = pftl_uniswap_bridge_ledger_hash(ledger)?;
    let burn = pftl_uniswap_import_return(ledger, burn_event_hash, pftl_recipient)?;
    let state_after_hash = pftl_uniswap_bridge_ledger_hash(ledger)?;
    let mut receipt = pftl_uniswap_base_receipt(
        ledger,
        "return_imported",
        state_before_hash,
        state_after_hash,
    );
    receipt.return_burn_event_hash = Some(burn.burn_event_hash.clone());
    receipt.bridge_controller = Some(burn.bridge_controller.clone());
    receipt.nonce = Some(burn.return_nonce.clone());
    receipt.ethereum_sender = Some(burn.ethereum_sender.clone());
    receipt.pftl_recipient = Some(burn.pftl_recipient.clone());
    receipt.amount_atoms = Some(burn.amount_atoms);
    receipt.burn_height = Some(burn.burn_height);
    receipt.finalized_height = Some(burn.finalized_height);
    validate_pftl_uniswap_transition_receipt(&receipt)?;
    Ok((burn, receipt))
}

pub fn pftl_uniswap_import_return(
    ledger: &mut PftlUniswapBridgeLedger,
    burn_event_hash: &str,
    pftl_recipient: &str,
) -> Result<PftlUniswapReturnBurnState, BridgeError> {
    validate_pftl_uniswap_bridge_ledger(ledger)?;
    validate_lower_hex("return_import.burn_event_hash", burn_event_hash, 64)?;
    validate_nonempty("return_import.pftl_recipient", pftl_recipient)?;
    let burn = ledger
        .return_burns
        .get(burn_event_hash)
        .cloned()
        .ok_or_else(|| BridgeError::new("unknown_return_burn", "return burn event is unknown"))?;
    if burn.status != PftlUniswapReturnBurnStatus::BurnObserved {
        return Err(BridgeError::new(
            "return_burn_not_importable",
            "only observed return burns can be imported",
        ));
    }
    if burn.pftl_recipient != pftl_recipient {
        return Err(BridgeError::new(
            "wrong_return_recipient",
            "return import recipient does not match the burn event",
        ));
    }
    let mut imported = burn.clone();
    imported.status = PftlUniswapReturnBurnStatus::Imported;
    let mut next = ledger.clone();
    next.pending_return_import_claims_atoms = checked_sub_atoms(
        "pending_return_import_claims_atoms",
        ledger.pending_return_import_claims_atoms,
        burn.amount_atoms,
    )?;
    next.pftl_spendable_supply_atoms = checked_add_atoms(
        "pftl_spendable_supply_atoms",
        ledger.pftl_spendable_supply_atoms,
        burn.amount_atoms,
    )?;
    pftl_uniswap_credit_native_spendable_balance(
        &mut next.native_spendable_balances_atoms,
        &burn.pftl_recipient,
        burn.amount_atoms,
    )?;
    next.return_burns
        .insert(burn_event_hash.to_string(), imported.clone());
    validate_pftl_uniswap_bridge_ledger(&next)?;
    *ledger = next;
    Ok(imported)
}

fn pftl_uniswap_live_supply_sum(ledger: &PftlUniswapBridgeLedger) -> Result<u64, BridgeError> {
    let live_supply = checked_add_atoms(
        "bridge_live_supply_atoms",
        ledger.pftl_spendable_supply_atoms,
        ledger.ethereum_spendable_supply_atoms,
    )?;
    let live_supply = checked_add_atoms(
        "bridge_live_supply_atoms",
        live_supply,
        ledger.other_registered_venue_supply_atoms,
    )?;
    let live_supply = checked_add_atoms(
        "bridge_live_supply_atoms",
        live_supply,
        ledger.outstanding_bridge_claims_atoms,
    )?;
    checked_add_atoms(
        "bridge_live_supply_atoms",
        live_supply,
        ledger.pending_return_import_claims_atoms,
    )
}

fn pftl_uniswap_native_spendable_balance_sum(
    balances: &BTreeMap<String, u64>,
) -> Result<u64, BridgeError> {
    balances.iter().try_fold(0_u64, |sum, (wallet, amount)| {
        validate_nonempty("native_spendable_balance.wallet", wallet)?;
        if *amount == 0 {
            return Err(BridgeError::new(
                "zero_native_spendable_balance",
                "native spendable balance map must not contain zero-balance entries",
            ));
        }
        checked_add_atoms("native_spendable_balance_sum", sum, *amount)
    })
}

fn pftl_uniswap_credit_native_spendable_balance(
    balances: &mut BTreeMap<String, u64>,
    wallet: &str,
    amount_atoms: u64,
) -> Result<(), BridgeError> {
    validate_nonempty("native_spendable_balance.wallet", wallet)?;
    if amount_atoms == 0 {
        return Err(BridgeError::new(
            "zero_native_spendable_credit",
            "native spendable credit amount must be nonzero",
        ));
    }
    let current = balances.get(wallet).copied().unwrap_or(0);
    let next = checked_add_atoms("native_spendable_balance", current, amount_atoms)?;
    balances.insert(wallet.to_string(), next);
    Ok(())
}

fn pftl_uniswap_debit_native_spendable_balance(
    balances: &mut BTreeMap<String, u64>,
    wallet: &str,
    amount_atoms: u64,
) -> Result<(), BridgeError> {
    validate_nonempty("native_spendable_balance.wallet", wallet)?;
    if amount_atoms == 0 {
        return Err(BridgeError::new(
            "zero_native_spendable_debit",
            "native spendable debit amount must be nonzero",
        ));
    }
    let current = balances.get(wallet).copied().unwrap_or(0);
    if amount_atoms > current {
        return Err(BridgeError::new(
            "insufficient_native_wallet_balance",
            "native spendable debit amount exceeds the source wallet balance",
        ));
    }
    let next = checked_sub_atoms("native_spendable_balance", current, amount_atoms)?;
    if next == 0 {
        balances.remove(wallet);
    } else {
        balances.insert(wallet.to_string(), next);
    }
    Ok(())
}

fn pftl_uniswap_native_balance_rows(
    balances: &BTreeMap<String, u64>,
    limit: usize,
) -> Vec<PftlUniswapNativeBalanceRow> {
    balances
        .iter()
        .take(limit)
        .map(|(wallet, amount_atoms)| PftlUniswapNativeBalanceRow {
            wallet: wallet.clone(),
            amount_atoms: *amount_atoms,
        })
        .collect()
}

fn pftl_uniswap_route_status_row(
    ledger: &PftlUniswapBridgeLedger,
) -> Result<PftlUniswapRouteStatusRow, BridgeError> {
    validate_pftl_uniswap_bridge_ledger(ledger)?;
    let supply_cap_remaining_atoms = checked_sub_atoms(
        "supply_cap_remaining_atoms",
        ledger.route_supply_cap_atoms,
        ledger.authorized_valid_supply_atoms,
    )?;
    let outstanding_export_packet_count = ledger
        .export_packets
        .values()
        .filter(|packet| packet.status == PftlUniswapExportPacketStatus::SourceDebited)
        .count() as u64;
    let consumed_export_packet_count = ledger
        .export_packets
        .values()
        .filter(|packet| packet.status == PftlUniswapExportPacketStatus::DestinationConsumed)
        .count() as u64;
    let refunded_export_packet_count = ledger
        .export_packets
        .values()
        .filter(|packet| packet.status == PftlUniswapExportPacketStatus::SourceRefunded)
        .count() as u64;
    let pending_return_burn_count = ledger
        .return_burns
        .values()
        .filter(|burn| burn.status == PftlUniswapReturnBurnStatus::BurnObserved)
        .count() as u64;
    let imported_return_burn_count = ledger
        .return_burns
        .values()
        .filter(|burn| burn.status == PftlUniswapReturnBurnStatus::Imported)
        .count() as u64;
    Ok(PftlUniswapRouteStatusRow {
        route_id: ledger.route_id.clone(),
        route_family: ledger.route_family.clone(),
        route_config_digest: ledger.route_config_digest.clone(),
        route_trust_class: ledger.route_trust_class.clone(),
        route_live: !ledger.paused && ledger.route_trust_class != ROUTE_TRUST_CLASS_DISABLED,
        paused: ledger.paused,
        native_nav_asset_id: ledger.native_nav_asset_id.clone(),
        settlement_asset_id: ledger.settlement_asset_id.clone(),
        wrapped_navcoin_token: ledger.wrapped_navcoin_token.clone(),
        handoff_controller: ledger.handoff_controller.clone(),
        settlement_adapter: ledger.settlement_adapter.clone(),
        ethereum_chain_id: ledger.ethereum_chain_id,
        latest_finalized_nav_epoch: ledger.latest_finalized_nav_epoch,
        route_supply_cap_atoms: ledger.route_supply_cap_atoms,
        packet_notional_cap_atoms: ledger.packet_notional_cap_atoms,
        authorized_valid_supply_atoms: ledger.authorized_valid_supply_atoms,
        supply_cap_remaining_atoms,
        outstanding_bridge_claims_atoms: ledger.outstanding_bridge_claims_atoms,
        pending_return_import_claims_atoms: ledger.pending_return_import_claims_atoms,
        primary_subscription_count: ledger.primary_subscription_nonces.len() as u64,
        export_packet_count: ledger.export_packets.len() as u64,
        outstanding_export_packet_count,
        consumed_export_packet_count,
        refunded_export_packet_count,
        return_burn_count: ledger.return_burns.len() as u64,
        pending_return_burn_count,
        imported_return_burn_count,
        ledger_hash: pftl_uniswap_bridge_ledger_hash(ledger)?,
    })
}

fn pftl_uniswap_export_packet_status_row(
    packet_hash: &str,
    packet: &PftlUniswapExportPacketState,
) -> PftlUniswapExportPacketStatusRow {
    let claim_class = match packet.status {
        PftlUniswapExportPacketStatus::SourceDebited => "outstanding_bridge_claim",
        PftlUniswapExportPacketStatus::DestinationConsumed => "destination_consumed",
        PftlUniswapExportPacketStatus::SourceRefunded => "source_refunded",
    };
    PftlUniswapExportPacketStatusRow {
        packet_hash: packet_hash.to_string(),
        nonce: packet.nonce.clone(),
        source_wallet: packet.source_wallet.clone(),
        ethereum_recipient: packet.ethereum_recipient.clone(),
        amount_atoms: packet.amount_atoms,
        source_height: packet.source_height,
        destination_deadline_seconds: packet.destination_deadline_seconds,
        refund_not_before_height: packet.refund_not_before_height,
        status: packet.status.clone(),
        claim_class: claim_class.to_string(),
    }
}

fn pftl_uniswap_return_burn_status_row(
    burn: &PftlUniswapReturnBurnState,
) -> PftlUniswapReturnBurnStatusRow {
    let claim_class = match burn.status {
        PftlUniswapReturnBurnStatus::BurnObserved => "pending_return_import_claim",
        PftlUniswapReturnBurnStatus::Imported => "return_imported",
    };
    PftlUniswapReturnBurnStatusRow {
        burn_event_hash: burn.burn_event_hash.clone(),
        ethereum_chain_id: burn.ethereum_chain_id,
        bridge_controller: burn.bridge_controller.clone(),
        wrapped_navcoin_token: burn.wrapped_navcoin_token.clone(),
        native_nav_asset_id: burn.native_nav_asset_id.clone(),
        ethereum_sender: burn.ethereum_sender.clone(),
        pftl_recipient: burn.pftl_recipient.clone(),
        amount_atoms: burn.amount_atoms,
        return_nonce: burn.return_nonce.clone(),
        burn_height: burn.burn_height,
        finalized_height: burn.finalized_height,
        status: burn.status.clone(),
        claim_class: claim_class.to_string(),
    }
}

pub fn validate_pftl_uniswap_route_config(
    config: &PftlUniswapRouteConfig,
) -> Result<(), BridgeError> {
    validate_nonempty("route_config.schema", &config.schema)?;
    if config.schema != "postfiat-pftl-uniswap-route-config-v1" {
        return Err(BridgeError::new(
            "bad_route_config_schema",
            "PFTL-to-Uniswap route config schema is not supported",
        ));
    }
    validate_nonempty("route_config.route_id", &config.route_id)?;
    validate_route_family(&config.route_family)?;
    validate_lower_hex(
        "route_config.native_nav_asset_id",
        &config.native_nav_asset_id,
        96,
    )?;
    validate_lower_hex(
        "route_config.settlement_asset_id",
        &config.settlement_asset_id,
        96,
    )?;
    validate_ethereum_address(
        "route_config.wrapped_navcoin_token",
        &config.wrapped_navcoin_token,
    )?;
    validate_ethereum_address(
        "route_config.handoff_controller",
        &config.handoff_controller,
    )?;
    validate_ethereum_address(
        "route_config.settlement_adapter",
        &config.settlement_adapter,
    )?;
    validate_ethereum_address("route_config.router", &config.router)?;
    validate_uniswap_pool_id_or_path(
        "route_config.uniswap_pool_id_or_path",
        &config.uniswap_pool_id_or_path,
    )?;
    validate_nonempty("route_config.verifier_mode", &config.verifier_mode)?;
    validate_route_trust_class(&config.route_trust_class)?;
    if config.route_trust_class == ROUTE_TRUST_CLASS_TRUSTLESS_FINALITY
        && !verifier_mode_claims_trustless_finality(&config.verifier_mode)
    {
        return Err(BridgeError::new(
            "trustless_finality_verifier_missing",
            "TRUSTLESS_FINALITY route config requires direct or succinct PFTL finality verifier mode",
        ));
    }
    validate_nonempty("route_config.failure_behavior", &config.failure_behavior)?;
    if config.failure_behavior != "refund_unconsumed_pftl_packet"
        && config.failure_behavior != "claim_wrapped_token_if_swap_fails"
    {
        return Err(BridgeError::new(
            "unsupported_failure_behavior",
            "PFTL-to-Uniswap route failure behavior is not supported",
        ));
    }
    if config.route_supply_cap_atoms == 0 || config.packet_notional_cap_atoms == 0 {
        return Err(BridgeError::new(
            "zero_route_cap",
            "PFTL-to-Uniswap route supply and packet notional caps must be nonzero",
        ));
    }
    if config.seed_nav_epoch == 0
        || config.seed_usdc_atoms == 0
        || config.seed_wrapped_navcoin_atoms == 0
    {
        return Err(BridgeError::new(
            "zero_seed_parameter",
            "PFTL-to-Uniswap seed NAV epoch, USDC atoms, and wrapped NAVCoin atoms must be nonzero",
        ));
    }
    validate_ethereum_address("route_config.lp_recipient", &config.lp_recipient)?;
    validate_nonempty("route_config.lp_custody_policy", &config.lp_custody_policy)?;
    Ok(())
}

fn validate_pftl_uniswap_packet(packet: &PftlUniswapMintAndSwapPacket) -> Result<(), BridgeError> {
    validate_nonempty("pftl_uniswap_packet.schema", &packet.schema)?;
    if packet.schema != "postfiat-pftl-uniswap-mint-and-swap-packet-v1" {
        return Err(BridgeError::new(
            "bad_pftl_uniswap_packet_schema",
            "PFTL-to-Uniswap packet schema is not supported",
        ));
    }
    validate_nonempty("pftl_uniswap_packet.route_id", &packet.route_id)?;
    validate_lower_hex(
        "pftl_uniswap_packet.config_digest",
        &packet.config_digest,
        96,
    )?;
    validate_lower_hex(
        "pftl_uniswap_packet.source_packet_hash",
        &packet.source_packet_hash,
        96,
    )?;
    validate_lower_hex(
        "pftl_uniswap_packet.source_receipt_hash",
        &packet.source_receipt_hash,
        96,
    )?;
    validate_lower_hex(
        "pftl_uniswap_packet.source_receipt_root",
        &packet.source_receipt_root,
        96,
    )?;
    validate_nonempty("pftl_uniswap_packet.source_wallet", &packet.source_wallet)?;
    validate_lower_hex(
        "pftl_uniswap_packet.settlement_asset_id",
        &packet.settlement_asset_id,
        96,
    )?;
    validate_lower_hex(
        "pftl_uniswap_packet.native_nav_asset_id",
        &packet.native_nav_asset_id,
        96,
    )?;
    validate_ethereum_address(
        "pftl_uniswap_packet.wrapped_navcoin_token",
        &packet.wrapped_navcoin_token,
    )?;
    validate_ethereum_address(
        "pftl_uniswap_packet.ethereum_recipient",
        &packet.ethereum_recipient,
    )?;
    validate_ethereum_address("pftl_uniswap_packet.token_out", &packet.token_out)?;
    validate_uniswap_pool_id_or_path(
        "pftl_uniswap_packet.uniswap_pool_id_or_path",
        &packet.uniswap_pool_id_or_path,
    )?;
    validate_lower_hex(
        "pftl_uniswap_packet.swap_path_hash",
        &packet.swap_path_hash,
        64,
    )?;
    validate_ethereum_address("pftl_uniswap_packet.router", &packet.router)?;
    if packet.settlement_amount_atoms == 0
        || packet.mint_amount_atoms == 0
        || packet.minimum_output_atoms == 0
        || packet.deadline_seconds == 0
        || packet.pricing_nav_epoch == 0
    {
        return Err(BridgeError::new(
            "zero_pftl_uniswap_packet_field",
            "PFTL-to-Uniswap packet amounts, deadline, and pricing epoch must be nonzero",
        ));
    }
    validate_lower_hex(
        "pftl_uniswap_packet.pricing_reserve_packet_hash",
        &packet.pricing_reserve_packet_hash,
        96,
    )?;
    validate_lower_hex("pftl_uniswap_packet.nonce", &packet.nonce, 64)?;
    Ok(())
}

fn validate_pftl_uniswap_export_packet_state(
    packet: &PftlUniswapExportPacketState,
) -> Result<(), BridgeError> {
    validate_lower_hex("export_packet.packet_hash", &packet.packet_hash, 96)?;
    validate_lower_hex("export_packet.nonce", &packet.nonce, 64)?;
    validate_nonempty("export_packet.source_wallet", &packet.source_wallet)?;
    validate_ethereum_address(
        "export_packet.ethereum_recipient",
        &packet.ethereum_recipient,
    )?;
    if packet.amount_atoms == 0
        || packet.source_height == 0
        || packet.destination_deadline_seconds == 0
        || packet.refund_not_before_height == 0
    {
        return Err(BridgeError::new(
            "zero_export_packet_field",
            "export packet amount, source height, deadline, and refund height must be nonzero",
        ));
    }
    if packet.refund_not_before_height <= packet.source_height {
        return Err(BridgeError::new(
            "bad_refund_window",
            "export packet refund_not_before_height must be after source height",
        ));
    }
    Ok(())
}

fn validate_pftl_uniswap_primary_subscription_request(
    request: &PftlUniswapPrimarySubscriptionRequest,
) -> Result<(), BridgeError> {
    validate_nonempty("primary_subscription.route_id", &request.route_id)?;
    validate_nonempty("primary_subscription.source_wallet", &request.source_wallet)?;
    validate_lower_hex(
        "primary_subscription.settlement_asset_id",
        &request.settlement_asset_id,
        96,
    )?;
    validate_lower_hex(
        "primary_subscription.subscription_nonce",
        &request.subscription_nonce,
        64,
    )?;
    primary_subscription_quote(request.quote.clone())?;
    Ok(())
}

pub fn validate_pftl_uniswap_launch_config(
    config: &PftlUniswapLaunchConfig,
) -> Result<(), BridgeError> {
    validate_nonempty("launch_config.schema", &config.schema)?;
    if config.schema != "postfiat-pftl-uniswap-launch-config-v1" {
        return Err(BridgeError::new(
            "bad_launch_config_schema",
            "PFTL-to-Uniswap launch config schema is not supported",
        ));
    }
    validate_nonempty("launch_config.route_id", &config.route_id)?;
    validate_lower_hex(
        "launch_config.route_config_digest",
        &config.route_config_digest,
        96,
    )?;
    validate_route_trust_class(&config.route_trust_class)?;
    validate_lower_hex(
        "launch_config.native_nav_asset_id",
        &config.native_nav_asset_id,
        96,
    )?;
    validate_lower_hex(
        "launch_config.settlement_asset_id",
        &config.settlement_asset_id,
        96,
    )?;
    validate_ethereum_address(
        "launch_config.wrapped_navcoin_token",
        &config.wrapped_navcoin_token,
    )?;
    validate_ethereum_address("launch_config.usdc_token", &config.usdc_token)?;
    validate_ethereum_address(
        "launch_config.handoff_controller",
        &config.handoff_controller,
    )?;
    validate_ethereum_address("launch_config.receipt_verifier", &config.receipt_verifier)?;
    validate_ethereum_address(
        "launch_config.settlement_adapter",
        &config.settlement_adapter,
    )?;
    validate_pftl_uniswap_official_uniswap_deployments(
        "launch_config.official_uniswap",
        &config.official_uniswap,
    )?;
    validate_lower_hex(
        "launch_config.uniswap_pool_key_hash",
        &config.uniswap_pool_key_hash,
        64,
    )?;
    validate_uniswap_pool_id_or_path("launch_config.uniswap_pool_id", &config.uniswap_pool_id)?;
    validate_pftl_uniswap_pool_seed_config(&config.seed)?;
    if !config.fork_rehearsal_required {
        return Err(BridgeError::new(
            "fork_rehearsal_not_required",
            "PFTL-to-Uniswap launch config must require fork rehearsal before live use",
        ));
    }
    Ok(())
}

pub fn validate_pftl_uniswap_fork_rehearsal_evidence(
    evidence: &PftlUniswapForkRehearsalEvidence,
    launch_config: &PftlUniswapLaunchConfig,
) -> Result<(), BridgeError> {
    validate_pftl_uniswap_launch_config(launch_config)?;
    validate_nonempty("fork_rehearsal.schema", &evidence.schema)?;
    if evidence.schema != "postfiat-pftl-uniswap-fork-rehearsal-evidence-v1" {
        return Err(BridgeError::new(
            "bad_fork_rehearsal_schema",
            "PFTL-to-Uniswap fork rehearsal evidence schema is not supported",
        ));
    }
    validate_nonempty("fork_rehearsal.rehearsal_id", &evidence.rehearsal_id)?;
    let launch_digest = pftl_uniswap_launch_config_digest(launch_config)?;
    if evidence.launch_config_digest != launch_digest
        || evidence.route_config_digest != launch_config.route_config_digest
    {
        return Err(BridgeError::new(
            "fork_rehearsal_config_mismatch",
            "fork rehearsal evidence does not match the launch config digest",
        ));
    }
    if evidence.fork_chain_id != launch_config.official_uniswap.chain_id
        || evidence.official_uniswap != launch_config.official_uniswap
    {
        return Err(BridgeError::new(
            "fork_rehearsal_official_uniswap_mismatch",
            "fork rehearsal evidence does not match the pinned official Uniswap deployments",
        ));
    }
    if evidence.fork_block_number == 0 {
        return Err(BridgeError::new(
            "zero_fork_block",
            "fork rehearsal block number must be nonzero",
        ));
    }
    if evidence.uniswap_pool_key_hash != launch_config.uniswap_pool_key_hash
        || evidence.uniswap_pool_id != launch_config.uniswap_pool_id
    {
        return Err(BridgeError::new(
            "fork_rehearsal_pool_mismatch",
            "fork rehearsal evidence does not match the launch pool key and pool id",
        ));
    }
    validate_lower_hex(
        "fork_rehearsal.seed_export_packet_hash",
        &evidence.seed_export_packet_hash,
        96,
    )?;
    validate_lower_hex(
        "fork_rehearsal.seed_receipt_root",
        &evidence.seed_receipt_root,
        96,
    )?;
    validate_tx_hash(
        "fork_rehearsal.seed_mint_tx_hash",
        &evidence.seed_mint_tx_hash,
    )?;
    validate_tx_hash("fork_rehearsal.seed_lp_tx_hash", &evidence.seed_lp_tx_hash)?;
    validate_tx_hash(
        "fork_rehearsal.external_buy_tx_hash",
        &evidence.external_buy_tx_hash,
    )?;
    validate_tx_hash(
        "fork_rehearsal.external_sell_tx_hash",
        &evidence.external_sell_tx_hash,
    )?;
    validate_tx_hash(
        "fork_rehearsal.mint_only_packet_tx_hash",
        &evidence.mint_only_packet_tx_hash,
    )?;
    validate_tx_hash(
        "fork_rehearsal.mint_and_swap_packet_tx_hash",
        &evidence.mint_and_swap_packet_tx_hash,
    )?;
    if evidence.state_view_liquidity_after_seed == 0
        || evidence.state_view_liquidity_after_buy == 0
        || evidence.state_view_liquidity_after_sell == 0
    {
        return Err(BridgeError::new(
            "zero_state_view_liquidity",
            "fork rehearsal must show nonzero StateView liquidity after seed, buy, and sell",
        ));
    }
    if evidence.user_buy_usdc_spent_atoms == 0
        || evidence.user_buy_wrapped_received_atoms == 0
        || evidence.user_sell_wrapped_spent_atoms == 0
        || evidence.user_sell_usdc_received_atoms == 0
    {
        return Err(BridgeError::new(
            "zero_external_trade_delta",
            "fork rehearsal external buy and sell balance deltas must be nonzero",
        ));
    }
    if evidence.canonical_supply_before_external_trades_atoms == 0
        || evidence.canonical_supply_before_external_trades_atoms
            != evidence.canonical_supply_after_external_trades_atoms
    {
        return Err(BridgeError::new(
            "external_trade_supply_changed",
            "external Uniswap buy and sell must not change canonical NAVCoin supply",
        ));
    }
    if !evidence.packet_consumed_without_manual_mint {
        return Err(BridgeError::new(
            "seed_not_canonical_packet",
            "fork rehearsal must prove seed supply came from packet consume, not manual mint",
        ));
    }
    if !evidence.min_output_failure_reverted_without_consume {
        return Err(BridgeError::new(
            "swap_failure_consume_not_reverted",
            "fork rehearsal must prove min-output failure reverts without consuming the packet",
        ));
    }
    Ok(())
}

fn validate_pftl_uniswap_official_uniswap_deployments(
    field: &'static str,
    deployments: &PftlUniswapOfficialUniswapV4Deployments,
) -> Result<(), BridgeError> {
    if deployments.chain_id == 0 {
        return Err(BridgeError::new(
            "zero_uniswap_chain_id",
            format!("{field}.chain_id must be nonzero"),
        ));
    }
    validate_nonempty(
        "official_uniswap.deployments_source_url",
        &deployments.deployments_source_url,
    )?;
    validate_lower_hex(
        "official_uniswap.deployments_table_hash",
        &deployments.deployments_table_hash,
        64,
    )?;
    validate_nonempty(
        "official_uniswap.checked_at_utc",
        &deployments.checked_at_utc,
    )?;
    validate_ethereum_address("official_uniswap.pool_manager", &deployments.pool_manager)?;
    validate_ethereum_address(
        "official_uniswap.position_manager",
        &deployments.position_manager,
    )?;
    validate_ethereum_address(
        "official_uniswap.universal_router",
        &deployments.universal_router,
    )?;
    validate_ethereum_address("official_uniswap.permit2", &deployments.permit2)?;
    validate_ethereum_address("official_uniswap.state_view", &deployments.state_view)?;
    Ok(())
}

fn validate_pftl_uniswap_pool_seed_config(
    seed: &PftlUniswapPoolSeedConfig,
) -> Result<(), BridgeError> {
    if seed.pricing_nav_epoch == 0
        || seed.seed_usdc_atoms == 0
        || seed.seed_wrapped_navcoin_atoms == 0
        || seed.nav_price_settlement_atoms_per_nav_atom == 0
    {
        return Err(BridgeError::new(
            "zero_seed_field",
            "launch seed NAV epoch, USDC, wrapped NAVCoin, and NAV price must be nonzero",
        ));
    }
    validate_lower_hex(
        "launch_seed.pricing_reserve_packet_hash",
        &seed.pricing_reserve_packet_hash,
        96,
    )?;
    let expected_wrapped = seed.seed_usdc_atoms / seed.nav_price_settlement_atoms_per_nav_atom;
    if expected_wrapped == 0 || expected_wrapped != seed.seed_wrapped_navcoin_atoms {
        return Err(BridgeError::new(
            "bad_seed_nav_amount",
            "seed wrapped NAVCoin amount must equal floor(seed USDC value / seed NAV price)",
        ));
    }
    if seed.tick_lower >= seed.tick_upper {
        return Err(BridgeError::new(
            "bad_seed_tick_range",
            "launch seed tick_lower must be below tick_upper",
        ));
    }
    if seed.fee_pips == 0 {
        return Err(BridgeError::new(
            "zero_seed_fee",
            "launch seed Uniswap fee must be nonzero",
        ));
    }
    validate_ethereum_address("launch_seed.lp_recipient", &seed.lp_recipient)?;
    validate_ethereum_address("launch_seed.position_recipient", &seed.position_recipient)?;
    validate_nonempty("launch_seed.lp_custody_policy", &seed.lp_custody_policy)?;
    Ok(())
}

fn validate_pftl_uniswap_export_request(
    request: &PftlUniswapExportDebitRequest,
) -> Result<(), BridgeError> {
    validate_nonempty("export_request.route_id", &request.route_id)?;
    let packet = PftlUniswapExportPacketState {
        packet_hash: request.packet_hash.clone(),
        nonce: request.nonce.clone(),
        source_wallet: request.source_wallet.clone(),
        ethereum_recipient: request.ethereum_recipient.clone(),
        amount_atoms: request.amount_atoms,
        source_height: request.source_height,
        destination_deadline_seconds: request.destination_deadline_seconds,
        refund_not_before_height: request.refund_not_before_height,
        status: PftlUniswapExportPacketStatus::SourceDebited,
    };
    validate_pftl_uniswap_export_packet_state(&packet)
}

fn validate_pftl_uniswap_return_burn_state(
    burn: &PftlUniswapReturnBurnState,
) -> Result<(), BridgeError> {
    validate_lower_hex("return_burn.burn_event_hash", &burn.burn_event_hash, 64)?;
    if burn.ethereum_chain_id == 0 {
        return Err(BridgeError::new(
            "zero_return_chain",
            "return burn Ethereum chain id must be nonzero",
        ));
    }
    validate_ethereum_address(
        "return_burn.wrapped_navcoin_token",
        &burn.wrapped_navcoin_token,
    )?;
    validate_ethereum_address("return_burn.bridge_controller", &burn.bridge_controller)?;
    validate_ethereum_address("return_burn.ethereum_sender", &burn.ethereum_sender)?;
    validate_lower_hex(
        "return_burn.native_nav_asset_id",
        &burn.native_nav_asset_id,
        96,
    )?;
    validate_lower_hex("return_burn.return_nonce", &burn.return_nonce, 64)?;
    validate_nonempty("return_burn.pftl_recipient", &burn.pftl_recipient)?;
    if burn.amount_atoms == 0 || burn.burn_height == 0 || burn.finalized_height == 0 {
        return Err(BridgeError::new(
            "zero_return_burn_field",
            "return burn amount, burn height, and finalized height must be nonzero",
        ));
    }
    Ok(())
}

fn validate_pftl_uniswap_return_burn_request(
    request: &PftlUniswapReturnBurnRequest,
) -> Result<(), BridgeError> {
    let burn = PftlUniswapReturnBurnState {
        burn_event_hash: request.burn_event_hash.clone(),
        ethereum_chain_id: request.ethereum_chain_id,
        bridge_controller: request.bridge_controller.clone(),
        wrapped_navcoin_token: request.wrapped_navcoin_token.clone(),
        native_nav_asset_id: request.native_nav_asset_id.clone(),
        ethereum_sender: request.ethereum_sender.clone(),
        pftl_recipient: request.pftl_recipient.clone(),
        amount_atoms: request.amount_atoms,
        return_nonce: request.return_nonce.clone(),
        burn_height: request.burn_height,
        finalized_height: request.finalized_height,
        status: PftlUniswapReturnBurnStatus::BurnObserved,
    };
    validate_pftl_uniswap_return_burn_state(&burn)
}

fn validate_pftl_uniswap_transition_receipt(
    receipt: &PftlUniswapTransitionReceipt,
) -> Result<(), BridgeError> {
    validate_nonempty("transition_receipt.schema", &receipt.schema)?;
    if receipt.schema != "postfiat-pftl-uniswap-transition-receipt-v1" {
        return Err(BridgeError::new(
            "bad_transition_receipt_schema",
            "PFTL-to-Uniswap transition receipt schema is not supported",
        ));
    }
    validate_nonempty("transition_receipt.transition", &receipt.transition)?;
    match receipt.transition.as_str() {
        "primary_subscription"
        | "export_debit"
        | "destination_consumed"
        | "source_refunded"
        | "return_burn_observed"
        | "return_imported" => {}
        _ => {
            return Err(BridgeError::new(
                "bad_transition_receipt_kind",
                "PFTL-to-Uniswap transition receipt kind is not supported",
            ));
        }
    }
    validate_nonempty("transition_receipt.route_id", &receipt.route_id)?;
    validate_lower_hex(
        "transition_receipt.route_config_digest",
        &receipt.route_config_digest,
        96,
    )?;
    validate_route_trust_class(&receipt.route_trust_class)?;
    validate_optional_hex(
        "transition_receipt.settlement_asset_id",
        &receipt.settlement_asset_id,
        96,
    )?;
    validate_lower_hex(
        "transition_receipt.native_nav_asset_id",
        &receipt.native_nav_asset_id,
        96,
    )?;
    validate_ethereum_address(
        "transition_receipt.wrapped_navcoin_token",
        &receipt.wrapped_navcoin_token,
    )?;
    if receipt.ethereum_chain_id == 0 {
        return Err(BridgeError::new(
            "zero_transition_receipt_chain",
            "transition receipt Ethereum chain id must be nonzero",
        ));
    }
    validate_lower_hex(
        "transition_receipt.state_before_hash",
        &receipt.state_before_hash,
        96,
    )?;
    validate_lower_hex(
        "transition_receipt.state_after_hash",
        &receipt.state_after_hash,
        96,
    )?;
    validate_optional_hex("transition_receipt.packet_hash", &receipt.packet_hash, 96)?;
    validate_optional_hex("transition_receipt.nonce", &receipt.nonce, 64)?;
    validate_optional_hex(
        "transition_receipt.return_burn_event_hash",
        &receipt.return_burn_event_hash,
        64,
    )?;
    validate_optional_hex(
        "transition_receipt.pricing_reserve_packet_hash",
        &receipt.pricing_reserve_packet_hash,
        96,
    )?;
    validate_optional_hex(
        "transition_receipt.non_consumption_proof_hash",
        &receipt.non_consumption_proof_hash,
        96,
    )?;
    if let Some(value) = &receipt.bridge_controller {
        validate_ethereum_address("transition_receipt.bridge_controller", value)?;
    }
    if let Some(value) = &receipt.ethereum_sender {
        validate_ethereum_address("transition_receipt.ethereum_sender", value)?;
    }
    if let Some(value) = &receipt.ethereum_recipient {
        validate_ethereum_address("transition_receipt.ethereum_recipient", value)?;
    }
    if let Some(value) = &receipt.source_wallet {
        validate_nonempty("transition_receipt.source_wallet", value)?;
    }
    if let Some(value) = &receipt.pftl_recipient {
        validate_nonempty("transition_receipt.pftl_recipient", value)?;
    }
    validate_optional_nonzero("transition_receipt.amount_atoms", receipt.amount_atoms)?;
    validate_optional_nonzero(
        "transition_receipt.settlement_amount_atoms",
        receipt.settlement_amount_atoms,
    )?;
    validate_optional_nonzero(
        "transition_receipt.requested_settlement_atoms",
        receipt.requested_settlement_atoms,
    )?;
    validate_optional_nonzero(
        "transition_receipt.accepted_settlement_atoms",
        receipt.accepted_settlement_atoms,
    )?;
    validate_optional_nonzero(
        "transition_receipt.minted_nav_atoms",
        receipt.minted_nav_atoms,
    )?;
    validate_optional_nonzero(
        "transition_receipt.nav_price_settlement_atoms_per_nav_atom",
        receipt.nav_price_settlement_atoms_per_nav_atom,
    )?;
    if let Some(rounding_rule) = &receipt.rounding_rule {
        validate_nonempty("transition_receipt.rounding_rule", rounding_rule)?;
    }
    validate_primary_subscription_receipt_fields(receipt)?;
    validate_optional_nonzero(
        "transition_receipt.pricing_nav_epoch",
        receipt.pricing_nav_epoch,
    )?;
    validate_optional_nonzero("transition_receipt.source_height", receipt.source_height)?;
    validate_optional_nonzero(
        "transition_receipt.destination_deadline_seconds",
        receipt.destination_deadline_seconds,
    )?;
    validate_optional_nonzero(
        "transition_receipt.refund_not_before_height",
        receipt.refund_not_before_height,
    )?;
    validate_optional_nonzero("transition_receipt.burn_height", receipt.burn_height)?;
    validate_optional_nonzero(
        "transition_receipt.finalized_height",
        receipt.finalized_height,
    )?;
    Ok(())
}

fn validate_primary_subscription_receipt_fields(
    receipt: &PftlUniswapTransitionReceipt,
) -> Result<(), BridgeError> {
    if receipt.transition != "primary_subscription" {
        return Ok(());
    }
    let requested = required_receipt_u64(
        "transition_receipt.requested_settlement_atoms",
        receipt.requested_settlement_atoms,
    )?;
    let accepted = required_receipt_u64(
        "transition_receipt.accepted_settlement_atoms",
        receipt.accepted_settlement_atoms,
    )?;
    let refund = receipt.refund_settlement_atoms.ok_or_else(|| {
        BridgeError::new(
            "missing_primary_receipt_field",
            "primary subscription receipt must include refund_settlement_atoms",
        )
    })?;
    let minted = required_receipt_u64(
        "transition_receipt.minted_nav_atoms",
        receipt.minted_nav_atoms,
    )?;
    required_receipt_u64(
        "transition_receipt.nav_price_settlement_atoms_per_nav_atom",
        receipt.nav_price_settlement_atoms_per_nav_atom,
    )?;
    if receipt.rounding_rule.as_deref().is_none() {
        return Err(BridgeError::new(
            "missing_primary_receipt_field",
            "primary subscription receipt must include rounding_rule",
        ));
    }
    let accounted = checked_add_atoms(
        "primary_subscription_requested_settlement_accounting",
        accepted,
        refund,
    )?;
    if accounted != requested {
        return Err(BridgeError::new(
            "bad_primary_receipt_accounting",
            "primary subscription receipt accepted plus refund amount must equal requested amount",
        ));
    }
    if receipt.amount_atoms != Some(minted) || receipt.settlement_amount_atoms != Some(accepted) {
        return Err(BridgeError::new(
            "primary_receipt_alias_mismatch",
            "primary subscription receipt aliases must match explicit minted and accepted amounts",
        ));
    }
    Ok(())
}

fn required_receipt_u64(field: &'static str, value: Option<u64>) -> Result<u64, BridgeError> {
    match value {
        Some(value) if value > 0 => Ok(value),
        Some(_) => Err(BridgeError::new(
            "zero_primary_receipt_field",
            format!("{field} must be nonzero"),
        )),
        None => Err(BridgeError::new(
            "missing_primary_receipt_field",
            format!("{field} is required for primary subscription receipts"),
        )),
    }
}

fn ensure_pftl_uniswap_route_live(ledger: &PftlUniswapBridgeLedger) -> Result<(), BridgeError> {
    if ledger.paused {
        return Err(BridgeError::new(
            "route_paused",
            "PFTL-to-Uniswap bridge route is paused",
        ));
    }
    if ledger.route_trust_class == ROUTE_TRUST_CLASS_DISABLED {
        return Err(BridgeError::new(
            "route_disabled",
            "PFTL-to-Uniswap bridge route is disabled",
        ));
    }
    Ok(())
}

fn pftl_uniswap_base_receipt(
    ledger: &PftlUniswapBridgeLedger,
    transition: &str,
    state_before_hash: String,
    state_after_hash: String,
) -> PftlUniswapTransitionReceipt {
    PftlUniswapTransitionReceipt {
        schema: "postfiat-pftl-uniswap-transition-receipt-v1".to_string(),
        transition: transition.to_string(),
        route_id: ledger.route_id.clone(),
        route_config_digest: ledger.route_config_digest.clone(),
        route_trust_class: ledger.route_trust_class.clone(),
        settlement_asset_id: None,
        native_nav_asset_id: ledger.native_nav_asset_id.clone(),
        wrapped_navcoin_token: ledger.wrapped_navcoin_token.clone(),
        ethereum_chain_id: ledger.ethereum_chain_id,
        bridge_controller: None,
        packet_hash: None,
        nonce: None,
        return_burn_event_hash: None,
        source_wallet: None,
        ethereum_sender: None,
        ethereum_recipient: None,
        pftl_recipient: None,
        amount_atoms: None,
        settlement_amount_atoms: None,
        requested_settlement_atoms: None,
        accepted_settlement_atoms: None,
        refund_settlement_atoms: None,
        minted_nav_atoms: None,
        nav_price_settlement_atoms_per_nav_atom: None,
        rounding_rule: None,
        pricing_nav_epoch: None,
        pricing_reserve_packet_hash: None,
        non_consumption_proof_hash: None,
        source_height: None,
        destination_deadline_seconds: None,
        refund_not_before_height: None,
        burn_height: None,
        finalized_height: None,
        state_before_hash,
        state_after_hash,
    }
}

fn validate_optional_hex(
    field: &'static str,
    value: &Option<String>,
    expected_len: usize,
) -> Result<(), BridgeError> {
    if let Some(value) = value {
        validate_lower_hex(field, value, expected_len)?;
    }
    Ok(())
}

fn validate_optional_nonzero(field: &'static str, value: Option<u64>) -> Result<(), BridgeError> {
    if value == Some(0) {
        return Err(BridgeError::new(
            "zero_transition_receipt_field",
            format!("{field} must be nonzero when present"),
        ));
    }
    Ok(())
}

fn checked_add_atoms(field: &'static str, left: u64, right: u64) -> Result<u64, BridgeError> {
    left.checked_add(right).ok_or_else(|| {
        BridgeError::new(
            "atom_overflow",
            format!("{field} overflow while adding {left} and {right}"),
        )
    })
}

fn checked_sub_atoms(field: &'static str, left: u64, right: u64) -> Result<u64, BridgeError> {
    left.checked_sub(right).ok_or_else(|| {
        BridgeError::new(
            "atom_underflow",
            format!("{field} underflow while subtracting {right} from {left}"),
        )
    })
}

fn validate_route_trust_class(value: &str) -> Result<(), BridgeError> {
    match value {
        ROUTE_TRUST_CLASS_CONTROLLED
        | ROUTE_TRUST_CLASS_OPTIMISTIC
        | ROUTE_TRUST_CLASS_TRUSTLESS_FINALITY
        | ROUTE_TRUST_CLASS_BFT_CHECKPOINT
        | ROUTE_TRUST_CLASS_DISABLED => Ok(()),
        _ => Err(BridgeError::new(
            "bad_route_trust_class",
            "route trust class must be CONTROLLED, OPTIMISTIC, TRUSTLESS_FINALITY, BFT_CHECKPOINT, or DISABLED",
        )),
    }
}

fn validate_route_family(value: &str) -> Result<(), BridgeError> {
    match value {
        PFTL_UNISWAP_ROUTE_FAMILY_PRIMARY_MINT | PFTL_UNISWAP_ROUTE_FAMILY_SECONDARY_INVENTORY => {
            Ok(())
        }
        _ => Err(BridgeError::new(
            "bad_route_family",
            "PFTL-to-Uniswap route family must be primary_pftl_mint or secondary_inventory",
        )),
    }
}

fn verifier_mode_claims_trustless_finality(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains("direct") || lower.contains("succinct") || lower.contains("trustless")
}

fn validate_ethereum_address(field: &'static str, value: &str) -> Result<(), BridgeError> {
    validate_nonempty(field, value)?;
    if value.len() != 42
        || !value.starts_with("0x")
        || !value[2..].bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(BridgeError::new(
            "bad_ethereum_address",
            format!("{field} must be a 20-byte 0x-prefixed Ethereum address"),
        ));
    }
    Ok(())
}

fn validate_uniswap_pool_id_or_path(field: &'static str, value: &str) -> Result<(), BridgeError> {
    validate_nonempty(field, value)?;
    if let Some(hex) = value.strip_prefix("0x") {
        if hex.len() != 64 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(BridgeError::new(
                "bad_uniswap_pool_id",
                format!("{field} must be a 32-byte 0x-prefixed pool id or a nonempty path id"),
            ));
        }
    }
    Ok(())
}

fn eq_ignore_ascii_case(left: &str, right: &str) -> bool {
    left.eq_ignore_ascii_case(right)
}

mod ethereum_checkpoint;
mod ethereum_receipt;
#[cfg(test)]
mod pftl_uniswap_tests;

pub use ethereum_checkpoint::{
    verify_ethereum_checkpoint_certificate, EthereumCheckpointVerificationError,
};
pub use ethereum_receipt::{
    decode_packet_cancelled_event, decode_packet_consumed_event, decode_return_burned_event,
    ethereum_keccak256, verify_ethereum_receipt_log, verify_packet_cancelled_event,
    verify_packet_consumed_event, verify_return_burned_event, EthereumLogV1, EthereumProofError,
    PacketCancelledEventV1, PacketConsumedEventV1, ReturnBurnedEventV1,
};
pub use postfiat_types::EthereumReceiptProofV1;
