import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import test from 'node:test';
import { canonicalFastSwapIntentBytes, bytesToHex } from './fastswap-v1.js';

const fill = (byte, length) => byte.toString(16).padStart(2, '0').repeat(length);
const key = (byte) => ({ object_id: fill(byte, 32), version: 1 });

function party(ownerAddress, publicKeyByte, offeredByte, receivedByte) {
  return {
    owner_address: ownerAddress,
    owner_pubkey: fill(publicKeyByte, 64),
    offered_asset_id: fill(offeredByte, 48),
    offered_asset_rule_hash: fill(offeredByte + 10, 48),
    offered_amount: 8,
    receives_asset_id: fill(receivedByte, 48),
    receives_asset_rule_hash: fill(receivedByte + 10, 48),
    receives_holder_permit_id: null,
    receives_amount: 1,
    asset_inputs: [key(publicKeyByte)],
    fee_inputs: [key(publicKeyByte + 20)],
    asset_change: 2,
    fee_change: 9,
    fee_burn_pft: 1,
  };
}

function conformanceIntent() {
  const party0 = party('pf-a', 1, 1, 2);
  const party1 = party('pf-b', 2, 2, 1);
  party1.offered_amount = party0.receives_amount;
  party1.receives_amount = party0.offered_amount;
  party0.receives_asset_rule_hash = party1.offered_asset_rule_hash;
  party1.receives_asset_rule_hash = party0.offered_asset_rule_hash;
  return {
    domain: {
      chain: { chain_id: 'postfiat-test', genesis_hash: fill(3, 48), protocol_version: 1 },
      fastswap_schema_version: 1,
      committee_epoch: 7,
      committee_root: fill(4, 48),
      validator_count: 6,
      quorum: 5,
    },
    policy_hash: fill(5, 48),
    rfq_hash: fill(6, 48),
    market_envelope_hash: fill(7, 48),
    nav_epoch: 59,
    expires_at_height: 100,
    nonce: fill(8, 32),
    party_0: party0,
    party_1: party1,
  };
}

test('JS FastSwap encoder matches the frozen Rust conformance vector', () => {
  const canonical = canonicalFastSwapIntentBytes(conformanceIntent());
  assert.equal(canonical.length, 1127);
  const digest = createHash('sha3-384')
    .update('postfiat.fastswap.intent_id.v1')
    .update(Uint8Array.of(0))
    .update(canonical)
    .digest('hex');
  assert.equal(digest, 'b66cf7d768f3cb0a39f278ab1332dad09f6b4951efb40f45aa72c9cab37f3c2d5a5c41291834ac39ac7c57034370bf17');
  assert.ok(bytesToHex(canonical).startsWith('50464641535453574150494e54454e54'));
});

test('JS FastSwap encoder fails closed on malleable party and object order', () => {
  const swapped = conformanceIntent();
  [swapped.party_0, swapped.party_1] = [swapped.party_1, swapped.party_0];
  assert.throws(() => canonicalFastSwapIntentBytes(swapped), /party order/);

  const duplicate = conformanceIntent();
  duplicate.party_0.asset_inputs.push(duplicate.party_0.asset_inputs[0]);
  assert.throws(() => canonicalFastSwapIntentBytes(duplicate), /sorted and unique/);
});
