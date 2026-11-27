/**
 * Exact-atom deterministic Pillar 7 NAV & fee engine (BigInt math).
 * Byte-identical to python/postfiat_rpc/navcoin_sdk.py and L1 Upgrade Train B.
 */

import {
  ERR_DISCLOSURE_REF_TOO_LONG,
  ERR_FX_FIX_FEE_EXCEEDS_NAV_CAP,
  ERR_INSUFFICIENT_UNCLAIMED,
  ERR_INVALID_ACCRUAL_CAP,
  ERR_INVALID_CLAIM_AMOUNT,
  ERR_ISSUER_SELF_ATTESTATION_FORBIDDEN,
  ERR_NAV_PER_UNIT_FORMULA_MISMATCH,
  ERR_NAV_SUPPLY_SNAPSHOT_STALE,
  ERR_NO_PENDING_SCHEDULE,
  ERR_NOTICE_TOO_SHORT,
  ERR_PENDING_INCREASE_EXIT_CAPACITY_LOCKED,
  ERR_RATE_CAP_EXCEEDED,
  ERR_RECIPIENT_EQUALS_ISSUER,
  ERR_SCHEDULE_ALREADY_ATTACHED,
  ERR_SCHEDULE_MISSING,
  ERR_SCHEDULE_RATES_LOCKED,
  ERR_UNAUTHORIZED_RECIPIENT,
  MAX_PRECISION,
  NAV_FEE_BPS_DENOMINATOR,
  NAV_FEE_CLAIM_TRANSACTION_KIND,
  NAV_FEE_MAX_ANNUAL_RATE_BPS,
  NAV_FEE_MAX_DISCLOSURE_REF_BYTES,
  NAV_FEE_MAX_FIX_FEE_CAP_BPS,
  NAV_FEE_MAX_HWM_RATE_BPS,
  NAV_FEE_MAX_SPREAD_BPS,
  NAV_FEE_MIN_INCREASE_NOTICE_BLOCKS,
  NAV_FEE_SCHEDULE_CANCEL_TRANSACTION_KIND,
  NAV_FEE_SCHEDULE_SET_TRANSACTION_KIND,
  NavFeeError,
  PROTOCOL_BLOCKS_PER_DAY,
  PROTOCOL_BLOCKS_PER_YEAR,
  REASON_CODES,
  U64_MAX,
} from "./constants.mjs";
import { normalizeAccount, parseExactUint, parseU32Number, parseU64 } from "./canonical.mjs";

function parsePrecision(precision) {
  return Number(parseExactUint(precision, "precision", BigInt(MAX_PRECISION)));
}

export function unitScale(precision) {
  return 10n ** BigInt(parsePrecision(precision));
}

function minBigInt(a, b) {
  return a < b ? a : b;
}

function maxBigInt(a, b) {
  return a > b ? a : b;
}

export function normalizeScheduleInput(raw) {
  return {
    annual_rate_bps: parseU32Number(raw.annual_rate_bps, "annual_rate_bps"),
    hwm_rate_bps: parseU32Number(raw.hwm_rate_bps, "hwm_rate_bps"),
    issue_spread_bps: parseU32Number(raw.issue_spread_bps, "issue_spread_bps"),
    redeem_spread_bps: parseU32Number(raw.redeem_spread_bps, "redeem_spread_bps"),
    fix_fee_cap_bps: parseU32Number(raw.fix_fee_cap_bps, "fix_fee_cap_bps"),
    recipient: String(raw.recipient),
    increase_notice_blocks: parseU64(
      raw.increase_notice_blocks ?? NAV_FEE_MIN_INCREASE_NOTICE_BLOCKS,
      "increase_notice_blocks",
    ),
    accrual_cap_blocks: parseU64(
      raw.accrual_cap_blocks ?? PROTOCOL_BLOCKS_PER_DAY,
      "accrual_cap_blocks",
    ),
    rates_locked: Boolean(raw.rates_locked ?? false),
    disclosure_ref: raw.disclosure_ref ?? null,
  };
}

export function normalizeSchedule(raw) {
  return {
    annual_rate_bps: parseU32Number(raw.annual_rate_bps, "annual_rate_bps"),
    hwm_rate_bps: parseU32Number(raw.hwm_rate_bps, "hwm_rate_bps"),
    issue_spread_bps: parseU32Number(raw.issue_spread_bps, "issue_spread_bps"),
    redeem_spread_bps: parseU32Number(raw.redeem_spread_bps, "redeem_spread_bps"),
    fix_fee_cap_bps: parseU32Number(raw.fix_fee_cap_bps, "fix_fee_cap_bps"),
    recipient: String(raw.recipient),
    increase_notice_blocks: parseU64(raw.increase_notice_blocks, "increase_notice_blocks"),
    accrual_cap_blocks: parseU64(raw.accrual_cap_blocks, "accrual_cap_blocks"),
    rates_locked: Boolean(raw.rates_locked ?? false),
    disclosure_ref: raw.disclosure_ref ?? null,
    attached_at_height: parseU64(raw.attached_at_height ?? 0n, "attached_at_height"),
  };
}

export function normalizePendingSchedule(raw) {
  if (raw == null) return null;
  return {
    schedule: normalizeSchedule(raw.schedule),
    proposed_at_height: parseU64(raw.proposed_at_height, "proposed_at_height"),
    effective_at_height: parseU64(raw.effective_at_height, "effective_at_height"),
    is_increase: Boolean(raw.is_increase),
  };
}

export function normalizeFeeState(raw) {
  if (raw == null) return null;
  return {
    active: normalizeSchedule(raw.active),
    pending: normalizePendingSchedule(raw.pending),
    high_water_mark_nav_per_unit: parseU64(
      raw.high_water_mark_nav_per_unit,
      "high_water_mark_nav_per_unit",
    ),
    last_accrual_height: parseU64(raw.last_accrual_height, "last_accrual_height"),
    unclaimed_fee_units: parseU64(raw.unclaimed_fee_units ?? 0n, "unclaimed_fee_units"),
    total_accrued_fee_units: parseU64(
      raw.total_accrued_fee_units ?? 0n,
      "total_accrued_fee_units",
    ),
    total_claimed_fee_units: parseU64(
      raw.total_claimed_fee_units ?? 0n,
      "total_claimed_fee_units",
    ),
  };
}

