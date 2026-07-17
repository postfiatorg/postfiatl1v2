//! PostFiat L1 WASM wallet core — keygen, signing, address derivation for browser use.
//!
//! All cryptographic operations happen client-side. No keys leave the browser.

use wasm_bindgen::prelude::*;

use postfiat_crypto_provider::{bytes_to_hex, ml_dsa_65_sign_with_context};
use postfiat_rpc_sdk::{
    derive_wallet_key_pair, validate_owned_certificate_domain_for_wallet,
    wallet_backup_from_master_seed, wallet_fastpay_transfer_certificate_digest_v3,
    wallet_fastpay_transfer_lock_id_v1, wallet_fastpay_unwrap_certificate_digest_v3,
    wallet_fastpay_unwrap_lock_id_v1, wallet_identity_from_backup,
    wallet_sign_asset_transaction_from_fields, wallet_sign_asset_transaction_from_quote,
    wallet_sign_escrow_transaction_from_fields, wallet_sign_escrow_transaction_from_quote,
    wallet_sign_offer_transaction_from_fields, wallet_sign_offer_transaction_from_quote,
    wallet_sign_owned_deposit as sdk_wallet_sign_owned_deposit,
    wallet_sign_owned_transfer_order_v3, wallet_sign_owned_unwrap_order_v3,
    wallet_sign_payment_v2_from_fields, wallet_sign_transfer_from_fields,
    wallet_sign_transfer_from_quote, wallet_verify_fastpay_apply_ack_v1, RpcRequest, RpcResponse,
    TransferFeeQuoteSummary, WalletBackupFile, WalletSignAssetTransactionFields,
    WalletSignEscrowTransactionFields, WalletSignOfferTransactionFields, WalletSignPaymentV2Fields,
    WalletSignTransferFields,
};
use postfiat_types::{
    FastPayApplyAckV1, FastPayRecoveryCapabilitiesV1, OwnedCertificateDomain, OwnedDepositV1,
    OwnedTransferCertificateV3, OwnedTransferOrder, OwnedTransferOrderV3, OwnedUnwrapCertificateV3,
    OwnedUnwrapOrder, OwnedUnwrapOrderV3,
};
use serde::Serialize;

fn to_json_js_value<T: Serialize>(value: &T) -> Result<JsValue, JsValue> {
    let json =
        serde_json::to_string(value).map_err(|e| JsValue::from_str(&format!("serialize: {e}")))?;
    js_sys::JSON::parse(&json).map_err(|e| {
        let detail = e
            .as_string()
            .unwrap_or_else(|| "unknown parse error".to_string());
        JsValue::from_str(&format!("js parse: {detail}"))
    })
}

/// Generate a wallet backup and identity from a master seed.
///
/// Returns a JS object: { address, public_key_hex, backup_json }
#[wasm_bindgen]
pub fn wallet_keygen(
    chain_id: &str,
    master_seed_hex: &str,
    account_index: u32,
) -> Result<JsValue, JsValue> {
    let backup = wallet_backup_from_master_seed(chain_id, master_seed_hex, account_index)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
    let identity =
        wallet_identity_from_backup(&backup).map_err(|e| JsValue::from_str(&e.to_string()))?;

    let backup_json =
        serde_json::to_string(&backup).map_err(|e| JsValue::from_str(&e.to_string()))?;

    let result = js_sys::Object::new();
    js_sys::Reflect::set(&result, &"address".into(), &identity.address.clone().into())
        .map_err(|_| JsValue::from_str("reflect set failed"))?;
    js_sys::Reflect::set(
        &result,
        &"public_key_hex".into(),
        &identity.public_key_hex.clone().into(),
    )
    .map_err(|_| JsValue::from_str("reflect set failed"))?;
    js_sys::Reflect::set(&result, &"backup_json".into(), &backup_json.into())
        .map_err(|_| JsValue::from_str("reflect set failed"))?;
    Ok(result.into())
}

/// Derive only the address from a master seed (no full keygen output).
#[wasm_bindgen]
pub fn wallet_address_from_seed(
    chain_id: &str,
    master_seed_hex: &str,
    account_index: u32,
) -> Result<String, JsValue> {
    let backup = wallet_backup_from_master_seed(chain_id, master_seed_hex, account_index)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
    let identity =
        wallet_identity_from_backup(&backup).map_err(|e| JsValue::from_str(&e.to_string()))?;
    Ok(identity.address)
}

