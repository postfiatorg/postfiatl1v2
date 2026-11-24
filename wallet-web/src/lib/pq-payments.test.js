import { test } from 'node:test';
import assert from 'node:assert/strict';
import { checkPublicTerms } from './pq-payments.js';

test('signer review binds every public payment field before relay', () => {
  for (const mode of ['Shield','Internal','Exit']) {
    const terms = { mode, asset:'cd'.repeat(48), amount:'9007199254740993', fee:'3',
      recipient:mode === 'Exit' ? 'pf' + '33'.repeat(20) : null,
      payer:mode === 'Shield' ? 'pf' + '21'.repeat(20) : null, sequence:mode === 'Shield' ? '9007199254740995' : null };
    const payment = { mode, asset:terms.asset, fee:terms.fee, recipient:terms.recipient, payer:terms.payer,
      sequence:terms.sequence, mint:mode === 'Shield' ? terms.amount : '0',
      exit:mode === 'Exit' ? terms.amount : '0', action_hash:'11'.repeat(48) };
    checkPublicTerms(payment,terms,payment.action_hash);
    for (const key of Object.keys(payment)) {
      assert.throws(() => checkPublicTerms({...payment,[key]:'different'},terms,payment.action_hash), /reviewed terms/);
    }
  }
});
