use postfiat_types::{
    AGGREGATE_PUBLIC_VALUES_V2_SCHEMA_VERSION, DEFAULT_MAX_NAV_SP1_PROOF_BYTES,
    DEFAULT_MAX_NAV_SP1_PUBLIC_VALUES_BYTES,
};
#[cfg(test)]
use postfiat_types::NAV_SP1_POLICY_HASH_HEX_LEN;
use sp1_verifier::{Groth16Verifier, GROTH16_VK_BYTES};

/// Public values decoded from the SP1 aggregate proof after Groth16 verification.
///
/// The verifier binds the proof to the SP1 program vkey stored in the
/// `sp1-groth16` NAV proof profile, then decodes the aggregate public-values
/// blob and checks both the valuation-policy hash and `verified_net_assets`.
/// The resulting `verified_net_assets` backs the floating-NAV floor invariant:
/// `verified_net_assets >= circulating_supply * nav_per_unit`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedSp1PublicValues {
    pub policy_hash_hex: String,
    pub verified_net_assets: u64,
    pub legacy_cash_omitted_verified_net_assets: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NavSp1VerifyOptions {
    pub allow_legacy_cash_omitted_verified_net_assets: bool,
}

impl NavSp1VerifyOptions {
    pub const fn strict() -> Self {
        Self {
            allow_legacy_cash_omitted_verified_net_assets: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavSp1VerifyError {
    MissingProof,
    ProofTooLarge,
    PublicValuesTooLarge,
    Groth16Invalid,
    PublicValuesDecode,
    SchemaVersionMismatch,
    PublicValuesMismatch,
    PolicyHashMismatch,
}

impl NavSp1VerifyError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::MissingProof => "missing_sp1_proof",
            Self::ProofTooLarge => "sp1_proof_too_large",
            Self::PublicValuesTooLarge => "sp1_public_values_too_large",
            Self::Groth16Invalid => "sp1_proof_invalid",
            Self::PublicValuesDecode => "sp1_public_values_decode_failed",
            Self::SchemaVersionMismatch => "sp1_public_values_schema_mismatch",
            Self::PublicValuesMismatch => "sp1_public_values_mismatch",
            Self::PolicyHashMismatch => "sp1_policy_hash_mismatch",
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::MissingProof => {
                "sp1-groth16 reserve submit requires sp1_proof_bytes and sp1_public_values"
                    .to_string()
            }
            Self::ProofTooLarge => "sp1 proof exceeds profile max_proof_bytes".to_string(),
            Self::PublicValuesTooLarge => {
                "sp1 public values exceed profile max_public_values_bytes".to_string()
            }
            Self::Groth16Invalid => "sp1 groth16 proof verification failed".to_string(),
            Self::PublicValuesDecode => {
                "sp1 public values could not be decoded as AggregatePublicValuesV2".to_string()
            }
            Self::SchemaVersionMismatch => {
                "sp1 public values schema_version must be AggregatePublicValuesV2".to_string()
            }
            Self::PublicValuesMismatch => {
                "decoded sp1 verified_net_assets does not match packet verified_net_assets"
                    .to_string()
            }
            Self::PolicyHashMismatch => {
                "decoded sp1 policy_hash does not match profile valuation_policy_hash"
                    .to_string()
            }
        }
    }
}

/// Verify a NAV reserve packet against an SP1 Groth16 aggregate proof.
///
/// This is the consensus entry point for `sp1-groth16` NAV profiles. It rejects
/// missing or oversized proof material before invoking the SP1 Groth16 verifier,
/// verifies the proof against the profile's SP1 program vkey and the SP1
/// verifier crate's Groth16 verifying key, then decodes the aggregate
/// public-values payload. The decoded valuation-policy hash must match the
/// profile and the decoded `verified_net_assets` must match the reserve packet.
///
/// The proof establishes the asset side of the floating-NAV invariant. The
/// separate collateralization check allows over-collateralization by enforcing
/// `verified_net_assets >= circulating_supply * nav_per_unit`, so non-integral
/// reserve values floor to the largest safe `nav_per_unit` rather than forcing
/// an exact stablecoin-style equality.
pub fn verify_sp1_groth16(
    profile: &NavProofProfile,
    verified_net_assets: u64,
    sp1_proof_bytes: &[u8],
    sp1_public_values: &[u8],
) -> Result<DecodedSp1PublicValues, NavSp1VerifyError> {
    verify_sp1_groth16_with_options(
        profile,
        verified_net_assets,
        sp1_proof_bytes,
        sp1_public_values,
        NavSp1VerifyOptions::strict(),
    )
}