export function normalizeAccrual(raw) {
  const reason = String(raw.reason_code);
  if (!REASON_CODES.has(reason)) {
    throw new Error(`unknown reason_code: ${reason}`);
  }
  return {
    asset_id: String(raw.asset_id),
    epoch: parseU64(raw.epoch, "epoch"),
    block_height: parseU64(raw.block_height, "block_height"),
    blocks_per_year: parseU64(raw.blocks_per_year, "blocks_per_year"),
    delta_blocks: parseU64(raw.delta_blocks, "delta_blocks"),
    verified_net_assets: parseU64(raw.verified_net_assets, "verified_net_assets"),
    supply_before: parseU64(raw.supply_before, "supply_before"),
    nav_before: parseU64(raw.nav_before, "nav_before"),
    hwm_before: parseU64(raw.hwm_before, "hwm_before"),
    annual_rate_bps_applied: parseU32Number(
      raw.annual_rate_bps_applied,
      "annual_rate_bps_applied",
    ),
    hwm_rate_bps_applied: parseU32Number(raw.hwm_rate_bps_applied, "hwm_rate_bps_applied"),
    time_fee_value: parseU64(raw.time_fee_value, "time_fee_value"),
    nav_after_time_fee: parseU64(raw.nav_after_time_fee, "nav_after_time_fee"),
    hwm_fee_value: parseU64(raw.hwm_fee_value, "hwm_fee_value"),
    total_fee_value: parseU64(raw.total_fee_value, "total_fee_value"),
    minted_fee_units: parseU64(raw.minted_fee_units, "minted_fee_units"),
    clipped_fee_units: parseU64(raw.clipped_fee_units, "clipped_fee_units"),
    supply_after: parseU64(raw.supply_after, "supply_after"),
    nav_after: parseU64(raw.nav_after, "nav_after"),
    hwm_after: parseU64(raw.hwm_after, "hwm_after"),
    reason_code: reason,
  };
}

export function formatAccrualVectorDict(accrual) {
  return {
    asset_id: accrual.asset_id,
    epoch: Number(accrual.epoch),
    block_height: accrual.block_height.toString(10),
    blocks_per_year: accrual.blocks_per_year.toString(10),
    delta_blocks: accrual.delta_blocks.toString(10),
    verified_net_assets: accrual.verified_net_assets.toString(10),
    supply_before: accrual.supply_before.toString(10),
    nav_before: accrual.nav_before.toString(10),
    hwm_before: accrual.hwm_before.toString(10),
    annual_rate_bps_applied: accrual.annual_rate_bps_applied,
    hwm_rate_bps_applied: accrual.hwm_rate_bps_applied,
    time_fee_value: accrual.time_fee_value.toString(10),
    nav_after_time_fee: accrual.nav_after_time_fee.toString(10),
    hwm_fee_value: accrual.hwm_fee_value.toString(10),
    total_fee_value: accrual.total_fee_value.toString(10),
    minted_fee_units: accrual.minted_fee_units.toString(10),
    clipped_fee_units: accrual.clipped_fee_units.toString(10),
    supply_after: accrual.supply_after.toString(10),
    nav_after: accrual.nav_after.toString(10),
    hwm_after: accrual.hwm_after.toString(10),
    reason_code: accrual.reason_code,
  };
}

// ---------------------------------------------------------------------------
// Universal floor(V * U / S) NAV check (ALL NAVCoins)
// ---------------------------------------------------------------------------

export function computeNavPerUnit(verifiedNetAssets, circulatingSupply, precision) {
  const v = parseU64(verifiedNetAssets, "verified_net_assets");
  const s = parseU64(circulatingSupply, "circulating_supply");
  const u = unitScale(precision);
  if (s === 0n) return 0n;
  return (v * u) / s;
}

export function verifyNavPerUnit({
  verifiedNetAssets,
  circulatingSupply,
  precision,
  claimedNavPerUnit,
  liveLedgerSupply = null,
}) {
  const v = parseU64(verifiedNetAssets, "verified_net_assets");
  const s = parseU64(circulatingSupply, "circulating_supply");
  const claimed = parseU64(claimedNavPerUnit, "claimed_nav_per_unit");
  const prec = parsePrecision(precision);
  const u = 10n ** BigInt(prec);

  if (liveLedgerSupply !== null && liveLedgerSupply !== undefined) {
    const liveS = parseU64(liveLedgerSupply, "live_ledger_supply");
    if (s !== liveS) {
      throw new NavFeeError(
        ERR_NAV_SUPPLY_SNAPSHOT_STALE,
        `packet circulating_supply (${s}) != live_ledger_supply (${liveS})`,
      );
    }
  }

  const expected = s === 0n ? 0n : (v * u) / s;
  if (s > 0n && claimed !== expected) {
    throw new NavFeeError(
      ERR_NAV_PER_UNIT_FORMULA_MISMATCH,
      `claimed nav_per_unit (${claimed}) != floor(V*U/S) (${expected})`,
    );
  }

  const remainder = s === 0n ? 0n : v * u - expected * s;
  return {
    valid: true,
    verified_net_assets: v,
    circulating_supply: s,
    precision: prec,
    unit_scale: u,
    expected_nav_per_unit: expected,
    claimed_nav_per_unit: claimed,
    floor_remainder_scaled: remainder,
  };
}

// ---------------------------------------------------------------------------
// Schedule validation & lifecycle
// ---------------------------------------------------------------------------

