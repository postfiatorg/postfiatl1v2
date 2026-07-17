use std::fmt;
use std::fs;
use std::io;
use std::path::Path;

use postfiat_crypto_provider::{
    address_from_public_key, bytes_to_hex as crypto_bytes_to_hex, hash_bytes as crypto_hash_bytes,
    hex_to_bytes as crypto_hex_to_bytes, ml_dsa_65_keygen_from_seed, ml_dsa_65_sign,
    ml_dsa_65_sign_with_context, ml_dsa_65_verify, ml_dsa_65_verify_with_context,
};
use postfiat_types::{
    AssetTransactionOperation, EscrowTransactionOperation, NftTransactionOperation,
    OfferTransactionOperation, OwnedCertificateDomain, OwnedTransferOrder, OwnedUnwrapOrder,
    PaymentMemo, SignedAssetTransaction, SignedEscrowTransaction,
    SignedNftTransaction, SignedOfferTransaction, SignedPaymentV2, SignedTransfer,
    UnsignedAssetTransaction, UnsignedEscrowTransaction, UnsignedNftTransaction,
    UnsignedOfferTransaction, UnsignedPaymentV2, UnsignedTransfer, ASSET_BURN_TRANSACTION_KIND,
    ASSET_CLAWBACK_TRANSACTION_KIND, ASSET_CREATE_TRANSACTION_KIND,
    ATOMIC_SWAP_TRANSACTION_KIND,
    ATOMIC_SETTLEMENT_TEMPLATE_ID_HEX_LEN, ESCROW_CANCEL_TRANSACTION_KIND,
    ESCROW_CONDITION_HASH_HEX_LEN, ESCROW_CREATE_TRANSACTION_KIND, ESCROW_FINISH_TRANSACTION_KIND,
    ESCROW_ID_HEX_LEN, ESCROW_STATE_CANCELED, ESCROW_STATE_FINISHED, ESCROW_STATE_OPEN,
    ISSUED_ASSET_ID_HEX_LEN, ISSUED_PAYMENT_TRANSACTION_KIND, NFT_BURN_TRANSACTION_KIND,
    NFT_COLLECTION_FLAG_BURN_LOCKED, NFT_COLLECTION_FLAG_TRANSFER_LOCKED, NFT_FLAG_ISSUER_BURNABLE,
    NFT_FLAG_TRANSFERABLE, NFT_ID_HEX_LEN, NFT_MINT_TRANSACTION_KIND,
    NFT_TRANSFER_TRANSACTION_KIND, OFFER_CANCEL_TRANSACTION_KIND, OFFER_CREATE_TRANSACTION_KIND,
    OFFER_ID_HEX_LEN, OFFER_STATE_CANCELED, OFFER_STATE_FILLED, OFFER_STATE_OPEN,
    OFFER_STATE_UNFUNDED, OFFER_TX_ROLE_CANCEL, OFFER_TX_ROLE_MAKER, OFFER_TX_ROLE_TAKER,
    PAYMENT_V2_TRANSACTION_KIND, TRUSTLINE_ID_HEX_LEN, TRUST_SET_TRANSACTION_KIND,
    NAV_ASSET_REGISTER_TRANSACTION_KIND, NAV_EPOCH_FINALIZE_TRANSACTION_KIND,
    NAV_HALT_TRANSACTION_KIND, NAV_MINT_AT_NAV_TRANSACTION_KIND,
    NAV_PROFILE_REGISTER_TRANSACTION_KIND, NAV_REDEEM_AT_NAV_TRANSACTION_KIND,
    NAV_ATTESTOR_REGISTER_TRANSACTION_KIND, NAV_RESERVE_ATTEST_TRANSACTION_KIND,
    NAV_REDEEM_SETTLE_TRANSACTION_KIND,
    NAV_RESERVE_CHALLENGE_TRANSACTION_KIND, NAV_RESERVE_SUBMIT_TRANSACTION_KIND,
    PFTL_UNISWAP_DESTINATION_CONSUME_TRANSACTION_KIND,
    PFTL_UNISWAP_EXPORT_DEBIT_TRANSACTION_KIND, PFTL_UNISWAP_PRIMARY_SUBSCRIBE_TRANSACTION_KIND,
    PFTL_UNISWAP_REFUND_SOURCE_TRANSACTION_KIND, PFTL_UNISWAP_RETURN_IMPORT_TRANSACTION_KIND,
    PFTL_UNISWAP_ROUTE_INIT_TRANSACTION_KIND,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Number, Value};
use sha3::{Digest, Sha3_384};
use zeroize::Zeroizing;

include!("atomic_swap_protocol.rs");

pub const CRATE_PURPOSE: &str = "RPC, wallet SDK, and local wallet flows";
pub const RPC_VERSION: &str = "postfiat-local-rpc-v1";
pub const METHOD_STATUS: &str = "status";
pub const METHOD_SERVER_INFO: &str = "server_info";
pub const METHOD_METRICS: &str = "metrics";
pub const METHOD_LEDGER: &str = "ledger";
pub const METHOD_VERIFY_STATE: &str = "verify_state";
pub const METHOD_VALIDATE_LOCAL_KEYS: &str = "validate_local_keys";
pub const METHOD_ACCOUNT: &str = "account";
pub const METHOD_ACCOUNT_TX: &str = "account_tx";
pub const METHOD_FEE: &str = "fee";
pub const METHOD_TRANSFER_FEE_QUOTE: &str = "transfer_fee_quote";
pub const METHOD_OWNED_SIGN: &str = "owned_sign";
pub const METHOD_OWNED_UNWRAP_SIGN: &str = "owned_unwrap_sign";
pub const METHOD_FASTSWAP_CAPABILITIES: &str = "fastswap_capabilities";
pub const METHOD_FASTSWAP_PREVIEW: &str = "fastswap_preview";
pub const METHOD_FASTSWAP_PREPARE: &str = "fastswap_prepare";
pub const METHOD_FASTSWAP_COMMIT: &str = "fastswap_commit";
pub const METHOD_FASTSWAP_APPLY: &str = "fastswap_apply";
pub const METHOD_FASTSWAP_CATCH_UP: &str = "fastswap_catch_up";
pub const METHOD_FASTSWAP_STATUS: &str = "fastswap_status";
pub const METHOD_FASTSWAP_EFFECTS: &str = "fastswap_effects";
pub const METHOD_FASTSWAP_VOTES: &str = "fastswap_votes";
pub const METHOD_FASTSWAP_NEW_ROUND_VOTE: &str = "fastswap_new_round_vote";
pub const METHOD_FASTSWAP_PROPOSE_ROUND: &str = "fastswap_propose_round";
pub const METHOD_FASTSWAP_PRECOMMIT: &str = "fastswap_precommit";
pub const METHOD_FASTSWAP_COMMIT_ROUND: &str = "fastswap_commit_round";
pub const METHOD_FASTSWAP_CANCEL_APPLY: &str = "fastswap_cancel_apply";
pub const METHOD_FASTLANE_EXIT: &str = "fastlane_exit";
pub const METHOD_FASTSWAP_CHECKPOINT_STATUS: &str = "fastswap_checkpoint_status";
pub const METHOD_FASTSWAP_OBJECTS: &str = "fastswap_objects";
pub const METHOD_FASTSWAP_POLICY: &str = "fastswap_policy";
pub const METHOD_FASTLANE_ASSET_CONTROL_PREPARE: &str = "fastlane_asset_control_prepare";
pub const METHOD_FASTLANE_ASSET_CONTROL_PREVIEW: &str = "fastlane_asset_control_preview";
pub const METHOD_FASTLANE_ASSET_CONTROL_APPLY: &str = "fastlane_asset_control_apply";
pub const METHOD_FASTLANE_ASSET_CONTROL_CATCH_UP: &str = "fastlane_asset_control_catch_up";
pub const METHOD_MEMPOOL_SUBMIT_FASTLANE_PRIMARY: &str = "mempool_submit_fastlane_primary";
pub const METHOD_MEMPOOL_SUBMIT_FASTLANE_PRIMARY_FINALITY: &str =
    "mempool_submit_fastlane_primary_finality";
pub const METHOD_ASSET_FEE_QUOTE: &str = "asset_fee_quote";
pub const METHOD_ESCROW_FEE_QUOTE: &str = "escrow_fee_quote";
pub const METHOD_NFT_FEE_QUOTE: &str = "nft_fee_quote";
pub const METHOD_OFFER_FEE_QUOTE: &str = "offer_fee_quote";
pub const METHOD_ATOMIC_SETTLEMENT_TEMPLATE: &str = "atomic_settlement_template";
pub const METHOD_OFFER_INFO: &str = "offer_info";
pub const METHOD_ACCOUNT_OFFERS: &str = "account_offers";
pub const METHOD_BOOK_OFFERS: &str = "book_offers";
pub const METHOD_ASSET_INFO: &str = "asset_info";
pub const METHOD_ACCOUNT_LINES: &str = "account_lines";
pub const METHOD_ACCOUNT_ASSETS: &str = "account_assets";
pub const METHOD_ISSUER_ASSETS: &str = "issuer_assets";
pub const METHOD_ESCROW_INFO: &str = "escrow_info";
pub const METHOD_ACCOUNT_ESCROWS: &str = "account_escrows";
pub const METHOD_NFT_INFO: &str = "nft_info";
pub const METHOD_ACCOUNT_NFTS: &str = "account_nfts";
pub const METHOD_ISSUER_NFTS: &str = "issuer_nfts";
pub const METHOD_RECEIPTS: &str = "receipts";
pub const METHOD_TX: &str = "tx";
pub const METHOD_BLOCKS: &str = "blocks";
pub const METHOD_VALIDATORS: &str = "validators";
pub const METHOD_MANIFESTS: &str = "manifests";
pub const METHOD_BATCH_ARCHIVE: &str = "batch_archive";
pub const METHOD_ARCHIVE_WINDOW: &str = "archive_window";
pub const METHOD_MEMPOOL_SUBMIT_TRANSFER: &str = "mempool_submit_transfer";
pub const METHOD_MEMPOOL_SUBMIT_SIGNED_TRANSFER: &str = "mempool_submit_signed_transfer";
pub const METHOD_MEMPOOL_SUBMIT_SIGNED_PAYMENT_V2: &str = "mempool_submit_signed_payment_v2";
pub const METHOD_MEMPOOL_SUBMIT_SIGNED_ASSET_TRANSACTION: &str =
    "mempool_submit_signed_asset_transaction";
pub const METHOD_MEMPOOL_SUBMIT_SIGNED_ESCROW_TRANSACTION: &str =
    "mempool_submit_signed_escrow_transaction";
pub const METHOD_MEMPOOL_SUBMIT_SIGNED_NFT_TRANSACTION: &str =
    "mempool_submit_signed_nft_transaction";
pub const METHOD_MEMPOOL_SUBMIT_SIGNED_OFFER_TRANSACTION: &str =
    "mempool_submit_signed_offer_transaction";
pub const METHOD_MEMPOOL_STATUS: &str = "mempool_status";
pub const METHOD_MEMPOOL_BATCH: &str = "mempool_batch";
pub const METHOD_APPLY_BATCH: &str = "apply_batch";
pub const METHOD_SHIELD_BATCH_MINT: &str = "shield_batch_mint";
pub const METHOD_SHIELD_BATCH_SPEND: &str = "shield_batch_spend";
pub const METHOD_SHIELD_BATCH_MIGRATE: &str = "shield_batch_migrate";
pub const METHOD_SHIELD_BATCH_ORCHARD: &str = "shield_batch_orchard";
pub const METHOD_SHIELD_BATCH_ORCHARD_DEPOSIT: &str = "shield_batch_orchard_deposit";
pub const METHOD_SHIELD_BATCH_ORCHARD_WITHDRAW: &str = "shield_batch_orchard_withdraw";
pub const METHOD_SHIELD_BATCH_SWAP: &str = "shield_batch_swap";
pub const METHOD_APPLY_SHIELD_BATCH: &str = "apply_shield_batch";
pub const METHOD_SHIELD_SCAN: &str = "shield_scan";
pub const METHOD_SHIELD_DISCLOSE: &str = "shield_disclose";
pub const METHOD_SHIELD_TURNSTILE: &str = "shield_turnstile";
pub const METHOD_BRIDGE_STATUS: &str = "bridge_status";
pub const METHOD_NAVCOIN_BRIDGE_ROUTES: &str = "navcoin_bridge_routes";
pub const METHOD_NAVCOIN_BRIDGE_PACKET: &str = "navcoin_bridge_packet";
pub const METHOD_NAVCOIN_BRIDGE_CLAIMS: &str = "navcoin_bridge_claims";
pub const METHOD_NAVCOIN_BRIDGE_SUPPLY_STATUS: &str = "navcoin_bridge_supply_status";
pub const METHOD_NAVCOIN_BRIDGE_RECEIPT_REPLAY: &str = "navcoin_bridge_receipt_replay";
pub const METHOD_NAVCOIN_BRIDGE_PACKET_PREFLIGHT: &str = "navcoin_bridge_packet_preflight";
pub const METHOD_BRIDGE_BATCH_DOMAIN: &str = "bridge_batch_domain";
pub const METHOD_BRIDGE_BATCH_TRANSFER: &str = "bridge_batch_transfer";
pub const METHOD_BRIDGE_BATCH_PAUSE: &str = "bridge_batch_pause";
pub const METHOD_BRIDGE_BATCH_RESUME: &str = "bridge_batch_resume";
pub const METHOD_APPLY_BRIDGE_BATCH: &str = "apply_bridge_batch";
pub const METRICS_SCHEMA: &str = "postfiat-node-metrics-v1";
pub const SERVER_INFO_SCHEMA: &str = "postfiat-server-info-v1";
pub const LEDGER_SCHEMA: &str = "postfiat-ledger-v1";
pub const FEE_SCHEMA: &str = "postfiat-fee-v1";
pub const VALIDATORS_SCHEMA: &str = "postfiat-validators-v1";
pub const MANIFESTS_SCHEMA: &str = "postfiat-manifests-v1";
pub const TRANSFER_FEE_QUOTE_SCHEMA: &str = "postfiat-transfer-fee-quote-v1";
pub const ASSET_FEE_QUOTE_SCHEMA: &str = "postfiat-asset-fee-quote-v1";
pub const ESCROW_FEE_QUOTE_SCHEMA: &str = "postfiat-escrow-fee-quote-v1";
pub const NFT_FEE_QUOTE_SCHEMA: &str = "postfiat-nft-fee-quote-v1";
pub const OFFER_FEE_QUOTE_SCHEMA: &str = "postfiat-offer-fee-quote-v1";
pub const ATOMIC_SETTLEMENT_TEMPLATE_SCHEMA: &str = "postfiat-atomic-settlement-template-v1";
pub const OFFER_INFO_SCHEMA: &str = "postfiat-offer-info-v1";
pub const ACCOUNT_OFFERS_SCHEMA: &str = "postfiat-account-offers-v1";
pub const BOOK_OFFERS_SCHEMA: &str = "postfiat-book-offers-v1";
pub const ASSET_INFO_SCHEMA: &str = "postfiat-asset-info-v1";
pub const ACCOUNT_LINES_SCHEMA: &str = "postfiat-account-lines-v1";
pub const ACCOUNT_ASSETS_SCHEMA: &str = "postfiat-account-assets-v1";
pub const ISSUER_ASSETS_SCHEMA: &str = "postfiat-issuer-assets-v1";
pub const ESCROW_INFO_SCHEMA: &str = "postfiat-escrow-info-v1";
pub const ACCOUNT_ESCROWS_SCHEMA: &str = "postfiat-account-escrows-v1";
pub const NFT_INFO_SCHEMA: &str = "postfiat-nft-info-v1";
pub const ACCOUNT_NFTS_SCHEMA: &str = "postfiat-account-nfts-v1";
pub const ISSUER_NFTS_SCHEMA: &str = "postfiat-issuer-nfts-v1";
pub const STATE_VERIFICATION_SCHEMA: &str = "postfiat-state-verification-v1";
pub const LOCAL_KEY_VALIDATION_SCHEMA: &str = "postfiat-local-key-validation-v1";
pub const HISTORY_ARCHIVE_WINDOW_SCHEMA: &str = "postfiat-history-archive-window-v1";
pub const HISTORY_ARCHIVE_HANDOFF_SCHEMA: &str = "postfiat-history-archive-handoff-v1";
pub const MAX_RPC_REQUEST_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_RPC_PARAM_NAME_BYTES: usize = 64;
pub const MAX_RPC_PARAM_STRING_BYTES: usize = 4096;
pub const MAX_RPC_SIGNED_TRANSFER_JSON_BYTES: usize = 64 * 1024;
pub const MAX_RPC_FASTPAY_JSON_BYTES: usize = 64 * 1024;
pub const MAX_RPC_ORCHARD_ACTION_JSON_BYTES: usize = 48 * 1024;
pub const MAX_RPC_ORCHARD_DEPOSIT_JSON_BYTES: usize = 48 * 1024;
pub const MAX_RPC_SHIELDED_SWAP_JSON_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_RPC_SHIELD_BATCH_JSON_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_RPC_ASSET_ORCHARD_ENCRYPTED_OUTPUT_BYTES: usize = 4096;
pub const MAX_RPC_ASSET_ORCHARD_PROOF_BYTES: usize = 1_048_576;
pub const MAX_RPC_READ_QUERY_LIMIT: usize = 512;
pub const MAX_RPC_BATCH_ARCHIVE_PAYLOAD_BYTES: usize = 8 * 1024 * 1024;
pub const WALLET_BACKUP_FILE_SCHEMA: &str = "postfiat-wallet-backup-v1";
pub const WALLET_DERIVATION_DOMAIN: &str = "postfiat.wallet.seed.v1";
pub const WALLET_DERIVATION_KDF: &str = "sha3-384-domain-truncate32";
pub const WALLET_KEY_ROLE_TRANSPARENT_SPEND: &str = "transparent-spend";
pub const OWNED_TRANSFER_CONTEXT: &[u8] = b"postfiat-l1-v2/owned-transfer/v2";
pub const OWNED_UNWRAP_CONTEXT: &[u8] = b"postfiat-l1-v2/owned-unwrap/v2";
const TRANSPARENT_ADDRESS_NAMESPACE: &str = "postfiat.address.v1";
const TRANSPARENT_TRANSFER_KIND: &str = "transparent_transfer";
const ML_DSA_65_ALGORITHM: &str = "ML-DSA-65";
const BATCH_ARCHIVE_PAYLOAD_HASH_DOMAIN: &str = "postfiat.batch_archive_payload.v1";
pub const ORCHARD_DEPOSIT_POLICY_ID: &str = "postfiat.orchard.deposit.v1";
pub const ORCHARD_WITHDRAW_POLICY_ID: &str = "postfiat.orchard.withdraw.v1";
const ASSET_ORCHARD_SWAP_ACTION_SCHEMA_V1: &str = "postfiat-asset-orchard-swap-action-v1";
const ASSET_ORCHARD_POOL_ID_V1: &str = "asset-orchard-v1";
const ASSET_ORCHARD_PROOF_SYSTEM_ID_V1: &str = "postfiat.privacy.asset-orchard-halo2.v1";
const ASSET_ORCHARD_CIRCUIT_ID_V1: &str = "asset_orchard.swap.pricing_bound.v4";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RpcRequest {
    pub version: String,
    pub id: String,
    pub method: String,
    pub params: Value,
}

impl RpcRequest {
    pub fn new(id: impl Into<String>, method: impl Into<String>, params: Value) -> Self {
        Self {
            version: RPC_VERSION.to_string(),
            id: id.into(),
            method: method.into(),
            params,
        }
    }

    pub fn empty(id: impl Into<String>, method: impl Into<String>) -> Self {
        Self::new(id, method, Value::Object(Map::new()))
    }

    pub fn with_param<T: Serialize>(
        mut self,
        key: impl Into<String>,
        value: T,
    ) -> Result<Self, serde_json::Error> {
        let value = serde_json::to_value(value)?;
        match &mut self.params {
            Value::Object(params) => {
                params.insert(key.into(), value);
            }
            _ => {
                let mut params = Map::new();
                params.insert(key.into(), value);
                self.params = Value::Object(params);
            }
        }
        Ok(self)
    }