pub fn verify_sp1_groth16_with_options(
    profile: &NavProofProfile,
    verified_net_assets: u64,
    sp1_proof_bytes: &[u8],
    sp1_public_values: &[u8],
    options: NavSp1VerifyOptions,
) -> Result<DecodedSp1PublicValues, NavSp1VerifyError> {
    if profile.verifier_kind != NAV_PROFILE_VERIFIER_SP1_GROTH16 {
        return Err(NavSp1VerifyError::Groth16Invalid);
    }
    if sp1_proof_bytes.is_empty() || sp1_public_values.is_empty() {
        return Err(NavSp1VerifyError::MissingProof);
    }
    let max_proof_bytes = if profile.max_proof_bytes == 0 {
        DEFAULT_MAX_NAV_SP1_PROOF_BYTES
    } else {
        profile.max_proof_bytes
    };
    let max_public_values_bytes = if profile.max_public_values_bytes == 0 {
        DEFAULT_MAX_NAV_SP1_PUBLIC_VALUES_BYTES
    } else {
        profile.max_public_values_bytes
    };
    if sp1_proof_bytes.len() as u64 > max_proof_bytes {
        return Err(NavSp1VerifyError::ProofTooLarge);
    }
    if sp1_public_values.len() as u64 > max_public_values_bytes {
        return Err(NavSp1VerifyError::PublicValuesTooLarge);
    }

    Groth16Verifier::verify(
        sp1_proof_bytes,
        sp1_public_values,
        &profile.sp1_program_vkey,
        &GROTH16_VK_BYTES,
    )
    .map_err(|_| NavSp1VerifyError::Groth16Invalid)?;

    let decoded = decode_aggregate_v2_totals(sp1_public_values)?;
    let matches_cash_included = decoded.verified_net_assets == verified_net_assets;
    let matches_legacy_cash_omitted = options.allow_legacy_cash_omitted_verified_net_assets
        && decoded.legacy_cash_omitted_verified_net_assets == Some(verified_net_assets);
    if !matches_cash_included && !matches_legacy_cash_omitted {
        return Err(NavSp1VerifyError::PublicValuesMismatch);
    }
    if decoded.policy_hash_hex != profile.valuation_policy_hash {
        return Err(NavSp1VerifyError::PolicyHashMismatch);
    }
    Ok(decoded)
}

fn decode_aggregate_v2_totals(bytes: &[u8]) -> Result<DecodedSp1PublicValues, NavSp1VerifyError> {
    if bytes.len() < 32 + 96 + 512 {
        return Err(NavSp1VerifyError::PublicValuesDecode);
    }
    let tuple_offset = read_word_usize(bytes, 0).map_err(|_| NavSp1VerifyError::PublicValuesDecode)?;
    if tuple_offset != 32 || tuple_offset >= bytes.len() {
        return Err(NavSp1VerifyError::PublicValuesDecode);
    }
    let base = tuple_offset;
    let schema_version = read_word_u32(bytes, base).map_err(|_| NavSp1VerifyError::PublicValuesDecode)?;
    if schema_version != AGGREGATE_PUBLIC_VALUES_V2_SCHEMA_VERSION {
        return Err(NavSp1VerifyError::SchemaVersionMismatch);
    }
    let policy_hash = read_word_bytes32(bytes, base + 64)
        .map_err(|_| NavSp1VerifyError::PublicValuesDecode)?;
    let totals_offset = base + 96;
    let spot_total = read_word_u128(bytes, totals_offset)
        .map_err(|_| NavSp1VerifyError::PublicValuesDecode)?;
    let cash_total = read_word_u128(bytes, totals_offset + 96)
        .map_err(|_| NavSp1VerifyError::PublicValuesDecode)?;
    let liability = read_word_u128(bytes, totals_offset + 224)
        .map_err(|_| NavSp1VerifyError::PublicValuesDecode)?;
    let legacy_cash_omitted_verified_net_assets = spot_total
        .checked_sub(liability)
        .and_then(|value| u64::try_from(value).ok());
    let verified_net_assets = spot_total
        .checked_add(cash_total)
        .ok_or(NavSp1VerifyError::PublicValuesDecode)?
        .checked_sub(liability)
        .ok_or(NavSp1VerifyError::PublicValuesDecode)?;
    let verified_net_assets = u64::try_from(verified_net_assets)
        .map_err(|_| NavSp1VerifyError::PublicValuesDecode)?;
    Ok(DecodedSp1PublicValues {
        policy_hash_hex: bytes_to_lower_hex(&policy_hash),
        verified_net_assets,
        legacy_cash_omitted_verified_net_assets,
    })
}

