//! Pillar 7 (NAVCoin optional fee schedules) and Pillar 8 (bridge holds, DEX
//! anti-poisoning, Issuer Registry, delegated 0-PFT credentials, and asset
//! access policies) client-side transaction payload builders & proof signers.
//!
//! Matches Train A Contract §4.5 and Train B Contract §3 & §4 field-for-field
//! without modifying `crates/types/`, `crates/execution/`, or `crates/node/`.

use crate::identity_signing::{is_lower_hex_len, validate_canonical_pf_account};
use postfiat_crypto_provider::{
    address_from_public_key, bytes_to_hex, hash_hex, hex_to_bytes, ml_dsa_65_sign_with_context,
    ml_dsa_65_verify_with_context, ML_DSA_65_ALGORITHM,
};
use postfiat_rpc_sdk::{derive_wallet_key_pair, WalletBackupFile};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

// ---------------------------------------------------------------------------
// Pillar 7 constants (Train B Contract §3.1)
// ---------------------------------------------------------------------------

pub const NAV_FEE_SCHEDULE_ATTACH_TRANSACTION_KIND: &str = "nav_fee_schedule_attach";
pub const NAV_FEE_SCHEDULE_SET_TRANSACTION_KIND: &str = "nav_fee_schedule_set";
pub const NAV_FEE_SCHEDULE_CANCEL_TRANSACTION_KIND: &str = "nav_fee_schedule_cancel";
pub const NAV_FEE_CLAIM_TRANSACTION_KIND: &str = "nav_fee_claim";

pub const PROTOCOL_BLOCKS_PER_7_DAYS: u64 = 604_800;
pub const PROTOCOL_BLOCKS_PER_YEAR: u64 = 31_536_000;
pub const NAV_FEE_BPS_DENOMINATOR: u64 = 10_000;
pub const NAV_FEE_MAX_ANNUAL_RATE_BPS: u32 = 500;
pub const NAV_FEE_MAX_HWM_RATE_BPS: u32 = 5_000;
pub const NAV_FEE_MAX_SPREAD_BPS: u32 = 500;
pub const NAV_FEE_MAX_FIX_FEE_CAP_BPS: u32 = 100;
pub const NAV_FEE_MIN_INCREASE_NOTICE_BLOCKS: u64 = PROTOCOL_BLOCKS_PER_7_DAYS;
pub const NAV_FEE_MAX_DISCLOSURE_REF_BYTES: usize = 128;

// ---------------------------------------------------------------------------
// Pillar 8 constants (Train A Contract §4.4–4.5 & Train B Contract §4.2)
// ---------------------------------------------------------------------------

pub const BRIDGE_HOLD_CLAIM_TRANSACTION_KIND: &str = "bridge_hold_claim";
pub const BRIDGE_HOLD_RETURN_TRANSACTION_KIND: &str = "bridge_hold_return";
pub const OFFER_PRUNE_INELIGIBLE_TRANSACTION_KIND: &str = "offer_prune_ineligible";
pub const MAX_OFFER_PRUNE_BATCH: usize = 64;

pub const ISSUER_REGISTER_TRANSACTION_KIND: &str = "issuer_register";
pub const ISSUER_UPDATE_TRANSACTION_KIND: &str = "issuer_update";
pub const ISSUER_ADMIN_TRANSFER_TRANSACTION_KIND: &str = "issuer_admin_transfer";
pub const ISSUER_DEREGISTER_TRANSACTION_KIND: &str = "issuer_deregister";
pub const CREDENTIAL_CREATE_TRANSACTION_KIND: &str = "credential_create";
pub const CREDENTIAL_ACCEPT_TRANSACTION_KIND: &str = "credential_accept";
pub const CREDENTIAL_CREATE_AND_ACCEPT_TRANSACTION_KIND: &str = "credential_create_and_accept";
pub const CREDENTIAL_REVOKE_TRANSACTION_KIND: &str = "credential_revoke";
pub const CREDENTIAL_DELETE_TRANSACTION_KIND: &str = "credential_delete";
pub const ASSET_ACCESS_SET_TRANSACTION_KIND: &str = "asset_access_set";

