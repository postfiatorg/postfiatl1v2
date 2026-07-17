import assert from 'node:assert/strict';
import test from 'node:test';

import { TxBuilder } from './tx-builder.js';

const quote = {
  chain_id: 'postfiat-wan-devnet',
  genesis_hash: 'a'.repeat(96),
  protocol_version: 1,
  minimum_fee: 1,
  sequence: 7,
  sender_meets_reserve_after_transfer: true,
};

const FASTPAY_DOMAIN = Object.freeze({
  schema: 'postfiat-owned-certificate-domain-v2',
  chain_id: quote.chain_id,
  genesis_hash: quote.genesis_hash,
  protocol_version: quote.protocol_version,
  registry_id: 'b'.repeat(96),
});

const FASTPAY_V3_CAPABILITIES = Object.freeze({
  schema: 'postfiat-fastpay-recovery-capabilities-v1',
  domain: Object.freeze({
    schema: 'postfiat-owned-certificate-domain-v3',
    chain_id: quote.chain_id,
    genesis_hash: quote.genesis_hash,
    protocol_version: quote.protocol_version,
    registry_id: 'c'.repeat(96),
  }),
  committee_epoch: 7,
  current_height: 100,
  validator_count: 4,
  quorum: 3,
  policy: Object.freeze({
    schema: 'postfiat-fastpay-recovery-policy-v1',
    activation_height: 90,
    max_validity_blocks: 10,
    max_recovery_blocks: 10,
  }),
});

const FASTPAY_V3_VALIDATORS = Array.from({ length: 4 }, (_, index) => ({
  node_id: `validator-${index}`,
  public_key_hex: `${index + 1}`.repeat(64),
}));

async function fastPayCapabilities() {
  return {
    owned_lane_enabled: true,
    owned_certificate_domain: FASTPAY_DOMAIN,
  };
}

test('sendTransfer without memos uses the existing v1 transfer path', async () => {
  const calls = [];
  const signedTransfer = { signed: 'v1' };
  const wasm = {
    wallet_sign_transfer(backupJson, quoteJson) {
      calls.push(['wallet_sign_transfer', backupJson, JSON.parse(quoteJson)]);
      return signedTransfer;
    },
    wallet_sign_payment_v2() {
      throw new Error('wallet_sign_payment_v2 should not be called');
    },
  };
  const rpc = {
    async transferFeeQuote(...args) {
      calls.push(['transferFeeQuote', ...args]);
      return { ok: true, result: quote };
    },
    async submitSignedTransferFinality(signedJson) {
      calls.push(['submitSignedTransferFinality', JSON.parse(signedJson)]);
      return { ok: true, result: { tx_id: 'tx-v1' } };
    },
    async submitSignedTransfer() {
      throw new Error('submitSignedTransfer fallback should not be called');
    },
  };

  const builder = new TxBuilder(rpc, () => wasm);
  builder.pollReceipt = async (txId, timeoutMs) => {
    calls.push(['pollReceipt', txId, timeoutMs]);
    return { accepted: true, tx_id: txId };
  };

  const result = await builder.sendTransfer('backup-json', 'pf-from', 'pf-to', 1000);

  assert.equal(result.txId, 'tx-v1');
  assert.deepEqual(calls, [
    ['transferFeeQuote', 'pf-from', 'pf-to', 1000],
    ['wallet_sign_transfer', 'backup-json', quote],
    ['submitSignedTransferFinality', signedTransfer],
    ['pollReceipt', 'tx-v1', 30000],
  ]);
});

test('depositToFastPay signs locally and requires the exact accepted receipt code', async () => {
  const calls = [];
  const sourcePubkeyHex = 'ab'.repeat(1952);
  const wasm = {
    wallet_sign_owned_deposit(backupJson, depositJson) {
      calls.push(['wallet_sign_owned_deposit', backupJson, JSON.parse(depositJson)]);
      return { operation: { OwnedDeposit: { signed: 'local-only' } } };
    },
  };
  const rpc = {
    async serverCapabilities() {
      calls.push(['serverCapabilities']);
      return {
        owned_lane_enabled: true,
        mempool_submit_enabled: true,
        chain_id: quote.chain_id,
        genesis_hash: quote.genesis_hash,
        protocol_version: quote.protocol_version,
        block_height: 12,
      };
    },
    async account(address) {
      calls.push(['account', address]);
      return { ok: true, result: { address, balance: 5_000_000, sequence: 2 } };
    },
    async submitFastlanePrimary(transactionJson) {
      const transaction = JSON.parse(transactionJson);
      calls.push(['submitFastlanePrimary', transaction]);
      assert.equal(JSON.stringify(transaction).includes('backup-json'), false);
      return { ok: true, result: { tx_id: 'owned-deposit-tx' } };
    },
  };
  const builder = new TxBuilder(rpc, () => wasm);
  builder.pollReceipt = async (txId, timeoutMs) => {
    calls.push(['pollReceipt', txId, timeoutMs]);
    return { accepted: true, code: 'owned_deposit_applied', tx_id: txId };
  };

  const result = await builder.depositToFastPay(
    'backup-json',
    'pf-source',
    sourcePubkeyHex,
    1.25,
  );

  assert.equal(result.receipt.code, 'owned_deposit_applied');
  assert.equal(result.deposit.sequence, 3);
  assert.equal(result.deposit.amount_atoms, 1_250_000);
  assert.equal(result.deposit.fee_pft, 1);
  assert.equal(result.deposit.asset, 'PFT');
  assert.equal(result.deposit.valid_through_height, 112);
  assert.equal(result.deposit.nonce.length, 32);
  assert.deepEqual(calls.map((call) => call[0]), [
    'serverCapabilities',
    'account',
    'wallet_sign_owned_deposit',
    'submitFastlanePrimary',
    'pollReceipt',
  ]);
});

test('depositToFastPay never treats rejected or unknown receipt code as success', async () => {
  const sourcePubkeyHex = 'cd'.repeat(1952);
  const rpc = {
    async serverCapabilities() {
      return {
        owned_lane_enabled: true,
        mempool_submit_enabled: true,
        chain_id: quote.chain_id,
        genesis_hash: quote.genesis_hash,
        protocol_version: quote.protocol_version,
        block_height: 1,
      };
    },
    async account() {
      return { ok: true, result: { balance: 2_000_000, sequence: 0 } };
    },
    async submitFastlanePrimary() {
      return { ok: true, result: { tx_id: 'rejected-deposit' } };
    },
  };
  const builder = new TxBuilder(rpc, () => ({
    wallet_sign_owned_deposit() { return { signed: true }; },
  }));
  builder.pollReceipt = async () => ({ accepted: false, code: 'owned_deposit_rejected' });

  await assert.rejects(
    () => builder.depositToFastPay('backup-json', 'pf-source', sourcePubkeyHex, 1),
    /requires accepted receipt code owned_deposit_applied/,
  );
});

test('sendTransfer returns inline finality receipt without polling', async () => {
  const signedTransfer = { signed: 'v1' };
  const receipt = { accepted: true, tx_id: 'tx-v1' };
  const wasm = {
    wallet_sign_transfer() {
      return signedTransfer;
    },
  };
  const rpc = {
    async transferFeeQuote() {
      return { ok: true, result: quote };
    },
    async submitSignedTransferFinality() {
      return {
        ok: true,
        result: {
          tx_id: 'tx-v1',
          finality: {
            local_hot_finality: [{ receipt }],
          },
        },
      };
    },
  };

  const builder = new TxBuilder(rpc, () => wasm);
  builder.pollReceipt = async () => {
    throw new Error('pollReceipt should not be called when inline finality includes the receipt');
  };

  const result = await builder.sendTransfer('backup-json', 'pf-from', 'pf-to', 1000);

  assert.deepEqual(result.receipt, receipt);
});