export function validateFeeScheduleInput(schedule, { issuer, maxEpochGapBlocks = null }) {
  const inp = normalizeScheduleInput(schedule);
  const canonicalIssuer = normalizeAccount(issuer, "issuer");
  const canonicalRecipient = normalizeAccount(inp.recipient, "recipient");

  if (canonicalRecipient === canonicalIssuer) {
    throw new NavFeeError(
      ERR_RECIPIENT_EQUALS_ISSUER,
      "fee recipient account must not equal issuer account",
    );
  }

  if (
    inp.annual_rate_bps > NAV_FEE_MAX_ANNUAL_RATE_BPS ||
    inp.hwm_rate_bps > NAV_FEE_MAX_HWM_RATE_BPS ||
    inp.issue_spread_bps > NAV_FEE_MAX_SPREAD_BPS ||
    inp.redeem_spread_bps > NAV_FEE_MAX_SPREAD_BPS ||
    inp.fix_fee_cap_bps > NAV_FEE_MAX_FIX_FEE_CAP_BPS
  ) {
    throw new NavFeeError(
      ERR_RATE_CAP_EXCEEDED,
      "fee schedule rate or spread exceeds protocol cap",
    );
  }

  if (inp.increase_notice_blocks < NAV_FEE_MIN_INCREASE_NOTICE_BLOCKS) {
    throw new NavFeeError(
      ERR_NOTICE_TOO_SHORT,
      `increase_notice_blocks (${inp.increase_notice_blocks}) < minimum (${NAV_FEE_MIN_INCREASE_NOTICE_BLOCKS})`,
    );
  }

  if (inp.accrual_cap_blocks === 0n) {
    throw new NavFeeError(ERR_INVALID_ACCRUAL_CAP, "accrual_cap_blocks must be > 0");
  }

  if (maxEpochGapBlocks !== null && maxEpochGapBlocks !== undefined) {
    const gapCap = parseU64(maxEpochGapBlocks, "max_epoch_gap_blocks");
    if (gapCap > 0n && inp.accrual_cap_blocks > gapCap) {
      throw new NavFeeError(
        ERR_INVALID_ACCRUAL_CAP,
        `accrual_cap_blocks (${inp.accrual_cap_blocks}) exceeds max_epoch_gap_blocks (${gapCap})`,
      );
    }
  }

  if (inp.disclosure_ref !== null) {
    if (typeof inp.disclosure_ref !== "string") {
      throw new TypeError("disclosure_ref must be a string when provided");
    }
    if (Buffer.byteLength(inp.disclosure_ref, "utf8") > NAV_FEE_MAX_DISCLOSURE_REF_BYTES) {
      throw new NavFeeError(
        ERR_DISCLOSURE_REF_TOO_LONG,
        `disclosure_ref exceeds ${NAV_FEE_MAX_DISCLOSURE_REF_BYTES} UTF-8 bytes`,
      );
    }
  }

  return {
    ...inp,
    recipient: canonicalRecipient,
  };
}

export function isScheduleIncrease(current, proposed) {
  return (
    proposed.annual_rate_bps > current.annual_rate_bps ||
    proposed.hwm_rate_bps > current.hwm_rate_bps ||
    proposed.issue_spread_bps > current.issue_spread_bps ||
    proposed.redeem_spread_bps > current.redeem_spread_bps ||
    proposed.fix_fee_cap_bps > current.fix_fee_cap_bps ||
    proposed.accrual_cap_blocks > current.accrual_cap_blocks ||
    proposed.increase_notice_blocks < current.increase_notice_blocks
  );
}

export function attachFeeSchedule({
  existingFeeState = null,
  issuer,
  scheduleInput,
  blockHeight,
  currentNavPerUnit = 0n,
  maxEpochGapBlocks = null,
}) {
  if (existingFeeState != null) {
    throw new NavFeeError(
      ERR_SCHEDULE_ALREADY_ATTACHED,
      "asset already has an attached fee schedule",
    );
  }
  const hAttach = parseU64(blockHeight, "block_height");
  const navNow = parseU64(currentNavPerUnit, "current_nav_per_unit");
  const validated = validateFeeScheduleInput(scheduleInput, { issuer, maxEpochGapBlocks });

  return {
    active: {
      ...validated,
      attached_at_height: hAttach,
    },
    pending: null,
    high_water_mark_nav_per_unit: navNow,
    last_accrual_height: hAttach,
    unclaimed_fee_units: 0n,
    total_accrued_fee_units: 0n,
    total_claimed_fee_units: 0n,
  };
}

export function proposeFeeScheduleUpdate({
  feeState,
  issuer,
  scheduleInput,
  blockHeight,
  maxEpochGapBlocks = null,
  recipientCosigned = false,
}) {
  if (feeState == null) {
    throw new NavFeeError(ERR_SCHEDULE_MISSING, "no fee schedule attached to asset");
  }
  const state = normalizeFeeState(feeState);
  const hNow = parseU64(blockHeight, "block_height");
  const validated = validateFeeScheduleInput(scheduleInput, { issuer, maxEpochGapBlocks });
  const active = state.active;
  const increase = isScheduleIncrease(active, validated);

  if (active.rates_locked && (!validated.rates_locked || increase)) {
    throw new NavFeeError(
      ERR_SCHEDULE_RATES_LOCKED,
      "fee schedule rates are permanently locked against increases or unlocking",
    );
  }

  let effectiveAtHeight;
  if (increase) {
    const notice = maxBigInt(active.increase_notice_blocks, NAV_FEE_MIN_INCREASE_NOTICE_BLOCKS);
    effectiveAtHeight = hNow + notice;
  } else if (active.rates_locked && validated.recipient !== active.recipient && !recipientCosigned) {
    effectiveAtHeight = hNow + NAV_FEE_MIN_INCREASE_NOTICE_BLOCKS;
  } else {
    effectiveAtHeight = hNow;
  }

  if (effectiveAtHeight > U64_MAX) {
    throw new RangeError("effective_at_height exceeds u64::MAX");
  }

  return {
    ...state,
    pending: {
      schedule: {
        ...validated,
        attached_at_height: active.attached_at_height,
      },
      proposed_at_height: hNow,
      effective_at_height: effectiveAtHeight,
      is_increase: increase,
    },
  };
}

