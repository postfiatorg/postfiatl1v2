use postfiat_rpc_sdk::{
    wallet_sign_offer_transaction_from_fields, WalletBackupFile, WalletSignOfferTransactionFields,
};
use postfiat_types::{OfferTransactionOperation, SignedOfferTransaction};
use serde_json::Value;

// Browser review compares JSON fields, while consensus signs typed fields. The
// consensus serializer omits a zero expiration for historical compatibility.
// Preserve an explicitly reviewed zero here without changing consensus bytes.
pub(crate) fn offer_envelope_for_browser(
    signed: &SignedOfferTransaction,
    reviewed_operation: &Value,
) -> Result<Value, String> {
    let mut envelope = serde_json::to_value(signed).map_err(|error| error.to_string())?;
    let mut operation =
        serde_json::to_value(&signed.unsigned.operation).map_err(|error| error.to_string())?;
    if let OfferTransactionOperation::OfferCreate(create) = &signed.unsigned.operation {
        if reviewed_operation.get("expiration_height").is_some() {
            envelope["unsigned"]["expiration_height"] = Value::from(create.expiration_height);
            operation["expiration_height"] = Value::from(create.expiration_height);
        }
    }
    // Do not return an envelope if serde discarded an unrecognized review field.
    if operation != *reviewed_operation {
        return Err("offer signing cannot preserve the reviewed operation".to_string());
    }
    Ok(envelope)
}

pub(crate) fn sign_offer_fields_for_browser(
    backup: &WalletBackupFile,
    fields_json: &str,
) -> Result<Value, String> {
    let reviewed: Value = serde_json::from_str(fields_json).map_err(|error| error.to_string())?;
    let fields: WalletSignOfferTransactionFields =
        serde_json::from_value(reviewed.clone()).map_err(|error| error.to_string())?;
    let signed = wallet_sign_offer_transaction_from_fields(backup, fields)
        .map_err(|error| error.to_string())?;
    offer_envelope_for_browser(&signed, &reviewed["operation"])
}
