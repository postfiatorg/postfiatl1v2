//! Deterministic, non-trading YOLO portfolio-target calculation.
//!
//! This is a direct Rust implementation of the owner-locked Python
//! methodology. It produces theoretical target quantities only and contains no
//! broker client, order type, execution loop, or account mutation capability.

use crate::yolo_collection::{domain_sha256, domain_sha256_canonical_bytes, parse_date, parse_python_utc, validate_digest};
use serde::{Deserialize, Serialize};
use std::cmp::{Ordering, Reverse};
use std::collections::{BTreeMap, BTreeSet};

pub const YOLO_PPB: u64 = 1_000_000_000;
pub const YOLO_PORTFOLIO_PARAMETER_SCHEMA_V1: &str = "postfiat.yolo.portfolio_parameters.v1";
pub const YOLO_PORTFOLIO_TARGET_INPUT_SCHEMA_V1: &str = "postfiat.yolo.portfolio_target_input.v1";
pub const YOLO_PORTFOLIO_TARGET_SCHEMA_V1: &str = "postfiat.yolo.portfolio_target.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct YoloPortfolioParametersV1 {
    pub schema: String,
    pub target_premium_ppb: u64,
    pub basket_size: u64,
    pub observation_count: u64,
    pub per_contract_valid_quotes: u64,
    pub maximum_quote_age_seconds: u64,
    pub scheduled_time_tolerance_seconds: u64,
    pub successor_minimum_dte: u64,
    pub successor_maximum_dte: u64,
    pub target_dte: u64,
    pub roll_dte: u64,
    pub minimum_moneyness_ppb: u64,
    pub maximum_moneyness_ppb: u64,
    pub minimum_open_interest: u64,
    pub minimum_session_volume: u64,
    pub minimum_two_sided_size: u64,
    pub maximum_relative_spread_bps: u64,
    pub spread_weight_ppb: u64,
    pub open_interest_weight_ppb: u64,
    pub volume_weight_ppb: u64,
    pub displayed_size_weight_ppb: u64,
    pub open_interest_cap: u64,
    pub volume_cap: u64,
    pub displayed_size_cap: u64,
    pub strike_retention_rank: u64,
}

impl YoloPortfolioParametersV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != YOLO_PORTFOLIO_PARAMETER_SCHEMA_V1 {
            return Err("portfolio parameter schema mismatch".to_string());
        }
        for (field, value) in [
            ("target_premium_ppb", self.target_premium_ppb),
            ("basket_size", self.basket_size),
            ("observation_count", self.observation_count),
            ("per_contract_valid_quotes", self.per_contract_valid_quotes),
            ("maximum_quote_age_seconds", self.maximum_quote_age_seconds),
            (
                "scheduled_time_tolerance_seconds",
                self.scheduled_time_tolerance_seconds,
            ),
            ("successor_minimum_dte", self.successor_minimum_dte),
            ("successor_maximum_dte", self.successor_maximum_dte),
            ("target_dte", self.target_dte),
            ("roll_dte", self.roll_dte),
            ("minimum_moneyness_ppb", self.minimum_moneyness_ppb),
            ("maximum_moneyness_ppb", self.maximum_moneyness_ppb),
            ("minimum_open_interest", self.minimum_open_interest),
            ("minimum_session_volume", self.minimum_session_volume),
            ("minimum_two_sided_size", self.minimum_two_sided_size),
            (
                "maximum_relative_spread_bps",
                self.maximum_relative_spread_bps,
            ),
            ("spread_weight_ppb", self.spread_weight_ppb),
            ("open_interest_weight_ppb", self.open_interest_weight_ppb),
            ("volume_weight_ppb", self.volume_weight_ppb),
            ("displayed_size_weight_ppb", self.displayed_size_weight_ppb),
            ("open_interest_cap", self.open_interest_cap),
            ("volume_cap", self.volume_cap),
            ("displayed_size_cap", self.displayed_size_cap),
            ("strike_retention_rank", self.strike_retention_rank),
        ] {
            if value == 0 {
                return Err(format!("{field} must be a positive integer"));
            }
        }
        if self.target_premium_ppb > YOLO_PPB {
            return Err("target_premium_ppb must not exceed PPB".to_string());
        }
        if self.per_contract_valid_quotes > self.observation_count {
            return Err("valid quote requirement exceeds observation count".to_string());
        }
        if self.successor_minimum_dte > self.successor_maximum_dte {
            return Err("successor DTE interval is inverted".to_string());
        }
        if self.target_dte < self.successor_minimum_dte
            || self.target_dte > self.successor_maximum_dte
        {
            return Err("target DTE must be inside the successor interval".to_string());
        }
        if self.minimum_moneyness_ppb > self.maximum_moneyness_ppb {
            return Err("moneyness interval is inverted".to_string());
        }
        let weight_sum = u128::from(self.spread_weight_ppb)
            + u128::from(self.open_interest_weight_ppb)
            + u128::from(self.volume_weight_ppb)
            + u128::from(self.displayed_size_weight_ppb);
        if weight_sum != u128::from(YOLO_PPB) {
            return Err("liquidity weights must sum to PPB".to_string());
        }
        if self.strike_retention_rank < self.basket_size {
            return Err("retention rank must be at least basket size".to_string());
        }
        Ok(())
    }

    pub fn sha256(&self) -> Result<String, String> {
        self.validate()?;
        domain_sha256(YOLO_PORTFOLIO_PARAMETER_SCHEMA_V1, self)
    }
}

// Target input/output structures below declare fields in canonical JSON key
// order. Tests compare direct serialization with the general canonical writer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct YoloContractQuoteV1 {
    pub ask_microdollars: u64,
    pub ask_size: u64,
    pub bid_microdollars: u64,
    pub bid_size: u64,
    pub deliverable_shares: u64,
    pub exercise_style: String,
    pub expiration_date: String,
    pub multiplier: u64,
    pub occ_symbol: String,
    pub open_interest: u64,
    pub option_halted: bool,
    pub option_type: String,
    pub quote_timestamp_utc: String,
    pub session_volume: u64,
    pub standard: bool,
    pub strike_microdollars: u64,
}

impl YoloContractQuoteV1 {
    fn validate(&self) -> Result<(), String> {
        if self.occ_symbol.is_empty() {
            return Err("occ_symbol is required".to_string());
        }
        parse_date("expirationDate", &self.expiration_date)?;
        if self.strike_microdollars == 0 {
            return Err("strike_microdollars must be a positive integer".to_string());
        }
        if self.multiplier == 0 || self.deliverable_shares == 0 {
            return Err("contract multiplier and deliverable shares must be positive".to_string());
        }
        parse_python_utc("quoteTimestampUtc", &self.quote_timestamp_utc)?;
        Ok(())
    }

