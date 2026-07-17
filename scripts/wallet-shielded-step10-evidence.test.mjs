import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';

import {
  buildNoPrivateMaterialRequestLog,
  buildStep10EvidenceSummary,
  scanPrivateMaterial,
} from './lib/wallet-shielded-step10-evidence.mjs';

function writeJson(path, value) {
  writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`);
}

function readJson(path, fallback = undefined) {
  try {
    return JSON.parse(readFileSync(path, 'utf8'));
  } catch (error) {
    if (fallback !== undefined && error && error.code === 'ENOENT') return fallback;
    throw error;
  }
}

function syntheticRun(index, direction) {
  return {
    run_index: index,
    direction,
    quote: { ok: true },
    local_action: { ok: true },
    relay: { ok: true },
    finalize: { ok: true },
    wire_privacy: { ok: true },
    assertions: {
      wallet_input_spent: true,
      expected_wallet_input_spent: true,
      wallet_output_spendable: true,
      pool_input_spent: true,
      zero_repair: true,
    },
  };
}

test('Step 10 request log rejects nested private material without echoing sensitive values', () => {
  const entries = [{
    at: '2026-07-02T00:00:00Z',
    url: 'https://wallet.example/api/shielded-nav-swap/swap',
    status: 200,
    ok: true,
    request: {
      route: 'shielded_navswap',
      wallet_address: 'pfabc',
      swap_action_json: JSON.stringify({
        schema: 'postfiat-asset-orchard-swap-action-v1',
        wallet_note: { seed_hex: 'should-not-leak' },
      }),
      user_supplied: 'seed-value-1234567890',
    },
  }];
  const log = buildNoPrivateMaterialRequestLog({
    entries,
    runIndex: 1,
    sensitiveLabels: [{ label: 'wallet_seed', value: 'seed-value-1234567890' }],
  });
  assert.equal(log.ok, false);
  assert.equal(log.scanned_request_count, 1);
  assert.ok(log.hits.some(hit => hit.type === 'forbidden_private_key' && hit.key === 'wallet_note'));
  assert.ok(log.hits.some(hit => hit.type === 'sensitive_value' && hit.label === 'wallet_seed'));
  assert.equal(JSON.stringify(log).includes('seed-value-1234567890'), false);
});

test('Step 10 request log accepts public proxy request material', () => {
  const cleanAction = {
    schema: 'postfiat-asset-orchard-swap-action-v1',
    pool_id: 'asset-orchard-v1',
    nullifiers: ['11'.repeat(32)],
    output_commitments: ['22'.repeat(32), '33'.repeat(32)],
    proof: 'aa',
  };
  const log = buildNoPrivateMaterialRequestLog({
    entries: [{
      at: '2026-07-02T00:00:00Z',
      url: 'https://wallet.example/api/shielded-nav-swap/swap',
      status: 200,
      ok: true,
      request: {
        route: 'shielded_navswap',
        wallet_address: 'pfabc',
        swap_action_json: JSON.stringify(cleanAction),
      },
    }],
    runIndex: 1,
  });
  assert.equal(log.ok, true);
  assert.deepEqual(scanPrivateMaterial(cleanAction), []);
});

test('Step 10 scanner rejects every Asset-Orchard note-opening and spend-authority field', () => {
  const privateMaterial = {
    diversifier: '01'.repeat(11),
    g_d: '02'.repeat(32),
    pk_d: '03'.repeat(32),
    psi: '04'.repeat(32),
    nk: '05'.repeat(32),
    rivk: '06'.repeat(32),
    spend_auth_signing_key: '07'.repeat(32),
    full_viewing_key_hex: '08'.repeat(32),
  };
  const hits = scanPrivateMaterial(privateMaterial);
  for (const key of Object.keys(privateMaterial)) {
    assert.ok(
      hits.some(hit => hit.type === 'forbidden_private_key' && hit.key === key),
      `missing private-material rejection for ${key}`,
    );
  }
});

test('Step 10 package summary requires two matching run summaries, request logs, and reload proof', () => {
  const dir = mkdtempSync(join(tmpdir(), 'postfiat-step10-package-'));
  writeJson(join(dir, 'run-1-evidence.json'), syntheticRun(1, 'a651->a652'));
  writeJson(join(dir, 'run-2-evidence.json'), syntheticRun(2, 'a652->a651'));
  writeJson(join(dir, 'run-1-no-private-material-request-log.json'), { ok: true });
  writeJson(join(dir, 'run-2-no-private-material-request-log.json'), { ok: true });
  writeJson(join(dir, 'reload-rescan-proof.json'), { ok: true });
  writeJson(join(dir, 'stakehub-operator-demo-command.json'), {
    command: 'python3 scripts/shielded-nav-swap-e2e-live.py --report-dir docs/evidence/stakehub-shielded-navswap-step10-test',
    evidence_slot: 'docs/evidence/stakehub-shielded-navswap-step10-<ts>',
  });
  const summary = buildStep10EvidenceSummary({
    evidenceDir: dir,
    files: { readJson },
  });
  assert.equal(summary.package_ok, true);
  assert.equal(summary.run_count, 2);
  assert.deepEqual(summary.directions, ['a651->a652', 'a652->a651']);
  assert.equal(summary.canonical_pair, true);
  assert.equal(summary.identical_pass_fail_summary_fields, true);
  assert.equal(summary.operator_approval_required_for_can_run, true);
});
