#!/usr/bin/env node
import fs from 'node:fs/promises';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

const DEFAULT_INPUTS = [
  'docs/evidence',
  'wallet-proxy/ux-screenshots',
].filter(Boolean);

const ADDITIVE_TOLERANCE_MS = 5;
const EXPLICIT_WALL_CLOCK_SCHEMA = 'postfiat-wallet-private-swap-click-receipt-wall-clock-v1';

function usage() {
  return [
    'usage: node scripts/zk-latency-phase0-report.mjs [--input DIR_OR_JSON ...] [--out FILE] [--label LABEL]',
    '',
    'Scans explicit additive wall-clock JSON records and writes a machine-readable zk latency table.',
    'Implicit recursive *_ms scraping is intentionally disabled so proof/transport buckets cannot overlap.',
    'No live devnet rounds are started by this harness.',
  ].join('\n');
}

function parseArgs(argv) {
  const inputs = [];
  let out = null;
  let label = 'local';
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === '--input') {
      inputs.push(argv[++i]);
    } else if (arg === '--out') {
      out = argv[++i];
    } else if (arg === '--label') {
      label = argv[++i];
    } else if (arg === '--help' || arg === '-h') {
      console.log(usage());
      process.exit(0);
    } else {
      throw new Error(`unknown argument: ${arg}`);
    }
  }
  return {
    inputs: inputs.length > 0 ? inputs : DEFAULT_INPUTS,
    out,
    label,
  };
}

async function exists(target) {
  try {
    await fs.stat(target);
    return true;
  } catch {
    return false;
  }
}

async function walkJsonFiles(target) {
  const stat = await fs.stat(target);
  if (stat.isFile()) {
    return target.endsWith('.json') ? [target] : [];
  }
  if (!stat.isDirectory()) {
    return [];
  }
  const entries = await fs.readdir(target, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    if (entry.name === 'node_modules' || entry.name === 'target' || entry.name === '.git') {
      continue;
    }
    const child = path.join(target, entry.name);
    if (entry.isDirectory()) {
      files.push(...await walkJsonFiles(child));
    } else if (entry.isFile() && entry.name.endsWith('.json')) {
      files.push(child);
    }
  }
  return files;
}

async function readJson(file) {
  try {
    return JSON.parse(await fs.readFile(file, 'utf8'));
  } catch {
    return null;
  }
}

