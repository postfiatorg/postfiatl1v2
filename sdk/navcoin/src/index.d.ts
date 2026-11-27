export type ExactUintInput = bigint | number | string;

export type NavFeeReasonCode =
  | "accrued"
  | "zero_supply"
  | "zero_net_assets"
  | "zero_rates"
  | "zero_delta_blocks"
  | "clamped_max_supply"
  | "clamped_full_value";

export const PROTOCOL_BLOCKS_PER_DAY: bigint;
export const PROTOCOL_BLOCKS_PER_7_DAYS: bigint;
export const PROTOCOL_BLOCKS_PER_30_DAYS: bigint;
export const PROTOCOL_BLOCKS_PER_YEAR: bigint;

export const NAV_FEE_BPS_DENOMINATOR: bigint;
export const NAV_FEE_MAX_ANNUAL_RATE_BPS: number;
export const NAV_FEE_MAX_HWM_RATE_BPS: number;
export const NAV_FEE_MAX_SPREAD_BPS: number;
export const NAV_FEE_MAX_FIX_FEE_CAP_BPS: number;
export const NAV_FEE_MIN_INCREASE_NOTICE_BLOCKS: bigint;
export const NAV_FEE_MAX_DISCLOSURE_REF_BYTES: number;

export const U32_MAX: bigint;
export const U64_MAX: bigint;
export const MAX_PRECISION: number;

export const NAV_FEE_SCHEDULE_ATTACH_TRANSACTION_KIND: "nav_fee_schedule_attach";
export const NAV_FEE_SCHEDULE_SET_TRANSACTION_KIND: "nav_fee_schedule_set";
export const NAV_FEE_SCHEDULE_CANCEL_TRANSACTION_KIND: "nav_fee_schedule_cancel";
export const NAV_FEE_CLAIM_TRANSACTION_KIND: "nav_fee_claim";

export const NAV_FEE_SCHEDULE_RPC_METHOD: "nav_fee_schedule";
export const NAV_FEE_HISTORY_RPC_METHOD: "nav_fee_history";
export const NAV_FEE_SCHEDULE_SCHEMA: "postfiat-nav-fee-schedule-v1";
export const NAV_FEE_HISTORY_SCHEMA: "postfiat-nav-fee-history-v1";

export const DEFAULT_VALUATION_UNIT: string;
export const DEFAULT_PROOF_PROFILE: string;

export class NavFeeError extends Error {
  readonly receiptCode: string;
  constructor(receiptCode: string, message?: string);
}

export interface NavFeeScheduleInputV1 {
  annual_rate_bps: number;
  hwm_rate_bps: number;
  issue_spread_bps: number;
  redeem_spread_bps: number;
  fix_fee_cap_bps: number;
  recipient: string;
  increase_notice_blocks?: ExactUintInput;
  accrual_cap_blocks?: ExactUintInput;
  rates_locked?: boolean;
  disclosure_ref?: string | null;
}

export interface NavFeeScheduleV1 {
  annual_rate_bps: number;
  hwm_rate_bps: number;
  issue_spread_bps: number;
  redeem_spread_bps: number;
  fix_fee_cap_bps: number;
  recipient: string;
  increase_notice_blocks: bigint;
  accrual_cap_blocks: bigint;
  rates_locked: boolean;
  disclosure_ref: string | null;
  attached_at_height: bigint;
}

export interface PendingNavFeeScheduleV1 {
  schedule: NavFeeScheduleV1;
  proposed_at_height: bigint;
  effective_at_height: bigint;
  is_increase: boolean;
}

export interface NavFeeStateV1 {
  active: NavFeeScheduleV1;
  pending: PendingNavFeeScheduleV1 | null;
  high_water_mark_nav_per_unit: bigint;
  last_accrual_height: bigint;
  unclaimed_fee_units: bigint;
  total_accrued_fee_units: bigint;
  total_claimed_fee_units: bigint;
}

export interface NavFeeAccrualV1 {
  asset_id: string;
  epoch: bigint;
  block_height: bigint;
  blocks_per_year: bigint;
  delta_blocks: bigint;
  verified_net_assets: bigint;
  supply_before: bigint;
  nav_before: bigint;
  hwm_before: bigint;
  annual_rate_bps_applied: number;
  hwm_rate_bps_applied: number;
  time_fee_value: bigint;
  nav_after_time_fee: bigint;
  hwm_fee_value: bigint;
  total_fee_value: bigint;
  minted_fee_units: bigint;
  clipped_fee_units: bigint;
  supply_after: bigint;
  nav_after: bigint;
  hwm_after: bigint;
  reason_code: NavFeeReasonCode;
}

