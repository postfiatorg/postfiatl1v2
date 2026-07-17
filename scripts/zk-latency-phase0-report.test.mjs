import test from 'node:test';
import assert from 'node:assert/strict';

import {
  collectExplicitTimingRecords,
  rowsFromRecords,
  validateAdditiveRecords,
} from './zk-latency-phase0-report.mjs';

function fixtureRecord(overrides = {}) {
  return {
    run_index: 1,
    direction: 'a651->a652',
    wall_clock: {
      schema: 'postfiat-wallet-private-swap-click-receipt-wall-clock-v1',
      measurement: 'click_to_certified_receipt',
      service_warmth: 'cold_fresh_service_first_swap',
      click_to_certified_receipt_ms: 35,
      stages: [
        { stage: 'local_action', ms: 10, source: '/asset-orchard/swap-actions' },
        { stage: 'proxy_certified_receipt', ms: 20, source: '/api/shielded-nav-swap/swap' },
        { stage: 'browser_response_settle', ms: 5, source: 'browser fetch capture' },
      ],
      ...overrides,
    },
  };
}

test('phase0 latency report accepts explicit additive wall-clock records', () => {
  const records = collectExplicitTimingRecords('/tmp/run-1-evidence.json', fixtureRecord());
  assert.equal(records.length, 1);
  assert.equal(validateAdditiveRecords(records).ok, true);
  const rows = rowsFromRecords(records);
  assert.equal(rows.filter(row => row.stage === 'total').length, 1);
  assert.equal(rows.find(row => row.stage === 'total').ms, 35);
});

test('phase0 latency report rejects non-additive stage totals', () => {
  const records = collectExplicitTimingRecords('/tmp/run-1-evidence.json', fixtureRecord({
    click_to_certified_receipt_ms: 25,
  }));
  const invariant = validateAdditiveRecords(records);
  assert.equal(invariant.ok, false);
  assert.equal(invariant.failures[0].code, 'non_additive_stage_total');
});

test('phase0 latency report rejects proof buckets that are not Halo2 generation', () => {
  const records = collectExplicitTimingRecords('/tmp/run-1-evidence.json', fixtureRecord({
    click_to_certified_receipt_ms: 35,
    stages: [
      { stage: 'proof', ms: 35, scope: 'certified_round_wait' },
    ],
  }));
  const invariant = validateAdditiveRecords(records);
  assert.equal(invariant.ok, false);
  assert.equal(invariant.failures[0].code, 'polluted_proof_stage');
});
