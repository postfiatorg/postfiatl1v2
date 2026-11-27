#!/usr/bin/env node
/**
 * Open NAVCoin SDK CLI (`pftl-navcoin`).
 *
 * Neutral protocol utility to:
 *  - build launch plans for zero-fee or fee-enabled NAVCoins (`create`)
 *  - verify universal floor(V * U / S) NAV equations (`verify-nav`)
 *  - inspect pending fee schedule changes (`inspect-hike`)
 *  - simulate multi-epoch exact-atom accrual (`simulate`)
 *  - verify on-chain NavFeeAccrualV1 receipts (`verify-receipt`)
 *  - validate the 100+ exact-atom cross-language vector suite (`verify-vectors`)
 */

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  DEFAULT_PROOF_PROFILE,
  DEFAULT_VALUATION_UNIT,
  NAV_FEE_MIN_INCREASE_NOTICE_BLOCKS,
  NavFeeError,
  PROTOCOL_BLOCKS_PER_DAY,
  PROTOCOL_BLOCKS_PER_YEAR,
  canonicalJsonString,
  createNavcoin,
  evaluateVector,
  inspectPendingHike,
  simulateEpochs,
  toJsonSafe,
  verifyNavPerUnit,
  verifyNavReceipt,
} from "../src/index.mjs";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const DEFAULT_VECTOR_PATH = path.resolve(__dirname, "../vectors/nav_fee_vectors_v1.json");

function parseFlags(argv) {
  const flags = {};
  const positional = [];
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg.startsWith("--")) {
      const eq = arg.indexOf("=");
      if (eq !== -1) {
        flags[arg.slice(2, eq)] = arg.slice(eq + 1);
      } else {
        const key = arg.slice(2);
        const next = argv[i + 1];
        if (next === undefined || next.startsWith("--")) {
          flags[key] = true;
        } else {
          flags[key] = next;
          i += 1;
        }
      }
    } else {
      positional.push(arg);
    }
  }
  return { flags, positional };
}

function readJsonArg(filePath) {
  return JSON.parse(fs.readFileSync(filePath, "utf8"));
}

function printOutput(obj, canonical = false) {
  if (canonical) {
    process.stdout.write(`${canonicalJsonString(obj)}\n`);
  } else {
    process.stdout.write(`${JSON.stringify(toJsonSafe(obj), null, 2)}\n`);
  }
}

function printHelp() {
  process.stdout.write(
    [
      "pftl-navcoin — Open NAVCoin SDK CLI (Pillar 7)",
      "",
      "Commands:",
      "  create          Build a launch plan for a zero-fee or fee-enabled NAVCoin",
      "  verify-nav      Verify exact floor(V * 10^precision / S) NAV math",
      "  inspect-hike    Inspect a pending fee schedule change and notice window",
      "  simulate        Simulate multi-epoch exact-atom accrual from a JSON scenario",
      "  verify-receipt  Independently recompute & verify a NavFeeAccrualV1 receipt",
      "  verify-vectors  Validate all cross-language exact-atom test vectors",
      "",
      "Use `<command> --help` for command-specific flags.",
    ].join("\n") + "\n",
  );
}

function cmdCreate(flags) {
  if (flags.help) {
    process.stdout.write(
      [
        "Usage: pftl-navcoin create --chain-id <id> --issuer <pf...> --code <CODE> --name <NAME> [options]",
        "",
        "Zero-fee by default. Pass --fee-recipient (plus optional rate flags) to attach NavFeeScheduleV1:",
        "  --precision <0..18>              (default: 6)",
        "  --max-supply <u64>               (optional)",
        "  --proof-profile <name>           (default: ledger-transparent-v1)",
        "  --valuation-unit <unit>          (default: usd_1e6)",
        "  --fee-recipient <pf...>          Enables optional fee schedule (must differ from issuer)",
        "  --annual-rate-bps <0..500>       Annual time-based rate cap (default: 0)",
        "  --hwm-rate-bps <0..5000>         High-water-mark gain rate (default: 0)",
        "  --issue-spread-bps <0..500>      Primary issue spread (default: 0)",
        "  --redeem-spread-bps <0..500>     Primary redeem spread (default: 0)",
        "  --fix-fee-cap-bps <0..100>       FX fix fee cap (default: 0)",
        "  --notice-blocks <>=604800>       Fee-increase notice period (default: 604800)",
        "  --accrual-cap-blocks <>0>        Max blocks accrued per interval (default: 86400)",
        "  --rates-locked                   Permanently lock rate ceilings",
        "  --disclosure-ref <str>           Optional <=128 byte reference",
        "  --canonical                      Emit byte-canonical JSON",
      ].join("\n") + "\n",
    );
    return 0;
  }

  let feeSchedule = null;
  if (flags["fee-recipient"] || flags["fee-schedule"]) {
    if (flags["fee-schedule"]) {
      feeSchedule = readJsonArg(flags["fee-schedule"]);
    } else {
      feeSchedule = {
        annual_rate_bps: Number(flags["annual-rate-bps"] ?? 0),
        hwm_rate_bps: Number(flags["hwm-rate-bps"] ?? 0),
        issue_spread_bps: Number(flags["issue-spread-bps"] ?? 0),
        redeem_spread_bps: Number(flags["redeem-spread-bps"] ?? 0),
        fix_fee_cap_bps: Number(flags["fix-fee-cap-bps"] ?? 0),
        recipient: flags["fee-recipient"],
        increase_notice_blocks: flags["notice-blocks"] ?? NAV_FEE_MIN_INCREASE_NOTICE_BLOCKS,
        accrual_cap_blocks: flags["accrual-cap-blocks"] ?? PROTOCOL_BLOCKS_PER_DAY,
        rates_locked: Boolean(flags["rates-locked"]),
        disclosure_ref: flags["disclosure-ref"] ?? null,
      };
    }
  }

  const plan = createNavcoin({
    chainId: flags["chain-id"],
    issuer: flags.issuer,
    code: flags.code,
    displayName: flags.name ?? flags.code,
    precision: flags.precision !== undefined ? Number(flags.precision) : 6,
    version: flags.version !== undefined ? Number(flags.version) : 1,
    maxSupply: flags["max-supply"] ?? null,
    proofProfile: flags["proof-profile"] ?? DEFAULT_PROOF_PROFILE,
    valuationUnit: flags["valuation-unit"] ?? DEFAULT_VALUATION_UNIT,
    requiresAuthorization: Boolean(flags["requires-authorization"]),
    freezeEnabled: Boolean(flags["freeze-enabled"]),
    clawbackEnabled: Boolean(flags["clawback-enabled"]),
    feeSchedule,
  });
  printOutput(plan, Boolean(flags.canonical));
  return 0;
}

