//! Provider-neutral, deterministic NAV reserve-proof manifest and guest logic.

use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use postfiat_types::{
    NavReservePublicValuesV1, NavReserveTrustCountsV1, NAV_RESERVE_PUBLIC_VALUES_SCHEMA_V1,
};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_384};

#[cfg(feature = "a666-public-adapters-v2")]
pub mod aave_v3;
#[cfg(feature = "a666-public-adapters-v2")]
pub mod bft_checkpoint;
pub mod evm_checkpoint;
#[cfg(feature = "a666-public-adapters-v2")]
pub mod evm_spot;
#[cfg(feature = "a666-public-adapters-v2")]
pub mod hyperliquid_receipt;
#[cfg(feature = "a666-public-adapters-v2")]
pub mod near_receipt;
#[cfg(feature = "a666-public-adapters-v2")]
pub mod solana_stake;

#[cfg(feature = "a666-public-adapters-v2")]
use aave_v3::{
    verify_aave_v3_proof_v1, AaveV3ProofV1, AaveV3VerifyContextV1, AAVE_V3_ADAPTER_KIND_V1,
};
use evm_checkpoint::{EvmErc20BalanceProofV1, EVM_ERC20_ADAPTER_KIND_V1};
#[cfg(feature = "a666-public-adapters-v2")]
use evm_spot::{
    verify_evm_spot_quantity_proof_v1, EvmSpotQuantityProofV1, EvmSpotVerifyContextV1,
    EVM_SPOT_ADAPTER_KIND_V1,
};
#[cfg(feature = "a666-public-adapters-v2")]
use hyperliquid_receipt::{
    verify_hyperliquid_receipt_proof_v1, HyperliquidReceiptProofV1,
    HyperliquidReceiptVerifyContextV1, HYPERLIQUID_RECEIPT_ADAPTER_KIND_V1,
};
#[cfg(feature = "a666-public-adapters-v2")]
use near_receipt::{
    verify_near_receipt_quantity_proof_v1, NearReceiptQuantityProofV1, NearReceiptVerifyContextV1,
    NEAR_RECEIPT_QUANTITY_ADAPTER_KIND_V1,
};
#[cfg(feature = "a666-public-adapters-v2")]
use solana_stake::{
    verify_solana_stake_attested_proof_v1, SolanaStakeAttestedProofV1, SolanaStakeVerifyContextV1,
    SOLANA_STAKE_ADAPTER_KIND_V1,
};

pub const MANIFEST_SCHEMA_V1: &str = "postfiat.reserve_source_manifest.v1";
pub const WITNESS_SCHEMA_V1: &str = "postfiat.reserve_proof_witness.v1";
pub const MAX_SOURCES: usize = 64;
pub const MAX_TEXT_BYTES: usize = 256;
pub const MAX_EVIDENCE_BYTES: usize = 16 * 1024;
pub const MAX_WITNESS_BYTES: usize = 8 * 1024 * 1024;

const MANIFEST_HASH_DOMAIN: &[u8] = b"postfiat.reserve_source_manifest_hash.v1";
const OBSERVATION_HASH_DOMAIN: &[u8] = b"postfiat.reserve_source_observation.v1";
const OBSERVATION_ROOT_DOMAIN: &[u8] = b"postfiat.reserve_source_observation_root.v1";
const QUANTITY_TRUST_ROOT_DOMAIN: &[u8] = b"postfiat.reserve_quantity_trust_root.v1";
const VALUATION_TRUST_ROOT_DOMAIN: &[u8] = b"postfiat.reserve_valuation_trust_root.v1";
const DISCLOSURE_ROOT_DOMAIN: &[u8] = b"postfiat.reserve_source_disclosure_root.v1";
const ATTESTATION_DOMAIN: &[u8] = b"postfiat.reserve_source_attestation.v1";
const PROTOCOL_RECEIPT_DOMAIN: &[u8] = b"postfiat.reserve_protocol_receipt.v1";
const ED25519_VERIFIER_COMMITMENT_DOMAIN: &[u8] =
    b"postfiat.reserve_ed25519_verifier_commitment.v1";
const OPAQUE_COMMITMENT_DOMAIN: &[u8] = b"postfiat.reserve_opaque_commitment.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustClassV1 {
    Cryptographic,
    Attested,
    Controlled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceDimensionV1 {
    Quantity,
    Valuation,
}

impl EvidenceDimensionV1 {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Quantity => "quantity",
            Self::Valuation => "valuation",
        }
    }
}

