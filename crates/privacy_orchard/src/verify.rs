use incrementalmerkletree::{frontier::Frontier, Hashable, Level, Position};
use nonempty::NonEmpty;
use orchard::{
    builder::{Builder, BundleType},
    bundle::{Authorization, Authorized, Bundle, ProofSizeEnforcement},
    circuit::{ProvingKey, VerifyingKey},
    keys::{FullViewingKey, PreparedIncomingViewingKey, Scope, SpendAuthorizingKey, SpendingKey},
    note::{RandomSeed, Rho, TransmittedNoteCiphertext},
    note_encryption::OrchardDomain,
    tree::{MerkleHashOrchard, MerklePath},
    value::NoteValue,
    Action, Address, Note, Proof,
};
use postfiat_crypto_provider::{bytes_to_hex, hash_bytes, hex_to_bytes};
use rand::{
    rngs::{OsRng, StdRng},
    CryptoRng, RngCore, SeedableRng,
};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};
use zcash_note_encryption::{
    try_note_decryption, EphemeralKeyBytes, ShieldedOutput, ENC_CIPHERTEXT_SIZE,
};
use zeroize::Zeroizing;
use zip32::AccountId;

use crate::timing::{
    asset_orchard_timing_elapsed_ms, record_asset_orchard_private_egress_action_verify_timing,
    record_asset_orchard_swap_proof_verify_timing,
    AssetOrchardPrivateEgressActionVerifyTimingReport, AssetOrchardSwapProofVerifyTimingReport,
};
use crate::{
    asset_orchard_private_egress_exit_binding_hash, AssetOrchardBoundedBytes, AssetOrchardError,
    AssetOrchardFieldElement, AssetOrchardPoint, AssetOrchardPricingClaim,
    AssetOrchardPrivateEgressAction, AssetOrchardPrivateEgressExitBindingPreimage,
    AssetOrchardPrivateEgressVerifyingKey, AssetOrchardSwapAccountingRecord,
    AssetOrchardSwapAction, AssetOrchardSwapBindingHash, AssetOrchardSwapVerifyingKey, AssetTag,
    BoundedHexBlob, EncryptedShieldedOutput, OrchardAnchor, OrchardBindingSignature,
    OrchardCircuitId, OrchardFlags, OrchardNullifier, OrchardOutputCommitment, OrchardProofBytes,
    OrchardProofSystemId, OrchardRandomizedVerificationKey, OrchardShieldedAction,
    OrchardSpendAuthSignature, OrchardTypeError, OrchardValueCommitment, ShieldedSwapAction,
    ShieldedSwapCommitment, ASSET_ORCHARD_CIRCUIT_ID_V1, ASSET_ORCHARD_POOL_ID_V1,
    ASSET_ORCHARD_PRIVATE_EGRESS_CIRCUIT_ID_V1, ASSET_ORCHARD_PROOF_SYSTEM_ID_V1,
    ORCHARD_COMPACT_CIPHERTEXT_BYTES, ORCHARD_EXTERNAL_BINDING_HASH_BYTES, ORCHARD_NULLIFIER_BYTES,
    SHIELDED_SWAP_ACTION_SCHEMA, SHIELDED_SWAP_CIRCUIT_ID,
    SHIELDED_SWAP_LEGACY_TRANSCRIPT_HASH_BYTES, SHIELDED_SWAP_LEG_COUNT,
    SHIELDED_SWAP_PROOF_SYSTEM_ID,
};

const ORCHARD_TREE_DEPTH: u8 = orchard::NOTE_COMMITMENT_TREE_DEPTH as u8;

pub const DEFAULT_MAX_ORCHARD_ACTIONS: usize = 8;
pub const ORCHARD_AUTHORIZING_SIGHASH_DOMAIN: &str =
    "postfiat.privacy.orchard-authorizing-sighash.v1";