    fn with_param_value(mut self, key: impl Into<String>, value: Value) -> Self {
        match &mut self.params {
            Value::Object(params) => {
                params.insert(key.into(), value);
            }
            _ => {
                let mut params = Map::new();
                params.insert(key.into(), value);
                self.params = Value::Object(params);
            }
        }
        self
    }

    pub fn to_pretty_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn validate_protocol(&self) -> Result<(), RpcProtocolError> {
        validate_common_fields(&self.version, &self.id)?;
        if self.method.trim().is_empty() {
            return Err(RpcProtocolError::EmptyMethod);
        }
        validate_protocol_string_len("method", &self.method)?;
        let params = self
            .params
            .as_object()
            .ok_or(RpcProtocolError::ParamsNotObject)?;
        for (key, value) in params {
            if key.trim().is_empty() {
                return Err(RpcProtocolError::EmptyParamName);
            }
            if key.len() > MAX_RPC_PARAM_NAME_BYTES {
                return Err(RpcProtocolError::ParamNameTooLong {
                    key: key.clone(),
                    max_bytes: MAX_RPC_PARAM_NAME_BYTES,
                });
            }
            validate_param_value(&self.method, key, value)?;
        }
        Ok(())
    }
}

fn string_value(value: impl Into<String>) -> Value {
    Value::String(value.into())
}

fn u32_value(value: u32) -> Value {
    Value::Number(Number::from(value as u64))
}

fn u64_value(value: u64) -> Value {
    Value::Number(Number::from(value))
}

fn bool_value(value: bool) -> Value {
    Value::Bool(value)
}

fn usize_value(value: usize) -> Value {
    Value::Number(Number::from(value as u64))
}

pub fn status_request(id: impl Into<String>) -> RpcRequest {
    RpcRequest::empty(id, METHOD_STATUS)
}

pub fn server_info_request(id: impl Into<String>) -> RpcRequest {
    RpcRequest::empty(id, METHOD_SERVER_INFO)
}

pub fn metrics_request(id: impl Into<String>) -> RpcRequest {
    RpcRequest::empty(id, METHOD_METRICS)
}

pub fn ledger_request(id: impl Into<String>, limit: Option<usize>) -> RpcRequest {
    let mut request = RpcRequest::empty(id, METHOD_LEDGER);
    if let Some(limit) = limit {
        request = request.with_param_value("limit", usize_value(limit));
    }
    request
}

pub fn verify_state_request(id: impl Into<String>) -> RpcRequest {
    RpcRequest::empty(id, METHOD_VERIFY_STATE)
}

pub fn validate_local_keys_request(id: impl Into<String>, validators: u32) -> RpcRequest {
    RpcRequest::empty(id, METHOD_VALIDATE_LOCAL_KEYS)
        .with_param_value("validators", u32_value(validators))
}

pub fn account_request(id: impl Into<String>, address: impl Into<String>) -> RpcRequest {
    RpcRequest::empty(id, METHOD_ACCOUNT).with_param_value("address", string_value(address))
}

pub fn account_tx_request(
    id: impl Into<String>,
    address: impl Into<String>,
    from_height: Option<u64>,
    to_height: Option<u64>,
    limit: Option<usize>,
) -> RpcRequest {
    let mut request =
        RpcRequest::empty(id, METHOD_ACCOUNT_TX).with_param_value("address", string_value(address));
    if let Some(from_height) = from_height {
        request = request.with_param_value("from_height", u64_value(from_height));
    }
    if let Some(to_height) = to_height {
        request = request.with_param_value("to_height", u64_value(to_height));
    }
    if let Some(limit) = limit {
        request = request.with_param_value("limit", usize_value(limit));
    }
    request
}

pub fn fee_request(id: impl Into<String>) -> RpcRequest {
    RpcRequest::empty(id, METHOD_FEE)
}

pub fn transfer_fee_quote_request(
    id: impl Into<String>,
    from: impl Into<String>,
    to: impl Into<String>,
    amount: u64,
    sequence: Option<u64>,
) -> RpcRequest {
    let mut request = RpcRequest::empty(id, METHOD_TRANSFER_FEE_QUOTE)
        .with_param_value("from", string_value(from))
        .with_param_value("to", string_value(to))
        .with_param_value("amount", u64_value(amount));
    if let Some(sequence) = sequence {
        request = request.with_param_value("sequence", u64_value(sequence));
    }
    request
}

pub fn asset_fee_quote_request(
    id: impl Into<String>,
    source: impl Into<String>,
    operation_json: impl Into<String>,
    sequence: Option<u64>,
) -> RpcRequest {
    let mut request = RpcRequest::empty(id, METHOD_ASSET_FEE_QUOTE)
        .with_param_value("source", string_value(source))
        .with_param_value("operation_json", string_value(operation_json));
    if let Some(sequence) = sequence {
        request = request.with_param_value("sequence", u64_value(sequence));
    }
    request
}

pub fn escrow_fee_quote_request(
    id: impl Into<String>,
    source: impl Into<String>,
    operation_json: impl Into<String>,
    sequence: Option<u64>,
) -> RpcRequest {
    let mut request = RpcRequest::empty(id, METHOD_ESCROW_FEE_QUOTE)
        .with_param_value("source", string_value(source))
        .with_param_value("operation_json", string_value(operation_json));
    if let Some(sequence) = sequence {
        request = request.with_param_value("sequence", u64_value(sequence));
    }
    request
}

pub fn nft_fee_quote_request(
    id: impl Into<String>,
    source: impl Into<String>,
    operation_json: impl Into<String>,
    sequence: Option<u64>,
) -> RpcRequest {
    let mut request = RpcRequest::empty(id, METHOD_NFT_FEE_QUOTE)
        .with_param_value("source", string_value(source))
        .with_param_value("operation_json", string_value(operation_json));
    if let Some(sequence) = sequence {
        request = request.with_param_value("sequence", u64_value(sequence));
    }
    request
}

pub fn offer_fee_quote_request(
    id: impl Into<String>,
    source: impl Into<String>,
    operation_json: impl Into<String>,
    sequence: Option<u64>,
) -> RpcRequest {
    let mut request = RpcRequest::empty(id, METHOD_OFFER_FEE_QUOTE)
        .with_param_value("source", string_value(source))
        .with_param_value("operation_json", string_value(operation_json));
    if let Some(sequence) = sequence {
        request = request.with_param_value("sequence", u64_value(sequence));
    }
    request
}

pub fn offer_info_request(id: impl Into<String>, offer_id: impl Into<String>) -> RpcRequest {
    RpcRequest::empty(id, METHOD_OFFER_INFO).with_param_value("offer_id", string_value(offer_id))
}

pub fn account_offers_request(
    id: impl Into<String>,
    account: impl Into<String>,
    state: Option<&str>,
    limit: Option<usize>,
) -> RpcRequest {
    let mut request = RpcRequest::empty(id, METHOD_ACCOUNT_OFFERS)
        .with_param_value("account", string_value(account));
    if let Some(state) = state {
        request = request.with_param_value("state", string_value(state));
    }
    if let Some(limit) = limit {
        request = request.with_param_value("limit", usize_value(limit));
    }
    request
}

pub fn book_offers_request(
    id: impl Into<String>,
    taker_gets_asset_id: impl Into<String>,
    taker_pays_asset_id: impl Into<String>,
    limit: Option<usize>,
) -> RpcRequest {
    let mut request = RpcRequest::empty(id, METHOD_BOOK_OFFERS)
        .with_param_value("taker_gets_asset_id", string_value(taker_gets_asset_id))
        .with_param_value("taker_pays_asset_id", string_value(taker_pays_asset_id));
    if let Some(limit) = limit {
        request = request.with_param_value("limit", usize_value(limit));
    }
    request
}

#[allow(clippy::too_many_arguments)]
pub fn atomic_settlement_template_request(
    id: impl Into<String>,
    left_owner: impl Into<String>,
    left_recipient: impl Into<String>,
    left_asset_id: impl Into<String>,
    left_amount: u64,
    right_owner: impl Into<String>,
    right_recipient: impl Into<String>,
    right_asset_id: impl Into<String>,
    right_amount: u64,
    condition: impl Into<String>,
    finish_after: u64,
    cancel_after: u64,
    left_sequence: Option<u64>,
    right_sequence: Option<u64>,
) -> RpcRequest {
    let mut request = RpcRequest::empty(id, METHOD_ATOMIC_SETTLEMENT_TEMPLATE)
        .with_param_value("left_owner", string_value(left_owner))
        .with_param_value("left_recipient", string_value(left_recipient))
        .with_param_value("left_asset_id", string_value(left_asset_id))
        .with_param_value("left_amount", u64_value(left_amount))
        .with_param_value("right_owner", string_value(right_owner))
        .with_param_value("right_recipient", string_value(right_recipient))
        .with_param_value("right_asset_id", string_value(right_asset_id))
        .with_param_value("right_amount", u64_value(right_amount))
        .with_param_value("condition", string_value(condition))
        .with_param_value("finish_after", u64_value(finish_after))
        .with_param_value("cancel_after", u64_value(cancel_after));
    if let Some(sequence) = left_sequence {
        request = request.with_param_value("left_sequence", u64_value(sequence));
    }
    if let Some(sequence) = right_sequence {
        request = request.with_param_value("right_sequence", u64_value(sequence));
    }
    request
}

pub fn asset_info_request(id: impl Into<String>, asset_id: impl Into<String>) -> RpcRequest {
    RpcRequest::empty(id, METHOD_ASSET_INFO).with_param_value("asset_id", string_value(asset_id))
}

pub fn account_lines_request(
    id: impl Into<String>,
    account: impl Into<String>,
    issuer: Option<&str>,
    asset_id: Option<&str>,
    limit: Option<usize>,
) -> RpcRequest {
    let mut request = RpcRequest::empty(id, METHOD_ACCOUNT_LINES)
        .with_param_value("account", string_value(account));
    if let Some(issuer) = issuer {
        request = request.with_param_value("issuer", string_value(issuer));
    }
    if let Some(asset_id) = asset_id {
        request = request.with_param_value("asset_id", string_value(asset_id));
    }
    if let Some(limit) = limit {
        request = request.with_param_value("limit", usize_value(limit));
    }
    request
}

pub fn account_assets_request(
    id: impl Into<String>,
    account: impl Into<String>,
    asset_id: Option<&str>,
    limit: Option<usize>,
) -> RpcRequest {
    let mut request = RpcRequest::empty(id, METHOD_ACCOUNT_ASSETS)
        .with_param_value("account", string_value(account));
    if let Some(asset_id) = asset_id {
        request = request.with_param_value("asset_id", string_value(asset_id));
    }
    if let Some(limit) = limit {
        request = request.with_param_value("limit", usize_value(limit));
    }
    request
}

pub fn issuer_assets_request(
    id: impl Into<String>,
    issuer: impl Into<String>,
    limit: Option<usize>,
) -> RpcRequest {
    let mut request = RpcRequest::empty(id, METHOD_ISSUER_ASSETS)
        .with_param_value("issuer", string_value(issuer));
    if let Some(limit) = limit {
        request = request.with_param_value("limit", usize_value(limit));
    }
    request
}

pub fn escrow_info_request(id: impl Into<String>, escrow_id: impl Into<String>) -> RpcRequest {
    RpcRequest::empty(id, METHOD_ESCROW_INFO).with_param_value("escrow_id", string_value(escrow_id))
}

pub fn account_escrows_request(
    id: impl Into<String>,
    account: impl Into<String>,
    role: Option<&str>,
    state: Option<&str>,
    limit: Option<usize>,
) -> RpcRequest {
    let mut request = RpcRequest::empty(id, METHOD_ACCOUNT_ESCROWS)
        .with_param_value("account", string_value(account));
    if let Some(role) = role {
        request = request.with_param_value("role", string_value(role));
    }
    if let Some(state) = state {
        request = request.with_param_value("state", string_value(state));
    }
    if let Some(limit) = limit {
        request = request.with_param_value("limit", usize_value(limit));
    }
    request
}

pub fn nft_info_request(id: impl Into<String>, nft_id: impl Into<String>) -> RpcRequest {
    RpcRequest::empty(id, METHOD_NFT_INFO).with_param_value("nft_id", string_value(nft_id))
}

pub fn account_nfts_request(
    id: impl Into<String>,
    account: impl Into<String>,
    include_burned: Option<bool>,
    limit: Option<usize>,
) -> RpcRequest {
    let mut request = RpcRequest::empty(id, METHOD_ACCOUNT_NFTS)
        .with_param_value("account", string_value(account));
    if let Some(include_burned) = include_burned {
        request = request.with_param_value("include_burned", bool_value(include_burned));
    }
    if let Some(limit) = limit {
        request = request.with_param_value("limit", usize_value(limit));
    }
    request
}

pub fn issuer_nfts_request(
    id: impl Into<String>,
    issuer: impl Into<String>,
    collection_id: Option<&str>,
    include_burned: Option<bool>,
    limit: Option<usize>,
) -> RpcRequest {
    let mut request =
        RpcRequest::empty(id, METHOD_ISSUER_NFTS).with_param_value("issuer", string_value(issuer));
    if let Some(collection_id) = collection_id {
        request = request.with_param_value("collection_id", string_value(collection_id));
    }
    if let Some(include_burned) = include_burned {
        request = request.with_param_value("include_burned", bool_value(include_burned));
    }
    if let Some(limit) = limit {
        request = request.with_param_value("limit", usize_value(limit));
    }
    request
}

pub fn receipts_request(
    id: impl Into<String>,
    tx_id: Option<&str>,
    limit: Option<usize>,
) -> RpcRequest {
    let mut request = RpcRequest::empty(id, METHOD_RECEIPTS);
    if let Some(tx_id) = tx_id {
        request = request.with_param_value("tx_id", string_value(tx_id));
    }
    if let Some(limit) = limit {
        request = request.with_param_value("limit", usize_value(limit));
    }
    request
}

pub fn tx_request(id: impl Into<String>, tx_id: impl Into<String>) -> RpcRequest {
    tx_request_with_audit(id, tx_id, false)
}

pub fn tx_request_with_audit(
    id: impl Into<String>,
    tx_id: impl Into<String>,
    audit_block_log: bool,
) -> RpcRequest {
    let mut request =
        RpcRequest::empty(id, METHOD_TX).with_param_value("tx_id", string_value(tx_id));
    if audit_block_log {
        request = request.with_param_value("audit_block_log", bool_value(true));
    }
    request
}

pub fn blocks_request(id: impl Into<String>, limit: Option<usize>) -> RpcRequest {
    blocks_request_from_height(id, None, limit)
}

pub fn blocks_request_from_height(
    id: impl Into<String>,
    from_height: Option<u64>,
    limit: Option<usize>,
) -> RpcRequest {
    let mut request = RpcRequest::empty(id, METHOD_BLOCKS);
    if let Some(from_height) = from_height {
        request = request.with_param_value("from_height", u64_value(from_height));
    }
    if let Some(limit) = limit {
        request = request.with_param_value("limit", usize_value(limit));
    }
    request
}

pub fn validators_request(id: impl Into<String>) -> RpcRequest {
    RpcRequest::empty(id, METHOD_VALIDATORS)
}

pub fn manifests_request(id: impl Into<String>) -> RpcRequest {
    RpcRequest::empty(id, METHOD_MANIFESTS)
}

pub fn batch_archive_request(
    id: impl Into<String>,
    batch_kind: Option<&str>,
    batch_id: Option<&str>,
    limit: Option<usize>,
) -> RpcRequest {
    let mut request = RpcRequest::empty(id, METHOD_BATCH_ARCHIVE);
    if let Some(batch_kind) = batch_kind {
        request = request.with_param_value("batch_kind", string_value(batch_kind));
    }
    if let Some(batch_id) = batch_id {
        request = request.with_param_value("batch_id", string_value(batch_id));
    }
    if let Some(limit) = limit {
        request = request.with_param_value("limit", usize_value(limit));
    }
    request
}

pub fn archive_window_request(
    id: impl Into<String>,
    from_height: u64,
    to_height: u64,
    archive_uri: Option<&str>,
) -> RpcRequest {
    let mut request = RpcRequest::empty(id, METHOD_ARCHIVE_WINDOW)
        .with_param_value("from_height", u64_value(from_height))
        .with_param_value("to_height", u64_value(to_height));
    if let Some(archive_uri) = archive_uri {
        request = request.with_param_value("archive_uri", string_value(archive_uri));
    }
    request
}

pub fn mempool_submit_transfer_request(
    id: impl Into<String>,
    to: impl Into<String>,
    amount: u64,
    key_file: Option<&str>,
) -> RpcRequest {
    let mut request = RpcRequest::empty(id, METHOD_MEMPOOL_SUBMIT_TRANSFER)
        .with_param_value("to", string_value(to))
        .with_param_value("amount", u64_value(amount));
    if let Some(key_file) = key_file {
        request = request.with_param_value("key_file", string_value(key_file));
    }
    request
}

pub fn mempool_submit_signed_transfer_request(
    id: impl Into<String>,
    transfer_file: impl Into<String>,
) -> RpcRequest {
    RpcRequest::empty(id, METHOD_MEMPOOL_SUBMIT_SIGNED_TRANSFER)
        .with_param_value("transfer_file", string_value(transfer_file))
}

pub fn mempool_submit_signed_transfer_json_request(
    id: impl Into<String>,
    signed_transfer_json: impl Into<String>,
) -> RpcRequest {
    RpcRequest::empty(id, METHOD_MEMPOOL_SUBMIT_SIGNED_TRANSFER)
        .with_param_value("signed_transfer_json", string_value(signed_transfer_json))
}

pub fn mempool_submit_signed_payment_v2_json_request(
    id: impl Into<String>,
    signed_payment_v2_json: impl Into<String>,
) -> RpcRequest {
    RpcRequest::empty(id, METHOD_MEMPOOL_SUBMIT_SIGNED_PAYMENT_V2).with_param_value(
        "signed_payment_v2_json",
        string_value(signed_payment_v2_json),
    )
}

pub fn mempool_submit_signed_asset_transaction_json_request(
    id: impl Into<String>,
    signed_asset_transaction_json: impl Into<String>,
) -> RpcRequest {
    RpcRequest::empty(id, METHOD_MEMPOOL_SUBMIT_SIGNED_ASSET_TRANSACTION).with_param_value(
        "signed_asset_transaction_json",
        string_value(signed_asset_transaction_json),
    )
}

pub fn mempool_submit_signed_escrow_transaction_json_request(
    id: impl Into<String>,
    signed_escrow_transaction_json: impl Into<String>,
) -> RpcRequest {
    RpcRequest::empty(id, METHOD_MEMPOOL_SUBMIT_SIGNED_ESCROW_TRANSACTION).with_param_value(
        "signed_escrow_transaction_json",
        string_value(signed_escrow_transaction_json),
    )
}

pub fn mempool_submit_signed_nft_transaction_json_request(
    id: impl Into<String>,
    signed_nft_transaction_json: impl Into<String>,
) -> RpcRequest {
    RpcRequest::empty(id, METHOD_MEMPOOL_SUBMIT_SIGNED_NFT_TRANSACTION).with_param_value(
        "signed_nft_transaction_json",
        string_value(signed_nft_transaction_json),
    )
}

pub fn mempool_submit_signed_offer_transaction_json_request(
    id: impl Into<String>,
    signed_offer_transaction_json: impl Into<String>,
) -> RpcRequest {
    RpcRequest::empty(id, METHOD_MEMPOOL_SUBMIT_SIGNED_OFFER_TRANSACTION).with_param_value(
        "signed_offer_transaction_json",
        string_value(signed_offer_transaction_json),
    )
}

pub fn mempool_status_request(id: impl Into<String>) -> RpcRequest {
    RpcRequest::empty(id, METHOD_MEMPOOL_STATUS)
}