test('sendTransfer can reuse reviewed quote without a second quote RPC', async () => {
  const calls = [];
  const signedTransfer = { signed: 'v1' };
  const wasm = {
    wallet_sign_transfer(backupJson, quoteJson) {
      calls.push(['wallet_sign_transfer', backupJson, JSON.parse(quoteJson)]);
      return signedTransfer;
    },
  };
  const rpc = {
    async transferFeeQuote() {
      throw new Error('transferFeeQuote should not be called when reviewed quote is provided');
    },
    async submitSignedTransferFinality(signedJson) {
      calls.push(['submitSignedTransferFinality', JSON.parse(signedJson)]);
      return {
        ok: true,
        result: {
          tx_id: 'tx-v1',
          finality: {
            local_hot_finality: [{ receipt: { accepted: true, tx_id: 'tx-v1' } }],
          },
        },
      };
    },
  };

  const builder = new TxBuilder(rpc, () => wasm);
  const result = await builder.sendTransfer(
    'backup-json',
    'pf-from',
    'pf-to',
    1000,
    undefined,
    quote,
  );

  assert.equal(result.txId, 'tx-v1');
  assert.deepEqual(calls, [
    ['wallet_sign_transfer', 'backup-json', quote],
    ['submitSignedTransferFinality', signedTransfer],
  ]);
});

test('sendTransfer with any memo uses payment v2 signing and finality submit', async () => {
  const calls = [];
  const signedPaymentV2 = { signed: 'v2' };
  const wasm = {
    wallet_sign_transfer() {
      throw new Error('wallet_sign_transfer should not be called');
    },
    wallet_sign_payment_v2(backupJson, fieldsJson) {
      calls.push(['wallet_sign_payment_v2', backupJson, JSON.parse(fieldsJson)]);
      return signedPaymentV2;
    },
  };
  const rpc = {
    async transferFeeQuote(...args) {
      calls.push(['transferFeeQuote', ...args]);
      return { ok: true, result: quote };
    },
    async submitSignedPaymentV2Finality(signedJson) {
      calls.push(['submitSignedPaymentV2Finality', JSON.parse(signedJson)]);
      return { ok: true, result: { tx_id: 'tx-v2' } };
    },
    async submitSignedPaymentV2() {
      throw new Error('submitSignedPaymentV2 fallback should not be called');
    },
    async submitSignedTransferFinality() {
      throw new Error('submitSignedTransferFinality should not be called');
    },
  };

  const builder = new TxBuilder(rpc, () => wasm);
  builder.pollReceipt = async (txId, timeoutMs) => {
    calls.push(['pollReceipt', txId, timeoutMs]);
    return { accepted: true, tx_id: txId };
  };

  const result = await builder.sendTransfer(
    'backup-json',
    'pf-from',
    'pf-to',
    1000,
    { memo_type: '', memo_format: '', memo_data: 'test-memo-ghash' },
  );

  const encodedMemos = {
    memo_type: '',
    memo_format: '',
    memo_data: '746573742d6d656d6f2d6768617368',
  };

  assert.equal(result.txId, 'tx-v2');
  assert.equal(result.paymentV2, true);
  assert.deepEqual(calls, [
    ['transferFeeQuote', 'pf-from', 'pf-to', 1000, encodedMemos],
    ['wallet_sign_payment_v2', 'backup-json', {
      chain_id: quote.chain_id,
      genesis_hash: quote.genesis_hash,
      protocol_version: quote.protocol_version,
      to: 'pf-to',
      amount: 1000,
      fee: quote.minimum_fee,
      sequence: quote.sequence,
      memos: [encodedMemos],
    }],
    ['submitSignedPaymentV2Finality', signedPaymentV2],
    ['pollReceipt', 'tx-v2', 30000],
  ]);
});

test('sendTransfer with memo returns inline payment v2 finality receipt without polling', async () => {
  const signedPaymentV2 = { signed: 'v2' };
  const receipt = { accepted: true, tx_id: 'tx-v2' };
  const wasm = {
    wallet_sign_payment_v2() {
      return signedPaymentV2;
    },
  };
  const rpc = {
    async transferFeeQuote() {
      return { ok: true, result: quote };
    },
    async submitSignedPaymentV2Finality() {
      return {
        ok: true,
        result: {
          tx_id: 'tx-v2',
          finality: {
            local_hot_finality: [{ receipt }],
          },
        },
      };
    },
  };

  const builder = new TxBuilder(rpc, () => wasm);
  builder.pollReceipt = async () => {
    throw new Error('pollReceipt should not be called when inline finality includes the payment v2 receipt');
  };

  const result = await builder.sendTransfer(
    'backup-json',
    'pf-from',
    'pf-to',
    1000,
    { memo_data: 'memo' },
  );

  assert.equal(result.paymentV2, true);
  assert.deepEqual(result.receipt, receipt);
});

test('sendTransfer with memo can reuse reviewed quote without a second quote RPC', async () => {
  const calls = [];
  const signedPaymentV2 = { signed: 'v2' };
  const memoInput = { memo_type: '', memo_format: '', memo_data: 'memo' };
  const memoFields = { memo_type: '', memo_format: '', memo_data: '6d656d6f' };
  const wasm = {
    wallet_sign_payment_v2(backupJson, fieldsJson) {
      calls.push(['wallet_sign_payment_v2', backupJson, JSON.parse(fieldsJson)]);
      return signedPaymentV2;
    },
  };
  const rpc = {
    async transferFeeQuote() {
      throw new Error('transferFeeQuote should not be called when reviewed quote is provided');
    },
    async submitSignedPaymentV2Finality(signedJson) {
      calls.push(['submitSignedPaymentV2Finality', JSON.parse(signedJson)]);
      return {
        ok: true,
        result: {
          tx_id: 'tx-v2',
          finality: {
            local_hot_finality: [{ receipt: { accepted: true, tx_id: 'tx-v2' } }],
          },
        },
      };
    },
  };

  const builder = new TxBuilder(rpc, () => wasm);
  const result = await builder.sendTransfer(
    'backup-json',
    'pf-from',
    'pf-to',
    1000,
    memoInput,
    quote,
  );

  assert.equal(result.txId, 'tx-v2');
  assert.equal(result.paymentV2, true);
  assert.deepEqual(calls, [
    ['wallet_sign_payment_v2', 'backup-json', {
      chain_id: quote.chain_id,
      genesis_hash: quote.genesis_hash,
      protocol_version: quote.protocol_version,
      to: 'pf-to',
      amount: 1000,
      fee: quote.minimum_fee,
      sequence: quote.sequence,
      memos: [memoFields],
    }],
    ['submitSignedPaymentV2Finality', signedPaymentV2],
  ]);
});

test('sendAssetTransfer signs reviewed asset operation and submits with finality', async () => {
  const calls = [];
  const operation = {
    operation: 'nav_redeem_at_nav',
    owner: 'pf-from',
    issuer: 'pfissuer',
    asset_id: 'aa'.repeat(48),
    amount: 7,
    epoch: 3,
    reserve_packet_hash: 'bb'.repeat(48),
  };
  const assetQuote = {
    chain_id: quote.chain_id,
    genesis_hash: quote.genesis_hash,
    protocol_version: quote.protocol_version,
    source: 'pf-from',
    minimum_fee: 3,
    sequence: 9,
    operation,
    sender_meets_reserve_after_fee: true,
  };
  const signedAsset = { signed: 'asset' };
  const wasm = {
    wallet_sign_asset_transaction_fields(backupJson, fieldsJson) {
      calls.push(['wallet_sign_asset_transaction_fields', backupJson, JSON.parse(fieldsJson)]);
      return signedAsset;
    },
  };
  const rpc = {
    async assetFeeQuote(source, operationJson) {
      calls.push(['assetFeeQuote', source, JSON.parse(operationJson)]);
      return { ok: true, result: assetQuote };
    },
    async submitSignedAssetTransactionFinality(signedJson) {
      calls.push(['submitSignedAssetTransactionFinality', JSON.parse(signedJson)]);
      return {
        ok: true,
        result: {
          tx_id: 'tx-asset',
          finality: {
            tx_id: 'tx-asset',
            confirmed: true,
            receipt: { accepted: true, tx_id: 'tx-asset', certified: true },
          },
        },
      };
    },
  };
  const builder = new TxBuilder(rpc, () => wasm);
  builder.pollReceipt = async (txId, timeoutMs) => {
    calls.push(['pollReceipt', txId, timeoutMs]);
    return { accepted: true, tx_id: txId };
  };

  const result = await builder.sendAssetTransfer('backup-json', 'pf-from', { operation });

  assert.equal(result.txId, 'tx-asset');
  assert.deepEqual(calls, [
    ['assetFeeQuote', 'pf-from', operation],
    ['wallet_sign_asset_transaction_fields', 'backup-json', {
      chain_id: quote.chain_id,
      genesis_hash: quote.genesis_hash,
      protocol_version: quote.protocol_version,
      source: 'pf-from',
      fee: 3,
      sequence: 9,
      operation,
    }],
    ['submitSignedAssetTransactionFinality', signedAsset],
  ]);
  assert.deepEqual(result.receipt, { accepted: true, tx_id: 'tx-asset', certified: true });
});