export function cancelPendingFeeSchedule(feeState) {
  if (feeState == null) {
    throw new NavFeeError(ERR_SCHEDULE_MISSING, "no fee schedule attached to asset");
  }
  const state = normalizeFeeState(feeState);
  if (state.pending == null) {
    throw new NavFeeError(ERR_NO_PENDING_SCHEDULE, "no pending fee schedule to cancel");
  }
  return {
    ...state,
    pending: null,
  };
}

export function claimAccruedFees({ feeState, caller, amount }) {
  if (feeState == null) {
    throw new NavFeeError(ERR_SCHEDULE_MISSING, "no fee schedule attached to asset");
  }
  const state = normalizeFeeState(feeState);
  const canonicalCaller = normalizeAccount(caller, "caller");
  if (canonicalCaller !== state.active.recipient) {
    throw new NavFeeError(
      ERR_UNAUTHORIZED_RECIPIENT,
      `caller ${canonicalCaller} does not match active fee recipient ${state.active.recipient}`,
    );
  }
  const claimAmount = parseU64(amount, "amount");
  if (claimAmount === 0n) {
    throw new NavFeeError(ERR_INVALID_CLAIM_AMOUNT, "claim amount must be > 0");
  }
  if (claimAmount > state.unclaimed_fee_units) {
    throw new NavFeeError(
      ERR_INSUFFICIENT_UNCLAIMED,
      `claim amount (${claimAmount}) exceeds unclaimed_fee_units (${state.unclaimed_fee_units})`,
    );
  }
  const nextState = {
    ...state,
    unclaimed_fee_units: state.unclaimed_fee_units - claimAmount,
    total_claimed_fee_units: state.total_claimed_fee_units + claimAmount,
  };
  return {
    feeState: nextState,
    receipt: {
      recipient: canonicalCaller,
      claimed_amount: claimAmount,
      unclaimed_remaining: nextState.unclaimed_fee_units,
      total_claimed_fee_units: nextState.total_claimed_fee_units,
    },
  };
}

// ---------------------------------------------------------------------------
// Anti-bypass guards
// ---------------------------------------------------------------------------

export function checkExitCapacityUpdate(feeState, currentRedeemCapacityAtoms, newRedeemCapacityAtoms) {
  const cur = parseU64(currentRedeemCapacityAtoms, "current_redeem_capacity_atoms");
  const nxt = parseU64(newRedeemCapacityAtoms, "new_redeem_capacity_atoms");
  const state = normalizeFeeState(feeState);
  if (state != null && state.pending != null && state.pending.is_increase && nxt < cur) {
    throw new NavFeeError(
      ERR_PENDING_INCREASE_EXIT_CAPACITY_LOCKED,
      "cannot reduce redeem exit capacity while a fee increase is pending",
    );
  }
  return true;
}

export function checkUnscheduledSpreadUpdate({
  currentIssueSpreadBps,
  currentRedeemSpreadBps,
  newIssueSpreadBps,
  newRedeemSpreadBps,
  proposedAtHeight,
  effectiveAtHeight,
}) {
  const curIssue = parseU32Number(currentIssueSpreadBps, "current_issue_spread_bps");
  const curRedeem = parseU32Number(currentRedeemSpreadBps, "current_redeem_spread_bps");
  const nxtIssue = parseU32Number(newIssueSpreadBps, "new_issue_spread_bps");
  const nxtRedeem = parseU32Number(newRedeemSpreadBps, "new_redeem_spread_bps");
  const hProp = parseU64(proposedAtHeight, "proposed_at_height");
  const hEff = parseU64(effectiveAtHeight, "effective_at_height");

  if (nxtIssue > NAV_FEE_MAX_SPREAD_BPS || nxtRedeem > NAV_FEE_MAX_SPREAD_BPS) {
    throw new NavFeeError(ERR_RATE_CAP_EXCEEDED, "spread exceeds 500 bps protocol cap");
  }
  const isHike = nxtIssue > curIssue || nxtRedeem > curRedeem;
  if (isHike && hEff < hProp + NAV_FEE_MIN_INCREASE_NOTICE_BLOCKS) {
    throw new NavFeeError(
      ERR_NOTICE_TOO_SHORT,
      "spread increase on unscheduled NAVCoin requires at least 604,800 blocks notice",
    );
  }
  return true;
}

export function checkFxFixFeeCap(feeBps, feeState = null) {
  const bps = parseU32Number(feeBps, "fee_bps");
  if (bps > NAV_FEE_MAX_FIX_FEE_CAP_BPS) {
    throw new NavFeeError(
      ERR_FX_FIX_FEE_EXCEEDS_NAV_CAP,
      `FX fix fee_bps (${bps}) exceeds protocol cap (${NAV_FEE_MAX_FIX_FEE_CAP_BPS})`,
    );
  }
  const state = normalizeFeeState(feeState);
  if (state != null && bps > state.active.fix_fee_cap_bps) {
    throw new NavFeeError(
      ERR_FX_FIX_FEE_EXCEEDS_NAV_CAP,
      `FX fix fee_bps (${bps}) exceeds asset fix_fee_cap_bps (${state.active.fix_fee_cap_bps})`,
    );
  }
  return true;
}

export function checkNonIssuerAttestation({ issuer, feeState = null, attestedValue, attestations }) {
  const attVal = parseU64(attestedValue, "attested_value");
  if (feeState == null || attVal === 0n) {
    return true;
  }
  const canonicalIssuer = normalizeAccount(issuer, "issuer");
  for (const item of attestations || []) {
    const passed = Boolean(item.pass ?? item.passed ?? false);
    if (!passed) continue;
    if (typeof item.attestor !== "string") continue;
    let attestor;
    try {
      attestor = normalizeAccount(item.attestor, "attestor");
    } catch {
      continue;
    }
    if (attestor !== canonicalIssuer) {
      return true;
    }
  }
  throw new NavFeeError(
    ERR_ISSUER_SELF_ATTESTATION_FORBIDDEN,
    "fee-enabled NAVCoin with attested_value > 0 requires a passing non-issuer attestor",
  );
}