pub fn mempool_batch_request(
    id: impl Into<String>,
    batch_file: impl Into<String>,
    max_transactions: Option<usize>,
) -> RpcRequest {
    let mut request = RpcRequest::empty(id, METHOD_MEMPOOL_BATCH)
        .with_param_value("batch_file", string_value(batch_file));
    if let Some(max_transactions) = max_transactions {
        request = request.with_param_value("max_transactions", usize_value(max_transactions));
    }
    request
}

pub fn apply_batch_request(id: impl Into<String>, batch_file: impl Into<String>) -> RpcRequest {
    RpcRequest::empty(id, METHOD_APPLY_BATCH)
        .with_param_value("batch_file", string_value(batch_file))
}

pub fn shield_batch_mint_request(
    id: impl Into<String>,
    owner: impl Into<String>,
    amount: u64,
    asset_id: Option<&str>,
    memo: Option<&str>,
    batch_file: impl Into<String>,
) -> RpcRequest {
    let mut request = RpcRequest::empty(id, METHOD_SHIELD_BATCH_MINT)
        .with_param_value("owner", string_value(owner))
        .with_param_value("amount", u64_value(amount))
        .with_param_value("batch_file", string_value(batch_file));
    if let Some(asset_id) = asset_id {
        request = request.with_param_value("asset_id", string_value(asset_id));
    }
    if let Some(memo) = memo {
        request = request.with_param_value("memo", string_value(memo));
    }
    request
}

pub fn shield_batch_spend_request(
    id: impl Into<String>,
    note_id: impl Into<String>,
    to: impl Into<String>,
    amount: u64,
    memo: Option<&str>,
    batch_file: impl Into<String>,
) -> RpcRequest {
    let mut request = RpcRequest::empty(id, METHOD_SHIELD_BATCH_SPEND)
        .with_param_value("note_id", string_value(note_id))
        .with_param_value("to", string_value(to))
        .with_param_value("amount", u64_value(amount))
        .with_param_value("batch_file", string_value(batch_file));
    if let Some(memo) = memo {
        request = request.with_param_value("memo", string_value(memo));
    }
    request
}

pub fn shield_batch_migrate_request(
    id: impl Into<String>,
    note_id: impl Into<String>,
    target_pool: impl Into<String>,
    memo: Option<&str>,
    batch_file: impl Into<String>,
) -> RpcRequest {
    let mut request = RpcRequest::empty(id, METHOD_SHIELD_BATCH_MIGRATE)
        .with_param_value("note_id", string_value(note_id))
        .with_param_value("target_pool", string_value(target_pool))
        .with_param_value("batch_file", string_value(batch_file));
    if let Some(memo) = memo {
        request = request.with_param_value("memo", string_value(memo));
    }
    request
}

pub fn shield_batch_orchard_request(
    id: impl Into<String>,
    action_file: impl Into<String>,
    batch_file: impl Into<String>,
) -> RpcRequest {
    RpcRequest::empty(id, METHOD_SHIELD_BATCH_ORCHARD)
        .with_param_value("action_file", string_value(action_file))
        .with_param_value("batch_file", string_value(batch_file))
}

pub fn shield_batch_orchard_json_request(
    id: impl Into<String>,
    action_json: impl Into<String>,
) -> RpcRequest {
    RpcRequest::empty(id, METHOD_SHIELD_BATCH_ORCHARD)
        .with_param_value("action_json", string_value(action_json))
}

pub fn shield_batch_orchard_deposit_request(
    id: impl Into<String>,
    deposit_file: impl Into<String>,
    batch_file: impl Into<String>,
) -> RpcRequest {
    RpcRequest::empty(id, METHOD_SHIELD_BATCH_ORCHARD_DEPOSIT)
        .with_param_value("deposit_file", string_value(deposit_file))
        .with_param_value("batch_file", string_value(batch_file))
}

pub fn shield_batch_orchard_deposit_json_request(
    id: impl Into<String>,
    deposit_json: impl Into<String>,
) -> RpcRequest {
    RpcRequest::empty(id, METHOD_SHIELD_BATCH_ORCHARD_DEPOSIT)
        .with_param_value("deposit_json", string_value(deposit_json))
}

#[allow(clippy::too_many_arguments)]
pub fn shield_batch_orchard_withdraw_request(
    id: impl Into<String>,
    action_file: impl Into<String>,
    to: impl Into<String>,
    amount: u64,
    fee: u64,
    policy_id: Option<&str>,
    disclosure_hash: Option<&str>,
    batch_file: impl Into<String>,
) -> RpcRequest {
    let mut request = RpcRequest::empty(id, METHOD_SHIELD_BATCH_ORCHARD_WITHDRAW)
        .with_param_value("action_file", string_value(action_file))
        .with_param_value("to", string_value(to))
        .with_param_value("amount", u64_value(amount))
        .with_param_value("fee", u64_value(fee))
        .with_param_value("batch_file", string_value(batch_file));
    if let Some(policy_id) = policy_id {
        request = request.with_param_value("policy_id", string_value(policy_id));
    }
    if let Some(disclosure_hash) = disclosure_hash {
        request = request.with_param_value("disclosure_hash", string_value(disclosure_hash));
    }
    request
}

pub fn shield_batch_orchard_withdraw_json_request(
    id: impl Into<String>,
    action_json: impl Into<String>,
    to: impl Into<String>,
    amount: u64,
    fee: u64,
    policy_id: Option<&str>,
    disclosure_hash: Option<&str>,
) -> RpcRequest {
    let mut request = RpcRequest::empty(id, METHOD_SHIELD_BATCH_ORCHARD_WITHDRAW)
        .with_param_value("action_json", string_value(action_json))
        .with_param_value("to", string_value(to))
        .with_param_value("amount", u64_value(amount))
        .with_param_value("fee", u64_value(fee));
    if let Some(policy_id) = policy_id {
        request = request.with_param_value("policy_id", string_value(policy_id));
    }
    if let Some(disclosure_hash) = disclosure_hash {
        request = request.with_param_value("disclosure_hash", string_value(disclosure_hash));
    }
    request
}

pub fn shield_batch_swap_request(
    id: impl Into<String>,
    swap_file: impl Into<String>,
    batch_file: impl Into<String>,
) -> RpcRequest {
    RpcRequest::empty(id, METHOD_SHIELD_BATCH_SWAP)
        .with_param_value("swap_file", string_value(swap_file))
        .with_param_value("batch_file", string_value(batch_file))
}

pub fn shield_batch_swap_json_request(
    id: impl Into<String>,
    swap_json: impl Into<String>,
) -> RpcRequest {
    RpcRequest::empty(id, METHOD_SHIELD_BATCH_SWAP)
        .with_param_value("swap_json", string_value(swap_json))
}

pub fn apply_shield_batch_request(
    id: impl Into<String>,
    batch_file: impl Into<String>,
) -> RpcRequest {
    RpcRequest::empty(id, METHOD_APPLY_SHIELD_BATCH)
        .with_param_value("batch_file", string_value(batch_file))
}

pub fn shield_scan_request(id: impl Into<String>, owner: impl Into<String>) -> RpcRequest {
    RpcRequest::empty(id, METHOD_SHIELD_SCAN).with_param_value("owner", string_value(owner))
}

pub fn shield_disclose_request(id: impl Into<String>, note_id: impl Into<String>) -> RpcRequest {
    RpcRequest::empty(id, METHOD_SHIELD_DISCLOSE).with_param_value("note_id", string_value(note_id))
}

pub fn shield_turnstile_request(id: impl Into<String>) -> RpcRequest {
    RpcRequest::empty(id, METHOD_SHIELD_TURNSTILE)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BridgeBatchDomainParams {
    pub domain_id: String,
    pub name: String,
    pub source_chain: Option<String>,
    pub target_chain: Option<String>,
    pub bridge_id: Option<String>,
    pub door_account: Option<String>,
    pub inbound_cap: u64,
    pub outbound_cap: u64,
    pub batch_file: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BridgeBatchTransferParams {
    pub domain_id: String,
    pub direction: String,
    pub from: String,
    pub to: String,
    pub asset_id: String,
    pub amount: u64,
    pub witness_id: String,
    pub witness_epoch: Option<u32>,
    pub witness_signer: Option<String>,
    pub batch_file: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavcoinBridgePacketParams {
    pub route_id: String,
    pub packet_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavcoinBridgeClaimsParams {
    pub route_id: String,
    pub limit: Option<usize>,
    pub include_terminal: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavcoinBridgeSupplyStatusParams {
    pub route_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavcoinBridgeReceiptReplayParams {
    pub route_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavcoinBridgePacketPreflightParams {
    pub route_id: String,
    pub packet_file: String,
}

pub fn bridge_status_request(id: impl Into<String>) -> RpcRequest {
    RpcRequest::empty(id, METHOD_BRIDGE_STATUS)
}

pub fn navcoin_bridge_routes_request(id: impl Into<String>) -> RpcRequest {
    RpcRequest::empty(id, METHOD_NAVCOIN_BRIDGE_ROUTES)
}

pub fn navcoin_bridge_packet_request(
    id: impl Into<String>,
    params: NavcoinBridgePacketParams,
) -> RpcRequest {
    RpcRequest::empty(id, METHOD_NAVCOIN_BRIDGE_PACKET)
        .with_param_value("route_id", string_value(params.route_id))
        .with_param_value("packet_hash", string_value(params.packet_hash))
}

pub fn navcoin_bridge_claims_request(
    id: impl Into<String>,
    params: NavcoinBridgeClaimsParams,
) -> RpcRequest {
    let mut request = RpcRequest::empty(id, METHOD_NAVCOIN_BRIDGE_CLAIMS)
        .with_param_value("route_id", string_value(params.route_id));
    if let Some(limit) = params.limit {
        request = request.with_param_value("limit", usize_value(limit));
    }
    if let Some(include_terminal) = params.include_terminal {
        request = request.with_param_value("include_terminal", bool_value(include_terminal));
    }
    request
}

pub fn navcoin_bridge_supply_status_request(
    id: impl Into<String>,
    params: NavcoinBridgeSupplyStatusParams,
) -> RpcRequest {
    RpcRequest::empty(id, METHOD_NAVCOIN_BRIDGE_SUPPLY_STATUS)
        .with_param_value("route_id", string_value(params.route_id))
}

pub fn navcoin_bridge_receipt_replay_request(
    id: impl Into<String>,
    params: NavcoinBridgeReceiptReplayParams,
) -> RpcRequest {
    RpcRequest::empty(id, METHOD_NAVCOIN_BRIDGE_RECEIPT_REPLAY)
        .with_param_value("route_id", string_value(params.route_id))
}

pub fn navcoin_bridge_packet_preflight_request(
    id: impl Into<String>,
    params: NavcoinBridgePacketPreflightParams,
) -> RpcRequest {
    RpcRequest::empty(id, METHOD_NAVCOIN_BRIDGE_PACKET_PREFLIGHT)
        .with_param_value("route_id", string_value(params.route_id))
        .with_param_value("packet_file", string_value(params.packet_file))
}

pub fn bridge_batch_domain_request(
    id: impl Into<String>,
    params: BridgeBatchDomainParams,
) -> RpcRequest {
    let mut request = RpcRequest::empty(id, METHOD_BRIDGE_BATCH_DOMAIN)
        .with_param_value("domain_id", string_value(params.domain_id))
        .with_param_value("name", string_value(params.name))
        .with_param_value("inbound_cap", u64_value(params.inbound_cap))
        .with_param_value("outbound_cap", u64_value(params.outbound_cap))
        .with_param_value("batch_file", string_value(params.batch_file));
    if let Some(source_chain) = params.source_chain {
        request = request.with_param_value("source_chain", string_value(source_chain));
    }
    if let Some(target_chain) = params.target_chain {
        request = request.with_param_value("target_chain", string_value(target_chain));
    }
    if let Some(bridge_id) = params.bridge_id {
        request = request.with_param_value("bridge_id", string_value(bridge_id));
    }
    if let Some(door_account) = params.door_account {
        request = request.with_param_value("door_account", string_value(door_account));
    }
    request
}

pub fn bridge_batch_transfer_request(
    id: impl Into<String>,
    params: BridgeBatchTransferParams,
) -> RpcRequest {
    let mut request = RpcRequest::empty(id, METHOD_BRIDGE_BATCH_TRANSFER)
        .with_param_value("domain_id", string_value(params.domain_id))
        .with_param_value("direction", string_value(params.direction))
        .with_param_value("from", string_value(params.from))
        .with_param_value("to", string_value(params.to))
        .with_param_value("asset_id", string_value(params.asset_id))
        .with_param_value("amount", u64_value(params.amount))
        .with_param_value("witness_id", string_value(params.witness_id))
        .with_param_value("batch_file", string_value(params.batch_file));
    if let Some(witness_epoch) = params.witness_epoch {
        request = request.with_param_value("witness_epoch", u32_value(witness_epoch));
    }
    if let Some(witness_signer) = params.witness_signer {
        request = request.with_param_value("witness_signer", string_value(witness_signer));
    }
    request
}

pub fn bridge_batch_pause_request(
    id: impl Into<String>,
    domain_id: impl Into<String>,
    batch_file: impl Into<String>,
) -> RpcRequest {
    RpcRequest::empty(id, METHOD_BRIDGE_BATCH_PAUSE)
        .with_param_value("domain_id", string_value(domain_id))
        .with_param_value("batch_file", string_value(batch_file))
}

pub fn bridge_batch_resume_request(
    id: impl Into<String>,
    domain_id: impl Into<String>,
    batch_file: impl Into<String>,
) -> RpcRequest {
    RpcRequest::empty(id, METHOD_BRIDGE_BATCH_RESUME)
        .with_param_value("domain_id", string_value(domain_id))
        .with_param_value("batch_file", string_value(batch_file))
}

pub fn apply_bridge_batch_request(
    id: impl Into<String>,
    batch_file: impl Into<String>,
) -> RpcRequest {
    RpcRequest::empty(id, METHOD_APPLY_BRIDGE_BATCH)
        .with_param_value("batch_file", string_value(batch_file))
}

pub fn fastswap_capabilities_request(id: impl Into<String>) -> RpcRequest {
    RpcRequest::empty(id, METHOD_FASTSWAP_CAPABILITIES)
}

pub fn fastswap_preview_request(
    id: impl Into<String>,
    signed_intent_json: impl Into<String>,
) -> RpcRequest {
    RpcRequest::empty(id, METHOD_FASTSWAP_PREVIEW)
        .with_param_value("signed_intent_json", string_value(signed_intent_json))
}

pub fn fastswap_prepare_request(
    id: impl Into<String>,
    signed_intent_json: impl Into<String>,
) -> RpcRequest {
    RpcRequest::empty(id, METHOD_FASTSWAP_PREPARE)
        .with_param_value("signed_intent_json", string_value(signed_intent_json))
}

pub fn fastswap_commit_request(
    id: impl Into<String>,
    lock_qc_json: impl Into<String>,
) -> RpcRequest {
    RpcRequest::empty(id, METHOD_FASTSWAP_COMMIT)
        .with_param_value("lock_qc_json", string_value(lock_qc_json))
}

pub fn fastswap_apply_request(
    id: impl Into<String>,
    decision_qc_json: impl Into<String>,
    signed_intent_json: impl Into<String>,
) -> RpcRequest {
    RpcRequest::empty(id, METHOD_FASTSWAP_APPLY)
        .with_param_value("decision_qc_json", string_value(decision_qc_json))
        .with_param_value("signed_intent_json", string_value(signed_intent_json))
}

pub fn fastswap_catch_up_request(
    id: impl Into<String>,
    lock_qc_json: impl Into<String>,
    decision_qc_json: impl Into<String>,
    signed_intent_json: impl Into<String>,
) -> RpcRequest {
    RpcRequest::empty(id, METHOD_FASTSWAP_CATCH_UP)
        .with_param_value("lock_qc_json", string_value(lock_qc_json))
        .with_param_value("decision_qc_json", string_value(decision_qc_json))
        .with_param_value("signed_intent_json", string_value(signed_intent_json))
}

pub fn fastswap_status_request(id: impl Into<String>, swap_id: impl Into<String>) -> RpcRequest {
    RpcRequest::empty(id, METHOD_FASTSWAP_STATUS)
        .with_param_value("swap_id", string_value(swap_id))
}

pub fn fastswap_effects_request(id: impl Into<String>, swap_id: impl Into<String>) -> RpcRequest {
    RpcRequest::empty(id, METHOD_FASTSWAP_EFFECTS)
        .with_param_value("swap_id", string_value(swap_id))
}

pub fn fastswap_votes_request(
    id: impl Into<String>,
    swap_id: impl Into<String>,
    phase: postfiat_types::FastSwapPhaseV1,
    round: u64,
) -> RpcRequest {
    RpcRequest::empty(id, METHOD_FASTSWAP_VOTES)
        .with_param_value("swap_id", string_value(swap_id))
        .with_param_value("phase", string_value(fastswap_phase_name(phase)))
        .with_param_value("round", u64_value(round))
}

pub fn fastswap_phase_name(phase: postfiat_types::FastSwapPhaseV1) -> &'static str {
    match phase {
        postfiat_types::FastSwapPhaseV1::Precommit => "precommit",
        postfiat_types::FastSwapPhaseV1::Commit => "commit",
        postfiat_types::FastSwapPhaseV1::Effects => "effects",
        postfiat_types::FastSwapPhaseV1::NewRound => "new_round",
        postfiat_types::FastSwapPhaseV1::CancelApply => "cancel_apply",
    }
}

pub fn parse_fastswap_phase(value: &str) -> Option<postfiat_types::FastSwapPhaseV1> {
    match value {
        "precommit" => Some(postfiat_types::FastSwapPhaseV1::Precommit),
        "commit" => Some(postfiat_types::FastSwapPhaseV1::Commit),
        "effects" => Some(postfiat_types::FastSwapPhaseV1::Effects),
        "new_round" => Some(postfiat_types::FastSwapPhaseV1::NewRound),
        "cancel_apply" => Some(postfiat_types::FastSwapPhaseV1::CancelApply),
        _ => None,
    }
}

pub fn fastswap_new_round_vote_request(
    id: impl Into<String>,
    swap_id: impl Into<String>,
    target_round: u64,
) -> RpcRequest {
    RpcRequest::empty(id, METHOD_FASTSWAP_NEW_ROUND_VOTE)
        .with_param_value("swap_id", string_value(swap_id))
        .with_param_value("target_round", u64_value(target_round))
}

pub fn fastswap_propose_round_request(
    id: impl Into<String>,
    proposal_json: impl Into<String>,
) -> RpcRequest {
    RpcRequest::empty(id, METHOD_FASTSWAP_PROPOSE_ROUND)
        .with_param_value("proposal_json", string_value(proposal_json))
}

pub fn fastswap_precommit_request(
    id: impl Into<String>,
    proposal_json: impl Into<String>,
) -> RpcRequest {
    RpcRequest::empty(id, METHOD_FASTSWAP_PRECOMMIT)
        .with_param_value("proposal_json", string_value(proposal_json))
}

pub fn fastswap_commit_round_request(
    id: impl Into<String>,
    precommit_qc_json: impl Into<String>,
) -> RpcRequest {
    RpcRequest::empty(id, METHOD_FASTSWAP_COMMIT_ROUND)
        .with_param_value("precommit_qc_json", string_value(precommit_qc_json))
}

pub fn fastswap_cancel_apply_request(
    id: impl Into<String>,
    decision_qc_json: impl Into<String>,
) -> RpcRequest {
    RpcRequest::empty(id, METHOD_FASTSWAP_CANCEL_APPLY)
        .with_param_value("decision_qc_json", string_value(decision_qc_json))
}

pub fn fastlane_exit_request(
    id: impl Into<String>,
    signed_exit_json: impl Into<String>,
) -> RpcRequest {
    RpcRequest::empty(id, METHOD_FASTLANE_EXIT)
        .with_param_value("signed_exit_json", string_value(signed_exit_json))
}

pub fn fastswap_checkpoint_status_request(
    id: impl Into<String>,
    previous_checkpoint_id: Option<String>,
) -> RpcRequest {
    let request = RpcRequest::empty(id, METHOD_FASTSWAP_CHECKPOINT_STATUS);
    match previous_checkpoint_id {
        Some(value) => request.with_param_value("previous_checkpoint_id", string_value(value)),
        None => request,
    }
}

pub fn fastswap_objects_request(
    id: impl Into<String>,
    owner_pubkey: impl Into<String>,
    asset_id: Option<String>,
    cursor: Option<(String, u64)>,
    limit: u64,
) -> RpcRequest {
    let mut request = RpcRequest::empty(id, METHOD_FASTSWAP_OBJECTS)
        .with_param_value("owner_pubkey", string_value(owner_pubkey))
        .with_param_value("limit", u64_value(limit));
    if let Some(asset_id) = asset_id {
        request = request.with_param_value("asset_id", string_value(asset_id));
    }
    if let Some((object_id, version)) = cursor {
        request = request
            .with_param_value("cursor_object_id", string_value(object_id))
            .with_param_value("cursor_version", u64_value(version));
    }
    request
}

pub fn fastswap_policy_by_hash_request(
    id: impl Into<String>,
    policy_hash: impl Into<String>,
) -> RpcRequest {
    RpcRequest::empty(id, METHOD_FASTSWAP_POLICY)
        .with_param_value("policy_hash", string_value(policy_hash))
}

pub fn fastswap_policy_by_pair_request(
    id: impl Into<String>,
    asset_0: impl Into<String>,
    asset_1: impl Into<String>,
) -> RpcRequest {
    RpcRequest::empty(id, METHOD_FASTSWAP_POLICY)
        .with_param_value("asset_0", string_value(asset_0))
        .with_param_value("asset_1", string_value(asset_1))
}

pub fn fastlane_asset_control_prepare_request(
    id: impl Into<String>,
    signed_command_json: impl Into<String>,
) -> RpcRequest {
    RpcRequest::empty(id, METHOD_FASTLANE_ASSET_CONTROL_PREPARE)
        .with_param_value("signed_command_json", string_value(signed_command_json))
}

pub fn fastlane_asset_control_preview_request(
    id: impl Into<String>,
    signed_command_json: impl Into<String>,
) -> RpcRequest {
    RpcRequest::empty(id, METHOD_FASTLANE_ASSET_CONTROL_PREVIEW)
        .with_param_value("signed_command_json", string_value(signed_command_json))
}

pub fn fastlane_asset_control_apply_request(
    id: impl Into<String>,
    decision_qc_json: impl Into<String>,
    signed_command_json: impl Into<String>,
) -> RpcRequest {
    RpcRequest::empty(id, METHOD_FASTLANE_ASSET_CONTROL_APPLY)
        .with_param_value("decision_qc_json", string_value(decision_qc_json))
        .with_param_value("signed_command_json", string_value(signed_command_json))
}

pub fn fastlane_asset_control_catch_up_request(
    id: impl Into<String>,
    lock_qc_json: impl Into<String>,
    decision_qc_json: impl Into<String>,
    signed_command_json: impl Into<String>,
) -> RpcRequest {
    RpcRequest::empty(id, METHOD_FASTLANE_ASSET_CONTROL_CATCH_UP)
        .with_param_value("lock_qc_json", string_value(lock_qc_json))
        .with_param_value("decision_qc_json", string_value(decision_qc_json))
        .with_param_value("signed_command_json", string_value(signed_command_json))
}

pub fn mempool_submit_fastlane_primary_request(
    id: impl Into<String>,
    fastlane_primary_json: impl Into<String>,
) -> RpcRequest {
    RpcRequest::empty(id, METHOD_MEMPOOL_SUBMIT_FASTLANE_PRIMARY)
        .with_param_value("fastlane_primary_json", string_value(fastlane_primary_json))
}

pub fn mempool_submit_fastlane_primary_finality_request(
    id: impl Into<String>,
    fastlane_primary_json: impl Into<String>,
) -> RpcRequest {
    RpcRequest::empty(id, METHOD_MEMPOOL_SUBMIT_FASTLANE_PRIMARY_FINALITY)
        .with_param_value("fastlane_primary_json", string_value(fastlane_primary_json))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RpcRequestKind {
    Status,
    ServerInfo,
    Metrics,
    Ledger,
    VerifyState,
    ValidateLocalKeys { validators: Option<u32> },
    Account,
    AccountTx,
    Fee,
    TransferFeeQuote,
    AtomicSwapFeeQuote,
    AssetFeeQuote,
    EscrowFeeQuote,
    NftFeeQuote,
    OfferFeeQuote,
    AtomicSettlementTemplate,
    OfferInfo,
    AccountOffers,
    BookOffers,
    AssetInfo,
    AccountLines,
    AccountAssets,
    IssuerAssets,
    EscrowInfo,
    AccountEscrows,
    NftInfo,
    AccountNfts,
    IssuerNfts,
    Receipts,
    Tx,
    Blocks,
    Validators,
    Manifests,
    BatchArchive,
    ArchiveWindow,
    MempoolSubmitTransfer,
    MempoolSubmitSignedTransfer,
    MempoolSubmitSignedPaymentV2,
    MempoolSubmitSignedAssetTransaction,
    MempoolSubmitSignedEscrowTransaction,
    MempoolSubmitSignedNftTransaction,
    MempoolSubmitSignedOfferTransaction,
    MempoolSubmitSignedAtomicSwapTransaction,
    MempoolSubmitSignedAtomicSwapTransactionFinality,
    MempoolStatus,
    MempoolBatch,
    ApplyBatch,
    ShieldBatchMint,
    ShieldBatchSpend,
    ShieldBatchMigrate,
    ShieldBatchOrchard,
    ShieldBatchOrchardDeposit,
    ShieldBatchOrchardWithdraw,
    ShieldBatchSwap,
    ApplyShieldBatch,
    ShieldScan,
    ShieldDisclose,
    ShieldTurnstile,
    BridgeStatus,
    NavcoinBridgeRoutes,
    NavcoinBridgePacket,
    NavcoinBridgeClaims,
    NavcoinBridgeSupplyStatus,
    NavcoinBridgeReceiptReplay,
    NavcoinBridgePacketPreflight,
    BridgeBatchDomain,
    BridgeBatchTransfer,
    BridgeBatchPause,
    BridgeBatchResume,
    ApplyBridgeBatch,
    FastSwapCapabilities,
    FastSwapPreview,
    FastSwapPrepare,
    FastSwapCommit,
    FastSwapApply,
    FastSwapCatchUp,
    FastSwapStatus,
    FastSwapEffects,
    FastSwapVotes,
    FastSwapNewRoundVote,
    FastSwapProposeRound,
    FastSwapPrecommit,
    FastSwapCommitRound,
    FastSwapCancelApply,
    FastLaneExit,
    FastSwapCheckpointStatus,
    FastSwapObjects,
    FastSwapPolicy,
    FastLaneAssetControlPrepare,
    FastLaneAssetControlPreview,
    FastLaneAssetControlApply,
    FastLaneAssetControlCatchUp,
    MempoolSubmitFastLanePrimary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RpcRequestValidationError {
    Protocol(RpcProtocolError),
    UnexpectedId { expected: String, found: String },
    UnexpectedMethod { expected: String, found: String },
    InvalidParams { field: String, message: String },
}

impl fmt::Display for RpcRequestValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Protocol(error) => write!(f, "{error}"),
            Self::UnexpectedId { expected, found } => {
                write!(
                    f,
                    "rpc request id `{found}` did not match expected `{expected}`"
                )
            }
            Self::UnexpectedMethod { expected, found } => {
                write!(
                    f,
                    "rpc request method `{found}` did not match expected `{expected}`"
                )
            }
            Self::InvalidParams { field, message } => {
                write!(f, "rpc request params field `{field}` invalid: {message}")
            }
        }
    }
}

impl std::error::Error for RpcRequestValidationError {}

pub fn request_from_json(raw: &str) -> Result<RpcRequest, serde_json::Error> {
    serde_json::from_str(raw)
}

pub fn read_request_file(path: impl AsRef<Path>) -> io::Result<RpcRequest> {
    let raw = read_bounded_text_file(path.as_ref(), "rpc request", MAX_RPC_REQUEST_BYTES)?;
    let request = request_from_json(&raw).map_err(invalid_json)?;
    request.validate_protocol().map_err(invalid_protocol_data)?;
    Ok(request)
}

pub fn write_request_file(path: impl AsRef<Path>, request: &RpcRequest) -> io::Result<()> {
    request
        .validate_protocol()
        .map_err(invalid_protocol_input)?;
    let json = request.to_pretty_json().map_err(invalid_json)?;
    if json.len() > MAX_RPC_REQUEST_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("rpc request exceeds {MAX_RPC_REQUEST_BYTES} bytes"),
        ));
    }
    fs::write(path, json)
}

pub fn validate_request(
    request: &RpcRequest,
    expected_id: Option<&str>,
    expected_kind: Option<RpcRequestKind>,
) -> Result<(), RpcRequestValidationError> {
    request
        .validate_protocol()
        .map_err(RpcRequestValidationError::Protocol)?;
    if let Some(expected) = expected_id {
        if request.id != expected {
            return Err(RpcRequestValidationError::UnexpectedId {
                expected: expected.to_string(),
                found: request.id.clone(),
            });
        }
    }
    if let Some(expected_kind) = expected_kind {
        let expected_method = request_kind_method(expected_kind);
        if request.method != expected_method {
            return Err(RpcRequestValidationError::UnexpectedMethod {
                expected: expected_method.to_string(),
                found: request.method.clone(),
            });
        }
        validate_request_params(request, expected_kind)?;
    }
    if contains_key_material_field(&request.params) {
        return Err(invalid_request_params(
            "params",
            "request contains key material fields",
        ));
    }
    Ok(())
}

pub fn validate_request_file(
    path: impl AsRef<Path>,
    expected_id: Option<&str>,
    expected_kind: Option<RpcRequestKind>,
) -> io::Result<RpcRequest> {
    let request = read_request_file(path)?;
    validate_request(&request, expected_id, expected_kind).map_err(invalid_request_validation)?;
    Ok(request)
}

fn invalid_json(error: serde_json::Error) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error)
}

