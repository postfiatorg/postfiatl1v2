/// Minimal, zero-dependency, exact 256-bit unsigned integer (little-endian 64-bit limbs)
/// for deterministic Pillar 7 NAV & fee arithmetic without silent u128 overflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NavU256(pub [u64; 4]);

impl PartialOrd for NavU256 {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for NavU256 {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.cmp_val(*other)
    }
}

impl NavU256 {
    pub const ZERO: Self = Self([0, 0, 0, 0]);
    pub const ONE: Self = Self([1, 0, 0, 0]);

    pub const fn from_u64(v: u64) -> Self {
        Self([v, 0, 0, 0])
    }

    pub const fn from_u128(v: u128) -> Self {
        Self([v as u64, (v >> 64) as u64, 0, 0])
    }

    pub const fn is_zero(self) -> bool {
        self.0[0] == 0 && self.0[1] == 0 && self.0[2] == 0 && self.0[3] == 0
    }

    pub fn pow10(exp: u8) -> Option<Self> {
        let mut acc = Self::ONE;
        let ten = Self::from_u64(10);
        for _ in 0..exp {
            acc = acc.checked_mul(ten)?;
        }
        Some(acc)
    }

    pub fn checked_add(self, rhs: Self) -> Option<Self> {
        let mut out = [0u64; 4];
        let mut carry = 0u128;
        for (i, slot) in out.iter_mut().enumerate() {
            let sum = u128::from(self.0[i]) + u128::from(rhs.0[i]) + carry;
            *slot = sum as u64;
            carry = sum >> 64;
        }
        if carry != 0 {
            None
        } else {
            Some(Self(out))
        }
    }

    pub fn checked_sub(self, rhs: Self) -> Option<Self> {
        let mut out = [0u64; 4];
        let mut borrow = 0u128;
        for (i, slot) in out.iter_mut().enumerate() {
            let a = u128::from(self.0[i]);
            let b = u128::from(rhs.0[i]) + borrow;
            if a >= b {
                *slot = (a - b) as u64;
                borrow = 0;
            } else {
                *slot = ((1u128 << 64) + a - b) as u64;
                borrow = 1;
            }
        }
        if borrow != 0 {
            None
        } else {
            Some(Self(out))
        }
    }

    pub fn checked_mul(self, rhs: Self) -> Option<Self> {
        let mut wide = [0u64; 8];
        for i in 0..4 {
            let mut carry = 0u128;
            for j in 0..4 {
                let prod = u128::from(self.0[i]) * u128::from(rhs.0[j])
                    + u128::from(wide[i + j])
                    + carry;
                wide[i + j] = prod as u64;
                carry = prod >> 64;
            }
            wide[i + 4] = carry as u64;
        }
        if wide[4] != 0 || wide[5] != 0 || wide[6] != 0 || wide[7] != 0 {
            None
        } else {
            Some(Self([wide[0], wide[1], wide[2], wide[3]]))
        }
    }

    fn cmp_val(self, rhs: Self) -> std::cmp::Ordering {
        for i in (0..4).rev() {
            match self.0[i].cmp(&rhs.0[i]) {
                std::cmp::Ordering::Equal => continue,
                ord => return ord,
            }
        }
        std::cmp::Ordering::Equal
    }

    fn bit(self, idx: usize) -> bool {
        ((self.0[idx / 64] >> (idx % 64)) & 1) != 0
    }

    fn set_bit(&mut self, idx: usize) {
        self.0[idx / 64] |= 1u64 << (idx % 64);
    }

    fn shl1(self) -> Self {
        Self([
            self.0[0] << 1,
            (self.0[1] << 1) | (self.0[0] >> 63),
            (self.0[2] << 1) | (self.0[1] >> 63),
            (self.0[3] << 1) | (self.0[2] >> 63),
        ])
    }

    fn bits(self) -> usize {
        for i in (0..4).rev() {
            if self.0[i] != 0 {
                return i * 64 + (64 - self.0[i].leading_zeros() as usize);
            }
        }
        0
    }

    pub fn div_rem(self, divisor: Self) -> Option<(Self, Self)> {
        if divisor.is_zero() {
            return None;
        }
        if self.cmp_val(divisor) == std::cmp::Ordering::Less {
            return Some((Self::ZERO, self));
        }
        let mut quotient = Self::ZERO;
        let mut remainder = Self::ZERO;
        let top = self.bits();
        for bit_idx in (0..top).rev() {
            remainder = remainder.shl1();
            if self.bit(bit_idx) {
                remainder.0[0] |= 1;
            }
            if remainder.cmp_val(divisor) != std::cmp::Ordering::Less {
                remainder = remainder.checked_sub(divisor)?;
                quotient.set_bit(bit_idx);
            }
        }
        Some((quotient, remainder))
    }

    pub fn to_u64_checked(self) -> Option<u64> {
        if self.0[1] == 0 && self.0[2] == 0 && self.0[3] == 0 {
            Some(self.0[0])
        } else {
            None
        }
    }