/// Sign a transfer using a fee quote from the RPC server.
///
/// backup_json: WalletBackupFile as JSON string
/// quote_json: TransferFeeQuoteSummary as JSON string
/// Returns: SignedTransfer as JS object
#[wasm_bindgen]
pub fn wallet_sign_transfer(backup_json: &str, quote_json: &str) -> Result<JsValue, JsValue> {
    let backup: WalletBackupFile = serde_json::from_str(backup_json)
        .map_err(|e| JsValue::from_str(&format!("backup parse: {e}")))?;
    let quote: TransferFeeQuoteSummary = serde_json::from_str(quote_json)
        .map_err(|e| JsValue::from_str(&format!("quote parse: {e}")))?;

    let signed = wallet_sign_transfer_from_quote(&backup, &quote)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    to_json_js_value(&signed)
}

/// Sign a transfer from explicit fields (no quote needed).
///
/// backup_json: WalletBackupFile as JSON string
/// fields_json: WalletSignTransferFields as JSON string
/// Returns: SignedTransfer as JS object
#[wasm_bindgen]
pub fn wallet_sign_transfer_fields(
    backup_json: &str,
    fields_json: &str,
) -> Result<JsValue, JsValue> {
    let backup: WalletBackupFile = serde_json::from_str(backup_json)
        .map_err(|e| JsValue::from_str(&format!("backup parse: {e}")))?;
    let fields: WalletSignTransferFields = serde_json::from_str(fields_json)
        .map_err(|e| JsValue::from_str(&format!("fields parse: {e}")))?;

    let signed = wallet_sign_transfer_from_fields(&backup, fields)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    to_json_js_value(&signed)
}

/// Sign a payment v2 (transfer with memos).
///
/// backup_json: WalletBackupFile as JSON string
/// fields_json: WalletSignPaymentV2Fields as JSON string
/// Returns: SignedPaymentV2 as JS object
#[wasm_bindgen]
pub fn wallet_sign_payment_v2(backup_json: &str, fields_json: &str) -> Result<JsValue, JsValue> {
    let backup: WalletBackupFile = serde_json::from_str(backup_json)
        .map_err(|e| JsValue::from_str(&format!("backup parse: {e}")))?;
    let fields: WalletSignPaymentV2Fields = serde_json::from_str(fields_json)
        .map_err(|e| JsValue::from_str(&format!("fields parse: {e}")))?;

    let signed = wallet_sign_payment_v2_from_fields(&backup, fields)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    to_json_js_value(&signed)
}

/// Sign an account-to-FastPay deposit locally and return the consensus primary
/// transaction. The wallet backup never crosses the browser boundary.
#[wasm_bindgen]
pub fn wallet_sign_owned_deposit(
    backup_json: &str,
    deposit_json: &str,
) -> Result<JsValue, JsValue> {
    let backup: WalletBackupFile = serde_json::from_str(backup_json)
        .map_err(|e| JsValue::from_str(&format!("backup parse: {e}")))?;
    let deposit: OwnedDepositV1 = serde_json::from_str(deposit_json)
        .map_err(|e| JsValue::from_str(&format!("owned deposit parse: {e}")))?;
    let transaction = sdk_wallet_sign_owned_deposit(&backup, deposit)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
    to_json_js_value(&transaction)
}

/// Sign an asset transaction using a fee quote from the RPC server.
///
/// backup_json: WalletBackupFile as JSON string
/// quote_json: raw RPC response JSON string containing AssetFeeQuoteSummary
/// Returns: SignedAssetTransaction as JS object
#[wasm_bindgen]
pub fn wallet_sign_asset_transaction(
    backup_json: &str,
    quote_json: &str,
) -> Result<JsValue, JsValue> {
    let backup: WalletBackupFile = serde_json::from_str(backup_json)
        .map_err(|e| JsValue::from_str(&format!("backup parse: {e}")))?;
    let quote_response: RpcResponse = serde_json::from_str(quote_json)
        .map_err(|e| JsValue::from_str(&format!("quote parse: {e}")))?;

    let signed = wallet_sign_asset_transaction_from_quote(&backup, &quote_response)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    to_json_js_value(&signed)
}