fn read_bounded_text_file(path: &Path, label: &str, max_bytes: usize) -> io::Result<String> {
    let metadata = fs::metadata(path).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("failed to stat {label} `{}`: {error}", path.display()),
        )
    })?;
    if metadata.len() > max_bytes as u64 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} `{}` exceeds {max_bytes} bytes", path.display()),
        ));
    }
    let raw = fs::read_to_string(path).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("failed to read {label} `{}`: {error}", path.display()),
        )
    })?;
    if raw.len() > max_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} `{}` exceeds {max_bytes} bytes", path.display()),
        ));
    }
    Ok(raw)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RpcEvent {
    pub event_type: String,
    pub subject: String,
    pub message: String,
}

impl RpcEvent {
    pub fn new(
        event_type: impl Into<String>,
        subject: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            event_type: event_type.into(),
            subject: subject.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RpcError {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RpcResponse {
    pub version: String,
    pub id: String,
    pub ok: bool,
    pub result: Option<Value>,
    pub error: Option<RpcError>,
    pub events: Vec<RpcEvent>,
}

impl RpcResponse {
    pub fn result_as<T: DeserializeOwned>(&self) -> Result<T, RpcResponseDecodeError> {
        if !self.ok {
            if let Some(error) = &self.error {
                return Err(RpcResponseDecodeError::RpcError {
                    code: error.code.clone(),
                    message: error.message.clone(),
                });
            }
            return Err(RpcResponseDecodeError::RpcError {
                code: "rpc_error".to_string(),
                message: "rpc response was not ok".to_string(),
            });
        }
        let result = self
            .result
            .clone()
            .ok_or(RpcResponseDecodeError::MissingResult)?;
        serde_json::from_value(result)
            .map_err(|error| RpcResponseDecodeError::InvalidResult(error.to_string()))
    }

    pub fn to_pretty_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn validate_protocol(&self) -> Result<(), RpcProtocolError> {
        validate_common_fields(&self.version, &self.id)?;
        if self.ok {
            if self.result.is_none() {
                return Err(RpcProtocolError::ResponseMissingResult);
            }
            if self.error.is_some() {
                return Err(RpcProtocolError::ResponseUnexpectedError);
            }
        } else {
            if self.result.is_some() {
                return Err(RpcProtocolError::ResponseUnexpectedResult);
            }
            let error = self
                .error
                .as_ref()
                .ok_or(RpcProtocolError::ResponseMissingError)?;
            if error.code.trim().is_empty() {
                return Err(RpcProtocolError::ResponseEmptyErrorCode);
            }
            if error.message.trim().is_empty() {
                return Err(RpcProtocolError::ResponseEmptyErrorMessage);
            }
            validate_response_error_field("code", &error.code)?;
            validate_response_error_field("message", &error.message)?;
        }
        validate_response_events(&self.events)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RpcResponseDecodeError {
    RpcError { code: String, message: String },
    MissingResult,
    InvalidResult(String),
}

impl fmt::Display for RpcResponseDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RpcError { code, message } => write!(f, "{code}: {message}"),
            Self::MissingResult => write!(f, "rpc response missing result"),
            Self::InvalidResult(error) => write!(f, "rpc result decode failed: {error}"),
        }
    }
}

impl std::error::Error for RpcResponseDecodeError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RpcResponseValidationError {
    Protocol(RpcProtocolError),
    UnexpectedId { expected: String, found: String },
    ExpectedSuccess { code: String, message: String },
    InvalidResult { field: String, message: String },
}

impl fmt::Display for RpcResponseValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Protocol(error) => write!(f, "{error}"),
            Self::UnexpectedId { expected, found } => {
                write!(
                    f,
                    "rpc response id `{found}` did not match expected `{expected}`"
                )
            }
            Self::ExpectedSuccess { code, message } => {
                write!(f, "rpc response was not ok: {code}: {message}")
            }
            Self::InvalidResult { field, message } => {
                write!(f, "rpc response result field `{field}` invalid: {message}")
            }
        }
    }
}

impl std::error::Error for RpcResponseValidationError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WalletSdkError {
    message: String,
}