// ---------------------------------------------------------------------------
// Exact 4-step deterministic epoch accrual
// ---------------------------------------------------------------------------

export function accrueEpoch({
  assetId,
  epoch,
  blockHeight,
  verifiedNetAssets,
  supplyBefore,
  precision,
  feeState,
  maxSupply = null,
  blocksPerYear = PROTOCOL_BLOCKS_PER_YEAR,
}) {
  const state = normalizeFeeState(feeState);
  const epochInt = parseU64(epoch, "epoch");
  const hNow = parseU64(blockHeight, "block_height");
  const v = parseU64(verifiedNetAssets, "verified_net_assets");
  const s = parseU64(supplyBefore, "supply_before");
  const u = unitScale(precision);
  const y = parseU64(blocksPerYear, "blocks_per_year");
  if (y === 0n) throw new RangeError("blocks_per_year must be > 0");
  const capSupply = maxSupply !== null && maxSupply !== undefined ? parseU64(maxSupply, "max_supply") : null;

  const oldSched = state.active;
  const pending = state.pending;

  let m;
  let p;
  let accrualCap;
  let nextActive;
  let nextPending;

  if (pending != null && hNow >= pending.effective_at_height) {
    const newSched = pending.schedule;
    m = Math.min(oldSched.annual_rate_bps, newSched.annual_rate_bps);
    p = Math.min(oldSched.hwm_rate_bps, newSched.hwm_rate_bps);
    accrualCap = minBigInt(oldSched.accrual_cap_blocks, newSched.accrual_cap_blocks);
    nextActive = newSched;
    nextPending = null;
  } else {
    m = oldSched.annual_rate_bps;
    p = oldSched.hwm_rate_bps;
    accrualCap = oldSched.accrual_cap_blocks;
    nextActive = oldSched;
    nextPending = pending;
  }

  const elapsed = hNow > state.last_accrual_height ? hNow - state.last_accrual_height : 0n;
  const deltaBlocks = minBigInt(elapsed, accrualCap);

  // Edge fail-safe 1: S == 0
  if (s === 0n) {
    const accrual = {
      asset_id: assetId,
      epoch: epochInt,
      block_height: hNow,
      blocks_per_year: y,
      delta_blocks: deltaBlocks,
      verified_net_assets: v,
      supply_before: 0n,
      nav_before: 0n,
      hwm_before: state.high_water_mark_nav_per_unit,
      annual_rate_bps_applied: m,
      hwm_rate_bps_applied: p,
      time_fee_value: 0n,
      nav_after_time_fee: 0n,
      hwm_fee_value: 0n,
      total_fee_value: 0n,
      minted_fee_units: 0n,
      clipped_fee_units: 0n,
      supply_after: 0n,
      nav_after: 0n,
      hwm_after: state.high_water_mark_nav_per_unit,
      reason_code: "zero_supply",
    };
    return {
      feeState: {
        ...state,
        active: nextActive,
        pending: nextPending,
        last_accrual_height: hNow,
      },
      accrual,
    };
  }

  // Edge fail-safe 2: V == 0
  if (v === 0n) {
    const accrual = {
      asset_id: assetId,
      epoch: epochInt,
      block_height: hNow,
      blocks_per_year: y,
      delta_blocks: deltaBlocks,
      verified_net_assets: 0n,
      supply_before: s,
      nav_before: 0n,
      hwm_before: state.high_water_mark_nav_per_unit,
      annual_rate_bps_applied: m,
      hwm_rate_bps_applied: p,
      time_fee_value: 0n,
      nav_after_time_fee: 0n,
      hwm_fee_value: 0n,
      total_fee_value: 0n,
      minted_fee_units: 0n,
      clipped_fee_units: 0n,
      supply_after: s,
      nav_after: 0n,
      hwm_after: state.high_water_mark_nav_per_unit,
      reason_code: "zero_net_assets",
    };
    return {
      feeState: {
        ...state,
        active: nextActive,
        pending: nextPending,
        last_accrual_height: hNow,
      },
      accrual,
    };
  }

  const navBefore = (v * u) / s;
  const hwmBefore =
    state.high_water_mark_nav_per_unit === 0n ? navBefore : state.high_water_mark_nav_per_unit;

  // Edge fail-safe 3: zero applied rates
  if (m === 0 && p === 0) {
    const hwmAfter = maxBigInt(hwmBefore, navBefore);
    const accrual = {
      asset_id: assetId,
      epoch: epochInt,
      block_height: hNow,
      blocks_per_year: y,
      delta_blocks: deltaBlocks,
      verified_net_assets: v,
      supply_before: s,
      nav_before: navBefore,
      hwm_before: hwmBefore,
      annual_rate_bps_applied: 0,
      hwm_rate_bps_applied: 0,
      time_fee_value: 0n,
      nav_after_time_fee: navBefore,
      hwm_fee_value: 0n,
      total_fee_value: 0n,
      minted_fee_units: 0n,
      clipped_fee_units: 0n,
      supply_after: s,
      nav_after: navBefore,
      hwm_after: hwmAfter,
      reason_code: "zero_rates",
    };
    return {
      feeState: {
        ...state,
        active: nextActive,
        pending: nextPending,
        high_water_mark_nav_per_unit: hwmAfter,
        last_accrual_height: hNow,
      },
      accrual,
    };
  }

  const mBig = BigInt(m);
  const pBig = BigInt(p);

  // Step 1: Time-based fee
  const fM = (v * mBig * deltaBlocks) / (NAV_FEE_BPS_DENOMINATOR * y);
  if (fM >= v) {
    const hwmAfter = maxBigInt(hwmBefore, navBefore);
    const accrual = {
      asset_id: assetId,
      epoch: epochInt,
      block_height: hNow,
      blocks_per_year: y,
      delta_blocks: deltaBlocks,
      verified_net_assets: v,
      supply_before: s,
      nav_before: navBefore,
      hwm_before: hwmBefore,
      annual_rate_bps_applied: m,
      hwm_rate_bps_applied: p,
      time_fee_value: 0n,
      nav_after_time_fee: navBefore,
      hwm_fee_value: 0n,
      total_fee_value: 0n,
      minted_fee_units: 0n,
      clipped_fee_units: 0n,
      supply_after: s,
      nav_after: navBefore,
      hwm_after: hwmAfter,
      reason_code: "clamped_full_value",
    };
    return {
      feeState: {
        ...state,
        active: nextActive,
        pending: nextPending,
        high_water_mark_nav_per_unit: hwmAfter,
        last_accrual_height: hNow,
      },
      accrual,
    };
  }

  const navAfterTimeFee = ((v - fM) * u) / s;

  // Step 2: High-water-mark gain fee
  const gain = navAfterTimeFee > hwmBefore ? navAfterTimeFee - hwmBefore : 0n;
  const fP = (gain * s * pBig) / (NAV_FEE_BPS_DENOMINATOR * u);

  // Step 3: Atomic unit mint
  const fTotal = fM + fP;
  if (fTotal >= v) {
    const hwmAfter = maxBigInt(hwmBefore, navBefore);
    const accrual = {
      asset_id: assetId,
      epoch: epochInt,
      block_height: hNow,
      blocks_per_year: y,
      delta_blocks: deltaBlocks,
      verified_net_assets: v,
      supply_before: s,
      nav_before: navBefore,
      hwm_before: hwmBefore,
      annual_rate_bps_applied: m,
      hwm_rate_bps_applied: p,
      time_fee_value: 0n,
      nav_after_time_fee: navBefore,
      hwm_fee_value: 0n,
      total_fee_value: 0n,
      minted_fee_units: 0n,
      clipped_fee_units: 0n,
      supply_after: s,
      nav_after: navBefore,
      hwm_after: hwmAfter,
      reason_code: "clamped_full_value",
    };
    return {
      feeState: {
        ...state,
        active: nextActive,
        pending: nextPending,
        high_water_mark_nav_per_unit: hwmAfter,
        last_accrual_height: hNow,
      },
      accrual,
    };
  }

  const deltaSRaw = (fTotal * s) / (v - fTotal);
  const effectiveSupplyCap = capSupply !== null ? minBigInt(capSupply, U64_MAX) : U64_MAX;

  let minted;
  let clipped;
  let reasonCode;

  if (s + deltaSRaw > effectiveSupplyCap) {
    minted = effectiveSupplyCap > s ? effectiveSupplyCap - s : 0n;
    clipped = deltaSRaw - minted;
    reasonCode = "clamped_max_supply";
  } else {
    minted = deltaSRaw;
    clipped = 0n;
    reasonCode = deltaBlocks === 0n && fTotal === 0n ? "zero_delta_blocks" : "accrued";
  }

  // Step 4: State update
  const sAfter = s + minted;
  const navAfter = (v * u) / sAfter;
  const hwmAfter = maxBigInt(hwmBefore, navAfter);

  const accrual = {
    asset_id: assetId,
    epoch: epochInt,
    block_height: hNow,
    blocks_per_year: y,
    delta_blocks: deltaBlocks,
    verified_net_assets: v,
    supply_before: s,
    nav_before: navBefore,
    hwm_before: hwmBefore,
    annual_rate_bps_applied: m,
    hwm_rate_bps_applied: p,
    time_fee_value: fM,
    nav_after_time_fee: navAfterTimeFee,
    hwm_fee_value: fP,
    total_fee_value: fTotal,
    minted_fee_units: minted,
    clipped_fee_units: clipped,
    supply_after: sAfter,
    nav_after: navAfter,
    hwm_after: hwmAfter,
    reason_code: reasonCode,
  };

  return {
    feeState: {
      active: nextActive,
      pending: nextPending,
      high_water_mark_nav_per_unit: hwmAfter,
      last_accrual_height: hNow,
      unclaimed_fee_units: state.unclaimed_fee_units + minted,
      total_accrued_fee_units: state.total_accrued_fee_units + minted,
      total_claimed_fee_units: state.total_claimed_fee_units,
    },
    accrual,
  };
}