/// Sign an asset transaction from explicit fields (no quote needed).
///
/// backup_json: WalletBackupFile as JSON string
/// fields_json: WalletSignAssetTransactionFields as JSON string
/// Returns: SignedAssetTransaction as JS object
#[wasm_bindgen]
pub fn wallet_sign_asset_transaction_fields(
    backup_json: &str,
    fields_json: &str,
) -> Result<JsValue, JsValue> {
    let backup: WalletBackupFile = serde_json::from_str(backup_json)
        .map_err(|e| JsValue::from_str(&format!("backup parse: {e}")))?;
    let fields: WalletSignAssetTransactionFields = serde_json::from_str(fields_json)
        .map_err(|e| JsValue::from_str(&format!("fields parse: {e}")))?;

    let signed = wallet_sign_asset_transaction_from_fields(&backup, fields)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    to_json_js_value(&signed)
}

/// Sign an escrow transaction using a fee quote from the RPC server.
///
/// backup_json: WalletBackupFile as JSON string
/// quote_json: raw RPC response JSON string containing EscrowFeeQuoteSummary
/// Returns: SignedEscrowTransaction as JS object
#[wasm_bindgen]
pub fn wallet_sign_escrow_transaction(
    backup_json: &str,
    quote_json: &str,
) -> Result<JsValue, JsValue> {
    let backup: WalletBackupFile = serde_json::from_str(backup_json)
        .map_err(|e| JsValue::from_str(&format!("backup parse: {e}")))?;
    let quote_response: RpcResponse = serde_json::from_str(quote_json)
        .map_err(|e| JsValue::from_str(&format!("quote parse: {e}")))?;

    let signed = wallet_sign_escrow_transaction_from_quote(&backup, &quote_response)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    to_json_js_value(&signed)
}

/// Sign an escrow transaction from explicit fields (no quote needed).
///
/// backup_json: WalletBackupFile as JSON string
/// fields_json: WalletSignEscrowTransactionFields as JSON string
/// Returns: SignedEscrowTransaction as JS object
#[wasm_bindgen]
pub fn wallet_sign_escrow_transaction_fields(
    backup_json: &str,
    fields_json: &str,
) -> Result<JsValue, JsValue> {
    let backup: WalletBackupFile = serde_json::from_str(backup_json)
        .map_err(|e| JsValue::from_str(&format!("backup parse: {e}")))?;
    let fields: WalletSignEscrowTransactionFields = serde_json::from_str(fields_json)
        .map_err(|e| JsValue::from_str(&format!("fields parse: {e}")))?;

    let signed = wallet_sign_escrow_transaction_from_fields(&backup, fields)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    to_json_js_value(&signed)
}

/// Sign an offer transaction using a fee quote from the RPC server.
///
/// backup_json: WalletBackupFile as JSON string
/// quote_json: raw RPC response JSON string containing OfferFeeQuoteSummary
/// Returns: SignedOfferTransaction as JS object
#[wasm_bindgen]
pub fn wallet_sign_offer_transaction(
    backup_json: &str,
    quote_json: &str,
) -> Result<JsValue, JsValue> {
    let backup: WalletBackupFile = serde_json::from_str(backup_json)
        .map_err(|e| JsValue::from_str(&format!("backup parse: {e}")))?;
    let quote_response: RpcResponse = serde_json::from_str(quote_json)
        .map_err(|e| JsValue::from_str(&format!("quote parse: {e}")))?;

    let signed = wallet_sign_offer_transaction_from_quote(&backup, &quote_response)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    to_json_js_value(&signed)
}

/// Sign an offer transaction from explicit fields (no quote needed).
///
/// backup_json: WalletBackupFile as JSON string
/// fields_json: WalletSignOfferTransactionFields as JSON string
/// Returns: SignedOfferTransaction as JS object
#[wasm_bindgen]
pub fn wallet_sign_offer_transaction_fields(
    backup_json: &str,
    fields_json: &str,
) -> Result<JsValue, JsValue> {
    let backup: WalletBackupFile = serde_json::from_str(backup_json)
        .map_err(|e| JsValue::from_str(&format!("backup parse: {e}")))?;
    let fields: WalletSignOfferTransactionFields = serde_json::from_str(fields_json)
        .map_err(|e| JsValue::from_str(&format!("fields parse: {e}")))?;

    let signed = wallet_sign_offer_transaction_from_fields(&backup, fields)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    to_json_js_value(&signed)
}

