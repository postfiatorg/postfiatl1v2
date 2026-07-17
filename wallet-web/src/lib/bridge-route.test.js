import test from 'node:test';
import assert from 'node:assert/strict';
import {
  loadGovernedVaultBridgeRoute,
  parseGovernedVaultBridgeRoute,
  vaultBridgeRouteProfileHash,
} from './bridge-route.js';
import { governedRouteBinding } from './evm.js';

const ASSET = '11'.repeat(48);
const GENESIS = '22'.repeat(48);

function response(overrides = {}, rebind = false) {
  const profile = {
    schema: 'postfiat.vault_bridge.route_profile.v1',
    route_id: 'arbitrum-pfusdc',
    asset_id: ASSET,
    source_chain_id: 42161,
    vault_address: '0x1111111111111111111111111111111111111111',
    vault_runtime_code_hash: `0x${'33'.repeat(32)}`,
    token_address: '0xaf88d065e77c8cc2239327c5edb3a432268e5831',
    token_runtime_code_hash: `0x${'44'.repeat(32)}`,
    route_epoch: 7,
    verifier_kind: 'multi-fetch-quorum',
    evidence_tier: 'independently-observed',
    verifier_policy_hash: '',
    verifier_program_vkey: '',
    verifier_proof_encoding: '',
    max_proof_bytes: 0,
    max_public_values_bytes: 0,
    max_snapshot_age_blocks: 100,
    challenge_window_blocks: 10,
    max_epoch_gap_blocks: 100,
    settle_deadline_blocks: 100,
    min_challenge_bond: 1,
    min_attestations: 5,
    minimum_confirmations: 64,
    activation_height: 100,
    expires_at_height: 500,
  };
  const presentedProfile = { ...profile, ...(overrides.profile || {}) };
  const authenticatedProfile = rebind ? presentedProfile : profile;
  const profileHash = vaultBridgeRouteProfileHash(authenticatedProfile);
  const result = {
    schema: 'postfiat.vault_bridge.route_report.v1',
    chain_id: 'postfiat-test',
    genesis_hash: GENESIS,
    current_height: 120,
    profile: presentedProfile,
    profile_hash: profileHash,
    route_binding: governedRouteBinding(profileHash, authenticatedProfile.route_epoch).slice(2),
    governance_route_epoch: 7,
    nav_profile_verifier_kind: authenticatedProfile.verifier_kind,
    nav_profile_policy_hash: profileHash,
    active: true,
  };
  return { ok: true, result: { ...result, ...overrides, profile: presentedProfile } };
}

const expected = {
  assetId: ASSET,
  chainId: 'postfiat-test',
  genesisHash: GENESIS,
  sourceChainId: 42161,
  tokenAddress: '0xaf88d065e77c8cC2239327C5EDb3A432268e5831',
};

test('accepts the complete active chain-authenticated route profile', async () => {
  const rpc = { vaultBridgeRoute: async (assetId) => (assert.equal(assetId, ASSET), response()) };
  const route = await loadGovernedVaultBridgeRoute(rpc, expected);
  assert.equal(route.routeEpoch, 7);
  assert.equal(route.routeBinding, governedRouteBinding(route.profileHash, route.routeEpoch));
  assert.equal(route.remainingBlocks, 380);
  assert.equal(route.evidenceTier, 'independently-observed');
});

test('fails closed on route substitution, stale route, downgrade, and retired vault', () => {
  assert.throws(() => parseGovernedVaultBridgeRoute(response({ profile_hash: '66'.repeat(48) }), expected), /preimage does not match/);
  assert.throws(() => parseGovernedVaultBridgeRoute(response({ route_binding: '00'.repeat(32) }), expected), /binding does not match/);
  assert.throws(() => parseGovernedVaultBridgeRoute(response({ current_height: 500 }), expected), /inactive or expired/);
  assert.throws(() => parseGovernedVaultBridgeRoute(
    response({ profile: { evidence_tier: 'receipt-proven' } }, true),
    expected,
  ), /evidence tier/);
  assert.throws(() => parseGovernedVaultBridgeRoute(response({
    profile: { vault_address: '0x9999999999999999999999999999999999999999' },
  }), expected), /preimage does not match/);
  assert.throws(() => parseGovernedVaultBridgeRoute(response({
    profile: { challenge_window_blocks: 11 },
  }), expected), /preimage does not match/);
  assert.throws(() => parseGovernedVaultBridgeRoute(response({
    profile: { verifier_program_vkey: `0x${'99'.repeat(32)}` },
  }), expected), /preimage does not match/);
  assert.throws(() => parseGovernedVaultBridgeRoute(response({
    profile: { vault_address: '0x1a15e6103d6af4e88924f748e13b829d3948dea9' },
  }), expected), /retired vault/);
});

test('same route API accepts a hash-bound receipt-proven verifier profile', () => {
  const stronger = response({
    profile: {
      verifier_kind: 'sp1-groth16',
      evidence_tier: 'receipt-proven',
      verifier_policy_hash: '55'.repeat(32),
      verifier_program_vkey: `0x${'66'.repeat(32)}`,
      verifier_proof_encoding: 'groth16',
      min_attestations: 0,
      minimum_confirmations: 0,
    },
  }, true);
  const route = parseGovernedVaultBridgeRoute(stronger, expected);
  assert.equal(route.verifierKind, 'sp1-groth16');
  assert.equal(route.evidenceTier, 'receipt-proven');
  assert.equal(route.profileHash, vaultBridgeRouteProfileHash(route.profile));
});

test('receipt-proven route fails closed on an incomplete verifier contract', () => {
  for (const profile of [
    { verifier_policy_hash: '' },
    { verifier_program_vkey: '' },
    { verifier_proof_encoding: 'plonk' },
    { minimum_confirmations: 1 },
  ]) {
    assert.throws(() => parseGovernedVaultBridgeRoute(response({
      profile: {
        verifier_kind: 'sp1-groth16',
        evidence_tier: 'receipt-proven',
        verifier_policy_hash: '55'.repeat(32),
        verifier_program_vkey: `0x${'66'.repeat(32)}`,
        verifier_proof_encoding: 'groth16',
        min_attestations: 0,
        minimum_confirmations: 0,
        ...profile,
      },
    }, true), expected));
  }
});

test('fails closed on wrong chain, genesis, source network, token, and asset', () => {
  for (const changed of [
    { chain_id: 'wrong' },
    { genesis_hash: '00'.repeat(48) },
    { profile: { source_chain_id: 1 } },
    { profile: { token_address: '0x2222222222222222222222222222222222222222' } },
    { profile: { asset_id: '77'.repeat(48) } },
  ]) {
    assert.throws(() => parseGovernedVaultBridgeRoute(response(changed), expected));
  }
});
