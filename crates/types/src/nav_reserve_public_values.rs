pub const NAV_RESERVE_PUBLIC_VALUES_SCHEMA_V1: &str =
    "postfiat.nav_reserve_public_values.v1";
pub const NAV_RESERVE_PUBLIC_VALUES_VERSION_V1: u32 = 1;
pub const NAV_RESERVE_PUBLIC_VALUES_MAGIC_V1: &[u8; 8] = b"PFNAV001";
pub const NAV_RESERVE_PUBLIC_VALUES_V1_BYTES: usize = 584;
pub const NAV_RESERVE_MAX_SOURCES_V1: u32 = 64;
pub const NAV_RESERVE_VALUATION_UNIT_ID_DOMAIN_V1: &str =
    "postfiat.nav_reserve_valuation_unit_id.v1";

/// Counted source trust classes are public and separate for quantity evidence
/// and valuation evidence. This prevents an SP1 aggregate from turning an
/// attested brokerage response into a cryptographic claim by branding alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct NavReserveTrustCountsV1 {
    pub cryptographic: u32,
    pub attested: u32,
    pub controlled: u32,
}

impl NavReserveTrustCountsV1 {
    pub fn checked_total(self) -> Result<u32, String> {
        self.cryptographic
            .checked_add(self.attested)
            .and_then(|value| value.checked_add(self.controlled))
            .ok_or_else(|| "nav reserve trust counts overflow u32".to_string())
    }
}

/// Canonical fixed-width public values for provider-neutral NAV reserve proofs.
///
/// All hashes are lowercase hex in the Rust representation and fixed bytes in
/// the wire representation. Integer encoding is big-endian. There are no
/// dynamic offsets, vectors, maps, floats, or platform-sized values.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavReservePublicValuesV1 {
    pub schema: String,
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
    pub source_observation_root: String,
    pub gross_assets: u64,
    pub total_liabilities: u64,
    pub verified_net_assets: u64,
    pub cryptographically_verified_value: u64,
    pub attested_value: u64,
    pub controlled_value: u64,
    pub source_count: u32,
    pub quantity_trust_counts: NavReserveTrustCountsV1,
    pub valuation_trust_counts: NavReserveTrustCountsV1,
    pub quantity_trust_root: String,
    pub valuation_trust_root: String,
    pub source_disclosure_root: String,
}