// ---------------------------------------------------------------------------
// Inspection, simulation & receipt verification
// ---------------------------------------------------------------------------

export function inspectPendingHike(feeState, currentBlockHeight) {
  const hNow = parseU64(currentBlockHeight, "current_block_height");
  const state = normalizeFeeState(feeState);
  if (state == null) {
    return {
      has_fee_schedule: false,
      has_pending: false,
      is_increase: false,
      exit_capacity_locked: false,
      blocks_remaining: 0n,
      ready_to_activate: false,
      changes: {},
    };
  }
  const pending = state.pending;
  if (pending == null) {
    return {
      has_fee_schedule: true,
      has_pending: false,
      is_increase: false,
      exit_capacity_locked: false,
      blocks_remaining: 0n,
      ready_to_activate: false,
      changes: {},
    };
  }

  const blocksRemaining =
    pending.effective_at_height > hNow ? pending.effective_at_height - hNow : 0n;
  const trackedFields = [
    "annual_rate_bps",
    "hwm_rate_bps",
    "issue_spread_bps",
    "redeem_spread_bps",
    "fix_fee_cap_bps",
    "recipient",
    "increase_notice_blocks",
    "accrual_cap_blocks",
    "rates_locked",
    "disclosure_ref",
  ];
  const changes = {};
  for (const field of trackedFields) {
    if (state.active[field] !== pending.schedule[field]) {
      changes[field] = { from: state.active[field], to: pending.schedule[field] };
    }
  }
  return {
    has_fee_schedule: true,
    has_pending: true,
    is_increase: pending.is_increase,
    exit_capacity_locked: pending.is_increase,
    proposed_at_height: pending.proposed_at_height,
    effective_at_height: pending.effective_at_height,
    blocks_remaining: blocksRemaining,
    ready_to_activate: hNow >= pending.effective_at_height,
    changes,
  };
}