pub const ORCHARD_MEMO_BYTES: usize = 512;
pub const ORCHARD_RAW_ADDRESS_BYTES: usize = 43;
pub const POSTFIAT_ORCHARD_COIN_TYPE: u32 = 1;
const MAX_DOMAIN_TEXT_BYTES: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrchardFrontierSnapshot {
    pub output_count: u64,
    pub root: String,
    pub latest_leaf: Option<String>,
    pub ommers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrchardAuthorizingDomain {
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub pool_id: String,
}

impl OrchardAuthorizingDomain {
    pub fn new(
        chain_id: impl Into<String>,
        genesis_hash: impl Into<String>,
        protocol_version: u32,
        pool_id: impl Into<String>,
    ) -> Result<Self, OrchardVerificationError> {
        let domain = Self {
            chain_id: chain_id.into(),
            genesis_hash: genesis_hash.into(),
            protocol_version,
            pool_id: pool_id.into(),
        };
        domain.validate()?;
        Ok(domain)
    }

    pub fn validate(&self) -> Result<(), OrchardVerificationError> {
        validate_domain_text("chain_id", &self.chain_id)?;
        validate_lower_hex_len("genesis_hash", &self.genesis_hash, 96)?;
        if self.protocol_version == 0 {
            return Err(OrchardVerificationError::new(
                "invalid_protocol_version",
                "protocol_version must be nonzero",
            ));
        }
        validate_domain_text("pool_id", &self.pool_id)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrchardVerificationContext {
    pub proof_system_id: OrchardProofSystemId,
    pub circuit_id: OrchardCircuitId,
    pub max_actions: usize,
    pub authorizing_sighash: [u8; 32],
}

impl OrchardVerificationContext {
    pub fn production_v2(authorizing_sighash: [u8; 32]) -> Self {
        Self {
            proof_system_id: OrchardProofSystemId::production_v2(),
            circuit_id: OrchardCircuitId::action_v2(),
            max_actions: DEFAULT_MAX_ORCHARD_ACTIONS,
            authorizing_sighash,
        }
    }

    pub fn for_bundle<T, V>(
        domain: &OrchardAuthorizingDomain,
        fee: u64,
        bundle: &Bundle<T, V>,
    ) -> Result<Self, OrchardVerificationError>
    where
        T: Authorization,
        V: Copy + Into<i64>,
    {
        Self::for_bundle_with_external_binding(domain, fee, None, bundle)
    }

    pub fn for_bundle_with_external_binding<T, V>(
        domain: &OrchardAuthorizingDomain,
        fee: u64,
        external_binding_hash: Option<&str>,
        bundle: &Bundle<T, V>,
    ) -> Result<Self, OrchardVerificationError>
    where
        T: Authorization,
        V: Copy + Into<i64>,
    {
        Ok(Self::production_v2(
            orchard_authorizing_sighash_with_external_binding(
                domain,
                fee,
                external_binding_hash,
                bundle,
            )?,
        ))
    }

    pub fn validate(&self) -> Result<(), OrchardVerificationError> {
        if self.proof_system_id != OrchardProofSystemId::production_v2() {
            return Err(OrchardVerificationError::new(
                "unsupported_proof_system",
                "verification context is not configured for Orchard/Halo2 v2",
            ));
        }
        if self.circuit_id != OrchardCircuitId::action_v2() {
            return Err(OrchardVerificationError::new(
                "unsupported_circuit",
                "verification context is not configured for Orchard action v2",
            ));
        }
        if self.max_actions == 0 {
            return Err(OrchardVerificationError::new(
                "invalid_action_bound",
                "max_actions must be greater than zero",
            ));
        }
        Ok(())
    }
}

pub fn orchard_authorizing_sighash<T, V>(
    domain: &OrchardAuthorizingDomain,
    fee: u64,
    bundle: &Bundle<T, V>,
) -> Result<[u8; 32], OrchardVerificationError>
where
    T: Authorization,
    V: Copy + Into<i64>,
{
    orchard_authorizing_sighash_with_external_binding(domain, fee, None, bundle)
}

pub fn orchard_authorizing_sighash_with_external_binding<T, V>(
    domain: &OrchardAuthorizingDomain,
    fee: u64,
    external_binding_hash: Option<&str>,
    bundle: &Bundle<T, V>,
) -> Result<[u8; 32], OrchardVerificationError>
where
    T: Authorization,
    V: Copy + Into<i64>,
{
    domain.validate()?;

    let action_count = u32::try_from(bundle.actions().iter().count()).map_err(|_| {
        OrchardVerificationError::new(
            "too_many_actions",
            "Orchard bundle action count does not fit canonical sighash encoding",
        )
    })?;
    let mut payload = Vec::new();
    append_str_field(&mut payload, "schema", ORCHARD_AUTHORIZING_SIGHASH_DOMAIN);
    append_str_field(&mut payload, "chain_id", &domain.chain_id);
    append_str_field(&mut payload, "genesis_hash", &domain.genesis_hash);
    append_u32_field(&mut payload, "protocol_version", domain.protocol_version);
    append_str_field(&mut payload, "pool_id", &domain.pool_id);
    append_str_field(
        &mut payload,
        "proof_system_id",
        OrchardProofSystemId::production_v2().as_str(),
    );
    append_str_field(
        &mut payload,
        "circuit_id",
        OrchardCircuitId::action_v2().as_str(),
    );
    append_u8_field(&mut payload, "flags", bundle.flags().to_byte());
    append_bytes_field(&mut payload, "anchor", &bundle.anchor().to_bytes());
    append_i64_field(
        &mut payload,
        "value_balance",
        (*bundle.value_balance()).into(),
    );
    append_u64_field(&mut payload, "fee", fee);
    append_external_binding_hash_field(&mut payload, external_binding_hash)?;
    append_u32_field(&mut payload, "action_count", action_count);

    for (index, action) in bundle.actions().iter().enumerate() {
        append_u32_field(&mut payload, "action_index", index as u32);
        append_bytes_field(
            &mut payload,
            "action.nullifier",
            &action.nullifier().to_bytes(),
        );
        let rk_bytes: [u8; 32] = action.rk().into();
        append_bytes_field(&mut payload, "action.rk", &rk_bytes);
        append_bytes_field(&mut payload, "action.cmx", &action.cmx().to_bytes());
        append_bytes_field(&mut payload, "action.cv_net", &action.cv_net().to_bytes());
        let encrypted_note = action.encrypted_note();
        append_bytes_field(&mut payload, "action.epk", &encrypted_note.epk_bytes);
        append_bytes_field(
            &mut payload,
            "action.enc_ciphertext",
            &encrypted_note.enc_ciphertext,
        );
        append_bytes_field(
            &mut payload,
            "action.out_ciphertext",
            &encrypted_note.out_ciphertext,
        );
    }

    let digest = hash_bytes(ORCHARD_AUTHORIZING_SIGHASH_DOMAIN, &payload);
    let mut sighash = [0u8; 32];
    sighash.copy_from_slice(&digest[..32]);
    Ok(sighash)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedOrchardBundle {
    pub proof_system_id: OrchardProofSystemId,
    pub circuit_id: OrchardCircuitId,
    pub flags: OrchardFlags,
    pub anchor: OrchardAnchor,
    pub action_count: usize,
    pub nullifiers: Vec<OrchardNullifier>,
    pub randomized_verification_keys: Vec<OrchardRandomizedVerificationKey>,
    pub value_commitments: Vec<OrchardValueCommitment>,
    pub output_commitments: Vec<OrchardOutputCommitment>,
    pub encrypted_outputs: Vec<EncryptedShieldedOutput>,
    pub value_balance: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchardDecryptedOutput {
    pub output_index: usize,
    pub commitment: String,
    pub nullifier: String,
    pub rho: String,
    pub rseed: String,
    pub value: u64,
    pub address_raw_hex: String,
    pub memo_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchardScanReport {
    pub total_output_count: usize,
    pub decrypted_count: usize,
    pub non_matching_count: usize,
    pub malformed_count: usize,
    pub decrypted_outputs: Vec<OrchardDecryptedOutput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchardMerkleWitness {
    pub position: u32,
    pub anchor: String,
    pub output_count: u64,
    pub auth_path: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchardSpendNote {
    pub output_index: usize,
    pub commitment: String,
    pub address_raw_hex: String,
    pub value: u64,
    pub rho: String,
    pub rseed: String,
    pub merkle_position: u32,
    pub witness_anchor: String,
    pub witness_auth_path: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShieldedSwapPrivateInput {
    pub asset_id: String,
    pub value: u64,
    pub asset_blinding: String,
    pub value_blinding: String,
    pub authorization_secret: String,
    pub authorization_proof: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShieldedSwapPrivateOutput {
    pub asset_id: String,
    pub value: u64,
    pub asset_blinding: String,
    pub value_blinding: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedShieldedSwap {
    pub proof_system_id: String,
    pub circuit_id: String,
    pub anchor: OrchardAnchor,
    pub nullifiers: Vec<OrchardNullifier>,
    pub output_commitments: Vec<OrchardOutputCommitment>,
    pub output_asset_commitments: Vec<ShieldedSwapCommitment>,
    pub output_value_commitments: Vec<ShieldedSwapCommitment>,
    pub encrypted_outputs: Vec<EncryptedShieldedOutput>,
    pub swap_binding_hash: ShieldedSwapCommitment,
    pub fee: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedAssetOrchardSwap {
    pub proof_system_id: String,
    pub circuit_id: String,
    pub pool_domain: AssetOrchardFieldElement,
    pub anchor: AssetOrchardFieldElement,
    pub nullifiers: Vec<AssetOrchardFieldElement>,
    pub randomized_verification_keys: Vec<AssetOrchardPoint>,
    pub output_commitments: Vec<AssetOrchardFieldElement>,
    pub encrypted_outputs: Vec<AssetOrchardBoundedBytes>,
    pub accounting_inputs: Vec<AssetOrchardSwapAccountingRecord>,
    pub accounting_outputs: Vec<AssetOrchardSwapAccountingRecord>,
    pub pricing: VerifiedAssetOrchardPricingClaim,
    pub swap_binding_hash: AssetOrchardSwapBindingHash,
    pub fee: u64,
}

/// Provenance-neutral input to validator pricing policy. B-interim populates
/// `DualSigned`; B-final is emitted only after the Halo2 proof verifies.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum AssetOrchardPricingClaimProvenance {
    DualSigned,
    CircuitProven,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedAssetOrchardPricingClaim {
    pub claim: AssetOrchardPricingClaim,
    pub action_binding_hash: AssetOrchardSwapBindingHash,
    pub provenance: AssetOrchardPricingClaimProvenance,
}

pub trait AssetOrchardPricingClaimEvidence {
    fn claim(&self) -> &AssetOrchardPricingClaim;
    fn action_binding_hash(&self) -> &AssetOrchardSwapBindingHash;
    fn provenance(&self) -> AssetOrchardPricingClaimProvenance;
}

impl AssetOrchardPricingClaimEvidence for VerifiedAssetOrchardPricingClaim {
    fn claim(&self) -> &AssetOrchardPricingClaim {
        &self.claim
    }
    fn action_binding_hash(&self) -> &AssetOrchardSwapBindingHash {
        &self.action_binding_hash
    }
    fn provenance(&self) -> AssetOrchardPricingClaimProvenance {
        self.provenance
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetOrchardPricingPolicy {
    pub nav_epoch: u64,
    pub reserve_packet_hash: String,
    pub nav_ratio_numerator: u64,
    pub nav_ratio_denominator: u64,
    pub band_bps: u16,
    pub base_asset_tag: AssetTag,
    pub quote_asset_tag: AssetTag,
    pub halted: bool,
}

/// Shared deterministic validator policy for attested (B-interim) and proven
/// (B-final) claim provenance. Provenance verification happens before this
/// boundary; pricing semantics remain identical across the upgrade.
pub fn validate_asset_orchard_pricing_policy(
    evidence: &impl AssetOrchardPricingClaimEvidence,
    policy: &AssetOrchardPricingPolicy,
) -> Result<(), OrchardVerificationError> {
    let claim = evidence.claim();
    claim.validate().map_err(OrchardVerificationError::from)?;
    if policy.halted {
        return Err(OrchardVerificationError::new(
            "asset_orchard_pricing_halted",
            "active NAV profile is halted",
        ));
    }
    if claim.nav_epoch != policy.nav_epoch {
        return Err(OrchardVerificationError::new(
            "asset_orchard_pricing_epoch_mismatch",
            "pricing claim epoch does not match finalized NAV epoch",
        ));
    }
    if claim.reserve_packet_hash != policy.reserve_packet_hash {
        return Err(OrchardVerificationError::new(
            "asset_orchard_pricing_packet_mismatch",
            "pricing claim reserve packet does not match finalized NAV packet",
        ));
    }
    if claim.band_bps != policy.band_bps {
        return Err(OrchardVerificationError::new(
            "asset_orchard_pricing_band_mismatch",
            "pricing claim band does not match active validator policy",
        ));
    }
    if (claim.base_asset_tag_lo, claim.base_asset_tag_hi)
        != (policy.base_asset_tag.lo, policy.base_asset_tag.hi)
        || (claim.quote_asset_tag_lo, claim.quote_asset_tag_hi)
            != (policy.quote_asset_tag.lo, policy.quote_asset_tag.hi)
    {
        return Err(OrchardVerificationError::new(
            "asset_orchard_pricing_pair_mismatch",
            "pricing claim asset tags do not match active validator policy",
        ));
    }
    if policy.nav_ratio_numerator == 0 || policy.nav_ratio_denominator == 0 {
        return Err(OrchardVerificationError::new(
            "invalid_asset_orchard_nav_ratio",
            "active NAV ratio terms must be nonzero",
        ));
    }
    let claim_scaled = u128::from(claim.ratio_numerator)
        .checked_mul(u128::from(policy.nav_ratio_denominator))
        .ok_or_else(|| {
            OrchardVerificationError::new(
                "asset_orchard_pricing_overflow",
                "claim ratio cross-product overflow",
            )
        })?;
    let nav_scaled = u128::from(policy.nav_ratio_numerator)
        .checked_mul(u128::from(claim.ratio_denominator))
        .ok_or_else(|| {
            OrchardVerificationError::new(
                "asset_orchard_pricing_overflow",
                "NAV ratio cross-product overflow",
            )
        })?;
    let deviation = claim_scaled.abs_diff(nav_scaled);
    let deviation_bps = deviation.checked_mul(10_000).ok_or_else(|| {
        OrchardVerificationError::new(
            "asset_orchard_pricing_overflow",
            "pricing deviation calculation overflow",
        )
    })?;
    let permitted = nav_scaled
        .checked_mul(u128::from(policy.band_bps))
        .ok_or_else(|| {
            OrchardVerificationError::new(
                "asset_orchard_pricing_overflow",
                "pricing band calculation overflow",
            )
        })?;
    if deviation_bps > permitted {
        return Err(OrchardVerificationError::new(
            "asset_orchard_pricing_off_band",
            "private swap ratio is outside the active NAV band",
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedAssetOrchardPrivateEgress {
    pub proof_system_id: String,
    pub circuit_id: String,
    pub pool_domain: AssetOrchardFieldElement,
    pub anchor: AssetOrchardFieldElement,
    pub nullifier: AssetOrchardFieldElement,
    pub randomized_verification_key: AssetOrchardPoint,
    pub asset_tag_lo: u128,
    pub asset_tag_hi: u128,
    pub amount: u64,
    pub fee: u64,
    pub exit_binding_hash: AssetOrchardSwapBindingHash,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrchardVerificationError {
    code: &'static str,
    message: String,
}

impl OrchardVerificationError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn code(&self) -> &'static str {
        self.code
    }
}

impl std::fmt::Display for OrchardVerificationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for OrchardVerificationError {}

impl From<OrchardTypeError> for OrchardVerificationError {
    fn from(error: OrchardTypeError) -> Self {
        Self::new(error.code(), error.to_string())
    }
}

impl From<AssetOrchardError> for OrchardVerificationError {
    fn from(error: AssetOrchardError) -> Self {
        Self::new(error.code(), error.to_string())
    }
}

pub fn shielded_swap_asset_commitment(
    asset_id: &str,
    blinding: &str,
) -> Result<ShieldedSwapCommitment, OrchardVerificationError> {
    validate_domain_text("asset_id", asset_id)?;
    validate_domain_text("asset_blinding", blinding)?;
    let mut payload = Vec::new();
    append_str_field(&mut payload, "asset_id", asset_id);
    append_str_field(&mut payload, "blinding", blinding);
    ShieldedSwapCommitment::parse_hex(bytes_to_hex(&hash_bytes(
        "postfiat.privacy.shielded-swap.asset-commitment.v1",
        &payload,
    )))
    .map_err(OrchardVerificationError::from)
}

pub fn shielded_swap_value_commitment(
    value: u64,
    blinding: &str,
) -> Result<ShieldedSwapCommitment, OrchardVerificationError> {
    validate_domain_text("value_blinding", blinding)?;
    let mut payload = Vec::new();
    append_u64_field(&mut payload, "value", value);
    append_str_field(&mut payload, "blinding", blinding);
    ShieldedSwapCommitment::parse_hex(bytes_to_hex(&hash_bytes(
        "postfiat.privacy.shielded-swap.value-commitment.v1",
        &payload,
    )))
    .map_err(OrchardVerificationError::from)
}

pub fn shielded_swap_authorization_proof(
    asset_id: &str,
    value: u64,
    authorization_secret: &str,
) -> Result<String, OrchardVerificationError> {
    validate_domain_text("asset_id", asset_id)?;
    validate_domain_text("authorization_secret", authorization_secret)?;
    let mut payload = Vec::new();
    append_str_field(&mut payload, "asset_id", asset_id);
    append_u64_field(&mut payload, "value", value);
    append_str_field(&mut payload, "authorization_secret", authorization_secret);
    Ok(bytes_to_hex(&hash_bytes(
        "postfiat.privacy.shielded-swap.authorization.v1",
        &payload,
    )))
}

pub fn shielded_swap_authorization_commitment(
    authorization_secret: &str,
) -> Result<ShieldedSwapCommitment, OrchardVerificationError> {
    validate_domain_text("authorization_secret", authorization_secret)?;
    let mut payload = Vec::new();
    append_str_field(&mut payload, "authorization_secret", authorization_secret);
    ShieldedSwapCommitment::parse_hex(bytes_to_hex(&hash_bytes(
        "postfiat.privacy.shielded-swap.authorization-commitment.v1",
        &payload,
    )))
    .map_err(OrchardVerificationError::from)
}

#[allow(clippy::too_many_arguments)]
pub fn shielded_swap_build_action_test_vector(
    domain: &OrchardAuthorizingDomain,
    pool_id: impl Into<String>,
    anchor: OrchardAnchor,
    inputs: [ShieldedSwapPrivateInput; SHIELDED_SWAP_LEG_COUNT],
    outputs: [ShieldedSwapPrivateOutput; SHIELDED_SWAP_LEG_COUNT],
    binding_nonce: &str,
    fee: u64,
) -> Result<ShieldedSwapAction, OrchardVerificationError> {
    domain.validate()?;
    let pool_id = pool_id.into();
    if pool_id != domain.pool_id {
        return Err(OrchardVerificationError::new(
            "pool_id_mismatch",
            format!(
                "shielded swap pool_id `{pool_id}` does not match domain pool_id `{}`",
                domain.pool_id
            ),
        ));
    }
    if fee != 0 {
        return Err(OrchardVerificationError::new(
            "unsupported_shielded_swap_fee",
            "shielded swap v1 requires fee 0 until fee burn accounting is specified",
        ));
    }
    validate_domain_text("binding_nonce", binding_nonce)?;
    validate_swap_private_inputs(&inputs)?;
    validate_swap_conservation(&inputs, &outputs)?;

    let mut input_asset_commitments = Vec::with_capacity(SHIELDED_SWAP_LEG_COUNT);
    let mut input_value_commitments = Vec::with_capacity(SHIELDED_SWAP_LEG_COUNT);
    let mut input_authorization_commitments = Vec::with_capacity(SHIELDED_SWAP_LEG_COUNT);
    let mut nullifiers = Vec::with_capacity(SHIELDED_SWAP_LEG_COUNT);
    for (index, input) in inputs.iter().enumerate() {
        let asset_commitment =
            shielded_swap_asset_commitment(&input.asset_id, &input.asset_blinding)?;
        let value_commitment = shielded_swap_value_commitment(input.value, &input.value_blinding)?;
        let authorization_commitment =
            shielded_swap_authorization_commitment(&input.authorization_secret)?;
        let input_index = index.to_string();
        nullifiers.push(derive_canonical_nullifier(
            "postfiat.privacy.shielded-swap.nullifier.v1",
            &[
                pool_id.as_str(),
                anchor.as_hex(),
                input_index.as_str(),
                asset_commitment.as_hex(),
                value_commitment.as_hex(),
                authorization_commitment.as_hex(),
            ],
        )?);
        input_asset_commitments.push(asset_commitment);
        input_value_commitments.push(value_commitment);
        input_authorization_commitments.push(authorization_commitment);
    }

    let mut output_commitments = Vec::with_capacity(SHIELDED_SWAP_LEG_COUNT);
    let mut output_asset_commitments = Vec::with_capacity(SHIELDED_SWAP_LEG_COUNT);
    let mut output_value_commitments = Vec::with_capacity(SHIELDED_SWAP_LEG_COUNT);
    let mut encrypted_outputs = Vec::with_capacity(SHIELDED_SWAP_LEG_COUNT);
    for (index, output) in outputs.iter().enumerate() {
        let asset_commitment =
            shielded_swap_asset_commitment(&output.asset_id, &output.asset_blinding)?;
        let value_commitment =
            shielded_swap_value_commitment(output.value, &output.value_blinding)?;
        let output_index = index.to_string();
        let output_commitment = derive_canonical_output_commitment(
            "postfiat.privacy.shielded-swap.output-commitment.v1",
            &[
                pool_id.as_str(),
                anchor.as_hex(),
                binding_nonce,
                output_index.as_str(),
                asset_commitment.as_hex(),
                value_commitment.as_hex(),
            ],
        )?;
        let encrypted_output = deterministic_swap_encrypted_output(
            &output_commitment,
            &[
                pool_id.as_str(),
                binding_nonce,
                output_index.as_str(),
                asset_commitment.as_hex(),
                value_commitment.as_hex(),
            ],
        )?;
        output_commitments.push(output_commitment);
        output_asset_commitments.push(asset_commitment);
        output_value_commitments.push(value_commitment);
        encrypted_outputs.push(encrypted_output);
    }

    let swap_binding_hash = shielded_swap_binding_hash_from_parts(
        domain,
        &pool_id,
        anchor.as_hex(),
        binding_nonce,
        &input_asset_commitments,
        &input_value_commitments,
        &input_authorization_commitments,
        &nullifiers,
        &output_commitments,
        &output_asset_commitments,
        &output_value_commitments,
    )?;
    let mut action = ShieldedSwapAction {
        schema: SHIELDED_SWAP_ACTION_SCHEMA.to_string(),
        pool_id,
        proof_system_id: SHIELDED_SWAP_PROOF_SYSTEM_ID.to_string(),
        circuit_id: SHIELDED_SWAP_CIRCUIT_ID.to_string(),
        anchor,
        nullifiers,
        input_asset_commitments,
        input_value_commitments,
        input_authorization_commitments,
        output_commitments,
        output_asset_commitments,
        output_value_commitments,
        encrypted_outputs,
        swap_binding_hash,
        fee,
        proof: BoundedHexBlob::from_bytes(
            &[1u8; SHIELDED_SWAP_LEGACY_TRANSCRIPT_HASH_BYTES],
            SHIELDED_SWAP_LEGACY_TRANSCRIPT_HASH_BYTES,
        )
        .map_err(OrchardVerificationError::from)?,
    };
    let proof_hash = shielded_swap_transcript_hash(domain, &action)?;
    action.proof = BoundedHexBlob::parse_hex(
        "shielded_swap_legacy_transcript_hash",
        proof_hash,
        SHIELDED_SWAP_LEGACY_TRANSCRIPT_HASH_BYTES,
    )
    .map_err(OrchardVerificationError::from)?;
    action.validate().map_err(OrchardVerificationError::from)?;
    Ok(action)
}

pub fn asset_orchard_domain_genesis_hash(
    genesis_hash_hex: &str,
) -> Result<[u8; 32], OrchardVerificationError> {
    validate_lower_hex_len("genesis_hash", genesis_hash_hex, 96)?;
    let genesis_hash_bytes = hex_to_bytes(genesis_hash_hex).map_err(|error| {
        OrchardVerificationError::new(
            "invalid_genesis_hash",
            format!("genesis_hash is not valid hex: {error}"),
        )
    })?;
    if genesis_hash_bytes.len() != 48 {
        return Err(OrchardVerificationError::new(
            "invalid_genesis_hash",
            format!(
                "asset-orchard domain adapter expected 48-byte chain genesis hash, got {} bytes",
                genesis_hash_bytes.len()
            ),
        ));
    }
    let mut hasher = Sha3_256::new();
    hasher.update(b"postfiat.asset_orchard.chain_genesis_32.v1");
    hasher.update((genesis_hash_bytes.len() as u32).to_le_bytes());
    hasher.update(&genesis_hash_bytes);
    hasher.finalize().as_slice().try_into().map_err(|_| {
        OrchardVerificationError::new(
            "invalid_asset_orchard_genesis_hash",
            "asset-orchard genesis adapter produced invalid digest length",
        )
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AssetOrchardVkVerificationPolicy {
    LiveCurrent,
    ArchiveReplay,
}

fn validate_asset_orchard_swap_vk_policy(
    circuit_id: &str,
    policy: AssetOrchardVkVerificationPolicy,
) -> Result<(), OrchardVerificationError> {
    if policy == AssetOrchardVkVerificationPolicy::LiveCurrent
        && circuit_id != ASSET_ORCHARD_CIRCUIT_ID_V1
    {
        return Err(OrchardVerificationError::new(
            "asset_orchard_legacy_circuit_replay_only",
            format!(
                "asset-orchard circuit `{circuit_id}` is replay-only; live verification requires `{ASSET_ORCHARD_CIRCUIT_ID_V1}`",
            ),
        ));
    }
    Ok(())
}

fn validate_asset_orchard_private_egress_vk_policy(
    circuit_id: &str,
    policy: AssetOrchardVkVerificationPolicy,
) -> Result<(), OrchardVerificationError> {
    if policy == AssetOrchardVkVerificationPolicy::LiveCurrent
        && circuit_id != ASSET_ORCHARD_PRIVATE_EGRESS_CIRCUIT_ID_V1
    {
        return Err(OrchardVerificationError::new(
            "asset_orchard_private_egress_legacy_circuit_replay_only",
            format!(
                "asset-orchard private egress circuit `{circuit_id}` is replay-only; live verification requires `{ASSET_ORCHARD_PRIVATE_EGRESS_CIRCUIT_ID_V1}`",
            ),
        ));
    }
    Ok(())
}

pub fn verify_serialized_asset_orchard_swap_action(
    action: &AssetOrchardSwapAction,
    domain: &OrchardAuthorizingDomain,
) -> Result<VerifiedAssetOrchardSwap, OrchardVerificationError> {
    verify_serialized_asset_orchard_swap_action_with_policy(
        action,
        domain,
        AssetOrchardVkVerificationPolicy::LiveCurrent,
    )
}

/// Verifies an immutable archived action using the exact VK named by its circuit ID.
/// Legacy VKs are intentionally inaccessible from the live verification entrypoint.
pub fn verify_serialized_asset_orchard_swap_action_for_archive_replay(
    action: &AssetOrchardSwapAction,
    domain: &OrchardAuthorizingDomain,
) -> Result<VerifiedAssetOrchardSwap, OrchardVerificationError> {
    verify_serialized_asset_orchard_swap_action_with_policy(
        action,
        domain,
        AssetOrchardVkVerificationPolicy::ArchiveReplay,
    )
}

fn verify_serialized_asset_orchard_swap_action_with_policy(
    action: &AssetOrchardSwapAction,
    domain: &OrchardAuthorizingDomain,
    vk_policy: AssetOrchardVkVerificationPolicy,
) -> Result<VerifiedAssetOrchardSwap, OrchardVerificationError> {
    let timing_enabled = std::env::var_os("POSTFIAT_ORCHARD_TIMING_STDERR").is_some();
    let timing_total = std::time::Instant::now();
    let mut timing_stage = timing_total;
    macro_rules! log_timing {
        ($label:literal) => {{
            if timing_enabled {
                let now = std::time::Instant::now();
                eprintln!(
                    "asset_orchard_swap_verify_timing label={} stage_ms={:.3} total_ms={:.3}",
                    $label,
                    timing_stage.elapsed().as_secs_f64() * 1000.0,
                    timing_total.elapsed().as_secs_f64() * 1000.0
                );
                timing_stage = now;
            }
        }};
    }
    action.validate()?;
    log_timing!("action_validate");
    domain.validate()?;
    log_timing!("domain_validate");
    if domain.pool_id != ASSET_ORCHARD_POOL_ID_V1 {
        return Err(OrchardVerificationError::new(
            "unsupported_asset_orchard_pool",
            format!(
                "asset-orchard verifier requires domain pool `{ASSET_ORCHARD_POOL_ID_V1}`, got `{}`",
                domain.pool_id
            ),
        ));
    }
    if action.pool_id != domain.pool_id {
        return Err(OrchardVerificationError::new(
            "pool_id_mismatch",
            format!(
                "asset-orchard action pool_id `{}` does not match domain pool_id `{}`",
                action.pool_id, domain.pool_id
            ),
        ));
    }
    if action.proof_system_id != ASSET_ORCHARD_PROOF_SYSTEM_ID_V1 {
        return Err(OrchardVerificationError::new(
            "unsupported_asset_orchard_proof_system",
            format!(
                "asset-orchard proof system `{}` is not supported",
                action.proof_system_id
            ),
        ));
    }
    validate_asset_orchard_swap_vk_policy(&action.circuit_id, vk_policy)?;
    let genesis_hash = asset_orchard_domain_genesis_hash(&domain.genesis_hash)?;
    log_timing!("domain_genesis_hash");
    action.validate_domain_binding(&domain.chain_id, genesis_hash, domain.protocol_version)?;
    log_timing!("domain_binding");
    action.verify_spend_authorizations(&domain.chain_id, genesis_hash, domain.protocol_version)?;
    log_timing!("spend_authorizations");
    let public_instance = action.public_instance()?;
    log_timing!("public_instance");
    let proof = action.proof.to_bytes()?;
    log_timing!("proof_bytes");
    let verifying_key = match vk_policy {
        AssetOrchardVkVerificationPolicy::LiveCurrent => AssetOrchardSwapVerifyingKey::cached()?,
        AssetOrchardVkVerificationPolicy::ArchiveReplay => {
            AssetOrchardSwapVerifyingKey::cached_for_archive_replay(&action.circuit_id)?
        }
    };
    log_timing!("verifying_key_cached");
    verifying_key.metadata().validate_release_pin()?;
    log_timing!("release_pin");
    let proof_verify_start = std::time::Instant::now();
    if let Err(error) = verifying_key.verify_proof(&proof, &public_instance) {
        record_asset_orchard_swap_proof_verify_timing(AssetOrchardSwapProofVerifyTimingReport {
            schema: "postfiat.asset_orchard_swap.proof_verify_timing.v1".to_string(),
            halo2_verify_proof_ms: asset_orchard_timing_elapsed_ms(proof_verify_start),
            result: "error".to_string(),
        });
        return Err(error.into());
    }
    record_asset_orchard_swap_proof_verify_timing(AssetOrchardSwapProofVerifyTimingReport {
        schema: "postfiat.asset_orchard_swap.proof_verify_timing.v1".to_string(),
        halo2_verify_proof_ms: asset_orchard_timing_elapsed_ms(proof_verify_start),
        result: "ok".to_string(),
    });
    log_timing!("verify_proof");
    let _ = timing_stage;

    Ok(VerifiedAssetOrchardSwap {
        proof_system_id: action.proof_system_id.clone(),
        circuit_id: action.circuit_id.clone(),
        pool_domain: action.pool_domain.clone(),
        anchor: action.anchor.clone(),
        nullifiers: action.nullifiers.clone(),
        randomized_verification_keys: action.randomized_verification_keys.clone(),
        output_commitments: action.output_commitments.clone(),
        encrypted_outputs: action.encrypted_outputs.clone(),
        accounting_inputs: action.accounting_inputs.clone(),
        accounting_outputs: action.accounting_outputs.clone(),
        pricing: VerifiedAssetOrchardPricingClaim {
            claim: action.pricing_claim.clone(),
            action_binding_hash: action.swap_binding_hash.clone(),
            provenance: AssetOrchardPricingClaimProvenance::CircuitProven,
        },
        swap_binding_hash: action.swap_binding_hash.clone(),
        fee: action.fee,
    })
}

pub fn verify_serialized_asset_orchard_private_egress_action(
    action: &AssetOrchardPrivateEgressAction,
    domain: &OrchardAuthorizingDomain,
    to: &str,
    asset_id: &str,
    policy_id: &str,
    disclosure_hash: &str,
) -> Result<VerifiedAssetOrchardPrivateEgress, OrchardVerificationError> {
    verify_serialized_asset_orchard_private_egress_action_with_policy(
        action,
        domain,
        to,
        asset_id,
        policy_id,
        disclosure_hash,
        AssetOrchardVkVerificationPolicy::LiveCurrent,
    )
}

/// Verifies immutable archived private egress using the VK named by its circuit ID.
pub fn verify_serialized_asset_orchard_private_egress_action_for_archive_replay(
    action: &AssetOrchardPrivateEgressAction,
    domain: &OrchardAuthorizingDomain,
    to: &str,
    asset_id: &str,
    policy_id: &str,
    disclosure_hash: &str,
) -> Result<VerifiedAssetOrchardPrivateEgress, OrchardVerificationError> {
    verify_serialized_asset_orchard_private_egress_action_with_policy(
        action,
        domain,
        to,
        asset_id,
        policy_id,
        disclosure_hash,
        AssetOrchardVkVerificationPolicy::ArchiveReplay,
    )
}

fn verify_serialized_asset_orchard_private_egress_action_with_policy(
    action: &AssetOrchardPrivateEgressAction,
    domain: &OrchardAuthorizingDomain,
    to: &str,
    asset_id: &str,
    policy_id: &str,
    disclosure_hash: &str,
    vk_policy: AssetOrchardVkVerificationPolicy,
) -> Result<VerifiedAssetOrchardPrivateEgress, OrchardVerificationError> {
    let total_start = std::time::Instant::now();
    let mut timing = AssetOrchardPrivateEgressActionVerifyTimingReport::default();

    macro_rules! finish_err {
        ($error:expr) => {{
            let error: OrchardVerificationError = $error.into();
            timing.total_ms = asset_orchard_timing_elapsed_ms(total_start);
            timing.result = format!("error:{}", error.code());
            record_asset_orchard_private_egress_action_verify_timing(timing);
            return Err(error);
        }};
    }

    macro_rules! timed_result {
        ($field:ident, $expr:expr) => {{
            let stage_start = std::time::Instant::now();
            match $expr {
                Ok(value) => {
                    timing.$field += asset_orchard_timing_elapsed_ms(stage_start);
                    value
                }
                Err(error) => {
                    timing.$field += asset_orchard_timing_elapsed_ms(stage_start);
                    finish_err!(error);
                }
            }
        }};
    }

    let stage_start = std::time::Instant::now();
    if let Err(error) = action.validate() {
        timing.metadata_pin_validation_ms += asset_orchard_timing_elapsed_ms(stage_start);
        finish_err!(error);
    }
    if let Err(error) = domain.validate() {
        timing.metadata_pin_validation_ms += asset_orchard_timing_elapsed_ms(stage_start);
        finish_err!(error);
    }
    if domain.pool_id != ASSET_ORCHARD_POOL_ID_V1 {
        timing.metadata_pin_validation_ms += asset_orchard_timing_elapsed_ms(stage_start);
        finish_err!(OrchardVerificationError::new(
            "unsupported_asset_orchard_pool",
            format!(
                "asset-orchard private egress verifier requires domain pool `{ASSET_ORCHARD_POOL_ID_V1}`, got `{}`",
                domain.pool_id
            ),
        ));
    }
    if action.pool_id != domain.pool_id {
        timing.metadata_pin_validation_ms += asset_orchard_timing_elapsed_ms(stage_start);
        finish_err!(OrchardVerificationError::new(
            "pool_id_mismatch",
            format!(
                "asset-orchard private egress pool_id `{}` does not match domain pool_id `{}`",
                action.pool_id, domain.pool_id
            ),
        ));
    }
    if action.proof_system_id != ASSET_ORCHARD_PROOF_SYSTEM_ID_V1 {
        timing.metadata_pin_validation_ms += asset_orchard_timing_elapsed_ms(stage_start);
        finish_err!(OrchardVerificationError::new(
            "unsupported_asset_orchard_proof_system",
            format!(
                "asset-orchard proof system `{}` is not supported",
                action.proof_system_id
            ),
        ));
    }
    if let Err(error) =
        validate_asset_orchard_private_egress_vk_policy(&action.circuit_id, vk_policy)
    {
        timing.metadata_pin_validation_ms += asset_orchard_timing_elapsed_ms(stage_start);
        finish_err!(error);
    }

    let expected_tag = match AssetTag::derive(asset_id) {
        Ok(expected_tag) => expected_tag,
        Err(error) => {
            timing.metadata_pin_validation_ms += asset_orchard_timing_elapsed_ms(stage_start);
            finish_err!(error);
        }
    };
    if action.asset_tag_lo != expected_tag.lo || action.asset_tag_hi != expected_tag.hi {
        timing.metadata_pin_validation_ms += asset_orchard_timing_elapsed_ms(stage_start);
        finish_err!(OrchardVerificationError::new(
            "asset_orchard_private_egress_asset_tag_mismatch",
            "private egress asset tag does not match asset_id",
        ));
    }
    timing.metadata_pin_validation_ms += asset_orchard_timing_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    let genesis_hash = match asset_orchard_domain_genesis_hash(&domain.genesis_hash) {
        Ok(genesis_hash) => genesis_hash,
        Err(error) => {
            timing.domain_binding_ms += asset_orchard_timing_elapsed_ms(stage_start);
            finish_err!(error);
        }
    };
    if let Err(error) =
        action.validate_domain_binding(&domain.chain_id, genesis_hash, domain.protocol_version)
    {
        timing.domain_binding_ms += asset_orchard_timing_elapsed_ms(stage_start);
        finish_err!(error);
    }
    timing.domain_binding_ms += asset_orchard_timing_elapsed_ms(stage_start);

    let expected_exit_binding = timed_result!(
        exit_binding_ms,
        asset_orchard_private_egress_exit_binding_hash(
            &AssetOrchardPrivateEgressExitBindingPreimage {
                chain_id: &domain.chain_id,
                genesis_hash,
                protocol_version: domain.protocol_version,
                pool_id: &domain.pool_id,
                circuit_id: &action.circuit_id,
                pool_domain: action.pool_domain.to_field()?,
                to,
                asset_id,
                amount: action.amount,
                fee: action.fee,
                policy_id,
                disclosure_hash,
            },
        )
    );
    let stage_start = std::time::Instant::now();
    let expected_exit_binding = AssetOrchardSwapBindingHash::from_bytes(&expected_exit_binding);
    if action.exit_binding_hash != expected_exit_binding {
        timing.exit_binding_ms += asset_orchard_timing_elapsed_ms(stage_start);
        finish_err!(OrchardVerificationError::new(
            "asset_orchard_private_egress_exit_binding_mismatch",
            "private egress exit_binding_hash does not match recipient/asset/policy fields",
        ));
    }
    timing.exit_binding_ms += asset_orchard_timing_elapsed_ms(stage_start);

    timed_result!(
        spend_auth_verification_ms,
        action.verify_spend_authorization(
            &domain.chain_id,
            genesis_hash,
            domain.protocol_version,
            to,
            asset_id,
            policy_id,
            disclosure_hash,
        )
    );
    let public_instance = timed_result!(public_instance_construction_ms, action.public_instance());
    let proof = timed_result!(proof_bytes_ms, action.proof.to_bytes());
    let verifying_key = timed_result!(
        verifying_key_cached_ms,
        match vk_policy {
            AssetOrchardVkVerificationPolicy::LiveCurrent => {
                AssetOrchardPrivateEgressVerifyingKey::cached()
            }
            AssetOrchardVkVerificationPolicy::ArchiveReplay => {
                AssetOrchardPrivateEgressVerifyingKey::cached_for_archive_replay(&action.circuit_id)
            }
        }
    );
    timed_result!(
        metadata_pin_validation_ms,
        verifying_key.metadata().validate_release_pin()
    );
    timed_result!(
        halo2_verify_proof_ms,
        verifying_key.verify_proof(&proof, &public_instance)
    );

    timing.total_ms = asset_orchard_timing_elapsed_ms(total_start);
    timing.result = "ok".to_string();
    record_asset_orchard_private_egress_action_verify_timing(timing);

    Ok(VerifiedAssetOrchardPrivateEgress {
        proof_system_id: action.proof_system_id.clone(),
        circuit_id: action.circuit_id.clone(),
        pool_domain: action.pool_domain.clone(),
        anchor: action.anchor.clone(),
        nullifier: action.nullifier.clone(),
        randomized_verification_key: action.randomized_verification_key.clone(),
        asset_tag_lo: action.asset_tag_lo,
        asset_tag_hi: action.asset_tag_hi,
        amount: action.amount,
        fee: action.fee,
        exit_binding_hash: action.exit_binding_hash.clone(),
    })
}

pub fn verify_serialized_shielded_swap_action(
    action: &ShieldedSwapAction,
    domain: &OrchardAuthorizingDomain,
) -> Result<VerifiedShieldedSwap, OrchardVerificationError> {
    let _ = (action, domain);
    Err(OrchardVerificationError::new(
        "shielded_swap_proof_verifier_unimplemented",
        "ShieldedSwap consensus verification requires a real asset-conservation ZK proof or homomorphic commitment verifier; transcript hashes and raw witness checks are prover-side test scaffolding only",
    ))
}

pub fn shielded_swap_transcript_hash(
    domain: &OrchardAuthorizingDomain,
    action: &ShieldedSwapAction,
) -> Result<String, OrchardVerificationError> {
    action.validate()?;
    domain.validate()?;
    let mut payload = Vec::new();
    append_str_field(
        &mut payload,
        "schema",
        "postfiat.privacy.shielded-swap.transcript.v1",
    );
    append_str_field(&mut payload, "chain_id", &domain.chain_id);
    append_str_field(&mut payload, "genesis_hash", &domain.genesis_hash);
    append_u32_field(&mut payload, "protocol_version", domain.protocol_version);
    append_str_field(&mut payload, "domain_pool_id", &domain.pool_id);
    append_str_field(&mut payload, "action_schema", &action.schema);
    append_str_field(&mut payload, "pool_id", &action.pool_id);
    append_str_field(&mut payload, "proof_system_id", &action.proof_system_id);
    append_str_field(&mut payload, "circuit_id", &action.circuit_id);
    append_str_field(&mut payload, "anchor", action.anchor.as_hex());
    append_u64_field(&mut payload, "fee", action.fee);
    append_str_field(
        &mut payload,
        "swap_binding_hash",
        action.swap_binding_hash.as_hex(),
    );
    append_swap_commitments(&mut payload, "nullifier", &action.nullifiers);
    append_swap_commitments(
        &mut payload,
        "input_asset_commitment",
        &action.input_asset_commitments,
    );
    append_swap_commitments(
        &mut payload,
        "input_value_commitment",
        &action.input_value_commitments,
    );
    append_swap_commitments(
        &mut payload,
        "input_authorization_commitment",
        &action.input_authorization_commitments,
    );
    append_swap_commitments(
        &mut payload,
        "output_commitment",
        &action.output_commitments,
    );
    append_swap_commitments(
        &mut payload,
        "output_asset_commitment",
        &action.output_asset_commitments,
    );
    append_swap_commitments(
        &mut payload,
        "output_value_commitment",
        &action.output_value_commitments,
    );
    append_u32_field(
        &mut payload,
        "encrypted_output_count",
        action.encrypted_outputs.len() as u32,
    );
    for (index, output) in action.encrypted_outputs.iter().enumerate() {
        append_u32_field(&mut payload, "encrypted_output_index", index as u32);
        append_str_field(&mut payload, "encrypted_output.cmx", output.cmx.as_hex());
        append_str_field(&mut payload, "encrypted_output.epk", output.epk.as_hex());
        append_str_field(
            &mut payload,
            "encrypted_output.enc_ciphertext",
            output.enc_ciphertext.as_hex(),
        );
        append_str_field(
            &mut payload,
            "encrypted_output.out_ciphertext",
            output.out_ciphertext.as_hex(),
        );
    }
    Ok(bytes_to_hex(&hash_bytes(
        "postfiat.privacy.shielded-swap.transcript.v1",
        &payload,
    )))
}

fn validate_swap_private_inputs(
    inputs: &[ShieldedSwapPrivateInput; SHIELDED_SWAP_LEG_COUNT],
) -> Result<(), OrchardVerificationError> {
    for input in inputs {
        validate_domain_text("asset_id", &input.asset_id)?;
        validate_domain_text("asset_blinding", &input.asset_blinding)?;
        validate_domain_text("value_blinding", &input.value_blinding)?;
        validate_domain_text("authorization_secret", &input.authorization_secret)?;
        if input.value == 0 {
            return Err(OrchardVerificationError::new(
                "zero_swap_value",
                "shielded swap input value must be nonzero",
            ));
        }
        let expected = shielded_swap_authorization_proof(
            &input.asset_id,
            input.value,
            &input.authorization_secret,
        )?;
        if input.authorization_proof != expected {
            return Err(OrchardVerificationError::new(
                "unauthorized_shielded_swap_input",
                "shielded swap input authorization proof does not match private note authority",
            ));
        }
    }
    if inputs[0].asset_id == inputs[1].asset_id {
        return Err(OrchardVerificationError::new(
            "same_asset_swap",
            "shielded swap v1 requires two different input assets",
        ));
    }
    Ok(())
}

fn validate_swap_conservation(
    inputs: &[ShieldedSwapPrivateInput; SHIELDED_SWAP_LEG_COUNT],
    outputs: &[ShieldedSwapPrivateOutput; SHIELDED_SWAP_LEG_COUNT],
) -> Result<(), OrchardVerificationError> {
    // Prover-side witness sanity check only. Consensus must never receive raw
    // asset ids or values; it must verify conservation through a real proof or
    // algebraic commitment check over public commitments.
    for output in outputs {
        validate_domain_text("output_asset_id", &output.asset_id)?;
        validate_domain_text("output_asset_blinding", &output.asset_blinding)?;
        validate_domain_text("output_value_blinding", &output.value_blinding)?;
        if output.value == 0 {
            return Err(OrchardVerificationError::new(
                "zero_swap_value",
                "shielded swap output value must be nonzero",
            ));
        }
        if !inputs.iter().any(|input| input.asset_id == output.asset_id) {
            return Err(OrchardVerificationError::new(
                "asset_conservation_failed",
                "shielded swap output asset is not present in inputs",
            ));
        }
    }
    for input in inputs {
        let mut output_sum = 0_u64;
        for output in outputs
            .iter()
            .filter(|output| output.asset_id == input.asset_id)
        {
            output_sum = output_sum.checked_add(output.value).ok_or_else(|| {
                OrchardVerificationError::new(
                    "value_conservation_overflow",
                    "shielded swap output value sum overflowed",
                )
            })?;
        }
        if output_sum == 0 {
            return Err(OrchardVerificationError::new(
                "asset_conservation_failed",
                "shielded swap input asset is missing from outputs",
            ));
        }
        if output_sum != input.value {
            return Err(OrchardVerificationError::new(
                "value_conservation_failed",
                format!(
                    "shielded swap asset conservation holds but value sum {output_sum} does not match input {}",
                    input.value
                ),
            ));
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn shielded_swap_binding_hash_from_parts(
    domain: &OrchardAuthorizingDomain,
    pool_id: &str,
    anchor: &str,
    binding_nonce: &str,
    input_asset_commitments: &[ShieldedSwapCommitment],
    input_value_commitments: &[ShieldedSwapCommitment],
    input_authorization_commitments: &[ShieldedSwapCommitment],
    nullifiers: &[OrchardNullifier],
    output_commitments: &[OrchardOutputCommitment],
    output_asset_commitments: &[ShieldedSwapCommitment],
    output_value_commitments: &[ShieldedSwapCommitment],
) -> Result<ShieldedSwapCommitment, OrchardVerificationError> {
    let mut payload = Vec::new();
    append_str_field(
        &mut payload,
        "schema",
        "postfiat.privacy.shielded-swap.binding.v1",
    );
    append_str_field(&mut payload, "chain_id", &domain.chain_id);
    append_str_field(&mut payload, "genesis_hash", &domain.genesis_hash);
    append_u32_field(&mut payload, "protocol_version", domain.protocol_version);
    append_str_field(&mut payload, "domain_pool_id", &domain.pool_id);
    append_str_field(&mut payload, "pool_id", pool_id);
    append_str_field(&mut payload, "anchor", anchor);
    append_str_field(&mut payload, "binding_nonce", binding_nonce);
    append_swap_commitments(&mut payload, "nullifier", nullifiers);
    append_swap_commitments(
        &mut payload,
        "input_asset_commitment",
        input_asset_commitments,
    );
    append_swap_commitments(
        &mut payload,
        "input_value_commitment",
        input_value_commitments,
    );
    append_swap_commitments(
        &mut payload,
        "input_authorization_commitment",
        input_authorization_commitments,
    );
    append_swap_commitments(&mut payload, "output_commitment", output_commitments);
    append_swap_commitments(
        &mut payload,
        "output_asset_commitment",
        output_asset_commitments,
    );
    append_swap_commitments(
        &mut payload,
        "output_value_commitment",
        output_value_commitments,
    );
    ShieldedSwapCommitment::parse_hex(bytes_to_hex(&hash_bytes(
        "postfiat.privacy.shielded-swap.binding.v1",
        &payload,
    )))
    .map_err(OrchardVerificationError::from)
}

fn derive_canonical_nullifier(
    domain: &str,
    parts: &[&str],
) -> Result<OrchardNullifier, OrchardVerificationError> {
    for counter in 0_u32..4096 {
        let candidate = derive_fixed_bytes(domain, parts, counter, ORCHARD_NULLIFIER_BYTES);
        if let Ok(nullifier) = OrchardNullifier::parse_hex(bytes_to_hex(&candidate)) {
            return Ok(nullifier);
        }
    }
    Err(OrchardVerificationError::new(
        "canonical_derivation_failed",
        "failed to derive a canonical Orchard nullifier",
    ))
}

fn derive_canonical_output_commitment(
    domain: &str,
    parts: &[&str],
) -> Result<OrchardOutputCommitment, OrchardVerificationError> {
    for counter in 0_u32..4096 {
        let candidate = derive_fixed_bytes(domain, parts, counter, crate::ORCHARD_COMMITMENT_BYTES);
        if let Ok(commitment) = OrchardOutputCommitment::parse_hex(bytes_to_hex(&candidate)) {
            return Ok(commitment);
        }
    }
    Err(OrchardVerificationError::new(
        "canonical_derivation_failed",
        "failed to derive a canonical Orchard output commitment",
    ))
}

fn deterministic_swap_encrypted_output(
    commitment: &OrchardOutputCommitment,
    parts: &[&str],
) -> Result<EncryptedShieldedOutput, OrchardVerificationError> {
    let epk = derive_fixed_bytes(
        "postfiat.privacy.shielded-swap.epk.v1",
        parts,
        0,
        crate::ORCHARD_EPK_BYTES,
    );
    let enc = derive_fixed_bytes(
        "postfiat.privacy.shielded-swap.enc-ciphertext.v1",
        parts,
        0,
        crate::ORCHARD_ENC_CIPHERTEXT_BYTES,
    );
    let out = derive_fixed_bytes(
        "postfiat.privacy.shielded-swap.out-ciphertext.v1",
        parts,
        0,
        crate::ORCHARD_OUT_CIPHERTEXT_BYTES,
    );
    EncryptedShieldedOutput::from_bytes(
        commitment.clone(),
        epk.as_slice()
            .try_into()
            .map_err(|_| OrchardVerificationError::new("invalid_epk", "invalid epk length"))?,
        enc.as_slice().try_into().map_err(|_| {
            OrchardVerificationError::new("invalid_enc_ciphertext", "invalid enc ciphertext length")
        })?,
        out.as_slice().try_into().map_err(|_| {
            OrchardVerificationError::new("invalid_out_ciphertext", "invalid out ciphertext length")
        })?,
        None,
    )
    .map_err(OrchardVerificationError::from)
}

fn derive_fixed_bytes(domain: &str, parts: &[&str], counter: u32, len: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(len);
    let mut block = counter;
    while out.len() < len {
        let mut payload = Vec::new();
        append_str_field(&mut payload, "domain", domain);
        append_u32_field(&mut payload, "counter", counter);
        append_u32_field(&mut payload, "block", block);
        append_u32_field(&mut payload, "part_count", parts.len() as u32);
        for part in parts {
            append_str_field(&mut payload, "part", part);
        }
        out.extend_from_slice(&hash_bytes(domain, &payload));
        block = block.saturating_add(1);
    }
    out.truncate(len);
    out
}

trait SwapHex {
    fn swap_hex(&self) -> &str;
}

impl SwapHex for ShieldedSwapCommitment {
    fn swap_hex(&self) -> &str {
        self.as_hex()
    }
}

impl SwapHex for OrchardNullifier {
    fn swap_hex(&self) -> &str {
        self.as_hex()
    }
}

impl SwapHex for OrchardOutputCommitment {
    fn swap_hex(&self) -> &str {
        self.as_hex()
    }
}

fn append_swap_commitments<T: SwapHex>(payload: &mut Vec<u8>, label: &str, values: &[T]) {
    append_str_field(payload, "commitment_list_label", label);
    append_u32_field(payload, "commitment_list_count", values.len() as u32);
    for (index, value) in values.iter().enumerate() {
        append_u32_field(payload, "commitment_list_index", index as u32);
        append_str_field(payload, "commitment_list_value", value.swap_hex());
    }
}

pub fn verify_authorized_bundle<V>(
    bundle: &Bundle<Authorized, V>,
    verifying_key: &VerifyingKey,
    context: &OrchardVerificationContext,
) -> Result<VerifiedOrchardBundle, OrchardVerificationError>
where
    V: Copy + Into<i64>,
{
    context.validate()?;

    let action_count = bundle.actions().iter().count();
    if action_count > context.max_actions {
        return Err(OrchardVerificationError::new(
            "too_many_actions",
            format!(
                "Orchard bundle has {action_count} actions, max {}",
                context.max_actions
            ),
        ));
    }

    bundle.verify_proof(verifying_key).map_err(|error| {
        OrchardVerificationError::new("proof_verification_failed", error.to_string())
    })?;

    verify_bundle_signatures(bundle, &context.authorizing_sighash)?;
    extract_verified_bundle(bundle, context, action_count)
}

pub fn orchard_action_from_authorized_bundle<V>(
    pool_id: impl Into<String>,
    fee: u64,
    bundle: &Bundle<Authorized, V>,
) -> Result<OrchardShieldedAction, OrchardVerificationError>
where
    V: Copy + Into<i64>,
{
    orchard_action_from_authorized_bundle_with_external_binding(pool_id, fee, None, bundle)
}

pub fn orchard_action_from_authorized_bundle_with_external_binding<V>(
    pool_id: impl Into<String>,
    fee: u64,
    external_binding_hash: Option<String>,
    bundle: &Bundle<Authorized, V>,
) -> Result<OrchardShieldedAction, OrchardVerificationError>
where
    V: Copy + Into<i64>,
{
    let action_count = bundle.actions().iter().count();
    let mut nullifiers = Vec::with_capacity(action_count);
    let mut randomized_verification_keys = Vec::with_capacity(action_count);
    let mut value_commitments = Vec::with_capacity(action_count);
    let mut output_commitments = Vec::with_capacity(action_count);
    let mut encrypted_outputs = Vec::with_capacity(action_count);
    let mut spend_authorization_signatures = Vec::with_capacity(action_count);

    for action in bundle.actions() {
        let commitment = OrchardOutputCommitment::from_orchard(*action.cmx());
        let encrypted_note = action.encrypted_note();
        encrypted_outputs.push(EncryptedShieldedOutput::from_bytes(
            commitment.clone(),
            &encrypted_note.epk_bytes,
            &encrypted_note.enc_ciphertext,
            &encrypted_note.out_ciphertext,
            None,
        )?);
        nullifiers.push(OrchardNullifier::from_orchard(*action.nullifier()));
        randomized_verification_keys
            .push(OrchardRandomizedVerificationKey::from_orchard(action.rk()));
        value_commitments.push(OrchardValueCommitment::from_orchard(action.cv_net()));
        output_commitments.push(commitment);
        spend_authorization_signatures.push(OrchardSpendAuthSignature::from_orchard(
            action.authorization(),
        ));
    }

    let action = OrchardShieldedAction {
        pool_id: pool_id.into(),
        proof_system_id: OrchardProofSystemId::production_v2(),
        circuit_id: OrchardCircuitId::action_v2(),
        flags: OrchardFlags::from_orchard(*bundle.flags()),
        anchor: OrchardAnchor::from_orchard(*bundle.anchor()),
        nullifiers,
        randomized_verification_keys,
        value_commitments,
        output_commitments,
        encrypted_outputs,
        value_balance: (*bundle.value_balance()).into(),
        external_binding_hash,
        fee,
        proof: OrchardProofBytes::from_bytes(bundle.authorization().proof().as_ref())?,
        spend_authorization_signatures,
        binding_signature: OrchardBindingSignature::from_orchard(
            bundle.authorization().binding_signature(),
        ),
    };
    action.validate()?;
    Ok(action)
}

pub fn orchard_bundle_from_action(
    action: &OrchardShieldedAction,
) -> Result<Bundle<Authorized, i64>, OrchardVerificationError> {
    action.validate()?;

    let mut actions = Vec::with_capacity(action.nullifiers.len());
    for index in 0..action.nullifiers.len() {
        let output = &action.encrypted_outputs[index];
        let encrypted_note = TransmittedNoteCiphertext {
            epk_bytes: output.epk.to_fixed_bytes("epk")?,
            enc_ciphertext: output.enc_ciphertext.to_fixed_bytes("enc_ciphertext")?,
            out_ciphertext: output.out_ciphertext.to_fixed_bytes("out_ciphertext")?,
        };
        let orchard_action = Action::from_parts(
            action.nullifiers[index].to_orchard()?,
            action.randomized_verification_keys[index].to_orchard()?,
            action.output_commitments[index].to_orchard()?,
            encrypted_note,
            action.value_commitments[index].to_orchard()?,
            action.spend_authorization_signatures[index].to_orchard()?,
        )
        .map_err(|error| {
            OrchardVerificationError::new(
                "invalid_orchard_action",
                format!("Orchard action {index} is not well-formed: {error}"),
            )
        })?;
        actions.push(orchard_action);
    }

    let actions = NonEmpty::from_vec(actions).ok_or_else(|| {
        OrchardVerificationError::new(
            "missing_action",
            "Orchard action payload must contain at least one action",
        )
    })?;

    Bundle::try_from_parts(
        actions,
        action.flags.to_orchard()?,
        action.value_balance,
        action.anchor.to_orchard()?,
        Authorized::from_parts(
            Proof::new(action.proof.to_bytes()?),
            action.binding_signature.to_orchard()?,
        ),
        ProofSizeEnforcement::Strict,
    )
    .map_err(|error| OrchardVerificationError::new("invalid_orchard_bundle", error.to_string()))
}

pub fn verify_serialized_orchard_action(
    action: &OrchardShieldedAction,
    verifying_key: &VerifyingKey,
    domain: &OrchardAuthorizingDomain,
) -> Result<VerifiedOrchardBundle, OrchardVerificationError> {
    if action.pool_id != domain.pool_id {
        return Err(OrchardVerificationError::new(
            "pool_id_mismatch",
            format!(
                "Orchard action pool_id `{}` does not match domain pool_id `{}`",
                action.pool_id, domain.pool_id
            ),
        ));
    }
    let bundle = orchard_bundle_from_action(action)?;
    let context = OrchardVerificationContext::for_bundle_with_external_binding(
        domain,
        action.fee,
        action.external_binding_hash.as_deref(),
        &bundle,
    )?;
    verify_authorized_bundle(&bundle, verifying_key, &context)
}

pub fn verify_serialized_orchard_action_with_built_key(
    action: &OrchardShieldedAction,
    domain: &OrchardAuthorizingDomain,
) -> Result<VerifiedOrchardBundle, OrchardVerificationError> {
    let verifying_key = VerifyingKey::build();
    verify_serialized_orchard_action(action, &verifying_key, domain)
}

pub fn orchard_build_output_action(
    domain: &OrchardAuthorizingDomain,
    pool_id: impl Into<String>,
    fee: u64,
    anchor: OrchardAnchor,
    recipient_address_raw_hex: &str,
    value: u64,
    memo: [u8; ORCHARD_MEMO_BYTES],
) -> Result<OrchardShieldedAction, OrchardVerificationError> {
    orchard_build_output_action_with_external_binding(
        domain,
        pool_id,
        fee,
        anchor,
        recipient_address_raw_hex,
        value,
        memo,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn orchard_build_output_action_with_external_binding(
    domain: &OrchardAuthorizingDomain,
    pool_id: impl Into<String>,
    fee: u64,
    anchor: OrchardAnchor,
    recipient_address_raw_hex: &str,
    value: u64,
    memo: [u8; ORCHARD_MEMO_BYTES],
    external_binding_hash: Option<&str>,
) -> Result<OrchardShieldedAction, OrchardVerificationError> {
    orchard_build_output_action_with_external_binding_rng(
        domain,
        pool_id,
        fee,
        anchor,
        recipient_address_raw_hex,
        value,
        memo,
        external_binding_hash,
        OsRng,
        OsRng,
        OsRng,
    )
}

#[allow(clippy::too_many_arguments)]
/// Builds a deterministic public fixture for vector tests.
///
/// This must not be used for production funds. Production callers should use
/// `orchard_build_output_action` or `orchard_build_output_action_with_external_binding`,
/// which keep all Orchard randomness on `OsRng`.
pub fn orchard_build_output_action_test_vector(
    domain: &OrchardAuthorizingDomain,
    pool_id: impl Into<String>,
    fee: u64,
    anchor: OrchardAnchor,
    recipient_address_raw_hex: &str,
    value: u64,
    memo: [u8; ORCHARD_MEMO_BYTES],
    external_binding_hash: Option<&str>,
    build_seed: [u8; 32],
    proof_seed: [u8; 32],
    signature_seed: [u8; 32],
) -> Result<OrchardShieldedAction, OrchardVerificationError> {
    orchard_build_output_action_with_external_binding_rng(
        domain,
        pool_id,
        fee,
        anchor,
        recipient_address_raw_hex,
        value,
        memo,
        external_binding_hash,
        StdRng::from_seed(build_seed),
        StdRng::from_seed(proof_seed),
        StdRng::from_seed(signature_seed),
    )
}

#[allow(clippy::too_many_arguments)]
fn orchard_build_output_action_with_external_binding_rng<BuildRng, ProofRng, SignatureRng>(
    domain: &OrchardAuthorizingDomain,
    pool_id: impl Into<String>,
    fee: u64,
    anchor: OrchardAnchor,
    recipient_address_raw_hex: &str,
    value: u64,
    memo: [u8; ORCHARD_MEMO_BYTES],
    external_binding_hash: Option<&str>,
    mut build_rng: BuildRng,
    mut proof_rng: ProofRng,
    mut signature_rng: SignatureRng,
) -> Result<OrchardShieldedAction, OrchardVerificationError>
where
    BuildRng: RngCore + CryptoRng,
    ProofRng: RngCore + CryptoRng,
    SignatureRng: RngCore + CryptoRng,
{
    domain.validate()?;
    let pool_id = pool_id.into();
    if pool_id != domain.pool_id {
        return Err(OrchardVerificationError::new(
            "pool_id_mismatch",
            format!(
                "Orchard output action pool_id `{pool_id}` does not match domain pool_id `{}`",
                domain.pool_id
            ),
        ));
    }

    let recipient = orchard_address_from_raw_hex(recipient_address_raw_hex)?;
    let mut builder = Builder::new(BundleType::DEFAULT, anchor.to_orchard()?);
    builder
        .add_output(None, recipient, NoteValue::from_raw(value), memo)
        .map_err(|error| OrchardVerificationError::new("add_output_failed", error.to_string()))?;

    let (unsigned_bundle, _) = builder
        .build::<i64>(&mut build_rng)
        .map_err(|error| OrchardVerificationError::new("build_bundle_failed", error.to_string()))?
        .ok_or_else(|| {
            OrchardVerificationError::new(
                "missing_orchard_bundle",
                "Orchard output builder produced no bundle",
            )
        })?;
    let sighash = orchard_authorizing_sighash_with_external_binding(
        domain,
        fee,
        external_binding_hash,
        &unsigned_bundle,
    )?;
    let proving_key = ProvingKey::build();
    let bundle = unsigned_bundle
        .create_proof(&proving_key, &mut proof_rng)
        .map_err(|error| OrchardVerificationError::new("create_proof_failed", error.to_string()))?
        .apply_signatures(&mut signature_rng, sighash, &[])
        .map_err(|error| {
            OrchardVerificationError::new("apply_signatures_failed", error.to_string())
        })?;

    orchard_action_from_authorized_bundle_with_external_binding(
        pool_id,
        fee,
        external_binding_hash.map(str::to_string),
        &bundle,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn orchard_build_spend_action(
    domain: &OrchardAuthorizingDomain,
    pool_id: impl Into<String>,
    fee: u64,
    anchor: OrchardAnchor,
    spending_key_bytes: [u8; 32],
    spend_note: &OrchardSpendNote,
    recipient_address_raw_hex: &str,
    recipient_value: u64,
    change_address_raw_hex: Option<&str>,
    memo: [u8; ORCHARD_MEMO_BYTES],
) -> Result<OrchardShieldedAction, OrchardVerificationError> {
    domain.validate()?;
    let pool_id = pool_id.into();
    if pool_id != domain.pool_id {
        return Err(OrchardVerificationError::new(
            "pool_id_mismatch",
            format!(
                "Orchard spend action pool_id `{pool_id}` does not match domain pool_id `{}`",
                domain.pool_id
            ),
        ));
    }
    if spend_note.witness_anchor != anchor.as_hex() {
        return Err(OrchardVerificationError::new(
            "anchor_mismatch",
            "Orchard spend witness anchor does not match requested action anchor",
        ));
    }
    let spending_key = orchard_spending_key_from_bytes(spending_key_bytes)?;
    let full_viewing_key = FullViewingKey::from(&spending_key);
    let spend_authorizing_key = SpendAuthorizingKey::from(&spending_key);
    let note = orchard_note_from_spend_note(spend_note)?;
    let merkle_path = orchard_merkle_path_from_spend_note(spend_note)?;
    let path_root = merkle_path.root(note.commitment().into());
    if bytes_to_hex(&path_root.to_bytes()) != anchor.as_hex() {
        return Err(OrchardVerificationError::new(
            "anchor_mismatch",
            "Orchard spend note witness does not resolve to requested anchor",
        ));
    }

    let output_value = spend_note.value.checked_sub(fee).ok_or_else(|| {
        OrchardVerificationError::new(
            "fee_exceeds_note_value",
            format!(
                "Orchard spend fee {fee} exceeds input note value {}",
                spend_note.value
            ),
        )
    })?;
    if recipient_value > output_value {
        return Err(OrchardVerificationError::new(
            "recipient_value_exceeds_spendable_value",
            format!(
                "Orchard spend recipient value {recipient_value} exceeds input value {} minus fee {fee}",
                spend_note.value
            ),
        ));
    }
    let change_value = output_value - recipient_value;
    if change_value > 0 && change_address_raw_hex.is_none() {
        return Err(OrchardVerificationError::new(
            "missing_change_address",
            "Orchard spend requires a change address when recipient value is below input minus fee",
        ));
    }
    let recipient = orchard_address_from_raw_hex(recipient_address_raw_hex)?;
    let mut builder = Builder::new(BundleType::DEFAULT, anchor.to_orchard()?);
    builder
        .add_spend(full_viewing_key, note, merkle_path)
        .map_err(|error| OrchardVerificationError::new("add_spend_failed", error.to_string()))?;
    builder
        .add_output(None, recipient, NoteValue::from_raw(recipient_value), memo)
        .map_err(|error| OrchardVerificationError::new("add_output_failed", error.to_string()))?;
    if change_value > 0 {
        let change_address = orchard_address_from_raw_hex(change_address_raw_hex.ok_or_else(|| {
            OrchardVerificationError::new(
                "missing_change_address",
                "Orchard spend requires a change address when recipient value is below input minus fee",
            )
        })?)?;
        builder
            .add_output(
                None,
                change_address,
                NoteValue::from_raw(change_value),
                [0u8; ORCHARD_MEMO_BYTES],
            )
            .map_err(|error| {
                OrchardVerificationError::new("add_change_output_failed", error.to_string())
            })?;
    }

    let (unsigned_bundle, _) = builder
        .build::<i64>(OsRng)
        .map_err(|error| OrchardVerificationError::new("build_bundle_failed", error.to_string()))?
        .ok_or_else(|| {
            OrchardVerificationError::new(
                "missing_orchard_bundle",
                "Orchard spend builder produced no bundle",
            )
        })?;
    let value_balance: i64 = *unsigned_bundle.value_balance();
    let expected_value_balance = i64::try_from(fee).map_err(|_| {
        OrchardVerificationError::new(
            "fee_too_large",
            "Orchard spend fee does not fit signed value-balance encoding",
        )
    })?;
    if value_balance != expected_value_balance {
        return Err(OrchardVerificationError::new(
            "unsupported_spend_value_balance",
            format!("Orchard spend action value balance {value_balance} does not match fee {fee}"),
        ));
    }
    let sighash = orchard_authorizing_sighash(domain, fee, &unsigned_bundle)?;
    let proving_key = ProvingKey::build();
    let bundle = unsigned_bundle
        .create_proof(&proving_key, OsRng)
        .map_err(|error| OrchardVerificationError::new("create_proof_failed", error.to_string()))?
        .apply_signatures(OsRng, sighash, &[spend_authorizing_key])
        .map_err(|error| {
            OrchardVerificationError::new("apply_signatures_failed", error.to_string())
        })?;

    orchard_action_from_authorized_bundle(pool_id, fee, &bundle)
}

#[allow(clippy::too_many_arguments)]
pub fn orchard_build_withdraw_action(
    domain: &OrchardAuthorizingDomain,
    pool_id: impl Into<String>,
    external_binding_hash: &str,
    fee: u64,
    anchor: OrchardAnchor,
    spending_key_bytes: [u8; 32],
    spend_note: &OrchardSpendNote,
    withdraw_amount: u64,
    change_address_raw_hex: Option<&str>,
    memo: [u8; ORCHARD_MEMO_BYTES],
) -> Result<OrchardShieldedAction, OrchardVerificationError> {
    domain.validate()?;
    let pool_id = pool_id.into();
    if pool_id != domain.pool_id {
        return Err(OrchardVerificationError::new(
            "pool_id_mismatch",
            format!(
                "Orchard withdraw action pool_id `{pool_id}` does not match domain pool_id `{}`",
                domain.pool_id
            ),
        ));
    }
    if withdraw_amount == 0 {
        return Err(OrchardVerificationError::new(
            "zero_withdraw_amount",
            "Orchard withdraw amount must be nonzero",
        ));
    }
    if spend_note.witness_anchor != anchor.as_hex() {
        return Err(OrchardVerificationError::new(
            "anchor_mismatch",
            "Orchard withdraw witness anchor does not match requested action anchor",
        ));
    }

    let spending_key = orchard_spending_key_from_bytes(spending_key_bytes)?;
    let full_viewing_key = FullViewingKey::from(&spending_key);
    let spend_authorizing_key = SpendAuthorizingKey::from(&spending_key);
    let note = orchard_note_from_spend_note(spend_note)?;
    let merkle_path = orchard_merkle_path_from_spend_note(spend_note)?;
    let path_root = merkle_path.root(note.commitment().into());
    if bytes_to_hex(&path_root.to_bytes()) != anchor.as_hex() {
        return Err(OrchardVerificationError::new(
            "anchor_mismatch",
            "Orchard withdraw note witness does not resolve to requested anchor",
        ));
    }

    let exit_value = withdraw_amount.checked_add(fee).ok_or_else(|| {
        OrchardVerificationError::new(
            "withdraw_value_overflow",
            "Orchard withdraw amount plus fee overflowed",
        )
    })?;
    let change_value = spend_note.value.checked_sub(exit_value).ok_or_else(|| {
        OrchardVerificationError::new(
            "withdraw_exceeds_note_value",
            format!(
                "Orchard withdraw amount {withdraw_amount} plus fee {fee} exceeds input value {}",
                spend_note.value
            ),
        )
    })?;
    if change_value > 0 && change_address_raw_hex.is_none() {
        return Err(OrchardVerificationError::new(
            "missing_change_address",
            "Orchard withdraw requires a change address when input exceeds amount plus fee",
        ));
    }

    let mut builder = Builder::new(BundleType::DEFAULT, anchor.to_orchard()?);
    builder
        .add_spend(full_viewing_key, note, merkle_path)
        .map_err(|error| OrchardVerificationError::new("add_spend_failed", error.to_string()))?;
    if change_value > 0 {
        let change_address =
            orchard_address_from_raw_hex(change_address_raw_hex.ok_or_else(|| {
                OrchardVerificationError::new(
                    "missing_change_address",
                    "Orchard withdraw requires a change address when input exceeds amount plus fee",
                )
            })?)?;
        builder
            .add_output(
                None,
                change_address,
                NoteValue::from_raw(change_value),
                memo,
            )
            .map_err(|error| {
                OrchardVerificationError::new("add_change_output_failed", error.to_string())
            })?;
    }

    let (unsigned_bundle, _) = builder
        .build::<i64>(OsRng)
        .map_err(|error| OrchardVerificationError::new("build_bundle_failed", error.to_string()))?
        .ok_or_else(|| {
            OrchardVerificationError::new(
                "missing_orchard_bundle",
                "Orchard withdraw builder produced no bundle",
            )
        })?;
    let value_balance: i64 = *unsigned_bundle.value_balance();
    let expected_value_balance = i64::try_from(exit_value).map_err(|_| {
        OrchardVerificationError::new(
            "withdraw_value_too_large",
            "Orchard withdraw amount plus fee does not fit signed value-balance encoding",
        )
    })?;
    if value_balance != expected_value_balance {
        return Err(OrchardVerificationError::new(
            "unsupported_withdraw_value_balance",
            format!(
                "Orchard withdraw action value balance {value_balance} does not match amount {withdraw_amount} plus fee {fee}"
            ),
        ));
    }
    let sighash = orchard_authorizing_sighash_with_external_binding(
        domain,
        fee,
        Some(external_binding_hash),
        &unsigned_bundle,
    )?;
    let proving_key = ProvingKey::build();
    let bundle = unsigned_bundle
        .create_proof(&proving_key, OsRng)
        .map_err(|error| OrchardVerificationError::new("create_proof_failed", error.to_string()))?
        .apply_signatures(OsRng, sighash, &[spend_authorizing_key])
        .map_err(|error| {
            OrchardVerificationError::new("apply_signatures_failed", error.to_string())
        })?;

    orchard_action_from_authorized_bundle_with_external_binding(
        pool_id,
        fee,
        Some(external_binding_hash.to_string()),
        &bundle,
    )
}

pub fn orchard_empty_anchor() -> OrchardAnchor {
    OrchardAnchor::from_orchard(orchard::Anchor::empty_tree())
}

pub fn orchard_anchor_from_commitments(
    commitments: &[OrchardOutputCommitment],
) -> Result<OrchardAnchor, OrchardVerificationError> {
    let mut frontier = Frontier::<MerkleHashOrchard, ORCHARD_TREE_DEPTH>::empty();
    append_commitments_to_frontier(&mut frontier, commitments)?;
    Ok(OrchardAnchor::from_orchard(frontier.root().into()))
}

pub fn orchard_frontier_snapshot_from_commitments(
    commitments: &[OrchardOutputCommitment],
) -> Result<OrchardFrontierSnapshot, OrchardVerificationError> {
    let mut frontier = Frontier::<MerkleHashOrchard, ORCHARD_TREE_DEPTH>::empty();
    append_commitments_to_frontier(&mut frontier, commitments)?;
    Ok(orchard_frontier_snapshot_from_frontier(&frontier))
}

pub fn orchard_frontier_snapshot_append_commitments(
    snapshot: Option<&OrchardFrontierSnapshot>,
    commitments: &[OrchardOutputCommitment],
) -> Result<OrchardFrontierSnapshot, OrchardVerificationError> {
    let mut frontier = match snapshot {
        Some(snapshot) => orchard_frontier_from_snapshot(snapshot)?,
        None => Frontier::<MerkleHashOrchard, ORCHARD_TREE_DEPTH>::empty(),
    };
    append_commitments_to_frontier(&mut frontier, commitments)?;
    Ok(orchard_frontier_snapshot_from_frontier(&frontier))
}

fn append_commitments_to_frontier(
    frontier: &mut Frontier<MerkleHashOrchard, ORCHARD_TREE_DEPTH>,
    commitments: &[OrchardOutputCommitment],
) -> Result<(), OrchardVerificationError> {
    for commitment in commitments {
        let commitment = commitment.to_orchard()?;
        let leaf = MerkleHashOrchard::from_cmx(&commitment);
        if !frontier.append(leaf) {
            return Err(OrchardVerificationError::new(
                "orchard_tree_full",
                "Orchard note commitment tree is full",
            ));
        }
    }
    Ok(())
}

fn orchard_frontier_from_snapshot(
    snapshot: &OrchardFrontierSnapshot,
) -> Result<Frontier<MerkleHashOrchard, ORCHARD_TREE_DEPTH>, OrchardVerificationError> {
    if snapshot.output_count == 0 {
        if snapshot.latest_leaf.is_some() || !snapshot.ommers.is_empty() {
            return Err(OrchardVerificationError::new(
                "orchard_frontier_cache_invalid_empty",
                "empty Orchard frontier cache cannot contain a latest leaf or ommers",
            ));
        }
        let frontier = Frontier::<MerkleHashOrchard, ORCHARD_TREE_DEPTH>::empty();
        validate_frontier_snapshot_root(snapshot, &frontier)?;
        return Ok(frontier);
    }

    let position = snapshot.output_count.checked_sub(1).ok_or_else(|| {
        OrchardVerificationError::new(
            "orchard_frontier_cache_invalid_count",
            "Orchard frontier cache output_count underflow",
        )
    })?;
    let latest_leaf = snapshot.latest_leaf.as_deref().ok_or_else(|| {
        OrchardVerificationError::new(
            "orchard_frontier_cache_missing_leaf",
            "non-empty Orchard frontier cache is missing latest_leaf",
        )
    })?;
    let leaf = orchard_merkle_hash_from_hex("orchard_frontier_latest_leaf", latest_leaf)?;
    let ommers = snapshot
        .ommers
        .iter()
        .map(|ommer| orchard_merkle_hash_from_hex("orchard_frontier_ommer", ommer))
        .collect::<Result<Vec<_>, _>>()?;
    let frontier = Frontier::<MerkleHashOrchard, ORCHARD_TREE_DEPTH>::from_parts(
        Position::from(position),
        leaf,
        ommers,
    )
    .map_err(|error| {
        OrchardVerificationError::new(
            "orchard_frontier_cache_invalid_parts",
            format!("Orchard frontier cache parts are inconsistent: {error:?}"),
        )
    })?;
    validate_frontier_snapshot_root(snapshot, &frontier)?;
    Ok(frontier)
}

fn validate_frontier_snapshot_root(
    snapshot: &OrchardFrontierSnapshot,
    frontier: &Frontier<MerkleHashOrchard, ORCHARD_TREE_DEPTH>,
) -> Result<(), OrchardVerificationError> {
    let expected_root = orchard_frontier_root_hex(frontier);
    if snapshot.root != expected_root {
        return Err(OrchardVerificationError::new(
            "orchard_frontier_cache_root_mismatch",
            "Orchard frontier cache root does not match frontier parts",
        ));
    }
    Ok(())
}

fn orchard_frontier_snapshot_from_frontier(
    frontier: &Frontier<MerkleHashOrchard, ORCHARD_TREE_DEPTH>,
) -> OrchardFrontierSnapshot {
    let root = orchard_frontier_root_hex(frontier);
    let Some(frontier) = frontier.value() else {
        return OrchardFrontierSnapshot {
            output_count: 0,
            root,
            latest_leaf: None,
            ommers: Vec::new(),
        };
    };
    OrchardFrontierSnapshot {
        output_count: u64::from(frontier.position()) + 1,
        root,
        latest_leaf: Some(bytes_to_hex(&frontier.leaf().to_bytes())),
        ommers: frontier
            .ommers()
            .iter()
            .map(|ommer| bytes_to_hex(&ommer.to_bytes()))
            .collect(),
    }
}

fn orchard_frontier_root_hex(frontier: &Frontier<MerkleHashOrchard, ORCHARD_TREE_DEPTH>) -> String {
    OrchardAnchor::from_orchard(frontier.root().into())
        .as_hex()
        .to_string()
}

fn orchard_merkle_hash_from_hex(
    field: &'static str,
    value: &str,
) -> Result<MerkleHashOrchard, OrchardVerificationError> {
    let bytes = fixed_lower_hex_array::<32>(field, value)?;
    Option::<MerkleHashOrchard>::from(MerkleHashOrchard::from_bytes(&bytes)).ok_or_else(|| {
        OrchardVerificationError::new(
            "orchard_frontier_cache_invalid_node",
            format!("{field} is not a canonical Orchard Merkle node"),
        )
    })
}

pub fn orchard_merkle_witness_from_commitments(
    commitments: &[OrchardOutputCommitment],
    position: usize,
) -> Result<OrchardMerkleWitness, OrchardVerificationError> {
    if position >= commitments.len() {
        return Err(OrchardVerificationError::new(
            "orchard_witness_position_out_of_range",
            format!(
                "Orchard witness position {position} is outside {} commitments",
                commitments.len()
            ),
        ));
    }
    let position_u32 = u32::try_from(position).map_err(|_| {
        OrchardVerificationError::new(
            "orchard_witness_position_out_of_range",
            "Orchard witness position does not fit u32",
        )
    })?;
    let output_count = u64::try_from(commitments.len()).map_err(|_| {
        OrchardVerificationError::new(
            "orchard_output_count_overflow",
            "Orchard output commitment count does not fit u64",
        )
    })?;
    if output_count > (1_u64 << ORCHARD_TREE_DEPTH) {
        return Err(OrchardVerificationError::new(
            "orchard_tree_full",
            "Orchard note commitment tree exceeds its fixed depth",
        ));
    }

    let mut level_nodes = commitments
        .iter()
        .map(|commitment| {
            commitment
                .to_orchard()
                .map(|commitment| MerkleHashOrchard::from_cmx(&commitment))
                .map_err(OrchardVerificationError::from)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut auth_path = Vec::with_capacity(usize::from(ORCHARD_TREE_DEPTH));
    let mut index = position;

    for level_index in 0..ORCHARD_TREE_DEPTH {
        let level = Level::from(level_index);
        let empty = MerkleHashOrchard::empty_root(level);
        let sibling_index = if index & 1 == 0 { index + 1 } else { index - 1 };
        let sibling = level_nodes.get(sibling_index).cloned().unwrap_or(empty);
        auth_path.push(sibling);

        let mut next_level = Vec::with_capacity(level_nodes.len().div_ceil(2));
        for pair in level_nodes.chunks(2) {
            let left = &pair[0];
            let right = pair.get(1).unwrap_or(&empty);
            next_level.push(MerkleHashOrchard::combine(level, left, right));
        }
        level_nodes = next_level;
        index /= 2;
    }

    let computed_anchor = level_nodes.first().ok_or_else(|| {
        OrchardVerificationError::new(
            "orchard_witness_root_missing",
            "Orchard witness construction produced no root",
        )
    })?;
    let anchor = orchard_anchor_from_commitments(commitments)?;
    let computed_anchor_hex = bytes_to_hex(&computed_anchor.to_bytes());
    if computed_anchor_hex != anchor.as_hex() {
        return Err(OrchardVerificationError::new(
            "orchard_witness_root_mismatch",
            "Orchard witness root does not match commitment frontier root",
        ));
    }

    Ok(OrchardMerkleWitness {
        position: position_u32,
        anchor: anchor.as_hex().to_string(),
        output_count,
        auth_path: auth_path
            .iter()
            .map(|node| bytes_to_hex(&node.to_bytes()))
            .collect(),
    })
}

fn orchard_note_from_spend_note(
    spend_note: &OrchardSpendNote,
) -> Result<Note, OrchardVerificationError> {
    let recipient = orchard_address_from_raw_hex(&spend_note.address_raw_hex)?;
    let rho_bytes = fixed_lower_hex_array::<32>("rho", &spend_note.rho)?;
    let rho = Option::<Rho>::from(Rho::from_bytes(&rho_bytes)).ok_or_else(|| {
        OrchardVerificationError::new("invalid_rho", "Orchard note rho is not canonical")
    })?;
    let rseed_bytes = fixed_lower_hex_array::<32>("rseed", &spend_note.rseed)?;
    let rseed =
        Option::<RandomSeed>::from(RandomSeed::from_bytes(rseed_bytes, &rho)).ok_or_else(|| {
            OrchardVerificationError::new(
                "invalid_rseed",
                "Orchard note rseed is not valid for the supplied rho",
            )
        })?;
    let note = Option::<Note>::from(Note::from_parts(
        recipient,
        NoteValue::from_raw(spend_note.value),
        rho,
        rseed,
    ))
    .ok_or_else(|| {
        OrchardVerificationError::new(
            "invalid_spend_note",
            "Orchard spend note parts do not form a valid note",
        )
    })?;
    let commitment = OrchardOutputCommitment::from_orchard(note.commitment().into());
    if commitment.as_hex() != spend_note.commitment {
        return Err(OrchardVerificationError::new(
            "commitment_mismatch",
            "Orchard spend note commitment does not match decrypted output commitment",
        ));
    }
    Ok(note)
}

fn orchard_merkle_path_from_spend_note(
    spend_note: &OrchardSpendNote,
) -> Result<MerklePath, OrchardVerificationError> {
    if spend_note.witness_auth_path.len() != usize::from(ORCHARD_TREE_DEPTH) {
        return Err(OrchardVerificationError::new(
            "invalid_witness_path",
            format!(
                "Orchard witness path must contain {} nodes",
                ORCHARD_TREE_DEPTH
            ),
        ));
    }
    let auth_path = spend_note
        .witness_auth_path
        .iter()
        .map(|node| {
            let bytes = fixed_lower_hex_array::<32>("witness_auth_path", node)?;
            Option::<MerkleHashOrchard>::from(MerkleHashOrchard::from_bytes(&bytes)).ok_or_else(
                || {
                    OrchardVerificationError::new(
                        "invalid_witness_path",
                        "Orchard witness path node is not canonical",
                    )
                },
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let auth_path: [MerkleHashOrchard; orchard::NOTE_COMMITMENT_TREE_DEPTH] = auth_path
        .try_into()
        .map_err(|auth_path: Vec<MerkleHashOrchard>| {
            OrchardVerificationError::new(
                "invalid_witness_path",
                format!("Orchard witness path has {} nodes", auth_path.len()),
            )
        })?;
    Ok(MerklePath::from_parts(
        spend_note.merkle_position,
        auth_path,
    ))
}

pub fn orchard_default_address_from_spending_key(
    spending_key_bytes: [u8; 32],
) -> Result<String, OrchardVerificationError> {
    let spending_key = orchard_spending_key_from_bytes(spending_key_bytes)?;
    let full_viewing_key = FullViewingKey::from(&spending_key);
    Ok(default_address_from_full_viewing_key(&full_viewing_key))
}

pub fn orchard_spending_key_from_zip32_seed(
    seed: &[u8],
    account_index: u32,
) -> Result<Zeroizing<[u8; 32]>, OrchardVerificationError> {
    let account = AccountId::try_from(account_index).map_err(|_| {
        OrchardVerificationError::new(
            "invalid_zip32_account",
            "Orchard ZIP32 account index must be below 2^31",
        )
    })?;
    let spending_key = SpendingKey::from_zip32_seed(seed, POSTFIAT_ORCHARD_COIN_TYPE, account)
        .map_err(|error| {
            OrchardVerificationError::new("zip32_spending_key_derivation_failed", error.to_string())
        })?;
    Ok(Zeroizing::new(*spending_key.to_bytes()))
}

pub fn orchard_full_viewing_key_from_spending_key(
    spending_key_bytes: [u8; 32],
) -> Result<[u8; 96], OrchardVerificationError> {
    let spending_key = orchard_spending_key_from_bytes(spending_key_bytes)?;
    Ok(FullViewingKey::from(&spending_key).to_bytes())
}

pub fn orchard_default_address_from_full_viewing_key(
    full_viewing_key_bytes: [u8; 96],
) -> Result<String, OrchardVerificationError> {
    let full_viewing_key = orchard_full_viewing_key_from_bytes(full_viewing_key_bytes)?;
    Ok(default_address_from_full_viewing_key(&full_viewing_key))
}

pub fn orchard_scan_encrypted_outputs_with_spending_key(
    spending_key_bytes: [u8; 32],
    nullifiers: &[OrchardNullifier],
    outputs: &[EncryptedShieldedOutput],
) -> Result<Vec<OrchardDecryptedOutput>, OrchardVerificationError> {
    Ok(orchard_scan_encrypted_outputs_report_with_spending_key(
        spending_key_bytes,
        nullifiers,
        outputs,
    )?
    .decrypted_outputs)
}

pub fn orchard_scan_encrypted_outputs_report_with_spending_key(
    spending_key_bytes: [u8; 32],
    nullifiers: &[OrchardNullifier],
    outputs: &[EncryptedShieldedOutput],
) -> Result<OrchardScanReport, OrchardVerificationError> {
    let full_viewing_key =
        FullViewingKey::from(&orchard_spending_key_from_bytes(spending_key_bytes)?);
    orchard_scan_encrypted_outputs_report_with_full_viewing_key(
        full_viewing_key.to_bytes(),
        nullifiers,
        outputs,
    )
}

pub fn orchard_scan_encrypted_outputs_with_full_viewing_key(
    full_viewing_key_bytes: [u8; 96],
    nullifiers: &[OrchardNullifier],
    outputs: &[EncryptedShieldedOutput],
) -> Result<Vec<OrchardDecryptedOutput>, OrchardVerificationError> {
    Ok(orchard_scan_encrypted_outputs_report_with_full_viewing_key(
        full_viewing_key_bytes,
        nullifiers,
        outputs,
    )?
    .decrypted_outputs)
}

pub fn orchard_scan_encrypted_outputs_report_with_full_viewing_key(
    full_viewing_key_bytes: [u8; 96],
    nullifiers: &[OrchardNullifier],
    outputs: &[EncryptedShieldedOutput],
) -> Result<OrchardScanReport, OrchardVerificationError> {
    if nullifiers.len() != outputs.len() {
        return Err(OrchardVerificationError::new(
            "orchard_scan_shape_mismatch",
            format!(
                "Orchard scan requires one action nullifier per encrypted output, got {} nullifiers and {} outputs",
                nullifiers.len(),
                outputs.len()
            ),
        ));
    }

    let full_viewing_key = orchard_full_viewing_key_from_bytes(full_viewing_key_bytes)?;
    let incoming_viewing_key = full_viewing_key.to_ivk(Scope::External);
    let prepared_incoming_viewing_key = PreparedIncomingViewingKey::new(&incoming_viewing_key);
    let mut decrypted = Vec::new();
    let mut non_matching_count = 0usize;
    let mut malformed_count = 0usize;

    for (output_index, (nullifier, output)) in nullifiers.iter().zip(outputs).enumerate() {
        let stored_output = match StoredOrchardOutput::from_parts(nullifier, output) {
            Ok(stored_output) => stored_output,
            Err(_) => {
                malformed_count += 1;
                continue;
            }
        };
        let domain = OrchardDomain::for_compact_action(&stored_output.compact_domain_action);
        if let Some((note, address, memo)) =
            try_note_decryption(&domain, &prepared_incoming_viewing_key, &stored_output)
        {
            decrypted.push(OrchardDecryptedOutput {
                output_index,
                commitment: output.cmx.as_hex().to_string(),
                nullifier: bytes_to_hex(&note.nullifier(&full_viewing_key).to_bytes()),
                rho: bytes_to_hex(&note.rho().to_bytes()),
                rseed: bytes_to_hex(note.rseed().as_bytes()),
                value: note.value().inner(),
                address_raw_hex: bytes_to_hex(&address.to_raw_address_bytes()),
                memo_hex: bytes_to_hex(&memo),
            });
        } else {
            non_matching_count += 1;
        }
    }

    Ok(OrchardScanReport {
        total_output_count: outputs.len(),
        decrypted_count: decrypted.len(),
        non_matching_count,
        malformed_count,
        decrypted_outputs: decrypted,
    })
}

fn default_address_from_full_viewing_key(full_viewing_key: &FullViewingKey) -> String {
    let address = full_viewing_key.address_at(0u32, Scope::External);
    bytes_to_hex(&address.to_raw_address_bytes())
}

fn orchard_address_from_raw_hex(
    recipient_address_raw_hex: &str,
) -> Result<Address, OrchardVerificationError> {
    if recipient_address_raw_hex.len() != ORCHARD_RAW_ADDRESS_BYTES * 2
        || !recipient_address_raw_hex
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(OrchardVerificationError::new(
            "invalid_orchard_address",
            format!(
                "Orchard raw address must be {} bytes of lowercase hex",
                ORCHARD_RAW_ADDRESS_BYTES
            ),
        ));
    }
    let bytes = hex_to_bytes(recipient_address_raw_hex).map_err(|error| {
        OrchardVerificationError::new(
            "invalid_orchard_address",
            format!("Orchard address raw hex is invalid: {error}"),
        )
    })?;
    let bytes: [u8; ORCHARD_RAW_ADDRESS_BYTES] = bytes.try_into().map_err(|bytes: Vec<u8>| {
        OrchardVerificationError::new(
            "invalid_orchard_address",
            format!(
                "Orchard raw address must decode to 43 bytes, got {}",
                bytes.len()
            ),
        )
    })?;
    Option::<Address>::from(Address::from_raw_address_bytes(&bytes)).ok_or_else(|| {
        OrchardVerificationError::new(
            "invalid_orchard_address",
            "Orchard raw address bytes are not canonical",
        )
    })
}

fn orchard_full_viewing_key_from_bytes(
    full_viewing_key_bytes: [u8; 96],
) -> Result<FullViewingKey, OrchardVerificationError> {
    FullViewingKey::from_bytes(&full_viewing_key_bytes).ok_or_else(|| {
        OrchardVerificationError::new(
            "invalid_orchard_full_viewing_key",
            "Orchard full viewing key bytes are not canonical",
        )
    })
}

fn orchard_spending_key_from_bytes(
    spending_key_bytes: [u8; 32],
) -> Result<SpendingKey, OrchardVerificationError> {
    Option::<SpendingKey>::from(SpendingKey::from_bytes(spending_key_bytes)).ok_or_else(|| {
        OrchardVerificationError::new(
            "invalid_orchard_spending_key",
            "Orchard spending key bytes are not canonical",
        )
    })
}

struct StoredOrchardOutput {
    cmx: [u8; 32],
    epk: [u8; 32],
    enc_ciphertext: [u8; ENC_CIPHERTEXT_SIZE],
    compact_domain_action: orchard::note_encryption::CompactAction,
}

impl StoredOrchardOutput {
    fn from_parts(
        action_nullifier: &OrchardNullifier,
        output: &EncryptedShieldedOutput,
    ) -> Result<Self, OrchardVerificationError> {
        output.validate()?;
        let cmx = output.cmx.to_orchard()?.to_bytes();
        let epk = output.epk.to_fixed_bytes("epk")?;
        let enc_ciphertext = output
            .enc_ciphertext
            .to_fixed_bytes::<ENC_CIPHERTEXT_SIZE>("enc_ciphertext")?;
        let compact_ciphertext = enc_ciphertext
            .get(..ORCHARD_COMPACT_CIPHERTEXT_BYTES)
            .ok_or_else(|| {
                OrchardVerificationError::new(
                    "invalid_compact_ciphertext_prefix",
                    "compact ciphertext prefix length is invalid",
                )
            })?
            .try_into()
            .map_err(|_| {
                OrchardVerificationError::new(
                    "invalid_compact_ciphertext_prefix",
                    "compact ciphertext prefix length is invalid",
                )
            })?;
        let compact_domain_action = orchard::note_encryption::CompactAction::from_parts(
            action_nullifier.to_orchard()?,
            output.cmx.to_orchard()?,
            EphemeralKeyBytes(epk),
            compact_ciphertext,
        );
        Ok(Self {
            cmx,
            epk,
            enc_ciphertext,
            compact_domain_action,
        })
    }
}

impl ShieldedOutput<OrchardDomain, ENC_CIPHERTEXT_SIZE> for StoredOrchardOutput {
    fn ephemeral_key(&self) -> EphemeralKeyBytes {
        EphemeralKeyBytes(self.epk)
    }

    fn cmstar_bytes(&self) -> [u8; 32] {
        self.cmx
    }

    fn enc_ciphertext(&self) -> &[u8; ENC_CIPHERTEXT_SIZE] {
        &self.enc_ciphertext
    }
}

fn verify_bundle_signatures<V>(
    bundle: &Bundle<Authorized, V>,
    authorizing_sighash: &[u8; 32],
) -> Result<(), OrchardVerificationError>
where
    V: Copy + Into<i64>,
{
    bundle
        .binding_validating_key()
        .verify(
            authorizing_sighash,
            bundle.authorization().binding_signature(),
        )
        .map_err(|_| {
            OrchardVerificationError::new(
                "binding_signature_invalid",
                "Orchard binding signature did not verify for authorizing sighash",
            )
        })?;

    for action in bundle.actions() {
        action
            .rk()
            .verify(authorizing_sighash, action.authorization())
            .map_err(|_| {
                OrchardVerificationError::new(
                    "spend_signature_invalid",
                    "Orchard spend authorization signature did not verify for authorizing sighash",
                )
            })?;
    }

    Ok(())
}

fn extract_verified_bundle<V>(
    bundle: &Bundle<Authorized, V>,
    context: &OrchardVerificationContext,
    action_count: usize,
) -> Result<VerifiedOrchardBundle, OrchardVerificationError>
where
    V: Copy + Into<i64>,
{
    let anchor = OrchardAnchor::from_orchard(*bundle.anchor());
    let mut nullifiers = Vec::with_capacity(action_count);
    let mut randomized_verification_keys = Vec::with_capacity(action_count);
    let mut value_commitments = Vec::with_capacity(action_count);
    let mut output_commitments = Vec::with_capacity(action_count);
    let mut encrypted_outputs = Vec::with_capacity(action_count);

    for action in bundle.actions() {
        let commitment = OrchardOutputCommitment::from_orchard(*action.cmx());
        let encrypted_note = action.encrypted_note();
        encrypted_outputs.push(EncryptedShieldedOutput::from_bytes(
            commitment.clone(),
            &encrypted_note.epk_bytes,
            &encrypted_note.enc_ciphertext,
            &encrypted_note.out_ciphertext,
            None,
        )?);
        nullifiers.push(OrchardNullifier::from_orchard(*action.nullifier()));
        randomized_verification_keys
            .push(OrchardRandomizedVerificationKey::from_orchard(action.rk()));
        value_commitments.push(OrchardValueCommitment::from_orchard(action.cv_net()));
        output_commitments.push(commitment);
    }

    Ok(VerifiedOrchardBundle {
        proof_system_id: context.proof_system_id.clone(),
        circuit_id: context.circuit_id.clone(),
        flags: OrchardFlags::from_orchard(*bundle.flags()),
        anchor,
        action_count,
        nullifiers,
        randomized_verification_keys,
        value_commitments,
        output_commitments,
        encrypted_outputs,
        value_balance: (*bundle.value_balance()).into(),
    })
}

fn append_str_field(payload: &mut Vec<u8>, label: &'static str, value: &str) {
    append_bytes_field(payload, label, value.as_bytes());
}

fn append_external_binding_hash_field(
    payload: &mut Vec<u8>,
    external_binding_hash: Option<&str>,
) -> Result<(), OrchardVerificationError> {
    if let Some(hash) = external_binding_hash {
        let bytes = fixed_lower_hex_array::<{ ORCHARD_EXTERNAL_BINDING_HASH_BYTES }>(
            "external_binding_hash",
            hash,
        )?;
        append_bytes_field(payload, "external_binding_hash", &bytes);
    } else {
        append_bytes_field(payload, "external_binding_hash", &[]);
    }
    Ok(())
}

fn append_u8_field(payload: &mut Vec<u8>, label: &'static str, value: u8) {
    append_bytes_field(payload, label, &[value]);
}

fn append_u32_field(payload: &mut Vec<u8>, label: &'static str, value: u32) {
    append_bytes_field(payload, label, &value.to_be_bytes());
}

fn append_u64_field(payload: &mut Vec<u8>, label: &'static str, value: u64) {
    append_bytes_field(payload, label, &value.to_be_bytes());
}

fn append_i64_field(payload: &mut Vec<u8>, label: &'static str, value: i64) {
    append_bytes_field(payload, label, &value.to_be_bytes());
}

fn append_bytes_field(payload: &mut Vec<u8>, label: &'static str, value: &[u8]) {
    append_len_prefixed(payload, label.as_bytes());
    append_len_prefixed(payload, value);
}

fn append_len_prefixed(payload: &mut Vec<u8>, value: &[u8]) {
    payload.extend_from_slice(&(value.len() as u64).to_be_bytes());
    payload.extend_from_slice(value);
}

fn validate_domain_text(field: &'static str, value: &str) -> Result<(), OrchardVerificationError> {
    if value.trim().is_empty() {
        return Err(OrchardVerificationError::new(
            "invalid_domain_text",
            format!("{field} must be nonempty"),
        ));
    }
    if value != value.trim() {
        return Err(OrchardVerificationError::new(
            "invalid_domain_text",
            format!("{field} must not have leading or trailing whitespace"),
        ));
    }
    if value.chars().any(char::is_control) {
        return Err(OrchardVerificationError::new(
            "invalid_domain_text",
            format!("{field} must not contain control characters"),
        ));
    }
    if value.len() > MAX_DOMAIN_TEXT_BYTES {
        return Err(OrchardVerificationError::new(
            "invalid_domain_text",
            format!("{field} must not exceed {MAX_DOMAIN_TEXT_BYTES} bytes"),
        ));
    }
    Ok(())
}

fn validate_lower_hex_len(
    field: &'static str,
    value: &str,
    len: usize,
) -> Result<(), OrchardVerificationError> {
    if value.len() != len
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(OrchardVerificationError::new(
            "invalid_domain_hex",
            format!("{field} must be {len} lowercase hex characters"),
        ));
    }
    Ok(())
}

fn fixed_lower_hex_array<const N: usize>(
    field: &'static str,
    value: &str,
) -> Result<[u8; N], OrchardVerificationError> {
    validate_lower_hex_len(field, value, N * 2)?;
    let bytes = hex_to_bytes(value).map_err(|error| {
        OrchardVerificationError::new("invalid_hex", format!("{field} has invalid hex: {error}"))
    })?;
    bytes.try_into().map_err(|bytes: Vec<u8>| {
        OrchardVerificationError::new(
            "invalid_hex_length",
            format!("{field} decoded to {} bytes, expected {N}", bytes.len()),
        )
    })
}

#[cfg(test)]
mod tests {
    use orchard::{
        builder::{Builder, BundleType, UnauthorizedBundle},
        circuit::{ProvingKey, VerifyingKey},
        keys::{FullViewingKey, Scope, SpendingKey},
        value::NoteValue,
        Anchor, Bundle,
    };
    use rand::{
        rngs::{OsRng, StdRng},
        RngCore, SeedableRng,
    };

    use super::*;

    #[test]
    fn legacy_vk_ids_are_archive_only_at_the_verifier_policy_boundary() {
        validate_asset_orchard_swap_vk_policy(
            ASSET_ORCHARD_CIRCUIT_ID_V1,
            AssetOrchardVkVerificationPolicy::LiveCurrent,
        )
        .expect("current swap circuit is live");
        assert_eq!(
            validate_asset_orchard_swap_vk_policy(
                crate::ASSET_ORCHARD_CIRCUIT_ID_V3_REPLAY,
                AssetOrchardVkVerificationPolicy::LiveCurrent,
            )
            .expect_err("legacy swap circuit must be replay-only")
            .code(),
            "asset_orchard_legacy_circuit_replay_only"
        );
        validate_asset_orchard_swap_vk_policy(
            crate::ASSET_ORCHARD_CIRCUIT_ID_V3_REPLAY,
            AssetOrchardVkVerificationPolicy::ArchiveReplay,
        )
        .expect("legacy swap circuit is available to archive replay");

        validate_asset_orchard_private_egress_vk_policy(
            ASSET_ORCHARD_PRIVATE_EGRESS_CIRCUIT_ID_V1,
            AssetOrchardVkVerificationPolicy::LiveCurrent,
        )
        .expect("current private-egress circuit is live");
        assert_eq!(
            validate_asset_orchard_private_egress_vk_policy(
                crate::ASSET_ORCHARD_PRIVATE_EGRESS_CIRCUIT_ID_V1_REPLAY,
                AssetOrchardVkVerificationPolicy::LiveCurrent,
            )
            .expect_err("legacy private-egress circuit must be replay-only")
            .code(),
            "asset_orchard_private_egress_legacy_circuit_replay_only"
        );
        validate_asset_orchard_private_egress_vk_policy(
            crate::ASSET_ORCHARD_PRIVATE_EGRESS_CIRCUIT_ID_V1_REPLAY,
            AssetOrchardVkVerificationPolicy::ArchiveReplay,
        )
        .expect("legacy private-egress circuit is available to archive replay");
    }

    #[test]
    fn pricing_policy_is_provenance_neutral_and_fails_closed_on_band_epoch_packet() {
        let base = AssetTag::derive("a651").unwrap();
        let quote = AssetTag::derive("pfUSDC").unwrap();
        let claim = AssetOrchardPricingClaim {
            nav_epoch: 59,
            reserve_packet_hash: "ab".repeat(48),
            ratio_numerator: 9,
            ratio_denominator: 5,
            mode: "at_nav_with_band".to_string(),
            band_bps: 0,
            base_asset_tag_lo: base.lo,
            base_asset_tag_hi: base.hi,
            quote_asset_tag_lo: quote.lo,
            quote_asset_tag_hi: quote.hi,
        };
        let evidence = VerifiedAssetOrchardPricingClaim {
            claim: claim.clone(),
            action_binding_hash: AssetOrchardSwapBindingHash::from_bytes(&[7; 64]),
            provenance: AssetOrchardPricingClaimProvenance::CircuitProven,
        };
        let policy = AssetOrchardPricingPolicy {
            nav_epoch: 59,
            reserve_packet_hash: "ab".repeat(48),
            nav_ratio_numerator: 9,
            nav_ratio_denominator: 5,
            band_bps: 0,
            base_asset_tag: base,
            quote_asset_tag: quote,
            halted: false,
        };
        validate_asset_orchard_pricing_policy(&evidence, &policy).unwrap();

        let mut off_band = evidence.clone();
        off_band.claim.ratio_numerator = 10;
        assert_eq!(
            validate_asset_orchard_pricing_policy(&off_band, &policy)
                .unwrap_err()
                .code(),
            "asset_orchard_pricing_off_band"
        );
        let mut wrong_epoch = evidence.clone();
        wrong_epoch.claim.nav_epoch += 1;
        assert_eq!(
            validate_asset_orchard_pricing_policy(&wrong_epoch, &policy)
                .unwrap_err()
                .code(),
            "asset_orchard_pricing_epoch_mismatch"
        );
        let mut wrong_packet = evidence;
        wrong_packet.claim.reserve_packet_hash = "cd".repeat(48);
        assert_eq!(
            validate_asset_orchard_pricing_policy(&wrong_packet, &policy)
                .unwrap_err()
                .code(),
            "asset_orchard_pricing_packet_mismatch"
        );
    }

    const TEST_SIGHASH: [u8; 32] = [42u8; 32];

    #[test]
    fn context_rejects_zero_action_bound() {
        let mut context = OrchardVerificationContext::production_v2(TEST_SIGHASH);
        context.max_actions = 0;

        assert_eq!(
            context
                .validate()
                .expect_err("zero action bound must fail")
                .code(),
            "invalid_action_bound"
        );
    }

    #[test]
    fn authorizing_sighash_is_deterministic_and_domain_separated() {
        let domain = test_domain();
        let bundle = generated_unsigned_output_bundle(StdRng::from_seed([3u8; 32]));

        let sighash = orchard_authorizing_sighash(&domain, 0, &bundle).expect("sighash");
        assert_eq!(
            sighash,
            orchard_authorizing_sighash(&domain, 0, &bundle).expect("repeat sighash")
        );
        assert_ne!(
            sighash,
            orchard_authorizing_sighash(&domain, 1, &bundle).expect("different fee sighash")
        );
        assert_ne!(
            sighash,
            orchard_authorizing_sighash_with_external_binding(
                &domain,
                0,
                Some(&"11".repeat(ORCHARD_EXTERNAL_BINDING_HASH_BYTES)),
                &bundle,
            )
            .expect("external-bound sighash")
        );

        let other_domain =
            OrchardAuthorizingDomain::new("postfiat-test", "b".repeat(96), 1, "orchard-v1")
                .expect("other domain");
        assert_ne!(
            sighash,
            orchard_authorizing_sighash(&other_domain, 0, &bundle).expect("other sighash")
        );
    }

    #[test]
    fn verify_adapter_accepts_generated_output_bundle_and_rejects_wrong_domain() {
        let verifying_key = VerifyingKey::build();
        let proving_key = ProvingKey::build();
        let domain = test_domain();
        let bundle = generated_output_bundle(&proving_key, &domain);
        let context = OrchardVerificationContext::for_bundle(&domain, 1, &bundle).expect("context");

        let verified =
            verify_authorized_bundle(&bundle, &verifying_key, &context).expect("verified bundle");

        assert_eq!(verified.action_count, 2);
        assert_eq!(verified.nullifiers.len(), 2);
        assert_eq!(verified.randomized_verification_keys.len(), 2);
        assert_eq!(verified.value_commitments.len(), 2);
        assert_eq!(verified.output_commitments.len(), 2);
        assert_eq!(verified.encrypted_outputs.len(), 2);
        assert_eq!(verified.value_balance, -10);
        let scanned = orchard_scan_encrypted_outputs_with_spending_key(
            [7u8; 32],
            &verified.nullifiers,
            &verified.encrypted_outputs,
        )
        .expect("scan outputs");
        assert_eq!(scanned.len(), 1);
        assert_eq!(scanned[0].value, 10);
        assert_eq!(scanned[0].nullifier.len(), 64);
        let scan_report = orchard_scan_encrypted_outputs_report_with_spending_key(
            [7u8; 32],
            &verified.nullifiers,
            &verified.encrypted_outputs,
        )
        .expect("scan report");
        assert_eq!(scan_report.total_output_count, 2);
        assert_eq!(scan_report.decrypted_count, 1);
        assert_eq!(scan_report.non_matching_count, 1);
        assert_eq!(scan_report.malformed_count, 0);
        assert_eq!(scan_report.decrypted_outputs, scanned);
        let mut malformed_outputs = verified.encrypted_outputs.clone();
        malformed_outputs[scanned[0].output_index].enc_ciphertext =
            crate::BoundedHexBlob::from_bytes(&[1u8], crate::ORCHARD_ENC_CIPHERTEXT_BYTES)
                .expect("short malformed ciphertext blob");
        let malformed_report = orchard_scan_encrypted_outputs_report_with_spending_key(
            [7u8; 32],
            &verified.nullifiers,
            &malformed_outputs,
        )
        .expect("malformed scan report");
        assert_eq!(malformed_report.total_output_count, 2);
        assert_eq!(malformed_report.malformed_count, 1);
        assert_eq!(malformed_report.decrypted_count, 0);
        assert_eq!(malformed_report.non_matching_count, 1);
        let stored_output_result = std::panic::catch_unwind(|| {
            StoredOrchardOutput::from_parts(
                &verified.nullifiers[scanned[0].output_index],
                &malformed_outputs[scanned[0].output_index],
            )
        });
        assert!(
            stored_output_result.is_ok(),
            "malformed output conversion must return Err, not panic"
        );
        let stored_output_error = match stored_output_result.expect("catch_unwind result") {
            Ok(_) => panic!("malformed output must be rejected"),
            Err(error) => error,
        };
        assert_eq!(stored_output_error.code(), "invalid_blob_length");
        let full_viewing_key =
            orchard_full_viewing_key_from_spending_key([7u8; 32]).expect("full viewing key");
        assert_eq!(
            scanned,
            orchard_scan_encrypted_outputs_with_full_viewing_key(
                full_viewing_key,
                &verified.nullifiers,
                &verified.encrypted_outputs,
            )
            .expect("scan with full viewing key")
        );
        assert_eq!(
            orchard_default_address_from_spending_key([7u8; 32]).expect("spending address"),
            orchard_default_address_from_full_viewing_key(full_viewing_key)
                .expect("viewing address")
        );
        let witness = orchard_merkle_witness_from_commitments(
            &verified.output_commitments,
            scanned[0].output_index,
        )
        .expect("spend witness");
        let spend_note = OrchardSpendNote {
            output_index: scanned[0].output_index,
            commitment: scanned[0].commitment.clone(),
            address_raw_hex: scanned[0].address_raw_hex.clone(),
            value: scanned[0].value,
            rho: scanned[0].rho.clone(),
            rseed: scanned[0].rseed.clone(),
            merkle_position: witness.position,
            witness_anchor: witness.anchor.clone(),
            witness_auth_path: witness.auth_path.clone(),
        };
        let spend_action = orchard_build_spend_action(
            &domain,
            "orchard-v1",
            0,
            OrchardAnchor::parse_hex(witness.anchor).expect("witness anchor"),
            [7u8; 32],
            &spend_note,
            &scanned[0].address_raw_hex,
            spend_note.value,
            None,
            [1u8; ORCHARD_MEMO_BYTES],
        )
        .expect("spend action");
        let verified_spend =
            verify_serialized_orchard_action_with_built_key(&spend_action, &domain)
                .expect("verified spend action");
        assert_eq!(verified_spend.value_balance, 0);
        assert_eq!(verified_spend.output_commitments.len(), 2);
        assert!(verified_spend
            .nullifiers
            .iter()
            .any(|nullifier| nullifier.as_hex() == scanned[0].nullifier));

        let action = orchard_action_from_authorized_bundle("orchard-v1", 1, &bundle)
            .expect("serialized action");
        let json = serde_json::to_string(&action).expect("serialize action");
        let parsed: OrchardShieldedAction = serde_json::from_str(&json).expect("parse action");
        parsed.validate().expect("valid parsed action");
        assert_eq!(parsed.flags, verified.flags);
        assert_eq!(parsed.anchor, verified.anchor);
        assert_eq!(parsed.nullifiers, verified.nullifiers);
        assert_eq!(
            parsed.randomized_verification_keys,
            verified.randomized_verification_keys
        );
        assert_eq!(parsed.value_commitments, verified.value_commitments);
        assert_eq!(parsed.output_commitments, verified.output_commitments);
        assert_eq!(parsed.encrypted_outputs, verified.encrypted_outputs);

        let reconstructed = orchard_bundle_from_action(&parsed).expect("reconstructed bundle");
        assert_eq!(
            orchard_authorizing_sighash(&domain, parsed.fee, &reconstructed)
                .expect("reconstructed sighash"),
            orchard_authorizing_sighash(&domain, action.fee, &bundle).expect("original sighash")
        );
        let mut padded_proof_action = parsed.clone();
        let mut padded_proof = padded_proof_action.proof.to_bytes().expect("proof bytes");
        padded_proof.push(0);
        padded_proof_action.proof =
            OrchardProofBytes::from_bytes(&padded_proof).expect("bounded padded proof");
        assert_eq!(
            orchard_bundle_from_action(&padded_proof_action)
                .expect_err("padded proof must fail before verification")
                .code(),
            "invalid_orchard_bundle"
        );
        let verified_from_serialized =
            verify_serialized_orchard_action(&parsed, &verifying_key, &domain)
                .expect("verified serialized action");
        assert_eq!(verified_from_serialized, verified);
        let mut mutated_fee = parsed.clone();
        mutated_fee.fee = mutated_fee.fee.saturating_add(1);
        assert_eq!(
            verify_serialized_orchard_action(&mutated_fee, &verifying_key, &domain)
                .expect_err("mutated action fee must fail")
                .code(),
            "binding_signature_invalid"
        );

        let external_binding_hash = "22".repeat(ORCHARD_EXTERNAL_BINDING_HASH_BYTES);
        let unsigned_external_bundle =
            generated_unsigned_output_bundle(StdRng::from_seed([31u8; 32]));
        let external_sighash = orchard_authorizing_sighash_with_external_binding(
            &domain,
            1,
            Some(&external_binding_hash),
            &unsigned_external_bundle,
        )
        .expect("external binding sighash");
        let external_bundle = unsigned_external_bundle
            .create_proof(&proving_key, OsRng)
            .expect("create external-bound proof")
            .apply_signatures(OsRng, external_sighash, &[])
            .expect("apply external-bound signatures");
        let external_action = orchard_action_from_authorized_bundle_with_external_binding(
            "orchard-v1",
            1,
            Some(external_binding_hash.clone()),
            &external_bundle,
        )
        .expect("serialized external-bound action");
        verify_serialized_orchard_action(&external_action, &verifying_key, &domain)
            .expect("verified external-bound action");
        let mut mutated_external = external_action.clone();
        mutated_external.external_binding_hash =
            Some("33".repeat(ORCHARD_EXTERNAL_BINDING_HASH_BYTES));
        assert_eq!(
            verify_serialized_orchard_action(&mutated_external, &verifying_key, &domain)
                .expect_err("mutated external binding hash must fail")
                .code(),
            "binding_signature_invalid"
        );
        let mut removed_external = external_action;
        removed_external.external_binding_hash = None;
        assert_eq!(
            verify_serialized_orchard_action(&removed_external, &verifying_key, &domain)
                .expect_err("removed external binding hash must fail")
                .code(),
            "binding_signature_invalid"
        );

        let wrong_pool_domain =
            OrchardAuthorizingDomain::new("postfiat-test", "a".repeat(96), 1, "other-pool")
                .expect("wrong pool domain");
        assert_eq!(
            verify_serialized_orchard_action(&parsed, &verifying_key, &wrong_pool_domain)
                .expect_err("wrong pool must fail")
                .code(),
            "pool_id_mismatch"
        );

        assert_eq!(
            {
                let wrong_domain =
                    OrchardAuthorizingDomain::new("postfiat-test", "c".repeat(96), 1, "orchard-v1")
                        .expect("wrong domain");
                let wrong_context =
                    OrchardVerificationContext::for_bundle(&wrong_domain, action.fee, &bundle)
                        .expect("wrong context");
                verify_authorized_bundle(&bundle, &verifying_key, &wrong_context)
            }
            .expect_err("wrong authorizing domain must fail")
            .code(),
            "binding_signature_invalid"
        );

        assert_eq!(
            verify_authorized_bundle(
                &bundle,
                &verifying_key,
                &OrchardVerificationContext::production_v2([7u8; 32]),
            )
            .expect_err("wrong authorizing sighash must fail")
            .code(),
            "binding_signature_invalid"
        );
    }

    #[test]
    fn orchard_anchor_from_commitments_matches_empty_anchor_and_changes_after_outputs() {
        let empty = orchard_anchor_from_commitments(&[]).expect("empty root");
        assert_eq!(empty, orchard_empty_anchor());

        let unsigned_bundle = generated_unsigned_output_bundle(StdRng::from_seed([17u8; 32]));
        let commitments = unsigned_bundle
            .actions()
            .iter()
            .map(|action| OrchardOutputCommitment::from_orchard(*action.cmx()))
            .collect::<Vec<_>>();

        let root = orchard_anchor_from_commitments(&commitments).expect("output root");
        assert_ne!(root, empty);
        assert_eq!(
            root,
            orchard_anchor_from_commitments(&commitments).expect("repeat output root")
        );
        let witness =
            orchard_merkle_witness_from_commitments(&commitments, 0).expect("first witness");
        assert_eq!(witness.position, 0);
        assert_eq!(witness.anchor, root.as_hex());
        assert_eq!(witness.output_count, commitments.len() as u64);
        assert_eq!(witness.auth_path.len(), usize::from(ORCHARD_TREE_DEPTH));
        assert_eq!(
            orchard_merkle_witness_from_commitments(&commitments, commitments.len())
                .expect_err("out-of-range witness must fail")
                .code(),
            "orchard_witness_position_out_of_range"
        );
    }

    #[test]
    fn orchard_frontier_snapshot_incremental_root_matches_full_after_each_append() {
        let mut commitments = Vec::new();
        for seed in 17_u8..23 {
            let unsigned_bundle = generated_unsigned_output_bundle(StdRng::from_seed([seed; 32]));
            commitments.extend(
                unsigned_bundle
                    .actions()
                    .iter()
                    .map(|action| OrchardOutputCommitment::from_orchard(*action.cmx())),
            );
        }

        let mut snapshot =
            orchard_frontier_snapshot_from_commitments(&[]).expect("empty frontier snapshot");
        let mut prefix = Vec::new();
        for commitment in commitments {
            prefix.push(commitment.clone());
            snapshot = orchard_frontier_snapshot_append_commitments(Some(&snapshot), &[commitment])
                .expect("incremental frontier append");
            let full_root = orchard_anchor_from_commitments(&prefix)
                .expect("full recompute root")
                .as_hex()
                .to_string();
            assert_eq!(snapshot.output_count, prefix.len() as u64);
            assert_eq!(snapshot.root, full_root);
            assert_eq!(snapshot.latest_leaf.is_some(), !prefix.is_empty());
        }
    }

    #[test]
    fn orchard_frontier_snapshot_rejects_malformed_cached_root() {
        let unsigned_bundle = generated_unsigned_output_bundle(StdRng::from_seed([24u8; 32]));
        let commitments = unsigned_bundle
            .actions()
            .iter()
            .map(|action| OrchardOutputCommitment::from_orchard(*action.cmx()))
            .collect::<Vec<_>>();
        let mut snapshot =
            orchard_frontier_snapshot_from_commitments(&commitments).expect("frontier snapshot");
        snapshot.root = "00".repeat(32);

        assert_eq!(
            orchard_frontier_snapshot_append_commitments(Some(&snapshot), &[])
                .expect_err("bad cache root must fail")
                .code(),
            "orchard_frontier_cache_root_mismatch"
        );
    }

    #[test]
    fn shielded_swap_builder_hides_private_witness_but_consensus_verifier_fails_closed() {
        let domain = swap_test_domain();
        let action = valid_swap_action(&domain, orchard_empty_anchor(), "binding-good")
            .expect("valid shielded swap");

        let json = serde_json::to_string(&action).expect("serialize swap action");
        assert!(!json.contains("asset-a"));
        assert!(!json.contains("asset-b"));
        assert!(!json.contains("auth-secret"));
        assert!(!json.contains("blinding"));

        assert_eq!(
            verify_serialized_shielded_swap_action(&action, &domain)
                .expect_err("test-vector transcript must not be accepted as consensus proof")
                .code(),
            "shielded_swap_proof_verifier_unimplemented"
        );
    }

    #[test]
    fn shielded_swap_builder_rejects_value_and_asset_non_conservation() {
        let domain = swap_test_domain();
        let anchor = orchard_empty_anchor();
        let inputs = valid_swap_inputs();

        let value_error = shielded_swap_build_action_test_vector(
            &domain,
            domain.pool_id.clone(),
            anchor.clone(),
            inputs.clone(),
            [
                swap_output("asset-b", 20, "out-b"),
                swap_output("asset-a", 30, "out-a"),
            ],
            "binding-bad-value",
            0,
        )
        .expect_err("non-conserved value must fail");
        assert_eq!(value_error.code(), "value_conservation_failed");

        let asset_error = shielded_swap_build_action_test_vector(
            &domain,
            domain.pool_id.clone(),
            anchor,
            inputs,
            [
                swap_output("asset-c", 70, "out-c"),
                swap_output("asset-a", 50, "out-a"),
            ],
            "binding-bad-asset",
            0,
        )
        .expect_err("non-conserved asset must fail");
        assert_eq!(asset_error.code(), "asset_conservation_failed");
    }

    #[test]
    fn shielded_swap_builder_rejects_unauthorized_input() {
        let domain = swap_test_domain();
        let mut inputs = valid_swap_inputs();
        inputs[0].authorization_proof = "00".repeat(48);

        let error = shielded_swap_build_action_test_vector(
            &domain,
            domain.pool_id.clone(),
            orchard_empty_anchor(),
            inputs,
            valid_swap_outputs(),
            "binding-unauthorized",
            0,
        )
        .expect_err("bad authorization proof must fail");
        assert_eq!(error.code(), "unauthorized_shielded_swap_input");
    }

    #[test]
    fn shielded_swap_verifier_fails_closed_before_transcript_checks() {
        let domain = swap_test_domain();
        let mut action = valid_swap_action(&domain, orchard_empty_anchor(), "binding-tamper")
            .expect("valid shielded swap");

        let wrong_domain =
            OrchardAuthorizingDomain::new("postfiat-test", "b".repeat(96), 1, "orchard-swap")
                .expect("wrong domain");
        assert_eq!(
            verify_serialized_shielded_swap_action(&action, &wrong_domain)
                .expect_err("legacy verifier must fail closed before transcript checks")
                .code(),
            "shielded_swap_proof_verifier_unimplemented"
        );

        action.swap_binding_hash =
            ShieldedSwapCommitment::parse_hex("11".repeat(48)).expect("replacement binding");
        assert_eq!(
            verify_serialized_shielded_swap_action(&action, &domain)
                .expect_err("legacy verifier must fail closed before transcript checks")
                .code(),
            "shielded_swap_proof_verifier_unimplemented"
        );
    }

    fn generated_output_bundle(
        proving_key: &ProvingKey,
        domain: &OrchardAuthorizingDomain,
    ) -> Bundle<Authorized, i64> {
        let unsigned_bundle = generated_unsigned_output_bundle(OsRng);
        let sighash = orchard_authorizing_sighash(domain, 1, &unsigned_bundle).expect("sighash");
        unsigned_bundle
            .create_proof(proving_key, OsRng)
            .expect("create proof")
            .apply_signatures(OsRng, sighash, &[])
            .expect("apply signatures")
    }

    fn generated_unsigned_output_bundle(rng: impl RngCore) -> UnauthorizedBundle<i64> {
        let spending_key = SpendingKey::from_bytes([7u8; 32]).unwrap();
        let recipient = FullViewingKey::from(&spending_key).address_at(0u32, Scope::External);
        let mut builder = Builder::new(BundleType::DEFAULT, Anchor::empty_tree());
        builder
            .add_output(None, recipient, NoteValue::from_raw(10), [0u8; 512])
            .expect("add output");

        let (bundle, _) = builder
            .build::<i64>(rng)
            .expect("build bundle")
            .expect("bundle should be present");
        bundle
    }

    fn test_domain() -> OrchardAuthorizingDomain {
        OrchardAuthorizingDomain::new("postfiat-test", "a".repeat(96), 1, "orchard-v1")
            .expect("test domain")
    }

    fn swap_test_domain() -> OrchardAuthorizingDomain {
        OrchardAuthorizingDomain::new("postfiat-test", "a".repeat(96), 1, "orchard-swap")
            .expect("swap test domain")
    }

    fn valid_swap_action(
        domain: &OrchardAuthorizingDomain,
        anchor: OrchardAnchor,
        binding_nonce: &str,
    ) -> Result<ShieldedSwapAction, OrchardVerificationError> {
        shielded_swap_build_action_test_vector(
            domain,
            domain.pool_id.clone(),
            anchor,
            valid_swap_inputs(),
            valid_swap_outputs(),
            binding_nonce,
            0,
        )
    }

    fn valid_swap_inputs() -> [ShieldedSwapPrivateInput; 2] {
        [
            swap_input("asset-a", 50, "in-a"),
            swap_input("asset-b", 70, "in-b"),
        ]
    }

    fn valid_swap_outputs() -> [ShieldedSwapPrivateOutput; 2] {
        [
            swap_output("asset-b", 70, "out-b"),
            swap_output("asset-a", 50, "out-a"),
        ]
    }

    fn swap_input(asset_id: &str, value: u64, tag: &str) -> ShieldedSwapPrivateInput {
        let authorization_secret = format!("{tag}-auth-secret");
        ShieldedSwapPrivateInput {
            asset_id: asset_id.to_string(),
            value,
            asset_blinding: format!("{tag}-asset-blinding"),
            value_blinding: format!("{tag}-value-blinding"),
            authorization_proof: shielded_swap_authorization_proof(
                asset_id,
                value,
                &authorization_secret,
            )
            .expect("authorization proof"),
            authorization_secret,
        }
    }

    fn swap_output(asset_id: &str, value: u64, tag: &str) -> ShieldedSwapPrivateOutput {
        ShieldedSwapPrivateOutput {
            asset_id: asset_id.to_string(),
            value,
            asset_blinding: format!("{tag}-asset-blinding"),
            value_blinding: format!("{tag}-value-blinding"),
        }
    }
}