impl NavReservePublicValuesV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != NAV_RESERVE_PUBLIC_VALUES_SCHEMA_V1 {
            return Err("nav reserve public-values schema mismatch".to_string());
        }
        validate_fixed_lower_hex("pftl_genesis_hash", &self.pftl_genesis_hash, 48)?;
        validate_fixed_lower_hex("nav_asset_id", &self.nav_asset_id, 48)?;
        validate_fixed_lower_hex("proof_profile_id", &self.proof_profile_id, 48)?;
        validate_fixed_lower_hex("valuation_policy_hash", &self.valuation_policy_hash, 32)?;
        validate_fixed_lower_hex("source_manifest_hash", &self.source_manifest_hash, 48)?;
        validate_fixed_lower_hex("valuation_unit_id", &self.valuation_unit_id, 48)?;
        validate_fixed_lower_hex("source_observation_root", &self.source_observation_root, 48)?;
        validate_fixed_lower_hex("quantity_trust_root", &self.quantity_trust_root, 48)?;
        validate_fixed_lower_hex("valuation_trust_root", &self.valuation_trust_root, 48)?;
        validate_fixed_lower_hex("source_disclosure_root", &self.source_disclosure_root, 48)?;
        if self.valuation_scale == 0 || self.observation_epoch == 0 {
            return Err("valuation scale and observation epoch must be nonzero".to_string());
        }
        if self.observation_not_before > self.observation_not_after {
            return Err("observation interval is inverted".to_string());
        }
        if self.source_count == 0 || self.source_count > NAV_RESERVE_MAX_SOURCES_V1 {
            return Err(format!(
                "source_count must be in 1..={NAV_RESERVE_MAX_SOURCES_V1}"
            ));
        }
        if self.quantity_trust_counts.checked_total()? != self.source_count
            || self.valuation_trust_counts.checked_total()? != self.source_count
        {
            return Err("quantity and valuation trust counts must each equal source_count".to_string());
        }
        let expected_net = self
            .gross_assets
            .checked_sub(self.total_liabilities)
            .ok_or_else(|| "total liabilities exceed gross assets".to_string())?;
        if self.verified_net_assets != expected_net {
            return Err("verified_net_assets must equal gross_assets minus liabilities".to_string());
        }
        let classified = self
            .cryptographically_verified_value
            .checked_add(self.attested_value)
            .and_then(|value| value.checked_add(self.controlled_value))
            .ok_or_else(|| "trust-classified value overflows u64".to_string())?;
        if classified != self.verified_net_assets {
            return Err("trust-classified values must equal verified_net_assets".to_string());
        }
        Ok(())
    }

    pub fn encode(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        let mut out = Vec::with_capacity(NAV_RESERVE_PUBLIC_VALUES_V1_BYTES);
        out.extend_from_slice(NAV_RESERVE_PUBLIC_VALUES_MAGIC_V1);
        out.extend_from_slice(&NAV_RESERVE_PUBLIC_VALUES_VERSION_V1.to_be_bytes());
        append_fixed_hex::<48>(&mut out, &self.pftl_genesis_hash)?;
        append_fixed_hex::<48>(&mut out, &self.nav_asset_id)?;
        append_fixed_hex::<48>(&mut out, &self.proof_profile_id)?;
        append_fixed_hex::<32>(&mut out, &self.valuation_policy_hash)?;
        append_fixed_hex::<48>(&mut out, &self.source_manifest_hash)?;
        append_fixed_hex::<48>(&mut out, &self.valuation_unit_id)?;
        for value in [
            self.valuation_scale,
            self.observation_epoch,
            self.observation_not_before,
            self.observation_not_after,
        ] {
            out.extend_from_slice(&value.to_be_bytes());
        }
        append_fixed_hex::<48>(&mut out, &self.source_observation_root)?;
        for value in [
            self.gross_assets,
            self.total_liabilities,
            self.verified_net_assets,
            self.cryptographically_verified_value,
            self.attested_value,
            self.controlled_value,
        ] {
            out.extend_from_slice(&value.to_be_bytes());
        }
        out.extend_from_slice(&self.source_count.to_be_bytes());
        for counts in [self.quantity_trust_counts, self.valuation_trust_counts] {
            out.extend_from_slice(&counts.cryptographic.to_be_bytes());
            out.extend_from_slice(&counts.attested.to_be_bytes());
            out.extend_from_slice(&counts.controlled.to_be_bytes());
        }
        append_fixed_hex::<48>(&mut out, &self.quantity_trust_root)?;
        append_fixed_hex::<48>(&mut out, &self.valuation_trust_root)?;
        append_fixed_hex::<48>(&mut out, &self.source_disclosure_root)?;
        if out.len() != NAV_RESERVE_PUBLIC_VALUES_V1_BYTES {
            return Err("nav reserve public-values encoder length invariant failed".to_string());
        }
        Ok(out)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() != NAV_RESERVE_PUBLIC_VALUES_V1_BYTES {
            return Err(format!(
                "nav reserve public values must be exactly {NAV_RESERVE_PUBLIC_VALUES_V1_BYTES} bytes"
            ));
        }
        let mut reader = NavReservePublicValuesReader { bytes, offset: 0 };
        if reader.take(8)? != NAV_RESERVE_PUBLIC_VALUES_MAGIC_V1 {
            return Err("nav reserve public-values magic mismatch".to_string());
        }
        let version = reader.u32()?;
        if version != NAV_RESERVE_PUBLIC_VALUES_VERSION_V1 {
            return Err("nav reserve public-values version mismatch".to_string());
        }
        let value = Self {
            schema: NAV_RESERVE_PUBLIC_VALUES_SCHEMA_V1.to_string(),
            pftl_genesis_hash: reader.hex(48)?,
            nav_asset_id: reader.hex(48)?,
            proof_profile_id: reader.hex(48)?,
            valuation_policy_hash: reader.hex(32)?,
            source_manifest_hash: reader.hex(48)?,
            valuation_unit_id: reader.hex(48)?,
            valuation_scale: reader.u64()?,
            observation_epoch: reader.u64()?,
            observation_not_before: reader.u64()?,
            observation_not_after: reader.u64()?,
            source_observation_root: reader.hex(48)?,
            gross_assets: reader.u64()?,
            total_liabilities: reader.u64()?,
            verified_net_assets: reader.u64()?,
            cryptographically_verified_value: reader.u64()?,
            attested_value: reader.u64()?,
            controlled_value: reader.u64()?,
            source_count: reader.u32()?,
            quantity_trust_counts: NavReserveTrustCountsV1 {
                cryptographic: reader.u32()?,
                attested: reader.u32()?,
                controlled: reader.u32()?,
            },
            valuation_trust_counts: NavReserveTrustCountsV1 {
                cryptographic: reader.u32()?,
                attested: reader.u32()?,
                controlled: reader.u32()?,
            },
            quantity_trust_root: reader.hex(48)?,
            valuation_trust_root: reader.hex(48)?,
            source_disclosure_root: reader.hex(48)?,
        };
        if reader.offset != bytes.len() {
            return Err("nav reserve public values have trailing bytes".to_string());
        }
        value.validate()?;
        Ok(value)
    }
}

