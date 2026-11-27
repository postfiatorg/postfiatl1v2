/**
 * High-level neutral Open NAVCoin SDK builders and Train B RPC helpers.
 * Zero fee clutter when feeSchedule is omitted.
 */

import {
  DEFAULT_PROOF_PROFILE,
  DEFAULT_VALUATION_UNIT,
  ERR_INVALID_CLAIM_AMOUNT,
  MAX_PRECISION,
  NAV_FEE_CLAIM_TRANSACTION_KIND,
  NAV_FEE_HISTORY_RPC_METHOD,
  NAV_FEE_HISTORY_SCHEMA,
  NAV_FEE_SCHEDULE_ATTACH_TRANSACTION_KIND,
  NAV_FEE_SCHEDULE_CANCEL_TRANSACTION_KIND,
  NAV_FEE_SCHEDULE_RPC_METHOD,
  NAV_FEE_SCHEDULE_SCHEMA,
  NAV_FEE_SCHEDULE_SET_TRANSACTION_KIND,
  NavFeeError,
  PROTOCOL_BLOCKS_PER_YEAR,
} from "./constants.mjs";
import {
  derivedAssetId,
  normalizeAccount,
  parseExactUint,
  parseU32Number,
  parseU64,
} from "./canonical.mjs";
import {
  normalizeAccrual,
  normalizeFeeState,
  validateFeeScheduleInput,
} from "./engine.mjs";

function formatWireScheduleInput(validated) {
  const out = {
    annual_rate_bps: validated.annual_rate_bps,
    hwm_rate_bps: validated.hwm_rate_bps,
    issue_spread_bps: validated.issue_spread_bps,
    redeem_spread_bps: validated.redeem_spread_bps,
    fix_fee_cap_bps: validated.fix_fee_cap_bps,
    recipient: validated.recipient,
    increase_notice_blocks: validated.increase_notice_blocks,
    accrual_cap_blocks: validated.accrual_cap_blocks,
    rates_locked: validated.rates_locked,
  };
  if (validated.disclosure_ref !== null && validated.disclosure_ref !== undefined) {
    out.disclosure_ref = validated.disclosure_ref;
  }
  return out;
}

export function buildNavFeeScheduleAttachOperation({
  issuer,
  assetId,
  schedule,
  maxEpochGapBlocks = null,
}) {
  const canonicalIssuer = normalizeAccount(issuer, "issuer");
  const validated = validateFeeScheduleInput(schedule, {
    issuer: canonicalIssuer,
    maxEpochGapBlocks,
  });
  return {
    operation: NAV_FEE_SCHEDULE_ATTACH_TRANSACTION_KIND,
    issuer: canonicalIssuer,
    asset_id: assetId,
    schedule: formatWireScheduleInput(validated),
  };
}

export function buildNavFeeScheduleSetOperation({
  issuer,
  assetId,
  schedule,
  maxEpochGapBlocks = null,
}) {
  const canonicalIssuer = normalizeAccount(issuer, "issuer");
  const validated = validateFeeScheduleInput(schedule, {
    issuer: canonicalIssuer,
    maxEpochGapBlocks,
  });
  return {
    operation: NAV_FEE_SCHEDULE_SET_TRANSACTION_KIND,
    issuer: canonicalIssuer,
    asset_id: assetId,
    schedule: formatWireScheduleInput(validated),
  };
}

export function buildNavFeeScheduleCancelOperation({ issuer, assetId }) {
  const canonicalIssuer = normalizeAccount(issuer, "issuer");
  return {
    operation: NAV_FEE_SCHEDULE_CANCEL_TRANSACTION_KIND,
    issuer: canonicalIssuer,
    asset_id: assetId,
  };
}

export function buildNavFeeClaimOperation({ recipient, assetId, amount }) {
  const canonicalRecipient = normalizeAccount(recipient, "recipient");
  const claimAmount = parseU64(amount, "amount");
  if (claimAmount === 0n) {
    throw new NavFeeError(ERR_INVALID_CLAIM_AMOUNT, "claim amount must be > 0");
  }
  return {
    operation: NAV_FEE_CLAIM_TRANSACTION_KIND,
    recipient: canonicalRecipient,
    asset_id: assetId,
    amount: claimAmount,
  };
}

/**
 * Build deterministic launch operations for either a zero-fee NAVCoin
 * (`feeSchedule == null`) or a fee-enabled NAVCoin (`feeSchedule` provided).
 */