test('signAssetTransaction signs a reviewed asset operation without submitting', async () => {
  const calls = [];
  const operation = {
    asset_burn: {
      owner: 'pf-from',
      issuer: 'pfissuer',
      asset_id: 'aa'.repeat(48),
      amount: 3,
    },
  };
  const assetQuote = {
    chain_id: quote.chain_id,
    genesis_hash: quote.genesis_hash,
    protocol_version: quote.protocol_version,
    source: 'pf-from',
    minimum_fee: 3,
    sequence: 9,
    operation,
    sender_meets_reserve_after_fee: true,
  };
  const signedAsset = { signed: 'asset-burn' };
  const wasm = {
    wallet_sign_asset_transaction_fields(backupJson, fieldsJson) {
      calls.push(['wallet_sign_asset_transaction_fields', backupJson, JSON.parse(fieldsJson)]);
      return signedAsset;
    },
  };
  const rpc = {
    async assetFeeQuote(source, operationJson) {
      calls.push(['assetFeeQuote', source, JSON.parse(operationJson)]);
      return { ok: true, result: assetQuote };
    },
    async submitSignedAssetTransactionFinality() {
      throw new Error('signAssetTransaction must not submit');
    },
  };
  const builder = new TxBuilder(rpc, () => wasm);

  const result = await builder.signAssetTransaction('backup-json', 'pf-from', { operation });

  assert.equal(result.signed, signedAsset);
  assert.equal(result.signedJson, JSON.stringify(signedAsset));
  assert.deepEqual(calls, [
    ['assetFeeQuote', 'pf-from', operation],
    ['wallet_sign_asset_transaction_fields', 'backup-json', {
      chain_id: quote.chain_id,
      genesis_hash: quote.genesis_hash,
      protocol_version: quote.protocol_version,
      source: 'pf-from',
      fee: 3,
      sequence: 9,
      operation,
    }],
  ]);
});

test('sendAssetTransfer accepts quoted numeric operation fields returned as strings', async () => {
  const calls = [];
  const reviewedOperation = {
    operation: 'trust_set',
    account: 'pf-from',
    issuer: 'pfissuer',
    asset_id: 'aa'.repeat(48),
    limit: '1000000000000',
  };
  const quotedOperation = {
    ...reviewedOperation,
    limit: 1000000000000,
  };
  const signedAsset = { signed: 'asset' };
  const wasm = {
    wallet_sign_asset_transaction_fields(backupJson, fieldsJson) {
      calls.push(['wallet_sign_asset_transaction_fields', backupJson, JSON.parse(fieldsJson)]);
      return signedAsset;
    },
  };
  const rpc = {
    async assetFeeQuote(source, operationJson) {
      calls.push(['assetFeeQuote', source, JSON.parse(operationJson)]);
      return {
        ok: true,
        result: {
          chain_id: quote.chain_id,
          genesis_hash: quote.genesis_hash,
          protocol_version: quote.protocol_version,
          source: 'pf-from',
          minimum_fee: 3,
          sequence: 9,
          operation: quotedOperation,
          sender_meets_reserve_after_fee: true,
        },
      };
    },
    async submitSignedAssetTransactionFinality(signedJson) {
      calls.push(['submitSignedAssetTransactionFinality', JSON.parse(signedJson)]);
      return { ok: true, result: { tx_id: 'tx-asset' } };
    },
  };
  const builder = new TxBuilder(rpc, () => wasm);
  builder.pollReceipt = async (txId) => ({ accepted: true, tx_id: txId });

  await builder.sendAssetTransfer('backup-json', 'pf-from', { operation: reviewedOperation });

  assert.deepEqual(calls, [
    ['assetFeeQuote', 'pf-from', reviewedOperation],
    ['wallet_sign_asset_transaction_fields', 'backup-json', {
      chain_id: quote.chain_id,
      genesis_hash: quote.genesis_hash,
      protocol_version: quote.protocol_version,
      source: 'pf-from',
      fee: 3,
      sequence: 9,
      operation: quotedOperation,
    }],
    ['submitSignedAssetTransactionFinality', signedAsset],
  ]);
});

test('sendAssetTransfer rejects source substitution before signing', async () => {
  const calls = [];
  const operation = {
    operation: 'trust_set',
    account: 'pf-from',
    issuer: 'pfissuer',
    asset_id: 'aa'.repeat(48),
    limit: 10,
  };
  const wasm = {
    wallet_sign_asset_transaction_fields() {
      calls.push('wallet_sign_asset_transaction_fields');
      throw new Error('asset signing should not happen');
    },
  };
  const rpc = {
    async assetFeeQuote() {
      calls.push('assetFeeQuote');
      return {
        ok: true,
        result: {
          chain_id: quote.chain_id,
          genesis_hash: quote.genesis_hash,
          protocol_version: quote.protocol_version,
          source: 'pf-attacker',
          minimum_fee: 3,
          sequence: 9,
          operation,
          sender_meets_reserve_after_fee: true,
        },
      };
    },
    async submitSignedAssetTransaction() {
      throw new Error('asset submit should not happen');
    },
  };
  const builder = new TxBuilder(rpc, () => wasm);

  await assert.rejects(
    () => builder.sendAssetTransfer('backup-json', 'pf-from', { operation }),
    /source does not match/,
  );
  assert.deepEqual(calls, ['assetFeeQuote']);
});

test('sendAssetTransfer rejects operation substitution before signing', async () => {
  const calls = [];
  const reviewedOperation = {
    operation: 'nav_redeem_at_nav',
    owner: 'pf-from',
    issuer: 'pfissuer',
    asset_id: 'aa'.repeat(48),
    amount: 7,
    epoch: 3,
    reserve_packet_hash: 'bb'.repeat(48),
  };
  const substitutedOperation = {
    ...reviewedOperation,
    amount: 8,
  };
  const wasm = {
    wallet_sign_asset_transaction_fields() {
      calls.push('wallet_sign_asset_transaction_fields');
      throw new Error('asset signing should not happen');
    },
  };
  const rpc = {
    async assetFeeQuote() {
      calls.push('assetFeeQuote');
      return {
        ok: true,
        result: {
          chain_id: quote.chain_id,
          genesis_hash: quote.genesis_hash,
          protocol_version: quote.protocol_version,
          source: 'pf-from',
          minimum_fee: 3,
          sequence: 9,
          operation: substitutedOperation,
          sender_meets_reserve_after_fee: true,
        },
      };
    },
    async submitSignedAssetTransaction() {
      throw new Error('asset submit should not happen');
    },
  };
  const builder = new TxBuilder(rpc, () => wasm);

  await assert.rejects(
    () => builder.sendAssetTransfer('backup-json', 'pf-from', { operation: reviewedOperation }),
    /operation does not match/,
  );
  assert.deepEqual(calls, ['assetFeeQuote']);
});