export interface UniversalNavCheckResult {
  valid: true;
  verified_net_assets: bigint;
  circulating_supply: bigint;
  precision: number;
  unit_scale: bigint;
  expected_nav_per_unit: bigint;
  claimed_nav_per_unit: bigint;
  floor_remainder_scaled: bigint;
}

export interface PendingHikeInspection {
  has_fee_schedule: boolean;
  has_pending: boolean;
  is_increase: boolean;
  exit_capacity_locked: boolean;
  proposed_at_height?: bigint;
  effective_at_height?: bigint;
  blocks_remaining: bigint;
  ready_to_activate: boolean;
  changes: Record<string, { from: unknown; to: unknown }>;
}

export interface CreateNavcoinOptions {
  chainId: string;
  issuer: string;
  code: string;
  displayName: string;
  precision?: number;
  version?: number;
  maxSupply?: ExactUintInput | null;
  proofProfile?: string;
  valuationUnit?: string;
  reserveOperator?: string | null;
  redemptionAccount?: string | null;
  requiresAuthorization?: boolean;
  freezeEnabled?: boolean;
  clawbackEnabled?: boolean;
  feeSchedule?: NavFeeScheduleInputV1 | null;
  accessPolicy?: Record<string, unknown> | null;
  maxEpochGapBlocks?: ExactUintInput | null;
}

export function parseExactUint(value: ExactUintInput, name?: string, maxValue?: bigint): bigint;
export function canonicalJsonString(value: unknown): string;
export function canonicalJsonBytes(value: unknown): Buffer;
export function toJsonSafe<T>(value: T): unknown;
export function sha3_384Hex(data: Uint8Array | string): string;
export function parseAddress(value: string): string;
export function derivedAssetId(chainId: string, issuer: string, code: string, version?: number): string;
export function navReservePacketId(assetId: string, epoch: ExactUintInput, reservePacketHash: string): string;

export function unitScale(precision: number): bigint;
export function computeNavPerUnit(
  verifiedNetAssets: ExactUintInput,
  circulatingSupply: ExactUintInput,
  precision: number,
): bigint;
export function verifyNavPerUnit(params: {
  verifiedNetAssets: ExactUintInput;
  circulatingSupply: ExactUintInput;
  precision: number;
  claimedNavPerUnit: ExactUintInput;
  liveLedgerSupply?: ExactUintInput | null;
}): UniversalNavCheckResult;

export function validateFeeScheduleInput(
  schedule: NavFeeScheduleInputV1,
  opts: { issuer: string; maxEpochGapBlocks?: ExactUintInput | null },
): NavFeeScheduleInputV1 & { increase_notice_blocks: bigint; accrual_cap_blocks: bigint };
export function isScheduleIncrease(
  current: NavFeeScheduleV1,
  proposed: NavFeeScheduleV1 | (NavFeeScheduleInputV1 & { increase_notice_blocks: bigint; accrual_cap_blocks: bigint }),
): boolean;

export function attachFeeSchedule(params: {
  existingFeeState?: NavFeeStateV1 | null;
  issuer: string;
  scheduleInput: NavFeeScheduleInputV1;
  blockHeight: ExactUintInput;
  currentNavPerUnit?: ExactUintInput;
  maxEpochGapBlocks?: ExactUintInput | null;
}): NavFeeStateV1;

export function proposeFeeScheduleUpdate(params: {
  feeState: NavFeeStateV1 | null;
  issuer: string;
  scheduleInput: NavFeeScheduleInputV1;
  blockHeight: ExactUintInput;
  maxEpochGapBlocks?: ExactUintInput | null;
  recipientCosigned?: boolean;
}): NavFeeStateV1;

export function cancelPendingFeeSchedule(feeState: NavFeeStateV1 | null): NavFeeStateV1;

export function claimAccruedFees(params: {
  feeState: NavFeeStateV1 | null;
  caller: string;
  amount: ExactUintInput;
}): {
  feeState: NavFeeStateV1;
  receipt: {
    recipient: string;
    claimed_amount: bigint;
    unclaimed_remaining: bigint;
    total_claimed_fee_units: bigint;
  };
};

