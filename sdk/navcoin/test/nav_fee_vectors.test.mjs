import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import {
  NAV_FEE_MAX_ANNUAL_RATE_BPS,
  NAV_FEE_MAX_FIX_FEE_CAP_BPS,
  NAV_FEE_MAX_HWM_RATE_BPS,
  NAV_FEE_MAX_SPREAD_BPS,
  NAV_FEE_MIN_INCREASE_NOTICE_BLOCKS,
  NavFeeError,
  PROTOCOL_BLOCKS_PER_YEAR,
  attachFeeSchedule,
  buildNavFeeClaimOperation,
  buildNavFeeHistoryRpcRequest,
  buildNavFeeScheduleCancelOperation,
  buildNavFeeScheduleRpcRequest,
  buildNavFeeScheduleSetOperation,
  cancelPendingFeeSchedule,
  canonicalJsonString,
  computeNavPerUnit,
  createNavcoin,
  derivedAssetId,
  evaluateVector,
  inspectPendingHike,
  navReservePacketId,
  parseNavFeeHistoryResponse,
  parseNavFeeScheduleResponse,
  proposeFeeScheduleUpdate,
  recomputeFees,
  verifyNavPerUnit,
  verifyNavReceipt,
} from "../src/index.mjs";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const VECTOR_PATH = path.resolve(__dirname, "../vectors/nav_fee_vectors_v1.json");
const CLI_PATH = path.resolve(__dirname, "../bin/navcoin-cli.mjs");

const ISSUER = "pf1111111111111111111111111111111111111111";
const RECIPIENT = "pf3333333333333333333333333333333333333333";

const vectorSuite = JSON.parse(fs.readFileSync(VECTOR_PATH, "utf8"));

test("vector suite schema, constants, and >= 100 exact-atom vectors", () => {
  assert.equal(vectorSuite.schema, "postfiat-nav-fee-vectors-v1");
  assert.ok(vectorSuite.vectors.length >= 100, `expected >= 100 vectors, got ${vectorSuite.vectors.length}`);
  assert.equal(vectorSuite.vector_count, vectorSuite.vectors.length);
  assert.equal(PROTOCOL_BLOCKS_PER_YEAR, 31_536_000n);
  assert.equal(NAV_FEE_MAX_ANNUAL_RATE_BPS, 500);
  assert.equal(NAV_FEE_MAX_HWM_RATE_BPS, 5_000);
  assert.equal(NAV_FEE_MAX_SPREAD_BPS, 500);
  assert.equal(NAV_FEE_MAX_FIX_FEE_CAP_BPS, 100);
  assert.equal(NAV_FEE_MIN_INCREASE_NOTICE_BLOCKS, 604_800n);
});

for (const vector of vectorSuite.vectors) {
  test(`vector ${vector.id} [${vector.kind}]: ${vector.description}`, () => {
    const actual = evaluateVector(vector);
    assert.deepEqual(actual, vector.expected);
  });
}

test("float64 inputs are rejected to prevent rounding drift", () => {
  assert.throws(() => computeNavPerUnit(1.5, 1, 6), RangeError);
  assert.throws(() => computeNavPerUnit(Number.MAX_SAFE_INTEGER + 2, 1, 6), RangeError);
  assert.throws(() => computeNavPerUnit(true, 1, 6), TypeError);
});

test("zero-fee createNavcoin emits zero fee operations and zero fee clutter", () => {
  const plan = createNavcoin({
    chainId: "postfiat-wan-devnet-2",
    issuer: ISSUER,
    code: "OPENNAV",
    displayName: "Open Reserve Unit",
    precision: 6,
  });
  assert.equal(plan.fee_enabled, false);
  assert.equal(plan.operations.length, 2);
  assert.deepEqual(
    plan.operations.map((op) => op.operation),
    ["asset_create", "nav_asset_register"],
  );
  assert.equal(plan.asset_id.length, 96);
});