impl WalletSdkError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for WalletSdkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for WalletSdkError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RpcResponseKind {
    Status,
    ServerInfo,
    Metrics,
    Ledger,
    VerifyState,
    ValidateLocalKeys { validators: Option<u32> },
    Account,
    AccountTx,
    Fee,
    TransferFeeQuote,
    AtomicSwapFeeQuote,
    AssetFeeQuote,
    EscrowFeeQuote,
    NftFeeQuote,
    OfferFeeQuote,
    AtomicSettlementTemplate,
    OfferInfo,
    AccountOffers,
    BookOffers,
    AssetInfo,
    AccountLines,
    AccountAssets,
    IssuerAssets,
    EscrowInfo,
    AccountEscrows,
    NftInfo,
    AccountNfts,
    IssuerNfts,
    Receipts,
    Tx,
    Blocks,
    Validators,
    Manifests,
    BatchArchive,
    ArchiveWindow,
    MempoolSubmitTransfer,
    MempoolSubmitSignedTransfer,
    MempoolSubmitSignedPaymentV2,
    MempoolSubmitSignedAssetTransaction,
    MempoolSubmitSignedEscrowTransaction,
    MempoolSubmitSignedNftTransaction,
    MempoolSubmitSignedOfferTransaction,
    MempoolSubmitSignedAtomicSwapTransaction,
    MempoolSubmitSignedAtomicSwapTransactionFinality,
    MempoolStatus,
    MempoolBatch,
    ApplyBatch,
    ShieldBatchMint,
    ShieldBatchSpend,
    ShieldBatchMigrate,
    ShieldBatchOrchard,
    ShieldBatchOrchardDeposit,
    ShieldBatchOrchardWithdraw,
    ShieldBatchSwap,
    ApplyShieldBatch,
    ShieldScan,
    ShieldDisclose,
    ShieldTurnstile,
    BridgeStatus,
    NavcoinBridgeRoutes,
    NavcoinBridgePacket,
    NavcoinBridgeClaims,
    NavcoinBridgeSupplyStatus,
    NavcoinBridgeReceiptReplay,
    NavcoinBridgePacketPreflight,
    BridgeBatchDomain,
    BridgeBatchTransfer,
    BridgeBatchPause,
    BridgeBatchResume,
    ApplyBridgeBatch,
    FastSwapCapabilities,
    FastSwapPreview,
    FastSwapVote,
    FastSwapStatus,
    FastSwapEffects,
    FastSwapVoteEvidence,
    FastSwapNewRoundVote,
    FastLaneExitVote,
    FastSwapCheckpointStatus,
    FastSwapObjects,
    FastSwapPolicy,
    FastLaneAssetControlPreview,
    MempoolSubmitFastLanePrimary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferFeeQuoteSummary {
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub from: String,
    pub to: String,
    pub amount: u64,
    pub sequence: u64,
    pub sequence_source: String,
    pub sender_balance: u64,
    pub sender_sequence: u64,
    pub mempool_pending_for_sender: u64,
    pub recipient_exists: bool,
    pub will_create_recipient_account: bool,
    pub base_transfer_fee: u64,
    pub state_expansion_fee: u64,
    pub minimum_fee: u64,
    pub account_reserve: u64,
    pub transfer_account_creation_fee: u64,
    pub transfer_fee_byte_quantum: u64,
    pub transfer_fee_per_quantum: u64,
    pub transfer_weight_bytes: u64,
    pub sender_balance_after_amount_and_fee: Option<u64>,
    pub sender_meets_reserve_after_transfer: bool,
    pub recipient_balance_after_amount: Option<u64>,
    pub recipient_meets_reserve_after_transfer: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetFeeQuoteSummary {
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub transaction_kind: String,
    pub source: String,
    pub sequence: u64,
    pub sequence_source: String,
    pub sender_balance: u64,
    pub sender_sequence: u64,
    pub mempool_pending_for_sender: u64,
    pub base_asset_fee: u64,
    pub state_expansion_fee: u64,
    pub minimum_fee: u64,
    pub account_reserve: u64,
    pub transfer_fee_byte_quantum: u64,
    pub transfer_fee_per_quantum: u64,
    pub asset_weight_bytes: u64,
    pub sender_balance_after_fee: Option<u64>,
    pub sender_meets_reserve_after_fee: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EscrowFeeQuoteSummary {
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub transaction_kind: String,
    pub source: String,
    pub sequence: u64,
    pub sequence_source: String,
    pub sender_balance: u64,
    pub sender_sequence: u64,
    pub mempool_pending_for_sender: u64,
    pub base_escrow_fee: u64,
    pub state_expansion_fee: u64,
    pub minimum_fee: u64,
    pub account_reserve: u64,
    pub transfer_fee_byte_quantum: u64,
    pub transfer_fee_per_quantum: u64,
    pub escrow_weight_bytes: u64,
    pub sender_balance_after_fee: Option<u64>,
    pub sender_meets_reserve_after_fee: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MempoolSubmitSummary {
    pub tx_id: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub from: String,
    pub to: String,
    pub amount: u64,
    pub fee: u64,
    pub sequence: u64,
    pub algorithm_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TxFinalitySummary {
    pub proof_id: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub tx_id: String,
    pub accepted: bool,
    pub receipt_code: String,
    pub receipt_message: String,
    pub receipt_index: u64,
    pub receipt_count: u64,
    pub block_height: u64,
    pub block_hash: String,
    pub state_root: String,
    pub certificate_id: String,
    pub registry_root: String,
    pub quorum: u64,
    pub validator_count: u64,
    pub vote_count: u64,
    pub block_count: u64,
    pub tip_hash: String,
    pub tip_state_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountSummary {
    pub address: String,
    pub balance: u64,
    pub sequence: u64,
    pub public_key_hex: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReceiptSummary {
    pub tx_id: String,
    pub accepted: bool,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalletBackupFile {
    pub schema: String,
    pub algorithm_id: String,
    pub kdf: String,
    pub derivation_domain: String,
    pub chain_id: String,
    pub account_index: u32,
    pub key_role: String,
    pub master_seed_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalletIdentity {
    pub algorithm_id: String,
    pub kdf: String,
    pub derivation_domain: String,
    pub chain_id: String,
    pub account_index: u32,
    pub key_role: String,
    pub address: String,
    pub public_key_hex: String,
    pub private_key_material_redacted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalletSignTransferFields {
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub to: String,
    pub amount: u64,
    pub fee: u64,
    pub sequence: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalletSignPaymentV2Fields {
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub to: String,
    pub amount: u64,
    pub fee: u64,
    pub sequence: u64,
    pub memos: Vec<PaymentMemo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalletSignAssetTransactionFields {
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub source: String,
    pub fee: u64,
    pub sequence: u64,
    pub operation: AssetTransactionOperation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalletSignEscrowTransactionFields {
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub source: String,
    pub fee: u64,
    pub sequence: u64,
    pub operation: EscrowTransactionOperation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalletSignNftTransactionFields {
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub source: String,
    pub fee: u64,
    pub sequence: u64,
    pub operation: NftTransactionOperation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalletSignOfferTransactionFields {
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub source: String,
    pub fee: u64,
    pub sequence: u64,
    pub operation: OfferTransactionOperation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalletOwnedTransferSignature {
    pub owner_pubkey_hex: String,
    pub owner_signature_hex: String,
    pub order: OwnedTransferOrder,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalletOwnedUnwrapSignature {
    pub owner_pubkey_hex: String,
    pub owner_signature_hex: String,
    pub order: OwnedUnwrapOrder,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchArchiveValidationContext {
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RpcProtocolError {
    UnsupportedVersion {
        found: String,
    },
    EmptyId,
    EmptyMethod,
    FieldTooLong {
        field: &'static str,
        max_bytes: usize,
    },
    ParamsNotObject,
    EmptyParamName,
    ParamNameTooLong {
        key: String,
        max_bytes: usize,
    },
    ParamStringTooLong {
        key: String,
        max_bytes: usize,
    },
    NestedParamObject(String),
    ResponseMissingResult,
    ResponseUnexpectedError,
    ResponseMissingError,
    ResponseUnexpectedResult,
    ResponseEmptyErrorCode,
    ResponseEmptyErrorMessage,
    ResponseErrorKeyMaterial {
        field: &'static str,
    },
    ResponseEmptyEventField {
        index: usize,
        field: &'static str,
    },
    ResponseEventKeyMaterial {
        index: usize,
        field: &'static str,
    },
}

impl fmt::Display for RpcProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedVersion { found } => {
                write!(
                    f,
                    "unsupported rpc version `{found}`, expected `{RPC_VERSION}`"
                )
            }
            Self::EmptyId => write!(f, "rpc id must not be empty"),
            Self::EmptyMethod => write!(f, "rpc method must not be empty"),
            Self::FieldTooLong { field, max_bytes } => {
                write!(f, "rpc field `{field}` must not exceed {max_bytes} bytes")
            }
            Self::ParamsNotObject => write!(f, "rpc request params must be an object"),
            Self::EmptyParamName => write!(f, "rpc request param names must not be empty"),
            Self::ParamNameTooLong { key, max_bytes } => {
                write!(
                    f,
                    "rpc request param name `{key}` must not exceed {max_bytes} bytes"
                )
            }
            Self::ParamStringTooLong { key, max_bytes } => {
                write!(
                    f,
                    "rpc request param `{key}` must not exceed {max_bytes} bytes"
                )
            }
            Self::NestedParamObject(key) => {
                write!(f, "rpc params cannot contain nested objects at `{key}`")
            }
            Self::ResponseMissingResult => write!(f, "successful rpc response missing result"),
            Self::ResponseUnexpectedError => {
                write!(f, "successful rpc response must not include error")
            }
            Self::ResponseMissingError => write!(f, "failed rpc response missing error"),
            Self::ResponseUnexpectedResult => {
                write!(f, "failed rpc response must not include result")
            }
            Self::ResponseEmptyErrorCode => write!(f, "rpc error code must not be empty"),
            Self::ResponseEmptyErrorMessage => write!(f, "rpc error message must not be empty"),
            Self::ResponseErrorKeyMaterial { field } => {
                write!(f, "rpc error field `{field}` contains key-material marker")
            }
            Self::ResponseEmptyEventField { index, field } => {
                write!(
                    f,
                    "rpc response event {index} field `{field}` must not be empty"
                )
            }
            Self::ResponseEventKeyMaterial { index, field } => {
                write!(
                    f,
                    "rpc response event {index} field `{field}` contains key-material marker"
                )
            }
        }
    }
}

impl std::error::Error for RpcProtocolError {}

fn validate_common_fields(version: &str, id: &str) -> Result<(), RpcProtocolError> {
    if version != RPC_VERSION {
        return Err(RpcProtocolError::UnsupportedVersion {
            found: version.to_string(),
        });
    }
    if id.trim().is_empty() {
        return Err(RpcProtocolError::EmptyId);
    }
    validate_protocol_string_len("id", id)?;
    Ok(())
}

fn validate_protocol_string_len(field: &'static str, value: &str) -> Result<(), RpcProtocolError> {
    if value.len() > MAX_RPC_PARAM_STRING_BYTES {
        return Err(RpcProtocolError::FieldTooLong {
            field,
            max_bytes: MAX_RPC_PARAM_STRING_BYTES,
        });
    }
    Ok(())
}

fn max_rpc_param_string_bytes(method: &str, key: &str) -> usize {
    if key == "signed_transfer_json"
        || key == "signed_payment_v2_json"
        || key == "signed_asset_transaction_json"
        || key == "signed_escrow_transaction_json"
        || key == "signed_nft_transaction_json"
        || key == "signed_atomic_swap_transaction_json"
        || key == "order_json"
        || key == "order_json_gzip_base64"
        || key == "cert_json"
        || key == "signed_intent_json"
        || key == "lock_qc_json"
        || key == "decision_qc_json"
        || key == "proposal_json"
        || key == "precommit_qc_json"
        || key == "signed_exit_json"
        || key == "fastlane_primary_json"
    {
        if key == "order_json" || key == "order_json_gzip_base64" || key == "cert_json" {
            MAX_RPC_FASTPAY_JSON_BYTES
        } else {
            MAX_RPC_SIGNED_TRANSFER_JSON_BYTES
        }
    } else if method == "shield_batch_finality" && key == "batch_json" {
        MAX_RPC_SHIELD_BATCH_JSON_BYTES
    } else if key == "action_json" {
        MAX_RPC_ORCHARD_ACTION_JSON_BYTES
    } else if key == "deposit_json" {
        MAX_RPC_ORCHARD_DEPOSIT_JSON_BYTES
    } else if key == "swap_json" {
        MAX_RPC_SHIELDED_SWAP_JSON_BYTES
    } else {
        MAX_RPC_PARAM_STRING_BYTES
    }
}

fn validate_response_events(events: &[RpcEvent]) -> Result<(), RpcProtocolError> {
    for (index, event) in events.iter().enumerate() {
        validate_response_event_field(index, "event_type", &event.event_type)?;
        validate_response_event_field(index, "subject", &event.subject)?;
        validate_response_event_field(index, "message", &event.message)?;
    }
    Ok(())
}

fn validate_response_error_field(field: &'static str, value: &str) -> Result<(), RpcProtocolError> {
    if contains_key_material_marker(value) {
        return Err(RpcProtocolError::ResponseErrorKeyMaterial { field });
    }
    Ok(())
}

fn validate_response_event_field(
    index: usize,
    field: &'static str,
    value: &str,
) -> Result<(), RpcProtocolError> {
    if value.trim().is_empty() {
        return Err(RpcProtocolError::ResponseEmptyEventField { index, field });
    }
    if contains_key_material_marker(value) {
        return Err(RpcProtocolError::ResponseEventKeyMaterial { index, field });
    }
    Ok(())
}

fn contains_key_material_marker(value: &str) -> bool {
    [
        "private_key_hex",
        "public_key_hex",
        "master_seed_hex",
        "spending_key_hex",
        "full_viewing_key_hex",
        "rseed",
    ]
    .iter()
    .any(|marker| value.contains(marker))
}

fn validate_param_value(method: &str, key: &str, value: &Value) -> Result<(), RpcProtocolError> {
    match value {
        Value::String(value) => {
            let max_bytes = max_rpc_param_string_bytes(method, key);
            if value.len() > max_bytes {
                return Err(RpcProtocolError::ParamStringTooLong {
                    key: key.to_string(),
                    max_bytes,
                });
            }
            Ok(())
        }
        Value::Array(values) => {
            for value in values {
                validate_param_value(method, key, value)?;
            }
            Ok(())
        }
        Value::Object(_) => Err(RpcProtocolError::NestedParamObject(key.to_string())),
        _ => Ok(()),
    }
}

fn request_kind_method(kind: RpcRequestKind) -> &'static str {
    match kind {
        RpcRequestKind::Status => METHOD_STATUS,
        RpcRequestKind::ServerInfo => METHOD_SERVER_INFO,
        RpcRequestKind::Metrics => METHOD_METRICS,
        RpcRequestKind::Ledger => METHOD_LEDGER,
        RpcRequestKind::VerifyState => METHOD_VERIFY_STATE,
        RpcRequestKind::ValidateLocalKeys { .. } => METHOD_VALIDATE_LOCAL_KEYS,
        RpcRequestKind::Account => METHOD_ACCOUNT,
        RpcRequestKind::AccountTx => METHOD_ACCOUNT_TX,
        RpcRequestKind::Fee => METHOD_FEE,
        RpcRequestKind::TransferFeeQuote => METHOD_TRANSFER_FEE_QUOTE,
        RpcRequestKind::AtomicSwapFeeQuote => METHOD_ATOMIC_SWAP_FEE_QUOTE,
        RpcRequestKind::AssetFeeQuote => METHOD_ASSET_FEE_QUOTE,
        RpcRequestKind::EscrowFeeQuote => METHOD_ESCROW_FEE_QUOTE,
        RpcRequestKind::NftFeeQuote => METHOD_NFT_FEE_QUOTE,
        RpcRequestKind::OfferFeeQuote => METHOD_OFFER_FEE_QUOTE,
        RpcRequestKind::AtomicSettlementTemplate => METHOD_ATOMIC_SETTLEMENT_TEMPLATE,
        RpcRequestKind::OfferInfo => METHOD_OFFER_INFO,
        RpcRequestKind::AccountOffers => METHOD_ACCOUNT_OFFERS,
        RpcRequestKind::BookOffers => METHOD_BOOK_OFFERS,
        RpcRequestKind::AssetInfo => METHOD_ASSET_INFO,
        RpcRequestKind::AccountLines => METHOD_ACCOUNT_LINES,
        RpcRequestKind::AccountAssets => METHOD_ACCOUNT_ASSETS,
        RpcRequestKind::IssuerAssets => METHOD_ISSUER_ASSETS,
        RpcRequestKind::EscrowInfo => METHOD_ESCROW_INFO,
        RpcRequestKind::AccountEscrows => METHOD_ACCOUNT_ESCROWS,
        RpcRequestKind::NftInfo => METHOD_NFT_INFO,
        RpcRequestKind::AccountNfts => METHOD_ACCOUNT_NFTS,
        RpcRequestKind::IssuerNfts => METHOD_ISSUER_NFTS,
        RpcRequestKind::Receipts => METHOD_RECEIPTS,
        RpcRequestKind::Tx => METHOD_TX,
        RpcRequestKind::Blocks => METHOD_BLOCKS,
        RpcRequestKind::Validators => METHOD_VALIDATORS,
        RpcRequestKind::Manifests => METHOD_MANIFESTS,
        RpcRequestKind::BatchArchive => METHOD_BATCH_ARCHIVE,
        RpcRequestKind::ArchiveWindow => METHOD_ARCHIVE_WINDOW,
        RpcRequestKind::MempoolSubmitTransfer => METHOD_MEMPOOL_SUBMIT_TRANSFER,
        RpcRequestKind::MempoolSubmitSignedTransfer => METHOD_MEMPOOL_SUBMIT_SIGNED_TRANSFER,
        RpcRequestKind::MempoolSubmitSignedPaymentV2 => METHOD_MEMPOOL_SUBMIT_SIGNED_PAYMENT_V2,
        RpcRequestKind::MempoolSubmitSignedAssetTransaction => {
            METHOD_MEMPOOL_SUBMIT_SIGNED_ASSET_TRANSACTION
        }
        RpcRequestKind::MempoolSubmitSignedEscrowTransaction => {
            METHOD_MEMPOOL_SUBMIT_SIGNED_ESCROW_TRANSACTION
        }
        RpcRequestKind::MempoolSubmitSignedNftTransaction => {
            METHOD_MEMPOOL_SUBMIT_SIGNED_NFT_TRANSACTION
        }
        RpcRequestKind::MempoolSubmitSignedOfferTransaction => {
            METHOD_MEMPOOL_SUBMIT_SIGNED_OFFER_TRANSACTION
        }
        RpcRequestKind::MempoolSubmitSignedAtomicSwapTransaction => {
            METHOD_MEMPOOL_SUBMIT_SIGNED_ATOMIC_SWAP_TRANSACTION
        }
        RpcRequestKind::MempoolSubmitSignedAtomicSwapTransactionFinality => {
            METHOD_MEMPOOL_SUBMIT_SIGNED_ATOMIC_SWAP_TRANSACTION_FINALITY
        }
        RpcRequestKind::MempoolStatus => METHOD_MEMPOOL_STATUS,
        RpcRequestKind::MempoolBatch => METHOD_MEMPOOL_BATCH,
        RpcRequestKind::ApplyBatch => METHOD_APPLY_BATCH,
        RpcRequestKind::ShieldBatchMint => METHOD_SHIELD_BATCH_MINT,
        RpcRequestKind::ShieldBatchSpend => METHOD_SHIELD_BATCH_SPEND,
        RpcRequestKind::ShieldBatchMigrate => METHOD_SHIELD_BATCH_MIGRATE,
        RpcRequestKind::ShieldBatchOrchard => METHOD_SHIELD_BATCH_ORCHARD,
        RpcRequestKind::ShieldBatchOrchardDeposit => METHOD_SHIELD_BATCH_ORCHARD_DEPOSIT,
        RpcRequestKind::ShieldBatchOrchardWithdraw => METHOD_SHIELD_BATCH_ORCHARD_WITHDRAW,
        RpcRequestKind::ShieldBatchSwap => METHOD_SHIELD_BATCH_SWAP,
        RpcRequestKind::ApplyShieldBatch => METHOD_APPLY_SHIELD_BATCH,
        RpcRequestKind::ShieldScan => METHOD_SHIELD_SCAN,
        RpcRequestKind::ShieldDisclose => METHOD_SHIELD_DISCLOSE,
        RpcRequestKind::ShieldTurnstile => METHOD_SHIELD_TURNSTILE,
        RpcRequestKind::BridgeStatus => METHOD_BRIDGE_STATUS,
        RpcRequestKind::NavcoinBridgeRoutes => METHOD_NAVCOIN_BRIDGE_ROUTES,
        RpcRequestKind::NavcoinBridgePacket => METHOD_NAVCOIN_BRIDGE_PACKET,
        RpcRequestKind::NavcoinBridgeClaims => METHOD_NAVCOIN_BRIDGE_CLAIMS,
        RpcRequestKind::NavcoinBridgeSupplyStatus => METHOD_NAVCOIN_BRIDGE_SUPPLY_STATUS,
        RpcRequestKind::NavcoinBridgeReceiptReplay => METHOD_NAVCOIN_BRIDGE_RECEIPT_REPLAY,
        RpcRequestKind::NavcoinBridgePacketPreflight => METHOD_NAVCOIN_BRIDGE_PACKET_PREFLIGHT,
        RpcRequestKind::BridgeBatchDomain => METHOD_BRIDGE_BATCH_DOMAIN,
        RpcRequestKind::BridgeBatchTransfer => METHOD_BRIDGE_BATCH_TRANSFER,
        RpcRequestKind::BridgeBatchPause => METHOD_BRIDGE_BATCH_PAUSE,
        RpcRequestKind::BridgeBatchResume => METHOD_BRIDGE_BATCH_RESUME,
        RpcRequestKind::ApplyBridgeBatch => METHOD_APPLY_BRIDGE_BATCH,
        RpcRequestKind::FastSwapCapabilities => METHOD_FASTSWAP_CAPABILITIES,
        RpcRequestKind::FastSwapPreview => METHOD_FASTSWAP_PREVIEW,
        RpcRequestKind::FastSwapPrepare => METHOD_FASTSWAP_PREPARE,
        RpcRequestKind::FastSwapCommit => METHOD_FASTSWAP_COMMIT,
        RpcRequestKind::FastSwapApply => METHOD_FASTSWAP_APPLY,
        RpcRequestKind::FastSwapCatchUp => METHOD_FASTSWAP_CATCH_UP,
        RpcRequestKind::FastSwapStatus => METHOD_FASTSWAP_STATUS,
        RpcRequestKind::FastSwapEffects => METHOD_FASTSWAP_EFFECTS,
        RpcRequestKind::FastSwapVotes => METHOD_FASTSWAP_VOTES,
        RpcRequestKind::FastSwapNewRoundVote => METHOD_FASTSWAP_NEW_ROUND_VOTE,
        RpcRequestKind::FastSwapProposeRound => METHOD_FASTSWAP_PROPOSE_ROUND,
        RpcRequestKind::FastSwapPrecommit => METHOD_FASTSWAP_PRECOMMIT,
        RpcRequestKind::FastSwapCommitRound => METHOD_FASTSWAP_COMMIT_ROUND,
        RpcRequestKind::FastSwapCancelApply => METHOD_FASTSWAP_CANCEL_APPLY,
        RpcRequestKind::FastLaneExit => METHOD_FASTLANE_EXIT,
        RpcRequestKind::FastSwapCheckpointStatus => METHOD_FASTSWAP_CHECKPOINT_STATUS,
        RpcRequestKind::FastSwapObjects => METHOD_FASTSWAP_OBJECTS,
        RpcRequestKind::FastSwapPolicy => METHOD_FASTSWAP_POLICY,
        RpcRequestKind::FastLaneAssetControlPrepare => METHOD_FASTLANE_ASSET_CONTROL_PREPARE,
        RpcRequestKind::FastLaneAssetControlPreview => METHOD_FASTLANE_ASSET_CONTROL_PREVIEW,
        RpcRequestKind::FastLaneAssetControlApply => METHOD_FASTLANE_ASSET_CONTROL_APPLY,
        RpcRequestKind::FastLaneAssetControlCatchUp => METHOD_FASTLANE_ASSET_CONTROL_CATCH_UP,
        RpcRequestKind::MempoolSubmitFastLanePrimary => METHOD_MEMPOOL_SUBMIT_FASTLANE_PRIMARY,
    }
}

fn validate_request_params(
    request: &RpcRequest,
    kind: RpcRequestKind,
) -> Result<(), RpcRequestValidationError> {
    match kind {
        RpcRequestKind::Status
        | RpcRequestKind::ServerInfo
        | RpcRequestKind::Metrics
        | RpcRequestKind::VerifyState
        | RpcRequestKind::Fee
        | RpcRequestKind::Validators
        | RpcRequestKind::Manifests => {
            if request.params.as_object().is_some_and(Map::is_empty) {
                Ok(())
            } else {
                Err(invalid_request_params(
                    "params",
                    "expected empty object params",
                ))
            }
        }
        RpcRequestKind::ValidateLocalKeys { validators } => {
            validate_local_key_request_params(&request.params, validators)
        }
        RpcRequestKind::Ledger => validate_ledger_request_params(&request.params),
        RpcRequestKind::Account => validate_account_request_params(&request.params),
        RpcRequestKind::AccountTx => validate_account_tx_request_params(&request.params),
        RpcRequestKind::TransferFeeQuote => {
            validate_transfer_fee_quote_request_params(&request.params)
        }
        RpcRequestKind::AtomicSwapFeeQuote => {
            validate_atomic_swap_fee_quote_request_params(&request.params)
        }
        RpcRequestKind::AssetFeeQuote => validate_asset_fee_quote_request_params(&request.params),
        RpcRequestKind::EscrowFeeQuote => validate_escrow_fee_quote_request_params(&request.params),
        RpcRequestKind::NftFeeQuote => validate_nft_fee_quote_request_params(&request.params),
        RpcRequestKind::OfferFeeQuote => validate_offer_fee_quote_request_params(&request.params),
        RpcRequestKind::AtomicSettlementTemplate => {
            validate_atomic_settlement_template_request_params(&request.params)
        }
        RpcRequestKind::OfferInfo => validate_offer_info_request_params(&request.params),
        RpcRequestKind::AccountOffers => validate_account_offers_request_params(&request.params),
        RpcRequestKind::BookOffers => validate_book_offers_request_params(&request.params),
        RpcRequestKind::AssetInfo => validate_asset_info_request_params(&request.params),
        RpcRequestKind::AccountLines => validate_account_lines_request_params(&request.params),
        RpcRequestKind::AccountAssets => validate_account_assets_request_params(&request.params),
        RpcRequestKind::IssuerAssets => validate_issuer_assets_request_params(&request.params),
        RpcRequestKind::EscrowInfo => validate_escrow_info_request_params(&request.params),
        RpcRequestKind::AccountEscrows => validate_account_escrows_request_params(&request.params),
        RpcRequestKind::NftInfo => validate_nft_info_request_params(&request.params),
        RpcRequestKind::AccountNfts => validate_account_nfts_request_params(&request.params),
        RpcRequestKind::IssuerNfts => validate_issuer_nfts_request_params(&request.params),
        RpcRequestKind::Receipts => validate_receipts_request_params(&request.params),
        RpcRequestKind::Tx => validate_tx_request_params(&request.params),
        RpcRequestKind::Blocks => validate_blocks_request_params(&request.params),
        RpcRequestKind::BatchArchive => validate_batch_archive_request_params(&request.params),
        RpcRequestKind::ArchiveWindow => validate_archive_window_request_params(&request.params),
        RpcRequestKind::MempoolSubmitTransfer => {
            validate_mempool_submit_transfer_request_params(&request.params)
        }
        RpcRequestKind::MempoolSubmitSignedTransfer => {
            validate_mempool_submit_signed_transfer_request_params(&request.params)
        }
        RpcRequestKind::MempoolSubmitSignedPaymentV2 => {
            validate_mempool_submit_signed_payment_v2_request_params(&request.params)
        }
        RpcRequestKind::MempoolSubmitSignedAssetTransaction => {
            validate_mempool_submit_signed_asset_transaction_request_params(&request.params)
        }
        RpcRequestKind::MempoolSubmitSignedEscrowTransaction => {
            validate_mempool_submit_signed_escrow_transaction_request_params(&request.params)
        }
        RpcRequestKind::MempoolSubmitSignedNftTransaction => {
            validate_mempool_submit_signed_nft_transaction_request_params(&request.params)
        }
        RpcRequestKind::MempoolSubmitSignedOfferTransaction => {
            validate_mempool_submit_signed_offer_transaction_request_params(&request.params)
        }
        RpcRequestKind::MempoolSubmitSignedAtomicSwapTransaction => {
            validate_signed_atomic_swap_request_params(&request.params)
        }
        RpcRequestKind::MempoolSubmitSignedAtomicSwapTransactionFinality => {
            validate_signed_atomic_swap_finality_request_params(&request.params)
        }
        RpcRequestKind::MempoolStatus => {
            if request.params.as_object().is_some_and(Map::is_empty) {
                Ok(())
            } else {
                Err(invalid_request_params(
                    "params",
                    "expected empty object params",
                ))
            }
        }
        RpcRequestKind::MempoolBatch => validate_mempool_batch_request_params(&request.params),
        RpcRequestKind::ApplyBatch => validate_apply_batch_request_params(&request.params),
        RpcRequestKind::ShieldBatchMint => {
            validate_shield_batch_mint_request_params(&request.params)
        }
        RpcRequestKind::ShieldBatchSpend => {
            validate_shield_batch_spend_request_params(&request.params)
        }
        RpcRequestKind::ShieldBatchMigrate => {
            validate_shield_batch_migrate_request_params(&request.params)
        }
        RpcRequestKind::ShieldBatchOrchard => {
            validate_shield_batch_orchard_request_params(&request.params)
        }
        RpcRequestKind::ShieldBatchOrchardDeposit => {
            validate_shield_batch_orchard_deposit_request_params(&request.params)
        }
        RpcRequestKind::ShieldBatchOrchardWithdraw => {
            validate_shield_batch_orchard_withdraw_request_params(&request.params)
        }
        RpcRequestKind::ShieldBatchSwap => {
            validate_shield_batch_swap_request_params(&request.params)
        }
        RpcRequestKind::ApplyShieldBatch => validate_apply_batch_request_params(&request.params),
        RpcRequestKind::ShieldScan => validate_shield_scan_request_params(&request.params),
        RpcRequestKind::ShieldDisclose => validate_shield_disclose_request_params(&request.params),
        RpcRequestKind::ShieldTurnstile => {
            if request.params.as_object().is_some_and(Map::is_empty) {
                Ok(())
            } else {
                Err(invalid_request_params(
                    "params",
                    "expected empty object params",
                ))
            }
        }
        RpcRequestKind::BridgeStatus => {
            if request.params.as_object().is_some_and(Map::is_empty) {
                Ok(())
            } else {
                Err(invalid_request_params(
                    "params",
                    "expected empty object params",
                ))
            }
        }
        RpcRequestKind::NavcoinBridgeRoutes => validate_navcoin_bridge_routes_request_params(&request.params),
        RpcRequestKind::NavcoinBridgePacket => validate_navcoin_bridge_packet_request_params(&request.params),
        RpcRequestKind::NavcoinBridgeClaims => validate_navcoin_bridge_claims_request_params(&request.params),
        RpcRequestKind::NavcoinBridgeSupplyStatus => {
            validate_navcoin_bridge_supply_status_request_params(&request.params)
        }
        RpcRequestKind::NavcoinBridgeReceiptReplay => {
            validate_navcoin_bridge_receipt_replay_request_params(&request.params)
        }
        RpcRequestKind::NavcoinBridgePacketPreflight => {
            validate_navcoin_bridge_packet_preflight_request_params(&request.params)
        }
        RpcRequestKind::BridgeBatchDomain => {
            validate_bridge_batch_domain_request_params(&request.params)
        }
        RpcRequestKind::BridgeBatchTransfer => {
            validate_bridge_batch_transfer_request_params(&request.params)
        }
        RpcRequestKind::BridgeBatchPause | RpcRequestKind::BridgeBatchResume => {
            validate_bridge_batch_pause_request_params(&request.params)
        }
        RpcRequestKind::ApplyBridgeBatch => validate_apply_batch_request_params(&request.params),
        RpcRequestKind::FastSwapCapabilities => {
            if request.params.as_object().is_some_and(Map::is_empty) {
                Ok(())
            } else {
                Err(invalid_request_params("params", "expected empty object params"))
            }
        }
        RpcRequestKind::FastSwapPreview => {
            validate_fastswap_string_params(&request.params, &["signed_intent_json"])
        }
        RpcRequestKind::FastSwapPrepare => {
            validate_fastswap_string_params(&request.params, &["signed_intent_json"])
        }
        RpcRequestKind::FastSwapCommit => {
            validate_fastswap_string_params(&request.params, &["lock_qc_json"])
        }
        RpcRequestKind::FastSwapApply => validate_fastswap_string_params(
            &request.params,
            &["decision_qc_json", "signed_intent_json"],
        ),
        RpcRequestKind::FastSwapCatchUp => validate_fastswap_string_params(
            &request.params,
            &["lock_qc_json", "decision_qc_json", "signed_intent_json"],
        ),
        RpcRequestKind::FastSwapStatus | RpcRequestKind::FastSwapEffects => {
            validate_fastswap_string_params(&request.params, &["swap_id"])
        }
        RpcRequestKind::FastSwapVotes => {
            let params = request_params(&request.params)?;
            require_only_params(params, &["swap_id", "phase", "round"])?;
            string_param(params, "swap_id")?;
            let phase = string_param(params, "phase")?;
            if parse_fastswap_phase(phase).is_none() {
                return Err(invalid_request_params("phase", "invalid FastSwap phase"));
            }
            params
                .get("round")
                .and_then(Value::as_u64)
                .ok_or_else(|| invalid_request_params("round", "expected u64"))?;
            Ok(())
        }
        RpcRequestKind::FastSwapNewRoundVote => {
            let params = request_params(&request.params)?;
            require_only_params(params, &["swap_id", "target_round"])?;
            string_param(params, "swap_id")?;
            nonzero_u64_param(params, "target_round")?;
            Ok(())
        }
        RpcRequestKind::FastSwapProposeRound | RpcRequestKind::FastSwapPrecommit => {
            validate_fastswap_string_params(&request.params, &["proposal_json"])
        }
        RpcRequestKind::FastSwapCommitRound => {
            validate_fastswap_string_params(&request.params, &["precommit_qc_json"])
        }
        RpcRequestKind::FastSwapCancelApply => {
            validate_fastswap_string_params(&request.params, &["decision_qc_json"])
        }
        RpcRequestKind::FastLaneExit => {
            validate_fastswap_string_params(&request.params, &["signed_exit_json"])
        }
        RpcRequestKind::FastSwapCheckpointStatus => {
            let params = request_params(&request.params)?;
            require_only_params(params, &["previous_checkpoint_id"])?;
            if params.contains_key("previous_checkpoint_id") {
                string_param(params, "previous_checkpoint_id")?;
            }
            Ok(())
        }
        RpcRequestKind::FastSwapObjects => {
            let params = request_params(&request.params)?;
            require_only_params(params, &["owner_pubkey", "asset_id", "cursor_object_id", "cursor_version", "limit"])?;
            string_param(params, "owner_pubkey")?;
            if params.contains_key("asset_id") {
                string_param(params, "asset_id")?;
            }
            let cursor_id = params.contains_key("cursor_object_id");
            let cursor_version = params.contains_key("cursor_version");
            if cursor_id != cursor_version {
                return Err(invalid_request_params("params", "cursor object id/version must be supplied together"));
            }
            if cursor_id {
                string_param(params, "cursor_object_id")?;
                nonzero_u64_param(params, "cursor_version")?;
            }
            nonzero_u64_param(params, "limit")?;
            Ok(())
        }
        RpcRequestKind::FastSwapPolicy => {
            let params = request_params(&request.params)?;
            require_only_params(params, &["policy_hash", "asset_0", "asset_1"])?;
            let by_hash = params.contains_key("policy_hash");
            let by_pair = params.contains_key("asset_0") && params.contains_key("asset_1");
            if by_hash == by_pair || (params.contains_key("asset_0") != params.contains_key("asset_1")) {
                return Err(invalid_request_params("params", "requires exactly policy_hash or asset_0+asset_1"));
            }
            if by_hash {
                string_param(params, "policy_hash")?;
            } else {
                string_param(params, "asset_0")?;
                string_param(params, "asset_1")?;
            }
            Ok(())
        }
        RpcRequestKind::FastLaneAssetControlPrepare => {
            validate_fastswap_string_params(&request.params, &["signed_command_json"])
        }
        RpcRequestKind::FastLaneAssetControlPreview => {
            validate_fastswap_string_params(&request.params, &["signed_command_json"])
        }
        RpcRequestKind::FastLaneAssetControlApply => validate_fastswap_string_params(
            &request.params,
            &["decision_qc_json", "signed_command_json"],
        ),
        RpcRequestKind::FastLaneAssetControlCatchUp => validate_fastswap_string_params(
            &request.params,
            &["lock_qc_json", "decision_qc_json", "signed_command_json"],
        ),
        RpcRequestKind::MempoolSubmitFastLanePrimary => {
            validate_fastswap_string_params(&request.params, &["fastlane_primary_json"])
        }
    }
}

fn validate_fastswap_string_params(
    params: &Value,
    required: &[&str],
) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    require_only_params(params, required)?;
    for key in required {
        string_param(params, key)?;
    }
    Ok(())
}

fn request_params(params: &Value) -> Result<&Map<String, Value>, RpcRequestValidationError> {
    params
        .as_object()
        .ok_or_else(|| invalid_request_params("params", "expected object params"))
}

fn validate_local_key_request_params(
    params: &Value,
    validators: Option<u32>,
) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    if params.len() != 1 {
        return Err(invalid_request_params(
            "params",
            "expected exactly one `validators` param",
        ));
    }
    let found = params
        .get("validators")
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid_request_params("validators", "expected unsigned integer value"))?;
    if found > u64::from(u32::MAX) {
        return Err(invalid_request_params(
            "validators",
            "expected value to fit in u32",
        ));
    }
    if let Some(expected) = validators {
        if found != u64::from(expected) {
            return Err(invalid_request_params(
                "validators",
                format!("expected {expected}, found {found}"),
            ));
        }
    }
    Ok(())
}

fn validate_account_request_params(params: &Value) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    require_only_params(params, &["address"])?;
    string_param(params, "address")?;
    Ok(())
}

fn validate_account_tx_request_params(params: &Value) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    require_only_params(params, &["address", "from_height", "to_height", "limit"])?;
    string_param(params, "address")?;
    optional_u64_param(params, "from_height")?;
    optional_u64_param(params, "to_height")?;
    if let (Some(from_height), Some(to_height)) = (
        params.get("from_height").and_then(Value::as_u64),
        params.get("to_height").and_then(Value::as_u64),
    ) {
        if from_height > to_height {
            return Err(invalid_request_params(
                "to_height",
                "expected to_height to be greater than or equal to from_height",
            ));
        }
    }
    optional_bounded_nonzero_usize_param(params, "limit", MAX_RPC_READ_QUERY_LIMIT)?;
    Ok(())
}

fn validate_ledger_request_params(params: &Value) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    require_only_params(params, &["limit"])?;
    optional_bounded_nonzero_usize_param(params, "limit", MAX_RPC_READ_QUERY_LIMIT)?;
    Ok(())
}

fn validate_transfer_fee_quote_request_params(
    params: &Value,
) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    require_only_params(
        params,
        &[
            "from",
            "to",
            "amount",
            "sequence",
            "memo_type",
            "memo_format",
            "memo_data",
        ],
    )?;
    string_param(params, "from")?;
    string_param(params, "to")?;
    nonzero_u64_param(params, "amount")?;
    optional_nonzero_u64_param(params, "sequence")?;
    optional_string_param(params, "memo_type")?;
    optional_string_param(params, "memo_format")?;
    optional_string_param(params, "memo_data")?;
    Ok(())
}

fn validate_asset_fee_quote_request_params(
    params: &Value,
) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    require_only_params(params, &["source", "operation_json", "sequence"])?;
    string_param(params, "source")?;
    string_param(params, "operation_json")?;
    optional_nonzero_u64_param(params, "sequence")?;
    Ok(())
}

fn validate_escrow_fee_quote_request_params(
    params: &Value,
) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    require_only_params(params, &["source", "operation_json", "sequence"])?;
    string_param(params, "source")?;
    string_param(params, "operation_json")?;
    optional_nonzero_u64_param(params, "sequence")?;
    Ok(())
}

fn validate_nft_fee_quote_request_params(params: &Value) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    require_only_params(params, &["source", "operation_json", "sequence"])?;
    string_param(params, "source")?;
    string_param(params, "operation_json")?;
    optional_nonzero_u64_param(params, "sequence")?;
    Ok(())
}

fn validate_offer_fee_quote_request_params(
    params: &Value,
) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    require_only_params(params, &["source", "operation_json", "sequence"])?;
    string_param(params, "source")?;
    string_param(params, "operation_json")?;
    optional_nonzero_u64_param(params, "sequence")?;
    Ok(())
}

fn validate_atomic_settlement_template_request_params(
    params: &Value,
) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    require_only_params(
        params,
        &[
            "left_owner",
            "left_recipient",
            "left_asset_id",
            "left_amount",
            "right_owner",
            "right_recipient",
            "right_asset_id",
            "right_amount",
            "condition",
            "finish_after",
            "cancel_after",
            "left_sequence",
            "right_sequence",
        ],
    )?;
    string_param(params, "left_owner")?;
    string_param(params, "left_recipient")?;
    string_param(params, "left_asset_id")?;
    nonzero_u64_param(params, "left_amount")?;
    string_param(params, "right_owner")?;
    string_param(params, "right_recipient")?;
    string_param(params, "right_asset_id")?;
    nonzero_u64_param(params, "right_amount")?;
    string_param(params, "condition")?;
    u64_param(params, "finish_after")?;
    nonzero_u64_param(params, "cancel_after")?;
    optional_nonzero_u64_param(params, "left_sequence")?;
    optional_nonzero_u64_param(params, "right_sequence")?;
    Ok(())
}

fn validate_offer_info_request_params(params: &Value) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    require_only_params(params, &["offer_id"])?;
    lower_hex_param(params, "offer_id", OFFER_ID_HEX_LEN)?;
    Ok(())
}

fn validate_account_offers_request_params(params: &Value) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    require_only_params(params, &["account", "state", "limit"])?;
    string_param(params, "account")?;
    if let Some(state) = params.get("state") {
        let state = state
            .as_str()
            .ok_or_else(|| invalid_request_params("state", "expected string value"))?;
        if !is_supported_offer_state(state) {
            return Err(invalid_request_params(
                "state",
                "expected open, filled, canceled, or unfunded",
            ));
        }
    }
    optional_bounded_nonzero_usize_param(params, "limit", MAX_RPC_READ_QUERY_LIMIT)?;
    Ok(())
}

fn validate_book_offers_request_params(params: &Value) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    require_only_params(
        params,
        &["taker_gets_asset_id", "taker_pays_asset_id", "limit"],
    )?;
    dex_asset_id_param(params, "taker_gets_asset_id")?;
    dex_asset_id_param(params, "taker_pays_asset_id")?;
    let taker_gets_asset_id = string_param(params, "taker_gets_asset_id")?;
    let taker_pays_asset_id = string_param(params, "taker_pays_asset_id")?;
    if taker_gets_asset_id == taker_pays_asset_id {
        return Err(invalid_request_params(
            "taker_pays_asset_id",
            "expected distinct DEX asset ids",
        ));
    }
    optional_bounded_nonzero_usize_param(params, "limit", MAX_RPC_READ_QUERY_LIMIT)?;
    Ok(())
}