export function checkExitCapacityUpdate(
  feeState: NavFeeStateV1 | null,
  currentRedeemCapacityAtoms: ExactUintInput,
  newRedeemCapacityAtoms: ExactUintInput,
): true;
export function checkUnscheduledSpreadUpdate(params: {
  currentIssueSpreadBps: number;
  currentRedeemSpreadBps: number;
  newIssueSpreadBps: number;
  newRedeemSpreadBps: number;
  proposedAtHeight: ExactUintInput;
  effectiveAtHeight: ExactUintInput;
}): true;
export function checkFxFixFeeCap(feeBps: number, feeState?: NavFeeStateV1 | null): true;
export function checkNonIssuerAttestation(params: {
  issuer: string;
  feeState?: NavFeeStateV1 | null;
  attestedValue: ExactUintInput;
  attestations: Array<{ attestor: string; pass?: boolean; passed?: boolean }>;
}): true;

export function accrueEpoch(params: {
  assetId: string;
  epoch: ExactUintInput;
  blockHeight: ExactUintInput;
  verifiedNetAssets: ExactUintInput;
  supplyBefore: ExactUintInput;
  precision: number;
  feeState: NavFeeStateV1;
  maxSupply?: ExactUintInput | null;
  blocksPerYear?: ExactUintInput;
}): { feeState: NavFeeStateV1; accrual: NavFeeAccrualV1 };

export function inspectPendingHike(
  feeState: NavFeeStateV1 | null,
  currentBlockHeight: ExactUintInput,
): PendingHikeInspection;

export function simulateEpochs(params: {
  assetId: string;
  issuer: string;
  precision: number;
  initialFeeState: NavFeeStateV1;
  initialSupply: ExactUintInput;
  epochs: Array<Record<string, unknown>>;
  maxSupply?: ExactUintInput | null;
  maxEpochGapBlocks?: ExactUintInput | null;
  blocksPerYear?: ExactUintInput;
}): {
  finalFeeState: NavFeeStateV1;
  finalSupply: bigint;
  accruals: NavFeeAccrualV1[];
  events: Array<Record<string, unknown>>;
};

export function recomputeFees(params: {
  assetId: string;
  epoch: ExactUintInput;
  blockHeight: ExactUintInput;
  verifiedNetAssets: ExactUintInput;
  supplyBefore: ExactUintInput;
  precision: number;
  feeStateBefore: NavFeeStateV1;
  maxSupply?: ExactUintInput | null;
  blocksPerYear?: ExactUintInput;
}): { feeState: NavFeeStateV1; accrual: NavFeeAccrualV1 };

export function verifyNavReceipt(params: {
  accrualReceipt: NavFeeAccrualV1 | Record<string, unknown>;
  precision: number;
  feeStateBefore: NavFeeStateV1;
  maxSupply?: ExactUintInput | null;
}): {
  valid: boolean;
  mismatches: Record<string, { expected: unknown; actual: unknown }>;
  expected: NavFeeAccrualV1;
  actual: NavFeeAccrualV1;
  nextFeeState: NavFeeStateV1;
};

export function createNavcoin(options: CreateNavcoinOptions): {
  schema: "postfiat-navcoin-launch-plan-v1";
  chain_id: string;
  asset_id: string;
  issuer: string;
  code: string;
  version: number;
  precision: number;
  fee_enabled: boolean;
  operations: Array<Record<string, unknown>>;
};

export function buildNavFeeScheduleAttachOperation(params: {
  issuer: string;
  assetId: string;
  schedule: NavFeeScheduleInputV1;
  maxEpochGapBlocks?: ExactUintInput | null;
}): Record<string, unknown>;
export function buildNavFeeScheduleSetOperation(params: {
  issuer: string;
  assetId: string;
  schedule: NavFeeScheduleInputV1;
  maxEpochGapBlocks?: ExactUintInput | null;
}): Record<string, unknown>;
export function buildNavFeeScheduleCancelOperation(params: {
  issuer: string;
  assetId: string;
}): Record<string, unknown>;
export function buildNavFeeClaimOperation(params: {
  recipient: string;
  assetId: string;
  amount: ExactUintInput;
}): Record<string, unknown>;

export function buildNavFeeScheduleRpcRequest(assetId: string, requestId?: string): Record<string, unknown>;
export function parseNavFeeScheduleResponse(payload: Record<string, unknown>): Record<string, unknown>;
export function buildNavFeeHistoryRpcRequest(
  assetId: string,
  opts?: { fromEpoch?: ExactUintInput | null; limit?: number; cursor?: string | null; requestId?: string },
): Record<string, unknown>;
export function parseNavFeeHistoryResponse(payload: Record<string, unknown>): Record<string, unknown>;

export function evaluateVector(vector: Record<string, unknown>): Record<string, unknown>;