test("fee-enabled createNavcoin validates caps and appends nav_fee_schedule_attach", () => {
  const plan = createNavcoin({
    chainId: "postfiat-wan-devnet-2",
    issuer: ISSUER,
    code: "FEENAV",
    displayName: "Scheduled Reserve Unit",
    precision: 8,
    maxSupply: 10_000_000_000_000n,
    feeSchedule: {
      annual_rate_bps: 100,
      hwm_rate_bps: 1000,
      issue_spread_bps: 50,
      redeem_spread_bps: 50,
      fix_fee_cap_bps: 25,
      recipient: RECIPIENT,
      increase_notice_blocks: 604_800n,
      accrual_cap_blocks: 86_400n,
    },
  });
  assert.equal(plan.fee_enabled, true);
  assert.equal(plan.operations.length, 3);
  assert.equal(plan.operations[2].operation, "nav_fee_schedule_attach");
  assert.equal(plan.operations[2].schedule.annual_rate_bps, 100);
  assert.equal(plan.operations[2].schedule.recipient, RECIPIENT);

  assert.throws(
    () =>
      createNavcoin({
        chainId: "postfiat-wan-devnet-2",
        issuer: ISSUER,
        code: "BADFEE",
        displayName: "Bad Fee",
        feeSchedule: {
          annual_rate_bps: 501,
          hwm_rate_bps: 0,
          issue_spread_bps: 0,
          redeem_spread_bps: 0,
          fix_fee_cap_bps: 0,
          recipient: RECIPIENT,
        },
      }),
    (err) => err instanceof NavFeeError && err.receiptCode === "nav_fee_rate_cap_exceeded",
  );
});

test("derivedAssetId and navReservePacketId match Rust consensus vectors", () => {
  const issuer = "pfissuer000000000000000000000000000000000";
  assert.equal(
    derivedAssetId("postfiat-local", issuer, "USD", 1),
    "7eb4ab19c010edff936edcff5e4e8c04300d15e1751c102ab91fcec8ac3e0c49738c877dcde38c473050c2d68ecff45f",
  );
  const assetId =
    "f0fcc338230cfbc693203d46deb36c39cfe7f492c088dbb4811b0bca54eda8082ec3d7c4bbfe0fe79fa42fe10b1713b8";
  const packetHash =
    "75c981a212c520fa45f7846c0265740a9ed9b8c7bedf9963ead6e0dd1828951c1337df22d68d2487516c68d3927a47f5";
  assert.equal(
    navReservePacketId(assetId, 1n, packetHash),
    "69eb1f8adda23bfd3f8078d4ad76e7cca15f9d80d191e0dfb57f79e5319f63fd708b2080afae35e0fce5c9f4952dcab9",
  );
});

test("pending hike inspection, cancel flow, operation builders, and receipt verification", () => {
  let state = attachFeeSchedule({
    issuer: ISSUER,
    blockHeight: 100_000n,
    currentNavPerUnit: 100_000_000n,
    scheduleInput: {
      annual_rate_bps: 100,
      hwm_rate_bps: 1000,
      issue_spread_bps: 50,
      redeem_spread_bps: 50,
      fix_fee_cap_bps: 25,
      recipient: RECIPIENT,
      increase_notice_blocks: 604_800n,
      accrual_cap_blocks: 1_000_000n,
    },
  });

  const beforeInspection = inspectPendingHike(state, 150_000n);
  assert.equal(beforeInspection.has_fee_schedule, true);
  assert.equal(beforeInspection.has_pending, false);

  state = proposeFeeScheduleUpdate({
    feeState: state,
    issuer: ISSUER,
    blockHeight: 150_000n,
    scheduleInput: {
      annual_rate_bps: 250,
      hwm_rate_bps: 1500,
      issue_spread_bps: 50,
      redeem_spread_bps: 50,
      fix_fee_cap_bps: 25,
      recipient: RECIPIENT,
      increase_notice_blocks: 604_800n,
      accrual_cap_blocks: 1_000_000n,
    },
  });

  const hike = inspectPendingHike(state, 200_000n);
  assert.equal(hike.has_pending, true);
  assert.equal(hike.is_increase, true);
  assert.equal(hike.exit_capacity_locked, true);
  assert.equal(hike.effective_at_height, 754_800n);
  assert.equal(hike.blocks_remaining, 554_800n);
  assert.equal(hike.ready_to_activate, false);
  assert.deepEqual(hike.changes.annual_rate_bps, { from: 100, to: 250 });

  state = cancelPendingFeeSchedule(state);
  assert.equal(inspectPendingHike(state, 200_000n).has_pending, false);

  // Recompute and verify Train B receipt
  const { accrual } = recomputeFees({
    assetId: "99bb".repeat(24),
    epoch: 12n,
    blockHeight: 415_360n,
    verifiedNetAssets: 1_100_000_000_000n,
    supplyBefore: 1_000_000_000_000n,
    precision: 8,
    feeStateBefore: state,
  });
  const okCheck = verifyNavReceipt({
    accrualReceipt: accrual,
    precision: 8,
    feeStateBefore: state,
  });
  assert.equal(okCheck.valid, true);
  assert.deepEqual(okCheck.mismatches, {});

  // Tampering with nav_after by 1 atom is caught
  const tampered = { ...accrual, nav_after: accrual.nav_after - 1n };
  const badCheck = verifyNavReceipt({
    accrualReceipt: tampered,
    precision: 8,
    feeStateBefore: state,
  });
  assert.equal(badCheck.valid, false);
  assert.ok("nav_after" in badCheck.mismatches);

  // Operation builders & RPC helpers
  const setOp = buildNavFeeScheduleSetOperation({
    issuer: ISSUER,
    assetId: "99bb".repeat(24),
    schedule: {
      annual_rate_bps: 50,
      hwm_rate_bps: 500,
      issue_spread_bps: 25,
      redeem_spread_bps: 25,
      fix_fee_cap_bps: 10,
      recipient: RECIPIENT,
    },
  });
  assert.equal(setOp.operation, "nav_fee_schedule_set");
  assert.equal(
    buildNavFeeScheduleCancelOperation({ issuer: ISSUER, assetId: "99bb".repeat(24) }).operation,
    "nav_fee_schedule_cancel",
  );
  assert.equal(
    buildNavFeeClaimOperation({ recipient: RECIPIENT, assetId: "99bb".repeat(24), amount: 500n })
      .operation,
    "nav_fee_claim",
  );

  const schedReq = buildNavFeeScheduleRpcRequest("99bb".repeat(24));
  assert.equal(schedReq.method, "nav_fee_schedule");
  const parsedSched = parseNavFeeScheduleResponse({
    result: {
      schema: "postfiat-nav-fee-schedule-v1",
      chain_id: "postfiat-wan-devnet-2",
      block_height: 200500,
      asset_id: "99bb".repeat(24),
      found: true,
      has_fee_schedule: false,
      blocks_per_year: 31536000,
      fee_state: null,
    },
  });
  assert.equal(parsedSched.has_fee_schedule, false);
  assert.equal(parsedSched.fee_state, null);

  const histReq = buildNavFeeHistoryRpcRequest("99bb".repeat(24), { fromEpoch: 1n, limit: 10 });
  assert.equal(histReq.method, "nav_fee_history");
  const parsedHist = parseNavFeeHistoryResponse({
    result: {
      schema: "postfiat-nav-fee-history-v1",
      chain_id: "postfiat-wan-devnet-2",
      asset_id: "99bb".repeat(24),
      accrual_count: 1,
      truncated: false,
      next_cursor: null,
      accruals: [accrual],
    },
  });
  assert.equal(parsedHist.accrual_count, 1);
  assert.equal(parsedHist.accruals[0].minted_fee_units, 9_265_979_203n);
});

