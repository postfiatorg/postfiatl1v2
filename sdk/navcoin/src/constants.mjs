/**
 * Frozen protocol constants for Pillar 7 (L1 Upgrade Train B, Sections 1 & 3.1).
 * Exact BigInt values ensure byte-identical integer math with Rust u256 and Python int.
 */

export const PROTOCOL_BLOCKS_PER_DAY = 86_400n;
export const PROTOCOL_BLOCKS_PER_7_DAYS = 604_800n;
export const PROTOCOL_BLOCKS_PER_30_DAYS = 2_592_000n;
export const PROTOCOL_BLOCKS_PER_YEAR = 31_536_000n;

export const NAV_FEE_BPS_DENOMINATOR = 10_000n;
export const NAV_FEE_MAX_ANNUAL_RATE_BPS = 500;
export const NAV_FEE_MAX_HWM_RATE_BPS = 5_000;
export const NAV_FEE_MAX_SPREAD_BPS = 500;
export const NAV_FEE_MAX_FIX_FEE_CAP_BPS = 100;
export const NAV_FEE_MIN_INCREASE_NOTICE_BLOCKS = PROTOCOL_BLOCKS_PER_7_DAYS;
export const NAV_FEE_MAX_DISCLOSURE_REF_BYTES = 128;

export const U32_MAX = 0xffff_ffffn;
export const U64_MAX = 0xffff_ffff_ffff_ffffn;
export const MAX_PRECISION = 18;

export const NAV_FEE_SCHEDULE_ATTACH_TRANSACTION_KIND = "nav_fee_schedule_attach";
export const NAV_FEE_SCHEDULE_SET_TRANSACTION_KIND = "nav_fee_schedule_set";
export const NAV_FEE_SCHEDULE_CANCEL_TRANSACTION_KIND = "nav_fee_schedule_cancel";
export const NAV_FEE_CLAIM_TRANSACTION_KIND = "nav_fee_claim";

export const NAV_FEE_SCHEDULE_RPC_METHOD = "nav_fee_schedule";
export const NAV_FEE_HISTORY_RPC_METHOD = "nav_fee_history";
export const NAV_FEE_SCHEDULE_SCHEMA = "postfiat-nav-fee-schedule-v1";
export const NAV_FEE_HISTORY_SCHEMA = "postfiat-nav-fee-history-v1";

export const DEFAULT_VALUATION_UNIT = "usd_1e6";
export const DEFAULT_PROOF_PROFILE = "ledger-transparent-v1";

export const ISSUED_ASSET_ID_DOMAIN = "postfiat.issued_asset_id.v1";
export const NAV_RESERVE_PACKET_ID_DOMAIN = "postfiat.nav_reserve_packet_id.v1";
export const ADDRESS_CHECKSUM_DOMAIN = "postfiat.address.checksum.v1";

export const REASON_CODES = Object.freeze(
  new Set([
    "accrued",
    "zero_supply",
    "zero_net_assets",
    "zero_rates",
    "zero_delta_blocks",
    "clamped_max_supply",
    "clamped_full_value",
  ]),
);

export const ERR_SCHEDULE_ALREADY_ATTACHED = "nav_fee_schedule_already_attached";
export const ERR_SCHEDULE_MISSING = "nav_fee_schedule_missing";
export const ERR_RATE_CAP_EXCEEDED = "nav_fee_rate_cap_exceeded";
export const ERR_NOTICE_TOO_SHORT = "nav_fee_notice_too_short";
export const ERR_RECIPIENT_EQUALS_ISSUER = "nav_fee_recipient_equals_issuer";
export const ERR_INVALID_RECIPIENT = "nav_fee_invalid_recipient";
export const ERR_INVALID_ACCRUAL_CAP = "nav_fee_invalid_accrual_cap";
export const ERR_DISCLOSURE_REF_TOO_LONG = "nav_fee_disclosure_ref_too_long";
export const ERR_SCHEDULE_RATES_LOCKED = "nav_fee_schedule_rates_locked";
export const ERR_NO_PENDING_SCHEDULE = "nav_fee_no_pending_schedule";
export const ERR_PENDING_INCREASE_EXIT_CAPACITY_LOCKED =
  "nav_fee_pending_increase_exit_capacity_locked";
export const ERR_INSUFFICIENT_UNCLAIMED = "nav_fee_insufficient_unclaimed";
export const ERR_UNAUTHORIZED_RECIPIENT = "nav_fee_unauthorized_recipient";
export const ERR_INVALID_CLAIM_AMOUNT = "nav_fee_invalid_claim_amount";
export const ERR_NAV_PER_UNIT_FORMULA_MISMATCH = "nav_per_unit_formula_mismatch";
export const ERR_NAV_SUPPLY_SNAPSHOT_STALE = "nav_supply_snapshot_stale";
export const ERR_ISSUER_SELF_ATTESTATION_FORBIDDEN =
  "nav_fee_issuer_self_attestation_forbidden";
export const ERR_FX_FIX_FEE_EXCEEDS_NAV_CAP = "fx_fix_fee_exceeds_nav_cap";

export class NavFeeError extends Error {
  constructor(receiptCode, message) {
    super(message || receiptCode);
    this.name = "NavFeeError";
    this.receiptCode = receiptCode;
  }
}