function cmdVerifyNav(flags) {
  const result = verifyNavPerUnit({
    verifiedNetAssets: flags["net-assets"],
    circulatingSupply: flags.supply,
    precision: Number(flags.precision ?? 6),
    claimedNavPerUnit: flags["claimed-nav"],
    liveLedgerSupply: flags["live-supply"] ?? null,
  });
  printOutput(result, Boolean(flags.canonical));
  return 0;
}

function cmdInspectHike(flags) {
  const feeState = flags.state ? readJsonArg(flags.state) : null;
  const inspection = inspectPendingHike(feeState, flags["block-height"] ?? 0n);
  printOutput(inspection, Boolean(flags.canonical));
  return 0;
}

function cmdSimulate(flags) {
  const scenario = readJsonArg(flags.input);
  const res = simulateEpochs({
    assetId: scenario.asset_id,
    issuer: scenario.issuer,
    precision: scenario.precision,
    initialFeeState: scenario.initial_fee_state,
    initialSupply: scenario.initial_supply,
    epochs: scenario.epochs,
    maxSupply: scenario.max_supply ?? null,
    maxEpochGapBlocks: scenario.max_epoch_gap_blocks ?? null,
    blocksPerYear: scenario.blocks_per_year ?? PROTOCOL_BLOCKS_PER_YEAR,
  });
  printOutput(res, Boolean(flags.canonical));
  return 0;
}

function cmdVerifyReceipt(flags) {
  const bundle = readJsonArg(flags.input);
  const res = verifyNavReceipt({
    accrualReceipt: bundle.accrual_receipt,
    precision: bundle.precision,
    feeStateBefore: bundle.fee_state_before,
    maxSupply: bundle.max_supply ?? null,
  });
  printOutput(res, Boolean(flags.canonical));
  return res.valid ? 0 : 1;
}

function cmdVerifyVectors(flags) {
  const vectorPath = flags.file ? path.resolve(flags.file) : DEFAULT_VECTOR_PATH;
  const suite = readJsonArg(vectorPath);
  const vectors = suite.vectors || [];
  let passed = 0;
  const failures = [];
  for (const vec of vectors) {
    const actual = evaluateVector(vec);
    const expStr = canonicalJsonString(vec.expected);
    const actStr = canonicalJsonString(actual);
    if (expStr === actStr) {
      passed += 1;
    } else {
      failures.push({ id: vec.id, expected: vec.expected, actual });
    }
  }
  const summary = {
    schema: suite.schema,
    vector_file: vectorPath,
    total: vectors.length,
    passed,
    failed: failures.length,
    failures,
  };
  printOutput(summary, Boolean(flags.canonical));
  return failures.length === 0 && vectors.length >= 100 ? 0 : 1;
}

function main() {
  const [, , command, ...rest] = process.argv;
  if (!command || command === "--help" || command === "-h") {
    printHelp();
    process.exit(0);
  }
  const { flags } = parseFlags(rest);
  try {
    let code = 0;
    if (command === "create") code = cmdCreate(flags);
    else if (command === "verify-nav") code = cmdVerifyNav(flags);
    else if (command === "inspect-hike") code = cmdInspectHike(flags);
    else if (command === "simulate") code = cmdSimulate(flags);
    else if (command === "verify-receipt") code = cmdVerifyReceipt(flags);
    else if (command === "verify-vectors") code = cmdVerifyVectors(flags);
    else {
      process.stderr.write(`unknown command: ${command}\n`);
      printHelp();
      code = 2;
    }
    process.exit(code);
  } catch (err) {
    if (err instanceof NavFeeError) {
      printOutput({ ok: false, error_code: err.receiptCode, message: err.message });
      process.exit(1);
    }
    process.stderr.write(`error: ${err.message}\n`);
    process.exit(1);
  }
}

main();
