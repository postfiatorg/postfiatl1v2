//! Pillar 6 Hive Chat on-chain identity (`identity_set`) payload builders and
//! dual-key (ML-DSA-65 + BIP-340 Nostr Schnorr) binding proof signers.
//!
//! Conforms to the locked Pillar 6 spec (`06-hive-identity.md`) and Train B
//! Contract §2 (`docs/specs/l1-upgrade-train-b-contract.md`).

use crate::unified_kdf::{
    decode_bip39_seed64_hex, derive_nostr_v2_keypair, encode_npub,
    nostr_v1_keypair_from_secret_hex, NostrKeyMaterial,
};
use k256::schnorr::{Signature as SchnorrSignature, SigningKey as SchnorrSigningKey, VerifyingKey};
use postfiat_crypto_provider::{
    address_from_public_key, bytes_to_hex, hash_hex, hex_to_bytes, ml_dsa_65_sign_with_context,
    ml_dsa_65_verify_with_context, ML_DSA_65_ALGORITHM,
};
use postfiat_rpc_sdk::{derive_wallet_key_pair, wallet_identity_from_backup, WalletBackupFile};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use sha3::{Digest as _, Sha3_384};

pub const IDENTITY_SET_TRANSACTION_KIND: &str = "identity_set";
pub const IDENTITY_MINT_STATE_EXPANSION_FEE: u64 = 20_000;
pub const IDENTITY_LINK_STATE_EXPANSION_FEE: u64 = 10_000;

pub const IDENTITY_MAX_LINKS: usize = 16;
pub const IDENTITY_MAX_RELAYS: usize = 5;
pub const IDENTITY_MAX_RELAY_URL_BYTES: usize = 128;
pub const IDENTITY_HANDLE_MIN_LEN: usize = 2;
pub const IDENTITY_HANDLE_MAX_LEN: usize = 32;
pub const IDENTITY_NAMESPACE_MAX_LEN: usize = 128;
pub const IDENTITY_NOSTR_PUBKEY_HEX_LEN: usize = 64;
pub const IDENTITY_BIP340_SIG_HEX_LEN: usize = 128;
pub const IDENTITY_XWING_PUBKEY_HEX_LEN: usize = 2432;
pub const IDENTITY_PQ_ADDRESS_MAX_LEN: usize = 128;

// Exact domain separation strings from Train B Contract §2.1.
pub const IDENTITY_LINK_ID_DOMAIN: &str = "postfiat.identity.link_id.v1";
pub const IDENTITY_NOSTR_PROOF_DOMAIN: &str = "postfiat-l1-v2/identity-nostr/v1";
pub const IDENTITY_VALIDATOR_LINK_DOMAIN: &str = "postfiat-l1-v2/identity-link/v1";
pub const IDENTITY_OLD_CHAIN_LINK_DOMAIN: &str = "postfiat-l1-v2/identity-old-chain/v1";
pub const IDENTITY_CONTROLLER_LINK_DOMAIN: &str = "postfiat-l1-v2/identity-controller/v1";
pub const IDENTITY_UNLINK_SELF_DOMAIN: &str = "postfiat-l1-v2/identity-unlink/v1";
pub const IDENTITY_ROTATE_ACCOUNT_DOMAIN: &str = "postfiat-l1-v2/identity-rotate-account/v1";
pub const IDENTITY_HANDLE_RESERVE_DOMAIN: &str = "postfiat.identity.handle_reserve.v1";
pub const IDENTITY_HANDLE_ATTESTATION_DOMAIN: &str = "postfiat-l1-v2/handle-attestation/v1";
pub const IDENTITY_SET_DUAL_PROOF_CONTEXT: &[u8] = b"postfiat-l1-v2/identity-set-binding/v1";

