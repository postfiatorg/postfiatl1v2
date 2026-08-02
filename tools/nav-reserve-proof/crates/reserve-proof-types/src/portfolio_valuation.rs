//! Canonical, provider-neutral commitment to the economic scope of one NAV
//! valuation policy.
//!
//! Source-specific verifier policies bind the exact contracts, programs,
//! committees, feeds, decimals, and haircuts. This policy binds which governed
//! sources comprise the portfolio, how each source is valued, and whether its
//! result is an asset or liability. Its hash is the 32-byte
//! `valuation_policy_hash` carried by the proof context and consensus profile.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{LiabilityTreatmentV1, MAX_SOURCES, MAX_TEXT_BYTES};

pub const PORTFOLIO_VALUATION_POLICY_SCHEMA_V1: &str =
    "postfiat.reserve_portfolio_valuation_policy.v1";
const POLICY_HASH_DOMAIN: &[u8] = b"postfiat.reserve_portfolio_valuation_policy_hash.v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortfolioValuationMethodV1 {
    /// Quantity and value are derived by the same source proof.
    IntegratedSourceProof,
    /// Quantity is fed into a separately verified Chainlink EVM state proof.
    EvmChainlinkStateProof,
}

impl PortfolioValuationMethodV1 {
    fn tag(self) -> u8 {
        match self {
            Self::IntegratedSourceProof => 1,
            Self::EvmChainlinkStateProof => 2,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortfolioValuationSourceV1 {
    pub source_id: String,
    pub asset_or_position_id: String,
    pub valuation_method: PortfolioValuationMethodV1,
    pub liability_treatment: LiabilityTreatmentV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortfolioValuationPolicyV1 {
    pub schema: String,
    pub nav_asset_id: String,
    pub valuation_unit_id: String,
    pub valuation_scale: u64,
    /// Canonically sorted by `source_id`; duplicate source or position IDs are
    /// invalid rather than silently normalized.
    pub sources: Vec<PortfolioValuationSourceV1>,
}

impl PortfolioValuationPolicyV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != PORTFOLIO_VALUATION_POLICY_SCHEMA_V1 {
            return Err("portfolio valuation policy schema mismatch".to_string());
        }
        validate_lower_hex("nav_asset_id", &self.nav_asset_id, 48)?;
        validate_lower_hex("valuation_unit_id", &self.valuation_unit_id, 48)?;
        if self.valuation_scale == 0 {
            return Err("portfolio valuation scale must be nonzero".to_string());
        }
        if self.sources.is_empty() || self.sources.len() > MAX_SOURCES {
            return Err(format!(
                "portfolio valuation source count must be in 1..={MAX_SOURCES}"
            ));
        }

        let mut previous_source: Option<&str> = None;
        let mut positions = std::collections::BTreeSet::new();
        for source in &self.sources {
            validate_identifier("source_id", &source.source_id)?;
            validate_identifier("asset_or_position_id", &source.asset_or_position_id)?;
            if previous_source >= Some(source.source_id.as_str()) {
                return Err(
                    "portfolio valuation sources must be uniquely sorted by source_id".to_string(),
                );
            }
            if !positions.insert(source.asset_or_position_id.as_str()) {
                return Err("portfolio valuation position IDs must be unique".to_string());
            }
            previous_source = Some(source.source_id.as_str());
        }
        Ok(())
    }

    pub fn hash(&self) -> Result<String, String> {
        self.validate()?;
        let mut bytes = Vec::new();
        append_bytes(&mut bytes, self.schema.as_bytes())?;
        append_hex(&mut bytes, &self.nav_asset_id, 48)?;
        append_hex(&mut bytes, &self.valuation_unit_id, 48)?;
        bytes.extend_from_slice(&self.valuation_scale.to_be_bytes());
        append_u32(&mut bytes, self.sources.len())?;
        for source in &self.sources {
            append_bytes(&mut bytes, source.source_id.as_bytes())?;
            append_bytes(&mut bytes, source.asset_or_position_id.as_bytes())?;
            bytes.push(source.valuation_method.tag());
            bytes.push(match source.liability_treatment {
                LiabilityTreatmentV1::Asset => 1,
                LiabilityTreatmentV1::Liability => 2,
            });
        }

        let mut hasher = Sha256::new();
        hasher.update(POLICY_HASH_DOMAIN);
        let byte_len = u64::try_from(bytes.len())
            .map_err(|_| "portfolio valuation policy length overflow".to_string())?;
        hasher.update(byte_len.to_be_bytes());
        hasher.update(bytes);
        Ok(hex::encode(hasher.finalize()))
    }
}

fn validate_identifier(label: &str, value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > MAX_TEXT_BYTES
        || !value.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || (index > 0 && matches!(byte, b'.' | b'_' | b':' | b'-'))
        })
    {
        return Err(format!("invalid portfolio valuation {label}"));
    }
    Ok(())
}

fn validate_lower_hex(label: &str, value: &str, bytes: usize) -> Result<(), String> {
    if value.len() != bytes * 2
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!(
            "portfolio valuation {label} must be lowercase hexadecimal with exactly {bytes} bytes"
        ));
    }
    Ok(())
}

fn append_u32(out: &mut Vec<u8>, value: usize) -> Result<(), String> {
    let value = u32::try_from(value).map_err(|_| "portfolio valuation length overflow")?;
    out.extend_from_slice(&value.to_be_bytes());
    Ok(())
}

fn append_bytes(out: &mut Vec<u8>, value: &[u8]) -> Result<(), String> {
    append_u32(out, value.len())?;
    out.extend_from_slice(value);
    Ok(())
}

fn append_hex(out: &mut Vec<u8>, value: &str, bytes: usize) -> Result<(), String> {
    validate_lower_hex("hash field", value, bytes)?;
    out.extend_from_slice(&hex::decode(value).map_err(|_| "invalid portfolio valuation hex")?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> PortfolioValuationPolicyV1 {
        PortfolioValuationPolicyV1 {
            schema: PORTFOLIO_VALUATION_POLICY_SCHEMA_V1.to_string(),
            nav_asset_id: "11".repeat(48),
            valuation_unit_id: "22".repeat(48),
            valuation_scale: 100_000_000,
            sources: vec![PortfolioValuationSourceV1 {
                source_id: "evm-spot".to_string(),
                asset_or_position_id: "evm-spot-set:a666-v1".to_string(),
                valuation_method: PortfolioValuationMethodV1::EvmChainlinkStateProof,
                liability_treatment: LiabilityTreatmentV1::Asset,
            }],
        }
    }

    #[test]
    fn hash_is_deterministic_and_semantic() {
        let policy = policy();
        assert_eq!(policy.hash().unwrap(), policy.hash().unwrap());
        assert_eq!(policy.hash().unwrap().len(), 64);

        let mut changed = policy.clone();
        changed.sources[0].liability_treatment = LiabilityTreatmentV1::Liability;
        assert_ne!(policy.hash().unwrap(), changed.hash().unwrap());
    }

    #[test]
    fn rejects_noncanonical_or_duplicate_sources() {
        let mut policy = policy();
        let mut earlier = policy.sources[0].clone();
        earlier.source_id = "aave".to_string();
        policy.sources.push(earlier);
        assert!(policy.validate().is_err());

        policy
            .sources
            .sort_by(|left, right| left.source_id.cmp(&right.source_id));
        assert!(policy.validate().is_err());
        policy.sources[0].asset_or_position_id = "aave-set:a666-v1".to_string();
        assert!(policy.validate().is_ok());
    }
}