export function simulateEpochs({
  assetId,
  issuer,
  precision,
  initialFeeState,
  initialSupply,
  epochs,
  maxSupply = null,
  maxEpochGapBlocks = null,
  blocksPerYear = PROTOCOL_BLOCKS_PER_YEAR,
}) {
  let state = normalizeFeeState(initialFeeState);
  let supply = parseU64(initialSupply, "initial_supply");
  const accruals = [];
  const events = [];

  for (const step of epochs) {
    if (step.propose_update != null) {
      const upd = step.propose_update;
      state = proposeFeeScheduleUpdate({
        feeState: state,
        issuer,
        scheduleInput: upd.schedule,
        blockHeight: upd.block_height,
        maxEpochGapBlocks,
        recipientCosigned: Boolean(upd.recipient_cosigned ?? false),
      });
      events.push({
        kind: NAV_FEE_SCHEDULE_SET_TRANSACTION_KIND,
        block_height: parseU64(upd.block_height, "block_height"),
        effective_at_height: state.pending ? state.pending.effective_at_height : 0n,
        is_increase: state.pending ? state.pending.is_increase : false,
      });
    }

    if (step.cancel_pending) {
      state = cancelPendingFeeSchedule(state);
      events.push({
        kind: NAV_FEE_SCHEDULE_CANCEL_TRANSACTION_KIND,
        block_height: parseU64(step.cancel_block_height ?? step.block_height, "cancel_block_height"),
      });
    }

    if (step.supply_before !== null && step.supply_before !== undefined) {
      supply = parseU64(step.supply_before, "supply_before");
    } else {
      if (step.external_mint_units) {
        supply += parseU64(step.external_mint_units, "external_mint_units");
      }
      if (step.external_redeem_units) {
        const redeem = parseU64(step.external_redeem_units, "external_redeem_units");
        if (redeem > supply) {
          throw new RangeError("external_redeem_units exceeds current supply");
        }
        supply -= redeem;
      }
    }

    const stepRes = accrueEpoch({
      assetId,
      epoch: step.epoch,
      blockHeight: step.block_height,
      verifiedNetAssets: step.verified_net_assets,
      supplyBefore: supply,
      precision,
      feeState: state,
      maxSupply,
      blocksPerYear,
    });
    state = stepRes.feeState;
    supply = stepRes.accrual.supply_after;
    accruals.push(stepRes.accrual);

    if (step.claim != null) {
      const claimRes = claimAccruedFees({
        feeState: state,
        caller: step.claim.caller,
        amount: step.claim.amount,
      });
      state = claimRes.feeState;
      events.push({
        kind: NAV_FEE_CLAIM_TRANSACTION_KIND,
        ...claimRes.receipt,
      });
    }
  }

  return {
    finalFeeState: state,
    finalSupply: supply,
    accruals,
    events,
  };
}

export function recomputeFees({
  assetId,
  epoch,
  blockHeight,
  verifiedNetAssets,
  supplyBefore,
  precision,
  feeStateBefore,
  maxSupply = null,
  blocksPerYear = PROTOCOL_BLOCKS_PER_YEAR,
}) {
  return accrueEpoch({
    assetId,
    epoch,
    blockHeight,
    verifiedNetAssets,
    supplyBefore,
    precision,
    feeState: feeStateBefore,
    maxSupply,
    blocksPerYear,
  });
}

export function verifyNavReceipt({ accrualReceipt, precision, feeStateBefore, maxSupply = null }) {
  const actual = normalizeAccrual(accrualReceipt);
  const { feeState: nextFeeState, accrual: expected } = recomputeFees({
    assetId: actual.asset_id,
    epoch: actual.epoch,
    blockHeight: actual.block_height,
    verifiedNetAssets: actual.verified_net_assets,
    supplyBefore: actual.supply_before,
    precision,
    feeStateBefore,
    maxSupply,
    blocksPerYear: actual.blocks_per_year,
  });

  const mismatches = {};
  for (const key of Object.keys(expected)) {
    if (expected[key] !== actual[key]) {
      mismatches[key] = { expected: expected[key], actual: actual[key] };
    }
  }

  if (actual.supply_after > 0n) {
    const expectedPostNav = computeNavPerUnit(
      actual.verified_net_assets,
      actual.supply_after,
      precision,
    );
    if (actual.nav_after !== expectedPostNav) {
      mismatches.universal_post_fee_nav = {
        expected: expectedPostNav,
        actual: actual.nav_after,
      };
    }
  }

  return {
    valid: Object.keys(mismatches).length === 0,
    mismatches,
    expected,
    actual,
    nextFeeState,
  };
}

// ---------------------------------------------------------------------------
// Cross-language vector evaluator
// ---------------------------------------------------------------------------