pub fn nav_reserve_valuation_unit_id(valuation_unit: &str) -> Result<String, String> {
    validate_text_field("nav reserve valuation unit", valuation_unit)?;
    Ok(hash_hex_domain(
        NAV_RESERVE_VALUATION_UNIT_ID_DOMAIN_V1,
        valuation_unit.as_bytes(),
    ))
}

fn validate_fixed_lower_hex(field: &str, value: &str, bytes: usize) -> Result<(), String> {
    if value.len() != bytes * 2
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!("{field} must be exactly {bytes} bytes of lowercase hex"));
    }
    Ok(())
}

fn append_fixed_hex<const N: usize>(out: &mut Vec<u8>, value: &str) -> Result<(), String> {
    validate_fixed_lower_hex("nav reserve public-values field", value, N)?;
    for pair in value.as_bytes().chunks_exact(2) {
        let text = std::str::from_utf8(pair).map_err(|_| "hex field is not UTF-8".to_string())?;
        out.push(u8::from_str_radix(text, 16).map_err(|_| "hex field is malformed".to_string())?);
    }
    Ok(())
}

struct NavReservePublicValuesReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl NavReservePublicValuesReader<'_> {
    fn take(&mut self, len: usize) -> Result<&[u8], String> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or_else(|| "nav reserve public-values offset overflow".to_string())?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or_else(|| "nav reserve public values are truncated".to_string())?;
        self.offset = end;
        Ok(value)
    }

    fn u32(&mut self) -> Result<u32, String> {
        let bytes: [u8; 4] = self
            .take(4)?
            .try_into()
            .map_err(|_| "nav reserve u32 is truncated".to_string())?;
        Ok(u32::from_be_bytes(bytes))
    }

    fn u64(&mut self) -> Result<u64, String> {
        let bytes: [u8; 8] = self
            .take(8)?
            .try_into()
            .map_err(|_| "nav reserve u64 is truncated".to_string())?;
        Ok(u64::from_be_bytes(bytes))
    }

    fn hex(&mut self, len: usize) -> Result<String, String> {
        Ok(self
            .take(len)?
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect())
    }
}

#[cfg(test)]
mod nav_reserve_public_values_tests {
    use super::*;