function isObject(value) {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function finiteMs(value) {
  const number = typeof value === 'number' ? value : Number(value);
  return Number.isFinite(number) && number >= 0 ? number : null;
}

function firstFiniteMs(...values) {
  for (const value of values) {
    const parsed = finiteMs(value);
    if (parsed !== null) return parsed;
  }
  return null;
}

function normalizeStage(stage, index) {
  if (!isObject(stage)) return null;
  const name = String(stage.stage || stage.name || stage.metric || '').trim();
  const ms = finiteMs(stage.ms);
  if (!name || ms === null) return null;
  return {
    stage: name,
    metric: String(stage.metric || name),
    ms,
    scope: typeof stage.scope === 'string'
      ? stage.scope
      : typeof stage.measurement_scope === 'string'
      ? stage.measurement_scope
      : null,
    source: typeof stage.source === 'string' ? stage.source : null,
    start_at_unix_ms: stage.start_at_unix_ms ?? null,
    end_at_unix_ms: stage.end_at_unix_ms ?? null,
    note: typeof stage.note === 'string' ? stage.note : null,
    order: Number.isInteger(stage.order) ? stage.order : index,
  };
}

function wallClockFromValue(file, value, jsonPath) {
  if (!isObject(value) || !isObject(value.wall_clock)) {
    return null;
  }
  const wallClock = value.wall_clock;
  if (wallClock.schema && wallClock.schema !== EXPLICIT_WALL_CLOCK_SCHEMA) {
    return null;
  }
  const rawStages = Array.isArray(wallClock.stages)
    ? wallClock.stages
    : Array.isArray(wallClock.stage_measurements)
    ? wallClock.stage_measurements
    : [];
  const stages = rawStages
    .map((stage, index) => normalizeStage(stage, index))
    .filter(Boolean)
    .sort((a, b) => a.order - b.order);
  const totalMs = firstFiniteMs(
    wallClock.click_to_certified_receipt_ms,
    wallClock.total_ms,
  );
  if (totalMs === null) {
    return null;
  }
  return {
    schema: 'postfiat-zk-latency-additive-record-v1',
    source_file: file,
    source_json_path: `${jsonPath}.wall_clock`,
    run_index: value.run_index ?? wallClock.run_index ?? null,
    direction: value.direction || wallClock.direction || 'unknown',
    circuit: wallClock.circuit || 'swap',
    measurement: wallClock.measurement || 'click_to_certified_receipt',
    run_label: wallClock.run_label || wallClock.service_warmth || 'unspecified',
    service_warmth: wallClock.service_warmth || wallClock.run_label || 'unspecified',
    boundary: wallClock.boundary || null,
    total_metric: wallClock.total_metric || 'click_to_certified_receipt_ms',
    total_ms: totalMs,
    total_start_at_unix_ms: wallClock.clicked_at_unix_ms ?? wallClock.start_at_unix_ms ?? null,
    total_end_at_unix_ms: wallClock.certified_receipt_at_unix_ms ?? wallClock.end_at_unix_ms ?? null,
    ui_completion_ms: firstFiniteMs(wallClock.click_to_ui_complete_ms),
    stages,
  };
}

function collectExplicitTimingRecords(file, json) {
  const records = [];
  function visit(value, jsonPath = '$') {
    if (Array.isArray(value)) {
      value.forEach((child, index) => visit(child, `${jsonPath}[${index}]`));
      return;
    }
    if (!isObject(value)) return;
    const record = wallClockFromValue(file, value, jsonPath);
    if (record) records.push(record);
    for (const [key, child] of Object.entries(value)) {
      if (key === 'wall_clock') continue;
      visit(child, `${jsonPath}.${key}`);
    }
  }
  visit(json);
  return records;
}

function validateAdditiveRecords(records, toleranceMs = ADDITIVE_TOLERANCE_MS) {
  const failures = [];
  for (const record of records) {
    if (!Array.isArray(record.stages) || record.stages.length === 0) {
      failures.push({
        source_file: record.source_file,
        source_json_path: record.source_json_path,
        code: 'missing_explicit_stages',
        message: 'explicit wall-clock record has no non-overlapping stages',
      });
      continue;
    }
    const stageSumMs = record.stages.reduce((sum, stage) => sum + stage.ms, 0);
    const deltaMs = Math.abs(stageSumMs - record.total_ms);
    if (deltaMs > toleranceMs) {
      failures.push({
        source_file: record.source_file,
        source_json_path: record.source_json_path,
        code: 'non_additive_stage_total',
        message: `stage sum ${stageSumMs.toFixed(3)}ms does not match total ${record.total_ms.toFixed(3)}ms`,
        stage_sum_ms: stageSumMs,
        total_ms: record.total_ms,
        delta_ms: deltaMs,
        tolerance_ms: toleranceMs,
      });
    }
    for (const stage of record.stages) {
      if (stage.stage === 'proof' && stage.scope !== 'halo2_proof_generation') {
        failures.push({
          source_file: record.source_file,
          source_json_path: record.source_json_path,
          code: 'polluted_proof_stage',
          message: 'proof stage is allowed only for direct Halo2 proof generation timing',
          stage,
        });
      }
    }
  }
  return {
    ok: failures.length === 0,
    tolerance_ms: toleranceMs,
    checked_records: records.length,
    failures,
  };
}

function rowsFromRecords(records) {
  const rows = [];
  for (const record of records) {
    for (const stage of record.stages) {
      rows.push({
        circuit: record.circuit,
        measurement: record.measurement,
        direction: record.direction,
        run_index: record.run_index,
        run_label: record.run_label,
        service_warmth: record.service_warmth,
        stage: stage.stage,
        metric: stage.metric,
        ms: stage.ms,
        scope: stage.scope,
        source: stage.source,
        source_file: record.source_file,
        source_json_path: record.source_json_path,
      });
    }
    rows.push({
      circuit: record.circuit,
      measurement: record.measurement,
      direction: record.direction,
      run_index: record.run_index,
      run_label: record.run_label,
      service_warmth: record.service_warmth,
      stage: 'total',
      metric: record.total_metric,
      ms: record.total_ms,
      scope: 'click_to_certified_receipt_wall_clock',
      source: record.boundary,
      source_file: record.source_file,
      source_json_path: record.source_json_path,
    });
  }
  return rows.sort((a, b) => (
    `${a.measurement}:${a.direction}:${a.run_index}:${a.stage}:${a.source_file}`
      .localeCompare(`${b.measurement}:${b.direction}:${b.run_index}:${b.stage}:${b.source_file}`)
  ));
}

function summarize(rows) {
  const summary = new Map();
  for (const row of rows) {
    const key = [
      row.circuit,
      row.measurement,
      row.direction,
      row.service_warmth,
      row.stage,
    ].join('\u0000');
    const current = summary.get(key) || {
      circuit: row.circuit,
      measurement: row.measurement,
      direction: row.direction,
      service_warmth: row.service_warmth,
      stage: row.stage,
      count: 0,
      total_ms: 0,
      max_ms: 0,
    };
    current.count += 1;
    current.total_ms += row.ms;
    current.max_ms = Math.max(current.max_ms, row.ms);
    summary.set(key, current);
  }
  return Array.from(summary.values())
    .map((entry) => ({
      ...entry,
      avg_ms: entry.count > 0 ? entry.total_ms / entry.count : 0,
    }))
    .sort((a, b) => (
      `${a.circuit}:${a.measurement}:${a.direction}:${a.service_warmth}:${a.stage}`
        .localeCompare(`${b.circuit}:${b.measurement}:${b.direction}:${b.service_warmth}:${b.stage}`)
    ));
}

async function buildReport(args) {
  const inputFiles = [];
  for (const input of args.inputs) {
    const resolved = path.resolve(input);
    if (await exists(resolved)) {
      inputFiles.push(...await walkJsonFiles(resolved));
    }
  }
  inputFiles.sort();

  const records = [];
  for (const file of inputFiles) {
    const json = await readJson(file);
    if (json !== null) {
      records.push(...collectExplicitTimingRecords(file, json));
    }
  }
  records.sort((a, b) => (
    `${a.measurement}:${a.direction}:${a.run_index}:${a.source_file}:${a.source_json_path}`
      .localeCompare(`${b.measurement}:${b.direction}:${b.run_index}:${b.source_file}:${b.source_json_path}`)
  ));
  const additive = validateAdditiveRecords(records);
  const rows = rowsFromRecords(records);
  return {
    ok: additive.ok,
    schema: 'postfiat-zk-latency-phase0-report-v2',
    label: args.label,
    generated_at_unix_ms: Date.now().toString(),
    live_rounds_started: false,
    input_count: args.inputs.length,
    scanned_json_files: inputFiles.length,
    explicit_record_count: records.length,
    row_count: rows.length,
    timing_policy: {
      implicit_recursive_ms_scrape: false,
      proof_stage_scope_required: 'halo2_proof_generation',
      additive_stage_tolerance_ms: ADDITIVE_TOLERANCE_MS,
    },
    additive_invariant: additive,
    records,
    rows,
    summary: summarize(rows),
  };
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const report = await buildReport(args);
  const serialized = `${JSON.stringify(report, null, 2)}\n`;
  if (args.out) {
    await fs.mkdir(path.dirname(path.resolve(args.out)), { recursive: true });
    await fs.writeFile(args.out, serialized);
  } else {
    process.stdout.write(serialized);
  }
}

const currentFile = fileURLToPath(import.meta.url);
if (process.argv[1] && path.resolve(process.argv[1]) === currentFile) {
  main().catch((error) => {
    console.error(error.stack || error.message);
    process.exit(1);
  });
}

export {
  ADDITIVE_TOLERANCE_MS,
  buildReport,
  collectExplicitTimingRecords,
  rowsFromRecords,
  summarize,
  validateAdditiveRecords,
};
