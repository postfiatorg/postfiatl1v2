#!/usr/bin/env node
import { readFileSync, writeFileSync } from 'node:fs';
import process from 'node:process';

import { buildStep10EvidenceSummary } from './lib/wallet-shielded-step10-evidence.mjs';

function readJson(path, fallback = undefined) {
  try {
    return JSON.parse(readFileSync(path, 'utf8'));
  } catch (error) {
    if (fallback !== undefined && error && error.code === 'ENOENT') return fallback;
    throw error;
  }
}

function writeJson(path, value) {
  writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`);
}

function argValue(name) {
  const idx = process.argv.indexOf(name);
  if (idx === -1) return null;
  return process.argv[idx + 1] || null;
}

const evidenceDir = argValue('--evidence-dir')
  || process.env.STEP10_EVIDENCE_DIR
  || process.env.ORCHARD_SWAP_E2E_OUT_DIR;
if (!evidenceDir) {
  console.error('usage: STEP10_EVIDENCE_DIR=docs/evidence/wallet-private-swap-step10-... node scripts/wallet-shielded-swap-step10-package.mjs');
  process.exit(2);
}

const operatorDemoCommand = argValue('--operator-demo-command')
  || process.env.STEP10_OPERATOR_DEMO_COMMAND
  || null;
const summary = buildStep10EvidenceSummary({
  evidenceDir: evidenceDir.replace(/\/+$/, ''),
  operatorDemoCommand,
  files: { readJson },
});
const outFile = `${evidenceDir.replace(/\/+$/, '')}/step10-summary.json`;
writeJson(outFile, summary);
console.log(JSON.stringify({
  ok: summary.package_ok,
  summary_file: outFile,
  run_count: summary.run_count,
  directions: summary.directions,
  request_logs_ok: summary.request_logs_ok,
  reload_rescan_ok: summary.reload_rescan_ok,
  operator_approval_required_for_can_run: summary.operator_approval_required_for_can_run,
}, null, 2));
if (summary.package_ok !== true) process.exit(1);
