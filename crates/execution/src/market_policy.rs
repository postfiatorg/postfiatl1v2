use core::fmt;

use postfiat_types::{
    market_ops_evm_evidence_root, MarketOpsEvmEvidenceBundle, MarketOpsEvmPoolStateEvidence,
};

pub const BPS: u128 = 10_000;
pub const USD_SCALE: u128 = 100_000_000;
pub const DEFAULT_UNIT_SCALE: u128 = 1_000_000_000_000_000_000;
pub const WEIGHT_DENOM: u128 = 100;
pub const DISCOUNT_TIME_WEIGHT: u128 = 25;
pub const DISCOUNT_VOLUME_WEIGHT: u128 = 15;
pub const DISCOUNT_SEVERITY_WEIGHT: u128 = 150;
pub const MAX_DISCOUNT_RESPONSE_BPS: u128 = 2_500;
pub const PREMIUM_TIME_WEIGHT: u128 = 2;
pub const PREMIUM_VOLUME_WEIGHT: u128 = 1;
pub const PREMIUM_SEVERITY_WEIGHT: u128 = 10;
pub const MAX_PREMIUM_RESPONSE_BPS: u128 = 1_500;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketPolicyError {
    DivisionByZero,
    EmptyValues,
    EvidenceRootMismatch,
    InvalidBps,
    InvalidEvidence,
    InvalidWindow,
    IneligibleEvidence,
    Overflow,
    SlippageLimitExceeded,
}

impl fmt::Display for MarketPolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DivisionByZero => write!(f, "market policy division by zero"),
            Self::EmptyValues => write!(f, "market policy percentile requires values"),
            Self::EvidenceRootMismatch => write!(f, "market policy evidence root mismatch"),
            Self::InvalidBps => write!(f, "market policy bps value exceeds 10000"),
            Self::InvalidEvidence => write!(f, "market policy evidence bundle is invalid"),
            Self::InvalidWindow => write!(f, "market policy venue window is invalid"),
            Self::IneligibleEvidence => {
                write!(f, "market policy evidence is ineligible for automatic caps")
            }
            Self::Overflow => write!(f, "market policy arithmetic overflow"),
            Self::SlippageLimitExceeded => {
                write!(f, "market policy quote exceeds slippage limit")
            }
        }
    }
}

