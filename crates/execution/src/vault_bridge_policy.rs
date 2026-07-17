#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VaultBridgePolicyError {
    HaircutBpsTooHigh,
    ArithmeticOverflow,
    ZeroBucketClaims,
}

impl VaultBridgePolicyError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::HaircutBpsTooHigh => "vault_bridge_haircut_bps_too_high",
            Self::ArithmeticOverflow => "vault_bridge_policy_arithmetic_overflow",
            Self::ZeroBucketClaims => "vault_bridge_zero_bucket_claims",
        }
    }

    pub fn message(&self) -> &'static str {
        match self {
            Self::HaircutBpsTooHigh => "vault bridge asset haircut basis points exceed 10000",
            Self::ArithmeticOverflow => "vault bridge asset policy arithmetic overflowed",
            Self::ZeroBucketClaims => "vault bridge asset bucket claims must be nonzero",
        }
    }
}

pub fn compute_counted_value(
    amount_atoms: u64,
    haircut_bps: u64,
) -> Result<u64, VaultBridgePolicyError> {
    if haircut_bps > 10_000 {
        return Err(VaultBridgePolicyError::HaircutBpsTooHigh);
    }
    let retained_bps = 10_000_u64
        .checked_sub(haircut_bps)
        .ok_or(VaultBridgePolicyError::ArithmeticOverflow)?;
    amount_atoms
        .checked_mul(retained_bps)
        .ok_or(VaultBridgePolicyError::ArithmeticOverflow)
        .map(|value| value / 10_000)
}

pub fn bucket_claim_atoms(
    outstanding_vault_bridge_atoms: u64,
    nav_subscription_allocations_atoms: u64,
    redemption_queue_atoms: u64,
    other_allocations_atoms: u64,
) -> Result<u64, VaultBridgePolicyError> {
    outstanding_vault_bridge_atoms
        .checked_add(nav_subscription_allocations_atoms)
        .and_then(|value| value.checked_add(redemption_queue_atoms))
        .and_then(|value| value.checked_add(other_allocations_atoms))
        .ok_or(VaultBridgePolicyError::ArithmeticOverflow)
}

pub fn bucket_factor_bps(
    recoverable_counted_atoms: u64,
    bucket_claim_atoms: u64,
) -> Result<u64, VaultBridgePolicyError> {
    if bucket_claim_atoms == 0 {
        return Err(VaultBridgePolicyError::ZeroBucketClaims);
    }
    let scaled = (recoverable_counted_atoms as u128)
        .checked_mul(10_000)
        .ok_or(VaultBridgePolicyError::ArithmeticOverflow)?
        / bucket_claim_atoms as u128;
    Ok(scaled.min(10_000) as u64)
}

pub fn redeemable_atoms(
    claim_atoms: u64,
    bucket_factor_bps: u64,
) -> Result<u64, VaultBridgePolicyError> {
    if bucket_factor_bps > 10_000 {
        return Err(VaultBridgePolicyError::HaircutBpsTooHigh);
    }
    claim_atoms
        .checked_mul(bucket_factor_bps)
        .ok_or(VaultBridgePolicyError::ArithmeticOverflow)
        .map(|value| value / 10_000)
}