test("CLI verify-vectors, create (zero-fee and fee-enabled), and verify-nav work end to end", () => {
  const vecOut = JSON.parse(
    execFileSync(process.execPath, [CLI_PATH, "verify-vectors"], { encoding: "utf8" }),
  );
  assert.equal(vecOut.failed, 0);
  assert.ok(vecOut.passed >= 100);

  const zeroPlan = JSON.parse(
    execFileSync(
      process.execPath,
      [
        CLI_PATH,
        "create",
        "--chain-id",
        "postfiat-wan-devnet-2",
        "--issuer",
        ISSUER,
        "--code",
        "OPENNAV",
        "--name",
        "Open Reserve Unit",
      ],
      { encoding: "utf8" },
    ),
  );
  assert.equal(zeroPlan.fee_enabled, false);
  assert.equal(zeroPlan.operations.length, 2);

  const feePlan = JSON.parse(
    execFileSync(
      process.execPath,
      [
        CLI_PATH,
        "create",
        "--chain-id",
        "postfiat-wan-devnet-2",
        "--issuer",
        ISSUER,
        "--code",
        "FEENAV",
        "--name",
        "Scheduled Reserve Unit",
        "--fee-recipient",
        RECIPIENT,
        "--annual-rate-bps",
        "100",
        "--hwm-rate-bps",
        "1000",
      ],
      { encoding: "utf8" },
    ),
  );
  assert.equal(feePlan.fee_enabled, true);
  assert.equal(feePlan.operations.length, 3);
  assert.equal(feePlan.operations[2].operation, "nav_fee_schedule_attach");

  const navOut = JSON.parse(
    execFileSync(
      process.execPath,
      [
        CLI_PATH,
        "verify-nav",
        "--net-assets",
        "1100000000000",
        "--supply",
        "1000000000000",
        "--precision",
        "8",
        "--claimed-nav",
        "110000000",
      ],
      { encoding: "utf8" },
    ),
  );
  assert.equal(navOut.valid, true);
  assert.equal(navOut.expected_nav_per_unit, "110000000");

  // Confirm canonical JSON renders exact unquoted BigInt integer tokens
  assert.equal(canonicalJsonString({ b: 18446744073709551615n, a: 1 }), '{"a":1,"b":18446744073709551615}');
});
