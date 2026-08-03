pub use postfiat_nav_reserve_protocol::nav_reserve_subscription_composite_source_root_v1;
use postfiat_types::{
    AGGREGATE_PUBLIC_VALUES_V2_SCHEMA_VERSION, DEFAULT_MAX_NAV_SP1_PROOF_BYTES,
    DEFAULT_MAX_NAV_SP1_PUBLIC_VALUES_BYTES, NAV_PROFILE_VERIFIER_SP1_NAV_RESERVE_V1,
    NavReservePublicValuesV1,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavReserveVerifyContext<'a> {
    pub pftl_genesis_hash: &'a str,
    pub nav_asset_id: &'a str,
    pub proof_profile_id: &'a str,
    pub valuation_policy_hash: &'a str,
    pub source_manifest_hash: &'a str,
    pub valuation_unit_id: &'a str,
    pub observation_epoch: u64,
    pub current_height: u64,
    pub expected_proof_net_assets: u64,
    pub packet_source_root: &'a str,
    /// The packet's legacy-named `attestor_root`. For the provider-neutral
    /// reserve ABI this commits the valuation trust classification and the
    /// identities/evidence that support it. The independently proof-bound
    /// `source_disclosure_root` remains available in decoded packet details.
    pub packet_attestor_root: &'a str,
    pub subscription_overlay_source_root: Option<&'a str>,
    pub subscription_overlay_value: u64,
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
    GenesisMismatch,
    AssetMismatch,
    ProfileMismatch,
    ManifestMismatch,
    ValuationUnitMismatch,
    EpochMismatch,
    ObservationInFuture,
    ObservationStale,
    ObservationSpanExceeded,
    SourceRootMismatch,
    AttestorRootMismatch,
    ControlledValueForbidden,
}

/// Verify a bounded SP1 Groth16 proof without imposing the NAV aggregate
/// program's ABI. Dedicated bridge programs decode and bind their own public
/// values after this cryptographic check.
pub fn verify_bounded_sp1_groth16(
    profile: &NavProofProfile,
    expected_verifier_kind: &str,
    sp1_proof_bytes: &[u8],
    sp1_public_values: &[u8],
) -> Result<(), NavSp1VerifyError> {
    verify_bounded_sp1_groth16_with_config(
        &profile.verifier_kind,
        expected_verifier_kind,
        &profile.sp1_program_vkey,
        profile.max_proof_bytes,
        profile.max_public_values_bytes,
        sp1_proof_bytes,
        sp1_public_values,
    )
}

/// Verify a bounded SP1 Groth16 proof against a secondary verifier authority
/// that is not the route profile's primary verifier. This lets fast ingress
/// coexist with the immutable Tier-4 confirmed-ingress/egress route binding.
pub fn verify_bounded_sp1_groth16_with_config(
    verifier_kind: &str,
    expected_verifier_kind: &str,
    sp1_program_vkey: &str,
    configured_max_proof_bytes: u64,
    configured_max_public_values_bytes: u64,
    sp1_proof_bytes: &[u8],
    sp1_public_values: &[u8],
) -> Result<(), NavSp1VerifyError> {
    if verifier_kind != expected_verifier_kind {
        return Err(NavSp1VerifyError::Groth16Invalid);
    }
    if sp1_proof_bytes.is_empty() || sp1_public_values.is_empty() {
        return Err(NavSp1VerifyError::MissingProof);
    }
    let max_proof_bytes = if configured_max_proof_bytes == 0 {
        DEFAULT_MAX_NAV_SP1_PROOF_BYTES
    } else {
        configured_max_proof_bytes
    };
    let max_public_values_bytes = if configured_max_public_values_bytes == 0 {
        DEFAULT_MAX_NAV_SP1_PUBLIC_VALUES_BYTES
    } else {
        configured_max_public_values_bytes
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
        sp1_program_vkey,
        &GROTH16_VK_BYTES,
    )
    .map_err(|_| NavSp1VerifyError::Groth16Invalid)
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
            Self::GenesisMismatch => "nav_reserve_genesis_mismatch",
            Self::AssetMismatch => "nav_reserve_asset_mismatch",
            Self::ProfileMismatch => "nav_reserve_profile_mismatch",
            Self::ManifestMismatch => "nav_reserve_manifest_mismatch",
            Self::ValuationUnitMismatch => "nav_reserve_valuation_unit_mismatch",
            Self::EpochMismatch => "nav_reserve_epoch_mismatch",
            Self::ObservationInFuture => "nav_reserve_observation_in_future",
            Self::ObservationStale => "nav_reserve_observation_stale",
            Self::ObservationSpanExceeded => "nav_reserve_observation_span_exceeded",
            Self::SourceRootMismatch => "nav_reserve_source_root_mismatch",
            Self::AttestorRootMismatch => "nav_reserve_attestor_root_mismatch",
            Self::ControlledValueForbidden => "nav_reserve_controlled_value_forbidden",
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
            Self::PublicValuesDecode => "sp1 public values could not be decoded".to_string(),
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
            Self::GenesisMismatch => "reserve proof targets another PFTL genesis".to_string(),
            Self::AssetMismatch => "reserve proof targets another NAV asset".to_string(),
            Self::ProfileMismatch => "reserve proof targets another proof profile".to_string(),
            Self::ManifestMismatch => "reserve proof source manifest does not match the profile".to_string(),
            Self::ValuationUnitMismatch => "reserve proof valuation unit does not match the profile".to_string(),
            Self::EpochMismatch => "reserve proof observation epoch does not match the packet epoch".to_string(),
            Self::ObservationInFuture => "reserve proof observation interval is in the future".to_string(),
            Self::ObservationStale => "reserve proof observation interval is stale".to_string(),
            Self::ObservationSpanExceeded => "reserve proof observation interval exceeds the profile bound".to_string(),
            Self::SourceRootMismatch => "reserve proof observation root does not match the packet source root".to_string(),
            Self::AttestorRootMismatch => "reserve proof valuation trust root does not match the packet attestor root".to_string(),
            Self::ControlledValueForbidden => {
                "reserve proof contains a controlled source forbidden by the profile".to_string()
            }
        }
    }
}