test('sendEscrowTransaction signs reviewed escrow operation and submits', async () => {
  const calls = [];
  const operation = {
    operation: 'escrow_create',
    owner: 'pf-from',
    recipient: 'pf-to',
    asset_id: 'PFT',
    amount: 5,
    condition: 'shared-secret',
    finish_after: 0,
    cancel_after: 100,
  };
  const escrowQuote = {
    chain_id: quote.chain_id,
    genesis_hash: quote.genesis_hash,
    protocol_version: quote.protocol_version,
    source: 'pf-from',
    minimum_fee: 3,
    sequence: 9,
    operation,
    sender_meets_reserve_after_fee: true,
  };
  const signedEscrow = { signed: 'escrow' };
  const wasm = {
    wallet_sign_escrow_transaction_fields(backupJson, fieldsJson) {
      calls.push(['wallet_sign_escrow_transaction_fields', backupJson, JSON.parse(fieldsJson)]);
      return signedEscrow;
    },
  };
  const rpc = {
    async escrowFeeQuote(source, operationJson, sequence) {
      calls.push(['escrowFeeQuote', source, JSON.parse(operationJson), sequence]);
      return { ok: true, result: escrowQuote };
    },
    async submitSignedEscrowTransaction(signedJson) {
      calls.push(['submitSignedEscrowTransaction', JSON.parse(signedJson)]);
      return { ok: true, result: { tx_id: 'tx-escrow' } };
    },
  };
  const builder = new TxBuilder(rpc, () => wasm);
  builder.pollReceipt = async (txId, timeoutMs) => {
    calls.push(['pollReceipt', txId, timeoutMs]);
    return { accepted: true, tx_id: txId };
  };

  const result = await builder.sendEscrowTransaction(
    'backup-json',
    'pf-from',
    { operation },
    { sequence: 9 },
  );

  assert.equal(result.txId, 'tx-escrow');
  assert.deepEqual(calls, [
    ['escrowFeeQuote', 'pf-from', operation, 9],
    ['wallet_sign_escrow_transaction_fields', 'backup-json', {
      chain_id: quote.chain_id,
      genesis_hash: quote.genesis_hash,
      protocol_version: quote.protocol_version,
      source: 'pf-from',
      fee: 3,
      sequence: 9,
      operation,
    }],
    ['submitSignedEscrowTransaction', signedEscrow],
    ['pollReceipt', 'tx-escrow', 30000],
  ]);
});

test('sendEscrowTransaction prefers certified escrow finality submit', async () => {
  const calls = [];
  const operation = {
    operation: 'escrow_create',
    owner: 'pf-from',
    recipient: 'pf-to',
    asset_id: 'PFT',
    amount: 5,
    condition: 'shared-secret',
    cancel_after: 100,
  };
  const escrowQuote = {
    chain_id: quote.chain_id,
    genesis_hash: quote.genesis_hash,
    protocol_version: quote.protocol_version,
    source: 'pf-from',
    minimum_fee: 3,
    sequence: 9,
    operation,
    sender_meets_reserve_after_fee: true,
  };
  const signedEscrow = { signed: 'escrow' };
  const finalityReceipt = { accepted: true, confirmed: true, tx_id: 'tx-escrow' };
  const wasm = {
    wallet_sign_escrow_transaction_fields(backupJson, fieldsJson) {
      calls.push(['wallet_sign_escrow_transaction_fields', backupJson, JSON.parse(fieldsJson)]);
      return signedEscrow;
    },
  };
  const rpc = {
    async escrowFeeQuote(source, operationJson, sequence) {
      calls.push(['escrowFeeQuote', source, JSON.parse(operationJson), sequence]);
      return { ok: true, result: escrowQuote };
    },
    async submitSignedEscrowTransactionFinality(signedJson) {
      calls.push(['submitSignedEscrowTransactionFinality', JSON.parse(signedJson)]);
      return {
        ok: true,
        result: {
          tx_id: 'tx-escrow',
          finality: { receipt: finalityReceipt },
        },
      };
    },
    async submitSignedEscrowTransaction() {
      throw new Error('raw escrow submit fallback should not be called');
    },
  };
  const builder = new TxBuilder(rpc, () => wasm);
  builder.pollReceipt = async () => {
    throw new Error('pollReceipt should not be called when finality receipt is inline');
  };

  const result = await builder.sendEscrowTransaction(
    'backup-json',
    'pf-from',
    { operation },
    { sequence: 9 },
  );

  assert.equal(result.txId, 'tx-escrow');
  assert.deepEqual(result.receipt, finalityReceipt);
  assert.deepEqual(calls, [
    ['escrowFeeQuote', 'pf-from', operation, 9],
    ['wallet_sign_escrow_transaction_fields', 'backup-json', {
      chain_id: quote.chain_id,
      genesis_hash: quote.genesis_hash,
      protocol_version: quote.protocol_version,
      source: 'pf-from',
      fee: 3,
      sequence: 9,
      operation,
    }],
    ['submitSignedEscrowTransactionFinality', signedEscrow],
  ]);
});

test('sendEscrowTransaction rejects insufficient fee reserve before signing', async () => {
  const calls = [];
  const wasm = {
    wallet_sign_escrow_transaction_fields() {
      calls.push('wallet_sign_escrow_transaction_fields');
      throw new Error('escrow signing should not happen');
    },
  };
  const rpc = {
    async escrowFeeQuote() {
      calls.push('escrowFeeQuote');
      return {
        ok: true,
        result: {
          sender_meets_reserve_after_fee: false,
        },
      };
    },
    async submitSignedEscrowTransaction() {
      throw new Error('escrow submit should not happen');
    },
  };
  const builder = new TxBuilder(rpc, () => wasm);

  await assert.rejects(
    () => builder.sendEscrowTransaction('backup-json', 'pf-from', {
      operation: { operation: 'escrow_cancel', escrow_id: 'escrow-1', owner: 'pf-from' },
    }),
    /Insufficient balance for escrow transaction fee/,
  );
  assert.deepEqual(calls, ['escrowFeeQuote']);
});

test('sendEscrowTransaction rejects operation substitution before signing', async () => {
  const calls = [];
  const reviewedOperation = {
    operation: 'escrow_create',
    owner: 'pf-from',
    recipient: 'pf-to',
    asset_id: 'PFT',
    amount: 5,
    condition: 'shared-secret',
    finish_after: 0,
    cancel_after: 100,
  };
  const substitutedOperation = {
    ...reviewedOperation,
    recipient: 'pf-attacker',
  };
  const wasm = {
    wallet_sign_escrow_transaction_fields() {
      calls.push('wallet_sign_escrow_transaction_fields');
      throw new Error('escrow signing should not happen');
    },
  };
  const rpc = {
    async escrowFeeQuote() {
      calls.push('escrowFeeQuote');
      return {
        ok: true,
        result: {
          chain_id: quote.chain_id,
          genesis_hash: quote.genesis_hash,
          protocol_version: quote.protocol_version,
          source: 'pf-from',
          minimum_fee: 3,
          sequence: 9,
          operation: substitutedOperation,
          sender_meets_reserve_after_fee: true,
        },
      };
    },
    async submitSignedEscrowTransaction() {
      throw new Error('escrow submit should not happen');
    },
  };
  const builder = new TxBuilder(rpc, () => wasm);

  await assert.rejects(
    () => builder.sendEscrowTransaction('backup-json', 'pf-from', {
      operation: reviewedOperation,
    }, { sequence: 9 }),
    /operation does not match/,
  );
  assert.deepEqual(calls, ['escrowFeeQuote']);
});

test('sendEscrowTransaction rejects sequence substitution before signing', async () => {
  const calls = [];
  const operation = {
    operation: 'escrow_cancel',
    escrow_id: 'escrow-1',
    owner: 'pf-from',
  };
  const wasm = {
    wallet_sign_escrow_transaction_fields() {
      calls.push('wallet_sign_escrow_transaction_fields');
      throw new Error('escrow signing should not happen');
    },
  };
  const rpc = {
    async escrowFeeQuote() {
      calls.push('escrowFeeQuote');
      return {
        ok: true,
        result: {
          chain_id: quote.chain_id,
          genesis_hash: quote.genesis_hash,
          protocol_version: quote.protocol_version,
          source: 'pf-from',
          minimum_fee: 3,
          sequence: 10,
          operation,
          sender_meets_reserve_after_fee: true,
        },
      };
    },
    async submitSignedEscrowTransaction() {
      throw new Error('escrow submit should not happen');
    },
  };
  const builder = new TxBuilder(rpc, () => wasm);

  await assert.rejects(
    () => builder.sendEscrowTransaction('backup-json', 'pf-from', { operation }, { sequence: 9 }),
    /sequence does not match/,
  );
  assert.deepEqual(calls, ['escrowFeeQuote']);
});