    fn same_static_terms(&self, other: &Self) -> bool {
        self.expiration_date == other.expiration_date
            && self.strike_microdollars == other.strike_microdollars
            && self.option_type == other.option_type
            && self.multiplier == other.multiplier
            && self.deliverable_shares == other.deliverable_shares
            && self.exercise_style == other.exercise_style
            && self.standard == other.standard
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct YoloMethodologySnapshotV1 {
    pub contracts: Vec<YoloContractQuoteV1>,
    pub observed_at_utc: String,
    pub scheduled_at_utc: String,
    pub sequence: u32,
    pub underlier_halted: bool,
    pub underlier_midpoint_microdollars: u64,
}

impl YoloMethodologySnapshotV1 {
    fn validate(&self) -> Result<(), String> {
        parse_python_utc("scheduledAtUtc", &self.scheduled_at_utc)?;
        parse_python_utc("observedAtUtc", &self.observed_at_utc)?;
        if self.underlier_midpoint_microdollars == 0 {
            return Err("underlier_midpoint_microdollars must be a positive integer".to_string());
        }
        let mut symbols = BTreeSet::new();
        for contract in &self.contracts {
            contract.validate()?;
            if !symbols.insert(contract.occ_symbol.as_str()) {
                return Err("contract identifiers must be unique in each snapshot".to_string());
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct YoloCurrentPositionV1 {
    pub occ_symbol: String,
    pub quantity: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct YoloPortfolioTargetInputV1 {
    pub accrued_liabilities_microdollars: u64,
    pub collection_sha256: String,
    pub incumbent_expiration: Option<String>,
    pub incumbent_symbols: Vec<String>,
    pub methodology_sha256: String,
    pub positions: Vec<YoloCurrentPositionV1>,
    pub schema: String,
    pub series_id: String,
    pub settled_cash_microdollars: u64,
    pub snapshots: Vec<YoloMethodologySnapshotV1>,
    pub trade_date: String,
    pub underlier_id: String,
    pub unsettled_cash_microdollars: u64,
}

impl YoloPortfolioTargetInputV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != YOLO_PORTFOLIO_TARGET_INPUT_SCHEMA_V1 {
            return Err("portfolio target input schema mismatch".to_string());
        }
        if self.series_id.is_empty() || self.underlier_id.is_empty() {
            return Err("series_id and underlier_id are required".to_string());
        }
        parse_date("tradeDate", &self.trade_date)?;
        validate_digest("methodology_sha256", &self.methodology_sha256)?;
        validate_digest("collection_sha256", &self.collection_sha256)?;
        let mut position_symbols = BTreeSet::new();
        for position in &self.positions {
            if position.occ_symbol.is_empty() {
                return Err("position occ_symbol is required".to_string());
            }
            if !position_symbols.insert(position.occ_symbol.as_str()) {
                return Err("positions must be unique by OCC symbol".to_string());
            }
        }
        let mut incumbents = BTreeSet::new();
        for symbol in &self.incumbent_symbols {
            if symbol.is_empty() {
                return Err("incumbent symbols must be non-empty strings".to_string());
            }
            if !incumbents.insert(symbol.as_str()) {
                return Err("incumbent symbols must be unique".to_string());
            }
        }
        if let Some(expiration) = &self.incumbent_expiration {
            parse_date("incumbentExpiration", expiration)?;
        }
        for snapshot in &self.snapshots {
            snapshot.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct YoloAggregatedContractV1 {
    pub ask_microdollars: u64,
    pub bid_microdollars: u64,
    pub dte: i64,
    pub exclusions: Vec<String>,
    pub expiration_date: String,
    pub liquidity_ppb: u64,
    pub mandate_eligible: bool,
    pub midpoint_microdollars: u64,
    pub moneyness_ppb: u64,
    pub multiplier: u64,
    pub occ_symbol: String,
    pub open_interest: u64,
    pub option_halted: bool,
    pub session_volume: u64,
    pub spread_bps: u64,
    pub strike_microdollars: u64,
    pub two_sided_size: u64,
    pub valid_quote_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct YoloTargetPositionV1 {
    pub current_quantity: u64,
    pub expiration_date: String,
    pub liquidity_ppb: u64,
    pub midpoint_microdollars: u64,
    pub multiplier: u64,
    pub occ_symbol: String,
    pub retained_incumbent: bool,
    pub target_delta: i128,
    pub target_quantity: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum YoloPortfolioTargetStatusV1 {
    TargetComputed,
    NoReconstitution,
    NoEligibleSuccessor,
    HaltedInstrument,
    UnresolvedCapitalPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct YoloPortfolioTargetV1 {
    pub collection_sha256: String,
    pub decision_nav_microdollars: Option<i128>,
    pub excluded_contracts: Vec<YoloAggregatedContractV1>,
    pub methodology_sha256: String,
    pub parameter_manifest_sha256: String,
    pub positions: Vec<YoloTargetPositionV1>,
    pub premium_budget_microdollars: Option<i128>,
    pub reason: String,
    pub schema: String,
    pub selected_expiration: Option<String>,
    pub sleeve_microdollars: Option<i128>,
    pub status: YoloPortfolioTargetStatusV1,
}

impl YoloPortfolioTargetV1 {
    pub fn sha256(&self) -> Result<String, String> {
        // Preserve the integer range accepted by the former serde_json::Value
        // conversion; direct serialization otherwise supports larger i128s.
        let numbers = self.decision_nav_microdollars.into_iter()
            .chain(self.premium_budget_microdollars)
            .chain(self.sleeve_microdollars)
            .chain(self.positions.iter().map(|position| position.target_delta));
        if numbers.into_iter().any(|value| value < i128::from(i64::MIN) || value > i128::from(u64::MAX)) {
            return Err("YOLO canonical JSON serialization failed".into());
        }
        let bytes = serde_json::to_vec(self).map_err(|_| "YOLO canonical JSON serialization failed")?;
        domain_sha256_canonical_bytes(YOLO_PORTFOLIO_TARGET_SCHEMA_V1, &bytes)
    }
}

fn median(mut values: Vec<u64>) -> Result<u64, String> {
    if values.is_empty() {
        return Err("median requires at least one value".to_string());
    }
    values.sort_unstable();
    let middle = values.len() / 2;
    if values.len() % 2 == 1 {
        Ok(values[middle])
    } else {
        let sum = u128::from(values[middle - 1]) + u128::from(values[middle]);
        u64::try_from(sum / 2).map_err(|_| "median exceeds numeric bounds".to_string())
    }
}

fn valid_quote(
    quote: &YoloContractQuoteV1,
    observed_at: &str,
    parameters: &YoloPortfolioParametersV1,
) -> Result<bool, String> {
    let observed = parse_python_utc("observed_at", observed_at)?;
    let quote_time = parse_python_utc("quote_timestamp", &quote.quote_timestamp_utc)?;
    let age = observed.signed_duration_since(quote_time);
    let maximum = i64::try_from(parameters.maximum_quote_age_seconds)
        .map_err(|_| "maximum quote age exceeds duration bounds".to_string())?
        .checked_mul(1_000_000)
        .ok_or_else(|| "maximum quote age exceeds duration bounds".to_string())?;
    let age_micros = age
        .num_microseconds()
        .ok_or_else(|| "quote age exceeds duration bounds".to_string())?;
    Ok(quote.bid_microdollars > 0
        && quote.bid_microdollars <= quote.ask_microdollars
        && age_micros >= 0
        && age_micros <= maximum)
}

fn liquidity(
    spread_bps: u64,
    open_interest: u64,
    volume: u64,
    size: u64,
    parameters: &YoloPortfolioParametersV1,
) -> Result<u64, String> {
    let ppb = u128::from(YOLO_PPB);
    let spread_penalty =
        ppb * u128::from(spread_bps) / u128::from(parameters.maximum_relative_spread_bps);
    let spread_quality = ppb.saturating_sub(spread_penalty);
    let oi = (ppb * u128::from(open_interest) / u128::from(parameters.open_interest_cap)).min(ppb);
    let vol = (ppb * u128::from(volume) / u128::from(parameters.volume_cap)).min(ppb);
    let displayed = (ppb * u128::from(size) / u128::from(parameters.displayed_size_cap)).min(ppb);
    let score = (u128::from(parameters.spread_weight_ppb) * spread_quality
        + u128::from(parameters.open_interest_weight_ppb) * oi
        + u128::from(parameters.volume_weight_ppb) * vol
        + u128::from(parameters.displayed_size_weight_ppb) * displayed)
        / ppb;
    u64::try_from(score).map_err(|_| "liquidity score exceeds numeric bounds".to_string())
}

fn aggregate(
    target: &YoloPortfolioTargetInputV1,
    parameters: &YoloPortfolioParametersV1,
) -> Result<(u64, Vec<YoloAggregatedContractV1>), String> {
    let mut snapshots: Vec<&YoloMethodologySnapshotV1> = target.snapshots.iter().collect();
    snapshots.sort_by_key(|snapshot| snapshot.sequence);
    if snapshots.len()
        != usize::try_from(parameters.observation_count)
            .map_err(|_| "observation count exceeds platform bounds".to_string())?
        || snapshots
            .iter()
            .enumerate()
            .any(|(sequence, snapshot)| snapshot.sequence as usize != sequence)
    {
        return Err("collection does not contain the required snapshot sequence".to_string());
    }
    for snapshot in &snapshots {
        let scheduled = parse_python_utc("scheduled", &snapshot.scheduled_at_utc)?;
        let observed = parse_python_utc("observed", &snapshot.observed_at_utc)?;
        let drift = observed.signed_duration_since(scheduled);
        let drift_micros = drift
            .num_microseconds()
            .ok_or_else(|| "snapshot drift exceeds duration bounds".to_string())?
            .unsigned_abs();
        let maximum = parameters
            .scheduled_time_tolerance_seconds
            .checked_mul(1_000_000)
            .ok_or_else(|| "scheduled-time tolerance exceeds duration bounds".to_string())?;
        if drift_micros > maximum {
            return Err("snapshot exceeds the scheduled-time tolerance".to_string());
        }
    }
    let underlier = median(
        snapshots
            .iter()
            .map(|snapshot| snapshot.underlier_midpoint_microdollars)
            .collect(),
    )?;
    // Index every raw observation once. BTreeMap preserves the former sorted
    // symbol order; each vector preserves chronological snapshot/quote order.
    // Quote validation stays below so the first reported error is unchanged.
    let mut by_symbol: BTreeMap<
        &str,
        Vec<(&YoloMethodologySnapshotV1, &YoloContractQuoteV1)>,
    > = BTreeMap::new();
    for snapshot in &snapshots {
        for quote in &snapshot.contracts {
            by_symbol
                .entry(quote.occ_symbol.as_str())
                .or_default()
                .push((snapshot, quote));
        }
    }
    let trade_date = parse_date("tradeDate", &target.trade_date)?;
    let mut contracts = Vec::new();
    for (symbol, raw_observations) in by_symbol {
        let mut observations: Vec<(&YoloMethodologySnapshotV1, &YoloContractQuoteV1)> = Vec::new();
        for &(snapshot, quote) in &raw_observations {
            if valid_quote(quote, &snapshot.observed_at_utc, parameters)? {
                observations.push((snapshot, quote));
            }
        }
        if observations.is_empty() {
            continue;
        }
        let representative = observations[0].1;
        let consistent = observations
            .iter()
            .all(|(_, quote)| representative.same_static_terms(quote));
        let valid_count = if consistent {
            u64::try_from(observations.len())
                .map_err(|_| "valid quote count exceeds numeric bounds".to_string())?
        } else {
            0
        };
        let bid = median(
            observations
                .iter()
                .map(|(_, quote)| quote.bid_microdollars)
                .collect(),
        )?;
        let ask = median(
            observations
                .iter()
                .map(|(_, quote)| quote.ask_microdollars)
                .collect(),
        )?;
        let midpoint = u64::try_from((u128::from(bid) + u128::from(ask)) / 2)
            .map_err(|_| "midpoint exceeds numeric bounds".to_string())?;
        let spread_bps = u64::try_from(10_000u128 * u128::from(ask - bid) / u128::from(midpoint))
            .map_err(|_| "relative spread exceeds numeric bounds".to_string())?;
        let size = median(
            observations
                .iter()
                .map(|(_, quote)| quote.bid_size.min(quote.ask_size))
                .collect(),
        )?;
        let interest = median(
            observations
                .iter()
                .map(|(_, quote)| quote.open_interest)
                .collect(),
        )?;
        let volume = observations
            .iter()
            .map(|(_, quote)| quote.session_volume)
            .max()
            .ok_or_else(|| "volume aggregation is empty".to_string())?;
        let expiration = parse_date("expirationDate", &representative.expiration_date)?;
        let dte = expiration.signed_duration_since(trade_date).num_days();
        let moneyness = u64::try_from(
            u128::from(representative.strike_microdollars) * u128::from(YOLO_PPB)
                / u128::from(underlier),
        )
        .map_err(|_| "moneyness exceeds numeric bounds".to_string())?;
        let final_quote = snapshots.last().and_then(|last| {
            raw_observations
                .iter()
                .find(|(snapshot, _)| snapshot.sequence == last.sequence)
                .map(|(_, quote)| *quote)
        });
        let halted = final_quote.is_none_or(|quote| quote.option_halted);
        let mandate = consistent
            && representative.standard
            && representative.option_type == "CALL"
            && representative.multiplier == 100
            && representative.deliverable_shares == 100
            && representative.exercise_style == "AMERICAN";
        let mut exclusions = Vec::new();
        for (condition, reason) in [
            (!consistent, "inconsistent_contract_terms"),
            (!mandate, "mandate_terms"),
            (
                valid_count < parameters.per_contract_valid_quotes,
                "insufficient_valid_quotes",
            ),
            (halted, "halted_or_missing_target_flag"),
            (
                moneyness < parameters.minimum_moneyness_ppb
                    || moneyness > parameters.maximum_moneyness_ppb,
                "moneyness",
            ),
            (
                spread_bps > parameters.maximum_relative_spread_bps,
                "relative_spread",
            ),
            (interest < parameters.minimum_open_interest, "open_interest"),
            (volume < parameters.minimum_session_volume, "session_volume"),
            (size < parameters.minimum_two_sided_size, "two_sided_size"),
        ] {
            if condition {
                exclusions.push(reason.to_string());
            }
        }
        contracts.push(YoloAggregatedContractV1 {
            occ_symbol: symbol.to_string(),
            expiration_date: representative.expiration_date.clone(),
            strike_microdollars: representative.strike_microdollars,
            multiplier: representative.multiplier,
            bid_microdollars: bid,
            ask_microdollars: ask,
            midpoint_microdollars: midpoint,
            two_sided_size: size,
            session_volume: volume,
            open_interest: interest,
            spread_bps,
            moneyness_ppb: moneyness,
            dte,
            valid_quote_count: valid_count,
            option_halted: halted,
            mandate_eligible: mandate,
            liquidity_ppb: liquidity(spread_bps, interest, volume, size, parameters)?,
            exclusions,
        });
    }
    Ok((underlier, contracts))
}

fn rank(
    left: &YoloAggregatedContractV1,
    right: &YoloAggregatedContractV1,
    underlier: u64,
) -> Ordering {
    Reverse(left.liquidity_ppb)
        .cmp(&Reverse(right.liquidity_ppb))
        .then_with(|| {
            left.strike_microdollars
                .abs_diff(underlier)
                .cmp(&right.strike_microdollars.abs_diff(underlier))
        })
        .then_with(|| left.strike_microdollars.cmp(&right.strike_microdollars))
        .then_with(|| left.occ_symbol.cmp(&right.occ_symbol))
}

fn blank_target(
    status: YoloPortfolioTargetStatusV1,
    reason: String,
    parameter_manifest_sha256: &str,
    target: &YoloPortfolioTargetInputV1,
    selected_expiration: Option<String>,
    decision_nav_microdollars: Option<i128>,
    premium_budget_microdollars: Option<i128>,
    sleeve_microdollars: Option<i128>,
    excluded_contracts: Vec<YoloAggregatedContractV1>,
) -> YoloPortfolioTargetV1 {
    YoloPortfolioTargetV1 {
        schema: YOLO_PORTFOLIO_TARGET_SCHEMA_V1.to_string(),
        status,
        reason,
        parameter_manifest_sha256: parameter_manifest_sha256.to_string(),
        methodology_sha256: target.methodology_sha256.clone(),
        collection_sha256: target.collection_sha256.clone(),
        selected_expiration,
        decision_nav_microdollars,
        premium_budget_microdollars,
        sleeve_microdollars,
        positions: Vec::new(),
        excluded_contracts,
    }
}

pub fn create_yolo_portfolio_target_v1(
    target: &YoloPortfolioTargetInputV1,
    parameters: &YoloPortfolioParametersV1,
) -> Result<YoloPortfolioTargetV1, String> {
    target.validate()?;
    parameters.validate()?;
    let parameter_manifest_sha256 = parameters.sha256()?;
    if target.incumbent_symbols.len()
        > usize::try_from(parameters.basket_size)
            .map_err(|_| "basket size exceeds platform bounds".to_string())?
    {
        return Ok(blank_target(
            YoloPortfolioTargetStatusV1::NoReconstitution,
            "prior target contains more incumbents than basket_size".to_string(),
            &parameter_manifest_sha256,
            target,
            None,
            None,
            None,
            None,
            Vec::new(),
        ));
    }
    let (underlier, aggregated) = match aggregate(target, parameters) {
        Ok(value) => value,
        Err(error) => {
            return Ok(blank_target(
                YoloPortfolioTargetStatusV1::NoReconstitution,
                error,
                &parameter_manifest_sha256,
                target,
                None,
                None,
                None,
                None,
                Vec::new(),
            ));
        }
    };
    let mut ordered_snapshots: Vec<&YoloMethodologySnapshotV1> = target.snapshots.iter().collect();
    ordered_snapshots.sort_by_key(|snapshot| snapshot.sequence);
    if ordered_snapshots
        .last()
        .is_some_and(|snapshot| snapshot.underlier_halted)
    {
        return Ok(blank_target(
            YoloPortfolioTargetStatusV1::HaltedInstrument,
            "target-time underlier halt flag is true".to_string(),
            &parameter_manifest_sha256,
            target,
            None,
            None,
            None,
            None,
            aggregated,
        ));
    }

    let mut by_expiration: BTreeMap<&str, Vec<&YoloAggregatedContractV1>> = BTreeMap::new();
    for contract in aggregated
        .iter()
        .filter(|contract| contract.exclusions.is_empty())
    {
        by_expiration
            .entry(&contract.expiration_date)
            .or_default()
            .push(contract);
    }
    let incumbent_set: BTreeSet<&str> = target
        .incumbent_symbols
        .iter()
        .map(String::as_str)
        .collect();
    let mut selected_expiration: Option<String> = None;
    let mut applicable: Vec<&YoloAggregatedContractV1> = Vec::new();
    if let Some(incumbent_expiration) = target.incumbent_expiration.as_deref() {
        let retention: Vec<&YoloAggregatedContractV1> = by_expiration
            .get(incumbent_expiration)
            .into_iter()
            .flatten()
            .copied()
            .filter(|contract| contract.dte > parameters.roll_dte as i64)
            .collect();
        if retention.len()
            >= usize::try_from(parameters.basket_size)
                .map_err(|_| "basket size exceeds platform bounds".to_string())?
        {
            selected_expiration = Some(incumbent_expiration.to_string());
            applicable = retention;
        }
    }

    if selected_expiration.is_none() {
        let mut candidates = Vec::new();
        for (expiration, contracts) in &by_expiration {
            let mut successor: Vec<&YoloAggregatedContractV1> = contracts
                .iter()
                .copied()
                .filter(|contract| {
                    contract.dte >= parameters.successor_minimum_dte as i64
                        && contract.dte <= parameters.successor_maximum_dte as i64
                })
                .collect();
            if successor.len()
                < usize::try_from(parameters.basket_size)
                    .map_err(|_| "basket size exceeds platform bounds".to_string())?
            {
                continue;
            }
            successor.sort_by(|left, right| rank(left, right, underlier));
            let top_count = usize::try_from(parameters.basket_size)
                .map_err(|_| "basket size exceeds platform bounds".to_string())?;
            let median_score = median(
                successor[..top_count]
                    .iter()
                    .map(|contract| contract.liquidity_ppb)
                    .collect(),
            )?;
            let dte_distance = successor[0].dte.abs_diff(parameters.target_dte as i64);
            candidates.push((
                (
                    dte_distance,
                    Reverse(median_score),
                    (*expiration).to_string(),
                ),
                (*expiration).to_string(),
                successor,
            ));
        }
        let Some((_, expiration, successor)) = candidates
            .into_iter()
            .min_by(|left, right| left.0.cmp(&right.0))
        else {
            return Ok(blank_target(
                YoloPortfolioTargetStatusV1::NoEligibleSuccessor,
                "no expiration contains the required successor-eligible calls".to_string(),
                &parameter_manifest_sha256,
                target,
                None,
                None,
                None,
                None,
                aggregated,
            ));
        };
        selected_expiration = Some(expiration);
        applicable = successor;
    }

    applicable.sort_by(|left, right| rank(left, right, underlier));
    let rank_by_symbol: BTreeMap<&str, usize> = applicable
        .iter()
        .enumerate()
        .map(|(index, contract)| (contract.occ_symbol.as_str(), index + 1))
        .collect();
    let retained: Vec<&YoloAggregatedContractV1> = applicable
        .iter()
        .copied()
        .filter(|contract| {
            incumbent_set.contains(contract.occ_symbol.as_str())
                && rank_by_symbol[contract.occ_symbol.as_str()]
                    <= parameters.strike_retention_rank as usize
        })
        .collect();
    let basket_size = usize::try_from(parameters.basket_size)
        .map_err(|_| "basket size exceeds platform bounds".to_string())?;
    let mut selected: Vec<&YoloAggregatedContractV1> =
        retained.iter().copied().take(basket_size).collect();
    let mut selected_symbols: BTreeSet<&str> = selected
        .iter()
        .map(|contract| contract.occ_symbol.as_str())
        .collect();
    for contract in &applicable {
        if selected.len() == basket_size {
            break;
        }
        if selected_symbols.insert(contract.occ_symbol.as_str()) {
            selected.push(contract);
        }
    }
    if selected.len() != basket_size {
        return Ok(blank_target(
            YoloPortfolioTargetStatusV1::NoReconstitution,
            "fewer than the required number of calls can be selected".to_string(),
            &parameter_manifest_sha256,
            target,
            selected_expiration,
            None,
            None,
            None,
            aggregated,
        ));
    }
    selected.sort_by(|left, right| left.occ_symbol.cmp(&right.occ_symbol));

    let aggregated_by_symbol: BTreeMap<&str, &YoloAggregatedContractV1> = aggregated
        .iter()
        .map(|contract| (contract.occ_symbol.as_str(), contract))
        .collect();
    let position_by_symbol: BTreeMap<&str, u64> = target
        .positions
        .iter()
        .map(|position| (position.occ_symbol.as_str(), position.quantity))
        .collect();
    if let Some(missing) = target.positions.iter().find(|position| {
        position.quantity > 0
            && aggregated_by_symbol
                .get(position.occ_symbol.as_str())
                .is_none_or(|contract| {
                    contract.valid_quote_count < parameters.per_contract_valid_quotes
                })
    }) {
        return Ok(blank_target(
            YoloPortfolioTargetStatusV1::NoReconstitution,
            format!(
                "current holding lacks a valid decision mark: {}",
                missing.occ_symbol
            ),
            &parameter_manifest_sha256,
            target,
            selected_expiration,
            None,
            None,
            None,
            aggregated,
        ));
    }
    let mut option_value = 0u128;
    for position in &target.positions {
        if position.quantity == 0 {
            continue;
        }
        let contract = aggregated_by_symbol[position.occ_symbol.as_str()];
        let value = u128::from(position.quantity)
            .checked_mul(u128::from(contract.midpoint_microdollars))
            .and_then(|value| value.checked_mul(u128::from(contract.multiplier)))
            .ok_or_else(|| "option value exceeds numeric bounds".to_string())?;
        option_value = option_value
            .checked_add(value)
            .ok_or_else(|| "option value exceeds numeric bounds".to_string())?;
    }
    let option_value_i128 = i128::try_from(option_value)
        .map_err(|_| "option value exceeds numeric bounds".to_string())?;
    let nav = i128::from(target.settled_cash_microdollars)
        .checked_add(i128::from(target.unsettled_cash_microdollars))
        .and_then(|value| value.checked_add(option_value_i128))
        .and_then(|value| value.checked_sub(i128::from(target.accrued_liabilities_microdollars)))
        .ok_or_else(|| "decision-time NAV exceeds numeric bounds".to_string())?;
    if nav <= 0 {
        return Ok(blank_target(
            YoloPortfolioTargetStatusV1::UnresolvedCapitalPolicy,
            "decision-time NAV is not positive".to_string(),
            &parameter_manifest_sha256,
            target,
            selected_expiration,
            Some(nav),
            Some(0),
            Some(0),
            aggregated,
        ));
    }
    let premium_budget = i128::from(parameters.target_premium_ppb)
        .checked_mul(nav)
        .ok_or_else(|| "premium budget exceeds numeric bounds".to_string())?
        / i128::from(YOLO_PPB);
    let sleeve = premium_budget / i128::from(parameters.basket_size);
    let retained_symbols: BTreeSet<&str> = retained
        .iter()
        .map(|contract| contract.occ_symbol.as_str())
        .collect();
    let mut positions = Vec::new();
    for contract in &selected {
        let denominator = u128::from(contract.midpoint_microdollars)
            .checked_mul(u128::from(contract.multiplier))
            .ok_or_else(|| "target sizing denominator exceeds numeric bounds".to_string())?;
        let target_quantity = u64::try_from(
            u128::try_from(sleeve).map_err(|_| "target sleeve is negative".to_string())?
                / denominator,
        )
        .map_err(|_| "target quantity exceeds numeric bounds".to_string())?;
        let current_quantity = position_by_symbol
            .get(contract.occ_symbol.as_str())
            .copied()
            .unwrap_or(0);
        positions.push(YoloTargetPositionV1 {
            occ_symbol: contract.occ_symbol.clone(),
            expiration_date: contract.expiration_date.clone(),
            midpoint_microdollars: contract.midpoint_microdollars,
            multiplier: contract.multiplier,
            current_quantity,
            target_quantity,
            target_delta: i128::from(target_quantity) - i128::from(current_quantity),
            liquidity_ppb: contract.liquidity_ppb,
            retained_incumbent: retained_symbols.contains(contract.occ_symbol.as_str()),
        });
    }
    for position in &target.positions {
        if position.quantity > 0 && !selected_symbols.contains(position.occ_symbol.as_str()) {
            let contract = aggregated_by_symbol[position.occ_symbol.as_str()];
            positions.push(YoloTargetPositionV1 {
                occ_symbol: position.occ_symbol.clone(),
                expiration_date: contract.expiration_date.clone(),
                midpoint_microdollars: contract.midpoint_microdollars,
                multiplier: contract.multiplier,
                current_quantity: position.quantity,
                target_quantity: 0,
                target_delta: -i128::from(position.quantity),
                liquidity_ppb: contract.liquidity_ppb,
                retained_incumbent: false,
            });
        }
    }
    positions.sort_by(|left, right| left.occ_symbol.cmp(&right.occ_symbol));
    let (status, reason) = if positions
        .iter()
        .all(|position| position.target_quantity == 0)
    {
        (
            YoloPortfolioTargetStatusV1::UnresolvedCapitalPolicy,
            "every selected premium sleeve rounds to zero contracts".to_string(),
        )
    } else {
        (
            YoloPortfolioTargetStatusV1::TargetComputed,
            "reference methodology target computed".to_string(),
        )
    };
    let selected_expiration_ref = selected_expiration.as_deref();
    let mut excluded_contracts = Vec::new();
    for contract in &aggregated {
        if selected_symbols.contains(contract.occ_symbol.as_str()) {
            continue;
        }
        let mut output = contract.clone();
        if output.exclusions.is_empty() {
            output.exclusions = if Some(output.expiration_date.as_str()) != selected_expiration_ref
            {
                vec!["expiration_not_selected".to_string()]
            } else {
                vec!["ranked_out".to_string()]
            };
        }
        excluded_contracts.push(output);
    }
    Ok(YoloPortfolioTargetV1 {
        schema: YOLO_PORTFOLIO_TARGET_SCHEMA_V1.to_string(),
        status,
        reason,
        parameter_manifest_sha256,
        methodology_sha256: target.methodology_sha256.clone(),
        collection_sha256: target.collection_sha256.clone(),
        selected_expiration,
        decision_nav_microdollars: Some(nav),
        premium_budget_microdollars: Some(premium_budget),
        sleeve_microdollars: Some(sleeve),
        positions,
        excluded_contracts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_target_serialization_matches_canonical_json_and_integer_bounds() {
        let mut value = input(&[("2026-11-03", &[47, 48, 49, 50, 51, 52, 53])]);
        for snapshot in &mut value.snapshots {
            snapshot.contracts[0].occ_symbol = "quote-\"é\\\n".to_string();
        }
        value.positions = vec![YoloCurrentPositionV1 { occ_symbol: "quote-\"é\\\n".into(), quantity: 1 }];
        assert_eq!(serde_json::to_vec(&value).unwrap(), crate::yolo_collection::canonical_bytes(&value).unwrap());
        let mut target = create_yolo_portfolio_target_v1(&value, &parameters()).unwrap();
        assert_eq!(serde_json::to_vec(&target).unwrap(), crate::yolo_collection::canonical_bytes(&target).unwrap());
        assert_eq!(target.sha256().unwrap(), domain_sha256(YOLO_PORTFOLIO_TARGET_SCHEMA_V1, &target).unwrap());
        for boundary in [i128::from(i64::MIN), i128::from(u64::MAX), i128::from(i64::MIN)-1,
                         i128::from(u64::MAX)+1, i128::MIN, i128::MAX] {
            target.decision_nav_microdollars = Some(boundary);
            assert_eq!(target.sha256(), domain_sha256(YOLO_PORTFOLIO_TARGET_SCHEMA_V1, &target));
        }
    }

    #[test]
    fn indexed_observations_preserve_final_flags_and_reordering() {
        let mut value = input(&[("2026-11-03", &[47, 48, 49, 50, 51, 52, 53])]);
        // Missing final observations remain halted/missing even with four valid
        // earlier quotes. A stale final quote still supplies its halt flag.
        value.snapshots[4].contracts.remove(0);
        value.snapshots[4].contracts[0].quote_timestamp_utc =
            "2026-09-03T14:30:00Z".to_string();
        value.snapshots[4].contracts[0].option_halted = true;
        // Invalid quotes contribute neither prices nor liquidity medians.
        value.snapshots[0].contracts[2].bid_microdollars = 0;
        let (_, rows) = aggregate(&value, &parameters()).unwrap();
        assert_eq!(rows.len(), 7);
        assert_eq!(rows[0].valid_quote_count, 4);
        assert!(rows[0].option_halted);
        assert_eq!(rows[1].valid_quote_count, 4);
        assert!(rows[1].option_halted);
        assert_eq!(rows[2].valid_quote_count, 4);
        assert_eq!(rows[2].bid_microdollars, 4_900_000);
        let original = create_yolo_portfolio_target_v1(&value, &parameters()).unwrap();
        value.snapshots.reverse();
        for snapshot in &mut value.snapshots {
            snapshot.contracts.reverse();
        }
        assert_eq!(original, create_yolo_portfolio_target_v1(&value, &parameters()).unwrap());
    }

    fn parameters() -> YoloPortfolioParametersV1 {
        YoloPortfolioParametersV1 {
            schema: YOLO_PORTFOLIO_PARAMETER_SCHEMA_V1.to_string(),
            target_premium_ppb: 900_000_000,
            basket_size: 5,
            observation_count: 5,
            per_contract_valid_quotes: 4,
            maximum_quote_age_seconds: 120,
            scheduled_time_tolerance_seconds: 30,
            successor_minimum_dte: 45,
            successor_maximum_dte: 75,
            target_dte: 60,
            roll_dte: 30,
            minimum_moneyness_ppb: 900_000_000,
            maximum_moneyness_ppb: 1_100_000_000,
            minimum_open_interest: 500,
            minimum_session_volume: 100,
            minimum_two_sided_size: 1,
            maximum_relative_spread_bps: 800,
            spread_weight_ppb: 500_000_000,
            open_interest_weight_ppb: 200_000_000,
            volume_weight_ppb: 200_000_000,
            displayed_size_weight_ppb: 100_000_000,
            open_interest_cap: 5_000,
            volume_cap: 1_000,
            displayed_size_cap: 20,
            strike_retention_rank: 8,
        }
    }

    fn quote(symbol: String, strike: u64, expiration: &str, observed: &str) -> YoloContractQuoteV1 {
        YoloContractQuoteV1 {
            occ_symbol: symbol,
            expiration_date: expiration.to_string(),
            strike_microdollars: strike * 1_000_000,
            bid_microdollars: 4_900_000,
            ask_microdollars: 5_100_000,
            bid_size: 10,
            ask_size: 10,
            session_volume: 500,
            open_interest: 2_500,
            quote_timestamp_utc: observed.to_string(),
            option_type: "CALL".to_string(),
            multiplier: 100,
            deliverable_shares: 100,
            exercise_style: "AMERICAN".to_string(),
            standard: true,
            option_halted: false,
        }
    }

    fn snapshot(
        sequence: u32,
        scheduled: &str,
        observed: &str,
        expirations: &[(&str, &[u64])],
    ) -> YoloMethodologySnapshotV1 {
        let contracts = expirations
            .iter()
            .flat_map(|(expiration, strikes)| {
                strikes.iter().map(move |strike| {
                    quote(
                        format!("HOOD-{expiration}-{strike:03}C"),
                        *strike,
                        expiration,
                        observed,
                    )
                })
            })
            .collect();
        YoloMethodologySnapshotV1 {
            sequence,
            scheduled_at_utc: scheduled.to_string(),
            observed_at_utc: observed.to_string(),
            underlier_midpoint_microdollars: 50_000_000,
            underlier_halted: false,
            contracts,
        }
    }

    fn snapshots(expirations: &[(&str, &[u64])]) -> Vec<YoloMethodologySnapshotV1> {
        [
            ("2026-09-04T14:30:00Z", "2026-09-04T14:30:01Z"),
            ("2026-09-04T14:37:30Z", "2026-09-04T14:37:31Z"),
            ("2026-09-04T14:45:00Z", "2026-09-04T14:45:01Z"),
            ("2026-09-04T14:52:30Z", "2026-09-04T14:52:31Z"),
            ("2026-09-04T15:00:00Z", "2026-09-04T15:00:01Z"),
        ]
        .iter()
        .enumerate()
        .map(|(sequence, (scheduled, observed))| {
            snapshot(sequence as u32, scheduled, observed, expirations)
        })
        .collect()
    }

    fn input(expirations: &[(&str, &[u64])]) -> YoloPortfolioTargetInputV1 {
        YoloPortfolioTargetInputV1 {
            schema: YOLO_PORTFOLIO_TARGET_INPUT_SCHEMA_V1.to_string(),
            series_id: "YOLO-HOOD-v1".to_string(),
            underlier_id: "HOOD-COMMON".to_string(),
            trade_date: "2026-09-04".to_string(),
            methodology_sha256: "aa".repeat(32),
            collection_sha256: "bb".repeat(32),
            snapshots: snapshots(expirations),
            positions: Vec::new(),
            settled_cash_microdollars: 100_000_000_000,
            unsettled_cash_microdollars: 0,
            accrued_liabilities_microdollars: 0,
            incumbent_expiration: None,
            incumbent_symbols: Vec::new(),
        }
    }

    #[test]
    fn target_matches_python_golden_vector() {
        let target = create_yolo_portfolio_target_v1(
            &input(&[("2026-11-03", &[48, 49, 50, 51, 52])]),
            &parameters(),
        )
        .unwrap();

        assert_eq!(
            parameters().sha256().unwrap(),
            "9d00d2032b1e96a4ebff029f6d52e2ec0f1f9570a2ff92eb1c514e12fcbb37cc"
        );
        assert_eq!(
            target.sha256().unwrap(),
            "3aa0c89fb073c393a44c45061431f6cb53d8ab45161a2675818fe2cf98dd0cc9"
        );
        assert_eq!(target.status, YoloPortfolioTargetStatusV1::TargetComputed);
        assert_eq!(target.selected_expiration.as_deref(), Some("2026-11-03"));
        assert_eq!(target.positions.len(), 5);
        assert!(target
            .positions
            .iter()
            .all(|position| position.target_quantity == 36
                && position.target_delta == 36
                && position.liquidity_ppb == 500_000_000));
    }

    #[test]
    fn target_is_independent_of_input_iteration_order() {
        let mut shuffled = input(&[("2026-11-03", &[48, 49, 50, 51, 52])]);
        shuffled.snapshots.reverse();
        for snapshot in &mut shuffled.snapshots {
            snapshot.contracts.reverse();
        }
        let ordered = input(&[("2026-11-03", &[48, 49, 50, 51, 52])]);

        assert_eq!(
            create_yolo_portfolio_target_v1(&shuffled, &parameters())
                .unwrap()
                .sha256()
                .unwrap(),
            create_yolo_portfolio_target_v1(&ordered, &parameters())
                .unwrap()
                .sha256()
                .unwrap()
        );
    }

    #[test]
    fn incomplete_halted_and_zero_sleeve_states_match_python() {
        let mut incomplete = input(&[("2026-11-03", &[48, 49, 50, 51, 52])]);
        incomplete.snapshots.pop();
        let incomplete_target =
            create_yolo_portfolio_target_v1(&incomplete, &parameters()).unwrap();
        assert_eq!(
            incomplete_target.status,
            YoloPortfolioTargetStatusV1::NoReconstitution
        );
        assert_eq!(
            incomplete_target.reason,
            "collection does not contain the required snapshot sequence"
        );

        let mut halted = input(&[("2026-11-03", &[48, 49, 50, 51, 52])]);
        halted.snapshots[4].underlier_halted = true;
        assert_eq!(
            create_yolo_portfolio_target_v1(&halted, &parameters())
                .unwrap()
                .status,
            YoloPortfolioTargetStatusV1::HaltedInstrument
        );

        let mut zero_sleeve = input(&[("2026-11-03", &[48, 49, 50, 51, 52])]);
        zero_sleeve.settled_cash_microdollars = 1_000_000;
        let zero_target = create_yolo_portfolio_target_v1(&zero_sleeve, &parameters()).unwrap();
        assert_eq!(
            zero_target.status,
            YoloPortfolioTargetStatusV1::UnresolvedCapitalPolicy
        );
        assert!(zero_target
            .positions
            .iter()
            .all(|position| position.target_quantity == 0));
    }

    #[test]
    fn retained_expiry_and_zero_target_exit_match_python() {
        let incumbent_strikes = &[48, 49, 50, 51, 52];
        let successor_strikes = &[48, 49, 50, 51, 52];
        let mut retained = input(&[
            ("2026-10-18", incumbent_strikes),
            ("2026-11-03", successor_strikes),
        ]);
        retained.incumbent_expiration = Some("2026-10-18".to_string());
        retained.incumbent_symbols = incumbent_strikes
            .iter()
            .map(|strike| format!("HOOD-2026-10-18-{strike:03}C"))
            .collect();
        let retained_target = create_yolo_portfolio_target_v1(&retained, &parameters()).unwrap();
        assert_eq!(
            retained_target.selected_expiration.as_deref(),
            Some("2026-10-18")
        );
        assert!(retained_target
            .positions
            .iter()
            .all(|position| position.retained_incumbent));

        let mut exit = input(&[("2026-11-03", &[47, 48, 49, 50, 51, 52, 53])]);
        exit.positions = vec![YoloCurrentPositionV1 {
            occ_symbol: "HOOD-2026-11-03-047C".to_string(),
            quantity: 3,
        }];
        let exit_target = create_yolo_portfolio_target_v1(&exit, &parameters()).unwrap();
        let held = exit_target
            .positions
            .iter()
            .find(|position| position.occ_symbol == "HOOD-2026-11-03-047C")
            .unwrap();
        assert_eq!(held.target_quantity, 0);
        assert_eq!(held.target_delta, -3);
    }

    fn assert_target_vector(
        name: &str,
        input: &YoloPortfolioTargetInputV1,
        expected_status: YoloPortfolioTargetStatusV1,
        expected_expiration: Option<&str>,
        expected_sha256: &str,
    ) {
        let target = create_yolo_portfolio_target_v1(input, &parameters()).unwrap();
        assert_eq!(target.status, expected_status, "{name} status");
        assert_eq!(
            target.selected_expiration.as_deref(),
            expected_expiration,
            "{name} expiration"
        );
        assert_eq!(target.sha256().unwrap(), expected_sha256, "{name} hash");
    }

    #[test]
    fn boundary_and_non_target_vectors_match_python_hashes() {
        let strikes = &[48, 49, 50, 51, 52];
        for (days, expiration, expected_expiration, expected_hash) in [
            (
                30,
                "2026-10-04",
                "2026-11-03",
                "c6e6389b55602c7dbb35a117c8019889de7bd76012456b2772d69ecab6a4dd68",
            ),
            (
                31,
                "2026-10-05",
                "2026-10-05",
                "3e7522496fab841fd968de82d60b23936f4cfd7b8deb11a7d4b079cdcfd330b9",
            ),
            (
                44,
                "2026-10-18",
                "2026-10-18",
                "c38ced08289d3e23c5687dd8ba691bd4a3590809688240c4a6b99ffe2f971dfc",
            ),
        ] {
            let mut target_input = input(&[(expiration, strikes), ("2026-11-03", strikes)]);
            target_input.incumbent_expiration = Some(expiration.to_string());
            target_input.incumbent_symbols = strikes
                .iter()
                .map(|strike| format!("HOOD-{expiration}-{strike:03}C"))
                .collect();
            assert_target_vector(
                &format!("incumbent_{days}"),
                &target_input,
                YoloPortfolioTargetStatusV1::TargetComputed,
                Some(expected_expiration),
                expected_hash,
            );
        }

        assert_target_vector(
            "successor_45_75",
            &input(&[("2026-10-19", strikes), ("2026-11-18", strikes)]),
            YoloPortfolioTargetStatusV1::TargetComputed,
            Some("2026-10-19"),
            "ea9301964d0df3cc13d648cb0fcc133024e255c7e5a112c9a7714320158af3e9",
        );
        assert_target_vector(
            "successor_44_76",
            &input(&[("2026-10-18", strikes), ("2026-11-19", strikes)]),
            YoloPortfolioTargetStatusV1::NoEligibleSuccessor,
            None,
            "b3a1c3d3f0824505861817baae846cbfc039d2c051bffadfd1fa92dba618b64a",
        );

        let six_strikes = &[47, 48, 49, 50, 51, 52];
        let missing_symbol = "HOOD-2026-11-03-047C";
        let mut missing_final = input(&[("2026-11-03", six_strikes)]);
        missing_final.snapshots[4]
            .contracts
            .retain(|contract| contract.occ_symbol != missing_symbol);
        assert_target_vector(
            "missing_final_quote",
            &missing_final,
            YoloPortfolioTargetStatusV1::TargetComputed,
            Some("2026-11-03"),
            "32cfe352f1f047e538eaf69daed4a70ea1444fd21d684a7b2cb71b9553dcc9cb",
        );

        let mut adjusted = input(&[("2026-11-03", six_strikes)]);
        for snapshot in &mut adjusted.snapshots {
            for contract in &mut snapshot.contracts {
                if contract.occ_symbol == missing_symbol {
                    contract.standard = false;
                }
            }
        }
        assert_target_vector(
            "adjusted_contract",
            &adjusted,
            YoloPortfolioTargetStatusV1::TargetComputed,
            Some("2026-11-03"),
            "1b188715dc93259e666e6583b76eed924a37b592722b892b1e4b63632914ebc8",
        );

        let mut exact_thresholds = input(&[("2026-11-03", &[45, 47, 50, 53, 55])]);
        for snapshot in &mut exact_thresholds.snapshots {
            for contract in &mut snapshot.contracts {
                contract.bid_microdollars = 4_800_000;
                contract.ask_microdollars = 5_200_000;
                contract.bid_size = 1;
                contract.ask_size = 1;
                contract.session_volume = 100;
                contract.open_interest = 500;
            }
        }
        assert_target_vector(
            "exact_thresholds",
            &exact_thresholds,
            YoloPortfolioTargetStatusV1::TargetComputed,
            Some("2026-11-03"),
            "60150aaeb429facd8086a2f2c98242c9a9feccc0d126d2e7818caac1d6fd94b6",
        );

        let mut zero_sleeve = input(&[("2026-11-03", strikes)]);
        zero_sleeve.settled_cash_microdollars = 1_000_000;
        assert_target_vector(
            "zero_sleeve",
            &zero_sleeve,
            YoloPortfolioTargetStatusV1::UnresolvedCapitalPolicy,
            Some("2026-11-03"),
            "9ac08a4e368160c3c33d8609658533c4ed1fc1575949d0c45d4480609236fc97",
        );

        let mut exit = input(&[("2026-11-03", &[47, 48, 49, 50, 51, 52, 53])]);
        exit.positions = vec![YoloCurrentPositionV1 {
            occ_symbol: missing_symbol.to_string(),
            quantity: 3,
        }];
        assert_target_vector(
            "zero_exit",
            &exit,
            YoloPortfolioTargetStatusV1::TargetComputed,
            Some("2026-11-03"),
            "bdc51bf806b5534159d03e18007af2aab2de2b8dbc8a17603d07d2c265a70edd",
        );

        let mut halted = input(&[("2026-11-03", strikes)]);
        halted.snapshots[4].underlier_halted = true;
        assert_target_vector(
            "underlier_halt",
            &halted,
            YoloPortfolioTargetStatusV1::HaltedInstrument,
            None,
            "4232d4800d36a502382a4850bf0a395d31f96937589004c937dfeca7547b7f6a",
        );

        let mut incomplete = input(&[("2026-11-03", strikes)]);
        incomplete.snapshots.pop();
        assert_target_vector(
            "incomplete",
            &incomplete,
            YoloPortfolioTargetStatusV1::NoReconstitution,
            None,
            "16b67ed2ff1a32faba0068d73293363d1b00c1e780a693bcb2de02e039a5491c",
        );

        let mut missing_mark = input(&[("2026-11-03", strikes)]);
        missing_mark.positions = vec![YoloCurrentPositionV1 {
            occ_symbol: "HOOD-MISSING".to_string(),
            quantity: 1,
        }];
        assert_target_vector(
            "missing_mark",
            &missing_mark,
            YoloPortfolioTargetStatusV1::NoReconstitution,
            Some("2026-11-03"),
            "8f026c05e836521ca90214eb841db364cf8654acd350d1583cd7ebcd34aa3690",
        );
    }

    #[test]
    fn quote_age_and_schedule_tolerance_boundaries_are_inclusive() {
        let mut exact = input(&[("2026-11-03", &[48, 49, 50, 51, 52])]);
        let exact_times = [
            ("2026-09-04T14:30:30Z", "2026-09-04T14:28:30Z"),
            ("2026-09-04T14:38:00Z", "2026-09-04T14:36:00Z"),
            ("2026-09-04T14:45:30Z", "2026-09-04T14:43:30Z"),
            ("2026-09-04T14:53:00Z", "2026-09-04T14:51:00Z"),
            ("2026-09-04T15:00:30Z", "2026-09-04T14:58:30Z"),
        ];
        for (snapshot, (observed, quote_time)) in exact.snapshots.iter_mut().zip(exact_times) {
            snapshot.observed_at_utc = observed.to_string();
            for contract in &mut snapshot.contracts {
                contract.quote_timestamp_utc = quote_time.to_string();
            }
        }
        assert_eq!(
            create_yolo_portfolio_target_v1(&exact, &parameters())
                .unwrap()
                .status,
            YoloPortfolioTargetStatusV1::TargetComputed
        );

        let mut late = exact.clone();
        late.snapshots[0].observed_at_utc = "2026-09-04T14:30:31Z".to_string();
        assert_eq!(
            create_yolo_portfolio_target_v1(&late, &parameters())
                .unwrap()
                .status,
            YoloPortfolioTargetStatusV1::NoReconstitution
        );

        let mut stale = input(&[("2026-11-03", &[48, 49, 50, 51, 52])]);
        let stale_times = [
            "2026-09-04T14:28:00Z",
            "2026-09-04T14:35:30Z",
            "2026-09-04T14:43:00Z",
            "2026-09-04T14:50:30Z",
            "2026-09-04T14:58:00Z",
        ];
        for (snapshot, quote_time) in stale.snapshots.iter_mut().zip(stale_times) {
            for contract in &mut snapshot.contracts {
                contract.quote_timestamp_utc = quote_time.to_string();
            }
        }
        assert_eq!(
            create_yolo_portfolio_target_v1(&stale, &parameters())
                .unwrap()
                .status,
            YoloPortfolioTargetStatusV1::NoEligibleSuccessor
        );
    }
}