fn validate_asset_info_request_params(params: &Value) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    require_only_params(params, &["asset_id"])?;
    lower_hex_param(params, "asset_id", ISSUED_ASSET_ID_HEX_LEN)?;
    Ok(())
}

fn validate_account_lines_request_params(params: &Value) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    require_only_params(params, &["account", "issuer", "asset_id", "limit"])?;
    string_param(params, "account")?;
    optional_string_param(params, "issuer")?;
    if params.contains_key("asset_id") {
        lower_hex_param(params, "asset_id", ISSUED_ASSET_ID_HEX_LEN)?;
    }
    optional_bounded_nonzero_usize_param(params, "limit", MAX_RPC_READ_QUERY_LIMIT)?;
    Ok(())
}

fn validate_account_assets_request_params(params: &Value) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    require_only_params(params, &["account", "asset_id", "limit"])?;
    string_param(params, "account")?;
    if params.contains_key("asset_id") {
        lower_hex_param(params, "asset_id", ISSUED_ASSET_ID_HEX_LEN)?;
    }
    optional_bounded_nonzero_usize_param(params, "limit", MAX_RPC_READ_QUERY_LIMIT)?;
    Ok(())
}

fn validate_issuer_assets_request_params(params: &Value) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    require_only_params(params, &["issuer", "limit"])?;
    string_param(params, "issuer")?;
    optional_bounded_nonzero_usize_param(params, "limit", MAX_RPC_READ_QUERY_LIMIT)?;
    Ok(())
}

fn validate_escrow_info_request_params(params: &Value) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    require_only_params(params, &["escrow_id"])?;
    lower_hex_param(params, "escrow_id", ESCROW_ID_HEX_LEN)?;
    Ok(())
}