test('sendOwnedTransfer refuses missing certificate domain before signing or mutation', async () => {
  let signed = false;
  const wasm = {
    wallet_sign_owned_transfer() {
      signed = true;
      throw new Error('must not sign without a certificate domain');
    },
  };
  const rpc = {
    async serverCapabilities() {
      return { owned_lane_enabled: true, owned_certificate_domain: null };
    },
    async ownedSign() {
      throw new Error('must not request votes without a certificate domain');
    },
  };
  const builder = new TxBuilder(rpc, () => wasm);

  await assert.rejects(
    () => builder.sendOwnedTransfer(
      'backup-json',
      'owner-pk',
      [{ id: 'object-1', version: 1, value: 2_000_001, asset: 'PFT' }],
      'recipient-pk',
      1,
      0.000001,
      [{ node_id: 'validator-0' }],
    ),
    /FastPay certificate domain is unavailable/,
  );
  assert.equal(signed, false);
});

test('sendOwnedTransfer v3 requires a cryptographic quorum of durable apply acknowledgements', async () => {
  const calls = [];
  const lockId = '1'.repeat(96);
  const certificateDigest = '2'.repeat(96);
  const terminalStateDigest = '3'.repeat(96);
  const orderDigest = '4'.repeat(96);
  const wasm = {
    wallet_fastpay_transfer_lock_id(orderJson) {
      calls.push(['lock', JSON.parse(orderJson)]);
      return lockId;
    },
    wallet_sign_owned_transfer_v3(backupJson, orderJson, capabilitiesJson) {
      const order = JSON.parse(orderJson);
      calls.push(['sign', backupJson, order, JSON.parse(capabilitiesJson)]);
      return { order, owner_pubkey_hex: 'owner-pk', owner_signature_hex: 'owner-sig' };
    },
    wallet_fastpay_transfer_certificate_digest(certJson) {
      calls.push(['digest', JSON.parse(certJson)]);
      return certificateDigest;
    },
    wallet_verify_fastpay_apply_ack(ackJson, publicKeyHex) {
      const ack = JSON.parse(ackJson);
      calls.push(['verify-ack', ack.validator_id, publicKeyHex]);
      return true;
    },
  };
  const rpc = {
    async ownedRecoveryCapabilities() {
      return { ok: true, result: FASTPAY_V3_CAPABILITIES };
    },
    async ownedSignV3(signedOrderJson, validatorId) {
      const signed = JSON.parse(signedOrderJson);
      assert.equal(signed.order.recovery.lock_id, lockId);
      return { ok: true, result: { validator_id: validatorId, signature_hex: `vote-${validatorId}` } };
    },
    async ownedApplyV3(certJson) {
      const certificate = JSON.parse(certJson);
      return {
        ok: true,
        result: {
          validators: FASTPAY_V3_VALIDATORS.slice(0, 3).map(validator => ({
            validator_id: validator.node_id,
            ok: true,
            result: {
              schema: 'postfiat-fastpay-apply-ack-v1',
              domain: FASTPAY_V3_CAPABILITIES.domain,
              committee_epoch: FASTPAY_V3_CAPABILITIES.committee_epoch,
              lock_id: certificate.order.recovery.lock_id,
              order_digest: orderDigest,
              certificate_digest: certificateDigest,
              terminal_state_digest: terminalStateDigest,
              validator_id: validator.node_id,
              signature_hex: `ack-${validator.node_id}`,
            },
          })),
        },
      };
    },
    async ownedApply() {
      throw new Error('v3 must not downgrade to legacy apply');
    },
  };
  const builder = new TxBuilder(rpc, () => wasm);
  const result = await builder.sendOwnedTransfer(
    'backup-json',
    'owner-pk',
    [{ id: 'object-v3', version: 1, value: 2_000_001, asset: 'PFT' }],
    'recipient-pk',
    1,
    0.000001,
    FASTPAY_V3_VALIDATORS,
  );

  assert.equal(result.status, 'finalized');
  assert.equal(result.acknowledgements.length, 3);
  assert.equal(calls.filter(call => call[0] === 'verify-ack').length, 3);
  assert.equal(result.order.recovery.expires_at_height, 110);
  assert.equal(result.order.recovery.recovery_closes_at_height, 120);
});

test('sendOwnedTransfer v3 reports recovery pending instead of success below authenticated apply quorum', async () => {
  const lockId = '1'.repeat(96);
  const certificateDigest = '2'.repeat(96);
  const wasm = {
    wallet_fastpay_transfer_lock_id: () => lockId,
    wallet_sign_owned_transfer_v3(_backupJson, orderJson) {
      return {
        order: JSON.parse(orderJson),
        owner_pubkey_hex: 'owner-pk',
        owner_signature_hex: 'owner-sig',
      };
    },
    wallet_fastpay_transfer_certificate_digest: () => certificateDigest,
    wallet_verify_fastpay_apply_ack: () => true,
  };
  const rpc = {
    async ownedRecoveryCapabilities() {
      return { ok: true, result: FASTPAY_V3_CAPABILITIES };
    },
    async ownedSignV3(_signedOrderJson, validatorId) {
      return { ok: true, result: { validator_id: validatorId, signature_hex: `vote-${validatorId}` } };
    },
    async ownedApplyV3(certJson) {
      const certificate = JSON.parse(certJson);
      return {
        ok: true,
        result: {
          validators: FASTPAY_V3_VALIDATORS.slice(0, 2).map(validator => ({
            validator_id: validator.node_id,
            ok: true,
            result: {
              schema: 'postfiat-fastpay-apply-ack-v1',
              domain: FASTPAY_V3_CAPABILITIES.domain,
              committee_epoch: 7,
              lock_id: certificate.order.recovery.lock_id,
              order_digest: '4'.repeat(96),
              certificate_digest: certificateDigest,
              terminal_state_digest: '3'.repeat(96),
              validator_id: validator.node_id,
              signature_hex: `ack-${validator.node_id}`,
            },
          })),
        },
      };
    },
  };
  const builder = new TxBuilder(rpc, () => wasm);
  await assert.rejects(
    () => builder.sendOwnedTransfer(
      'backup-json',
      'owner-pk',
      [{ id: 'object-v3', version: 1, value: 2_000_001, asset: 'PFT' }],
      'recipient-pk',
      1,
      0.000001,
      FASTPAY_V3_VALIDATORS,
    ),
    error => error?.code === 'fastpay_recovery_pending'
      && error?.recovery?.certificate?.order?.recovery?.lock_id === lockId,
  );
});

test('recoverFastPay reveals a retained quorum certificate and gates on its accepted receipt code', async () => {
  const certificate = {
    order: {
      domain: FASTPAY_V3_CAPABILITIES.domain,
      recovery: {
        schema: 'postfiat-fastpay-order-recovery-v1',
        committee_epoch: 7,
        lock_id: '1'.repeat(96),
        valid_from_height: 100,
        expires_at_height: 110,
        recovery_closes_at_height: 120,
      },
      inputs: [{ id: 'object-v3', version: 1 }],
      outputs: [{ owner_pubkey_hex: 'recipient-pk', value: 1, asset: 'PFT' }],
      fee: 0,
      nonce: 1,
      memos: [],
    },
    owner_pubkey_hex: 'owner-pk',
    owner_signature_hex: 'owner-sig',
    votes: FASTPAY_V3_VALIDATORS.slice(0, 3).map(validator => ({
      validator_id: validator.node_id,
      signature_hex: `vote-${validator.node_id}`,
    })),
  };
  const rpc = {
    async ownedRecoveryCapabilities() {
      return {
        ok: true,
        result: { ...FASTPAY_V3_CAPABILITIES, current_height: 111 },
      };
    },
    async ownedRecoveryStatus(lockId) {
      return { ok: true, result: { status: 'open_or_unknown', lock_id: lockId } };
    },
    async submitFastlanePrimary(transactionJson) {
      const transaction = JSON.parse(transactionJson);
      assert.deepEqual(transaction, {
        operation: {
          kind: 'fast_pay_recovery_reveal',
          certificate: { operation: 'transfer', certificate },
        },
      });
      return { ok: true, result: { tx_id: 'reveal-tx' } };
    },
  };
  const builder = new TxBuilder(rpc, async () => ({}));
  builder.pollReceipt = async txId => ({
    tx_id: txId,
    accepted: true,
    code: 'fastpay_recovery_certificate_revealed',
  });
  const result = await builder.recoverFastPay({ certificate });
  assert.equal(result.status, 'certificate_revealed');
  assert.equal(result.next_action_height, 120);
});

