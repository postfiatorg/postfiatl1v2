import test from 'node:test';
import assert from 'node:assert/strict';
import { pfusdcWithdrawalCapacity, preparePfusdcWithdrawal, recoverablePfusdcWithdrawal } from './pfusdc-withdrawal.js';
import { PFUSDC_ASSET_ID } from './utils.js';

const route = { profileHash: '11'.repeat(48), profile: { source_chain_id: 1, vault_address: `0x${'22'.repeat(20)}`, token_address: `0x${'33'.repeat(20)}` } };
const status = { asset_id: PFUSDC_ASSET_ID, issuer: `pf${'44'.repeat(20)}`, finalized_epoch: 51, finalized_reserve_packet_hash: '55'.repeat(48), buckets: [{ bucket_id: '66'.repeat(48), source_domain: `erc20_bridge_vault:1:0x${'22'.repeat(20)}:0x${'33'.repeat(20)}`, policy_hash: '11'.repeat(48), status: 'active', outstanding_vault_bridge_atoms: 2_000_000 }] };

test('prepares an exact mainnet pfUSDC withdrawal from the unique governed bucket', () => {
  assert.equal(pfusdcWithdrawalCapacity({ status, route }).amountAtoms, 2_000_000n);
  const operation = preparePfusdcWithdrawal({ status, route, owner: `pf${'77'.repeat(20)}`, ethereumRecipient: `0x${'88'.repeat(20)}`, amountAtoms: 1_000_000 });
  assert.equal(operation.operation, 'vault_bridge_burn_to_redeem');
  assert.equal(operation.destination_ref, `evm-erc20:1:0x${'88'.repeat(20)}`);
  assert.equal(operation.bucket_id, '66'.repeat(48));
  assert.equal(operation.amount_atoms, 1_000_000);
});

test('fails closed when no unique active deployment-bound reserve bucket exists', () => {
  assert.throws(() => preparePfusdcWithdrawal({ status: { ...status, buckets: [] }, route, owner: `pf${'77'.repeat(20)}`, ethereumRecipient: `0x${'88'.repeat(20)}`, amountAtoms: 1 }), /No active Ethereum reserve/);
});

test('fails before signing when the active route cannot cover the requested amount', () => {
  assert.throws(() => preparePfusdcWithdrawal({ status, route, owner: `pf${'77'.repeat(20)}`, ethereumRecipient: `0x${'88'.repeat(20)}`, amountAtoms: 2_000_001 }), /currently available/);
});

test('source-series enforcement requires and exposes the active bucket series', () => {
  const sourceSeriesId = '99'.repeat(48);
  const enforced = {
    ...status,
    source_series_enforced: true,
    buckets: [{ ...status.buckets[0], source_series_id: sourceSeriesId }],
  };
  assert.equal(
    pfusdcWithdrawalCapacity({ status: enforced, route }).bucket.source_series_id,
    sourceSeriesId,
  );
  assert.throws(
    () => pfusdcWithdrawalCapacity({
      status: { ...enforced, buckets: [{ ...enforced.buckets[0], source_series_id: '' }] },
      route,
    }),
    /no valid source-series identity/,
  );
});

test('recovers only an unregistered pending burn bound to the current governed bucket', () => {
  const owner = `pf${'77'.repeat(20)}`;
  const currentBurn = '99'.repeat(48);
  const recoveryStatus = { ...status, redemptions: [
    { state: 'pending', owner, bucket_id: 'aa'.repeat(48), burn_tx_id: 'bb'.repeat(48), withdrawal_recipient: `0x${'cc'.repeat(20)}`, amount_atoms: 9_932_863, created_at_height: 1_000 },
    { state: 'settled', owner, bucket_id: '66'.repeat(48), burn_tx_id: 'dd'.repeat(48), withdrawal_recipient: `0x${'ee'.repeat(20)}`, amount_atoms: 1, created_at_height: 1_001 },
    { state: 'pending', owner, bucket_id: '66'.repeat(48), burn_tx_id: currentBurn, withdrawal_recipient: `0x${'88'.repeat(20)}`, amount_atoms: 481_552, created_at_height: 999 },
  ] };
  const recovery = recoverablePfusdcWithdrawal({ status: recoveryStatus, route, owner, jobs: [] });
  assert.deepEqual(recovery, {
    burn_tx_id: currentBurn,
    owner,
    ethereum_recipient: `0x${'88'.repeat(20)}`,
    amount_atoms: '481552',
    asset_id: PFUSDC_ASSET_ID,
  });
  assert.equal(recoverablePfusdcWithdrawal({ status: recoveryStatus, route, owner, jobs: [{ request: { burn_tx_id: currentBurn } }] }), null);
});