fn validate_account_escrows_request_params(
    params: &Value,
) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    require_only_params(params, &["account", "role", "state", "limit"])?;
    string_param(params, "account")?;
    if let Some(role) = params.get("role") {
        let role = role
            .as_str()
            .ok_or_else(|| invalid_request_params("role", "expected string value"))?;
        if !matches!(role, "owner" | "recipient") {
            return Err(invalid_request_params(
                "role",
                "expected owner or recipient",
            ));
        }
    }
    if let Some(state) = params.get("state") {
        let state = state
            .as_str()
            .ok_or_else(|| invalid_request_params("state", "expected string value"))?;
        if !matches!(
            state,
            ESCROW_STATE_OPEN | ESCROW_STATE_FINISHED | ESCROW_STATE_CANCELED
        ) {
            return Err(invalid_request_params(
                "state",
                "expected open, finished, or canceled",
            ));
        }
    }
    optional_bounded_nonzero_usize_param(params, "limit", MAX_RPC_READ_QUERY_LIMIT)?;
    Ok(())
}

fn validate_nft_info_request_params(params: &Value) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    require_only_params(params, &["nft_id"])?;
    lower_hex_param(params, "nft_id", NFT_ID_HEX_LEN)?;
    Ok(())
}

fn validate_account_nfts_request_params(params: &Value) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    require_only_params(params, &["account", "include_burned", "limit"])?;
    string_param(params, "account")?;
    optional_bool_param(params, "include_burned")?;
    optional_bounded_nonzero_usize_param(params, "limit", MAX_RPC_READ_QUERY_LIMIT)?;
    Ok(())
}

fn validate_issuer_nfts_request_params(params: &Value) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    require_only_params(
        params,
        &["issuer", "collection_id", "include_burned", "limit"],
    )?;
    string_param(params, "issuer")?;
    optional_string_param(params, "collection_id")?;
    optional_bool_param(params, "include_burned")?;
    optional_bounded_nonzero_usize_param(params, "limit", MAX_RPC_READ_QUERY_LIMIT)?;
    Ok(())
}

fn validate_receipts_request_params(params: &Value) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    require_only_params(params, &["tx_id", "limit"])?;
    optional_string_param(params, "tx_id")?;
    optional_bounded_nonzero_usize_param(params, "limit", MAX_RPC_READ_QUERY_LIMIT)?;
    Ok(())
}

fn validate_tx_request_params(params: &Value) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    require_only_params(params, &["tx_id", "audit_block_log"])?;
    lower_hex_param(params, "tx_id", 96)?;
    optional_bool_param(params, "audit_block_log")?;
    Ok(())
}

fn validate_blocks_request_params(params: &Value) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    require_only_params(params, &["from_height", "limit"])?;
    optional_u64_param(params, "from_height")?;
    optional_bounded_nonzero_usize_param(params, "limit", MAX_RPC_READ_QUERY_LIMIT)?;
    Ok(())
}

fn validate_batch_archive_request_params(params: &Value) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    require_only_params(params, &["batch_kind", "batch_id", "limit"])?;
    optional_batch_kind_param(params, "batch_kind")?;
    optional_lower_hex_param(params, "batch_id", 96)?;
    optional_bounded_nonzero_usize_param(params, "limit", MAX_RPC_READ_QUERY_LIMIT)?;
    Ok(())
}

fn validate_archive_window_request_params(params: &Value) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    require_only_params(params, &["from_height", "to_height", "archive_uri"])?;
    let from_height = nonzero_u64_param(params, "from_height")?;
    let to_height = nonzero_u64_param(params, "to_height")?;
    if to_height < from_height {
        return Err(invalid_request_params(
            "to_height",
            "expected to_height to be greater than or equal to from_height",
        ));
    }
    let window_len = to_height
        .checked_sub(from_height)
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| invalid_request_params("to_height", "archive window overflow"))?;
    if window_len > MAX_RPC_READ_QUERY_LIMIT as u64 {
        return Err(invalid_request_params(
            "to_height",
            format!("archive window must not exceed {MAX_RPC_READ_QUERY_LIMIT} blocks"),
        ));
    }
    optional_string_param(params, "archive_uri")?;
    Ok(())
}

fn validate_mempool_submit_transfer_request_params(
    params: &Value,
) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    require_only_params(params, &["to", "amount", "key_file"])?;
    string_param(params, "to")?;
    nonzero_u64_param(params, "amount")?;
    optional_string_param(params, "key_file")?;
    Ok(())
}

fn validate_mempool_submit_signed_transfer_request_params(
    params: &Value,
) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    require_only_params(params, &["transfer_file", "signed_transfer_json"])?;
    let has_transfer_file = params.contains_key("transfer_file");
    let has_signed_transfer_json = params.contains_key("signed_transfer_json");
    match (has_transfer_file, has_signed_transfer_json) {
        (true, false) => {
            string_param(params, "transfer_file")?;
        }
        (false, true) => {
            string_param(params, "signed_transfer_json")?;
        }
        (false, false) => {
            return Err(invalid_request_params(
                "transfer_file",
                "expected `transfer_file` or `signed_transfer_json`",
            ));
        }
        (true, true) => {
            return Err(invalid_request_params(
                "params",
                "expected only one of `transfer_file` or `signed_transfer_json`",
            ));
        }
    }
    Ok(())
}

fn validate_mempool_submit_signed_payment_v2_request_params(
    params: &Value,
) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    require_only_params(params, &["signed_payment_v2_json"])?;
    string_param(params, "signed_payment_v2_json")?;
    Ok(())
}

fn validate_mempool_submit_signed_asset_transaction_request_params(
    params: &Value,
) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    require_only_params(params, &["signed_asset_transaction_json"])?;
    string_param(params, "signed_asset_transaction_json")?;
    Ok(())
}

fn validate_mempool_submit_signed_escrow_transaction_request_params(
    params: &Value,
) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    require_only_params(params, &["signed_escrow_transaction_json"])?;
    string_param(params, "signed_escrow_transaction_json")?;
    Ok(())
}

fn validate_mempool_submit_signed_nft_transaction_request_params(
    params: &Value,
) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    require_only_params(params, &["signed_nft_transaction_json"])?;
    string_param(params, "signed_nft_transaction_json")?;
    Ok(())
}

fn validate_mempool_submit_signed_offer_transaction_request_params(
    params: &Value,
) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    require_only_params(params, &["signed_offer_transaction_json"])?;
    string_param(params, "signed_offer_transaction_json")?;
    Ok(())
}

fn validate_mempool_batch_request_params(params: &Value) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    require_only_params(params, &["batch_file", "max_transactions"])?;
    string_param(params, "batch_file")?;
    optional_nonzero_usize_param(params, "max_transactions")?;
    Ok(())
}

fn validate_apply_batch_request_params(params: &Value) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    require_only_params(params, &["batch_file"])?;
    string_param(params, "batch_file")?;
    Ok(())
}

fn validate_shield_batch_mint_request_params(
    params: &Value,
) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    require_only_params(
        params,
        &["owner", "amount", "asset_id", "memo", "batch_file"],
    )?;
    string_param(params, "owner")?;
    nonzero_u64_param(params, "amount")?;
    optional_string_param(params, "asset_id")?;
    optional_string_param(params, "memo")?;
    string_param(params, "batch_file")?;
    Ok(())
}

fn validate_shield_batch_spend_request_params(
    params: &Value,
) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    require_only_params(params, &["note_id", "to", "amount", "memo", "batch_file"])?;
    string_param(params, "note_id")?;
    string_param(params, "to")?;
    nonzero_u64_param(params, "amount")?;
    optional_string_param(params, "memo")?;
    string_param(params, "batch_file")?;
    Ok(())
}

fn validate_shield_batch_migrate_request_params(
    params: &Value,
) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    require_only_params(params, &["note_id", "target_pool", "memo", "batch_file"])?;
    string_param(params, "note_id")?;
    string_param(params, "target_pool")?;
    optional_string_param(params, "memo")?;
    string_param(params, "batch_file")?;
    Ok(())
}

fn validate_shield_batch_orchard_request_params(
    params: &Value,
) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    require_only_params(params, &["action_file", "action_json", "batch_file"])?;
    validate_orchard_action_source_params(params, false)?;
    Ok(())
}

fn validate_shield_batch_orchard_deposit_request_params(
    params: &Value,
) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    require_only_params(params, &["deposit_file", "deposit_json", "batch_file"])?;
    let file = params.contains_key("deposit_file");
    let json = params.contains_key("deposit_json");
    match (file, json) {
        (true, true) => {
            return Err(invalid_request_params(
                "params",
                "provide only one of deposit_file or deposit_json",
            ))
        }
        (false, false) => {
            return Err(invalid_request_params(
                "params",
                "missing deposit_file or deposit_json",
            ))
        }
        _ => {}
    }
    if file {
        string_param(params, "deposit_file")?;
    }
    if let Some(deposit_json) = params.get("deposit_json").and_then(Value::as_str) {
        if deposit_json.trim().is_empty() {
            return Err(invalid_request_params(
                "deposit_json",
                "expected nonempty JSON string",
            ));
        }
        if deposit_json.len() > MAX_RPC_ORCHARD_DEPOSIT_JSON_BYTES {
            return Err(invalid_request_params(
                "deposit_json",
                format!("expected at most {MAX_RPC_ORCHARD_DEPOSIT_JSON_BYTES} bytes"),
            ));
        }
    }
    if params.contains_key("batch_file") {
        string_param(params, "batch_file")?;
    }
    Ok(())
}

fn validate_shield_batch_orchard_withdraw_request_params(
    params: &Value,
) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    require_only_params(
        params,
        &[
            "action_file",
            "action_json",
            "to",
            "amount",
            "fee",
            "policy_id",
            "disclosure_hash",
            "batch_file",
        ],
    )?;
    validate_orchard_action_source_params(params, false)?;
    string_param(params, "to")?;
    nonzero_u64_param(params, "amount")?;
    u64_param(params, "fee")?;
    if let Some(action_json) = params.get("action_json").and_then(Value::as_str) {
        let parsed = validate_orchard_action_json_request_param(action_json)?;
        let action_fee = parsed
            .as_object()
            .and_then(|object| object.get("fee"))
            .and_then(Value::as_u64)
            .ok_or_else(|| invalid_request_params("action_json.fee", "expected u64 fee"))?;
        let fee = params
            .get("fee")
            .and_then(Value::as_u64)
            .ok_or_else(|| invalid_request_params("fee", "expected u64 value"))?;
        if action_fee != fee {
            return Err(invalid_request_params(
                "action_json.fee",
                format!("expected withdraw payload fee {fee}, found {action_fee}"),
            ));
        }
    }
    if params.contains_key("policy_id") {
        let policy_id = string_param(params, "policy_id")?;
        if policy_id != ORCHARD_WITHDRAW_POLICY_ID {
            return Err(invalid_request_params(
                "policy_id",
                format!("unsupported Orchard withdraw policy `{policy_id}`"),
            ));
        }
    }
    optional_lower_hex_param(params, "disclosure_hash", 96)?;
    if params.contains_key("batch_file") {
        string_param(params, "batch_file")?;
    }
    Ok(())
}

fn validate_shield_batch_swap_request_params(
    params: &Value,
) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    require_only_params(params, &["swap_file", "swap_json", "batch_file"])?;
    let has_swap_file = params.contains_key("swap_file");
    let has_swap_json = params.contains_key("swap_json");
    match (has_swap_file, has_swap_json) {
        (true, false) => {
            string_param(params, "swap_file")?;
            string_param(params, "batch_file")?;
        }
        (false, true) => {
            validate_shielded_swap_json_request_param(string_param(params, "swap_json")?)?;
            if params.contains_key("batch_file") {
                string_param(params, "batch_file")?;
            }
        }
        (false, false) => {
            return Err(invalid_request_params(
                "swap_file",
                "expected `swap_file` or `swap_json`",
            ));
        }
        (true, true) => {
            return Err(invalid_request_params(
                "params",
                "expected only one of `swap_file` or `swap_json`",
            ));
        }
    }
    Ok(())
}

fn validate_orchard_action_source_params(
    params: &Map<String, Value>,
    require_server_spool: bool,
) -> Result<(), RpcRequestValidationError> {
    let has_action_file = params.contains_key("action_file");
    let has_action_json = params.contains_key("action_json");
    match (has_action_file, has_action_json) {
        (true, false) => {
            if require_server_spool {
                return Err(invalid_request_params(
                    "action_file",
                    "remote Orchard batch creation requires action_json",
                ));
            }
            string_param(params, "action_file")?;
            string_param(params, "batch_file")?;
        }
        (false, true) => {
            validate_orchard_action_json_request_param(string_param(params, "action_json")?)?;
            if params.contains_key("batch_file") && require_server_spool {
                return Err(invalid_request_params(
                    "batch_file",
                    "remote Orchard batch creation uses server-controlled batch spool paths",
                ));
            }
            if params.contains_key("batch_file") {
                string_param(params, "batch_file")?;
            }
        }
        (false, false) => {
            return Err(invalid_request_params(
                "action_file",
                "expected `action_file` or `action_json`",
            ));
        }
        (true, true) => {
            return Err(invalid_request_params(
                "params",
                "expected only one of `action_file` or `action_json`",
            ));
        }
    }
    Ok(())
}

fn validate_orchard_action_json_request_param(
    action_json: &str,
) -> Result<Value, RpcRequestValidationError> {
    if action_json.len() > MAX_RPC_ORCHARD_ACTION_JSON_BYTES {
        return Err(invalid_request_params(
            "action_json",
            format!("expected at most {MAX_RPC_ORCHARD_ACTION_JSON_BYTES} bytes"),
        ));
    }
    let parsed = serde_json::from_str::<Value>(action_json)
        .map_err(|error| invalid_request_params("action_json", format!("{error}")))?;
    if !parsed.is_object() {
        return Err(invalid_request_params(
            "action_json",
            "expected Orchard action JSON object",
        ));
    }
    if contains_private_key_material_field(&parsed) {
        return Err(invalid_request_params(
            "action_json",
            "Orchard action payload contains private key material fields",
        ));
    }
    for field in [
        "pool_id",
        "proof_system_id",
        "circuit_id",
        "anchor",
        "nullifiers",
        "output_commitments",
        "encrypted_outputs",
        "value_balance",
        "fee",
        "proof",
        "binding_signature",
    ] {
        if !parsed
            .as_object()
            .is_some_and(|object| object.contains_key(field))
        {
            return Err(invalid_request_params(
                "action_json",
                format!("missing Orchard action field `{field}`"),
            ));
        }
    }
    Ok(parsed)
}

fn validate_shielded_swap_json_request_param(
    swap_json: &str,
) -> Result<Value, RpcRequestValidationError> {
    if swap_json.len() > MAX_RPC_SHIELDED_SWAP_JSON_BYTES {
        return Err(invalid_request_params(
            "swap_json",
            format!("expected at most {MAX_RPC_SHIELDED_SWAP_JSON_BYTES} bytes"),
        ));
    }
    let parsed = serde_json::from_str::<Value>(swap_json)
        .map_err(|error| invalid_request_params("swap_json", format!("{error}")))?;
    validate_shielded_swap_action_json_object(&parsed, "swap_json")?;
    Ok(parsed)
}

fn validate_shielded_swap_action_json_object(
    parsed: &Value,
    field_prefix: &str,
) -> Result<(), RpcRequestValidationError> {
    if !parsed.is_object() {
        return Err(invalid_request_params(
            field_prefix,
            "expected ShieldedSwap action JSON object",
        ));
    }
    if contains_private_key_material_field(parsed) {
        return Err(invalid_request_params(
            field_prefix,
            "ShieldedSwap action payload contains private key material fields",
        ));
    }
    let object = parsed
        .as_object()
        .ok_or_else(|| invalid_request_params(field_prefix, "expected JSON object"))?;
    if object.contains_key("pool_domain") || object.contains_key("randomized_verification_keys") {
        return validate_asset_orchard_swap_action_json_object(parsed, field_prefix);
    }
    validate_legacy_shielded_swap_action_json_object(parsed, field_prefix)
}

fn validate_asset_orchard_swap_action_json_object(
    parsed: &Value,
    field_prefix: &str,
) -> Result<(), RpcRequestValidationError> {
    require_json_fields(
        parsed,
        field_prefix,
        "AssetOrchardSwap action",
        &[
            "version",
            "schema",
            "pool_id",
            "proof_system_id",
            "circuit_id",
            "pool_domain",
            "anchor",
            "nullifiers",
            "randomized_verification_keys",
            "output_commitments",
            "encrypted_outputs",
            "pricing_claim",
            "swap_binding_hash",
            "fee",
            "proof",
            "spend_authorization_signatures",
        ],
    )?;
    validate_json_u64_eq_field(parsed, "version", 1, field_prefix)?;
    validate_json_string_eq_field(
        parsed,
        "schema",
        ASSET_ORCHARD_SWAP_ACTION_SCHEMA_V1,
        field_prefix,
    )?;
    validate_json_string_eq_field(parsed, "pool_id", ASSET_ORCHARD_POOL_ID_V1, field_prefix)?;
    validate_json_string_eq_field(
        parsed,
        "proof_system_id",
        ASSET_ORCHARD_PROOF_SYSTEM_ID_V1,
        field_prefix,
    )?;
    validate_json_string_eq_field(
        parsed,
        "circuit_id",
        ASSET_ORCHARD_CIRCUIT_ID_V1,
        field_prefix,
    )?;
    validate_json_lower_hex_field(parsed, "pool_domain", 64, field_prefix)?;
    validate_json_lower_hex_field(parsed, "anchor", 64, field_prefix)?;
    validate_json_hex_array(parsed, "nullifiers", 2, 64, field_prefix)?;
    validate_json_hex_array(
        parsed,
        "randomized_verification_keys",
        2,
        64,
        field_prefix,
    )?;
    validate_json_hex_array(parsed, "output_commitments", 2, 64, field_prefix)?;
    validate_json_lower_hex_blob_array(
        parsed,
        "encrypted_outputs",
        2,
        MAX_RPC_ASSET_ORCHARD_ENCRYPTED_OUTPUT_BYTES,
        field_prefix,
    )?;
    let pricing = parsed
        .get("pricing_claim")
        .ok_or_else(|| invalid_request_params(format!("{field_prefix}.pricing_claim"), "missing pricing claim"))?;
    let pricing_prefix = format!("{field_prefix}.pricing_claim");
    for field in ["nav_epoch", "ratio_numerator", "ratio_denominator"] {
        let value = pricing.get(field).and_then(Value::as_u64).unwrap_or(0);
        if value == 0 {
            return Err(invalid_request_params(format!("{pricing_prefix}.{field}"), "expected nonzero u64"));
        }
    }
    validate_json_lower_hex_field(pricing, "reserve_packet_hash", 96, &pricing_prefix)?;
    for tag in ["base_asset_tag_lo", "base_asset_tag_hi", "quote_asset_tag_lo", "quote_asset_tag_hi"] {
        validate_json_lower_hex_field(pricing, tag, 32, &pricing_prefix)?;
    }
    let mode = pricing.get("mode").and_then(Value::as_str).unwrap_or_default();
    if !matches!(mode, "at_nav" | "at_nav_with_band" | "negotiated") {
        return Err(invalid_request_params(format!("{pricing_prefix}.mode"), "unsupported pricing mode"));
    }
    if pricing.get("band_bps").and_then(Value::as_u64).is_none_or(|band| band > 10_000) {
        return Err(invalid_request_params(format!("{pricing_prefix}.band_bps"), "expected u64 <= 10000"));
    }
    validate_json_lower_hex_field(parsed, "swap_binding_hash", 128, field_prefix)?;
    validate_json_u64_eq_field(parsed, "fee", 0, field_prefix)?;
    validate_json_lower_hex_blob_field(
        parsed,
        "proof",
        MAX_RPC_ASSET_ORCHARD_PROOF_BYTES,
        field_prefix,
    )?;
    validate_json_hex_array(
        parsed,
        "spend_authorization_signatures",
        2,
        128,
        field_prefix,
    )?;
    Ok(())
}