test('recoverFastPay orders bounded cancellation for a partial lock and never accepts an unknown receipt', async () => {
  const signedOrder = {
    operation: 'transfer',
    signed_order: {
      order: {
        domain: FASTPAY_V3_CAPABILITIES.domain,
        recovery: {
          schema: 'postfiat-fastpay-order-recovery-v1',
          committee_epoch: 7,
          lock_id: '1'.repeat(96),
          valid_from_height: 100,
          expires_at_height: 110,
          recovery_closes_at_height: 120,
        },
        inputs: [{ id: 'object-v3', version: 1 }],
        outputs: [{ owner_pubkey_hex: 'recipient-pk', value: 1, asset: 'PFT' }],
        fee: 0,
        nonce: 1,
        memos: [],
      },
      owner_pubkey_hex: 'owner-pk',
      owner_signature_hex: 'owner-sig',
    },
  };
  let submitted = null;
  const rpc = {
    async ownedRecoveryCapabilities() {
      return {
        ok: true,
        result: { ...FASTPAY_V3_CAPABILITIES, current_height: 120 },
      };
    },
    async ownedRecoveryStatus() {
      return { ok: true, result: { status: 'open_or_unknown' } };
    },
    async submitFastlanePrimary(transactionJson) {
      submitted = JSON.parse(transactionJson);
      return { ok: true, result: { tx_id: 'decision-tx' } };
    },
  };
  const builder = new TxBuilder(rpc, async () => ({}));
  builder.pollReceipt = async () => ({ accepted: null, code: 'unknown' });
  await assert.rejects(
    () => builder.recoverFastPay({ signed_order: signedOrder }),
    error => error?.code === 'fastpay_recovery_pending',
  );
  assert.equal(submitted.operation.kind, 'fast_pay_recovery_decision');
  assert.equal(submitted.operation.request.submitted_at_height, 121);
  assert.deepEqual(submitted.operation.request.signed_order, signedOrder);
});

test('sendOwnedTransfer collects FastPay quorum without waiting for slow validator', async () => {
  const calls = [];
  const validators = Array.from({ length: 6 }, (_, index) => ({ node_id: `validator-${index}` }));
  const wasm = {
    wallet_sign_owned_transfer(backupJson, orderJson) {
      calls.push(['wallet_sign_owned_transfer', backupJson, JSON.parse(orderJson)]);
      return {
        owner_pubkey_hex: 'owner-pk',
        owner_signature_hex: 'owner-sig',
      };
    },
  };
  const rpc = {
    serverCapabilities: fastPayCapabilities,
    async ownedSign(orderJson, validatorId) {
      calls.push(['ownedSign', validatorId, JSON.parse(orderJson)]);
      if (validatorId === 'validator-5') {
        return new Promise(() => {});
      }
      return {
        ok: true,
        result: {
          validator_id: validatorId,
          signature_hex: `sig-${validatorId}`,
        },
      };
    },
    async ownedApply(certJson) {
      const cert = JSON.parse(certJson);
      calls.push(['ownedApply', cert.votes.map(vote => vote.validator_id)]);
      return { ok: true, result: { applied_count: 6 } };
    },
  };
  const builder = new TxBuilder(rpc, () => wasm);

  const result = await builder.sendOwnedTransfer(
    'backup-json',
    'owner-pk',
    [{ id: 'object-1', version: 1, value: 2_000_001, asset: 'PFT' }],
    'recipient-pk',
    1,
    0.000001,
    validators,
  );

  assert.equal(result.votes.length, 5);
  assert.deepEqual(result.votes.map(vote => vote.validator_id), [
    'validator-0',
    'validator-1',
    'validator-2',
    'validator-3',
    'validator-4',
  ]);
  assert.deepEqual(calls.at(-1), [
    'ownedApply',
    ['validator-0', 'validator-1', 'validator-2', 'validator-3', 'validator-4'],
  ]);
  const signCalls = calls.filter(call => call[0] === 'ownedSign');
  assert.equal(signCalls.length, 6);
  for (const [, , envelope] of signCalls) {
    assert.deepEqual(envelope, {
      order: result.order,
      owner_pubkey_hex: 'owner-pk',
      owner_signature_hex: 'owner-sig',
    });
  }
});

test('sendOwnedTransfer rejects fewer than FastPay quorum votes', async () => {
  const validators = Array.from({ length: 6 }, (_, index) => ({ node_id: `validator-${index}` }));
  const wasm = {
    wallet_sign_owned_transfer() {
      return {
        owner_pubkey_hex: 'owner-pk',
        owner_signature_hex: 'owner-sig',
      };
    },
  };
  const rpc = {
    serverCapabilities: fastPayCapabilities,
    async ownedSign(_orderJson, validatorId) {
      if (['validator-0', 'validator-1', 'validator-2', 'validator-3'].includes(validatorId)) {
        return {
          ok: true,
          result: {
            validator_id: validatorId,
            signature_hex: `sig-${validatorId}`,
          },
        };
      }
      throw new Error('vote unavailable');
    },
    async ownedApply() {
      throw new Error('ownedApply must not run without quorum');
    },
  };
  const builder = new TxBuilder(rpc, () => wasm);

  await assert.rejects(
    () => builder.sendOwnedTransfer(
      'backup-json',
      'owner-pk',
      [{ id: 'object-1', version: 1, value: 2_000_001, asset: 'PFT' }],
      'recipient-pk',
      1,
      0.000001,
      validators,
    ),
    /FastPay collected 4 validator votes, need 5/,
  );
});

test('sendOwnedTransfer retries another owned object when validators refuse a stale lock', async () => {
  const calls = [];
  const validators = Array.from({ length: 6 }, (_, index) => ({ node_id: `validator-${index}` }));
  const wasm = {
    wallet_sign_owned_transfer(backupJson, orderJson) {
      const order = JSON.parse(orderJson);
      calls.push(['wallet_sign_owned_transfer', backupJson, order.inputs[0].id]);
      return {
        owner_pubkey_hex: 'owner-pk',
        owner_signature_hex: `owner-sig-${order.inputs[0].id}`,
      };
    },
  };
  const rpc = {
    serverCapabilities: fastPayCapabilities,
    async ownedSign(orderJson, validatorId) {
      const { order } = JSON.parse(orderJson);
      calls.push(['ownedSign', validatorId, order.inputs[0].id]);
      if (order.inputs[0].id === 'locked-object') {
        return {
          ok: false,
          error: {
            code: 'owned_sign_failed',
            message: 'owned-sign refused: input locked-object v1 is locked by a different order',
          },
        };
      }
      return {
        ok: true,
        result: {
          validator_id: validatorId,
          signature_hex: `sig-${validatorId}`,
        },
      };
    },
    async ownedApply(certJson) {
      const cert = JSON.parse(certJson);
      calls.push(['ownedApply', cert.order.inputs[0].id, cert.votes.map(vote => vote.validator_id)]);
      return { ok: true, result: { applied_count: 5 } };
    },
  };
  const builder = new TxBuilder(rpc, () => wasm);

  const result = await builder.sendOwnedTransfer(
    'backup-json',
    'owner-pk',
    [
      { id: 'locked-object', version: 1, value: 2_000_001, asset: 'PFT' },
      { id: 'fresh-object', version: 1, value: 2_000_001, asset: 'PFT' },
    ],
    'recipient-pk',
    1,
    0.000001,
    validators,
  );

  assert.equal(result.input.id, 'fresh-object');
  assert.equal(result.attempts.length, 2);
  assert.equal(result.attempts[0].input_id, 'locked-object');
  assert.equal(result.attempts[0].votes, 0);
  assert.equal(result.attempts[0].failures.length, 6);
  assert.equal(result.votes.length, 5);
  assert.deepEqual(calls.at(-1), [
    'ownedApply',
    'fresh-object',
    ['validator-0', 'validator-1', 'validator-2', 'validator-3', 'validator-4'],
  ]);
});