export function evaluateVector(vector) {
  const kind = vector.kind;
  const inp = vector.input;

  if (kind === "universal_nav") {
    try {
      const res = verifyNavPerUnit({
        verifiedNetAssets: inp.verified_net_assets,
        circulatingSupply: inp.circulating_supply,
        precision: inp.precision,
        claimedNavPerUnit: inp.claimed_nav_per_unit,
        liveLedgerSupply: inp.live_ledger_supply ?? null,
      });
      return {
        ok: true,
        expected_nav_per_unit: res.expected_nav_per_unit.toString(10),
        floor_remainder_scaled: res.floor_remainder_scaled.toString(10),
      };
    } catch (err) {
      if (err instanceof NavFeeError) {
        return { ok: false, error_code: err.receiptCode };
      }
      throw err;
    }
  }

  if (kind === "schedule_validation") {
    const subkind = inp.subkind ?? "attach";
    try {
      if (subkind === "attach") {
        const state = attachFeeSchedule({
          existingFeeState: inp.existing_fee_state ?? null,
          issuer: inp.issuer,
          scheduleInput: inp.schedule,
          blockHeight: inp.block_height,
          currentNavPerUnit: inp.current_nav_per_unit ?? 0n,
          maxEpochGapBlocks: inp.max_epoch_gap_blocks ?? null,
        });
        return {
          ok: true,
          attached_at_height: state.active.attached_at_height.toString(10),
          last_accrual_height: state.last_accrual_height.toString(10),
          high_water_mark_nav_per_unit: state.high_water_mark_nav_per_unit.toString(10),
        };
      }
      if (subkind === "unscheduled_spread") {
        checkUnscheduledSpreadUpdate({
          currentIssueSpreadBps: inp.current_issue_spread_bps,
          currentRedeemSpreadBps: inp.current_redeem_spread_bps,
          newIssueSpreadBps: inp.new_issue_spread_bps,
          newRedeemSpreadBps: inp.new_redeem_spread_bps,
          proposedAtHeight: inp.proposed_at_height,
          effectiveAtHeight: inp.effective_at_height,
        });
        return { ok: true };
      }
      if (subkind === "fx_fix_cap") {
        checkFxFixFeeCap(inp.fee_bps, inp.fee_state ?? null);
        return { ok: true };
      }
      if (subkind === "non_issuer_attestor") {
        checkNonIssuerAttestation({
          issuer: inp.issuer,
          feeState: inp.fee_state ?? null,
          attestedValue: inp.attested_value,
          attestations: inp.attestations,
        });
        return { ok: true };
      }
      throw new Error(`unknown schedule_validation subkind: ${subkind}`);
    } catch (err) {
      if (err instanceof NavFeeError) {
        return { ok: false, error_code: err.receiptCode };
      }
      throw err;
    }
  }

  if (kind === "single_epoch_accrual") {
    const { feeState: nextState, accrual } = accrueEpoch({
      assetId: inp.asset_id,
      epoch: inp.epoch,
      blockHeight: inp.block_height,
      verifiedNetAssets: inp.verified_net_assets,
      supplyBefore: inp.supply_before,
      precision: inp.precision,
      feeState: inp.fee_state,
      maxSupply: inp.max_supply ?? null,
      blocksPerYear: inp.blocks_per_year ?? PROTOCOL_BLOCKS_PER_YEAR,
    });
    return {
      ok: true,
      accrual: formatAccrualVectorDict(accrual),
      next_active_annual_rate_bps: nextState.active.annual_rate_bps,
      next_active_hwm_rate_bps: nextState.active.hwm_rate_bps,
      pending_cleared: nextState.pending == null,
      unclaimed_fee_units: nextState.unclaimed_fee_units.toString(10),
      total_accrued_fee_units: nextState.total_accrued_fee_units.toString(10),
      last_accrual_height: nextState.last_accrual_height.toString(10),
    };
  }

  if (kind === "pending_notice_and_lifecycle") {
    const action = inp.action;
    try {
      if (action === "propose") {
        const nxt = proposeFeeScheduleUpdate({
          feeState: inp.fee_state ?? null,
          issuer: inp.issuer,
          scheduleInput: inp.schedule,
          blockHeight: inp.block_height,
          maxEpochGapBlocks: inp.max_epoch_gap_blocks ?? null,
          recipientCosigned: Boolean(inp.recipient_cosigned ?? false),
        });
        return {
          ok: true,
          is_increase: nxt.pending.is_increase,
          effective_at_height: nxt.pending.effective_at_height.toString(10),
          proposed_at_height: nxt.pending.proposed_at_height.toString(10),
        };
      }
      if (action === "cancel") {
        const nxt = cancelPendingFeeSchedule(inp.fee_state ?? null);
        return { ok: true, pending_cleared: nxt.pending == null };
      }
      if (action === "exit_capacity") {
        checkExitCapacityUpdate(
          inp.fee_state ?? null,
          inp.current_redeem_capacity_atoms,
          inp.new_redeem_capacity_atoms,
        );
        return { ok: true };
      }
      if (action === "claim") {
        const { feeState: nxt, receipt } = claimAccruedFees({
          feeState: inp.fee_state ?? null,
          caller: inp.caller,
          amount: inp.amount,
        });
        return {
          ok: true,
          claimed_amount: receipt.claimed_amount.toString(10),
          unclaimed_remaining: nxt.unclaimed_fee_units.toString(10),
          total_claimed_fee_units: nxt.total_claimed_fee_units.toString(10),
        };
      }
      throw new Error(`unknown lifecycle action: ${action}`);
    } catch (err) {
      if (err instanceof NavFeeError) {
        return { ok: false, error_code: err.receiptCode };
      }
      throw err;
    }
  }

  if (kind === "multi_epoch_compounding") {
    const sim = simulateEpochs({
      assetId: inp.asset_id,
      issuer: inp.issuer,
      precision: inp.precision,
      initialFeeState: inp.initial_fee_state,
      initialSupply: inp.initial_supply,
      epochs: inp.epochs,
      maxSupply: inp.max_supply ?? null,
      maxEpochGapBlocks: inp.max_epoch_gap_blocks ?? null,
      blocksPerYear: inp.blocks_per_year ?? PROTOCOL_BLOCKS_PER_YEAR,
    });
    const finalState = sim.finalFeeState;
    return {
      ok: true,
      final_supply: sim.finalSupply.toString(10),
      final_hwm: finalState.high_water_mark_nav_per_unit.toString(10),
      final_unclaimed_fee_units: finalState.unclaimed_fee_units.toString(10),
      final_total_accrued_fee_units: finalState.total_accrued_fee_units.toString(10),
      final_total_claimed_fee_units: finalState.total_claimed_fee_units.toString(10),
      final_active_annual_rate_bps: finalState.active.annual_rate_bps,
      final_active_hwm_rate_bps: finalState.active.hwm_rate_bps,
      final_active_recipient: finalState.active.recipient,
      final_has_pending: finalState.pending != null,
      accruals: sim.accruals.map(formatAccrualVectorDict),
    };
  }

  throw new Error(`unknown vector kind: ${kind}`);
}