/// Validate the provider-neutral reserve ABI after proof verification. Kept as
/// a separate function so every context and arithmetic binding can be tested
/// without manufacturing a Groth16 proof for each negative case.
pub fn validate_nav_reserve_public_values_context(
    profile: &NavProofProfile,
    values: &NavReservePublicValuesV1,
    context: &NavReserveVerifyContext<'_>,
) -> Result<(), NavSp1VerifyError> {
    values
        .validate()
        .map_err(|_| NavSp1VerifyError::PublicValuesDecode)?;
    if values.pftl_genesis_hash != context.pftl_genesis_hash {
        return Err(NavSp1VerifyError::GenesisMismatch);
    }
    if values.nav_asset_id != context.nav_asset_id {
        return Err(NavSp1VerifyError::AssetMismatch);
    }
    if values.proof_profile_id != context.proof_profile_id {
        return Err(NavSp1VerifyError::ProfileMismatch);
    }
    if values.valuation_policy_hash != context.valuation_policy_hash {
        return Err(NavSp1VerifyError::PolicyHashMismatch);
    }
    if values.source_manifest_hash != context.source_manifest_hash {
        return Err(NavSp1VerifyError::ManifestMismatch);
    }
    if values.valuation_unit_id != context.valuation_unit_id {
        return Err(NavSp1VerifyError::ValuationUnitMismatch);
    }
    if values.observation_epoch != context.observation_epoch {
        return Err(NavSp1VerifyError::EpochMismatch);
    }
    if values.observation_not_after > context.current_height {
        return Err(NavSp1VerifyError::ObservationInFuture);
    }
    let span = values
        .observation_not_after
        .checked_sub(values.observation_not_before)
        .ok_or(NavSp1VerifyError::PublicValuesDecode)?;
    if span > profile.max_observation_span_blocks {
        return Err(NavSp1VerifyError::ObservationSpanExceeded);
    }
    if profile.max_snapshot_age_blocks != 0
        && context
            .current_height
            .checked_sub(values.observation_not_after)
            .ok_or(NavSp1VerifyError::ObservationInFuture)?
            > profile.max_snapshot_age_blocks
    {
        return Err(NavSp1VerifyError::ObservationStale);
    }
    if values.verified_net_assets != context.expected_proof_net_assets {
        return Err(NavSp1VerifyError::PublicValuesMismatch);
    }
    let expected_source_root = if let Some(overlay_root) = context.subscription_overlay_source_root {
        nav_reserve_subscription_composite_source_root_v1(
            values,
            overlay_root,
            context.subscription_overlay_value,
        )
        .map_err(|_| NavSp1VerifyError::PublicValuesDecode)?
        .0
    } else {
        if context.subscription_overlay_value != 0 {
            return Err(NavSp1VerifyError::SourceRootMismatch);
        }
        values.source_observation_root.clone()
    };
    if expected_source_root != context.packet_source_root {
        return Err(NavSp1VerifyError::SourceRootMismatch);
    }
    if values.valuation_trust_root != context.packet_attestor_root {
        return Err(NavSp1VerifyError::AttestorRootMismatch);
    }
    if !profile.allow_controlled_sources
        && (values.controlled_value != 0
            || values.quantity_trust_counts.controlled != 0
            || values.valuation_trust_counts.controlled != 0)
    {
        return Err(NavSp1VerifyError::ControlledValueForbidden);
    }
    Ok(())
}