    fn fixture() -> NavReservePublicValuesV1 {
        NavReservePublicValuesV1 {
            schema: NAV_RESERVE_PUBLIC_VALUES_SCHEMA_V1.to_string(),
            pftl_genesis_hash: "01".repeat(48),
            nav_asset_id: "02".repeat(48),
            proof_profile_id: "03".repeat(48),
            valuation_policy_hash: "04".repeat(32),
            source_manifest_hash: "05".repeat(48),
            valuation_unit_id: "06".repeat(48),
            valuation_scale: 1_000_000,
            observation_epoch: 7,
            observation_not_before: 40,
            observation_not_after: 44,
            source_observation_root: "07".repeat(48),
            gross_assets: 1_200_000,
            total_liabilities: 200_000,
            verified_net_assets: 1_000_000,
            cryptographically_verified_value: 600_000,
            attested_value: 400_000,
            controlled_value: 0,
            source_count: 3,
            quantity_trust_counts: NavReserveTrustCountsV1 {
                cryptographic: 2,
                attested: 1,
                controlled: 0,
            },
            valuation_trust_counts: NavReserveTrustCountsV1 {
                cryptographic: 1,
                attested: 2,
                controlled: 0,
            },
            quantity_trust_root: "08".repeat(48),
            valuation_trust_root: "09".repeat(48),
            source_disclosure_root: "0a".repeat(48),
        }
    }

    #[test]
    fn nav_reserve_public_values_v1_round_trip_is_exact_and_canonical() {
        let value = fixture();
        let encoded = value.encode().expect("valid fixture must encode");
        assert_eq!(encoded.len(), NAV_RESERVE_PUBLIC_VALUES_V1_BYTES);
        assert_eq!(NavReservePublicValuesV1::decode(&encoded), Ok(value));
    }

    #[test]
    fn nav_reserve_public_values_v1_rejects_malformed_lengths_and_magic() {
        let encoded = fixture().encode().expect("valid fixture must encode");
        for malformed in [&encoded[..encoded.len() - 1], &encoded[..12]] {
            assert!(NavReservePublicValuesV1::decode(malformed).is_err());
        }
        let mut trailing = encoded.clone();
        trailing.push(0);
        assert!(NavReservePublicValuesV1::decode(&trailing).is_err());
        let mut wrong_magic = encoded;
        wrong_magic[0] ^= 1;
        assert!(NavReservePublicValuesV1::decode(&wrong_magic).is_err());
    }

    #[test]
    fn nav_reserve_public_values_v1_rejects_inconsistent_totals() {
        let mut value = fixture();
        value.total_liabilities = value.gross_assets + 1;
        assert!(value.encode().is_err());

        let mut value = fixture();
        value.verified_net_assets += 1;
        assert!(value.encode().is_err());

        let mut value = fixture();
        value.cryptographically_verified_value = u64::MAX;
        assert!(value.encode().is_err());
    }

    #[test]
    fn nav_reserve_public_values_v1_rejects_bad_time_sources_and_hex() {
        let mut value = fixture();
        value.observation_not_before = value.observation_not_after + 1;
        assert!(value.encode().is_err());

        let mut value = fixture();
        value.source_count = NAV_RESERVE_MAX_SOURCES_V1 + 1;
        assert!(value.encode().is_err());

        let mut value = fixture();
        value.quantity_trust_counts.cryptographic = u32::MAX;
        assert!(value.encode().is_err());

        let mut value = fixture();
        value.nav_asset_id = "AB".repeat(48);
        assert!(value.encode().is_err());
    }

    #[test]
    fn nav_reserve_valuation_unit_id_is_domain_separated() {
        assert_eq!(
            nav_reserve_valuation_unit_id("USD:6"),
            nav_reserve_valuation_unit_id("USD:6")
        );
        assert_ne!(
            nav_reserve_valuation_unit_id("USD:6"),
            nav_reserve_valuation_unit_id("NOK:6")
        );
        assert!(nav_reserve_valuation_unit_id("").is_err());
    }
}