/// Sign a FastPay owned-transfer order with the wallet owner's key.
///
/// backup_json: WalletBackupFile as JSON string
/// order_json: OwnedTransferOrder as JSON string
/// Returns: JS object { owner_pubkey_hex, owner_signature_hex, order }
#[wasm_bindgen]
pub fn wallet_sign_owned_transfer(backup_json: &str, order_json: &str) -> Result<JsValue, JsValue> {
    let backup: WalletBackupFile = serde_json::from_str(backup_json)
        .map_err(|e| JsValue::from_str(&format!("backup parse: {e}")))?;
    let order: OwnedTransferOrder = serde_json::from_str(order_json)
        .map_err(|e| JsValue::from_str(&format!("order parse: {e}")))?;
    validate_owned_certificate_domain_for_wallet(&backup, &order.domain)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    let key_pair =
        derive_wallet_key_pair(&backup).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let owner_pubkey_hex = bytes_to_hex(&key_pair.public_key);

    // Compute canonical signing bytes (same logic as execution::owned_transfer_signing_bytes)
    let signing_bytes = owned_transfer_signing_bytes(&order);

    // Sign with ML-DSA-65 using the owned-transfer context
    const OWNED_TRANSFER_CONTEXT: &[u8] = b"postfiat-l1-v2/owned-transfer/v2";
    let signature = ml_dsa_65_sign_with_context(
        &key_pair.private_key,
        &signing_bytes,
        OWNED_TRANSFER_CONTEXT,
    )
    .map_err(|e| JsValue::from_str(&format!("sign failed: {e}")))?;

    let owner_signature_hex = bytes_to_hex(&signature);

    let result = js_sys::Object::new();
    js_sys::Reflect::set(
        &result,
        &"owner_pubkey_hex".into(),
        &owner_pubkey_hex.clone().into(),
    )
    .map_err(|_| JsValue::from_str("reflect set failed"))?;
    js_sys::Reflect::set(
        &result,
        &"owner_signature_hex".into(),
        &owner_signature_hex.clone().into(),
    )
    .map_err(|_| JsValue::from_str("reflect set failed"))?;
    js_sys::Reflect::set(&result, &"order".into(), &order_json.into())
        .map_err(|_| JsValue::from_str("reflect set failed"))?;
    Ok(result.into())
}

/// Sign a FastPay owned-unwrap order with the wallet owner's key.
///
/// backup_json: WalletBackupFile as JSON string
/// order_json: OwnedUnwrapOrder as JSON string
/// Returns: JS object { owner_pubkey_hex, owner_signature_hex, order }
#[wasm_bindgen]
pub fn wallet_sign_owned_unwrap(backup_json: &str, order_json: &str) -> Result<JsValue, JsValue> {
    let backup: WalletBackupFile = serde_json::from_str(backup_json)
        .map_err(|e| JsValue::from_str(&format!("backup parse: {e}")))?;
    let order: OwnedUnwrapOrder = serde_json::from_str(order_json)
        .map_err(|e| JsValue::from_str(&format!("order parse: {e}")))?;
    validate_owned_certificate_domain_for_wallet(&backup, &order.domain)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    let key_pair =
        derive_wallet_key_pair(&backup).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let owner_pubkey_hex = bytes_to_hex(&key_pair.public_key);
    let signing_bytes = owned_unwrap_signing_bytes(&order);

    const OWNED_UNWRAP_CONTEXT: &[u8] = b"postfiat-l1-v2/owned-unwrap/v2";
    let signature =
        ml_dsa_65_sign_with_context(&key_pair.private_key, &signing_bytes, OWNED_UNWRAP_CONTEXT)
            .map_err(|e| JsValue::from_str(&format!("sign failed: {e}")))?;

    let owner_signature_hex = bytes_to_hex(&signature);
    let result = js_sys::Object::new();
    js_sys::Reflect::set(
        &result,
        &"owner_pubkey_hex".into(),
        &owner_pubkey_hex.clone().into(),
    )
    .map_err(|_| JsValue::from_str("reflect set failed"))?;
    js_sys::Reflect::set(
        &result,
        &"owner_signature_hex".into(),
        &owner_signature_hex.clone().into(),
    )
    .map_err(|_| JsValue::from_str("reflect set failed"))?;
    js_sys::Reflect::set(&result, &"order".into(), &order_json.into())
        .map_err(|_| JsValue::from_str("reflect set failed"))?;
    Ok(result.into())
}