pub fn verify_nav_reserve_sp1_groth16(
    profile: &NavProofProfile,
    context: &NavReserveVerifyContext<'_>,
    sp1_proof_bytes: &[u8],
    sp1_public_values: &[u8],
) -> Result<NavReservePublicValuesV1, NavSp1VerifyError> {
    verify_bounded_sp1_groth16(
        profile,
        NAV_PROFILE_VERIFIER_SP1_NAV_RESERVE_V1,
        sp1_proof_bytes,
        sp1_public_values,
    )?;
    let values = NavReservePublicValuesV1::decode(sp1_public_values)
        .map_err(|_| NavSp1VerifyError::PublicValuesDecode)?;
    validate_nav_reserve_public_values_context(profile, &values, context)?;
    Ok(values)
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
    use postfiat_types::{NavProfileRegisterOperation, NavProofProfile};

    const FIXTURE_DIR: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/sp1-aggregate-regen-monero-crypto"
    );

    fn sp1_profile(policy_hash_hex: &str) -> NavProofProfile {
        NavProofProfile::new(
            "pfissuer",
            NAV_PROFILE_VERIFIER_SP1_GROTH16,
            "legacy-fixed-aggregate-v2",
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

    fn reserve_profile() -> NavProofProfile {
        NavProofProfile::new(
            "pfissuer",
            NAV_PROFILE_VERIFIER_SP1_GROTH16,
            "manifest-driven",
            20,
            1,
            100,
            0,
            0,
            0,
            0,
            "04".repeat(32),
            "0x004d1cd3f36e6ea60662af428edbea9d3aba45f04fe496da909d6bbe9fbf9258",
            "groth16",
            0,
            postfiat_types::NAV_RESERVE_PUBLIC_VALUES_V1_BYTES as u64,
        )
        .expect("SP1 base profile")
        .with_nav_reserve_bindings(
            postfiat_types::NAV_RESERVE_PUBLIC_VALUES_SCHEMA_V1,
            "05".repeat(48),
            "06".repeat(48),
            8,
            false,
        )
        .expect("reserve profile")
    }

    fn reserve_values(profile: &NavProofProfile) -> NavReservePublicValuesV1 {
        NavReservePublicValuesV1 {
            schema: postfiat_types::NAV_RESERVE_PUBLIC_VALUES_SCHEMA_V1.to_string(),
            pftl_genesis_hash: "01".repeat(48),
            nav_asset_id: "02".repeat(48),
            proof_profile_id: profile.profile_id.clone(),
            valuation_policy_hash: profile.valuation_policy_hash.clone(),
            source_manifest_hash: profile.source_manifest_hash.clone(),
            valuation_unit_id: profile.valuation_unit_id.clone(),
            valuation_scale: 1_000_000,
            observation_epoch: 7,
            observation_not_before: 90,
            observation_not_after: 95,
            source_observation_root: "07".repeat(48),
            gross_assets: 1_100,
            total_liabilities: 100,
            verified_net_assets: 1_000,
            cryptographically_verified_value: 600,
            attested_value: 400,
            controlled_value: 0,
            source_count: 2,
            quantity_trust_counts: postfiat_types::NavReserveTrustCountsV1 {
                cryptographic: 1,
                attested: 1,
                controlled: 0,
            },
            valuation_trust_counts: postfiat_types::NavReserveTrustCountsV1 {
                cryptographic: 0,
                attested: 2,
                controlled: 0,
            },
            quantity_trust_root: "08".repeat(48),
            valuation_trust_root: "09".repeat(48),
            source_disclosure_root: "0a".repeat(48),
        }
    }

    fn chain_bound_qualified_reserve_profile() -> NavProofProfile {
        NavProfileRegisterOperation {
            registrant: "pf0fae169e4293feebc8c9119febb4fd995a667b37".to_string(),
            verifier_kind: NAV_PROFILE_VERIFIER_SP1_NAV_RESERVE_V1.to_string(),
            source_class: "manifest-driven".to_string(),
            max_snapshot_age_blocks: 10_000,
            challenge_window_blocks: 1,
            max_epoch_gap_blocks: 100,
            settle_deadline_blocks: 0,
            min_challenge_bond: 0,
            min_attestations: 0,
            tolerance_bp: 0,
            bridge_observer_min_confirmations: 0,
            valuation_policy_hash: "04".repeat(32),
            vault_bridge_route_policy_hash: String::new(),
            sp1_program_vkey:
                "0x000c7271e0711abce0c61d293222fd4a144599a779db8cadadc4df35e31a4100"
                    .to_string(),
            sp1_proof_encoding: "groth16".to_string(),
            max_proof_bytes: 4_096,
            max_public_values_bytes: postfiat_types::NAV_RESERVE_PUBLIC_VALUES_V1_BYTES as u64,
            public_values_schema: postfiat_types::NAV_RESERVE_PUBLIC_VALUES_SCHEMA_V1.to_string(),
            source_manifest_hash: "9da4e2ba55939f138475026946d2728d9b40d3f4c7762289a70aae94584eac924b9a788c6df25c9276cc83f1616ef0e5".to_string(),
            valuation_unit_id: "05".repeat(48),
            max_observation_span_blocks: 8,
            allow_controlled_sources: true,
        }
        .to_profile()
        .expect("derive chain-bound qualified profile")
    }

    fn a666_shadow_reserve_profile() -> NavProofProfile {
        NavProfileRegisterOperation {
            registrant: "pffcb93d9f87a843a8aa34e1adf241f5d58143e81b".to_string(),
            verifier_kind: NAV_PROFILE_VERIFIER_SP1_NAV_RESERVE_V1.to_string(),
            source_class: "manifest-driven-a666-reserves-v1".to_string(),
            max_snapshot_age_blocks: 900,
            challenge_window_blocks: 1,
            max_epoch_gap_blocks: 128,
            settle_deadline_blocks: 256,
            min_challenge_bond: 0,
            min_attestations: 0,
            tolerance_bp: 0,
            bridge_observer_min_confirmations: 0,
            valuation_policy_hash:
                "076c071e44127158ef82350e7feeb64e0be0a06bf8ba4be5f0374ac36b992ac7"
                    .to_string(),
            vault_bridge_route_policy_hash: String::new(),
            sp1_program_vkey:
                "0x007e32678376339d48df4db28a9825d5fb229cedb8b2e5c92295d4580c9d32f8"
                    .to_string(),
            sp1_proof_encoding: "groth16".to_string(),
            max_proof_bytes: 4_096,
            max_public_values_bytes: postfiat_types::NAV_RESERVE_PUBLIC_VALUES_V1_BYTES as u64,
            public_values_schema: postfiat_types::NAV_RESERVE_PUBLIC_VALUES_SCHEMA_V1.to_string(),
            source_manifest_hash: "56fdd19addb6d4e19e0e60094576bf481602498f0252c99cc37ab480621f21ce8d68e348261b65ba78aa5648ec88e5aa".to_string(),
            valuation_unit_id: "c67872c31caa85cbe6dd287a1e060f0f5cfc0e9f3c5bd85a7569897fd0cefb031583b7afc001e7d1afa492e9abf77d60".to_string(),
            max_observation_span_blocks: 8,
            allow_controlled_sources: false,
        }
        .to_profile()
        .expect("derive A666 shadow reserve profile")
    }

    #[test]
    fn nav_reserve_v1_chain_bound_real_proof_verifies_in_consensus() {
        let proof = postfiat_crypto_provider::hex_to_bytes(include_str!(
            "../testdata/nav-reserve-v1-qualified-proof-calldata.hex"
        )
        .trim())
        .expect("decode qualified proof calldata");
        let public_values = postfiat_crypto_provider::hex_to_bytes(include_str!(
            "../testdata/nav-reserve-v1-qualified-public-values.hex"
        )
        .trim())
        .expect("decode qualified public values");
        let profile = chain_bound_qualified_reserve_profile();
        assert_eq!(
            profile.profile_id,
            "3d78cac1f539d3d2e56f6f38c958242aa0bcd13661c733834896bc9c49a48211d716bd4cad83d478b2fa5d85b22a0c7e"
        );
        let values = NavReservePublicValuesV1::decode(&public_values)
            .expect("decode qualified reserve public values");
        let context = NavReserveVerifyContext {
            pftl_genesis_hash: &values.pftl_genesis_hash,
            nav_asset_id: &values.nav_asset_id,
            proof_profile_id: &profile.profile_id,
            valuation_policy_hash: &profile.valuation_policy_hash,
            source_manifest_hash: &profile.source_manifest_hash,
            valuation_unit_id: &profile.valuation_unit_id,
            observation_epoch: values.observation_epoch,
            current_height: 776,
            expected_proof_net_assets: 1_100,
            packet_source_root: &values.source_observation_root,
            packet_attestor_root: &values.valuation_trust_root,
            subscription_overlay_source_root: None,
            subscription_overlay_value: 0,
        };
        let verified = verify_nav_reserve_sp1_groth16(
            &profile,
            &context,
            &proof,
            &public_values,
        )
        .expect("consensus accepts chain-bound qualified proof");
        assert_eq!(verified, values);

        let mut tampered = proof;
        tampered[100] ^= 1;
        assert_eq!(
            verify_nav_reserve_sp1_groth16(
                &profile,
                &context,
                &tampered,
                &public_values,
            )
            .expect_err("tampered qualified proof must reject"),
            NavSp1VerifyError::Groth16Invalid,
        );
    }

    #[test]
    fn a666_successor_shadow_real_proof_verifies_in_consensus() {
        let proof = postfiat_crypto_provider::hex_to_bytes(include_str!(
            "../testdata/a666-nav-reserve-v1-shadow-proof-calldata.hex"
        )
        .trim())
        .expect("decode A666 shadow proof calldata");
        let public_values = postfiat_crypto_provider::hex_to_bytes(include_str!(
            "../testdata/a666-nav-reserve-v1-shadow-public-values.hex"
        )
        .trim())
        .expect("decode A666 shadow public values");
        let profile = a666_shadow_reserve_profile();
        assert_eq!(
            profile.profile_id,
            "a18c1bbee443f5f9958592cf65f4c16ee837c5c717cf9144f5b54f90bea267c80b4ae161001c2f7e5267fdc424c366c3"
        );
        let values = NavReservePublicValuesV1::decode(&public_values)
            .expect("decode A666 shadow reserve public values");
        assert_eq!(
            values.nav_asset_id,
            "521c6c630bb48d4a37ab4a7bd4900dd2caa2d9e99499e452da3c7ce75b3d74b62d20e18555642bec32174498cbee5e2c"
        );
        assert_eq!(values.gross_assets, 2_846_461_376_975);
        assert_eq!(values.total_liabilities, 20_088_300_169);
        assert_eq!(values.verified_net_assets, 2_826_373_076_806);
        let context = NavReserveVerifyContext {
            pftl_genesis_hash: &values.pftl_genesis_hash,
            nav_asset_id: &values.nav_asset_id,
            proof_profile_id: &profile.profile_id,
            valuation_policy_hash: &profile.valuation_policy_hash,
            source_manifest_hash: &profile.source_manifest_hash,
            valuation_unit_id: &profile.valuation_unit_id,
            observation_epoch: 3,
            current_height: 776,
            expected_proof_net_assets: 2_826_373_076_806,
            packet_source_root: &values.source_observation_root,
            packet_attestor_root: &values.valuation_trust_root,
            subscription_overlay_source_root: None,
            subscription_overlay_value: 0,
        };
        assert_eq!(
            verify_nav_reserve_sp1_groth16(&profile, &context, &proof, &public_values)
                .expect("consensus accepts the A666 successor shadow proof"),
            values
        );

        let mut tampered = proof;
        tampered[100] ^= 1;
        assert_eq!(
            verify_nav_reserve_sp1_groth16(
                &profile,
                &context,
                &tampered,
                &public_values,
            )
            .expect_err("tampered A666 shadow proof must reject"),
            NavSp1VerifyError::Groth16Invalid,
        );
    }

    fn reserve_context<'a>(
        profile: &'a NavProofProfile,
        values: &'a NavReservePublicValuesV1,
    ) -> NavReserveVerifyContext<'a> {
        NavReserveVerifyContext {
            pftl_genesis_hash: &values.pftl_genesis_hash,
            nav_asset_id: &values.nav_asset_id,
            proof_profile_id: &profile.profile_id,
            valuation_policy_hash: &profile.valuation_policy_hash,
            source_manifest_hash: &profile.source_manifest_hash,
            valuation_unit_id: &profile.valuation_unit_id,
            observation_epoch: values.observation_epoch,
            current_height: 100,
            expected_proof_net_assets: values.verified_net_assets,
            packet_source_root: &values.source_observation_root,
            packet_attestor_root: &values.valuation_trust_root,
            subscription_overlay_source_root: None,
            subscription_overlay_value: 0,
        }
    }

    #[test]
    fn nav_reserve_context_accepts_fully_bound_values() {
        let profile = reserve_profile();
        let values = reserve_values(&profile);
        assert_eq!(
            validate_nav_reserve_public_values_context(
                &profile,
                &values,
                &reserve_context(&profile, &values),
            ),
            Ok(())
        );
    }

    #[test]
    fn nav_reserve_context_rejects_substitution_and_replay() {
        let profile = reserve_profile();
        let values = reserve_values(&profile);

        let mut context = reserve_context(&profile, &values);
        context.pftl_genesis_hash = "11";
        assert_eq!(
            validate_nav_reserve_public_values_context(&profile, &values, &context),
            Err(NavSp1VerifyError::GenesisMismatch)
        );
        let mut context = reserve_context(&profile, &values);
        context.nav_asset_id = "22";
        assert_eq!(
            validate_nav_reserve_public_values_context(&profile, &values, &context),
            Err(NavSp1VerifyError::AssetMismatch)
        );
        let mut context = reserve_context(&profile, &values);
        context.proof_profile_id = "33";
        assert_eq!(
            validate_nav_reserve_public_values_context(&profile, &values, &context),
            Err(NavSp1VerifyError::ProfileMismatch)
        );
        let mut context = reserve_context(&profile, &values);
        context.source_manifest_hash = "44";
        assert_eq!(
            validate_nav_reserve_public_values_context(&profile, &values, &context),
            Err(NavSp1VerifyError::ManifestMismatch)
        );
        let mut context = reserve_context(&profile, &values);
        context.observation_epoch += 1;
        assert_eq!(
            validate_nav_reserve_public_values_context(&profile, &values, &context),
            Err(NavSp1VerifyError::EpochMismatch)
        );
    }

    #[test]
    fn nav_reserve_context_rejects_time_roots_totals_and_controlled_value() {
        let profile = reserve_profile();
        let values = reserve_values(&profile);

        let mut context = reserve_context(&profile, &values);
        context.current_height = 94;
        assert_eq!(
            validate_nav_reserve_public_values_context(&profile, &values, &context),
            Err(NavSp1VerifyError::ObservationInFuture)
        );
        let mut context = reserve_context(&profile, &values);
        context.current_height = 116;
        assert_eq!(
            validate_nav_reserve_public_values_context(&profile, &values, &context),
            Err(NavSp1VerifyError::ObservationStale)
        );
        let mut long_values = values.clone();
        long_values.observation_not_before = 80;
        assert_eq!(
            validate_nav_reserve_public_values_context(
                &profile,
                &long_values,
                &reserve_context(&profile, &long_values),
            ),
            Err(NavSp1VerifyError::ObservationSpanExceeded)
        );
        let mut context = reserve_context(&profile, &values);
        context.expected_proof_net_assets += 1;
        assert_eq!(
            validate_nav_reserve_public_values_context(&profile, &values, &context),
            Err(NavSp1VerifyError::PublicValuesMismatch)
        );
        let mut context = reserve_context(&profile, &values);
        context.packet_source_root = "77";
        assert_eq!(
            validate_nav_reserve_public_values_context(&profile, &values, &context),
            Err(NavSp1VerifyError::SourceRootMismatch)
        );
        let mut context = reserve_context(&profile, &values);
        context.packet_attestor_root = "88";
        assert_eq!(
            validate_nav_reserve_public_values_context(&profile, &values, &context),
            Err(NavSp1VerifyError::AttestorRootMismatch)
        );
        let mut controlled = values.clone();
        controlled.controlled_value = 1;
        controlled.attested_value -= 1;
        assert_eq!(
            validate_nav_reserve_public_values_context(
                &profile,
                &controlled,
                &reserve_context(&profile, &controlled),
            ),
            Err(NavSp1VerifyError::ControlledValueForbidden)
        );

        let mut zero_value_controlled_source = values.clone();
        zero_value_controlled_source
            .quantity_trust_counts
            .cryptographic -= 1;
        zero_value_controlled_source.quantity_trust_counts.controlled += 1;
        assert_eq!(
            validate_nav_reserve_public_values_context(
                &profile,
                &zero_value_controlled_source,
                &reserve_context(&profile, &zero_value_controlled_source),
            ),
            Err(NavSp1VerifyError::ControlledValueForbidden)
        );
    }
}