test('sendOwnedTransfer signs locally and never sends the wallet backup to the proxy', async () => {
  const validators = Array.from({ length: 6 }, (_, index) => ({ node_id: `validator-${index}` }));
  let localSignerBackup = null;
  const wasm = {
    wallet_sign_owned_transfer(backupJson, orderJson) {
      localSignerBackup = backupJson;
      return {
        order: JSON.parse(orderJson),
        owner_pubkey_hex: 'owner-pk',
        owner_signature_hex: 'local-owner-sig',
      };
    },
  };
  const rpc = {
    serverCapabilities: fastPayCapabilities,
    async walletSignOwnedTransfer() {
      throw new Error('wallet backup crossed the self-custody boundary');
    },
    async ownedSign(_orderJson, validatorId) {
      return {
        ok: true,
        result: {
          validator_id: validatorId,
          signature_hex: `sig-${validatorId}`,
        },
      };
    },
    async ownedApply(certJson) {
      const cert = JSON.parse(certJson);
      assert.equal(cert.owner_signature_hex, 'local-owner-sig');
      return { ok: true, result: { applied_count: 5 } };
    },
  };
  const builder = new TxBuilder(rpc, () => wasm);

  const result = await builder.sendOwnedTransfer(
    'backup-json',
    'owner-pk',
    [{ id: 'object-1', version: 1, value: 2_000_001, asset: 'PFT' }],
    'recipient-pk',
    1,
    0.000001,
    validators,
  );

  assert.equal(localSignerBackup, 'backup-json');
  assert.equal(result.cert.owner_signature_hex, 'local-owner-sig');
});

test('unwrapOwnedTransfer signs amount-based unwrap and applies quorum certificate', async () => {
  const calls = [];
  const validators = Array.from({ length: 6 }, (_, index) => ({ node_id: `validator-${index}` }));
  const wasm = {
    wallet_sign_owned_unwrap(backupJson, orderJson) {
      const order = JSON.parse(orderJson);
      calls.push(['wallet_sign_owned_unwrap', backupJson, order]);
      return {
        owner_pubkey_hex: 'owner-pk',
        owner_signature_hex: 'owner-unwrap-sig',
      };
    },
  };
  const rpc = {
    serverCapabilities: fastPayCapabilities,
    async ownedUnwrapSign(orderJson, validatorId) {
      calls.push(['ownedUnwrapSign', validatorId, JSON.parse(orderJson)]);
      if (validatorId === 'validator-5') {
        return new Promise(() => {});
      }
      return {
        ok: true,
        result: {
          validator_id: validatorId,
          signature_hex: `sig-${validatorId}`,
        },
      };
    },
    async ownedUnwrapApply(certJson) {
      const cert = JSON.parse(certJson);
      calls.push(['ownedUnwrapApply', cert]);
      return {
        ok: true,
        result: {
          credited: 1_500_000,
          credited_to: 'pf-account',
        },
      };
    },
  };
  const builder = new TxBuilder(rpc, () => wasm);

  const result = await builder.unwrapOwnedTransfer(
    'backup-json',
    'owner-pk',
    [
      { id: 'large-object', version: 1, value: 3_000_000, asset: 'PFT' },
      { id: 'small-object', version: 2, value: 2_000_000, asset: 'PFT' },
    ],
    'pf-account',
    1.5,
    0,
    validators,
  );

  assert.equal(result.input.id, 'small-object');
  assert.equal(result.order.to_address, 'pf-account');
  assert.equal(result.order.amount, 1_500_000);
  assert.deepEqual(result.order.inputs, [{ id: 'small-object', version: 2 }]);
  assert.equal(result.votes.length, 5);
  assert.deepEqual(result.votes.map(vote => vote.validator_id), [
    'validator-0',
    'validator-1',
    'validator-2',
    'validator-3',
    'validator-4',
  ]);
  const applyCall = calls.find(call => call[0] === 'ownedUnwrapApply');
  assert.ok(applyCall);
  assert.equal(applyCall[1].owner_pubkey_hex, 'owner-pk');
  assert.equal(applyCall[1].owner_signature_hex, 'owner-unwrap-sig');
  assert.deepEqual(applyCall[1].votes.map(vote => vote.validator_id), [
    'validator-0',
    'validator-1',
    'validator-2',
    'validator-3',
    'validator-4',
  ]);
  const signCalls = calls.filter(call => call[0] === 'ownedUnwrapSign');
  assert.equal(signCalls.length, 6);
  for (const [, , envelope] of signCalls) {
    assert.deepEqual(envelope, {
      order: result.order,
      owner_pubkey_hex: 'owner-pk',
      owner_signature_hex: 'owner-unwrap-sig',
    });
  }
});

test('unwrapOwnedTransfer signs locally and never sends the wallet backup to the proxy', async () => {
  const validators = Array.from({ length: 6 }, (_, index) => ({ node_id: `validator-${index}` }));
  let localSignerBackup = null;
  const wasm = {
    wallet_sign_owned_unwrap(backupJson, orderJson) {
      localSignerBackup = backupJson;
      return {
        order: JSON.parse(orderJson),
        owner_pubkey_hex: 'owner-pk',
        owner_signature_hex: 'local-unwrap-owner-sig',
      };
    },
  };
  const rpc = {
    serverCapabilities: fastPayCapabilities,
    async walletSignOwnedUnwrap() {
      throw new Error('wallet backup crossed the self-custody boundary');
    },
    async ownedUnwrapSign(_orderJson, validatorId) {
      return {
        ok: true,
        result: {
          validator_id: validatorId,
          signature_hex: `sig-${validatorId}`,
        },
      };
    },
    async ownedUnwrapApply(certJson) {
      const cert = JSON.parse(certJson);
      assert.equal(cert.owner_signature_hex, 'local-unwrap-owner-sig');
      return { ok: true, result: { credited: 1_000_000, credited_to: 'pf-account' } };
    },
  };
  const builder = new TxBuilder(rpc, () => wasm);

  const result = await builder.unwrapOwnedTransfer(
    'backup-json',
    'owner-pk',
    [{ id: 'object-1', version: 1, value: 1_000_000, asset: 'PFT' }],
    'pf-account',
    1,
    0,
    validators,
  );
  assert.equal(localSignerBackup, 'backup-json');
  assert.equal(result.cert.owner_signature_hex, 'local-unwrap-owner-sig');
});

test('unwrapOwnedTransfer combines fragmented FastPay objects for standard unwrap', async () => {
  const validators = Array.from({ length: 6 }, (_, index) => ({ node_id: `validator-${index}` }));
  const expectedInputs = Array.from({ length: 20 }, (_, index) => ({ id: `object-${index}`, version: 1 }));
  const wasm = {
    wallet_sign_owned_unwrap(_backupJson, orderJson) {
      const order = JSON.parse(orderJson);
      assert.deepEqual(order.inputs, expectedInputs);
      assert.equal(order.amount, 1_950_000);
      return {
        owner_pubkey_hex: 'owner-pk',
        owner_signature_hex: 'owner-unwrap-sig',
      };
    },
  };
  const rpc = {
    serverCapabilities: fastPayCapabilities,
    async ownedUnwrapSign(orderJson, validatorId) {
      const { order } = JSON.parse(orderJson);
      assert.equal(order.inputs.length, 20);
      return {
        ok: true,
        result: {
          validator_id: validatorId,
          signature_hex: `sig-${validatorId}`,
        },
      };
    },
    async ownedUnwrapApply(certJson) {
      const cert = JSON.parse(certJson);
      assert.equal(cert.order.inputs.length, 20);
      assert.equal(cert.votes.length, 5);
      return {
        ok: true,
        result: {
          credited: 1_950_000,
          credited_to: 'pf-account',
          change_object: { value: 50_000 },
        },
      };
    },
  };
  const builder = new TxBuilder(rpc, () => wasm);

  const result = await builder.unwrapOwnedTransfer(
    'backup-json',
    'owner-pk',
    Array.from({ length: 20 }, (_, index) => ({
      id: `object-${index}`,
      version: 1,
      value: 100_000,
      asset: 'PFT',
    })),
    'pf-account',
    1.95,
    0,
    validators,
  );

  assert.equal(result.inputs.length, 20);
  assert.deepEqual(result.order.inputs, expectedInputs);
  assert.equal(result.applyResult.change_object.value, 50_000);
});