/// Sign a recovery-safe FastPay v3 transfer against the exact live capability
/// returned by `owned_recovery_capabilities`.
#[wasm_bindgen]
pub fn wallet_sign_owned_transfer_v3(
    backup_json: &str,
    order_json: &str,
    capabilities_json: &str,
) -> Result<JsValue, JsValue> {
    let backup: WalletBackupFile = serde_json::from_str(backup_json)
        .map_err(|error| JsValue::from_str(&format!("backup parse: {error}")))?;
    let order: OwnedTransferOrderV3 = serde_json::from_str(order_json)
        .map_err(|error| JsValue::from_str(&format!("FastPay v3 order parse: {error}")))?;
    let capabilities: FastPayRecoveryCapabilitiesV1 = serde_json::from_str(capabilities_json)
        .map_err(|error| JsValue::from_str(&format!("FastPay capabilities parse: {error}")))?;
    let signed = wallet_sign_owned_transfer_order_v3(&backup, order, &capabilities)
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    to_json_js_value(&signed)
}

/// Sign a recovery-safe FastPay v3 unwrap against the exact live capability.
#[wasm_bindgen]
pub fn wallet_sign_owned_unwrap_v3(
    backup_json: &str,
    order_json: &str,
    capabilities_json: &str,
) -> Result<JsValue, JsValue> {
    let backup: WalletBackupFile = serde_json::from_str(backup_json)
        .map_err(|error| JsValue::from_str(&format!("backup parse: {error}")))?;
    let order: OwnedUnwrapOrderV3 = serde_json::from_str(order_json)
        .map_err(|error| JsValue::from_str(&format!("FastPay v3 unwrap parse: {error}")))?;
    let capabilities: FastPayRecoveryCapabilitiesV1 = serde_json::from_str(capabilities_json)
        .map_err(|error| JsValue::from_str(&format!("FastPay capabilities parse: {error}")))?;
    let signed = wallet_sign_owned_unwrap_order_v3(&backup, order, &capabilities)
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    to_json_js_value(&signed)
}

#[wasm_bindgen]
pub fn wallet_fastpay_transfer_lock_id(order_json: &str) -> Result<String, JsValue> {
    let order: OwnedTransferOrderV3 = serde_json::from_str(order_json)
        .map_err(|error| JsValue::from_str(&format!("FastPay v3 order parse: {error}")))?;
    Ok(wallet_fastpay_transfer_lock_id_v1(&order))
}

#[wasm_bindgen]
pub fn wallet_fastpay_unwrap_lock_id(order_json: &str) -> Result<String, JsValue> {
    let order: OwnedUnwrapOrderV3 = serde_json::from_str(order_json)
        .map_err(|error| JsValue::from_str(&format!("FastPay v3 unwrap parse: {error}")))?;
    Ok(wallet_fastpay_unwrap_lock_id_v1(&order))
}

#[wasm_bindgen]
pub fn wallet_fastpay_transfer_certificate_digest(
    certificate_json: &str,
) -> Result<String, JsValue> {
    let certificate: OwnedTransferCertificateV3 = serde_json::from_str(certificate_json)
        .map_err(|error| JsValue::from_str(&format!("FastPay v3 certificate parse: {error}")))?;
    wallet_fastpay_transfer_certificate_digest_v3(&certificate)
        .map_err(|error| JsValue::from_str(&error.to_string()))
}

#[wasm_bindgen]
pub fn wallet_fastpay_unwrap_certificate_digest(certificate_json: &str) -> Result<String, JsValue> {
    let certificate: OwnedUnwrapCertificateV3 = serde_json::from_str(certificate_json)
        .map_err(|error| JsValue::from_str(&format!("FastPay v3 certificate parse: {error}")))?;
    wallet_fastpay_unwrap_certificate_digest_v3(&certificate)
        .map_err(|error| JsValue::from_str(&error.to_string()))
}

#[wasm_bindgen]
pub fn wallet_verify_fastpay_apply_ack(
    acknowledgement_json: &str,
    validator_public_key_hex: &str,
) -> Result<bool, JsValue> {
    let acknowledgement: FastPayApplyAckV1 =
        serde_json::from_str(acknowledgement_json).map_err(|error| {
            JsValue::from_str(&format!("FastPay apply acknowledgement parse: {error}"))
        })?;
    wallet_verify_fastpay_apply_ack_v1(&acknowledgement, validator_public_key_hex)
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    Ok(true)
}

