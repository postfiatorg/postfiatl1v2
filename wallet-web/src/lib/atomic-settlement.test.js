import assert from 'node:assert/strict';
import test from 'node:test';

import {
  findAtomicWalletCancelLeg,
  findAtomicWalletCreateLeg,
  findAtomicWalletFinishLeg,
} from './atomic-settlement.js';

function templateFixture() {
  return {
    schema: 'postfiat-atomic-settlement-template-v1',
    left: {
      owner: 'pf-left',
      recipient: 'pf-right',
      escrow_id: 'escrow-left',
      sequence: 7,
      transaction_kind: 'escrow_create',
      operation: {
        operation: 'escrow_create',
        owner: 'pf-left',
        recipient: 'pf-right',
        asset_id: 'PFT',
        amount: 1,
      },
    },
    right: {
      owner: 'pf-right',
      recipient: 'pf-left',
      escrow_id: 'escrow-right',
      sequence: 9,
      transaction_kind: 'escrow_create',
      operation: {
        operation: 'escrow_create',
        owner: 'pf-right',
        recipient: 'pf-left',
        asset_id: 'a'.repeat(96),
        amount: 2,
      },
    },
  };
}

test('findAtomicWalletCreateLeg returns the wallet-owned left leg', () => {
  const match = findAtomicWalletCreateLeg(templateFixture(), 'pf-left');
  assert.equal(match.side, 'left');
  assert.equal(match.sequence, 7);
  assert.equal(match.escrowId, 'escrow-left');
  assert.equal(match.operation.owner, 'pf-left');
});

test('findAtomicWalletCreateLeg returns the wallet-owned right leg', () => {
  const match = findAtomicWalletCreateLeg(templateFixture(), 'pf-right');
  assert.equal(match.side, 'right');
  assert.equal(match.sequence, 9);
  assert.equal(match.escrowId, 'escrow-right');
  assert.equal(match.operation.owner, 'pf-right');
});

test('findAtomicWalletCreateLeg returns null when wallet is not a participant', () => {
  assert.equal(findAtomicWalletCreateLeg(templateFixture(), 'pf-other'), null);
});

test('findAtomicWalletCreateLeg rejects mismatched owner metadata', () => {
  const template = templateFixture();
  template.left.operation.owner = 'pf-other';
  assert.throws(
    () => findAtomicWalletCreateLeg(template, 'pf-other'),
    /owner does not match/,
  );
});

test('findAtomicWalletCreateLeg rejects non-create wallet-owned legs', () => {
  const template = templateFixture();
  template.left.transaction_kind = 'escrow_finish';
  assert.throws(
    () => findAtomicWalletCreateLeg(template, 'pf-left'),
    /not an escrow_create/,
  );
});

test('findAtomicWalletFinishLeg builds the incoming escrow finish operation', () => {
  const finish = findAtomicWalletFinishLeg(templateFixture(), 'pf-left', 'shared-secret');
  assert.equal(finish.side, 'right');
  assert.equal(finish.escrowId, 'escrow-right');
  assert.deepEqual(finish.operation, {
    operation: 'escrow_finish',
    escrow_id: 'escrow-right',
    owner: 'pf-right',
    recipient: 'pf-left',
    fulfillment: 'shared-secret',
  });
});

test('findAtomicWalletCancelLeg builds the wallet-owned cancel operation', () => {
  const cancel = findAtomicWalletCancelLeg(templateFixture(), 'pf-left');
  assert.equal(cancel.side, 'left');
  assert.equal(cancel.escrowId, 'escrow-left');
  assert.deepEqual(cancel.operation, {
    operation: 'escrow_cancel',
    escrow_id: 'escrow-left',
    owner: 'pf-left',
  });
});