impl std::error::Error for MarketPolicyError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NavFloor {
    pub nav_per_unit_usd_e8: u128,
    pub nav_floor_usd_e8: u128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackingCapacity {
    pub backing_required_usd_e8: u128,
    pub verified_capacity_remaining_usd_e8: u128,
    pub verified_capacity_remaining_atoms: u128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlignmentReserveParams {
    pub policy_min_usd_e8: u128,
    pub min_alignment_bps: u128,
    pub stress_repeat_factor_14d: u128,
    pub stress_repeat_factor_90d: u128,
    pub stale_epochs_allowed: u128,
    pub max_decay_per_epoch_bps: u128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlignmentReserveRequirement {
    pub minimum_alignment_reserve_usd_e8: u128,
    pub stress_support_need_14d_usd_e8: u128,
    pub stress_support_need_90d_usd_e8: u128,
    pub latency_buffer_usd_e8: u128,
    pub raw_required_alignment_reserve_usd_e8: u128,
    pub required_alignment_reserve_next_usd_e8: u128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResponseCurveMetrics {
    pub frequency_time_bps: u128,
    pub frequency_volume_bps: u128,
    pub severity_bps: u128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResponseCurveWeights {
    pub time_weight: u128,
    pub volume_weight: u128,
    pub severity_weight: u128,
    pub max_response_bps: u128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VenueObservation {
    pub dt_seconds: u128,
    pub price_usd_e8: u128,
    pub volume_usd_e8: u128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VenueMetrics {
    pub breach_time_seconds: u128,
    pub frequency_time_bps: u128,
    pub frequency_volume_bps: u128,
    pub severity_bps: u128,
}

impl VenueMetrics {
    pub fn response_curve_metrics(self) -> ResponseCurveMetrics {
        ResponseCurveMetrics {
            frequency_time_bps: self.frequency_time_bps,
            frequency_volume_bps: self.frequency_volume_bps,
            severity_bps: self.severity_bps,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VenueEvidenceReplayConfig {
    pub expected_venue_id: [u8; 32],
    pub expected_pool_config_hash: [u8; 32],
    pub expected_hook_code_hash: [u8; 32],
    pub unit_scale: u128,
    pub nav_floor_usd_e8: u128,
    pub discount_trigger_bps: u128,
    pub premium_trigger_bps: u128,
    pub slippage_limit_bps: u128,
    pub data_window_start: u64,
    pub data_window_end: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VenueEvidenceReplayReport {
    pub evidence_root: [u8; 32],
    pub eligible_observations: Vec<VenueObservation>,
    pub ineligible_observation_count: usize,
    pub discount_metrics: VenueMetrics,
    pub premium_metrics: VenueMetrics,
    pub discount_response_bps: u128,
    pub premium_response_bps: u128,
    pub latest_depth_limited_cap_usd_e8: u128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConstantProductPoolState {
    pub base_reserve_atoms: u128,
    pub quote_reserve_usd_e8: u128,
    pub unit_scale: u128,
    pub fee_bps: u128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PriceMove {
    RaiseWithQuote,
    LowerWithBase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReserveDeployLimits {
    pub available_alignment_reserve_usd_e8: u128,
    pub venue_policy_cap_usd_e8: u128,
    pub depth_limited_cap_usd_e8: u128,
    pub cooldown_limited_cap_usd_e8: u128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReserveDeployCap {
    pub response_cap_usd_e8: u128,
    pub reserve_deploy_cap_usd_e8: u128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MintCapLimits {
    pub policy_max_mint_atoms: u128,
    pub venue_bid_depth_atoms: u128,
    pub cooldown_mint_atoms: u128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MintCap {
    pub market_response_mint_atoms: u128,
    pub mint_cap_atoms: u128,
}

pub fn replay_evm_venue_evidence(
    bundle: &MarketOpsEvmEvidenceBundle,
    expected_evidence_root: [u8; 32],
    config: VenueEvidenceReplayConfig,
) -> Result<VenueEvidenceReplayReport, MarketPolicyError> {
    check_bps(config.discount_trigger_bps)?;
    check_bps(config.premium_trigger_bps)?;
    check_bps(config.slippage_limit_bps)?;
    if config.unit_scale == 0 || config.nav_floor_usd_e8 == 0 {
        return Err(MarketPolicyError::DivisionByZero);
    }
    if config.data_window_start >= config.data_window_end {
        return Err(MarketPolicyError::InvalidWindow);
    }
    bundle
        .validate()
        .map_err(|_| MarketPolicyError::InvalidEvidence)?;
    let evidence_root =
        market_ops_evm_evidence_root(bundle).map_err(|_| MarketPolicyError::InvalidEvidence)?;
    if evidence_root != expected_evidence_root {
        return Err(MarketPolicyError::EvidenceRootMismatch);
    }
    if bundle.venue_id != config.expected_venue_id
        || bundle.pool_config_hash != config.expected_pool_config_hash
        || bundle.hook_code_hash != config.expected_hook_code_hash
    {
        return Err(MarketPolicyError::IneligibleEvidence);
    }

    let discount_boundary =
        compute_discount_boundary(config.nav_floor_usd_e8, config.discount_trigger_bps)?;
    let premium_boundary =
        compute_premium_boundary(config.nav_floor_usd_e8, config.premium_trigger_bps)?;
    let mut eligible_observations = Vec::new();
    let mut ineligible_observation_count = 0usize;
    let mut latest_depth_limited_cap_usd_e8 = 0u128;

    for pool_state in &bundle.pool_states {
        if pool_state.timestamp < config.data_window_start
            || pool_state.timestamp > config.data_window_end
            || !checkpoint_covers_pool_state(bundle, pool_state)
        {
            ineligible_observation_count = ineligible_observation_count
                .checked_add(1)
                .ok_or(MarketPolicyError::Overflow)?;
            continue;
        }

        let Ok(pool) = exact_constant_product_pool_state(pool_state, config.unit_scale) else {
            ineligible_observation_count = ineligible_observation_count
                .checked_add(1)
                .ok_or(MarketPolicyError::Overflow)?;
            continue;
        };
        let Ok(replayed_price_usd_e8) = constant_product_spot_price_usd_e8(pool) else {
            ineligible_observation_count = ineligible_observation_count
                .checked_add(1)
                .ok_or(MarketPolicyError::Overflow)?;
            continue;
        };
        if replayed_price_usd_e8 != pool_state.price_usd_e8 {
            ineligible_observation_count = ineligible_observation_count
                .checked_add(1)
                .ok_or(MarketPolicyError::Overflow)?;
            continue;
        }
        if pool_state.price_usd_e8 < discount_boundary
            && quote_cost_to_reach_price_usd_e8(pool, discount_boundary, config.slippage_limit_bps)
                .is_err()
        {
            ineligible_observation_count = ineligible_observation_count
                .checked_add(1)
                .ok_or(MarketPolicyError::Overflow)?;
            continue;
        }
        if pool_state.price_usd_e8 > premium_boundary
            && quote_cost_to_reach_price_usd_e8(pool, premium_boundary, config.slippage_limit_bps)
                .is_err()
        {
            ineligible_observation_count = ineligible_observation_count
                .checked_add(1)
                .ok_or(MarketPolicyError::Overflow)?;
            continue;
        }

        latest_depth_limited_cap_usd_e8 = if pool_state.price_usd_e8 < discount_boundary {
            quote_cost_to_reach_price_usd_e8(pool, discount_boundary, config.slippage_limit_bps)?
        } else {
            0
        };
        eligible_observations.push(VenueObservation {
            dt_seconds: u128::from(pool_state.dt_seconds),
            price_usd_e8: pool_state.price_usd_e8,
            volume_usd_e8: pool_state.volume_usd_e8,
        });
    }

    if eligible_observations.is_empty() {
        return Err(MarketPolicyError::IneligibleEvidence);
    }

    let window_seconds = config
        .data_window_end
        .checked_sub(config.data_window_start)
        .ok_or(MarketPolicyError::InvalidWindow)?;
    let discount_metrics = compute_discount_metrics(
        &eligible_observations,
        config.nav_floor_usd_e8,
        config.discount_trigger_bps,
        u128::from(window_seconds),
    )?;
    let premium_metrics = compute_premium_metrics(
        &eligible_observations,
        config.nav_floor_usd_e8,
        config.premium_trigger_bps,
        u128::from(window_seconds),
    )?;
    let discount_response_bps =
        compute_discount_response_bps(discount_metrics.response_curve_metrics())?;
    let premium_response_bps =
        compute_premium_response_bps(premium_metrics.response_curve_metrics())?;

    Ok(VenueEvidenceReplayReport {
        evidence_root,
        eligible_observations,
        ineligible_observation_count,
        discount_metrics,
        premium_metrics,
        discount_response_bps,
        premium_response_bps,
        latest_depth_limited_cap_usd_e8,
    })
}

pub fn constant_product_spot_price_usd_e8(
    pool: ConstantProductPoolState,
) -> Result<u128, MarketPolicyError> {
    if pool.base_reserve_atoms == 0 || pool.unit_scale == 0 {
        return Err(MarketPolicyError::DivisionByZero);
    }
    mul_div_floor(
        pool.quote_reserve_usd_e8,
        pool.unit_scale,
        pool.base_reserve_atoms,
    )
}

pub fn quote_cost_to_reach_price_usd_e8(
    pool: ConstantProductPoolState,
    target_price_usd_e8: u128,
    slippage_limit_bps: u128,
) -> Result<u128, MarketPolicyError> {
    check_bps(slippage_limit_bps)?;
    check_bps(pool.fee_bps)?;
    if target_price_usd_e8 == 0 {
        return Err(MarketPolicyError::DivisionByZero);
    }
    let current_price = constant_product_spot_price_usd_e8(pool)?;
    if current_price == target_price_usd_e8 {
        return Ok(0);
    }
    let price_move = if current_price < target_price_usd_e8 {
        PriceMove::RaiseWithQuote
    } else {
        PriceMove::LowerWithBase
    };
    let price_delta = current_price.abs_diff(target_price_usd_e8);
    let move_bps = mul_div_ceil(price_delta, BPS, current_price)?;
    if move_bps > slippage_limit_bps {
        return Err(MarketPolicyError::SlippageLimitExceeded);
    }

    let invariant = pool
        .base_reserve_atoms
        .checked_mul(pool.quote_reserve_usd_e8)
        .ok_or(MarketPolicyError::Overflow)?;
    let target_quote_square = mul_div_floor(target_price_usd_e8, invariant, pool.unit_scale)?;
    let target_quote_reserve = integer_sqrt_ceil(target_quote_square)?;
    match price_move {
        PriceMove::RaiseWithQuote => {
            if target_quote_reserve <= pool.quote_reserve_usd_e8 {
                return Ok(0);
            }
            let quote_delta = target_quote_reserve - pool.quote_reserve_usd_e8;
            let effective_input_bps = BPS
                .checked_sub(pool.fee_bps)
                .ok_or(MarketPolicyError::InvalidBps)?;
            if effective_input_bps == 0 {
                return Err(MarketPolicyError::DivisionByZero);
            }
            mul_div_ceil(quote_delta, BPS, effective_input_bps)
        }
        PriceMove::LowerWithBase => Ok(pool
            .quote_reserve_usd_e8
            .saturating_sub(target_quote_reserve)),
    }
}

pub fn mul_div_floor(x: u128, y: u128, z: u128) -> Result<u128, MarketPolicyError> {
    if z == 0 {
        return Err(MarketPolicyError::DivisionByZero);
    }
    Ok(x.checked_mul(y).ok_or(MarketPolicyError::Overflow)? / z)
}

pub fn mul_div_ceil(x: u128, y: u128, z: u128) -> Result<u128, MarketPolicyError> {
    if z == 0 {
        return Err(MarketPolicyError::DivisionByZero);
    }
    let product = x.checked_mul(y).ok_or(MarketPolicyError::Overflow)?;
    let quotient = product / z;
    if product % z == 0 {
        Ok(quotient)
    } else {
        quotient.checked_add(1).ok_or(MarketPolicyError::Overflow)
    }
}

pub fn compute_nav_floor(
    verified_net_assets_usd_e8: u128,
    valid_global_supply_atoms: u128,
    floor_factor_bps: u128,
) -> Result<NavFloor, MarketPolicyError> {
    compute_nav_floor_with_unit_scale(
        verified_net_assets_usd_e8,
        valid_global_supply_atoms,
        floor_factor_bps,
        DEFAULT_UNIT_SCALE,
    )
}

pub fn compute_nav_floor_with_unit_scale(
    verified_net_assets_usd_e8: u128,
    valid_global_supply_atoms: u128,
    floor_factor_bps: u128,
    unit_scale: u128,
) -> Result<NavFloor, MarketPolicyError> {
    check_bps(floor_factor_bps)?;
    if valid_global_supply_atoms == 0 || unit_scale == 0 {
        return Err(MarketPolicyError::DivisionByZero);
    }
    let nav_per_unit_usd_e8 = mul_div_floor(
        verified_net_assets_usd_e8,
        unit_scale,
        valid_global_supply_atoms,
    )?;
    let nav_floor_usd_e8 = mul_div_floor(nav_per_unit_usd_e8, floor_factor_bps, BPS)?;
    Ok(NavFloor {
        nav_per_unit_usd_e8,
        nav_floor_usd_e8,
    })
}

pub fn compute_backing_capacity(
    verified_net_assets_usd_e8: u128,
    valid_global_supply_atoms: u128,
    nav_floor_usd_e8: u128,
) -> Result<BackingCapacity, MarketPolicyError> {
    compute_backing_capacity_with_unit_scale(
        verified_net_assets_usd_e8,
        valid_global_supply_atoms,
        nav_floor_usd_e8,
        DEFAULT_UNIT_SCALE,
    )
}

pub fn compute_backing_capacity_with_unit_scale(
    verified_net_assets_usd_e8: u128,
    valid_global_supply_atoms: u128,
    nav_floor_usd_e8: u128,
    unit_scale: u128,
) -> Result<BackingCapacity, MarketPolicyError> {
    if nav_floor_usd_e8 == 0 || unit_scale == 0 {
        return Err(MarketPolicyError::DivisionByZero);
    }
    let backing_required_usd_e8 =
        mul_div_floor(valid_global_supply_atoms, nav_floor_usd_e8, unit_scale)?;
    let verified_capacity_remaining_usd_e8 =
        verified_net_assets_usd_e8.saturating_sub(backing_required_usd_e8);
    let verified_capacity_remaining_atoms = mul_div_floor(
        verified_capacity_remaining_usd_e8,
        unit_scale,
        nav_floor_usd_e8,
    )?;
    Ok(BackingCapacity {
        backing_required_usd_e8,
        verified_capacity_remaining_usd_e8,
        verified_capacity_remaining_atoms,
    })
}

pub fn post_mint_backing_invariant_holds(
    verified_net_assets_after_usd_e8: u128,
    valid_global_supply_after_atoms: u128,
    nav_floor_usd_e8: u128,
) -> Result<bool, MarketPolicyError> {
    post_mint_backing_invariant_holds_with_unit_scale(
        verified_net_assets_after_usd_e8,
        valid_global_supply_after_atoms,
        nav_floor_usd_e8,
        DEFAULT_UNIT_SCALE,
    )
}

pub fn post_mint_backing_invariant_holds_with_unit_scale(
    verified_net_assets_after_usd_e8: u128,
    valid_global_supply_after_atoms: u128,
    nav_floor_usd_e8: u128,
    unit_scale: u128,
) -> Result<bool, MarketPolicyError> {
    let lhs = verified_net_assets_after_usd_e8
        .checked_mul(unit_scale)
        .ok_or(MarketPolicyError::Overflow)?;
    let rhs = valid_global_supply_after_atoms
        .checked_mul(nav_floor_usd_e8)
        .ok_or(MarketPolicyError::Overflow)?;
    Ok(lhs >= rhs)
}

pub fn compute_discount_boundary(
    nav_floor_usd_e8: u128,
    discount_trigger_bps: u128,
) -> Result<u128, MarketPolicyError> {
    check_bps(discount_trigger_bps)?;
    mul_div_floor(nav_floor_usd_e8, BPS - discount_trigger_bps, BPS)
}

pub fn compute_premium_boundary(
    nav_floor_usd_e8: u128,
    premium_trigger_bps: u128,
) -> Result<u128, MarketPolicyError> {
    check_bps(premium_trigger_bps)?;
    mul_div_floor(nav_floor_usd_e8, BPS + premium_trigger_bps, BPS)
}

pub fn discount_frequency_time_bps(
    observations: &[VenueObservation],
    nav_floor_usd_e8: u128,
    discount_trigger_bps: u128,
    window_seconds: u128,
) -> Result<u128, MarketPolicyError> {
    Ok(compute_discount_metrics(
        observations,
        nav_floor_usd_e8,
        discount_trigger_bps,
        window_seconds,
    )?
    .frequency_time_bps)
}

pub fn discount_frequency_volume_bps(
    observations: &[VenueObservation],
    nav_floor_usd_e8: u128,
    discount_trigger_bps: u128,
    window_seconds: u128,
) -> Result<u128, MarketPolicyError> {
    Ok(compute_discount_metrics(
        observations,
        nav_floor_usd_e8,
        discount_trigger_bps,
        window_seconds,
    )?
    .frequency_volume_bps)
}

pub fn discount_severity_bps(
    observations: &[VenueObservation],
    nav_floor_usd_e8: u128,
    discount_trigger_bps: u128,
    window_seconds: u128,
) -> Result<u128, MarketPolicyError> {
    Ok(compute_discount_metrics(
        observations,
        nav_floor_usd_e8,
        discount_trigger_bps,
        window_seconds,
    )?
    .severity_bps)
}

pub fn premium_frequency_time_bps(
    observations: &[VenueObservation],
    nav_floor_usd_e8: u128,
    premium_trigger_bps: u128,
    window_seconds: u128,
) -> Result<u128, MarketPolicyError> {
    Ok(compute_premium_metrics(
        observations,
        nav_floor_usd_e8,
        premium_trigger_bps,
        window_seconds,
    )?
    .frequency_time_bps)
}

pub fn premium_frequency_volume_bps(
    observations: &[VenueObservation],
    nav_floor_usd_e8: u128,
    premium_trigger_bps: u128,
    window_seconds: u128,
) -> Result<u128, MarketPolicyError> {
    Ok(compute_premium_metrics(
        observations,
        nav_floor_usd_e8,
        premium_trigger_bps,
        window_seconds,
    )?
    .frequency_volume_bps)
}

pub fn premium_severity_bps(
    observations: &[VenueObservation],
    nav_floor_usd_e8: u128,
    premium_trigger_bps: u128,
    window_seconds: u128,
) -> Result<u128, MarketPolicyError> {
    Ok(compute_premium_metrics(
        observations,
        nav_floor_usd_e8,
        premium_trigger_bps,
        window_seconds,
    )?
    .severity_bps)
}

pub fn compute_discount_metrics(
    observations: &[VenueObservation],
    nav_floor_usd_e8: u128,
    discount_trigger_bps: u128,
    window_seconds: u128,
) -> Result<VenueMetrics, MarketPolicyError> {
    let discount_boundary = compute_discount_boundary(nav_floor_usd_e8, discount_trigger_bps)?;
    compute_venue_metrics(
        observations,
        nav_floor_usd_e8,
        discount_boundary,
        window_seconds,
        VenueSide::Discount,
    )
}

pub fn compute_premium_metrics(
    observations: &[VenueObservation],
    nav_floor_usd_e8: u128,
    premium_trigger_bps: u128,
    window_seconds: u128,
) -> Result<VenueMetrics, MarketPolicyError> {
    let premium_boundary = compute_premium_boundary(nav_floor_usd_e8, premium_trigger_bps)?;
    compute_venue_metrics(
        observations,
        nav_floor_usd_e8,
        premium_boundary,
        window_seconds,
        VenueSide::Premium,
    )
}

pub fn pct_bps(values: &[u128], q_bps: u128) -> Result<u128, MarketPolicyError> {
    check_bps(q_bps)?;
    if values.is_empty() {
        return Err(MarketPolicyError::EmptyValues);
    }
    let mut sorted_values = values.to_vec();
    sorted_values.sort_unstable();
    let one_based = if q_bps == 0 {
        1
    } else {
        mul_div_ceil(q_bps, sorted_values.len() as u128, BPS)?
    };
    let index = one_based
        .checked_sub(1)
        .ok_or(MarketPolicyError::Overflow)? as usize;
    Ok(sorted_values[index.min(sorted_values.len() - 1)])
}

pub fn compute_alignment_reserve_requirement(
    portfolio_floor_value_usd_e8: u128,
    cost_to_restore_14d_usd_e8: &[u128],
    cost_to_restore_90d_usd_e8: &[u128],
    params: AlignmentReserveParams,
    previous_required_alignment_reserve_usd_e8: u128,
) -> Result<AlignmentReserveRequirement, MarketPolicyError> {
    check_bps(params.min_alignment_bps)?;
    check_bps(params.max_decay_per_epoch_bps)?;

    let minimum_from_portfolio =
        mul_div_floor(portfolio_floor_value_usd_e8, params.min_alignment_bps, BPS)?;
    let minimum_alignment_reserve_usd_e8 = params.policy_min_usd_e8.max(minimum_from_portfolio);

    let stress_support_need_14d_usd_e8 = params
        .stress_repeat_factor_14d
        .checked_mul(pct_bps(cost_to_restore_14d_usd_e8, 9_900)?)
        .ok_or(MarketPolicyError::Overflow)?;
    let stress_support_need_90d_usd_e8 = params
        .stress_repeat_factor_90d
        .checked_mul(pct_bps(cost_to_restore_90d_usd_e8, 9_500)?)
        .ok_or(MarketPolicyError::Overflow)?;
    let latency_buffer_usd_e8 = params
        .stale_epochs_allowed
        .checked_mul(pct_bps(cost_to_restore_14d_usd_e8, 9_500)?)
        .ok_or(MarketPolicyError::Overflow)?;

    let raw_required_alignment_reserve_usd_e8 = minimum_alignment_reserve_usd_e8
        .max(stress_support_need_14d_usd_e8)
        .max(stress_support_need_90d_usd_e8)
        .max(latency_buffer_usd_e8);

    let required_alignment_reserve_next_usd_e8 =
        if raw_required_alignment_reserve_usd_e8 >= previous_required_alignment_reserve_usd_e8 {
            raw_required_alignment_reserve_usd_e8
        } else {
            let decayed_previous = mul_div_floor(
                previous_required_alignment_reserve_usd_e8,
                BPS - params.max_decay_per_epoch_bps,
                BPS,
            )?;
            raw_required_alignment_reserve_usd_e8.max(decayed_previous)
        };

    Ok(AlignmentReserveRequirement {
        minimum_alignment_reserve_usd_e8,
        stress_support_need_14d_usd_e8,
        stress_support_need_90d_usd_e8,
        latency_buffer_usd_e8,
        raw_required_alignment_reserve_usd_e8,
        required_alignment_reserve_next_usd_e8,
    })
}

pub fn compute_response_curve(
    metrics: ResponseCurveMetrics,
    weights: ResponseCurveWeights,
) -> Result<u128, MarketPolicyError> {
    check_bps(weights.max_response_bps)?;
    let time_component = weights
        .time_weight
        .checked_mul(metrics.frequency_time_bps)
        .ok_or(MarketPolicyError::Overflow)?;
    let volume_component = weights
        .volume_weight
        .checked_mul(metrics.frequency_volume_bps)
        .ok_or(MarketPolicyError::Overflow)?;
    let severity_component = weights
        .severity_weight
        .checked_mul(metrics.severity_bps)
        .ok_or(MarketPolicyError::Overflow)?;
    let weighted_sum = time_component
        .checked_add(volume_component)
        .and_then(|sum| sum.checked_add(severity_component))
        .ok_or(MarketPolicyError::Overflow)?;
    Ok((weighted_sum / WEIGHT_DENOM).min(weights.max_response_bps))
}

pub fn bootstrap_discount_response_weights() -> ResponseCurveWeights {
    ResponseCurveWeights {
        time_weight: DISCOUNT_TIME_WEIGHT,
        volume_weight: DISCOUNT_VOLUME_WEIGHT,
        severity_weight: DISCOUNT_SEVERITY_WEIGHT,
        max_response_bps: MAX_DISCOUNT_RESPONSE_BPS,
    }
}

pub fn bootstrap_premium_response_weights() -> ResponseCurveWeights {
    ResponseCurveWeights {
        time_weight: PREMIUM_TIME_WEIGHT,
        volume_weight: PREMIUM_VOLUME_WEIGHT,
        severity_weight: PREMIUM_SEVERITY_WEIGHT,
        max_response_bps: MAX_PREMIUM_RESPONSE_BPS,
    }
}

pub fn compute_discount_response_bps(
    metrics: ResponseCurveMetrics,
) -> Result<u128, MarketPolicyError> {
    compute_response_curve(metrics, bootstrap_discount_response_weights())
}

pub fn compute_premium_response_bps(
    metrics: ResponseCurveMetrics,
) -> Result<u128, MarketPolicyError> {
    compute_response_curve(metrics, bootstrap_premium_response_weights())
}

pub fn compute_reserve_deploy_cap(
    funded_alignment_reserve_usd_e8: u128,
    discount_response_bps: u128,
    limits: ReserveDeployLimits,
) -> Result<ReserveDeployCap, MarketPolicyError> {
    check_bps(discount_response_bps)?;
    let response_cap_usd_e8 =
        mul_div_floor(funded_alignment_reserve_usd_e8, discount_response_bps, BPS)?;
    let reserve_deploy_cap_usd_e8 = limits
        .available_alignment_reserve_usd_e8
        .min(limits.venue_policy_cap_usd_e8)
        .min(response_cap_usd_e8)
        .min(limits.depth_limited_cap_usd_e8)
        .min(limits.cooldown_limited_cap_usd_e8);
    Ok(ReserveDeployCap {
        response_cap_usd_e8,
        reserve_deploy_cap_usd_e8,
    })
}

pub fn compute_mint_cap(
    valid_global_supply_atoms: u128,
    premium_response_bps: u128,
    verified_capacity_remaining_atoms: u128,
    limits: MintCapLimits,
) -> Result<MintCap, MarketPolicyError> {
    check_bps(premium_response_bps)?;
    let market_response_mint_atoms =
        mul_div_floor(valid_global_supply_atoms, premium_response_bps, BPS)?;
    let mint_cap_atoms = limits
        .policy_max_mint_atoms
        .min(verified_capacity_remaining_atoms)
        .min(market_response_mint_atoms)
        .min(limits.venue_bid_depth_atoms)
        .min(limits.cooldown_mint_atoms);
    Ok(MintCap {
        market_response_mint_atoms,
        mint_cap_atoms,
    })
}

fn checkpoint_covers_pool_state(
    bundle: &MarketOpsEvmEvidenceBundle,
    pool_state: &MarketOpsEvmPoolStateEvidence,
) -> bool {
    bundle.hook_checkpoints.iter().any(|checkpoint| {
        checkpoint.checkpoint_count == pool_state.checkpoint_count
            && checkpoint.block_number >= pool_state.block_number
            && checkpoint.swap_count >= pool_state.observation_sequence
    })
}

fn exact_constant_product_pool_state(
    pool_state: &MarketOpsEvmPoolStateEvidence,
    unit_scale: u128,
) -> Result<ConstantProductPoolState, MarketPolicyError> {
    if !pool_state.replayable {
        return Err(MarketPolicyError::IneligibleEvidence);
    }
    check_bps(u128::from(pool_state.fee_bps))?;
    if pool_state.base_reserve_atoms == 0 || pool_state.quote_reserve_usd_e8 == 0 || unit_scale == 0
    {
        return Err(MarketPolicyError::DivisionByZero);
    }
    Ok(ConstantProductPoolState {
        base_reserve_atoms: pool_state.base_reserve_atoms,
        quote_reserve_usd_e8: pool_state.quote_reserve_usd_e8,
        unit_scale,
        fee_bps: u128::from(pool_state.fee_bps),
    })
}

fn integer_sqrt_ceil(value: u128) -> Result<u128, MarketPolicyError> {
    let floor = integer_sqrt_floor(value);
    if floor
        .checked_mul(floor)
        .ok_or(MarketPolicyError::Overflow)?
        == value
    {
        Ok(floor)
    } else {
        floor.checked_add(1).ok_or(MarketPolicyError::Overflow)
    }
}

fn integer_sqrt_floor(value: u128) -> u128 {
    if value < 2 {
        return value;
    }
    let mut low = 1u128;
    let mut high = 1u128 << 64;
    while low < high {
        let mid = low + ((high - low + 1) / 2);
        if mid <= value / mid {
            low = mid;
        } else {
            high = mid - 1;
        }
    }
    low
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VenueSide {
    Discount,
    Premium,
}

fn compute_venue_metrics(
    observations: &[VenueObservation],
    nav_floor_usd_e8: u128,
    boundary_usd_e8: u128,
    window_seconds: u128,
    side: VenueSide,
) -> Result<VenueMetrics, MarketPolicyError> {
    if nav_floor_usd_e8 == 0 || window_seconds == 0 {
        return Err(MarketPolicyError::DivisionByZero);
    }

    let mut total_time_seconds = 0u128;
    let mut breach_time_seconds = 0u128;
    let mut total_volume_usd_e8 = 0u128;
    let mut breach_volume_usd_e8 = 0u128;
    let mut severity_time_weighted_bps = 0u128;

    for observation in observations {
        total_time_seconds = total_time_seconds
            .checked_add(observation.dt_seconds)
            .ok_or(MarketPolicyError::Overflow)?;
        total_volume_usd_e8 = total_volume_usd_e8
            .checked_add(observation.volume_usd_e8)
            .ok_or(MarketPolicyError::Overflow)?;

        let excess_usd_e8 = match side {
            VenueSide::Discount if observation.price_usd_e8 < boundary_usd_e8 => {
                boundary_usd_e8 - observation.price_usd_e8
            }
            VenueSide::Premium if observation.price_usd_e8 > boundary_usd_e8 => {
                observation.price_usd_e8 - boundary_usd_e8
            }
            _ => continue,
        };

        breach_time_seconds = breach_time_seconds
            .checked_add(observation.dt_seconds)
            .ok_or(MarketPolicyError::Overflow)?;
        breach_volume_usd_e8 = breach_volume_usd_e8
            .checked_add(observation.volume_usd_e8)
            .ok_or(MarketPolicyError::Overflow)?;
        let excess_bps = mul_div_floor(excess_usd_e8, BPS, nav_floor_usd_e8)?;
        let weighted_severity = observation
            .dt_seconds
            .checked_mul(excess_bps)
            .ok_or(MarketPolicyError::Overflow)?;
        severity_time_weighted_bps = severity_time_weighted_bps
            .checked_add(weighted_severity)
            .ok_or(MarketPolicyError::Overflow)?;
    }

    if total_time_seconds > window_seconds {
        return Err(MarketPolicyError::InvalidWindow);
    }

    let frequency_time_bps = mul_div_floor(breach_time_seconds, BPS, window_seconds)?;
    let frequency_volume_bps =
        mul_div_floor(breach_volume_usd_e8, BPS, total_volume_usd_e8.max(1))?;
    let severity_bps = if breach_time_seconds == 0 {
        0
    } else {
        severity_time_weighted_bps / breach_time_seconds
    };

    Ok(VenueMetrics {
        breach_time_seconds,
        frequency_time_bps,
        frequency_volume_bps,
        severity_bps,
    })
}

fn check_bps(value: u128) -> Result<(), MarketPolicyError> {
    if value > BPS {
        return Err(MarketPolicyError::InvalidBps);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use postfiat_types::{
        market_ops_evm_evidence_root, MarketOpsEvmEvidenceBundle, MarketOpsEvmHeaderEvidence,
        MarketOpsEvmLogEvidence, MarketOpsEvmPoolStateEvidence, MarketOpsEvmReceiptEvidence,
        MarketOpsHookCheckpointEvidence,
    };

    use super::*;

    fn usd(amount: u128) -> u128 {
        amount * USD_SCALE
    }

    fn units(amount: u128) -> u128 {
        amount * DEFAULT_UNIT_SCALE
    }

    #[test]
    fn nominal_example_reproduces_bootstrap_caps() {
        let valid_global_supply_atoms = units(1_000_000);
        let nav_floor =
            compute_nav_floor(usd(5_000_000), valid_global_supply_atoms, BPS).expect("nav floor");
        assert_eq!(usd(5), nav_floor.nav_floor_usd_e8);

        let capacity = compute_backing_capacity(
            usd(5_200_000),
            valid_global_supply_atoms,
            nav_floor.nav_floor_usd_e8,
        )
        .expect("backing capacity");
        assert_eq!(usd(5_000_000), capacity.backing_required_usd_e8);
        assert_eq!(usd(200_000), capacity.verified_capacity_remaining_usd_e8);
        assert_eq!(units(40_000), capacity.verified_capacity_remaining_atoms);

        let alignment = compute_alignment_reserve_requirement(
            usd(5_000_000),
            &[usd(20_000), usd(45_000), usd(45_000)],
            &[usd(30_000), usd(45_000), usd(60_000)],
            AlignmentReserveParams {
                policy_min_usd_e8: usd(25_000),
                min_alignment_bps: 100,
                stress_repeat_factor_14d: 3,
                stress_repeat_factor_90d: 2,
                stale_epochs_allowed: 1,
                max_decay_per_epoch_bps: 1_000,
            },
            0,
        )
        .expect("alignment reserve");
        assert_eq!(usd(50_000), alignment.minimum_alignment_reserve_usd_e8);
        assert_eq!(usd(135_000), alignment.stress_support_need_14d_usd_e8);
        assert_eq!(usd(120_000), alignment.stress_support_need_90d_usd_e8);
        assert_eq!(usd(45_000), alignment.latency_buffer_usd_e8);
        assert_eq!(
            usd(135_000),
            alignment.required_alignment_reserve_next_usd_e8
        );

        let discount_response = compute_discount_response_bps(ResponseCurveMetrics {
            frequency_time_bps: 4_200,
            frequency_volume_bps: 2_500,
            severity_bps: 200,
        })
        .expect("discount response");
        assert_eq!(1_725, discount_response);

        let reserve_cap = compute_reserve_deploy_cap(
            usd(150_000),
            discount_response,
            ReserveDeployLimits {
                available_alignment_reserve_usd_e8: usd(150_000),
                venue_policy_cap_usd_e8: usd(50_000),
                depth_limited_cap_usd_e8: usd(30_000),
                cooldown_limited_cap_usd_e8: usd(40_000),
            },
        )
        .expect("reserve cap");
        assert_eq!(usd(25_875), reserve_cap.response_cap_usd_e8);
        assert_eq!(usd(25_875), reserve_cap.reserve_deploy_cap_usd_e8);

        let premium_response = compute_premium_response_bps(ResponseCurveMetrics {
            frequency_time_bps: 1_800,
            frequency_volume_bps: 2_200,
            severity_bps: 250,
        })
        .expect("premium response");
        assert_eq!(83, premium_response);

        let mint_cap = compute_mint_cap(
            valid_global_supply_atoms,
            premium_response,
            capacity.verified_capacity_remaining_atoms,
            MintCapLimits {
                policy_max_mint_atoms: units(50_000),
                venue_bid_depth_atoms: units(12_000),
                cooldown_mint_atoms: units(10_000),
            },
        )
        .expect("mint cap");
        assert_eq!(units(8_300), mint_cap.market_response_mint_atoms);
        assert_eq!(units(8_300), mint_cap.mint_cap_atoms);
    }

    #[test]
    fn venue_observations_reproduce_bootstrap_responses() {
        let nav_floor_usd_e8 = usd(5);
        let window_seconds = 10_000;
        let discount_observations = [
            VenueObservation {
                dt_seconds: 4_200,
                price_usd_e8: 475_000_000,
                volume_usd_e8: usd(2_500),
            },
            VenueObservation {
                dt_seconds: 5_800,
                price_usd_e8: usd(5),
                volume_usd_e8: usd(7_500),
            },
        ];
        let discount_metrics = compute_discount_metrics(
            &discount_observations,
            nav_floor_usd_e8,
            300,
            window_seconds,
        )
        .expect("discount metrics");
        assert_eq!(4_200, discount_metrics.breach_time_seconds);
        assert_eq!(4_200, discount_metrics.frequency_time_bps);
        assert_eq!(2_500, discount_metrics.frequency_volume_bps);
        assert_eq!(200, discount_metrics.severity_bps);
        assert_eq!(
            discount_metrics.frequency_time_bps,
            discount_frequency_time_bps(
                &discount_observations,
                nav_floor_usd_e8,
                300,
                window_seconds
            )
            .expect("discount time frequency")
        );
        assert_eq!(
            discount_metrics.frequency_volume_bps,
            discount_frequency_volume_bps(
                &discount_observations,
                nav_floor_usd_e8,
                300,
                window_seconds
            )
            .expect("discount volume frequency")
        );
        assert_eq!(
            discount_metrics.severity_bps,
            discount_severity_bps(
                &discount_observations,
                nav_floor_usd_e8,
                300,
                window_seconds
            )
            .expect("discount severity")
        );
        assert_eq!(
            1_725,
            compute_discount_response_bps(discount_metrics.response_curve_metrics())
                .expect("discount response")
        );

        let premium_observations = [
            VenueObservation {
                dt_seconds: 1_800,
                price_usd_e8: 562_500_000,
                volume_usd_e8: usd(2_200),
            },
            VenueObservation {
                dt_seconds: 8_200,
                price_usd_e8: usd(5),
                volume_usd_e8: usd(7_800),
            },
        ];
        let premium_metrics = compute_premium_metrics(
            &premium_observations,
            nav_floor_usd_e8,
            1_000,
            window_seconds,
        )
        .expect("premium metrics");
        assert_eq!(1_800, premium_metrics.breach_time_seconds);
        assert_eq!(1_800, premium_metrics.frequency_time_bps);
        assert_eq!(2_200, premium_metrics.frequency_volume_bps);
        assert_eq!(250, premium_metrics.severity_bps);
        assert_eq!(
            premium_metrics.frequency_time_bps,
            premium_frequency_time_bps(
                &premium_observations,
                nav_floor_usd_e8,
                1_000,
                window_seconds
            )
            .expect("premium time frequency")
        );
        assert_eq!(
            premium_metrics.frequency_volume_bps,
            premium_frequency_volume_bps(
                &premium_observations,
                nav_floor_usd_e8,
                1_000,
                window_seconds
            )
            .expect("premium volume frequency")
        );
        assert_eq!(
            premium_metrics.severity_bps,
            premium_severity_bps(
                &premium_observations,
                nav_floor_usd_e8,
                1_000,
                window_seconds
            )
            .expect("premium severity")
        );
        assert_eq!(
            83,
            compute_premium_response_bps(premium_metrics.response_curve_metrics())
                .expect("premium response")
        );
    }

    #[test]
    fn evm_venue_evidence_bundle_replays_expected_market_metrics() {
        let bundle = evm_evidence_bundle();
        let evidence_root = market_ops_evm_evidence_root(&bundle).expect("evidence root");

        let report = replay_evm_venue_evidence(&bundle, evidence_root, evm_replay_config())
            .expect("replay evm evidence");

        assert_eq!(evidence_root, report.evidence_root);
        assert_eq!(3, report.eligible_observations.len());
        assert_eq!(0, report.ineligible_observation_count);
        assert_eq!(4_200, report.discount_metrics.breach_time_seconds);
        assert_eq!(4_200, report.discount_metrics.frequency_time_bps);
        assert_eq!(2_500, report.discount_metrics.frequency_volume_bps);
        assert_eq!(200, report.discount_metrics.severity_bps);
        assert_eq!(1_725, report.discount_response_bps);
        assert_eq!(1_800, report.premium_metrics.breach_time_seconds);
        assert_eq!(1_800, report.premium_metrics.frequency_time_bps);
        assert_eq!(2_200, report.premium_metrics.frequency_volume_bps);
        assert_eq!(250, report.premium_metrics.severity_bps);
        assert_eq!(83, report.premium_response_bps);
        assert_eq!(0, report.latest_depth_limited_cap_usd_e8);
    }

    #[test]
    fn evm_venue_replay_excludes_unreplayable_observation_from_caps() {
        let mut bundle = evm_evidence_bundle();
        bundle.pool_states[1].replayable = false;
        let evidence_root = market_ops_evm_evidence_root(&bundle).expect("evidence root");

        let report = replay_evm_venue_evidence(&bundle, evidence_root, evm_replay_config())
            .expect("replay evm evidence with one manual-only observation");

        assert_eq!(2, report.eligible_observations.len());
        assert_eq!(1, report.ineligible_observation_count);
        assert_eq!(0, report.premium_metrics.breach_time_seconds);
        assert_eq!(0, report.premium_response_bps);
    }

    #[test]
    fn evm_venue_replay_rejects_wrong_evidence_root() {
        let bundle = evm_evidence_bundle();

        assert_eq!(
            replay_evm_venue_evidence(&bundle, [9u8; 32], evm_replay_config()),
            Err(MarketPolicyError::EvidenceRootMismatch)
        );
    }

    #[test]
    fn adversarial_deterministic_replay_same_inputs_same_metrics() {
        let bundle = evm_evidence_bundle();
        let evidence_root = market_ops_evm_evidence_root(&bundle).expect("evidence root");

        let first =
            replay_evm_venue_evidence(&bundle, evidence_root, evm_replay_config()).expect("first");
        let second =
            replay_evm_venue_evidence(&bundle, evidence_root, evm_replay_config()).expect("second");

        assert_eq!(first.evidence_root, second.evidence_root);
        assert_eq!(first.eligible_observations, second.eligible_observations);
        assert_eq!(first.discount_metrics, second.discount_metrics);
        assert_eq!(first.premium_metrics, second.premium_metrics);
        assert_eq!(first.discount_response_bps, second.discount_response_bps);
        assert_eq!(first.premium_response_bps, second.premium_response_bps);
    }

    #[test]
    fn adversarial_oracle_manipulation_thin_move_does_not_unlock_material_caps() {
        let mut bundle = evm_evidence_bundle();
        bundle.pool_states = vec![
            MarketOpsEvmPoolStateEvidence {
                block_number: 1,
                observation_sequence: 1,
                timestamp: 1_000,
                dt_seconds: 1,
                checkpoint_count: 1,
                price_usd_e8: 484_000_000,
                volume_usd_e8: 1,
                zero_for_one: true,
                fee_bps: 30,
                liquidity: 100,
                base_reserve_atoms: 100,
                quote_reserve_usd_e8: 484_000_000,
                replayable: true,
            },
            MarketOpsEvmPoolStateEvidence {
                block_number: 1,
                observation_sequence: 2,
                timestamp: 1_001,
                dt_seconds: 9_999,
                checkpoint_count: 1,
                price_usd_e8: usd(5),
                volume_usd_e8: usd(10_000),
                zero_for_one: false,
                fee_bps: 30,
                liquidity: 100,
                base_reserve_atoms: 100,
                quote_reserve_usd_e8: usd(5),
                replayable: true,
            },
        ];
        bundle.hook_checkpoints[0].swap_count = 2;
        let evidence_root = market_ops_evm_evidence_root(&bundle).expect("evidence root");

        let report = replay_evm_venue_evidence(&bundle, evidence_root, evm_replay_config())
            .expect("thin replay");

        assert!(report.discount_response_bps < 100);
        assert_eq!(0, report.premium_response_bps);
    }

    #[test]
    fn adversarial_sustained_discount_increases_cap_only_within_policy_bounds() {
        let observations = [
            VenueObservation {
                dt_seconds: 500_000,
                price_usd_e8: 450_000_000,
                volume_usd_e8: usd(5_000_000),
            },
            VenueObservation {
                dt_seconds: 709_600,
                price_usd_e8: 450_000_000,
                volume_usd_e8: usd(7_000_000),
            },
        ];
        let metrics =
            compute_discount_metrics(&observations, usd(5), 300, 1_209_600).expect("metrics");
        let response =
            compute_discount_response_bps(metrics.response_curve_metrics()).expect("response");

        assert_eq!(MAX_DISCOUNT_RESPONSE_BPS, response);
        let reserve_cap = compute_reserve_deploy_cap(
            usd(150_000),
            response,
            ReserveDeployLimits {
                available_alignment_reserve_usd_e8: usd(20_000),
                venue_policy_cap_usd_e8: usd(25_000),
                depth_limited_cap_usd_e8: usd(30_000),
                cooldown_limited_cap_usd_e8: usd(40_000),
            },
        )
        .expect("reserve cap");
        assert_eq!(usd(37_500), reserve_cap.response_cap_usd_e8);
        assert_eq!(usd(20_000), reserve_cap.reserve_deploy_cap_usd_e8);
    }

    #[test]
    fn adversarial_operator_non_funding_drives_reserve_cap_to_zero() {
        let reserve_cap = compute_reserve_deploy_cap(
            usd(150_000),
            1_725,
            ReserveDeployLimits {
                available_alignment_reserve_usd_e8: 0,
                venue_policy_cap_usd_e8: usd(50_000),
                depth_limited_cap_usd_e8: usd(30_000),
                cooldown_limited_cap_usd_e8: usd(40_000),
            },
        )
        .expect("reserve cap");

        assert_eq!(0, reserve_cap.reserve_deploy_cap_usd_e8);
    }

    #[test]
    fn arithmetic_errors_fail_closed() {
        assert_eq!(
            mul_div_floor(1, 1, 0),
            Err(MarketPolicyError::DivisionByZero)
        );
        assert_eq!(
            mul_div_floor(u128::MAX, 2, 1),
            Err(MarketPolicyError::Overflow)
        );
        assert_eq!(pct_bps(&[], 9_500), Err(MarketPolicyError::EmptyValues));
        assert_eq!(
            compute_discount_boundary(usd(5), BPS + 1),
            Err(MarketPolicyError::InvalidBps)
        );
        assert_eq!(
            compute_discount_metrics(
                &[VenueObservation {
                    dt_seconds: 2,
                    price_usd_e8: usd(4),
                    volume_usd_e8: 1,
                }],
                usd(5),
                300,
                1,
            ),
            Err(MarketPolicyError::InvalidWindow)
        );
    }

    fn evm_replay_config() -> VenueEvidenceReplayConfig {
        VenueEvidenceReplayConfig {
            expected_venue_id: fixed32(0x37),
            expected_pool_config_hash: fixed32(0x38),
            expected_hook_code_hash: fixed32(0x39),
            unit_scale: 100,
            nav_floor_usd_e8: usd(5),
            discount_trigger_bps: 300,
            premium_trigger_bps: 1_000,
            slippage_limit_bps: 500,
            data_window_start: 1_000,
            data_window_end: 11_000,
        }
    }

    fn evm_evidence_bundle() -> MarketOpsEvmEvidenceBundle {
        MarketOpsEvmEvidenceBundle {
            encoding_version: 1,
            chain_id: 1,
            venue_id: fixed32(0x37),
            pool_id: fixed32(0xab),
            pool_manager: fixed20(0x11),
            hook_address: fixed20(0x22),
            pool_config_hash: fixed32(0x38),
            hook_code_hash: fixed32(0x39),
            headers: vec![MarketOpsEvmHeaderEvidence {
                block_number: 1,
                block_hash: fixed32(0x01),
                parent_hash: fixed32(0x02),
                state_root: fixed32(0x03),
                receipts_root: fixed32(0x04),
                timestamp: 1_000,
            }],
            receipts: vec![MarketOpsEvmReceiptEvidence {
                block_number: 1,
                transaction_index: 0,
                receipt_hash: fixed32(0x05),
                status: true,
                logs_root: fixed32(0x06),
            }],
            logs: vec![MarketOpsEvmLogEvidence {
                block_number: 1,
                transaction_index: 0,
                log_index: 0,
                address: fixed20(0x22),
                topics: vec![fixed32(0x07), fixed32(0xab)],
                data_hash: fixed32(0x08),
            }],
            hook_checkpoints: vec![MarketOpsHookCheckpointEvidence {
                block_number: 1,
                log_index: 0,
                pool_id: fixed32(0xab),
                checkpoint_count: 1,
                swap_count: 3,
                depth_count: 1,
                swap_root: fixed32(0x09),
                depth_root: fixed32(0x0a),
                pftl_state_hash: fixed32(0x0b),
            }],
            pool_states: vec![
                MarketOpsEvmPoolStateEvidence {
                    block_number: 1,
                    observation_sequence: 1,
                    timestamp: 1_000,
                    dt_seconds: 4_200,
                    checkpoint_count: 1,
                    price_usd_e8: 475_000_000,
                    volume_usd_e8: usd(2_500),
                    zero_for_one: true,
                    fee_bps: 30,
                    liquidity: 100,
                    base_reserve_atoms: 100,
                    quote_reserve_usd_e8: 475_000_000,
                    replayable: true,
                },
                MarketOpsEvmPoolStateEvidence {
                    block_number: 1,
                    observation_sequence: 2,
                    timestamp: 5_200,
                    dt_seconds: 1_800,
                    checkpoint_count: 1,
                    price_usd_e8: 562_500_000,
                    volume_usd_e8: usd(2_200),
                    zero_for_one: false,
                    fee_bps: 30,
                    liquidity: 100,
                    base_reserve_atoms: 100,
                    quote_reserve_usd_e8: 562_500_000,
                    replayable: true,
                },
                MarketOpsEvmPoolStateEvidence {
                    block_number: 1,
                    observation_sequence: 3,
                    timestamp: 7_000,
                    dt_seconds: 4_000,
                    checkpoint_count: 1,
                    price_usd_e8: usd(5),
                    volume_usd_e8: usd(5_300),
                    zero_for_one: true,
                    fee_bps: 30,
                    liquidity: 100,
                    base_reserve_atoms: 100,
                    quote_reserve_usd_e8: usd(5),
                    replayable: true,
                },
            ],
        }
    }

    fn fixed32(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    fn fixed20(byte: u8) -> [u8; 20] {
        [byte; 20]
    }
}