/// Canonical, domain-separated signing bytes for an owned-transfer order.
/// Mirrors `postfiat_execution::owned_transfer_signing_bytes` exactly so that
/// signatures are interoperable between WASM and the on-chain verifier.
fn owned_transfer_signing_bytes(order: &OwnedTransferOrder) -> Vec<u8> {
    let mut out = b"postfiat.owned-transfer.v2\0".to_vec();
    append_owned_certificate_domain(&mut out, &order.domain);
    out.extend(&(order.inputs.len() as u64).to_le_bytes());
    for r in &order.inputs {
        out.extend(&(r.id.len() as u64).to_le_bytes());
        out.extend(r.id.as_bytes());
        out.extend(&r.version.to_le_bytes());
    }
    out.extend(&(order.outputs.len() as u64).to_le_bytes());
    for o in &order.outputs {
        out.extend(&(o.owner_pubkey_hex.len() as u64).to_le_bytes());
        out.extend(o.owner_pubkey_hex.as_bytes());
        out.extend(&o.value.to_le_bytes());
        out.extend(&(o.asset.len() as u64).to_le_bytes());
        out.extend(o.asset.as_bytes());
    }
    out.extend(&order.fee.to_le_bytes());
    out.extend(&order.nonce.to_le_bytes());
    out.extend(&(order.memos.len() as u64).to_le_bytes());
    for m in &order.memos {
        out.extend(m.memo_type.as_bytes());
        out.push(0);
        out.extend(m.memo_format.as_bytes());
        out.push(0);
        out.extend(m.memo_data.as_bytes());
        out.push(0);
    }
    out
}

/// Canonical signing bytes for an owned-unwrap order.
fn owned_unwrap_signing_bytes(order: &OwnedUnwrapOrder) -> Vec<u8> {
    let mut out = b"postfiat.owned-unwrap.v2\0".to_vec();
    append_owned_certificate_domain(&mut out, &order.domain);
    out.extend(&(order.inputs.len() as u64).to_le_bytes());
    for r in &order.inputs {
        out.extend(&(r.id.len() as u64).to_le_bytes());
        out.extend(r.id.as_bytes());
        out.extend(&r.version.to_le_bytes());
    }
    out.extend(&(order.to_address.len() as u64).to_le_bytes());
    out.extend(order.to_address.as_bytes());
    out.extend(&order.amount.to_le_bytes());
    out.extend(&(order.asset.len() as u64).to_le_bytes());
    out.extend(order.asset.as_bytes());
    out.extend(&order.fee.to_le_bytes());
    out.extend(&order.nonce.to_le_bytes());
    out.extend(&(order.memos.len() as u64).to_le_bytes());
    for m in &order.memos {
        out.extend(m.memo_type.as_bytes());
        out.push(0);
        out.extend(m.memo_format.as_bytes());
        out.push(0);
        out.extend(m.memo_data.as_bytes());
        out.push(0);
    }
    out
}