fn read_word_usize(bytes: &[u8], offset: usize) -> Result<usize, ()> {
    let value = read_word_u128(bytes, offset)?;
    usize::try_from(value).map_err(|_| ())
}

fn read_word_u32(bytes: &[u8], offset: usize) -> Result<u32, ()> {
    let value = read_word_u128(bytes, offset)?;
    u32::try_from(value).map_err(|_| ())
}

fn read_word_u128(bytes: &[u8], offset: usize) -> Result<u128, ()> {
    require_range(bytes, offset, 32)?;
    let word = &bytes[offset..offset + 32];
    if word[..16].iter().any(|byte| *byte != 0) {
        return Err(());
    }
    Ok(u128::from_be_bytes(word[16..32].try_into().map_err(|_| ())?))
}

fn read_word_bytes32(bytes: &[u8], offset: usize) -> Result<[u8; 32], ()> {
    require_range(bytes, offset, 32)?;
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes[offset..offset + 32]);
    Ok(out)
}

fn require_range(bytes: &[u8], offset: usize, len: usize) -> Result<(), ()> {
    let end = offset.checked_add(len).ok_or(())?;
    if end > bytes.len() {
        Err(())
    } else {
        Ok(())
    }
}

fn bytes_to_lower_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod unit_tests {
    use super::*;
    use postfiat_types::NavProofProfile;

    const FIXTURE_DIR: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/sp1-aggregate-regen-monero-crypto"
    );

    fn sp1_profile(policy_hash_hex: &str) -> NavProofProfile {
        NavProofProfile::new(
            "pfissuer",
            NAV_PROFILE_VERIFIER_SP1_GROTH16,
            "stakehub-pol-v2",
            100_000,
            1,
            100_000,
            0,
            0,
            0,
            0,
            policy_hash_hex,
            "0x004d1cd3f36e6ea60662af428edbea9d3aba45f04fe496da909d6bbe9fbf9258",
            "groth16",
            0,
            0,
        )
        .expect("profile")
    }

    fn fixture_bytes(name: &str) -> Vec<u8> {
        std::fs::read(format!("{FIXTURE_DIR}/{name}")).unwrap_or_else(|error| {
            panic!("missing fixture {name} at {FIXTURE_DIR}: {error}")
        })
    }

    fn write_word_u128(bytes: &mut [u8], offset: usize, value: u128) {
        bytes[offset + 16..offset + 32].copy_from_slice(&value.to_be_bytes());
    }

    #[test]
    fn decode_fixture_public_values_totals() {
        let public_values = fixture_bytes("aggregate-public-values.bin");
        let decoded = decode_aggregate_v2_totals(&public_values).expect("decode");
        assert_eq!(decoded.policy_hash_hex.len(), NAV_SP1_POLICY_HASH_HEX_LEN);
        assert_eq!(decoded.verified_net_assets, 2_364_869_341_670);
    }

    #[test]
    fn decode_public_values_includes_cash_in_verified_net_assets() {
        let mut public_values = vec![0_u8; 32 + 96 + 512];
        write_word_u128(&mut public_values, 0, 32);
        write_word_u128(
            &mut public_values,
            32,
            u128::from(AGGREGATE_PUBLIC_VALUES_V2_SCHEMA_VERSION),
        );
        public_values[96..128].copy_from_slice(&[0x11; 32]);

        let totals_offset = 128;
        write_word_u128(&mut public_values, totals_offset, 1_000);
        write_word_u128(&mut public_values, totals_offset + 96, 300);
        write_word_u128(&mut public_values, totals_offset + 192, 9_999);
        write_word_u128(&mut public_values, totals_offset + 224, 125);

        let decoded = decode_aggregate_v2_totals(&public_values).expect("decode");

        assert_eq!(decoded.policy_hash_hex, "11".repeat(32));
        assert_eq!(decoded.verified_net_assets, 1_175);
        assert_eq!(decoded.legacy_cash_omitted_verified_net_assets, Some(875));
    }

    #[test]
    fn nav_sp1_known_good_fixture_verifies_and_binds() {
        let public_values = fixture_bytes("aggregate-public-values.bin");
        let proof = fixture_bytes("aggregate-proof-calldata.bin");
        let decoded = decode_aggregate_v2_totals(&public_values).expect("decode");
        let profile = sp1_profile(&decoded.policy_hash_hex);
        let result = verify_sp1_groth16(
            &profile,
            decoded.verified_net_assets,
            &proof,
            &public_values,
        );
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn nav_sp1_rejects_tampered_proof() {
        let public_values = fixture_bytes("aggregate-public-values.bin");
        let mut proof = fixture_bytes("aggregate-proof-calldata.bin");
        proof[0] ^= 0xff;
        let decoded = decode_aggregate_v2_totals(&public_values).expect("decode");
        let profile = sp1_profile(&decoded.policy_hash_hex);
        assert_eq!(
            verify_sp1_groth16(
                &profile,
                decoded.verified_net_assets,
                &proof,
                &public_values,
            )
            .unwrap_err(),
            NavSp1VerifyError::Groth16Invalid
        );
    }

    #[test]
    fn nav_sp1_rejects_mismatched_verified_net_assets() {
        let public_values = fixture_bytes("aggregate-public-values.bin");
        let proof = fixture_bytes("aggregate-proof-calldata.bin");
        let decoded = decode_aggregate_v2_totals(&public_values).expect("decode");
        let profile = sp1_profile(&decoded.policy_hash_hex);
        assert_eq!(
            verify_sp1_groth16(
                &profile,
                decoded.verified_net_assets + 1,
                &proof,
                &public_values,
            )
            .unwrap_err(),
            NavSp1VerifyError::PublicValuesMismatch
        );
    }

    #[test]
    fn nav_sp1_legacy_cash_omitted_match_is_optioned() {
        let public_values = fixture_bytes("aggregate-public-values.bin");
        let proof = fixture_bytes("aggregate-proof-calldata.bin");
        let decoded = decode_aggregate_v2_totals(&public_values).expect("decode");
        let legacy_verified_net_assets = decoded
            .legacy_cash_omitted_verified_net_assets
            .expect("legacy cash-omitted total");
        assert_ne!(decoded.verified_net_assets, legacy_verified_net_assets);
        let profile = sp1_profile(&decoded.policy_hash_hex);

        assert_eq!(
            verify_sp1_groth16(
                &profile,
                legacy_verified_net_assets,
                &proof,
                &public_values,
            )
            .unwrap_err(),
            NavSp1VerifyError::PublicValuesMismatch
        );

        let result = verify_sp1_groth16_with_options(
            &profile,
            legacy_verified_net_assets,
            &proof,
            &public_values,
            NavSp1VerifyOptions {
                allow_legacy_cash_omitted_verified_net_assets: true,
            },
        );
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn nav_sp1_rejects_wrong_vkey() {
        let public_values = fixture_bytes("aggregate-public-values.bin");
        let proof = fixture_bytes("aggregate-proof-calldata.bin");
        let decoded = decode_aggregate_v2_totals(&public_values).expect("decode");
        let mut profile = sp1_profile(&decoded.policy_hash_hex);
        profile.sp1_program_vkey =
            "0x0000000000000000000000000000000000000000000000000000000000000001".to_string();
        assert_eq!(
            verify_sp1_groth16(
                &profile,
                decoded.verified_net_assets,
                &proof,
                &public_values,
            )
            .unwrap_err(),
            NavSp1VerifyError::Groth16Invalid
        );
    }

    #[test]
    fn nav_sp1_rejects_policy_hash_mismatch() {
        let public_values = fixture_bytes("aggregate-public-values.bin");
        let proof = fixture_bytes("aggregate-proof-calldata.bin");
        let decoded = decode_aggregate_v2_totals(&public_values).expect("decode");
        let profile = sp1_profile("22".repeat(32).as_str());
        assert_eq!(
            verify_sp1_groth16(
                &profile,
                decoded.verified_net_assets,
                &proof,
                &public_values,
            )
            .unwrap_err(),
            NavSp1VerifyError::PolicyHashMismatch
        );
    }
}
