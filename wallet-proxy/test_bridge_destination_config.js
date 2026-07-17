'use strict';

// P0-WALLET-BRIDGE-DEST-01: the relay must obtain every money-destination
// field from authenticated chain state and reject wallet/event substitution.

const assert = require('assert');

process.env.VAULT_BRIDGE_VAULT_ADDRESS = '0x9999999999999999999999999999999999999999';
process.env.VAULT_BRIDGE_VAULT_CODE_HASH = `0x${'99'.repeat(32)}`;
process.env.VAULT_BRIDGE_TOKEN_ADDRESS = '0x8888888888888888888888888888888888888888';
process.env.VAULT_BRIDGE_POLICY_HASH = '77'.repeat(48);

const {
  assertVaultBridgeEvidenceMatches,
  governedVaultBridgeRelayConfig,
  vaultBridgeRelayConfig,
} = require('./server');

const profileHash = '55'.repeat(48);
const routeBinding = '73e568bb40a10a8d5cf37e7d59c608eef2479b643e4c928421b8c94d7fe6d365';
const vault = '0x1111111111111111111111111111111111111111';
const token = '0xaf88d065e77c8cc2239327c5edb3a432268e5831';
const txHash = `0x${'aa'.repeat(32)}`;

function routeReport(assetId) {
  return {
    schema: 'postfiat.vault_bridge.route_report.v1',
    current_height: 120,
    profile_hash: profileHash,
    route_binding: routeBinding,
    governance_route_epoch: 7,
    nav_profile_policy_hash: profileHash,
    active: true,
    profile: {
      schema: 'postfiat.vault_bridge.route_profile.v1',
      asset_id: assetId,
      source_chain_id: 42161,
      vault_address: vault,
      vault_runtime_code_hash: `0x${'33'.repeat(32)}`,
      token_address: token,
      token_runtime_code_hash: `0x${'44'.repeat(32)}`,
      route_epoch: 7,
      activation_height: 100,
      expires_at_height: 500,
    },
  };
}

async function main() {
  const base = vaultBridgeRelayConfig();
  assert.strictEqual(base.vault_address, undefined);
  assert.strictEqual(base.token_address, undefined);
  assert.strictEqual(base.policy_hash, undefined);

  const rpcRequest = async (_host, _port, request) => {
    assert.strictEqual(request.method, 'vault_bridge_route');
    assert.deepStrictEqual(request.params, { asset_id: base.asset_id });
    return { ok: true, result: routeReport(base.asset_id) };
  };
  const governed = await governedVaultBridgeRelayConfig(base, rpcRequest);
  assert.strictEqual(governed.vault_address, vault);
  assert.strictEqual(governed.token_address, token);
  assert.strictEqual(governed.policy_hash, profileHash);
  assert.strictEqual(governed.route_epoch, 7);
  assert.strictEqual(governed.route_binding, routeBinding);

  const body = {
    route_profile_hash: profileHash,
    route_epoch: 7,
    route_binding: routeBinding,
    pftl_recipient: 'pf-recipient',
    depositor: '0x2222222222222222222222222222222222222222',
    deposit_id: 'bb'.repeat(32),
    amount_atoms: '100',
  };
  const evidence = {
    source_chain_id: 42161,
    vault_address: vault,
    token_address: token,
    tx_hash: txHash,
    pftl_recipient: body.pftl_recipient,
    depositor: body.depositor,
    deposit_id: body.deposit_id,
    amount_atoms: 100,
    route_binding: routeBinding,
  };
  assert.doesNotThrow(() => assertVaultBridgeEvidenceMatches(evidence, body, governed, txHash));

  for (const [field, value] of [
    ['route_profile_hash', '66'.repeat(48)],
    ['route_epoch', 8],
    ['route_binding', '77'.repeat(32)],
  ]) {
    assert.throws(
      () => assertVaultBridgeEvidenceMatches(evidence, { ...body, [field]: value }, governed, txHash),
      /active governed route/,
    );
  }
  assert.throws(
    () => assertVaultBridgeEvidenceMatches(
      { ...evidence, route_binding: '88'.repeat(32) },
      body,
      governed,
      txHash,
    ),
    /event route binding/,
  );

  console.log('P0-WALLET-BRIDGE-DEST-01 governed backend regression passed');
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
