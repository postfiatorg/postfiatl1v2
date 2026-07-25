#!/usr/bin/env node

import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import {
  buildClaim,
  buildLock,
  buildRefund,
  buildSplit,
  extractClaimPreimage,
} from './bitcoin_htlc.mjs'

const runtimeRoot = '/home/postfiat/tmp/pftl-btc-navcoin-20260725'
const secret = '42'.repeat(32)
const digest = createHash('sha256')
  .update(Buffer.from(secret, 'hex'))
  .digest('hex')
const split = buildSplit(runtimeRoot, {
  txid: '11'.repeat(32),
  vout: 0,
  valueSats: 100_000,
})
assert.equal(split.conservation, true)
const lock = buildLock(runtimeRoot, {
  scenario: 'unit',
  owner: 'user',
  recipient: 'coordinator',
  inputTxid: split.txid,
  inputVout: 0,
  inputValueSats: 25_000,
  digestHex: digest,
  lockHeight: 400_000,
  amountSats: 20_000,
})
assert.equal(lock.conservation, true)
const claim = buildClaim(runtimeRoot, lock, secret)
assert.equal(claim.conservation, true)
assert.equal(extractClaimPreimage(claim.raw_tx, digest), secret)
assert.throws(
  () => extractClaimPreimage(claim.raw_tx, '00'.repeat(32)),
  /does not satisfy payment hash/,
)
const refund = buildRefund(runtimeRoot, lock)
assert.equal(refund.conservation, true)
assert.notEqual(claim.txid, refund.txid)
process.stdout.write('bitcoin HTLC offline tests: PASS\n')