test('unwrapOwnedTransfer v3 combines objects and requires authenticated apply quorum', async () => {
  const lockId = '1'.repeat(96);
  const certificateDigest = '2'.repeat(96);
  const wasm = {
    wallet_fastpay_unwrap_lock_id: () => lockId,
    wallet_sign_owned_unwrap_v3(_backupJson, orderJson) {
      return {
        order: JSON.parse(orderJson),
        owner_pubkey_hex: 'owner-pk',
        owner_signature_hex: 'owner-sig',
      };
    },
    wallet_fastpay_unwrap_certificate_digest: () => certificateDigest,
    wallet_verify_fastpay_apply_ack: () => true,
  };
  const rpc = {
    async ownedRecoveryCapabilities() {
      return { ok: true, result: FASTPAY_V3_CAPABILITIES };
    },
    async ownedUnwrapSignV3(_signedOrderJson, validatorId) {
      return { ok: true, result: { validator_id: validatorId, signature_hex: `vote-${validatorId}` } };
    },
    async ownedUnwrapApplyV3(certJson) {
      const certificate = JSON.parse(certJson);
      assert.equal(certificate.order.inputs.length, 2);
      return {
        ok: true,
        result: {
          validators: FASTPAY_V3_VALIDATORS.slice(0, 3).map(validator => ({
            validator_id: validator.node_id,
            ok: true,
            result: {
              schema: 'postfiat-fastpay-apply-ack-v1',
              domain: FASTPAY_V3_CAPABILITIES.domain,
              committee_epoch: 7,
              lock_id: lockId,
              order_digest: '4'.repeat(96),
              certificate_digest: certificateDigest,
              terminal_state_digest: '3'.repeat(96),
              validator_id: validator.node_id,
              signature_hex: `ack-${validator.node_id}`,
            },
          })),
        },
      };
    },
  };
  const builder = new TxBuilder(rpc, () => wasm);
  const result = await builder.unwrapOwnedTransfer(
    'backup-json',
    'owner-pk',
    [
      { id: 'fragment-a', version: 1, value: 600_000, asset: 'PFT' },
      { id: 'fragment-b', version: 2, value: 500_000, asset: 'PFT' },
    ],
    'pf-destination',
    1,
    0,
    FASTPAY_V3_VALIDATORS,
  );
  assert.equal(result.status, 'finalized');
  assert.equal(result.inputs.length, 2);
  assert.equal(result.acknowledgements.length, 3);
});

test('publishPublicKey signs a 1-atom self-transfer through the Account-lane finality path', async () => {
  const calls = [];
  const signedTransfer = { signed: 'v1', public_key_hex: 'wallet-pk-hex' };
  const wasm = {
    wallet_sign_transfer(backupJson, quoteJson) {
      calls.push(['wallet_sign_transfer', backupJson, JSON.parse(quoteJson)]);
      return signedTransfer;
    },
    wallet_sign_payment_v2() {
      throw new Error('wallet_sign_payment_v2 should not be called for publish');
    },
  };
  const quote = {
    chain_id: 'postfiat-wan-devnet',
    genesis_hash: 'a'.repeat(96),
    protocol_version: 1,
    minimum_fee: 1,
    sequence: 0,
    sender_meets_reserve_after_transfer: true,
  };
  const rpc = {
    async transferFeeQuote(from, to, amount) {
      calls.push(['transferFeeQuote', from, to, amount]);
      assert.equal(from, 'pf-self');
      assert.equal(to, 'pf-self');
      assert.equal(amount, 1); // smallest non-zero amount
      return { ok: true, result: quote };
    },
    async submitSignedTransferFinality(signedJson) {
      calls.push(['submitSignedTransferFinality', JSON.parse(signedJson)]);
      return { ok: true, result: { tx_id: 'tx-publish', finality: { local_hot_finality: [{ receipt: { accepted: true, code: 'accepted', tx_id: 'tx-publish' } }] } } };
    },
  };

  const builder = new TxBuilder(rpc, () => wasm);
  const result = await builder.publishPublicKey('backup-json', 'pf-self');

  assert.equal(result.txId, 'tx-publish');
  assert.equal(result.receipt.accepted, true);
  // Verify the self-transfer shape: from === to === address, amount === 1 atom.
  assert.deepEqual(calls, [
    ['transferFeeQuote', 'pf-self', 'pf-self', 1],
    ['wallet_sign_transfer', 'backup-json', quote],
    ['submitSignedTransferFinality', signedTransfer],
  ]);
});

test('publishPublicKey rejects when the wallet is not unlocked', async () => {
  const builder = new TxBuilder({}, () => ({}));
  await assert.rejects(
    () => builder.publishPublicKey(null, 'pf-self'),
    /Wallet not unlocked/,
  );
  await assert.rejects(
    () => builder.publishPublicKey('backup-json', ''),
    /Wallet address is missing/,
  );
});

test('publishPublicKey fails closed unless the final receipt has the explicit accepted code', async () => {
  const wasm = { wallet_sign_transfer() { return { signed: 'v1' }; } };
  const rpc = {
    async transferFeeQuote() {
      return { ok: true, result: { sender_meets_reserve_after_transfer: true } };
    },
    async submitSignedTransferFinality() {
      return {
        ok: true,
        result: {
          tx_id: 'tx-ambiguous',
          finality: {
            local_hot_finality: [{
              receipt: { accepted: true, tx_id: 'tx-ambiguous' },
            }],
          },
        },
      };
    },
  };
  const builder = new TxBuilder(rpc, () => wasm);

  await assert.rejects(
    () => builder.publishPublicKey('backup-json', 'pf-self'),
    /explicit accepted receipt code/,
  );
});

test('ensurePublicKeyPublished is a no-op when the key is already published', async () => {
  const calls = [];
  const wasm = {
    wallet_sign_transfer() { calls.push('wallet_sign_transfer'); return {}; },
  };
  const rpc = {
    async transferFeeQuote() { calls.push('transferFeeQuote'); return { ok: true, result: {} }; },
    async submitSignedTransferFinality() { calls.push('submitSignedTransferFinality'); return { ok: true, result: {} }; },
  };
  const builder = new TxBuilder(rpc, () => wasm);

  const out = await builder.ensurePublicKeyPublished('backup-json', 'pf-self', 'already-published-hex');

  assert.equal(out.published, true);
  assert.equal(out.result, undefined);
  assert.deepEqual(calls, []); // no signing, no RPC
});

test('ensurePublicKeyPublished publishes (1-atom self-transfer) when not yet published', async () => {
  const calls = [];
  const signedTransfer = { signed: 'v1', public_key_hex: 'pk' };
  const wasm = {
    wallet_sign_transfer() { calls.push('wallet_sign_transfer'); return signedTransfer; },
  };
  const quote = {
    chain_id: 'postfiat-wan-devnet', genesis_hash: 'a'.repeat(96), protocol_version: 1,
    minimum_fee: 1, sequence: 0, sender_meets_reserve_after_transfer: true,
  };
  const rpc = {
    async transferFeeQuote(from, to, amount) {
      calls.push(['transferFeeQuote', from, to, amount]);
      return { ok: true, result: quote };
    },
    async submitSignedTransferFinality() {
      calls.push('submitSignedTransferFinality');
      return { ok: true, result: { tx_id: 'tx', finality: { local_hot_finality: [{ receipt: { accepted: true, code: 'accepted', tx_id: 'tx' } }] } } };
    },
  };
  const builder = new TxBuilder(rpc, () => wasm);

  const out = await builder.ensurePublicKeyPublished('backup-json', 'pf-self', null);

  assert.equal(out.published, true);
  assert.ok(out.result); // publish ran
  assert.deepEqual(calls, [
    ['transferFeeQuote', 'pf-self', 'pf-self', 1],
    'wallet_sign_transfer',
    'submitSignedTransferFinality',
  ]);
});

test('ensurePublicKeyPublished throws if the publish self-transfer is rejected', async () => {
  const wasm = { wallet_sign_transfer() { return {}; } };
  const rpc = {
    async transferFeeQuote() { return { ok: true, result: { sender_meets_reserve_after_transfer: true, minimum_fee: 1, sequence: 0, chain_id: 'c', genesis_hash: 'g', protocol_version: 1 } }; },
    async submitSignedTransferFinality() {
      return { ok: true, result: { tx_id: 'tx', finality: { local_hot_finality: [{ receipt: { accepted: false, code: 'x', message: 'nope', tx_id: 'tx' } }] } } };
    },
  };
  const builder = new TxBuilder(rpc, () => wasm);
  await assert.rejects(
    () => builder.ensurePublicKeyPublished('backup-json', 'pf-self', false),
    /explicit accepted receipt code/,
  );
});