export function createNavcoin({
  chainId,
  issuer,
  code,
  displayName,
  precision = 6,
  version = 1,
  maxSupply = null,
  proofProfile = DEFAULT_PROOF_PROFILE,
  valuationUnit = DEFAULT_VALUATION_UNIT,
  reserveOperator = null,
  redemptionAccount = null,
  requiresAuthorization = false,
  freezeEnabled = false,
  clawbackEnabled = false,
  feeSchedule = null,
  accessPolicy = null,
  maxEpochGapBlocks = null,
}) {
  const canonicalIssuer = normalizeAccount(issuer, "issuer");
  const canonicalOperator =
    reserveOperator != null
      ? normalizeAccount(reserveOperator, "reserve_operator")
      : canonicalIssuer;
  const canonicalRedemption =
    redemptionAccount != null
      ? normalizeAccount(redemptionAccount, "redemption_account")
      : canonicalIssuer;
  const prec = Number(parseExactUint(precision, "precision", BigInt(MAX_PRECISION)));
  const ver = parseU32Number(version, "version");
  const assetId = derivedAssetId(chainId, canonicalIssuer, code, ver);

  const assetCreate = {
    operation: "asset_create",
    issuer: canonicalIssuer,
    code,
    version: ver,
    precision: prec,
    display_name: displayName,
    requires_authorization: Boolean(requiresAuthorization),
    freeze_enabled: Boolean(freezeEnabled),
    clawback_enabled: Boolean(clawbackEnabled),
  };
  if (maxSupply !== null && maxSupply !== undefined) {
    assetCreate.max_supply = parseU64(maxSupply, "max_supply");
  }

  const navAssetRegister = {
    operation: "nav_asset_register",
    issuer: canonicalIssuer,
    asset_id: assetId,
    reserve_operator: canonicalOperator,
    proof_profile: proofProfile,
    valuation_unit: valuationUnit,
    redemption_account: canonicalRedemption,
  };

  const operations = [assetCreate, navAssetRegister];

  if (accessPolicy != null) {
    if (!requiresAuthorization) {
      throw new Error("accessPolicy requires requiresAuthorization=true on asset_create");
    }
    operations.push({
      operation: "asset_access_set",
      issuer: canonicalIssuer,
      asset_id: assetId,
      ...accessPolicy,
    });
  }

  const feeEnabled = feeSchedule != null;
  if (feeSchedule != null) {
    operations.push(
      buildNavFeeScheduleAttachOperation({
        issuer: canonicalIssuer,
        assetId,
        schedule: feeSchedule,
        maxEpochGapBlocks,
      }),
    );
  }

  return {
    schema: "postfiat-navcoin-launch-plan-v1",
    chain_id: chainId,
    asset_id: assetId,
    issuer: canonicalIssuer,
    code,
    version: ver,
    precision: prec,
    fee_enabled: feeEnabled,
    operations,
  };
}

export function buildNavFeeScheduleRpcRequest(assetId, requestId = "nav-fee-1") {
  return {
    id: requestId,
    method: NAV_FEE_SCHEDULE_RPC_METHOD,
    params: { asset_id: assetId },
  };
}

export function parseNavFeeScheduleResponse(payload) {
  const result = payload.result ?? payload;
  if (result.schema !== NAV_FEE_SCHEDULE_SCHEMA) {
    throw new Error(`unexpected nav_fee_schedule schema: ${result.schema}`);
  }
  const hasFeeSchedule = Boolean(result.has_fee_schedule ?? false);
  const feeState =
    hasFeeSchedule && result.fee_state != null ? normalizeFeeState(result.fee_state) : null;
  return {
    schema: NAV_FEE_SCHEDULE_SCHEMA,
    chain_id: result.chain_id,
    block_height: parseU64(result.block_height ?? 0n, "block_height"),
    asset_id: result.asset_id,
    found: Boolean(result.found ?? false),
    has_fee_schedule: hasFeeSchedule,
    blocks_per_year: parseU64(result.blocks_per_year ?? PROTOCOL_BLOCKS_PER_YEAR, "blocks_per_year"),
    fee_state: feeState,
  };
}

export function buildNavFeeHistoryRpcRequest(
  assetId,
  { fromEpoch = null, limit = 50, cursor = null, requestId = "nav-fee-hist-1" } = {},
) {
  const params = {
    asset_id: assetId,
    limit: parseU32Number(limit, "limit"),
    cursor,
  };
  if (fromEpoch !== null && fromEpoch !== undefined) {
    params.from_epoch = parseU64(fromEpoch, "from_epoch");
  }
  return {
    id: requestId,
    method: NAV_FEE_HISTORY_RPC_METHOD,
    params,
  };
}

export function parseNavFeeHistoryResponse(payload) {
  const result = payload.result ?? payload;
  if (result.schema !== NAV_FEE_HISTORY_SCHEMA) {
    throw new Error(`unexpected nav_fee_history schema: ${result.schema}`);
  }
  const accruals = (result.accruals || []).map(normalizeAccrual);
  return {
    schema: NAV_FEE_HISTORY_SCHEMA,
    chain_id: result.chain_id,
    asset_id: result.asset_id,
    accrual_count: accruals.length,
    truncated: Boolean(result.truncated ?? false),
    next_cursor: result.next_cursor ?? null,
    accruals,
  };
}