// Pillar 6 §1 client-side v1 signing guard prefixes.
pub const COLLABORATION_AUTH_PREFIX: &str = "Task Node collaboration authorization";
pub const NOSTR_V1_DERIVATION_PREFIX: &str = "Task Node private messaging identity";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityKind {
    Human,
    Agent,
    Service,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityRotateMode {
    Planned,
    Compromised,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityLinkKind {
    ValidatorNode,
    OldChainAccount,
    AgentController,
    CustomNip05,
    EvmAddress,
    SolanaAddress,
}

impl IdentityLinkKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ValidatorNode => "validator_node",
            Self::OldChainAccount => "old_chain_account",
            Self::AgentController => "agent_controller",
            Self::CustomNip05 => "custom_nip05",
            Self::EvmAddress => "evm_address",
            Self::SolanaAddress => "solana_address",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandleAttestationV1 {
    pub namespace: String,
    pub handle: String,
    pub account: String,
    pub expires_at_height: u64,
    pub registrar_account: String,
    pub registrar_pubkey_hex: String,
    pub registrar_signature_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum IdentitySetActionV1 {
    Mint {
        nostr_pubkey: String,
        nostr_proof_sig_hex: String,
        derivation_version: u8,
        key_index: u32,
        identity_kind: IdentityKind,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        handle_attestation: Option<HandleAttestationV1>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reservation_salt_hex: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        relays: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        avatar_nft_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        message_key: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pq_receive_address: Option<String>,
    },
    Update {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        relays: Option<Vec<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        avatar_nft_id: Option<Option<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        message_key: Option<Option<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pq_receive_address: Option<Option<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        handle_attestation: Option<HandleAttestationV1>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reservation_salt_hex: Option<String>,
    },
    Link {
        kind: IdentityLinkKind,
        target: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        proof_pubkey_hex: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        proof_sig_hex: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        metadata_hash: Option<String>,
    },
    Unlink {
        link_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        self_unlink_pubkey_hex: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        self_unlink_sig_hex: Option<String>,
    },
    Rotate {
        mode: IdentityRotateMode,
        new_nostr_pubkey: String,
        new_nostr_proof_sig_hex: String,
        derivation_version: u8,
        key_index: u32,
    },
    RotateAccount {
        new_account: String,
        new_account_pubkey_hex: String,
        new_account_sig_hex: String,
    },
    Revoke {
        reason_code: String,
    },
}

/// Wire-compatible `AssetTransactionOperation` payload for Train B `identity_set`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentitySetOperationPayload {
    pub operation: String,
    pub account: String,
    pub payload: IdentitySetActionV1,
}

/// Dual-key binding bundle produced client-side (ML-DSA-65 account signature + BIP-340 Schnorr proof).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedIdentitySetBundle {
    pub schema: String,
    pub chain_id: String,
    pub account: String,
    pub tx_sequence: u64,
    pub nostr_pubkey_hex: String,
    pub npub: String,
    pub nostr_proof_sig_hex: String,
    pub algorithm_id: String,
    pub ml_dsa_public_key_hex: String,
    pub ml_dsa_binding_sig_hex: String,
    pub operation: IdentitySetOperationPayload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityCosignatureProof {
    pub domain: String,
    pub signer_account: String,
    pub algorithm_id: String,
    pub public_key_hex: String,
    pub signature_hex: String,
}

// ---------------------------------------------------------------------------
// Strict Pillar 6 validation helpers
// ---------------------------------------------------------------------------

pub fn is_lower_hex_len(value: &str, expected_len: usize) -> bool {
    value.len() == expected_len
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

pub fn validate_canonical_pf_account(field: &str, account: &str) -> Result<(), String> {
    if account.len() != 42
        || !account.starts_with("pf")
        || !account[2..]
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(format!(
            "{field} must be a canonical lowercase pf... address"
        ));
    }
    Ok(())
}

/// Enforce canonical handle syntax: `[a-z0-9._-]`, 2..=32 chars, no leading/trailing punctuation.
pub fn validate_identity_handle(handle: &str) -> Result<(), String> {
    if !(IDENTITY_HANDLE_MIN_LEN..=IDENTITY_HANDLE_MAX_LEN).contains(&handle.len()) {
        return Err("identity_invalid_handle: handle length must be 2..=32".to_string());
    }
    let bytes = handle.as_bytes();
    let is_punct = |b: u8| matches!(b, b'.' | b'_' | b'-');
    if is_punct(bytes[0]) || is_punct(bytes[bytes.len() - 1]) {
        return Err(
            "identity_invalid_handle: handle must not start or end with punctuation".to_string(),
        );
    }
    if !bytes
        .iter()
        .all(|&b| b.is_ascii_lowercase() || b.is_ascii_digit() || is_punct(b))
    {
        return Err("identity_invalid_handle: handle must match [a-z0-9._-]".to_string());
    }
    Ok(())
}

pub fn validate_identity_relays(relays: &[String]) -> Result<(), String> {
    if relays.len() > IDENTITY_MAX_RELAYS {
        return Err(format!("relays count exceeds max {IDENTITY_MAX_RELAYS}"));
    }
    for relay in relays {
        if relay.len() > IDENTITY_MAX_RELAY_URL_BYTES
            || !relay.starts_with("wss://")
            || relay.len() <= 6
            || relay
                .bytes()
                .any(|b| b.is_ascii_whitespace() || b.is_ascii_control())
        {
            return Err(format!("invalid relay URL `{relay}`"));
        }
    }
    Ok(())
}

pub fn validate_optional_identity_fields(
    avatar_nft_id: Option<&str>,
    message_key: Option<&str>,
    pq_receive_address: Option<&str>,
) -> Result<(), String> {
    if let Some(nft) = avatar_nft_id {
        if !is_lower_hex_len(nft, 96) {
            return Err("avatar_nft_id must be 96 lowercase hex chars".to_string());
        }
    }
    if let Some(mk) = message_key {
        if !is_lower_hex_len(mk, IDENTITY_XWING_PUBKEY_HEX_LEN) {
            return Err(format!(
                "message_key must be {IDENTITY_XWING_PUBKEY_HEX_LEN} lowercase hex chars"
            ));
        }
    }
    if let Some(pq) = pq_receive_address {
        if !pq.starts_with("pfpq1")
            || pq.len() > IDENTITY_PQ_ADDRESS_MAX_LEN
            || pq
                .bytes()
                .any(|b| b.is_ascii_whitespace() || b.is_ascii_control())
        {
            return Err("pq_receive_address must be a valid pfpq1... address <= 128B".to_string());
        }
    }
    Ok(())
}

/// Pillar 6 §1 client-side security guard preventing cross-purpose signing on v1 old-chain keys.
pub fn validate_v1_signing_guard(purpose: &str, message: &str) -> Result<(), String> {
    match purpose {
        "collaboration" => {
            if !message.starts_with(COLLABORATION_AUTH_PREFIX) {
                return Err(format!(
                    "collaboration signing guard rejected message without prefix `{COLLABORATION_AUTH_PREFIX}`"
                ));
            }
            if message.contains(NOSTR_V1_DERIVATION_PREFIX) {
                return Err(
                    "collaboration signing guard blocked nostr v1 derivation phrase".into(),
                );
            }
            Ok(())
        }
        "nostr_v1_derivation" => {
            if !message.starts_with(NOSTR_V1_DERIVATION_PREFIX) {
                return Err(format!(
                    "nostr v1 derivation guard rejected message without prefix `{NOSTR_V1_DERIVATION_PREFIX}`"
                ));
            }
            Ok(())
        }
        _ => Err(format!("unknown v1 signing guard purpose `{purpose}`")),
    }
}

pub fn validate_identity_set_operation(op: &IdentitySetOperationPayload) -> Result<(), String> {
    if op.operation != IDENTITY_SET_TRANSACTION_KIND {
        return Err(format!(
            "operation must be `{IDENTITY_SET_TRANSACTION_KIND}`"
        ));
    }
    validate_canonical_pf_account("identity_set.account", &op.account)?;
    match &op.payload {
        IdentitySetActionV1::Mint {
            nostr_pubkey,
            nostr_proof_sig_hex,
            derivation_version,
            handle_attestation,
            relays,
            avatar_nft_id,
            message_key,
            pq_receive_address,
            ..
        } => {
            if !matches!(derivation_version, 1 | 2) {
                return Err("derivation_version must be 1 or 2".to_string());
            }
            if !is_lower_hex_len(nostr_pubkey, IDENTITY_NOSTR_PUBKEY_HEX_LEN) {
                return Err("nostr_pubkey must be 64 lowercase hex chars".to_string());
            }
            if !is_lower_hex_len(nostr_proof_sig_hex, IDENTITY_BIP340_SIG_HEX_LEN) {
                return Err("nostr_proof_sig_hex must be 128 lowercase hex chars".to_string());
            }
            if let Some(att) = handle_attestation {
                validate_handle_attestation(att, &op.account)?;
            }
            validate_identity_relays(relays)?;
            validate_optional_identity_fields(
                avatar_nft_id.as_deref(),
                message_key.as_deref(),
                pq_receive_address.as_deref(),
            )?;
        }
        IdentitySetActionV1::Update {
            relays,
            avatar_nft_id,
            message_key,
            pq_receive_address,
            handle_attestation,
            ..
        } => {
            if let Some(relays) = relays {
                validate_identity_relays(relays)?;
            }
            validate_optional_identity_fields(
                avatar_nft_id.as_ref().and_then(|v| v.as_deref()),
                message_key.as_ref().and_then(|v| v.as_deref()),
                pq_receive_address.as_ref().and_then(|v| v.as_deref()),
            )?;
            if let Some(att) = handle_attestation {
                validate_handle_attestation(att, &op.account)?;
            }
        }
        IdentitySetActionV1::Link {
            target,
            metadata_hash,
            ..
        } => {
            if target.trim().is_empty() || target.len() > 256 {
                return Err("link target must be 1..=256 bytes".to_string());
            }
            if let Some(hash) = metadata_hash {
                if !is_lower_hex_len(hash, 96) {
                    return Err("link metadata_hash must be 96 lowercase hex chars".to_string());
                }
            }
        }
        IdentitySetActionV1::Unlink { link_id, .. } => {
            if !is_lower_hex_len(link_id, 96) {
                return Err("unlink link_id must be 96 lowercase hex chars".to_string());
            }
        }
        IdentitySetActionV1::Rotate {
            new_nostr_pubkey,
            new_nostr_proof_sig_hex,
            derivation_version,
            ..
        } => {
            if !matches!(derivation_version, 1 | 2) {
                return Err("derivation_version must be 1 or 2".to_string());
            }
            if !is_lower_hex_len(new_nostr_pubkey, IDENTITY_NOSTR_PUBKEY_HEX_LEN) {
                return Err("new_nostr_pubkey must be 64 lowercase hex chars".to_string());
            }
            if !is_lower_hex_len(new_nostr_proof_sig_hex, IDENTITY_BIP340_SIG_HEX_LEN) {
                return Err("new_nostr_proof_sig_hex must be 128 lowercase hex chars".to_string());
            }
        }
        IdentitySetActionV1::RotateAccount {
            new_account,
            new_account_pubkey_hex,
            new_account_sig_hex,
        } => {
            validate_canonical_pf_account("rotate_account.new_account", new_account)?;
            if new_account == &op.account {
                return Err("rotate_account.new_account must differ from current account".into());
            }
            let pk = hex_to_bytes(new_account_pubkey_hex)
                .map_err(|e| format!("invalid new_account_pubkey_hex: {e}"))?;
            if address_from_public_key(&pk) != *new_account {
                return Err("new_account_pubkey_hex does not derive to new_account".to_string());
            }
            if new_account_sig_hex.is_empty() {
                return Err("new_account_sig_hex must be non-empty".to_string());
            }
        }
        IdentitySetActionV1::Revoke { reason_code } => {
            if reason_code.trim().is_empty() || reason_code.len() > 64 {
                return Err("revoke reason_code must be 1..=64 bytes".to_string());
            }
        }
    }
    Ok(())
}

fn validate_handle_attestation(att: &HandleAttestationV1, account: &str) -> Result<(), String> {
    if att.namespace.is_empty() || att.namespace.len() > IDENTITY_NAMESPACE_MAX_LEN {
        return Err("handle attestation namespace invalid".to_string());
    }
    validate_identity_handle(&att.handle)?;
    if att.account != account {
        return Err("handle attestation account does not match identity account".to_string());
    }
    validate_canonical_pf_account(
        "handle_attestation.registrar_account",
        &att.registrar_account,
    )?;
    if att.expires_at_height == 0
        || att.registrar_pubkey_hex.is_empty()
        || att.registrar_signature_hex.is_empty()
    {
        return Err("handle attestation missing required fields".to_string());
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Deterministic domain-separated preimages & commitments
// ---------------------------------------------------------------------------

pub fn identity_nostr_proof_preimage(
    op: &str,
    chain_id: &str,
    account: &str,
    tx_sequence: u64,
) -> Vec<u8> {
    format!(
        "{IDENTITY_NOSTR_PROOF_DOMAIN}\nop={op}\nchain_id={chain_id}\naccount={account}\nsequence={tx_sequence}\n"
    )
    .into_bytes()
}

/// Salted handle reservation commitment (Train B Contract §2.2 & §2.4):
/// `SHA3-384("postfiat.identity.handle_reserve.v1\0" || handle || "\0" || nostr_pubkey_hex || "\0" || salt_bytes)`.
pub fn compute_handle_reservation_commitment(
    handle: &str,
    nostr_pubkey_hex: &str,
    salt_hex: &str,
) -> Result<String, String> {
    validate_identity_handle(handle)?;
    if !is_lower_hex_len(nostr_pubkey_hex, IDENTITY_NOSTR_PUBKEY_HEX_LEN) {
        return Err("nostr_pubkey_hex must be 64 lowercase hex chars".to_string());
    }
    let salt_bytes = hex_to_bytes(salt_hex).map_err(|e| format!("invalid salt_hex: {e}"))?;
    if salt_bytes.is_empty() {
        return Err("reservation salt must be non-empty".to_string());
    }
    let mut hasher = Sha3_384::new();
    hasher.update(IDENTITY_HANDLE_RESERVE_DOMAIN.as_bytes());
    hasher.update([0u8]);
    hasher.update(handle.as_bytes());
    hasher.update([0u8]);
    hasher.update(nostr_pubkey_hex.as_bytes());
    hasher.update([0u8]);
    hasher.update(&salt_bytes);
    Ok(bytes_to_hex(hasher.finalize().as_slice()))
}

pub fn compute_identity_link_id(
    account: &str,
    kind: IdentityLinkKind,
    target: &str,
    identity_sequence: u64,
) -> String {
    let preimage = format!(
        "account={account}\nkind={}\ntarget={target}\nsequence={identity_sequence}\n",
        kind.as_str()
    );
    hash_hex(IDENTITY_LINK_ID_DOMAIN, preimage.as_bytes())
}

pub fn identity_rotate_account_preimage(
    chain_id: &str,
    old_account: &str,
    new_account: &str,
    identity_sequence: u64,
) -> Vec<u8> {
    format!(
        "{IDENTITY_ROTATE_ACCOUNT_DOMAIN}\nchain_id={chain_id}\nold_account={old_account}\nnew_account={new_account}\nidentity_sequence={identity_sequence}\n"
    )
    .into_bytes()
}

pub fn identity_controller_link_preimage(
    chain_id: &str,
    agent_account: &str,
    controller_account: &str,
    tx_sequence: u64,
) -> Vec<u8> {
    format!(
        "{IDENTITY_CONTROLLER_LINK_DOMAIN}\nchain_id={chain_id}\nagent_account={agent_account}\ncontroller_account={controller_account}\nsequence={tx_sequence}\n"
    )
    .into_bytes()
}

pub fn identity_self_unlink_preimage(chain_id: &str, link_id: &str) -> Vec<u8> {
    format!("{IDENTITY_UNLINK_SELF_DOMAIN}\nchain_id={chain_id}\nlink_id={link_id}\n").into_bytes()
}

// ---------------------------------------------------------------------------
// BIP-340 Nostr Schnorr proof sign & verify (pure-Rust `k256::schnorr`)
// ---------------------------------------------------------------------------

pub fn sign_nostr_binding_proof(
    nostr_key: &NostrKeyMaterial,
    op: &str,
    chain_id: &str,
    account: &str,
    tx_sequence: u64,
) -> Result<String, String> {
    if !matches!(op, "mint" | "rotate") {
        return Err(format!("unsupported Nostr proof op `{op}`"));
    }
    if chain_id.is_empty() || tx_sequence == 0 {
        return Err("chain_id must be non-empty and tx_sequence must be nonzero".to_string());
    }
    validate_canonical_pf_account("account", account)?;

    let preimage = identity_nostr_proof_preimage(op, chain_id, account, tx_sequence);
    let signing_key = SchnorrSigningKey::from_bytes(nostr_key.secret_key.as_slice())
        .map_err(|e| format!("invalid Nostr signing key: {e}"))?;
    // Deterministic auxiliary randomness bound to the domain-separated preimage.
    let mut aux_hasher = Sha256::new();
    aux_hasher.update(b"postfiat.bip340.aux_rand.v1\0");
    aux_hasher.update(&preimage);
    let aux_rand: [u8; 32] = aux_hasher.finalize().into();

    let sig = signing_key
        .sign_raw(&preimage, &aux_rand)
        .map_err(|e| format!("BIP-340 Schnorr sign failed: {e}"))?;
    let sig_hex = bytes_to_hex(sig.to_bytes().as_slice());

    if !verify_nostr_binding_proof(
        &nostr_key.pubkey_xonly_hex,
        &sig_hex,
        op,
        chain_id,
        account,
        tx_sequence,
    )? {
        return Err("BIP-340 Schnorr proof self-verification failed".to_string());
    }
    Ok(sig_hex)
}

pub fn verify_nostr_binding_proof(
    nostr_pubkey_hex: &str,
    nostr_proof_sig_hex: &str,
    op: &str,
    chain_id: &str,
    account: &str,
    tx_sequence: u64,
) -> Result<bool, String> {
    if !is_lower_hex_len(nostr_pubkey_hex, IDENTITY_NOSTR_PUBKEY_HEX_LEN) {
        return Err("nostr_pubkey_hex must be 64 lowercase hex chars".to_string());
    }
    if !is_lower_hex_len(nostr_proof_sig_hex, IDENTITY_BIP340_SIG_HEX_LEN) {
        return Err("nostr_proof_sig_hex must be 128 lowercase hex chars".to_string());
    }
    let pk_bytes = hex_to_bytes(nostr_pubkey_hex).map_err(|e| e.to_string())?;
    let sig_bytes = hex_to_bytes(nostr_proof_sig_hex).map_err(|e| e.to_string())?;
    let verifying_key = VerifyingKey::from_bytes(&pk_bytes)
        .map_err(|e| format!("invalid BIP-340 x-only pubkey: {e}"))?;
    let signature = SchnorrSignature::try_from(sig_bytes.as_slice())
        .map_err(|e| format!("invalid BIP-340 signature: {e}"))?;
    let preimage = identity_nostr_proof_preimage(op, chain_id, account, tx_sequence);
    Ok(verifying_key.verify_raw(&preimage, &signature).is_ok())
}

// ---------------------------------------------------------------------------
// Dual-key (ML-DSA-65 + BIP-340 Schnorr) `identity_set` signers & verifiers
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityMintParams {
    pub tx_sequence: u64,
    pub key_index: u32,
    pub identity_kind: IdentityKind,
    #[serde(default)]
    pub handle_attestation: Option<HandleAttestationV1>,
    #[serde(default)]
    pub reservation_salt_hex: Option<String>,
    #[serde(default)]
    pub relays: Vec<String>,
    #[serde(default)]
    pub avatar_nft_id: Option<String>,
    #[serde(default)]
    pub message_key: Option<String>,
    #[serde(default)]
    pub pq_receive_address: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityRotateParams {
    pub tx_sequence: u64,
    pub new_key_index: u32,
    pub mode: IdentityRotateMode,
}

fn sign_dual_key_bundle(
    backup: &WalletBackupFile,
    nostr_key: &NostrKeyMaterial,
    op_kind: &str,
    tx_sequence: u64,
    operation: IdentitySetOperationPayload,
    nostr_proof_sig_hex: String,
) -> Result<SignedIdentitySetBundle, String> {
    validate_identity_set_operation(&operation)?;
    let kp = derive_wallet_key_pair(backup).map_err(|e| e.to_string())?;
    let account = address_from_public_key(&kp.public_key);
    if operation.account != account {
        return Err("identity_set account does not match wallet backup address".to_string());
    }
    let op_bytes = serde_json::to_vec(&operation).map_err(|e| e.to_string())?;
    let mut binding_preimage = Vec::with_capacity(op_bytes.len() + 128);
    binding_preimage.extend_from_slice(IDENTITY_SET_DUAL_PROOF_CONTEXT);
    binding_preimage.push(0u8);
    binding_preimage.extend_from_slice(op_kind.as_bytes());
    binding_preimage.push(0u8);
    binding_preimage.extend_from_slice(backup.chain_id.as_bytes());
    binding_preimage.push(0u8);
    binding_preimage.extend_from_slice(&tx_sequence.to_be_bytes());
    binding_preimage.extend_from_slice(&op_bytes);

    let ml_dsa_sig = ml_dsa_65_sign_with_context(
        &kp.private_key,
        &binding_preimage,
        IDENTITY_SET_DUAL_PROOF_CONTEXT,
    )
    .map_err(|e| format!("ML-DSA-65 identity binding sign failed: {e}"))?;
    if !ml_dsa_65_verify_with_context(
        &kp.public_key,
        &binding_preimage,
        &ml_dsa_sig,
        IDENTITY_SET_DUAL_PROOF_CONTEXT,
    ) {
        return Err("ML-DSA-65 identity binding self-verification failed".to_string());
    }

    Ok(SignedIdentitySetBundle {
        schema: "postfiat-identity-set-dual-proof-v1".to_string(),
        chain_id: backup.chain_id.clone(),
        account,
        tx_sequence,
        nostr_pubkey_hex: nostr_key.pubkey_xonly_hex.clone(),
        npub: nostr_key.npub.clone(),
        nostr_proof_sig_hex,
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        ml_dsa_public_key_hex: bytes_to_hex(&kp.public_key),
        ml_dsa_binding_sig_hex: bytes_to_hex(&ml_dsa_sig),
        operation,
    })
}

/// Sign a Pillar 6 `identity_set` `mint` dual-key bundle using unified v2 BIP-39 Nostr derivation.
pub fn sign_identity_set_mint_v2(
    backup: &WalletBackupFile,
    bip39_seed64_hex: &str,
    params: IdentityMintParams,
) -> Result<SignedIdentitySetBundle, String> {
    let seed64 = decode_bip39_seed64_hex(bip39_seed64_hex)?;
    let nostr_key =
        derive_nostr_v2_keypair(seed64.as_slice(), backup.account_index, params.key_index)?;
    sign_identity_set_mint_with_nostr_key(backup, &nostr_key, params)
}

/// Sign a Pillar 6 `identity_set` `mint` dual-key bundle preserving a legacy v1 Nostr secret key.
pub fn sign_identity_set_mint_v1(
    backup: &WalletBackupFile,
    nostr_v1_secret_hex: &str,
    params: IdentityMintParams,
) -> Result<SignedIdentitySetBundle, String> {
    let nostr_key = nostr_v1_keypair_from_secret_hex(nostr_v1_secret_hex, params.key_index)?;
    sign_identity_set_mint_with_nostr_key(backup, &nostr_key, params)
}

fn sign_identity_set_mint_with_nostr_key(
    backup: &WalletBackupFile,
    nostr_key: &NostrKeyMaterial,
    params: IdentityMintParams,
) -> Result<SignedIdentitySetBundle, String> {
    let identity = wallet_identity_from_backup(backup).map_err(|e| e.to_string())?;
    let nostr_proof_sig_hex = sign_nostr_binding_proof(
        nostr_key,
        "mint",
        &backup.chain_id,
        &identity.address,
        params.tx_sequence,
    )?;
    let operation = IdentitySetOperationPayload {
        operation: IDENTITY_SET_TRANSACTION_KIND.to_string(),
        account: identity.address,
        payload: IdentitySetActionV1::Mint {
            nostr_pubkey: nostr_key.pubkey_xonly_hex.clone(),
            nostr_proof_sig_hex: nostr_proof_sig_hex.clone(),
            derivation_version: nostr_key.derivation_version,
            key_index: params.key_index,
            identity_kind: params.identity_kind,
            handle_attestation: params.handle_attestation,
            reservation_salt_hex: params.reservation_salt_hex,
            relays: params.relays,
            avatar_nft_id: params.avatar_nft_id,
            message_key: params.message_key,
            pq_receive_address: params.pq_receive_address,
        },
    };
    sign_dual_key_bundle(
        backup,
        nostr_key,
        "mint",
        params.tx_sequence,
        operation,
        nostr_proof_sig_hex,
    )
}

/// Sign a Pillar 6 `identity_set` `rotate` dual-key bundle to a new unified v2 Nostr key.
pub fn sign_identity_set_rotate_v2(
    backup: &WalletBackupFile,
    bip39_seed64_hex: &str,
    params: IdentityRotateParams,
) -> Result<SignedIdentitySetBundle, String> {
    let seed64 = decode_bip39_seed64_hex(bip39_seed64_hex)?;
    let nostr_key = derive_nostr_v2_keypair(
        seed64.as_slice(),
        backup.account_index,
        params.new_key_index,
    )?;
    let identity = wallet_identity_from_backup(backup).map_err(|e| e.to_string())?;
    let nostr_proof_sig_hex = sign_nostr_binding_proof(
        &nostr_key,
        "rotate",
        &backup.chain_id,
        &identity.address,
        params.tx_sequence,
    )?;
    let operation = IdentitySetOperationPayload {
        operation: IDENTITY_SET_TRANSACTION_KIND.to_string(),
        account: identity.address,
        payload: IdentitySetActionV1::Rotate {
            mode: params.mode,
            new_nostr_pubkey: nostr_key.pubkey_xonly_hex.clone(),
            new_nostr_proof_sig_hex: nostr_proof_sig_hex.clone(),
            derivation_version: 2,
            key_index: params.new_key_index,
        },
    };
    sign_dual_key_bundle(
        backup,
        &nostr_key,
        "rotate",
        params.tx_sequence,
        operation,
        nostr_proof_sig_hex,
    )
}

/// Verify BOTH the ML-DSA-65 account binding signature AND the BIP-340 Nostr Schnorr proof.
pub fn verify_signed_identity_set_bundle(bundle: &SignedIdentitySetBundle) -> Result<bool, String> {
    validate_identity_set_operation(&bundle.operation)?;
    let (op_kind, op_nostr_pk, op_nostr_sig) = match &bundle.operation.payload {
        IdentitySetActionV1::Mint {
            nostr_pubkey,
            nostr_proof_sig_hex,
            ..
        } => ("mint", nostr_pubkey.as_str(), nostr_proof_sig_hex.as_str()),
        IdentitySetActionV1::Rotate {
            new_nostr_pubkey,
            new_nostr_proof_sig_hex,
            ..
        } => (
            "rotate",
            new_nostr_pubkey.as_str(),
            new_nostr_proof_sig_hex.as_str(),
        ),
        _ => return Err("dual-key bundle requires mint or rotate payload".to_string()),
    };
    if bundle.nostr_pubkey_hex != op_nostr_pk || bundle.nostr_proof_sig_hex != op_nostr_sig {
        return Err("bundle nostr fields do not match operation payload".to_string());
    }
    if encode_npub(&bundle.nostr_pubkey_hex)? != bundle.npub {
        return Err("bundle npub does not match nostr_pubkey_hex".to_string());
    }
    let pk = hex_to_bytes(&bundle.ml_dsa_public_key_hex).map_err(|e| e.to_string())?;
    if address_from_public_key(&pk) != bundle.account || bundle.operation.account != bundle.account
    {
        return Err("ml_dsa_public_key_hex does not derive to bundle account".to_string());
    }
    let sig = hex_to_bytes(&bundle.ml_dsa_binding_sig_hex).map_err(|e| e.to_string())?;
    let op_bytes = serde_json::to_vec(&bundle.operation).map_err(|e| e.to_string())?;
    let mut binding_preimage = Vec::with_capacity(op_bytes.len() + 128);
    binding_preimage.extend_from_slice(IDENTITY_SET_DUAL_PROOF_CONTEXT);
    binding_preimage.push(0u8);
    binding_preimage.extend_from_slice(op_kind.as_bytes());
    binding_preimage.push(0u8);
    binding_preimage.extend_from_slice(bundle.chain_id.as_bytes());
    binding_preimage.push(0u8);
    binding_preimage.extend_from_slice(&bundle.tx_sequence.to_be_bytes());
    binding_preimage.extend_from_slice(&op_bytes);

    let ml_dsa_ok = ml_dsa_65_verify_with_context(
        &pk,
        &binding_preimage,
        &sig,
        IDENTITY_SET_DUAL_PROOF_CONTEXT,
    );
    let schnorr_ok = verify_nostr_binding_proof(
        &bundle.nostr_pubkey_hex,
        &bundle.nostr_proof_sig_hex,
        op_kind,
        &bundle.chain_id,
        &bundle.account,
        bundle.tx_sequence,
    )?;
    Ok(ml_dsa_ok && schnorr_ok)
}

/// Sign destination-account ML-DSA-65 co-signature for `identity_set` `rotate_account`.
pub fn sign_identity_rotate_account_cosig(
    new_account_backup: &WalletBackupFile,
    old_account: &str,
    identity_sequence: u64,
) -> Result<IdentityCosignatureProof, String> {
    validate_canonical_pf_account("old_account", old_account)?;
    if identity_sequence == 0 {
        return Err("identity_sequence must be nonzero".to_string());
    }
    let kp = derive_wallet_key_pair(new_account_backup).map_err(|e| e.to_string())?;
    let new_account = address_from_public_key(&kp.public_key);
    if new_account == old_account {
        return Err("new_account must differ from old_account".to_string());
    }
    let preimage = identity_rotate_account_preimage(
        &new_account_backup.chain_id,
        old_account,
        &new_account,
        identity_sequence,
    );
    let sig = ml_dsa_65_sign_with_context(
        &kp.private_key,
        &preimage,
        IDENTITY_ROTATE_ACCOUNT_DOMAIN.as_bytes(),
    )
    .map_err(|e| format!("rotate_account cosig sign failed: {e}"))?;
    Ok(IdentityCosignatureProof {
        domain: IDENTITY_ROTATE_ACCOUNT_DOMAIN.to_string(),
        signer_account: new_account,
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        public_key_hex: bytes_to_hex(&kp.public_key),
        signature_hex: bytes_to_hex(&sig),
    })
}

/// Sign human-controller ML-DSA-65 co-signature for an `agent_controller` identity link.
pub fn sign_identity_controller_link_proof(
    controller_backup: &WalletBackupFile,
    agent_account: &str,
    tx_sequence: u64,
) -> Result<IdentityCosignatureProof, String> {
    validate_canonical_pf_account("agent_account", agent_account)?;
    if tx_sequence == 0 {
        return Err("tx_sequence must be nonzero".to_string());
    }
    let kp = derive_wallet_key_pair(controller_backup).map_err(|e| e.to_string())?;
    let controller_account = address_from_public_key(&kp.public_key);
    let preimage = identity_controller_link_preimage(
        &controller_backup.chain_id,
        agent_account,
        &controller_account,
        tx_sequence,
    );
    let sig = ml_dsa_65_sign_with_context(
        &kp.private_key,
        &preimage,
        IDENTITY_CONTROLLER_LINK_DOMAIN.as_bytes(),
    )
    .map_err(|e| format!("controller link proof sign failed: {e}"))?;
    Ok(IdentityCosignatureProof {
        domain: IDENTITY_CONTROLLER_LINK_DOMAIN.to_string(),
        signer_account: controller_account,
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        public_key_hex: bytes_to_hex(&kp.public_key),
        signature_hex: bytes_to_hex(&sig),
    })
}

/// Sign linked-key ML-DSA-65 self-unlink proof committing strictly to `link_id` (not owner seq).
pub fn sign_identity_self_unlink_proof(
    linked_backup: &WalletBackupFile,
    link_id: &str,
) -> Result<IdentityCosignatureProof, String> {
    if !is_lower_hex_len(link_id, 96) {
        return Err("link_id must be 96 lowercase hex chars".to_string());
    }
    let kp = derive_wallet_key_pair(linked_backup).map_err(|e| e.to_string())?;
    let signer_account = address_from_public_key(&kp.public_key);
    let preimage = identity_self_unlink_preimage(&linked_backup.chain_id, link_id);
    let sig = ml_dsa_65_sign_with_context(
        &kp.private_key,
        &preimage,
        IDENTITY_UNLINK_SELF_DOMAIN.as_bytes(),
    )
    .map_err(|e| format!("self-unlink proof sign failed: {e}"))?;
    Ok(IdentityCosignatureProof {
        domain: IDENTITY_UNLINK_SELF_DOMAIN.to_string(),
        signer_account,
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        public_key_hex: bytes_to_hex(&kp.public_key),
        signature_hex: bytes_to_hex(&sig),
    })
}