pub const ASSET_BRIDGE_HOLD_KEY_DOMAIN: &str = "postfiat.asset_bridge_hold_key.v1";
pub const CREDENTIAL_ISSUER_ID_DOMAIN: &str = "postfiat.credential_issuer_id.v1";
pub const CREDENTIAL_ID_DOMAIN: &str = "postfiat.credential_id.v1";
pub const CREDENTIAL_DELEGATED_ACCEPT_DOMAIN: &str = "postfiat.credential.delegated_accept.v1";

pub const MAX_ISSUER_SIGNING_ACCOUNTS: usize = 16;
pub const MAX_ISSUER_CREDENTIAL_TYPES: usize = 32;
pub const MAX_ISSUER_VENDOR_DIDS: usize = 8;
pub const MAX_CREDENTIAL_POLICY_OR_CLAUSES: usize = 8;
pub const MAX_CREDENTIAL_POLICY_AND_PAIRS: usize = 4;
pub const ASSET_ACCESS_MIN_TIGHTEN_NOTICE_BLOCKS: u64 = PROTOCOL_BLOCKS_PER_7_DAYS;

// ---------------------------------------------------------------------------
// Pillar 7 structs & builders
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavFeeScheduleInputV1 {
    pub annual_rate_bps: u32,
    pub hwm_rate_bps: u32,
    pub issue_spread_bps: u32,
    pub redeem_spread_bps: u32,
    pub fix_fee_cap_bps: u32,
    pub recipient: String,
    pub increase_notice_blocks: u64,
    pub accrual_cap_blocks: u64,
    #[serde(default)]
    pub rates_locked: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disclosure_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavFeeScheduleOperationPayload {
    pub operation: String,
    pub issuer: String,
    pub asset_id: String,
    pub schedule: NavFeeScheduleInputV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavFeeScheduleCancelPayload {
    pub operation: String,
    pub issuer: String,
    pub asset_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavFeeClaimPayload {
    pub operation: String,
    pub recipient: String,
    pub asset_id: String,
    pub amount: u64,
}

pub fn validate_nav_fee_schedule_input(
    issuer: &str,
    schedule: &NavFeeScheduleInputV1,
) -> Result<(), String> {
    validate_canonical_pf_account("nav_fee.issuer", issuer)?;
    validate_canonical_pf_account("nav_fee.recipient", &schedule.recipient)?;
    if schedule.recipient == issuer {
        return Err("nav_fee_recipient_equals_issuer: recipient must differ from issuer".into());
    }
    if schedule.annual_rate_bps > NAV_FEE_MAX_ANNUAL_RATE_BPS {
        return Err(format!(
            "nav_fee_rate_cap_exceeded: annual_rate_bps exceeds {NAV_FEE_MAX_ANNUAL_RATE_BPS}"
        ));
    }
    if schedule.hwm_rate_bps > NAV_FEE_MAX_HWM_RATE_BPS {
        return Err(format!(
            "nav_fee_rate_cap_exceeded: hwm_rate_bps exceeds {NAV_FEE_MAX_HWM_RATE_BPS}"
        ));
    }
    if schedule.issue_spread_bps > NAV_FEE_MAX_SPREAD_BPS
        || schedule.redeem_spread_bps > NAV_FEE_MAX_SPREAD_BPS
    {
        return Err(format!(
            "nav_fee_rate_cap_exceeded: spread_bps exceeds {NAV_FEE_MAX_SPREAD_BPS}"
        ));
    }
    if schedule.fix_fee_cap_bps > NAV_FEE_MAX_FIX_FEE_CAP_BPS {
        return Err(format!(
            "nav_fee_rate_cap_exceeded: fix_fee_cap_bps exceeds {NAV_FEE_MAX_FIX_FEE_CAP_BPS}"
        ));
    }
    if schedule.increase_notice_blocks < NAV_FEE_MIN_INCREASE_NOTICE_BLOCKS {
        return Err(format!(
            "nav_fee_notice_too_short: increase_notice_blocks must be >= {NAV_FEE_MIN_INCREASE_NOTICE_BLOCKS}"
        ));
    }
    if schedule.accrual_cap_blocks == 0 {
        return Err("accrual_cap_blocks must be nonzero".to_string());
    }
    if let Some(ref disclosure) = schedule.disclosure_ref {
        if disclosure.is_empty() || disclosure.len() > NAV_FEE_MAX_DISCLOSURE_REF_BYTES {
            return Err(format!(
                "disclosure_ref must be 1..={NAV_FEE_MAX_DISCLOSURE_REF_BYTES} bytes"
            ));
        }
    }
    Ok(())
}

pub fn build_nav_fee_schedule_attach_payload(
    issuer: &str,
    asset_id: &str,
    schedule: NavFeeScheduleInputV1,
) -> Result<NavFeeScheduleOperationPayload, String> {
    if !is_lower_hex_len(asset_id, 96) {
        return Err("asset_id must be 96 lowercase hex chars".to_string());
    }
    validate_nav_fee_schedule_input(issuer, &schedule)?;
    Ok(NavFeeScheduleOperationPayload {
        operation: NAV_FEE_SCHEDULE_ATTACH_TRANSACTION_KIND.to_string(),
        issuer: issuer.to_string(),
        asset_id: asset_id.to_string(),
        schedule,
    })
}

pub fn build_nav_fee_schedule_set_payload(
    issuer: &str,
    asset_id: &str,
    schedule: NavFeeScheduleInputV1,
) -> Result<NavFeeScheduleOperationPayload, String> {
    if !is_lower_hex_len(asset_id, 96) {
        return Err("asset_id must be 96 lowercase hex chars".to_string());
    }
    validate_nav_fee_schedule_input(issuer, &schedule)?;
    Ok(NavFeeScheduleOperationPayload {
        operation: NAV_FEE_SCHEDULE_SET_TRANSACTION_KIND.to_string(),
        issuer: issuer.to_string(),
        asset_id: asset_id.to_string(),
        schedule,
    })
}

pub fn build_nav_fee_schedule_cancel_payload(
    issuer: &str,
    asset_id: &str,
) -> Result<NavFeeScheduleCancelPayload, String> {
    validate_canonical_pf_account("nav_fee_cancel.issuer", issuer)?;
    if !is_lower_hex_len(asset_id, 96) {
        return Err("asset_id must be 96 lowercase hex chars".to_string());
    }
    Ok(NavFeeScheduleCancelPayload {
        operation: NAV_FEE_SCHEDULE_CANCEL_TRANSACTION_KIND.to_string(),
        issuer: issuer.to_string(),
        asset_id: asset_id.to_string(),
    })
}

pub fn build_nav_fee_claim_payload(
    recipient: &str,
    asset_id: &str,
    amount: u64,
) -> Result<NavFeeClaimPayload, String> {
    validate_canonical_pf_account("nav_fee_claim.recipient", recipient)?;
    if !is_lower_hex_len(asset_id, 96) {
        return Err("asset_id must be 96 lowercase hex chars".to_string());
    }
    if amount == 0 {
        return Err("nav_fee_claim.amount must be nonzero".to_string());
    }
    Ok(NavFeeClaimPayload {
        operation: NAV_FEE_CLAIM_TRANSACTION_KIND.to_string(),
        recipient: recipient.to_string(),
        asset_id: asset_id.to_string(),
        amount,
    })
}

// ---------------------------------------------------------------------------
// Pillar 8 deterministic keys, builders & delegated 0-PFT accept signer
// ---------------------------------------------------------------------------

pub fn compute_asset_bridge_hold_key(asset_id: &str, recipient: &str) -> Result<String, String> {
    if !is_lower_hex_len(asset_id, 96) {
        return Err("asset_id must be 96 lowercase hex chars".to_string());
    }
    validate_canonical_pf_account("recipient", recipient)?;
    let preimage = format!("asset_id={asset_id}\nrecipient={recipient}\n");
    Ok(hash_hex(ASSET_BRIDGE_HOLD_KEY_DOMAIN, preimage.as_bytes()))
}

pub fn compute_credential_issuer_id(
    chain_id: &str,
    admin_account: &str,
    registration_nonce: u64,
) -> Result<String, String> {
    if chain_id.is_empty() {
        return Err("chain_id must be non-empty".to_string());
    }
    validate_canonical_pf_account("admin_account", admin_account)?;
    let preimage = format!(
        "chain_id={chain_id}\nadmin_account={admin_account}\nregistration_nonce={registration_nonce}\n"
    );
    Ok(hash_hex(CREDENTIAL_ISSUER_ID_DOMAIN, preimage.as_bytes()))
}

pub fn compute_credential_id(
    subject: &str,
    issuer_id: &str,
    credential_type: &str,
) -> Result<String, String> {
    validate_canonical_pf_account("subject", subject)?;
    if !is_lower_hex_len(issuer_id, 96) {
        return Err("issuer_id must be 96 lowercase hex chars".to_string());
    }
    if credential_type.trim().is_empty() || credential_type.len() > 128 {
        return Err("credential_type must be 1..=128 bytes".to_string());
    }
    let preimage =
        format!("subject={subject}\nissuer_id={issuer_id}\ncredential_type={credential_type}\n");
    Ok(hash_hex(CREDENTIAL_ID_DOMAIN, preimage.as_bytes()))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeHoldClaimPayload {
    pub operation: String,
    pub recipient: String,
    pub asset_id: String,
    pub amount: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeHoldReturnPayload {
    pub operation: String,
    pub recipient: String,
    pub asset_id: String,
    pub amount: u64,
    #[serde(default)]
    pub burn: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OfferPruneIneligiblePayload {
    pub operation: String,
    pub pruner: String,
    pub offer_ids: Vec<String>,
}

pub fn build_bridge_hold_claim_payload(
    recipient: &str,
    asset_id: &str,
    amount: u64,
) -> Result<BridgeHoldClaimPayload, String> {
    validate_canonical_pf_account("bridge_hold_claim.recipient", recipient)?;
    if !is_lower_hex_len(asset_id, 96) {
        return Err("asset_id must be 96 lowercase hex chars".to_string());
    }
    if amount == 0 {
        return Err("bridge_hold_claim.amount must be nonzero".to_string());
    }
    Ok(BridgeHoldClaimPayload {
        operation: BRIDGE_HOLD_CLAIM_TRANSACTION_KIND.to_string(),
        recipient: recipient.to_string(),
        asset_id: asset_id.to_string(),
        amount,
    })
}

pub fn build_bridge_hold_return_payload(
    recipient: &str,
    asset_id: &str,
    amount: u64,
    burn: bool,
) -> Result<BridgeHoldReturnPayload, String> {
    validate_canonical_pf_account("bridge_hold_return.recipient", recipient)?;
    if !is_lower_hex_len(asset_id, 96) {
        return Err("asset_id must be 96 lowercase hex chars".to_string());
    }
    if amount == 0 {
        return Err("bridge_hold_return.amount must be nonzero".to_string());
    }
    Ok(BridgeHoldReturnPayload {
        operation: BRIDGE_HOLD_RETURN_TRANSACTION_KIND.to_string(),
        recipient: recipient.to_string(),
        asset_id: asset_id.to_string(),
        amount,
        burn,
    })
}

pub fn build_offer_prune_ineligible_payload(
    pruner: &str,
    offer_ids: Vec<String>,
) -> Result<OfferPruneIneligiblePayload, String> {
    validate_canonical_pf_account("offer_prune_ineligible.pruner", pruner)?;
    if offer_ids.is_empty() || offer_ids.len() > MAX_OFFER_PRUNE_BATCH {
        return Err(format!(
            "offer_ids must contain 1..={MAX_OFFER_PRUNE_BATCH} distinct offer IDs"
        ));
    }
    let mut seen = BTreeSet::new();
    for id in &offer_ids {
        if !is_lower_hex_len(id, 96) {
            return Err("each offer_id must be 96 lowercase hex chars".to_string());
        }
        if !seen.insert(id.as_str()) {
            return Err("offer_ids must be distinct".to_string());
        }
    }
    Ok(OfferPruneIneligiblePayload {
        operation: OFFER_PRUNE_INELIGIBLE_TRANSACTION_KIND.to_string(),
        pruner: pruner.to_string(),
        offer_ids,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssuerRegisterPayload {
    pub operation: String,
    pub admin_account: String,
    pub domain: String,
    pub signing_accounts: Vec<String>,
    pub allowed_credential_types: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vendor_dids: Vec<String>,
    pub registration_nonce: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetApprovalMode {
    Either,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetLapsedExitRule {
    ReturnOnly,
    StrictFreezeClawback,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetWrapperMode {
    PftlOnly,
    PftlPlusEthereumFreeWrapper,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialRequirementPairV1 {
    pub issuer_id: String,
    pub credential_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialPolicyRuleV1 {
    pub all_of: Vec<CredentialRequirementPairV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialPolicyV1 {
    pub any_of: Vec<CredentialPolicyRuleV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetAccessSetPayload {
    pub operation: String,
    pub issuer: String,
    pub asset_id: String,
    #[serde(default)]
    pub disable_authorization_permanently: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_mode: Option<AssetApprovalMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lapsed_exit_rule: Option<AssetLapsedExitRule>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wrapper_mode: Option<AssetWrapperMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_policy: Option<CredentialPolicyV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notice_blocks: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emergency_remove_issuer_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialAcceptPayload {
    pub operation: String,
    pub subject: String,
    pub credential_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialDeletePayload {
    pub operation: String,
    pub caller: String,
    pub credential_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialCreateAndAcceptPayload {
    pub operation: String,
    pub signing_account: String,
    pub issuer_id: String,
    pub subject: String,
    pub credential_type: String,
    pub commitment_hash: String,
    pub expires_at_height: u64,
    pub subject_pubkey_hex: String,
    pub subject_accept_signature_hex: String,
    pub subject_accept_nonce: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DelegatedCredentialAcceptInput {
    pub chain_id: String,
    pub genesis_hash: String,
    pub subject: String,
    pub issuer_id: String,
    pub credential_type: String,
    pub commitment_hash: String,
    pub expires_at_height: u64,
    pub subject_accept_nonce: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedDelegatedCredentialAccept {
    pub domain: String,
    pub subject: String,
    pub algorithm_id: String,
    pub subject_pubkey_hex: String,
    pub subject_accept_signature_hex: String,
    pub subject_accept_nonce: u64,
}

pub fn build_issuer_register_payload(
    admin_account: &str,
    domain: &str,
    signing_accounts: Vec<String>,
    allowed_credential_types: Vec<String>,
    vendor_dids: Vec<String>,
    registration_nonce: u64,
) -> Result<IssuerRegisterPayload, String> {
    validate_canonical_pf_account("issuer_register.admin_account", admin_account)?;
    if domain.is_empty() || domain.len() > 128 || domain.bytes().any(|b| b.is_ascii_whitespace()) {
        return Err("issuer_register.domain invalid".to_string());
    }
    if signing_accounts.is_empty() || signing_accounts.len() > MAX_ISSUER_SIGNING_ACCOUNTS {
        return Err(format!(
            "issuer_limit_exceeded: signing_accounts must be 1..={MAX_ISSUER_SIGNING_ACCOUNTS}"
        ));
    }
    for acc in &signing_accounts {
        validate_canonical_pf_account("issuer_register.signing_account", acc)?;
    }
    if allowed_credential_types.is_empty()
        || allowed_credential_types.len() > MAX_ISSUER_CREDENTIAL_TYPES
    {
        return Err(format!(
            "issuer_limit_exceeded: allowed_credential_types must be 1..={MAX_ISSUER_CREDENTIAL_TYPES}"
        ));
    }
    let expected_prefix = format!("{domain}/");
    for ct in &allowed_credential_types {
        if !ct.starts_with(&expected_prefix) || ct.len() <= expected_prefix.len() || ct.len() > 128
        {
            return Err(format!(
                "issuer_invalid_credential_type: `{ct}` must be prefixed by `{expected_prefix}`"
            ));
        }
    }
    if vendor_dids.len() > MAX_ISSUER_VENDOR_DIDS {
        return Err(format!(
            "issuer_limit_exceeded: vendor_dids exceeds {MAX_ISSUER_VENDOR_DIDS}"
        ));
    }
    Ok(IssuerRegisterPayload {
        operation: ISSUER_REGISTER_TRANSACTION_KIND.to_string(),
        admin_account: admin_account.to_string(),
        domain: domain.to_string(),
        signing_accounts,
        allowed_credential_types,
        vendor_dids,
        registration_nonce,
    })
}

pub fn build_asset_access_set_payload(
    mut payload: AssetAccessSetPayload,
) -> Result<AssetAccessSetPayload, String> {
    payload.operation = ASSET_ACCESS_SET_TRANSACTION_KIND.to_string();
    validate_canonical_pf_account("asset_access_set.issuer", &payload.issuer)?;
    if !is_lower_hex_len(&payload.asset_id, 96) {
        return Err("asset_id must be 96 lowercase hex chars".to_string());
    }
    if let Some(notice) = payload.notice_blocks {
        if notice < ASSET_ACCESS_MIN_TIGHTEN_NOTICE_BLOCKS {
            return Err(format!(
                "asset_access_policy_bounds_exceeded: notice_blocks must be >= {ASSET_ACCESS_MIN_TIGHTEN_NOTICE_BLOCKS}"
            ));
        }
    }
    if let Some(ref policy) = payload.credential_policy {
        if policy.any_of.len() > MAX_CREDENTIAL_POLICY_OR_CLAUSES {
            return Err(format!(
                "asset_access_policy_bounds_exceeded: any_of exceeds {MAX_CREDENTIAL_POLICY_OR_CLAUSES}"
            ));
        }
        for rule in &policy.any_of {
            if rule.all_of.is_empty() || rule.all_of.len() > MAX_CREDENTIAL_POLICY_AND_PAIRS {
                return Err(format!(
                    "asset_access_policy_bounds_exceeded: all_of must be 1..={MAX_CREDENTIAL_POLICY_AND_PAIRS}"
                ));
            }
            for pair in &rule.all_of {
                if !is_lower_hex_len(&pair.issuer_id, 96) {
                    return Err("policy pair issuer_id must be 96 lowercase hex chars".to_string());
                }
                if pair.credential_type.is_empty() || pair.credential_type.len() > 128 {
                    return Err("policy pair credential_type invalid".to_string());
                }
            }
        }
    }
    if let Some(ref remove_id) = payload.emergency_remove_issuer_id {
        if !is_lower_hex_len(remove_id, 96) {
            return Err("emergency_remove_issuer_id must be 96 lowercase hex chars".to_string());
        }
    }
    Ok(payload)
}

pub fn build_credential_accept_payload(
    subject: &str,
    credential_id: &str,
) -> Result<CredentialAcceptPayload, String> {
    validate_canonical_pf_account("credential_accept.subject", subject)?;
    if !is_lower_hex_len(credential_id, 96) {
        return Err("credential_id must be 96 lowercase hex chars".to_string());
    }
    Ok(CredentialAcceptPayload {
        operation: CREDENTIAL_ACCEPT_TRANSACTION_KIND.to_string(),
        subject: subject.to_string(),
        credential_id: credential_id.to_string(),
    })
}

pub fn build_credential_delete_payload(
    caller: &str,
    credential_id: &str,
) -> Result<CredentialDeletePayload, String> {
    validate_canonical_pf_account("credential_delete.caller", caller)?;
    if !is_lower_hex_len(credential_id, 96) {
        return Err("credential_id must be 96 lowercase hex chars".to_string());
    }
    Ok(CredentialDeletePayload {
        operation: CREDENTIAL_DELETE_TRANSACTION_KIND.to_string(),
        caller: caller.to_string(),
        credential_id: credential_id.to_string(),
    })
}

/// Exact ML-DSA-65 preimage for delegated 0-PFT `credential_create_and_accept` (Train B §4.5).
pub fn delegated_credential_accept_preimage(input: &DelegatedCredentialAcceptInput) -> Vec<u8> {
    format!(
        "{CREDENTIAL_DELEGATED_ACCEPT_DOMAIN}\nchain_id={}\ngenesis_hash={}\nsubject={}\nissuer_id={}\ncredential_type={}\ncommitment_hash={}\nexpires_at_height={}\nsubject_accept_nonce={}\n",
        input.chain_id,
        input.genesis_hash,
        input.subject,
        input.issuer_id,
        input.credential_type,
        input.commitment_hash,
        input.expires_at_height,
        input.subject_accept_nonce
    )
    .into_bytes()
}

/// Subject signs a 0-PFT delegated credential acceptance locally with their ML-DSA-65 key.
pub fn sign_delegated_credential_accept(
    subject_backup: &WalletBackupFile,
    input: &DelegatedCredentialAcceptInput,
) -> Result<SignedDelegatedCredentialAccept, String> {
    if input.chain_id != subject_backup.chain_id {
        return Err("delegated accept chain_id does not match wallet backup".to_string());
    }
    if !is_lower_hex_len(&input.genesis_hash, 96)
        || !is_lower_hex_len(&input.issuer_id, 96)
        || !is_lower_hex_len(&input.commitment_hash, 96)
    {
        return Err("genesis_hash, issuer_id, and commitment_hash must be 96 lowercase hex".into());
    }
    if input.credential_type.is_empty() || input.expires_at_height == 0 {
        return Err("credential_type must be non-empty and expires_at_height nonzero".to_string());
    }
    let kp = derive_wallet_key_pair(subject_backup).map_err(|e| e.to_string())?;
    let derived_subject = address_from_public_key(&kp.public_key);
    if derived_subject != input.subject {
        return Err(format!(
            "subject `{}` does not match wallet address `{derived_subject}`",
            input.subject
        ));
    }
    let preimage = delegated_credential_accept_preimage(input);
    let sig = ml_dsa_65_sign_with_context(
        &kp.private_key,
        &preimage,
        CREDENTIAL_DELEGATED_ACCEPT_DOMAIN.as_bytes(),
    )
    .map_err(|e| format!("delegated accept sign failed: {e}"))?;
    let signed = SignedDelegatedCredentialAccept {
        domain: CREDENTIAL_DELEGATED_ACCEPT_DOMAIN.to_string(),
        subject: derived_subject,
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        subject_pubkey_hex: bytes_to_hex(&kp.public_key),
        subject_accept_signature_hex: bytes_to_hex(&sig),
        subject_accept_nonce: input.subject_accept_nonce,
    };
    if !verify_delegated_credential_accept(input, &signed)? {
        return Err("delegated accept self-verification failed".to_string());
    }
    Ok(signed)
}

pub fn verify_delegated_credential_accept(
    input: &DelegatedCredentialAcceptInput,
    signed: &SignedDelegatedCredentialAccept,
) -> Result<bool, String> {
    let pk = hex_to_bytes(&signed.subject_pubkey_hex).map_err(|e| e.to_string())?;
    if address_from_public_key(&pk) != input.subject || signed.subject != input.subject {
        return Ok(false);
    }
    let sig = hex_to_bytes(&signed.subject_accept_signature_hex).map_err(|e| e.to_string())?;
    let preimage = delegated_credential_accept_preimage(input);
    Ok(ml_dsa_65_verify_with_context(
        &pk,
        &preimage,
        &sig,
        CREDENTIAL_DELEGATED_ACCEPT_DOMAIN.as_bytes(),
    ))
}

pub fn build_credential_create_and_accept_payload(
    signing_account: &str,
    input: &DelegatedCredentialAcceptInput,
    signed_accept: &SignedDelegatedCredentialAccept,
) -> Result<CredentialCreateAndAcceptPayload, String> {
    validate_canonical_pf_account(
        "credential_create_and_accept.signing_account",
        signing_account,
    )?;
    if !verify_delegated_credential_accept(input, signed_accept)? {
        return Err("credential_delegated_accept_invalid".to_string());
    }
    Ok(CredentialCreateAndAcceptPayload {
        operation: CREDENTIAL_CREATE_AND_ACCEPT_TRANSACTION_KIND.to_string(),
        signing_account: signing_account.to_string(),
        issuer_id: input.issuer_id.clone(),
        subject: input.subject.clone(),
        credential_type: input.credential_type.clone(),
        commitment_hash: input.commitment_hash.clone(),
        expires_at_height: input.expires_at_height,
        subject_pubkey_hex: signed_accept.subject_pubkey_hex.clone(),
        subject_accept_signature_hex: signed_accept.subject_accept_signature_hex.clone(),
        subject_accept_nonce: input.subject_accept_nonce,
    })
}