fn append_owned_certificate_domain(out: &mut Vec<u8>, domain: &OwnedCertificateDomain) {
    for value in [
        domain.schema.as_str(),
        domain.chain_id.as_str(),
        domain.genesis_hash.as_str(),
        domain.registry_id.as_str(),
    ] {
        out.extend(&(value.len() as u64).to_le_bytes());
        out.extend(value.as_bytes());
    }
    out.extend(&domain.protocol_version.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use postfiat_types::{OwnedObjectRef, OwnedOutputSpec};

    #[test]
    fn owned_transfer_signing_bytes_use_fixed_width_lengths() {
        let order = OwnedTransferOrder {
            domain: OwnedCertificateDomain {
                schema: postfiat_types::OWNED_CERTIFICATE_DOMAIN_SCHEMA_V2.to_string(),
                chain_id: "postfiat-wallet-wasm".to_string(),
                genesis_hash: "ab".repeat(48),
                protocol_version: 1,
                registry_id: "cd".repeat(48),
            },
            inputs: vec![OwnedObjectRef {
                id: "ab".to_string(),
                version: 7,
            }],
            outputs: vec![OwnedOutputSpec {
                owner_pubkey_hex: "cdef".to_string(),
                value: 11,
                asset: "PFT".to_string(),
            }],
            fee: 1,
            nonce: 42,
            memos: Vec::new(),
        };

        let bytes = owned_transfer_signing_bytes(&order);
        let mut offset = b"postfiat.owned-transfer.v2\0".len();
        for value in [
            order.domain.schema.as_str(),
            order.domain.chain_id.as_str(),
            order.domain.genesis_hash.as_str(),
            order.domain.registry_id.as_str(),
        ] {
            assert_eq!(
                &bytes[offset..offset + 8],
                &(value.len() as u64).to_le_bytes()
            );
            offset += 8;
            assert_eq!(&bytes[offset..offset + value.len()], value.as_bytes());
            offset += value.len();
        }
        assert_eq!(
            &bytes[offset..offset + 4],
            &order.domain.protocol_version.to_le_bytes()
        );
        offset += 4;
        assert_eq!(&bytes[offset..offset + 8], &1u64.to_le_bytes());
        offset += 8;
        assert_eq!(&bytes[offset..offset + 8], &2u64.to_le_bytes());
        offset += 8 + "ab".len() + 8;
        assert_eq!(&bytes[offset..offset + 8], &1u64.to_le_bytes());
        offset += 8;
        assert_eq!(&bytes[offset..offset + 8], &4u64.to_le_bytes());
        offset += 8 + "cdef".len() + 8;
        assert_eq!(&bytes[offset..offset + 8], &3u64.to_le_bytes());
    }
}
///
/// method: RPC method name
/// params_json: params as JSON object string
/// Returns: complete RPC request JSON string ready to send
#[wasm_bindgen]
pub fn make_rpc_request(method: &str, params_json: &str) -> Result<String, JsValue> {
    let params: serde_json::Value = serde_json::from_str(params_json)
        .map_err(|e| JsValue::from_str(&format!("params parse: {e}")))?;

    let id = format!("wasm-{}", counter());
    let request = RpcRequest::new(&id, method, params);
    serde_json::to_string(&request).map_err(|e| JsValue::from_str(&format!("serialize: {e}")))
}

/// Parse and validate an RPC response.
///
/// response_json: raw response JSON string from the server
/// Returns: JS object { ok, result, error }
#[wasm_bindgen]
pub fn parse_rpc_response(response_json: &str) -> Result<JsValue, JsValue> {
    let response: postfiat_rpc_sdk::RpcResponse = serde_json::from_str(response_json)
        .map_err(|e| JsValue::from_str(&format!("response parse: {e}")))?;

    let result = js_sys::Object::new();
    js_sys::Reflect::set(&result, &"ok".into(), &response.ok.into())
        .map_err(|_| JsValue::from_str("reflect set failed"))?;
    if let Some(err) = &response.error {
        let err_obj = js_sys::Object::new();
        js_sys::Reflect::set(&err_obj, &"code".into(), &err.code.clone().into())
            .map_err(|_| JsValue::from_str("reflect set failed"))?;
        js_sys::Reflect::set(&err_obj, &"message".into(), &err.message.clone().into())
            .map_err(|_| JsValue::from_str("reflect set failed"))?;
        js_sys::Reflect::set(&result, &"error".into(), &err_obj.into())
            .map_err(|_| JsValue::from_str("reflect set failed"))?;
    } else {
        js_sys::Reflect::set(&result, &"error".into(), &JsValue::NULL)
            .map_err(|_| JsValue::from_str("reflect set failed"))?;
    }
    if let Some(res) = &response.result {
        let res_str = serde_json::to_string(res)
            .map_err(|e| JsValue::from_str(&format!("result serialize: {e}")))?;
        let parsed = js_sys::JSON::parse(&res_str)
            .map_err(|_| JsValue::from_str("result JSON.parse failed"))?;
        js_sys::Reflect::set(&result, &"result".into(), &parsed)
            .map_err(|_| JsValue::from_str("reflect set failed"))?;
    } else {
        js_sys::Reflect::set(&result, &"result".into(), &JsValue::NULL)
            .map_err(|_| JsValue::from_str("reflect set failed"))?;
    }
    Ok(result.into())
}

/// Generate a random 32-byte master seed (64 hex chars).
///
/// Uses getrandom with the `js` feature for browser-compatible randomness.
#[wasm_bindgen]
pub fn random_master_seed() -> Result<String, JsValue> {
    let mut seed = [0u8; 32];
    getrandom::getrandom(&mut seed).map_err(|e| JsValue::from_str(&format!("random: {e}")))?;
    Ok(bytes_to_hex(&seed))
}

// Simple incrementing counter for request IDs
fn counter() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    COUNTER.fetch_add(1, Ordering::Relaxed)
}
