import test from 'node:test';
import assert from 'node:assert/strict';

import {
  createPnokFixClientRequestId,
  formatAssetAtoms,
  verifyPnokFixQuote,
} from './pnok-fix.js';

const BASE = '01'.repeat(48);
const QUOTE = '02'.repeat(48);

function fixture() {
  const readiness = {
    schema: 'postfiat-pnok-private-fix-wallet-readiness-v1',
    execution_privacy: 'private on PFTL',
    source_boundary: 'controlled sandbox checkpoint',
    base_asset_id: BASE,
    quote_asset_id: QUOTE,
    base_atoms: '20000000',
    quote_atoms: '210',
    ratio_numerator: 21,
    ratio_denominator: 2_000_000,
  };
  const packet = {
    packet_hash: '03'.repeat(48),
    base_asset_id: BASE,
    quote_asset_id: QUOTE,
    source_label: 'pnok_demo_fix',
    ratio_numerator: 21,
    ratio_denominator: 2_000_000,
    band_bps: 0,
    fee_bps: 0,
    minimum_base_atoms: 20_000_000,
    capacity_base_atoms: 20_000_000 * 19,
    capacity_quote_atoms: 210 * 19,
    max_fills: 19,
    expires_at_height: 900,
  };
  const list = { ok: true, result: { fixes: [{ status: 'active', remaining_fill_slots: 19, state: { packet } }] } };
  const quote = { ok: true, result: {
    fix_packet_hash: packet.packet_hash,
    base_asset: { asset_id: BASE }, quote_asset: { asset_id: QUOTE },
    base_atoms: 20_000_000, quote_atoms: 210, exact_division: true,
    fee_atoms: 0, price_impact_bps: 0, source_label: 'pnok_demo_fix', current_height: 800,
  } };
  return { readiness, list, quote };
}

test('formats asset atoms without floating point', () => {
  assert.equal(formatAssetAtoms('20000000', 6), '20.000000');
  assert.equal(formatAssetAtoms('210', 0), '210');
});

test('verifies the exact active pNOK demo FIX and quote', () => {
  const { readiness, list, quote } = fixture();
  const verified = verifyPnokFixQuote(readiness, list, quote);
  assert.equal(verified.quote.quote_atoms, 210);
});

test('fails closed when the backend quote differs by one atom', () => {
  const { readiness, list, quote } = fixture();
  quote.result.quote_atoms = 209;
  assert.throws(() => verifyPnokFixQuote(readiness, list, quote), /recomputation/);
});

test('fails closed when max_fills exceeds aggregate atom capacity', () => {
  const { readiness, list, quote } = fixture();
  list.result.fixes[0].state.packet.capacity_base_atoms = 20_000_000;
  list.result.fixes[0].state.packet.capacity_quote_atoms = 210;
  assert.throws(() => verifyPnokFixQuote(readiness, list, quote), /exactly one active/);
});

test('generates bounded durable request IDs', () => {
  const cryptoApi = { getRandomValues(bytes) { bytes.fill(0xab); return bytes; } };
  assert.equal(createPnokFixClientRequestId(cryptoApi), `pnok-wallet-${'ab'.repeat(12)}`);
});