fn validate_legacy_shielded_swap_action_json_object(
    parsed: &Value,
    field_prefix: &str,
) -> Result<(), RpcRequestValidationError> {
    for field in [
        "schema",
        "pool_id",
        "proof_system_id",
        "circuit_id",
        "anchor",
        "nullifiers",
        "input_asset_commitments",
        "input_value_commitments",
        "input_authorization_commitments",
        "output_commitments",
        "output_asset_commitments",
        "output_value_commitments",
        "encrypted_outputs",
        "swap_binding_hash",
        "fee",
        "proof",
    ] {
        if !parsed
            .as_object()
            .is_some_and(|object| object.contains_key(field))
        {
            return Err(invalid_request_params(
                field_prefix,
                format!("missing ShieldedSwap action field `{field}`"),
            ));
        }
    }
    validate_json_string_field(parsed, "schema", field_prefix)?;
    validate_json_string_field(parsed, "pool_id", field_prefix)?;
    validate_json_string_field(parsed, "proof_system_id", field_prefix)?;
    validate_json_string_field(parsed, "circuit_id", field_prefix)?;
    validate_json_lower_hex_field(parsed, "anchor", 64, field_prefix)?;
    validate_json_lower_hex_field(parsed, "swap_binding_hash", 96, field_prefix)?;
    validate_json_lower_hex_field(parsed, "proof", 96, field_prefix)?;
    if parsed
        .as_object()
        .and_then(|object| object.get("fee"))
        .and_then(Value::as_u64)
        .is_none()
    {
        return Err(invalid_request_params(
            format!("{field_prefix}.fee"),
            "expected u64 fee",
        ));
    }
    validate_json_hex_array(parsed, "nullifiers", 2, 64, field_prefix)?;
    validate_json_hex_array(parsed, "input_asset_commitments", 2, 96, field_prefix)?;
    validate_json_hex_array(parsed, "input_value_commitments", 2, 96, field_prefix)?;
    validate_json_hex_array(parsed, "input_authorization_commitments", 2, 96, field_prefix)?;
    validate_json_hex_array(parsed, "output_commitments", 2, 64, field_prefix)?;
    validate_json_hex_array(parsed, "output_asset_commitments", 2, 96, field_prefix)?;
    validate_json_hex_array(parsed, "output_value_commitments", 2, 96, field_prefix)?;
    let encrypted_outputs = parsed
        .as_object()
        .and_then(|object| object.get("encrypted_outputs"))
        .and_then(Value::as_array)
        .ok_or_else(|| {
            invalid_request_params(
                format!("{field_prefix}.encrypted_outputs"),
                "expected two encrypted outputs",
            )
        })?;
    if encrypted_outputs.len() != 2 {
        return Err(invalid_request_params(
            format!("{field_prefix}.encrypted_outputs"),
            "expected exactly two encrypted outputs",
        ));
    }
    for (index, output) in encrypted_outputs.iter().enumerate() {
        let prefix = format!("{field_prefix}.encrypted_outputs[{index}]");
        validate_json_lower_hex_field(output, "cmx", 64, &prefix)?;
        validate_json_lower_hex_field(output, "epk", 64, &prefix)?;
        validate_json_lower_hex_field(
            output,
            "enc_ciphertext",
            1160,
            &prefix,
        )?;
        validate_json_lower_hex_field(output, "out_ciphertext", 160, &prefix)?;
        if output
            .as_object()
            .is_some_and(|object| object.contains_key("compact_ciphertext"))
        {
            validate_json_lower_hex_field(output, "compact_ciphertext", 104, &prefix)?;
        }
    }
    Ok(())
}

fn require_json_fields(
    parsed: &Value,
    field_prefix: &str,
    action_name: &str,
    fields: &[&str],
) -> Result<(), RpcRequestValidationError> {
    for field in fields {
        if !parsed
            .as_object()
            .is_some_and(|object| object.contains_key(*field))
        {
            return Err(invalid_request_params(
                field_prefix,
                format!("missing {action_name} field `{field}`"),
            ));
        }
    }
    Ok(())
}

fn validate_json_string_field(
    value: &Value,
    field: &str,
    field_prefix: &str,
) -> Result<(), RpcRequestValidationError> {
    if value
        .as_object()
        .and_then(|object| object.get(field))
        .and_then(Value::as_str)
        .is_some_and(|found| !found.trim().is_empty())
    {
        Ok(())
    } else {
        Err(invalid_request_params(
            format!("{field_prefix}.{field}"),
            "expected nonempty string",
        ))
    }
}

fn validate_json_string_eq_field(
    value: &Value,
    field: &str,
    expected: &str,
    field_prefix: &str,
) -> Result<(), RpcRequestValidationError> {
    let found = value
        .as_object()
        .and_then(|object| object.get(field))
        .and_then(Value::as_str)
        .ok_or_else(|| {
            invalid_request_params(format!("{field_prefix}.{field}"), "expected string value")
        })?;
    if found != expected {
        return Err(invalid_request_params(
            format!("{field_prefix}.{field}"),
            format!("expected `{expected}`"),
        ));
    }
    Ok(())
}

fn validate_json_u64_eq_field(
    value: &Value,
    field: &str,
    expected: u64,
    field_prefix: &str,
) -> Result<(), RpcRequestValidationError> {
    let found = value
        .as_object()
        .and_then(|object| object.get(field))
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            invalid_request_params(format!("{field_prefix}.{field}"), "expected u64 value")
        })?;
    if found != expected {
        return Err(invalid_request_params(
            format!("{field_prefix}.{field}"),
            format!("expected {expected}"),
        ));
    }
    Ok(())
}

fn validate_json_lower_hex_field(
    value: &Value,
    field: &str,
    expected_len: usize,
    field_prefix: &str,
) -> Result<(), RpcRequestValidationError> {
    let found = value
        .as_object()
        .and_then(|object| object.get(field))
        .and_then(Value::as_str)
        .ok_or_else(|| {
            invalid_request_params(
                format!("{field_prefix}.{field}"),
                format!("expected {expected_len} lowercase hex characters"),
            )
        })?;
    if !is_lower_hex_len(found, expected_len) {
        return Err(invalid_request_params(
            format!("{field_prefix}.{field}"),
            format!("expected {expected_len} lowercase hex characters"),
        ));
    }
    Ok(())
}

fn validate_json_lower_hex_blob_field(
    value: &Value,
    field: &str,
    max_bytes: usize,
    field_prefix: &str,
) -> Result<(), RpcRequestValidationError> {
    let found = value
        .as_object()
        .and_then(|object| object.get(field))
        .and_then(Value::as_str)
        .ok_or_else(|| {
            invalid_request_params(
                format!("{field_prefix}.{field}"),
                format!("expected nonempty lowercase hex blob up to {max_bytes} bytes"),
            )
        })?;
    validate_lower_hex_blob(
        found,
        max_bytes,
        format!("{field_prefix}.{field}"),
    )
}

fn validate_json_hex_array(
    value: &Value,
    field: &str,
    expected_count: usize,
    expected_len: usize,
    field_prefix: &str,
) -> Result<(), RpcRequestValidationError> {
    let entries = value
        .as_object()
        .and_then(|object| object.get(field))
        .and_then(Value::as_array)
        .ok_or_else(|| {
            invalid_request_params(
                format!("{field_prefix}.{field}"),
                format!("expected {expected_count} hex entries"),
            )
        })?;
    if entries.len() != expected_count {
        return Err(invalid_request_params(
            format!("{field_prefix}.{field}"),
            format!("expected exactly {expected_count} entries"),
        ));
    }
    for (index, entry) in entries.iter().enumerate() {
        let Some(found) = entry.as_str() else {
            return Err(invalid_request_params(
                format!("{field_prefix}.{field}[{index}]"),
                format!("expected {expected_len} lowercase hex characters"),
            ));
        };
        if !is_lower_hex_len(found, expected_len) {
            return Err(invalid_request_params(
                format!("{field_prefix}.{field}[{index}]"),
                format!("expected {expected_len} lowercase hex characters"),
            ));
        }
    }
    Ok(())
}

fn validate_json_lower_hex_blob_array(
    value: &Value,
    field: &str,
    expected_count: usize,
    max_bytes: usize,
    field_prefix: &str,
) -> Result<(), RpcRequestValidationError> {
    let entries = value
        .as_object()
        .and_then(|object| object.get(field))
        .and_then(Value::as_array)
        .ok_or_else(|| {
            invalid_request_params(
                format!("{field_prefix}.{field}"),
                format!("expected {expected_count} lower-hex blobs"),
            )
        })?;
    if entries.len() != expected_count {
        return Err(invalid_request_params(
            format!("{field_prefix}.{field}"),
            format!("expected exactly {expected_count} entries"),
        ));
    }
    for (index, entry) in entries.iter().enumerate() {
        let Some(found) = entry.as_str() else {
            return Err(invalid_request_params(
                format!("{field_prefix}.{field}[{index}]"),
                format!("expected nonempty lowercase hex blob up to {max_bytes} bytes"),
            ));
        };
        validate_lower_hex_blob(
            found,
            max_bytes,
            format!("{field_prefix}.{field}[{index}]"),
        )?;
    }
    Ok(())
}

fn validate_lower_hex_blob(
    found: &str,
    max_bytes: usize,
    field: String,
) -> Result<(), RpcRequestValidationError> {
    if found.is_empty() || found.len() % 2 != 0 || found.len() > max_bytes * 2 {
        return Err(invalid_request_params(
            field,
            format!("expected nonempty lowercase hex blob up to {max_bytes} bytes"),
        ));
    }
    if !found
        .bytes()
        .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        return Err(invalid_request_params(
            field,
            format!("expected nonempty lowercase hex blob up to {max_bytes} bytes"),
        ));
    }
    Ok(())
}

fn validate_shield_scan_request_params(params: &Value) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    require_only_params(params, &["owner"])?;
    string_param(params, "owner")?;
    Ok(())
}

fn validate_shield_disclose_request_params(
    params: &Value,
) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    require_only_params(params, &["note_id"])?;
    string_param(params, "note_id")?;
    Ok(())
}

fn validate_navcoin_bridge_routes_request_params(
    params: &Value,
) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    require_only_params(params, &[])?;
    Ok(())
}

fn validate_navcoin_bridge_packet_request_params(
    params: &Value,
) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    require_only_params(params, &["route_id", "packet_hash"])?;
    string_param(params, "route_id")?;
    lower_hex_param(params, "packet_hash", 96)?;
    Ok(())
}

fn validate_navcoin_bridge_claims_request_params(
    params: &Value,
) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    require_only_params(params, &["route_id", "limit", "include_terminal"])?;
    string_param(params, "route_id")?;
    optional_bounded_nonzero_usize_param(params, "limit", MAX_RPC_READ_QUERY_LIMIT)?;
    optional_bool_param(params, "include_terminal")?;
    Ok(())
}

fn validate_navcoin_bridge_supply_status_request_params(
    params: &Value,
) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    require_only_params(params, &["route_id"])?;
    string_param(params, "route_id")?;
    Ok(())
}

fn validate_navcoin_bridge_receipt_replay_request_params(
    params: &Value,
) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    require_only_params(params, &["route_id"])?;
    string_param(params, "route_id")?;
    Ok(())
}

fn validate_navcoin_bridge_packet_preflight_request_params(
    params: &Value,
) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    require_only_params(params, &["route_id", "packet_file"])?;
    string_param(params, "route_id")?;
    string_param(params, "packet_file")?;
    Ok(())
}

fn validate_bridge_batch_domain_request_params(
    params: &Value,
) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    require_only_params(
        params,
        &[
            "domain_id",
            "name",
            "source_chain",
            "target_chain",
            "bridge_id",
            "door_account",
            "inbound_cap",
            "outbound_cap",
            "batch_file",
        ],
    )?;
    string_param(params, "domain_id")?;
    string_param(params, "name")?;
    optional_string_param(params, "source_chain")?;
    optional_string_param(params, "target_chain")?;
    optional_string_param(params, "bridge_id")?;
    optional_string_param(params, "door_account")?;
    nonzero_u64_param(params, "inbound_cap")?;
    nonzero_u64_param(params, "outbound_cap")?;
    string_param(params, "batch_file")?;
    Ok(())
}

fn validate_bridge_batch_transfer_request_params(
    params: &Value,
) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    require_only_params(
        params,
        &[
            "domain_id",
            "direction",
            "from",
            "to",
            "asset_id",
            "amount",
            "witness_id",
            "witness_epoch",
            "witness_signer",
            "batch_file",
        ],
    )?;
    string_param(params, "domain_id")?;
    bridge_direction_param(params, "direction")?;
    string_param(params, "from")?;
    string_param(params, "to")?;
    string_param(params, "asset_id")?;
    nonzero_u64_param(params, "amount")?;
    string_param(params, "witness_id")?;
    optional_u32_param(params, "witness_epoch")?;
    optional_string_param(params, "witness_signer")?;
    string_param(params, "batch_file")?;
    Ok(())
}

fn validate_bridge_batch_pause_request_params(
    params: &Value,
) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    require_only_params(params, &["domain_id", "batch_file"])?;
    string_param(params, "domain_id")?;
    string_param(params, "batch_file")?;
    Ok(())
}

fn require_only_params(
    params: &Map<String, Value>,
    allowed: &[&str],
) -> Result<(), RpcRequestValidationError> {
    for key in params.keys() {
        if !allowed.iter().any(|allowed| allowed == key) {
            return Err(invalid_request_params(
                key,
                format!("unexpected param `{key}`"),
            ));
        }
    }
    Ok(())
}

fn optional_string_param(
    params: &Map<String, Value>,
    key: &str,
) -> Result<(), RpcRequestValidationError> {
    if params.contains_key(key) {
        string_param(params, key)?;
    }
    Ok(())
}

fn string_param<'a>(
    params: &'a Map<String, Value>,
    key: &str,
) -> Result<&'a str, RpcRequestValidationError> {
    let value = params
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_request_params(key, "expected string value"))?;
    if value.trim().is_empty() {
        return Err(invalid_request_params(
            key,
            "expected nonempty string value",
        ));
    }
    let max_bytes = max_rpc_param_string_bytes("", key);
    if value.len() > max_bytes {
        return Err(invalid_request_params(
            key,
            format!("expected string at most {max_bytes} bytes"),
        ));
    }
    Ok(value)
}

fn u64_param(params: &Map<String, Value>, key: &str) -> Result<u64, RpcRequestValidationError> {
    params
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid_request_params(key, "expected unsigned integer value"))
}

fn nonzero_u64_param(
    params: &Map<String, Value>,
    key: &str,
) -> Result<u64, RpcRequestValidationError> {
    let value = u64_param(params, key)?;
    if value == 0 {
        return Err(invalid_request_params(
            key,
            "expected nonzero unsigned integer value",
        ));
    }
    Ok(value)
}

fn optional_nonzero_u64_param(
    params: &Map<String, Value>,
    key: &str,
) -> Result<(), RpcRequestValidationError> {
    if params.contains_key(key) {
        nonzero_u64_param(params, key)?;
    }
    Ok(())
}

fn optional_u64_param(
    params: &Map<String, Value>,
    key: &str,
) -> Result<(), RpcRequestValidationError> {
    if params.contains_key(key) {
        u64_param(params, key)?;
    }
    Ok(())
}

fn optional_nonzero_usize_param(
    params: &Map<String, Value>,
    key: &str,
) -> Result<(), RpcRequestValidationError> {
    optional_bounded_nonzero_usize_param(params, key, usize::MAX)
}

fn optional_bool_param(
    params: &Map<String, Value>,
    key: &str,
) -> Result<(), RpcRequestValidationError> {
    if let Some(value) = params.get(key) {
        value
            .as_bool()
            .ok_or_else(|| invalid_request_params(key, "expected boolean value"))?;
    }
    Ok(())
}

fn optional_bounded_nonzero_usize_param(
    params: &Map<String, Value>,
    key: &str,
    max: usize,
) -> Result<(), RpcRequestValidationError> {
    if let Some(value) = params.get(key) {
        let value = value
            .as_u64()
            .ok_or_else(|| invalid_request_params(key, "expected unsigned integer value"))?;
        if value == 0 {
            return Err(invalid_request_params(
                key,
                "expected nonzero unsigned integer value",
            ));
        }
        if value > usize::MAX as u64 {
            return Err(invalid_request_params(
                key,
                "expected value to fit in usize",
            ));
        }
        if value > max as u64 {
            return Err(invalid_request_params(
                key,
                format!("expected value at most {max}"),
            ));
        }
    }
    Ok(())
}

fn optional_u32_param(
    params: &Map<String, Value>,
    key: &str,
) -> Result<(), RpcRequestValidationError> {
    if let Some(value) = params.get(key) {
        let value = value
            .as_u64()
            .ok_or_else(|| invalid_request_params(key, "expected unsigned integer value"))?;
        if value > u64::from(u32::MAX) {
            return Err(invalid_request_params(key, "expected value to fit in u32"));
        }
    }
    Ok(())
}

fn bridge_direction_param(
    params: &Map<String, Value>,
    key: &str,
) -> Result<(), RpcRequestValidationError> {
    let direction = string_param(params, key)?;
    if !matches!(direction, "inbound" | "outbound") {
        return Err(invalid_request_params(
            key,
            format!("expected `inbound` or `outbound`, found `{direction}`"),
        ));
    }
    Ok(())
}

fn optional_batch_kind_param(
    params: &Map<String, Value>,
    key: &str,
) -> Result<(), RpcRequestValidationError> {
    if let Some(value) = params.get(key) {
        let batch_kind = value
            .as_str()
            .ok_or_else(|| invalid_request_params(key, "expected string value"))?;
        if batch_kind.trim().is_empty() {
            return Err(invalid_request_params(
                key,
                "expected nonempty string value",
            ));
        }
        if !is_supported_batch_kind(batch_kind) {
            return Err(invalid_request_params(
                key,
                format!("unsupported batch kind `{batch_kind}`"),
            ));
        }
    }
    Ok(())
}

fn optional_lower_hex_param(
    params: &Map<String, Value>,
    key: &str,
    expected_len: usize,
) -> Result<(), RpcRequestValidationError> {
    if params.contains_key(key) {
        let found = string_param(params, key)?;
        if !is_lower_hex_len(found, expected_len) {
            return Err(invalid_request_params(
                key,
                format!("expected {expected_len} lowercase hex characters"),
            ));
        }
    }
    Ok(())
}

fn lower_hex_param(
    params: &Map<String, Value>,
    key: &str,
    expected_len: usize,
) -> Result<(), RpcRequestValidationError> {
    let found = string_param(params, key)?;
    if !is_lower_hex_len(found, expected_len) {
        return Err(invalid_request_params(
            key,
            format!("expected {expected_len} lowercase hex characters"),
        ));
    }
    Ok(())
}

fn dex_asset_id_param(
    params: &Map<String, Value>,
    key: &str,
) -> Result<(), RpcRequestValidationError> {
    let found = string_param(params, key)?;
    if found == "PFT" || is_lower_hex_len(found, ISSUED_ASSET_ID_HEX_LEN) {
        return Ok(());
    }
    Err(invalid_request_params(
        key,
        format!("expected PFT or {ISSUED_ASSET_ID_HEX_LEN} lowercase hex characters"),
    ))
}

fn invalid_request_params(
    field: impl Into<String>,
    message: impl Into<String>,
) -> RpcRequestValidationError {
    RpcRequestValidationError::InvalidParams {
        field: field.into(),
        message: message.into(),
    }
}

pub fn success_response<T: Serialize>(
    id: impl Into<String>,
    result: &T,
    events: Vec<RpcEvent>,
) -> Result<RpcResponse, serde_json::Error> {
    Ok(RpcResponse {
        version: RPC_VERSION.to_string(),
        id: id.into(),
        ok: true,
        result: Some(serde_json::to_value(result)?),
        error: None,
        events,
    })
}

pub fn error_response(
    id: impl Into<String>,
    code: impl Into<String>,
    message: impl Into<String>,
    events: Vec<RpcEvent>,
) -> RpcResponse {
    RpcResponse {
        version: RPC_VERSION.to_string(),
        id: id.into(),
        ok: false,
        result: None,
        error: Some(RpcError {
            code: code.into(),
            message: message.into(),
        }),
        events,
    }
}

include!("protocol_response_helpers.rs");