impl TrustClassV1 {
    fn tag(self) -> u8 {
        match self {
            Self::Cryptographic => 1,
            Self::Attested => 2,
            Self::Controlled => 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LiabilityTreatmentV1 {
    Asset,
    Liability,
}

impl LiabilityTreatmentV1 {
    fn tag(self) -> u8 {
        match self {
            Self::Asset => 1,
            Self::Liability => 2,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FreshnessPolicyV1 {
    pub max_age_blocks: u64,
    pub max_observation_span_blocks: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceManifestEntryV1 {
    pub source_id: String,
    pub adapter_kind: String,
    pub source_domain: String,
    pub asset_or_position_id: String,
    pub reserve_owner_commitment: String,
    /// Verifier authority for the quantity evidence. This is separate from
    /// reserve ownership and from the valuation authority.
    pub quantity_verifier_commitment: String,
    /// Verifier authority for prices, marks, and haircuts.
    pub valuation_verifier_commitment: String,
    pub quantity_evidence_class: TrustClassV1,
    pub valuation_evidence_class: TrustClassV1,
    pub freshness_policy: FreshnessPolicyV1,
    pub haircut_policy_hash: String,
    pub liability_treatment: LiabilityTreatmentV1,
    pub adapter_schema_version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceManifestV1 {
    pub schema: String,
    pub sources: Vec<SourceManifestEntryV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SourceEvidenceV1 {
    Controlled {
        evidence_commitment: String,
    },
    AttestedEd25519 {
        evidence_commitment: String,
        verifier_public_key: String,
        signature: String,
    },
    /// A protocol-generated signed receipt. The manifest must select the
    /// `ed25519-protocol-receipt-v1` adapter and pin its protocol key.
    ProtocolReceiptEd25519 {
        evidence_commitment: String,
        verifier_public_key: String,
        signature: String,
    },
    /// An ERC-20 balance proven by Ethereum MPT account/storage proofs under a
    /// quorum-signed, manifest-pinned EVM state checkpoint.
    EvmErc20BftCheckpointMpt {
        evidence_commitment: String,
        proof: Box<EvmErc20BalanceProofV1>,
    },
    /// HyperCore state and prices emitted by a governed reader contract in a
    /// HyperEVM receipt included under a quorum-certified block header.
    #[cfg(feature = "a666-public-adapters-v2")]
    HyperliquidReceipt {
        evidence_commitment: String,
        proof: Box<HyperliquidReceiptProofV1>,
    },
    /// Staked and unstaked yoctoNEAR proven by a reader callback receipt and
    /// Merkle paths beneath a quorum-certified NEAR head. Valuation remains a
    /// separate evidence dimension.
    #[cfg(feature = "a666-public-adapters-v2")]
    NearReceiptQuantity {
        evidence_commitment: String,
        proof: Box<NearReceiptQuantityProofV1>,
    },
    /// Aave V3 collateral, debt, reserve-index, and oracle state proven
    /// beneath a quorum-certified EVM state root.
    #[cfg(feature = "a666-public-adapters-v2")]
    AaveV3 {
        evidence_commitment: String,
        proof: Box<AaveV3ProofV1>,
    },
    /// Exact governed native/ERC-20 spot positions across one or more EVM
    /// chains, proven beneath quorum-certified state roots. Prices remain a
    /// separate valuation evidence dimension.
    #[cfg(feature = "a666-public-adapters-v2")]
    EvmSpotQuantity {
        evidence_commitment: String,
        proof: Box<EvmSpotQuantityProofV1>,
    },
    /// Publicly specified and independently signed Solana stake snapshots.
    /// The manifest truthfully classifies this quantity evidence as attested.
    #[cfg(feature = "a666-public-adapters-v2")]
    SolanaStakeAttested {
        evidence_commitment: String,
        proof: Box<SolanaStakeAttestedProofV1>,
    },
    AdapterProof {
        evidence_commitment: String,
        proof: Vec<u8>,
    },
}

impl SourceEvidenceV1 {
    pub fn class(&self) -> TrustClassV1 {
        match self {
            Self::Controlled { .. } => TrustClassV1::Controlled,
            Self::AttestedEd25519 { .. } => TrustClassV1::Attested,
            Self::ProtocolReceiptEd25519 { .. }
            | Self::EvmErc20BftCheckpointMpt { .. }
            | Self::AdapterProof { .. } => TrustClassV1::Cryptographic,
            #[cfg(feature = "a666-public-adapters-v2")]
            Self::HyperliquidReceipt { .. }
            | Self::NearReceiptQuantity { .. }
            | Self::AaveV3 { .. }
            | Self::EvmSpotQuantity { .. } => TrustClassV1::Cryptographic,
            #[cfg(feature = "a666-public-adapters-v2")]
            Self::SolanaStakeAttested { .. } => TrustClassV1::Attested,
        }
    }

    fn commitment(&self) -> &str {
        match self {
            Self::Controlled {
                evidence_commitment,
            }
            | Self::AttestedEd25519 {
                evidence_commitment,
                ..
            }
            | Self::ProtocolReceiptEd25519 {
                evidence_commitment,
                ..
            }
            | Self::EvmErc20BftCheckpointMpt {
                evidence_commitment,
                ..
            }
            | Self::AdapterProof {
                evidence_commitment,
                ..
            } => evidence_commitment,
            #[cfg(feature = "a666-public-adapters-v2")]
            Self::HyperliquidReceipt {
                evidence_commitment,
                ..
            }
            | Self::NearReceiptQuantity {
                evidence_commitment,
                ..
            }
            | Self::AaveV3 {
                evidence_commitment,
                ..
            }
            | Self::EvmSpotQuantity {
                evidence_commitment,
                ..
            } => evidence_commitment,
            #[cfg(feature = "a666-public-adapters-v2")]
            Self::SolanaStakeAttested {
                evidence_commitment,
                ..
            } => evidence_commitment,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceObservationV1 {
    pub source_id: String,
    pub observed_at_block: u64,
    pub gross_assets: u64,
    pub total_liabilities: u64,
    pub quantity_evidence: SourceEvidenceV1,
    pub valuation_evidence: SourceEvidenceV1,
    pub disclosure_commitment: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReserveProofContextV1 {
    pub pftl_genesis_hash: String,
    pub nav_asset_id: String,
    pub proof_profile_id: String,
    pub valuation_policy_hash: String,
    pub source_manifest_hash: String,
    pub valuation_unit_id: String,
    pub valuation_scale: u64,
    pub observation_epoch: u64,
    pub observation_not_before: u64,
    pub observation_not_after: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReserveProofWitnessV1 {
    pub schema: String,
    pub context: ReserveProofContextV1,
    pub manifest: SourceManifestV1,
    pub observations: Vec<SourceObservationV1>,
}

impl SourceManifestV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != MANIFEST_SCHEMA_V1 {
            return Err("reserve manifest schema mismatch".to_string());
        }
        if self.sources.is_empty() || self.sources.len() > MAX_SOURCES {
            return Err(format!(
                "reserve manifest source count must be in 1..={MAX_SOURCES}"
            ));
        }
        let mut previous: Option<&str> = None;
        for source in &self.sources {
            validate_identifier("source_id", &source.source_id)?;
            validate_identifier("adapter_kind", &source.adapter_kind)?;
            validate_identifier("source_domain", &source.source_domain)?;
            validate_text("asset_or_position_id", &source.asset_or_position_id)?;
            validate_hex(
                "reserve_owner_commitment",
                &source.reserve_owner_commitment,
                48,
            )?;
            validate_hex(
                "quantity_verifier_commitment",
                &source.quantity_verifier_commitment,
                48,
            )?;
            validate_hex(
                "valuation_verifier_commitment",
                &source.valuation_verifier_commitment,
                48,
            )?;
            validate_hex("haircut_policy_hash", &source.haircut_policy_hash, 48)?;
            if source.adapter_schema_version == 0 {
                return Err("adapter_schema_version must be nonzero".to_string());
            }
            if source.freshness_policy.max_age_blocks == 0
                || source.freshness_policy.max_observation_span_blocks == 0
            {
                return Err("source freshness bounds must be nonzero".to_string());
            }
            // V1 represents liabilities associated with an asset source in
            // `SourceObservationV1::total_liabilities`. A standalone
            // liability source needs signed trust-bucket arithmetic that is
            // not present in the fixed V1 public-values ABI. Reject it here
            // instead of committing a field whose advertised meaning the
            // guest would silently ignore.
            if source.liability_treatment == LiabilityTreatmentV1::Liability {
                return Err(
                    "standalone liability sources are unsupported by reserve-proof v1; attach liabilities to an asset source"
                        .to_string(),
                );
            }
            if let Some(previous) = previous {
                if source.source_id.as_str() <= previous {
                    return Err(
                        "reserve manifest sources must be strictly sorted and unique".to_string(),
                    );
                }
            }
            previous = Some(&source.source_id);
        }
        Ok(())
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        let mut out = Vec::new();
        append_bytes(&mut out, self.schema.as_bytes())?;
        append_u32(&mut out, self.sources.len())?;
        for source in &self.sources {
            for value in [
                source.source_id.as_bytes(),
                source.adapter_kind.as_bytes(),
                source.source_domain.as_bytes(),
                source.asset_or_position_id.as_bytes(),
            ] {
                append_bytes(&mut out, value)?;
            }
            append_hex(&mut out, &source.reserve_owner_commitment, 48)?;
            append_hex(&mut out, &source.quantity_verifier_commitment, 48)?;
            append_hex(&mut out, &source.valuation_verifier_commitment, 48)?;
            out.push(source.quantity_evidence_class.tag());
            out.push(source.valuation_evidence_class.tag());
            out.extend_from_slice(&source.freshness_policy.max_age_blocks.to_be_bytes());
            out.extend_from_slice(
                &source
                    .freshness_policy
                    .max_observation_span_blocks
                    .to_be_bytes(),
            );
            append_hex(&mut out, &source.haircut_policy_hash, 48)?;
            out.push(source.liability_treatment.tag());
            out.extend_from_slice(&source.adapter_schema_version.to_be_bytes());
        }
        Ok(out)
    }

    pub fn hash(&self) -> Result<String, String> {
        Ok(hash48(MANIFEST_HASH_DOMAIN, &[&self.canonical_bytes()?]))
    }
}

pub fn execute_reserve_proof(
    witness: &ReserveProofWitnessV1,
) -> Result<NavReservePublicValuesV1, String> {
    if witness.schema != WITNESS_SCHEMA_V1 {
        return Err("reserve proof witness schema mismatch".to_string());
    }
    witness.manifest.validate()?;
    validate_context(&witness.context)?;
    if witness.manifest.hash()? != witness.context.source_manifest_hash {
        return Err("source manifest hash does not match proof context".to_string());
    }
    if witness.observations.len() != witness.manifest.sources.len() {
        return Err("observation count must equal manifest source count".to_string());
    }
    let span = witness
        .context
        .observation_not_after
        .checked_sub(witness.context.observation_not_before)
        .ok_or_else(|| "observation interval is inverted".to_string())?;

    let mut gross_assets = 0u64;
    let mut total_liabilities = 0u64;
    let mut classified = [0u64; 3];
    let mut quantity_counts = [0u32; 3];
    let mut valuation_counts = [0u32; 3];
    let mut observation_leaves = Vec::with_capacity(witness.observations.len());
    let mut quantity_leaves = Vec::with_capacity(witness.observations.len());
    let mut valuation_leaves = Vec::with_capacity(witness.observations.len());
    let mut disclosure_leaves = Vec::with_capacity(witness.observations.len());

    for (entry, observation) in witness
        .manifest
        .sources
        .iter()
        .zip(witness.observations.iter())
    {
        if observation.source_id != entry.source_id {
            return Err("observations must exactly follow canonical manifest order".to_string());
        }
        if observation.observed_at_block < witness.context.observation_not_before
            || observation.observed_at_block > witness.context.observation_not_after
        {
            return Err(format!(
                "source {} is outside the observation interval",
                entry.source_id
            ));
        }
        if span > entry.freshness_policy.max_observation_span_blocks {
            return Err(format!(
                "source {} observation interval exceeds its bound",
                entry.source_id
            ));
        }
        let age = witness
            .context
            .observation_not_after
            .checked_sub(observation.observed_at_block)
            .ok_or_else(|| "source observation is in the future".to_string())?;
        if age > entry.freshness_policy.max_age_blocks {
            return Err(format!("source {} observation is stale", entry.source_id));
        }
        if observation.total_liabilities > observation.gross_assets {
            return Err(format!(
                "source {} liabilities exceed assets",
                entry.source_id
            ));
        }
        validate_hex(
            "disclosure_commitment",
            &observation.disclosure_commitment,
            48,
        )?;
        verify_evidence(
            &witness.context,
            entry,
            observation,
            "quantity",
            &observation.quantity_evidence,
            entry.quantity_evidence_class,
        )?;
        verify_evidence(
            &witness.context,
            entry,
            observation,
            "valuation",
            &observation.valuation_evidence,
            entry.valuation_evidence_class,
        )?;

        gross_assets = gross_assets
            .checked_add(observation.gross_assets)
            .ok_or_else(|| "gross assets overflow u64".to_string())?;
        total_liabilities = total_liabilities
            .checked_add(observation.total_liabilities)
            .ok_or_else(|| "total liabilities overflow u64".to_string())?;
        let net = observation.gross_assets - observation.total_liabilities;
        let weakest = entry
            .quantity_evidence_class
            .max(entry.valuation_evidence_class);
        let class_index = usize::from(weakest.tag() - 1);
        classified[class_index] = classified[class_index]
            .checked_add(net)
            .ok_or_else(|| "trust-classified value overflow u64".to_string())?;
        increment_count(&mut quantity_counts, entry.quantity_evidence_class)?;
        increment_count(&mut valuation_counts, entry.valuation_evidence_class)?;

        let observation_bytes = canonical_observation_bytes(entry, observation)?;
        observation_leaves.push(hash48_bytes(OBSERVATION_HASH_DOMAIN, &[&observation_bytes]));
        quantity_leaves.push(trust_leaf(entry, entry.quantity_evidence_class)?);
        valuation_leaves.push(trust_leaf(entry, entry.valuation_evidence_class)?);
        disclosure_leaves.push(disclosure_leaf(entry, observation)?);
    }

    let verified_net_assets = gross_assets
        .checked_sub(total_liabilities)
        .ok_or_else(|| "total liabilities exceed gross assets".to_string())?;
    let public_values = NavReservePublicValuesV1 {
        schema: NAV_RESERVE_PUBLIC_VALUES_SCHEMA_V1.to_string(),
        pftl_genesis_hash: witness.context.pftl_genesis_hash.clone(),
        nav_asset_id: witness.context.nav_asset_id.clone(),
        proof_profile_id: witness.context.proof_profile_id.clone(),
        valuation_policy_hash: witness.context.valuation_policy_hash.clone(),
        source_manifest_hash: witness.context.source_manifest_hash.clone(),
        valuation_unit_id: witness.context.valuation_unit_id.clone(),
        valuation_scale: witness.context.valuation_scale,
        observation_epoch: witness.context.observation_epoch,
        observation_not_before: witness.context.observation_not_before,
        observation_not_after: witness.context.observation_not_after,
        source_observation_root: root_hash(OBSERVATION_ROOT_DOMAIN, &observation_leaves),
        gross_assets,
        total_liabilities,
        verified_net_assets,
        cryptographically_verified_value: classified[0],
        attested_value: classified[1],
        controlled_value: classified[2],
        source_count: u32::try_from(witness.observations.len())
            .map_err(|_| "source count overflows u32".to_string())?,
        quantity_trust_counts: trust_counts(quantity_counts),
        valuation_trust_counts: trust_counts(valuation_counts),
        quantity_trust_root: root_hash(QUANTITY_TRUST_ROOT_DOMAIN, &quantity_leaves),
        valuation_trust_root: root_hash(VALUATION_TRUST_ROOT_DOMAIN, &valuation_leaves),
        source_disclosure_root: root_hash(DISCLOSURE_ROOT_DOMAIN, &disclosure_leaves),
    };
    public_values.validate()?;
    Ok(public_values)
}

fn validate_context(context: &ReserveProofContextV1) -> Result<(), String> {
    for (field, value, bytes) in [
        ("pftl_genesis_hash", &context.pftl_genesis_hash, 48usize),
        ("nav_asset_id", &context.nav_asset_id, 48),
        ("proof_profile_id", &context.proof_profile_id, 48),
        ("valuation_policy_hash", &context.valuation_policy_hash, 32),
        ("source_manifest_hash", &context.source_manifest_hash, 48),
        ("valuation_unit_id", &context.valuation_unit_id, 48),
    ] {
        validate_hex(field, value, bytes)?;
    }
    if context.valuation_scale == 0 || context.observation_epoch == 0 {
        return Err("valuation scale and observation epoch must be nonzero".to_string());
    }
    if context.observation_not_before > context.observation_not_after {
        return Err("observation interval is inverted".to_string());
    }
    Ok(())
}

fn verify_evidence(
    context: &ReserveProofContextV1,
    entry: &SourceManifestEntryV1,
    observation: &SourceObservationV1,
    dimension: &str,
    evidence: &SourceEvidenceV1,
    expected_class: TrustClassV1,
) -> Result<(), String> {
    if evidence.class() != expected_class {
        return Err(format!(
            "source {} {dimension} trust class mismatch",
            entry.source_id
        ));
    }
    validate_hex("evidence_commitment", evidence.commitment(), 48)?;
    match evidence {
        SourceEvidenceV1::Controlled { .. } => Ok(()),
        SourceEvidenceV1::AttestedEd25519 {
            verifier_public_key,
            signature,
            ..
        } => verify_signed_evidence(
            context,
            entry,
            observation,
            dimension,
            evidence,
            verifier_public_key,
            signature,
            ATTESTATION_DOMAIN,
            "attestation",
        ),
        SourceEvidenceV1::ProtocolReceiptEd25519 {
            verifier_public_key,
            signature,
            ..
        } => {
            if entry.adapter_kind != "ed25519-protocol-receipt-v1" {
                return Err(format!(
                    "source {} protocol receipt requires ed25519-protocol-receipt-v1 adapter",
                    entry.source_id
                ));
            }
            verify_signed_evidence(
                context,
                entry,
                observation,
                dimension,
                evidence,
                verifier_public_key,
                signature,
                PROTOCOL_RECEIPT_DOMAIN,
                "protocol receipt",
            )
        }
        SourceEvidenceV1::EvmErc20BftCheckpointMpt {
            evidence_commitment,
            proof,
        } => {
            if dimension != "quantity" {
                return Err(format!(
                    "source {} EVM balance proof is only valid for quantity evidence",
                    entry.source_id
                ));
            }
            if entry.adapter_kind != EVM_ERC20_ADAPTER_KIND_V1 {
                return Err(format!(
                    "source {} EVM balance proof requires {EVM_ERC20_ADAPTER_KIND_V1} adapter",
                    entry.source_id
                ));
            }
            proof.verify(
                &context.pftl_genesis_hash,
                &context.nav_asset_id,
                &context.proof_profile_id,
                &context.valuation_policy_hash,
                &context.source_manifest_hash,
                &entry.source_id,
                &entry.source_domain,
                &entry.asset_or_position_id,
                &entry.reserve_owner_commitment,
                &entry.quantity_verifier_commitment,
                observation.observed_at_block,
                evidence_commitment,
            )
        }
        #[cfg(feature = "a666-public-adapters-v2")]
        SourceEvidenceV1::HyperliquidReceipt {
            evidence_commitment,
            proof,
        } => {
            if entry.adapter_kind != HYPERLIQUID_RECEIPT_ADAPTER_KIND_V1
                || entry.adapter_schema_version != 1
            {
                return Err(format!(
                    "source {} Hyperliquid receipt requires {HYPERLIQUID_RECEIPT_ADAPTER_KIND_V1} adapter schema 1",
                    entry.source_id
                ));
            }
            verify_hyperliquid_receipt_proof_v1(
                proof,
                &HyperliquidReceiptVerifyContextV1 {
                    pftl_genesis_hash: &context.pftl_genesis_hash,
                    nav_asset_id: &context.nav_asset_id,
                    proof_profile_id: &context.proof_profile_id,
                    valuation_policy_hash: &context.valuation_policy_hash,
                    source_manifest_hash: &context.source_manifest_hash,
                    source_id: &entry.source_id,
                    source_domain: &entry.source_domain,
                    asset_or_position_id: &entry.asset_or_position_id,
                    reserve_owner_commitment: &entry.reserve_owner_commitment,
                    quantity_verifier_commitment: &entry.quantity_verifier_commitment,
                    valuation_verifier_commitment: &entry.valuation_verifier_commitment,
                    observed_at_pftl_height: observation.observed_at_block,
                    expected_gross_assets: observation.gross_assets,
                    expected_total_liabilities: observation.total_liabilities,
                    expected_evidence_commitment: evidence_commitment,
                },
            )
            .map(|_| ())
            .map_err(|error| {
                format!(
                    "source {} {dimension} Hyperliquid receipt verification failed: {error:?}",
                    entry.source_id
                )
            })
        }
        #[cfg(feature = "a666-public-adapters-v2")]
        SourceEvidenceV1::NearReceiptQuantity {
            evidence_commitment,
            proof,
        } => {
            if dimension != "quantity" {
                return Err(format!(
                    "source {} NEAR receipt proof is only valid for quantity evidence",
                    entry.source_id
                ));
            }
            if entry.adapter_kind != NEAR_RECEIPT_QUANTITY_ADAPTER_KIND_V1
                || entry.adapter_schema_version != 1
            {
                return Err(format!(
                    "source {} NEAR receipt requires {NEAR_RECEIPT_QUANTITY_ADAPTER_KIND_V1} adapter schema 1",
                    entry.source_id
                ));
            }
            verify_near_receipt_quantity_proof_v1(
                proof,
                &NearReceiptVerifyContextV1 {
                    pftl_genesis_hash: &context.pftl_genesis_hash,
                    nav_asset_id: &context.nav_asset_id,
                    proof_profile_id: &context.proof_profile_id,
                    valuation_policy_hash: &context.valuation_policy_hash,
                    source_manifest_hash: &context.source_manifest_hash,
                    source_id: &entry.source_id,
                    source_domain: &entry.source_domain,
                    asset_or_position_id: &entry.asset_or_position_id,
                    reserve_owner_commitment: &entry.reserve_owner_commitment,
                    quantity_verifier_commitment: &entry.quantity_verifier_commitment,
                    observed_at_pftl_height: observation.observed_at_block,
                    expected_evidence_commitment: evidence_commitment,
                },
            )
            .map(|_| ())
            .map_err(|error| {
                format!(
                    "source {} NEAR quantity receipt verification failed: {error:?}",
                    entry.source_id
                )
            })
        }
        #[cfg(feature = "a666-public-adapters-v2")]
        SourceEvidenceV1::AaveV3 {
            evidence_commitment,
            proof,
        } => {
            if entry.adapter_kind != AAVE_V3_ADAPTER_KIND_V1 || entry.adapter_schema_version != 1 {
                return Err(format!(
                    "source {} Aave proof requires {AAVE_V3_ADAPTER_KIND_V1} adapter schema 1",
                    entry.source_id
                ));
            }
            verify_aave_v3_proof_v1(
                proof,
                &AaveV3VerifyContextV1 {
                    pftl_genesis_hash: &context.pftl_genesis_hash,
                    nav_asset_id: &context.nav_asset_id,
                    proof_profile_id: &context.proof_profile_id,
                    valuation_policy_hash: &context.valuation_policy_hash,
                    source_manifest_hash: &context.source_manifest_hash,
                    source_id: &entry.source_id,
                    source_domain: &entry.source_domain,
                    asset_or_position_id: &entry.asset_or_position_id,
                    reserve_owner_commitment: &entry.reserve_owner_commitment,
                    quantity_verifier_commitment: &entry.quantity_verifier_commitment,
                    valuation_verifier_commitment: &entry.valuation_verifier_commitment,
                    observed_at_pftl_height: observation.observed_at_block,
                    expected_gross_assets: observation.gross_assets,
                    expected_total_liabilities: observation.total_liabilities,
                    expected_evidence_commitment: evidence_commitment,
                },
            )
            .map(|_| ())
            .map_err(|error| {
                format!(
                    "source {} {dimension} Aave verification failed: {error:?}",
                    entry.source_id
                )
            })
        }
        #[cfg(feature = "a666-public-adapters-v2")]
        SourceEvidenceV1::EvmSpotQuantity {
            evidence_commitment,
            proof,
        } => {
            if dimension != "quantity" {
                return Err(format!(
                    "source {} EVM spot proof is only valid for quantity evidence",
                    entry.source_id
                ));
            }
            if entry.adapter_kind != EVM_SPOT_ADAPTER_KIND_V1 || entry.adapter_schema_version != 1 {
                return Err(format!(
                    "source {} EVM spot proof requires {EVM_SPOT_ADAPTER_KIND_V1} adapter schema 1",
                    entry.source_id
                ));
            }
            verify_evm_spot_quantity_proof_v1(
                proof,
                &EvmSpotVerifyContextV1 {
                    pftl_genesis_hash: &context.pftl_genesis_hash,
                    nav_asset_id: &context.nav_asset_id,
                    proof_profile_id: &context.proof_profile_id,
                    valuation_policy_hash: &context.valuation_policy_hash,
                    source_manifest_hash: &context.source_manifest_hash,
                    source_id: &entry.source_id,
                    source_domain: &entry.source_domain,
                    asset_or_position_id: &entry.asset_or_position_id,
                    reserve_owner_commitment: &entry.reserve_owner_commitment,
                    quantity_verifier_commitment: &entry.quantity_verifier_commitment,
                    observed_at_pftl_height: observation.observed_at_block,
                    expected_evidence_commitment: evidence_commitment,
                },
            )
            .map(|_| ())
            .map_err(|error| {
                format!(
                    "source {} EVM spot quantity verification failed: {error:?}",
                    entry.source_id
                )
            })
        }
        #[cfg(feature = "a666-public-adapters-v2")]
        SourceEvidenceV1::SolanaStakeAttested {
            evidence_commitment,
            proof,
        } => {
            if dimension != "quantity" {
                return Err(format!(
                    "source {} Solana stake proof is only valid for quantity evidence",
                    entry.source_id
                ));
            }
            if entry.adapter_kind != SOLANA_STAKE_ADAPTER_KIND_V1
                || entry.adapter_schema_version != 1
            {
                return Err(format!(
                    "source {} Solana stake proof requires {SOLANA_STAKE_ADAPTER_KIND_V1} adapter schema 1",
                    entry.source_id
                ));
            }
            verify_solana_stake_attested_proof_v1(
                proof,
                &SolanaStakeVerifyContextV1 {
                    pftl_genesis_hash: &context.pftl_genesis_hash,
                    nav_asset_id: &context.nav_asset_id,
                    proof_profile_id: &context.proof_profile_id,
                    valuation_policy_hash: &context.valuation_policy_hash,
                    source_manifest_hash: &context.source_manifest_hash,
                    source_id: &entry.source_id,
                    source_domain: &entry.source_domain,
                    asset_or_position_id: &entry.asset_or_position_id,
                    reserve_owner_commitment: &entry.reserve_owner_commitment,
                    quantity_verifier_commitment: &entry.quantity_verifier_commitment,
                    observed_at_pftl_height: observation.observed_at_block,
                    expected_evidence_commitment: evidence_commitment,
                },
            )
            .map(|_| ())
            .map_err(|error| {
                format!(
                    "source {} Solana stake quantity verification failed: {error:?}",
                    entry.source_id
                )
            })
        }
        SourceEvidenceV1::AdapterProof { proof, .. } => {
            if proof.len() > MAX_EVIDENCE_BYTES {
                return Err("adapter proof exceeds bounded maximum".to_string());
            }
            Err(format!(
                "source {} adapter {} has no registered cryptographic verifier",
                entry.source_id, entry.adapter_kind
            ))
        }
    }
}

/// Return the exact statement signed by an Ed25519 attestor or protocol
/// receipt key. This is the public construction path for independent source
/// adapters; callers must still attach the signature and run
/// `verify_observation_evidence` (or the complete witness executor).
pub fn ed25519_evidence_signing_statement(
    context: &ReserveProofContextV1,
    entry: &SourceManifestEntryV1,
    observation: &SourceObservationV1,
    dimension: EvidenceDimensionV1,
) -> Result<Vec<u8>, String> {
    validate_context(context)?;
    if observation.source_id != entry.source_id {
        return Err("evidence statement source_id does not match manifest entry".to_string());
    }
    let dimension = dimension.as_str();
    let evidence = match dimension {
        "quantity" => &observation.quantity_evidence,
        "valuation" => &observation.valuation_evidence,
        _ => unreachable!("typed evidence dimension"),
    };
    let expected_class = match dimension {
        "quantity" => entry.quantity_evidence_class,
        "valuation" => entry.valuation_evidence_class,
        _ => unreachable!("typed evidence dimension"),
    };
    if evidence.class() != expected_class {
        return Err(format!(
            "source {} {dimension} trust class mismatch",
            entry.source_id
        ));
    }
    match evidence {
        SourceEvidenceV1::AttestedEd25519 {
            verifier_public_key,
            ..
        } => signed_evidence_key_and_statement(
            context,
            entry,
            observation,
            dimension,
            evidence,
            verifier_public_key,
            ATTESTATION_DOMAIN,
            "attestation",
        )
        .map(|(_, statement)| statement),
        SourceEvidenceV1::ProtocolReceiptEd25519 {
            verifier_public_key,
            ..
        } => {
            if entry.adapter_kind != "ed25519-protocol-receipt-v1" {
                return Err(format!(
                    "source {} protocol receipt requires ed25519-protocol-receipt-v1 adapter",
                    entry.source_id
                ));
            }
            signed_evidence_key_and_statement(
                context,
                entry,
                observation,
                dimension,
                evidence,
                verifier_public_key,
                PROTOCOL_RECEIPT_DOMAIN,
                "protocol receipt",
            )
            .map(|(_, statement)| statement)
        }
        _ => Err("evidence statement is only defined for Ed25519 signed evidence".to_string()),
    }
}

/// Derive the exact manifest commitment for an Ed25519 attestation or
/// protocol-receipt key.
pub fn ed25519_verifier_commitment(verifier_public_key: &str) -> Result<String, String> {
    validate_hex("Ed25519 verifier public key", verifier_public_key, 32)?;
    let public_key_bytes = hex::decode(verifier_public_key)
        .map_err(|_| "invalid Ed25519 verifier public key".to_string())?;
    Ok(hash48(
        ED25519_VERIFIER_COMMITMENT_DOMAIN,
        &[&public_key_bytes],
    ))
}

/// Commit a bounded public adapter artifact or policy description under a
/// labeled provider-neutral domain. This is for opaque manifest/evidence
/// commitments; source-specific cryptographic adapters still define and
/// verify their own proof preimages.
pub fn opaque_commitment(label: &str, bytes: &[u8]) -> Result<String, String> {
    validate_identifier("opaque commitment label", label)?;
    if bytes.is_empty() || bytes.len() > MAX_EVIDENCE_BYTES {
        return Err(format!(
            "opaque commitment input must be in 1..={MAX_EVIDENCE_BYTES} bytes"
        ));
    }
    Ok(hash48(OPAQUE_COMMITMENT_DOMAIN, &[label.as_bytes(), bytes]))
}

/// Verify one fully assembled evidence dimension without requiring callers to
/// duplicate the guest's evidence dispatch rules.
pub fn verify_observation_evidence(
    context: &ReserveProofContextV1,
    entry: &SourceManifestEntryV1,
    observation: &SourceObservationV1,
    dimension: EvidenceDimensionV1,
) -> Result<(), String> {
    let (evidence, expected_class) = match dimension {
        EvidenceDimensionV1::Quantity => (
            &observation.quantity_evidence,
            entry.quantity_evidence_class,
        ),
        EvidenceDimensionV1::Valuation => (
            &observation.valuation_evidence,
            entry.valuation_evidence_class,
        ),
    };
    verify_evidence(
        context,
        entry,
        observation,
        dimension.as_str(),
        evidence,
        expected_class,
    )
}

#[allow(clippy::too_many_arguments)]
fn verify_signed_evidence(
    context: &ReserveProofContextV1,
    entry: &SourceManifestEntryV1,
    observation: &SourceObservationV1,
    dimension: &str,
    evidence: &SourceEvidenceV1,
    verifier_public_key: &str,
    signature: &str,
    domain: &[u8],
    label: &str,
) -> Result<(), String> {
    let (key, statement) = signed_evidence_key_and_statement(
        context,
        entry,
        observation,
        dimension,
        evidence,
        verifier_public_key,
        domain,
        label,
    )?;
    validate_hex(&format!("{label} signature"), signature, 64)?;
    let signature = Signature::from_slice(
        &hex::decode(signature).map_err(|_| format!("invalid {label} signature"))?,
    )
    .map_err(|_| format!("invalid {label} signature length"))?;
    key.verify(&statement, &signature).map_err(|_| {
        format!(
            "source {} {dimension} {label} signature invalid",
            entry.source_id
        )
    })
}

#[allow(clippy::too_many_arguments)]
fn signed_evidence_key_and_statement(
    context: &ReserveProofContextV1,
    entry: &SourceManifestEntryV1,
    observation: &SourceObservationV1,
    dimension: &str,
    evidence: &SourceEvidenceV1,
    verifier_public_key: &str,
    domain: &[u8],
    label: &str,
) -> Result<(VerifyingKey, Vec<u8>), String> {
    validate_hex(
        &format!("{label} verifier public key"),
        verifier_public_key,
        32,
    )?;
    let public_key_bytes: [u8; 32] = hex::decode(verifier_public_key)
        .map_err(|_| format!("invalid {label} verifier public key"))?
        .try_into()
        .map_err(|_| format!("invalid {label} verifier public key length"))?;
    let commitment = ed25519_verifier_commitment(verifier_public_key)?;
    if commitment != verifier_commitment(entry, dimension)? {
        return Err(format!(
            "source {} {dimension} verifier key commitment mismatch",
            entry.source_id
        ));
    }
    let key = VerifyingKey::from_bytes(&public_key_bytes)
        .map_err(|_| "invalid Ed25519 verification key".to_string())?;
    let statement =
        signed_evidence_statement(context, entry, observation, dimension, evidence, domain)?;
    Ok((key, statement))
}

fn verifier_commitment<'a>(
    entry: &'a SourceManifestEntryV1,
    dimension: &str,
) -> Result<&'a str, String> {
    match dimension {
        "quantity" => Ok(&entry.quantity_verifier_commitment),
        "valuation" => Ok(&entry.valuation_verifier_commitment),
        _ => Err("reserve evidence dimension is unsupported".to_string()),
    }
}

fn signed_evidence_statement(
    context: &ReserveProofContextV1,
    entry: &SourceManifestEntryV1,
    observation: &SourceObservationV1,
    dimension: &str,
    evidence: &SourceEvidenceV1,
    domain: &[u8],
) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    for value in [
        context.pftl_genesis_hash.as_bytes(),
        context.nav_asset_id.as_bytes(),
        context.proof_profile_id.as_bytes(),
        context.valuation_policy_hash.as_bytes(),
        context.source_manifest_hash.as_bytes(),
        context.valuation_unit_id.as_bytes(),
        entry.source_id.as_bytes(),
        entry.adapter_kind.as_bytes(),
        entry.source_domain.as_bytes(),
        entry.asset_or_position_id.as_bytes(),
        entry.reserve_owner_commitment.as_bytes(),
        entry.quantity_verifier_commitment.as_bytes(),
        entry.valuation_verifier_commitment.as_bytes(),
        entry.haircut_policy_hash.as_bytes(),
        dimension.as_bytes(),
        evidence.commitment().as_bytes(),
        observation.quantity_evidence.commitment().as_bytes(),
        observation.valuation_evidence.commitment().as_bytes(),
        observation.disclosure_commitment.as_bytes(),
    ] {
        append_bytes(&mut out, value)?;
    }
    out.extend_from_slice(&context.valuation_scale.to_be_bytes());
    out.extend_from_slice(&context.observation_epoch.to_be_bytes());
    out.extend_from_slice(&context.observation_not_before.to_be_bytes());
    out.extend_from_slice(&context.observation_not_after.to_be_bytes());
    out.extend_from_slice(&observation.observed_at_block.to_be_bytes());
    out.extend_from_slice(&observation.gross_assets.to_be_bytes());
    out.extend_from_slice(&observation.total_liabilities.to_be_bytes());
    out.push(entry.quantity_evidence_class.tag());
    out.push(entry.valuation_evidence_class.tag());
    out.push(entry.liability_treatment.tag());
    out.extend_from_slice(&entry.adapter_schema_version.to_be_bytes());
    Ok(domain_message(domain, &out))
}

fn canonical_observation_bytes(
    entry: &SourceManifestEntryV1,
    observation: &SourceObservationV1,
) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    append_bytes(&mut out, entry.source_id.as_bytes())?;
    out.extend_from_slice(&observation.observed_at_block.to_be_bytes());
    out.extend_from_slice(&observation.gross_assets.to_be_bytes());
    out.extend_from_slice(&observation.total_liabilities.to_be_bytes());
    out.push(entry.quantity_evidence_class.tag());
    out.push(entry.valuation_evidence_class.tag());
    append_hex(&mut out, observation.quantity_evidence.commitment(), 48)?;
    append_hex(&mut out, observation.valuation_evidence.commitment(), 48)?;
    append_hex(&mut out, &observation.disclosure_commitment, 48)?;
    Ok(out)
}

fn trust_leaf(entry: &SourceManifestEntryV1, class: TrustClassV1) -> Result<[u8; 48], String> {
    let mut out = Vec::new();
    append_bytes(&mut out, entry.source_id.as_bytes())?;
    out.push(class.tag());
    Ok(hash48_bytes(b"postfiat.reserve_trust_leaf.v1", &[&out]))
}

fn disclosure_leaf(
    entry: &SourceManifestEntryV1,
    observation: &SourceObservationV1,
) -> Result<[u8; 48], String> {
    let mut out = Vec::new();
    append_bytes(&mut out, entry.source_id.as_bytes())?;
    append_hex(&mut out, &observation.disclosure_commitment, 48)?;
    Ok(hash48_bytes(
        b"postfiat.reserve_disclosure_leaf.v1",
        &[&out],
    ))
}

fn root_hash(domain: &[u8], leaves: &[[u8; 48]]) -> String {
    let count = u32::try_from(leaves.len())
        .unwrap_or(u32::MAX)
        .to_be_bytes();
    let mut parts: Vec<&[u8]> = Vec::with_capacity(leaves.len() + 1);
    parts.push(&count);
    parts.extend(leaves.iter().map(<[u8; 48]>::as_slice));
    hash48(domain, &parts)
}

fn increment_count(counts: &mut [u32; 3], class: TrustClassV1) -> Result<(), String> {
    let index = usize::from(class.tag() - 1);
    counts[index] = counts[index]
        .checked_add(1)
        .ok_or_else(|| "trust count overflow u32".to_string())?;
    Ok(())
}

fn trust_counts(values: [u32; 3]) -> NavReserveTrustCountsV1 {
    NavReserveTrustCountsV1 {
        cryptographic: values[0],
        attested: values[1],
        controlled: values[2],
    }
}

fn validate_identifier(field: &str, value: &str) -> Result<(), String> {
    validate_text(field, value)?;
    if !value.bytes().enumerate().all(|(index, byte)| {
        byte.is_ascii_lowercase()
            || byte.is_ascii_digit()
            || (index > 0 && matches!(byte, b'.' | b'_' | b':' | b'-'))
    }) {
        return Err(format!("{field} must be canonical lowercase ASCII"));
    }
    Ok(())
}

fn validate_text(field: &str, value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > MAX_TEXT_BYTES
        || value.bytes().any(|byte| byte.is_ascii_control())
    {
        return Err(format!("{field} must be bounded non-control UTF-8"));
    }
    Ok(())
}

fn validate_hex(field: &str, value: &str, bytes: usize) -> Result<(), String> {
    if value.len() != bytes * 2
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!(
            "{field} must be exactly {bytes} bytes of lowercase hex"
        ));
    }
    Ok(())
}

fn append_bytes(out: &mut Vec<u8>, value: &[u8]) -> Result<(), String> {
    let length = u32::try_from(value.len()).map_err(|_| "canonical field too long".to_string())?;
    out.extend_from_slice(&length.to_be_bytes());
    out.extend_from_slice(value);
    Ok(())
}

fn append_u32(out: &mut Vec<u8>, value: usize) -> Result<(), String> {
    out.extend_from_slice(
        &u32::try_from(value)
            .map_err(|_| "canonical count overflows u32".to_string())?
            .to_be_bytes(),
    );
    Ok(())
}

fn append_hex(out: &mut Vec<u8>, value: &str, bytes: usize) -> Result<(), String> {
    validate_hex("canonical hex field", value, bytes)?;
    out.extend_from_slice(
        &hex::decode(value).map_err(|_| "canonical hex decode failed".to_string())?,
    );
    Ok(())
}

fn domain_message(domain: &[u8], payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + domain.len() + payload.len());
    out.extend_from_slice(&(domain.len() as u32).to_be_bytes());
    out.extend_from_slice(domain);
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(payload);
    out
}

fn hash48(domain: &[u8], parts: &[&[u8]]) -> String {
    hex::encode(hash48_bytes(domain, parts))
}

fn hash48_bytes(domain: &[u8], parts: &[&[u8]]) -> [u8; 48] {
    let mut hasher = Sha3_384::new();
    hasher.update((domain.len() as u32).to_be_bytes());
    hasher.update(domain);
    for part in parts {
        hasher.update((part.len() as u64).to_be_bytes());
        hasher.update(part);
    }
    hasher.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    fn entry(id: &str) -> SourceManifestEntryV1 {
        SourceManifestEntryV1 {
            source_id: id.to_string(),
            adapter_kind: "controlled-fixture-v1".to_string(),
            source_domain: "fixture".to_string(),
            asset_or_position_id: format!("position:{id}"),
            reserve_owner_commitment: "11".repeat(48),
            quantity_verifier_commitment: "00".repeat(48),
            valuation_verifier_commitment: "00".repeat(48),
            quantity_evidence_class: TrustClassV1::Controlled,
            valuation_evidence_class: TrustClassV1::Controlled,
            freshness_policy: FreshnessPolicyV1 {
                max_age_blocks: 10,
                max_observation_span_blocks: 10,
            },
            haircut_policy_hash: "22".repeat(48),
            liability_treatment: LiabilityTreatmentV1::Asset,
            adapter_schema_version: 1,
        }
    }

    fn fixture() -> ReserveProofWitnessV1 {
        let manifest = SourceManifestV1 {
            schema: MANIFEST_SCHEMA_V1.to_string(),
            sources: vec![entry("alpha"), entry("beta")],
        };
        let manifest_hash = manifest.hash().unwrap();
        ReserveProofWitnessV1 {
            schema: WITNESS_SCHEMA_V1.to_string(),
            context: ReserveProofContextV1 {
                pftl_genesis_hash: "01".repeat(48),
                nav_asset_id: "02".repeat(48),
                proof_profile_id: "03".repeat(48),
                valuation_policy_hash: "04".repeat(32),
                source_manifest_hash: manifest_hash,
                valuation_unit_id: "05".repeat(48),
                valuation_scale: 1_000_000,
                observation_epoch: 7,
                observation_not_before: 100,
                observation_not_after: 102,
            },
            manifest,
            observations: vec![
                observation("alpha", 100, 900, 100),
                observation("beta", 101, 500, 200),
            ],
        }
    }

    fn observation(id: &str, block: u64, assets: u64, liabilities: u64) -> SourceObservationV1 {
        SourceObservationV1 {
            source_id: id.to_string(),
            observed_at_block: block,
            gross_assets: assets,
            total_liabilities: liabilities,
            quantity_evidence: SourceEvidenceV1::Controlled {
                evidence_commitment: "33".repeat(48),
            },
            valuation_evidence: SourceEvidenceV1::Controlled {
                evidence_commitment: "44".repeat(48),
            },
            disclosure_commitment: "55".repeat(48),
        }
    }

    #[test]
    fn ed25519_verifier_commitment_is_domain_separated_and_exact() {
        let key = SigningKey::from_bytes(&[7u8; 32]);
        let public_key = hex::encode(key.verifying_key().to_bytes());
        let expected = hash48(
            ED25519_VERIFIER_COMMITMENT_DOMAIN,
            &[&key.verifying_key().to_bytes()],
        );
        assert_eq!(ed25519_verifier_commitment(&public_key).unwrap(), expected);
        assert!(ed25519_verifier_commitment(&"07".repeat(31)).is_err());
    }

    #[test]
    fn opaque_commitment_is_labeled_bounded_and_deterministic() {
        let first = opaque_commitment("reserve-owner", b"ethereum:0x1234").unwrap();
        assert_eq!(
            first,
            opaque_commitment("reserve-owner", b"ethereum:0x1234").unwrap()
        );
        assert_ne!(
            first,
            opaque_commitment("disclosure", b"ethereum:0x1234").unwrap()
        );
        assert!(opaque_commitment("bad label", b"value").is_err());
        assert!(opaque_commitment("reserve-owner", &[]).is_err());
        assert!(opaque_commitment("reserve-owner", &vec![0; MAX_EVIDENCE_BYTES + 1]).is_err());
    }

    #[test]
    fn manifest_hash_and_execution_are_deterministic() {
        let witness = fixture();
        let first = execute_reserve_proof(&witness).unwrap();
        let second = execute_reserve_proof(&witness).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.gross_assets, 1_400);
        assert_eq!(first.total_liabilities, 300);
        assert_eq!(first.verified_net_assets, 1_100);
        assert_eq!(first.controlled_value, 1_100);
        assert_eq!(first.source_count, 2);
        assert_eq!(first.encode().unwrap().len(), 584);
    }

    #[test]
    fn rejects_duplicate_order_substitution_stale_and_overflow() {
        let mut witness = fixture();
        witness.manifest.sources.swap(0, 1);
        assert!(witness.manifest.validate().is_err());

        let mut witness = fixture();
        witness.observations.swap(0, 1);
        assert!(execute_reserve_proof(&witness).is_err());

        let mut witness = fixture();
        witness.observations[0].observed_at_block = 1;
        assert!(execute_reserve_proof(&witness).is_err());

        let mut witness = fixture();
        witness.observations[0].gross_assets = u64::MAX;
        assert!(execute_reserve_proof(&witness).is_err());
    }

    #[test]
    fn cryptographic_claim_fails_without_registered_adapter() {
        let mut witness = fixture();
        witness.manifest.sources[0].quantity_evidence_class = TrustClassV1::Cryptographic;
        witness.context.source_manifest_hash = witness.manifest.hash().unwrap();
        witness.observations[0].quantity_evidence = SourceEvidenceV1::AdapterProof {
            evidence_commitment: "66".repeat(48),
            proof: vec![1, 2, 3],
        };
        assert!(execute_reserve_proof(&witness)
            .unwrap_err()
            .contains("no registered cryptographic verifier"));
    }

    #[test]
    fn rejects_manifest_count_identifier_and_evidence_bounds() {
        let mut manifest = SourceManifestV1 {
            schema: MANIFEST_SCHEMA_V1.to_string(),
            sources: Vec::new(),
        };
        assert!(manifest.validate().is_err());

        manifest.sources = (0..=MAX_SOURCES)
            .map(|index| entry(&format!("source-{index:02}")))
            .collect();
        assert!(manifest.validate().is_err());

        let mut witness = fixture();
        witness.manifest.sources[0].source_id = "UPPERCASE".to_string();
        assert!(witness.manifest.validate().is_err());

        let mut witness = fixture();
        witness.manifest.sources[0].quantity_evidence_class = TrustClassV1::Cryptographic;
        witness.context.source_manifest_hash = witness.manifest.hash().unwrap();
        witness.observations[0].quantity_evidence = SourceEvidenceV1::AdapterProof {
            evidence_commitment: "66".repeat(48),
            proof: vec![0; MAX_EVIDENCE_BYTES + 1],
        };
        assert!(execute_reserve_proof(&witness)
            .unwrap_err()
            .contains("bounded maximum"));

        let mut manifest = fixture().manifest;
        manifest.sources[0].liability_treatment = LiabilityTreatmentV1::Liability;
        assert!(manifest
            .validate()
            .unwrap_err()
            .contains("standalone liability sources are unsupported"));
    }

    #[test]
    fn rejects_context_interval_liability_and_trust_substitution() {
        let mut witness = fixture();
        witness.context.source_manifest_hash = "99".repeat(48);
        assert!(execute_reserve_proof(&witness)
            .unwrap_err()
            .contains("manifest hash"));

        let mut witness = fixture();
        witness.context.observation_not_before = 103;
        assert!(execute_reserve_proof(&witness)
            .unwrap_err()
            .contains("inverted"));

        let mut witness = fixture();
        witness.context.observation_not_after = 111;
        assert!(execute_reserve_proof(&witness)
            .unwrap_err()
            .contains("exceeds its bound"));

        let mut witness = fixture();
        witness.observations[0].observed_at_block = 99;
        assert!(execute_reserve_proof(&witness)
            .unwrap_err()
            .contains("outside the observation interval"));

        let mut witness = fixture();
        witness.observations[0].total_liabilities = 901;
        assert!(execute_reserve_proof(&witness)
            .unwrap_err()
            .contains("liabilities exceed assets"));

        let mut witness = fixture();
        witness.observations[0].quantity_evidence = SourceEvidenceV1::AttestedEd25519 {
            evidence_commitment: "66".repeat(48),
            verifier_public_key: "77".repeat(32),
            signature: "88".repeat(64),
        };
        assert!(execute_reserve_proof(&witness)
            .unwrap_err()
            .contains("trust class mismatch"));
    }

    #[test]
    fn attestation_binds_key_context_amounts_and_dimension() {
        let signing_key = SigningKey::from_bytes(&[7u8; 32]);
        let public_key = signing_key.verifying_key().to_bytes();
        let mut witness = fixture();
        witness.manifest.sources[0].quantity_evidence_class = TrustClassV1::Attested;
        witness.manifest.sources[0].quantity_verifier_commitment =
            hash48(ED25519_VERIFIER_COMMITMENT_DOMAIN, &[&public_key]);
        witness.context.source_manifest_hash = witness.manifest.hash().unwrap();
        let evidence = SourceEvidenceV1::AttestedEd25519 {
            evidence_commitment: "66".repeat(48),
            verifier_public_key: hex::encode(public_key),
            signature: "00".repeat(64),
        };
        witness.observations[0].quantity_evidence = evidence;
        let statement = ed25519_evidence_signing_statement(
            &witness.context,
            &witness.manifest.sources[0],
            &witness.observations[0],
            EvidenceDimensionV1::Quantity,
        )
        .unwrap();
        let SourceEvidenceV1::AttestedEd25519 { signature, .. } =
            &mut witness.observations[0].quantity_evidence
        else {
            unreachable!();
        };
        *signature = hex::encode(signing_key.sign(&statement).to_bytes());

        verify_observation_evidence(
            &witness.context,
            &witness.manifest.sources[0],
            &witness.observations[0],
            EvidenceDimensionV1::Quantity,
        )
        .unwrap();

        let values = execute_reserve_proof(&witness).unwrap();
        assert_eq!(values.quantity_trust_counts.attested, 1);
        assert_eq!(values.valuation_trust_counts.controlled, 2);
        assert_eq!(values.controlled_value, 1_100);

        for tampered in [
            {
                let mut value = witness.clone();
                value.context.nav_asset_id = "aa".repeat(48);
                value
            },
            {
                let mut value = witness.clone();
                value.observations[0].gross_assets += 1;
                value
            },
            {
                let mut value = witness.clone();
                if let SourceEvidenceV1::AttestedEd25519 { signature, .. } =
                    &mut value.observations[0].quantity_evidence
                {
                    signature.replace_range(0..2, "ff");
                }
                value
            },
        ] {
            assert!(execute_reserve_proof(&tampered)
                .unwrap_err()
                .contains("attestation signature invalid"));
        }
    }

    #[test]
    fn protocol_receipt_is_cryptographic_and_binds_registered_adapter_and_key() {
        let signing_key = SigningKey::from_bytes(&[8u8; 32]);
        let public_key = signing_key.verifying_key().to_bytes();
        let mut witness = fixture();
        witness.manifest.sources[0].adapter_kind = "ed25519-protocol-receipt-v1".to_string();
        witness.manifest.sources[0].quantity_evidence_class = TrustClassV1::Cryptographic;
        witness.manifest.sources[0].quantity_verifier_commitment =
            hash48(ED25519_VERIFIER_COMMITMENT_DOMAIN, &[&public_key]);
        witness.context.source_manifest_hash = witness.manifest.hash().unwrap();
        let evidence = SourceEvidenceV1::ProtocolReceiptEd25519 {
            evidence_commitment: "ab".repeat(48),
            verifier_public_key: hex::encode(public_key),
            signature: "00".repeat(64),
        };
        witness.observations[0].quantity_evidence = evidence;
        let statement = signed_evidence_statement(
            &witness.context,
            &witness.manifest.sources[0],
            &witness.observations[0],
            "quantity",
            &witness.observations[0].quantity_evidence,
            PROTOCOL_RECEIPT_DOMAIN,
        )
        .unwrap();
        let SourceEvidenceV1::ProtocolReceiptEd25519 { signature, .. } =
            &mut witness.observations[0].quantity_evidence
        else {
            unreachable!();
        };
        *signature = hex::encode(signing_key.sign(&statement).to_bytes());

        let values = execute_reserve_proof(&witness).unwrap();
        assert_eq!(values.quantity_trust_counts.cryptographic, 1);

        let mut wrong_adapter = witness.clone();
        wrong_adapter.manifest.sources[0].adapter_kind = "operator-attestation-v1".to_string();
        wrong_adapter.context.source_manifest_hash = wrong_adapter.manifest.hash().unwrap();
        assert!(execute_reserve_proof(&wrong_adapter)
            .unwrap_err()
            .contains("requires ed25519-protocol-receipt-v1"));

        let mut wrong_key = witness;
        wrong_key.manifest.sources[0].quantity_verifier_commitment = "cd".repeat(48);
        wrong_key.context.source_manifest_hash = wrong_key.manifest.hash().unwrap();
        assert!(execute_reserve_proof(&wrong_key)
            .unwrap_err()
            .contains("verifier key commitment mismatch"));
    }
}