    pub fn to_u128_checked(self) -> Option<u128> {
        if self.0[2] == 0 && self.0[3] == 0 {
            Some(u128::from(self.0[0]) | (u128::from(self.0[1]) << 64))
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavFeeScheduleV1 {
    /// Annual time-based rate in bps (0..=500).
    pub annual_rate_bps: u32,
    /// High-water-mark gain rate in bps (0..=5_000).
    pub hwm_rate_bps: u32,
    /// Primary issue spread cap in bps (0..=500).
    pub issue_spread_bps: u32,
    /// Primary redeem spread cap in bps (0..=500).
    pub redeem_spread_bps: u32,
    /// FX fix fee cap in bps (0..=100).
    pub fix_fee_cap_bps: u32,
    /// Canonical `pf...` account receiving claimed fee units; MUST NOT equal `nav_asset.issuer`.
    pub recipient: String,
    /// Notice period in blocks for any fee increase (>= 604_800).
    pub increase_notice_blocks: u64,
    /// Max blocks accrued per finalization interval (> 0, <= profile.max_epoch_gap_blocks when set).
    pub accrual_cap_blocks: u64,
    /// One-way lock (`false -> true`): permanently freezes all rate ceilings & `accrual_cap_blocks`
    /// while still permitting visible `recipient` rotation.
    #[serde(default)]
    pub rates_locked: bool,
    /// Optional IPFS CID or Pillar 2 message pointer (<= 128 bytes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disclosure_ref: Option<String>,
    pub attached_at_height: u64,
}

impl NavFeeScheduleV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.annual_rate_bps > NAV_FEE_MAX_ANNUAL_RATE_BPS
            || self.hwm_rate_bps > NAV_FEE_MAX_HWM_RATE_BPS
            || self.issue_spread_bps > NAV_FEE_MAX_SPREAD_BPS
            || self.redeem_spread_bps > NAV_FEE_MAX_SPREAD_BPS
            || self.fix_fee_cap_bps > NAV_FEE_MAX_FIX_FEE_CAP_BPS
        {
            return Err("nav fee schedule rate or spread exceeds protocol cap".to_string());
        }
        validate_text_field("nav_fee_schedule.recipient", &self.recipient)?;
        if self.increase_notice_blocks < NAV_FEE_MIN_INCREASE_NOTICE_BLOCKS {
            return Err(format!(
                "nav fee schedule increase_notice_blocks must be >= {NAV_FEE_MIN_INCREASE_NOTICE_BLOCKS}"
            ));
        }
        if self.accrual_cap_blocks == 0 {
            return Err("nav fee schedule accrual_cap_blocks must be nonzero".to_string());
        }
        if let Some(disclosure_ref) = &self.disclosure_ref {
            if disclosure_ref.len() > NAV_FEE_MAX_DISCLOSURE_REF_BYTES {
                return Err(format!(
                    "nav fee schedule disclosure_ref exceeds {NAV_FEE_MAX_DISCLOSURE_REF_BYTES} bytes"
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingNavFeeScheduleV1 {
    pub schedule: NavFeeScheduleV1,
    pub proposed_at_height: u64,
    /// First block height at which this pending schedule activates on finalization.
    pub effective_at_height: u64,
    pub is_increase: bool,
}

impl PendingNavFeeScheduleV1 {
    pub fn validate(&self) -> Result<(), String> {
        self.schedule.validate()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavFeeStateV1 {
    pub active: NavFeeScheduleV1,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending: Option<PendingNavFeeScheduleV1>,
    /// High-water mark in valuation atoms per unit (initialized to `N_current` on first S > 0 finalization).
    pub high_water_mark_nav_per_unit: u64,
    /// Block height of last accrual (initialized to `h_attach`, never 0).
    pub last_accrual_height: u64,
    /// Unclaimed accrued fee units in protocol bucket (included in `circulating_supply`).
    pub unclaimed_fee_units: u64,
    pub total_accrued_fee_units: u64,
    pub total_claimed_fee_units: u64,
}

impl NavFeeStateV1 {
    pub fn validate(&self) -> Result<(), String> {
        self.active.validate()?;
        if let Some(pending) = &self.pending {
            pending.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavFeeAccrualV1 {
    pub asset_id: String,
    pub epoch: u64,
    pub block_height: u64,
    /// Exact protocol constant `PROTOCOL_BLOCKS_PER_YEAR` (31_536_000) used for recomputation.
    pub blocks_per_year: u64,
    pub delta_blocks: u64,
    pub verified_net_assets: u64,
    pub supply_before: u64,
    pub nav_before: u64,
    pub hwm_before: u64,
    pub annual_rate_bps_applied: u32,
    pub hwm_rate_bps_applied: u32,
    pub time_fee_value: u64,
    pub nav_after_time_fee: u64,
    pub hwm_fee_value: u64,
    pub total_fee_value: u64,
    pub minted_fee_units: u64,
    pub clipped_fee_units: u64,
    pub supply_after: u64,
    pub nav_after: u64,
    pub hwm_after: u64,
    /// "accrued" | "zero_supply" | "zero_net_assets" | "zero_rates" |
    /// "zero_delta_blocks" | "clamped_max_supply" | "clamped_full_value"
    pub reason_code: String,
}

impl NavFeeAccrualV1 {
    pub fn validate(&self) -> Result<(), String> {
        validate_lower_hex_len(
            "nav_fee_accrual.asset_id",
            &self.asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        if self.blocks_per_year == 0 {
            return Err("nav_fee_accrual.blocks_per_year must be nonzero".to_string());
        }
        if !matches!(
            self.reason_code.as_str(),
            "accrued"
                | "zero_supply"
                | "zero_net_assets"
                | "zero_rates"
                | "zero_delta_blocks"
                | "clamped_max_supply"
                | "clamped_full_value"
        ) {
            return Err(format!(
                "invalid nav_fee_accrual.reason_code `{}`",
                self.reason_code
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavFeeScheduleInputV1 {
    pub annual_rate_bps: u32,
    pub hwm_rate_bps: u32,
    pub issue_spread_bps: u32,
    pub redeem_spread_bps: u32,
    pub fix_fee_cap_bps: u32,
    pub recipient: String,
    pub increase_notice_blocks: u64,
    pub accrual_cap_blocks: u64,
    #[serde(default)]
    pub rates_locked: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disclosure_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavFeeScheduleAttachOperation {
    pub issuer: String,
    pub asset_id: String,
    pub schedule: NavFeeScheduleInputV1,
}

impl NavFeeScheduleAttachOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("nav_fee_schedule_attach.issuer", &self.issuer)?;
        validate_lower_hex_len(
            "nav_fee_schedule_attach.asset_id",
            &self.asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        validate_text_field("nav_fee_schedule_attach.recipient", &self.schedule.recipient)?;
        Ok(())
    }

    pub fn signing_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavFeeScheduleSetOperation {
    pub issuer: String,
    pub asset_id: String,
    pub schedule: NavFeeScheduleInputV1,
}

impl NavFeeScheduleSetOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("nav_fee_schedule_set.issuer", &self.issuer)?;
        validate_lower_hex_len(
            "nav_fee_schedule_set.asset_id",
            &self.asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        validate_text_field("nav_fee_schedule_set.recipient", &self.schedule.recipient)?;
        Ok(())
    }

    pub fn signing_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavFeeScheduleCancelOperation {
    pub issuer: String,
    pub asset_id: String,
}

impl NavFeeScheduleCancelOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("nav_fee_schedule_cancel.issuer", &self.issuer)?;
        validate_lower_hex_len(
            "nav_fee_schedule_cancel.asset_id",
            &self.asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        Ok(())
    }

    pub fn signing_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavFeeClaimOperation {
    pub recipient: String,
    pub asset_id: String,
    pub amount: u64,
}

impl NavFeeClaimOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("nav_fee_claim.recipient", &self.recipient)?;
        validate_lower_hex_len(
            "nav_fee_claim.asset_id",
            &self.asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        if self.amount == 0 {
            return Err("nav_fee_claim.amount must be nonzero".to_string());
        }
        Ok(())
    }

    pub fn signing_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }
}

pub fn nav_fee_accrual_key(asset_id: &str, epoch: u64) -> String {
    format!("{asset_id}:{epoch:020}")
}

/// Canonical PFTL account normalization helper for Pillar 7 fee schedule validation.
fn nav_fee_normalize_canonical_account(
    address: &str,
    field: &str,
) -> Result<String, (&'static str, String)> {
    postfiat_crypto_provider::validate_canonical_address(address)
        .map_err(|err| {
            (
                "nav_fee_invalid_recipient",
                format!("{field} `{address}` is not a valid canonical PFTL address: {err}"),
            )
        })
        .map(|_| address.to_ascii_lowercase())
}

/// Exact universal `floor(V * 10^precision / S)` calculation using widened 256-bit arithmetic.
/// Returns `(expected_nav_per_unit, floor_remainder_scaled)`.
pub fn nav_fee_compute_nav_per_unit_and_remainder(
    verified_net_assets: u64,
    circulating_supply: u64,
    precision: u8,
) -> Result<(u64, u128), (&'static str, String)> {
    if precision > MAX_ISSUED_ASSET_PRECISION {
        return Err((
            "bad_nav_asset_precision",
            format!("precision {precision} exceeds {MAX_ISSUED_ASSET_PRECISION}"),
        ));
    }
    if circulating_supply == 0 {
        return Ok((0, 0));
    }
    let scale = NavU256::pow10(precision).ok_or_else(|| {
        (
            "bad_nav_asset_precision",
            "precision scale overflow".to_string(),
        )
    })?;
    let numerator = NavU256::from_u64(verified_net_assets)
        .checked_mul(scale)
        .ok_or_else(|| ("nav_math_overflow", "V * U overflowed u256".to_string()))?;
    let (quotient, remainder) = numerator
        .div_rem(NavU256::from_u64(circulating_supply))
        .ok_or_else(|| ("nav_math_overflow", "division by zero".to_string()))?;
    let expected = quotient.to_u64_checked().ok_or_else(|| {
        (
            "nav_per_unit_formula_mismatch",
            "floor(V * U / S) exceeds u64::MAX".to_string(),
        )
    })?;
    let rem_u128 = remainder.to_u128_checked().ok_or_else(|| {
        (
            "nav_math_overflow",
            "floor remainder exceeds u128::MAX".to_string(),
        )
    })?;
    Ok((expected, rem_u128))
}

/// Universal NAV integrity check (ALL NAVCoins, ALL proof profiles).
pub fn nav_fee_verify_nav_per_unit(
    verified_net_assets: u64,
    circulating_supply: u64,
    precision: u8,
    claimed_nav_per_unit: u64,
    live_ledger_supply: Option<u64>,
) -> Result<(u64, u128), (&'static str, String)> {
    if let Some(live_s) = live_ledger_supply {
        if circulating_supply != live_s {
            return Err((
                "nav_supply_snapshot_stale",
                format!(
                    "packet circulating_supply ({circulating_supply}) != live_ledger_supply ({live_s})"
                ),
            ));
        }
    }
    let (expected, remainder) = nav_fee_compute_nav_per_unit_and_remainder(
        verified_net_assets,
        circulating_supply,
        precision,
    )?;
    if circulating_supply > 0 && claimed_nav_per_unit != expected {
        return Err((
            "nav_per_unit_formula_mismatch",
            format!("claimed nav_per_unit ({claimed_nav_per_unit}) != floor(V*U/S) ({expected})"),
        ));
    }
    Ok((expected, remainder))
}

pub fn nav_fee_validate_schedule_input(
    input: &NavFeeScheduleInputV1,
    issuer: &str,
    max_epoch_gap_blocks: Option<u64>,
) -> Result<NavFeeScheduleInputV1, (&'static str, String)> {
    let canonical_issuer = nav_fee_normalize_canonical_account(issuer, "issuer")?;
    let canonical_recipient = nav_fee_normalize_canonical_account(&input.recipient, "recipient")?;
    if canonical_recipient == canonical_issuer {
        return Err((
            "nav_fee_recipient_equals_issuer",
            "fee recipient account must not equal issuer account".to_string(),
        ));
    }
    if input.annual_rate_bps > NAV_FEE_MAX_ANNUAL_RATE_BPS
        || input.hwm_rate_bps > NAV_FEE_MAX_HWM_RATE_BPS
        || input.issue_spread_bps > NAV_FEE_MAX_SPREAD_BPS
        || input.redeem_spread_bps > NAV_FEE_MAX_SPREAD_BPS
        || input.fix_fee_cap_bps > NAV_FEE_MAX_FIX_FEE_CAP_BPS
    {
        return Err((
            "nav_fee_rate_cap_exceeded",
            "fee schedule rate or spread exceeds protocol cap".to_string(),
        ));
    }
    if input.increase_notice_blocks < NAV_FEE_MIN_INCREASE_NOTICE_BLOCKS {
        return Err((
            "nav_fee_notice_too_short",
            format!(
                "increase_notice_blocks ({}) < minimum ({NAV_FEE_MIN_INCREASE_NOTICE_BLOCKS})",
                input.increase_notice_blocks
            ),
        ));
    }
    if input.accrual_cap_blocks == 0 {
        return Err((
            "nav_fee_invalid_accrual_cap",
            "accrual_cap_blocks must be > 0".to_string(),
        ));
    }
    if let Some(gap_cap) = max_epoch_gap_blocks {
        if gap_cap > 0 && input.accrual_cap_blocks > gap_cap {
            return Err((
                "nav_fee_invalid_accrual_cap",
                format!(
                    "accrual_cap_blocks ({}) exceeds max_epoch_gap_blocks ({gap_cap})",
                    input.accrual_cap_blocks
                ),
            ));
        }
    }
    if let Some(disclosure_ref) = &input.disclosure_ref {
        if disclosure_ref.len() > NAV_FEE_MAX_DISCLOSURE_REF_BYTES {
            return Err((
                "nav_fee_disclosure_ref_too_long",
                format!("disclosure_ref exceeds {NAV_FEE_MAX_DISCLOSURE_REF_BYTES} UTF-8 bytes"),
            ));
        }
    }
    let mut normalized = input.clone();
    normalized.recipient = canonical_recipient;
    Ok(normalized)
}

pub fn nav_fee_is_schedule_increase(
    current: &NavFeeScheduleV1,
    proposed: &NavFeeScheduleInputV1,
) -> bool {
    proposed.annual_rate_bps > current.annual_rate_bps
        || proposed.hwm_rate_bps > current.hwm_rate_bps
        || proposed.issue_spread_bps > current.issue_spread_bps
        || proposed.redeem_spread_bps > current.redeem_spread_bps
        || proposed.fix_fee_cap_bps > current.fix_fee_cap_bps
        || proposed.accrual_cap_blocks > current.accrual_cap_blocks
        || proposed.increase_notice_blocks < current.increase_notice_blocks
}

pub fn nav_fee_attach_schedule(
    existing_fee_state: Option<&NavFeeStateV1>,
    issuer: &str,
    schedule_input: &NavFeeScheduleInputV1,
    block_height: u64,
    current_nav_per_unit: u64,
    max_epoch_gap_blocks: Option<u64>,
) -> Result<NavFeeStateV1, (&'static str, String)> {
    if existing_fee_state.is_some() {
        return Err((
            "nav_fee_schedule_already_attached",
            "asset already has an attached fee schedule".to_string(),
        ));
    }
    let validated = nav_fee_validate_schedule_input(schedule_input, issuer, max_epoch_gap_blocks)?;
    Ok(NavFeeStateV1 {
        active: NavFeeScheduleV1 {
            annual_rate_bps: validated.annual_rate_bps,
            hwm_rate_bps: validated.hwm_rate_bps,
            issue_spread_bps: validated.issue_spread_bps,
            redeem_spread_bps: validated.redeem_spread_bps,
            fix_fee_cap_bps: validated.fix_fee_cap_bps,
            recipient: validated.recipient,
            increase_notice_blocks: validated.increase_notice_blocks,
            accrual_cap_blocks: validated.accrual_cap_blocks,
            rates_locked: validated.rates_locked,
            disclosure_ref: validated.disclosure_ref,
            attached_at_height: block_height,
        },
        pending: None,
        high_water_mark_nav_per_unit: current_nav_per_unit,
        last_accrual_height: block_height,
        unclaimed_fee_units: 0,
        total_accrued_fee_units: 0,
        total_claimed_fee_units: 0,
    })
}

pub fn nav_fee_propose_schedule_update(
    fee_state: Option<&NavFeeStateV1>,
    issuer: &str,
    schedule_input: &NavFeeScheduleInputV1,
    block_height: u64,
    max_epoch_gap_blocks: Option<u64>,
    recipient_cosigned: bool,
) -> Result<NavFeeStateV1, (&'static str, String)> {
    let state = fee_state.ok_or_else(|| {
        (
            "nav_fee_schedule_missing",
            "no fee schedule attached to asset".to_string(),
        )
    })?;
    let validated = nav_fee_validate_schedule_input(schedule_input, issuer, max_epoch_gap_blocks)?;
    let active = &state.active;
    let is_increase = nav_fee_is_schedule_increase(active, &validated);
    if active.rates_locked && (!validated.rates_locked || is_increase) {
        return Err((
            "nav_fee_schedule_rates_locked",
            "fee schedule rates are permanently locked against increases or unlocking".to_string(),
        ));
    }
    let effective_at_height = if is_increase {
        let notice = active
            .increase_notice_blocks
            .max(NAV_FEE_MIN_INCREASE_NOTICE_BLOCKS);
        block_height.checked_add(notice).ok_or_else(|| {
            (
                "nav_fee_effective_height_overflow",
                "effective_at_height exceeds u64::MAX".to_string(),
            )
        })?
    } else if active.rates_locked && validated.recipient != active.recipient && !recipient_cosigned {
        block_height
            .checked_add(NAV_FEE_MIN_INCREASE_NOTICE_BLOCKS)
            .ok_or_else(|| {
                (
                    "nav_fee_effective_height_overflow",
                    "effective_at_height exceeds u64::MAX".to_string(),
                )
            })?
    } else {
        block_height
    };
    let mut next = state.clone();
    next.pending = Some(PendingNavFeeScheduleV1 {
        schedule: NavFeeScheduleV1 {
            annual_rate_bps: validated.annual_rate_bps,
            hwm_rate_bps: validated.hwm_rate_bps,
            issue_spread_bps: validated.issue_spread_bps,
            redeem_spread_bps: validated.redeem_spread_bps,
            fix_fee_cap_bps: validated.fix_fee_cap_bps,
            recipient: validated.recipient,
            increase_notice_blocks: validated.increase_notice_blocks,
            accrual_cap_blocks: validated.accrual_cap_blocks,
            rates_locked: validated.rates_locked,
            disclosure_ref: validated.disclosure_ref,
            attached_at_height: active.attached_at_height,
        },
        proposed_at_height: block_height,
        effective_at_height,
        is_increase,
    });
    Ok(next)
}

pub fn nav_fee_cancel_pending(
    fee_state: Option<&NavFeeStateV1>,
) -> Result<NavFeeStateV1, (&'static str, String)> {
    let state = fee_state.ok_or_else(|| {
        (
            "nav_fee_schedule_missing",
            "no fee schedule attached to asset".to_string(),
        )
    })?;
    if state.pending.is_none() {
        return Err((
            "nav_fee_no_pending_schedule",
            "no pending fee schedule to cancel".to_string(),
        ));
    }
    let mut next = state.clone();
    next.pending = None;
    Ok(next)
}

pub fn nav_fee_claim_units(
    fee_state: Option<&NavFeeStateV1>,
    caller: &str,
    amount: u64,
) -> Result<NavFeeStateV1, (&'static str, String)> {
    let state = fee_state.ok_or_else(|| {
        (
            "nav_fee_schedule_missing",
            "no fee schedule attached to asset".to_string(),
        )
    })?;
    let canonical_caller = nav_fee_normalize_canonical_account(caller, "caller")?;
    if canonical_caller != state.active.recipient.to_ascii_lowercase() {
        return Err((
            "nav_fee_unauthorized_recipient",
            format!(
                "caller {canonical_caller} does not match active fee recipient {}",
                state.active.recipient
            ),
        ));
    }
    if amount == 0 {
        return Err((
            "nav_fee_invalid_claim_amount",
            "claim amount must be > 0".to_string(),
        ));
    }
    if amount > state.unclaimed_fee_units {
        return Err((
            "nav_fee_insufficient_unclaimed",
            format!(
                "claim amount ({amount}) exceeds unclaimed_fee_units ({})",
                state.unclaimed_fee_units
            ),
        ));
    }
    let mut next = state.clone();
    next.unclaimed_fee_units = next
        .unclaimed_fee_units
        .checked_sub(amount)
        .ok_or_else(|| ("nav_fee_insufficient_unclaimed", "underflow".to_string()))?;
    next.total_claimed_fee_units = next
        .total_claimed_fee_units
        .checked_add(amount)
        .ok_or_else(|| ("nav_fee_claim_overflow", "total claimed overflow".to_string()))?;
    Ok(next)
}

pub fn nav_fee_check_exit_capacity_update(
    fee_state: Option<&NavFeeStateV1>,
    current_redeem_capacity_atoms: u64,
    new_redeem_capacity_atoms: u64,
) -> Result<(), (&'static str, String)> {
    if let Some(state) = fee_state {
        if state.pending.as_ref().is_some_and(|p| p.is_increase)
            && new_redeem_capacity_atoms < current_redeem_capacity_atoms
        {
            return Err((
                "nav_fee_pending_increase_exit_capacity_locked",
                "cannot reduce redeem exit capacity while a fee increase is pending".to_string(),
            ));
        }
    }
    Ok(())
}

pub fn nav_fee_check_unscheduled_spread_update(
    current_issue_spread_bps: u32,
    current_redeem_spread_bps: u32,
    new_issue_spread_bps: u32,
    new_redeem_spread_bps: u32,
    proposed_at_height: u64,
    effective_at_height: u64,
) -> Result<(), (&'static str, String)> {
    if new_issue_spread_bps > NAV_FEE_MAX_SPREAD_BPS
        || new_redeem_spread_bps > NAV_FEE_MAX_SPREAD_BPS
    {
        return Err((
            "nav_fee_rate_cap_exceeded",
            "spread exceeds 500 bps protocol cap".to_string(),
        ));
    }
    let is_hike = new_issue_spread_bps > current_issue_spread_bps
        || new_redeem_spread_bps > current_redeem_spread_bps;
    let min_effective = proposed_at_height.saturating_add(NAV_FEE_MIN_INCREASE_NOTICE_BLOCKS);
    if is_hike && effective_at_height < min_effective {
        return Err((
            "nav_fee_notice_too_short",
            "spread increase on unscheduled NAVCoin requires at least 604,800 blocks notice"
                .to_string(),
        ));
    }
    Ok(())
}

pub fn nav_fee_check_fx_fix_fee_cap(
    fee_bps: u32,
    fee_state: Option<&NavFeeStateV1>,
) -> Result<(), (&'static str, String)> {
    if fee_bps > NAV_FEE_MAX_FIX_FEE_CAP_BPS {
        return Err((
            "fx_fix_fee_exceeds_nav_cap",
            format!("FX fix fee_bps ({fee_bps}) exceeds protocol cap ({NAV_FEE_MAX_FIX_FEE_CAP_BPS})"),
        ));
    }
    if let Some(state) = fee_state {
        if fee_bps > state.active.fix_fee_cap_bps {
            return Err((
                "fx_fix_fee_exceeds_nav_cap",
                format!(
                    "FX fix fee_bps ({fee_bps}) exceeds asset fix_fee_cap_bps ({})",
                    state.active.fix_fee_cap_bps
                ),
            ));
        }
    }
    Ok(())
}

pub fn nav_fee_check_non_issuer_attestation(
    issuer: &str,
    fee_state: Option<&NavFeeStateV1>,
    attested_value: u64,
    attestations: &[NavReserveAttestation],
) -> Result<(), (&'static str, String)> {
    if fee_state.is_none() || attested_value == 0 {
        return Ok(());
    }
    let canonical_issuer = issuer.to_ascii_lowercase();
    for att in attestations {
        if !att.pass {
            continue;
        }
        if postfiat_crypto_provider::validate_canonical_address(&att.attestor).is_err() {
            continue;
        }
        if att.attestor.to_ascii_lowercase() != canonical_issuer {
            return Ok(());
        }
    }
    Err((
        "nav_fee_issuer_self_attestation_forbidden",
        "fee-enabled NAVCoin with attested_value > 0 requires a passing non-issuer attestor"
            .to_string(),
    ))
}

/// Exact 4-step deterministic Pillar 7 epoch accrual (u256 arithmetic, floor rounding).
pub fn nav_fee_accrue_epoch(
    asset_id: &str,
    epoch: u64,
    block_height: u64,
    verified_net_assets: u64,
    supply_before: u64,
    precision: u8,
    fee_state: &NavFeeStateV1,
    max_supply: Option<u64>,
    blocks_per_year: u64,
) -> Result<(NavFeeStateV1, NavFeeAccrualV1), (&'static str, String)> {
    if blocks_per_year == 0 {
        return Err((
            "nav_fee_invalid_blocks_per_year",
            "blocks_per_year must be > 0".to_string(),
        ));
    }
    let scale = NavU256::pow10(precision).ok_or_else(|| {
        (
            "bad_nav_asset_precision",
            format!("precision {precision} exceeds {MAX_ISSUED_ASSET_PRECISION}"),
        )
    })?;

    let old_sched = &fee_state.active;
    let (m, p, accrual_cap, next_active, next_pending) = match &fee_state.pending {
        Some(pending) if block_height >= pending.effective_at_height => {
            let new_sched = &pending.schedule;
            (
                old_sched.annual_rate_bps.min(new_sched.annual_rate_bps),
                old_sched.hwm_rate_bps.min(new_sched.hwm_rate_bps),
                old_sched
                    .accrual_cap_blocks
                    .min(new_sched.accrual_cap_blocks),
                new_sched.clone(),
                None,
            )
        }
        pending => (
            old_sched.annual_rate_bps,
            old_sched.hwm_rate_bps,
            old_sched.accrual_cap_blocks,
            old_sched.clone(),
            pending.clone(),
        ),
    };

    let elapsed = block_height.saturating_sub(fee_state.last_accrual_height);
    let delta_blocks = elapsed.min(accrual_cap);

    // Edge fail-safe 1: S == 0
    if supply_before == 0 {
        let accrual = NavFeeAccrualV1 {
            asset_id: asset_id.to_string(),
            epoch,
            block_height,
            blocks_per_year,
            delta_blocks,
            verified_net_assets,
            supply_before: 0,
            nav_before: 0,
            hwm_before: fee_state.high_water_mark_nav_per_unit,
            annual_rate_bps_applied: m,
            hwm_rate_bps_applied: p,
            time_fee_value: 0,
            nav_after_time_fee: 0,
            hwm_fee_value: 0,
            total_fee_value: 0,
            minted_fee_units: 0,
            clipped_fee_units: 0,
            supply_after: 0,
            nav_after: 0,
            hwm_after: fee_state.high_water_mark_nav_per_unit,
            reason_code: "zero_supply".to_string(),
        };
        let mut next_state = fee_state.clone();
        next_state.active = next_active;
        next_state.pending = next_pending;
        next_state.last_accrual_height = block_height;
        return Ok((next_state, accrual));
    }

    // Edge fail-safe 2: V == 0
    if verified_net_assets == 0 {
        let accrual = NavFeeAccrualV1 {
            asset_id: asset_id.to_string(),
            epoch,
            block_height,
            blocks_per_year,
            delta_blocks,
            verified_net_assets: 0,
            supply_before,
            nav_before: 0,
            hwm_before: fee_state.high_water_mark_nav_per_unit,
            annual_rate_bps_applied: m,
            hwm_rate_bps_applied: p,
            time_fee_value: 0,
            nav_after_time_fee: 0,
            hwm_fee_value: 0,
            total_fee_value: 0,
            minted_fee_units: 0,
            clipped_fee_units: 0,
            supply_after: supply_before,
            nav_after: 0,
            hwm_after: fee_state.high_water_mark_nav_per_unit,
            reason_code: "zero_net_assets".to_string(),
        };
        let mut next_state = fee_state.clone();
        next_state.active = next_active;
        next_state.pending = next_pending;
        next_state.last_accrual_height = block_height;
        return Ok((next_state, accrual));
    }

    let v_256 = NavU256::from_u64(verified_net_assets);
    let s_256 = NavU256::from_u64(supply_before);
    let (nav_before_256, _) = v_256
        .checked_mul(scale)
        .and_then(|n| n.div_rem(s_256))
        .ok_or_else(|| ("nav_math_overflow", "nav_before overflow".to_string()))?;
    let nav_before = nav_before_256
        .to_u64_checked()
        .ok_or_else(|| ("nav_math_overflow", "nav_before exceeds u64::MAX".to_string()))?;
    let hwm_before = if fee_state.high_water_mark_nav_per_unit == 0 {
        nav_before
    } else {
        fee_state.high_water_mark_nav_per_unit
    };

    // Edge fail-safe 3: zero applied rates
    if m == 0 && p == 0 {
        let hwm_after = hwm_before.max(nav_before);
        let accrual = NavFeeAccrualV1 {
            asset_id: asset_id.to_string(),
            epoch,
            block_height,
            blocks_per_year,
            delta_blocks,
            verified_net_assets,
            supply_before,
            nav_before,
            hwm_before,
            annual_rate_bps_applied: 0,
            hwm_rate_bps_applied: 0,
            time_fee_value: 0,
            nav_after_time_fee: nav_before,
            hwm_fee_value: 0,
            total_fee_value: 0,
            minted_fee_units: 0,
            clipped_fee_units: 0,
            supply_after: supply_before,
            nav_after: nav_before,
            hwm_after,
            reason_code: "zero_rates".to_string(),
        };
        let mut next_state = fee_state.clone();
        next_state.active = next_active;
        next_state.pending = next_pending;
        next_state.high_water_mark_nav_per_unit = hwm_after;
        next_state.last_accrual_height = block_height;
        return Ok((next_state, accrual));
    }

    // Step 1: Time-based fee
    let time_denom = NavU256::from_u64(NAV_FEE_BPS_DENOMINATOR)
        .checked_mul(NavU256::from_u64(blocks_per_year))
        .ok_or_else(|| ("nav_math_overflow", "time fee denominator overflow".to_string()))?;
    let (f_m_256, _) = v_256
        .checked_mul(NavU256::from_u64(u64::from(m)))
        .and_then(|n| n.checked_mul(NavU256::from_u64(delta_blocks)))
        .and_then(|n| n.div_rem(time_denom))
        .ok_or_else(|| ("nav_math_overflow", "time fee overflow".to_string()))?;

    let clamped_full = |m: u32, p: u32| -> (NavFeeStateV1, NavFeeAccrualV1) {
        let hwm_after = hwm_before.max(nav_before);
        let accrual = NavFeeAccrualV1 {
            asset_id: asset_id.to_string(),
            epoch,
            block_height,
            blocks_per_year,
            delta_blocks,
            verified_net_assets,
            supply_before,
            nav_before,
            hwm_before,
            annual_rate_bps_applied: m,
            hwm_rate_bps_applied: p,
            time_fee_value: 0,
            nav_after_time_fee: nav_before,
            hwm_fee_value: 0,
            total_fee_value: 0,
            minted_fee_units: 0,
            clipped_fee_units: 0,
            supply_after: supply_before,
            nav_after: nav_before,
            hwm_after,
            reason_code: "clamped_full_value".to_string(),
        };
        let mut next_state = fee_state.clone();
        next_state.active = next_active.clone();
        next_state.pending = next_pending.clone();
        next_state.high_water_mark_nav_per_unit = hwm_after;
        next_state.last_accrual_height = block_height;
        (next_state, accrual)
    };

    if f_m_256 >= v_256 {
        return Ok(clamped_full(m, p));
    }
    let f_m = f_m_256
        .to_u64_checked()
        .ok_or_else(|| ("nav_math_overflow", "time fee exceeds u64::MAX".to_string()))?;

    let (nav_after_time_fee_256, _) = NavU256::from_u64(verified_net_assets - f_m)
        .checked_mul(scale)
        .and_then(|n| n.div_rem(s_256))
        .ok_or_else(|| ("nav_math_overflow", "nav_after_time_fee overflow".to_string()))?;
    let nav_after_time_fee = nav_after_time_fee_256.to_u64_checked().ok_or_else(|| {
        (
            "nav_math_overflow",
            "nav_after_time_fee exceeds u64::MAX".to_string(),
        )
    })?;

    // Step 2: High-water-mark gain fee
    let gain = nav_after_time_fee.saturating_sub(hwm_before);
    let hwm_denom = NavU256::from_u64(NAV_FEE_BPS_DENOMINATOR)
        .checked_mul(scale)
        .ok_or_else(|| ("nav_math_overflow", "hwm fee denominator overflow".to_string()))?;
    let (f_p_256, _) = NavU256::from_u64(gain)
        .checked_mul(s_256)
        .and_then(|n| n.checked_mul(NavU256::from_u64(u64::from(p))))
        .and_then(|n| n.div_rem(hwm_denom))
        .ok_or_else(|| ("nav_math_overflow", "hwm fee overflow".to_string()))?;

    // Step 3: Atomic unit mint
    let f_total_256 = f_m_256
        .checked_add(f_p_256)
        .ok_or_else(|| ("nav_math_overflow", "total fee overflow".to_string()))?;
    if f_total_256 >= v_256 {
        return Ok(clamped_full(m, p));
    }
    let f_p = f_p_256
        .to_u64_checked()
        .ok_or_else(|| ("nav_math_overflow", "hwm fee exceeds u64::MAX".to_string()))?;
    let f_total = f_total_256
        .to_u64_checked()
        .ok_or_else(|| ("nav_math_overflow", "total fee exceeds u64::MAX".to_string()))?;

    let v_minus_f = NavU256::from_u64(verified_net_assets - f_total);
    let (delta_s_raw_256, _) = f_total_256
        .checked_mul(s_256)
        .and_then(|n| n.div_rem(v_minus_f))
        .ok_or_else(|| ("nav_math_overflow", "delta_s_raw overflow".to_string()))?;

    let effective_supply_cap = max_supply.unwrap_or(u64::MAX);
    let effective_supply_cap_256 = NavU256::from_u64(effective_supply_cap);
    let s_plus_raw_256 = s_256
        .checked_add(delta_s_raw_256)
        .ok_or_else(|| ("nav_math_overflow", "supply + delta_s_raw overflow".to_string()))?;

    let (minted, clipped, reason_code) = if s_plus_raw_256 > effective_supply_cap_256 {
        let minted = effective_supply_cap.saturating_sub(supply_before);
        let clipped_256 = delta_s_raw_256
            .checked_sub(NavU256::from_u64(minted))
            .ok_or_else(|| ("nav_math_overflow", "clipped underflow".to_string()))?;
        let clipped = clipped_256
            .to_u64_checked()
            .ok_or_else(|| ("nav_math_overflow", "clipped exceeds u64::MAX".to_string()))?;
        (minted, clipped, "clamped_max_supply")
    } else {
        let minted = delta_s_raw_256
            .to_u64_checked()
            .ok_or_else(|| ("nav_math_overflow", "minted exceeds u64::MAX".to_string()))?;
        let reason = if delta_blocks == 0 && f_total == 0 {
            "zero_delta_blocks"
        } else {
            "accrued"
        };
        (minted, 0, reason)
    };

    // Step 4: State update
    let supply_after = supply_before
        .checked_add(minted)
        .ok_or_else(|| ("nav_math_overflow", "supply_after overflow".to_string()))?;
    let (nav_after_256, _) = v_256
        .checked_mul(scale)
        .and_then(|n| n.div_rem(NavU256::from_u64(supply_after)))
        .ok_or_else(|| ("nav_math_overflow", "nav_after overflow".to_string()))?;
    let nav_after = nav_after_256
        .to_u64_checked()
        .ok_or_else(|| ("nav_math_overflow", "nav_after exceeds u64::MAX".to_string()))?;
    let hwm_after = hwm_before.max(nav_after);

    let accrual = NavFeeAccrualV1 {
        asset_id: asset_id.to_string(),
        epoch,
        block_height,
        blocks_per_year,
        delta_blocks,
        verified_net_assets,
        supply_before,
        nav_before,
        hwm_before,
        annual_rate_bps_applied: m,
        hwm_rate_bps_applied: p,
        time_fee_value: f_m,
        nav_after_time_fee,
        hwm_fee_value: f_p,
        total_fee_value: f_total,
        minted_fee_units: minted,
        clipped_fee_units: clipped,
        supply_after,
        nav_after,
        hwm_after,
        reason_code: reason_code.to_string(),
    };

    let mut next_state = fee_state.clone();
    next_state.active = next_active;
    next_state.pending = next_pending;
    next_state.high_water_mark_nav_per_unit = hwm_after;
    next_state.last_accrual_height = block_height;
    next_state.unclaimed_fee_units = next_state
        .unclaimed_fee_units
        .checked_add(minted)
        .ok_or_else(|| ("nav_math_overflow", "unclaimed_fee_units overflow".to_string()))?;
    next_state.total_accrued_fee_units = next_state
        .total_accrued_fee_units
        .checked_add(minted)
        .ok_or_else(|| {
            (
                "nav_math_overflow",
                "total_accrued_fee_units overflow".to_string(),
            )
        })?;

    Ok((next_state, accrual))
}
