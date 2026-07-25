#!/usr/bin/env node

import { readFileSync } from 'node:fs'
import {
  broadcast,
  buildClaim,
  buildLock,
  buildRefund,
  buildSplit,
  coreStatus,
  coreTx,
  extractClaimPreimage,
  initWallets,
  testMempool,
} from './bitcoin_htlc.mjs'

function readJson(path) {
  return JSON.parse(readFileSync(path, 'utf8'))
}

function output(value) {
  process.stdout.write(`${JSON.stringify(value, null, 2)}\n`)
}

const [command, ...args] = process.argv.slice(2)
try {
  if (command === 'init' && args.length === 1) {
    output(initWallets(args[0]))
  } else if (command === 'status' && args.length === 0) {
    output(coreStatus())
  } else if (command === 'split' && args.length === 2) {
    output(buildSplit(args[0], readJson(args[1])))
  } else if (command === 'lock' && args.length === 2) {
    output(buildLock(args[0], readJson(args[1])))
  } else if (command === 'claim' && args.length === 3) {
    output(buildClaim(args[0], readJson(args[1]), args[2]))
  } else if (command === 'refund' && args.length === 2) {
    output(buildRefund(args[0], readJson(args[1])))
  } else if (command === 'test' && args.length === 1) {
    output(testMempool(readJson(args[0]).raw_tx))
  } else if (command === 'broadcast' && args.length === 1) {
    output(broadcast(readJson(args[0]).raw_tx))
  } else if (command === 'tx' && args.length === 1) {
    output(coreTx(args[0]))
  } else if (command === 'extract' && args.length === 2) {
    output({
      preimage: extractClaimPreimage(readJson(args[0]).raw_tx, args[1]),
    })
  } else {
    throw new Error(
      'usage: btc_ops.mjs init ROOT | status | split ROOT UTXO_JSON | ' +
        'lock ROOT REQUEST_JSON | claim ROOT LOCK_JSON PREIMAGE | ' +
        'refund ROOT LOCK_JSON | test TX_JSON | broadcast TX_JSON | ' +
        'tx TXID | extract CLAIM_JSON HASH',
    )
  }
} catch (error) {
  process.stderr.write(
    `${JSON.stringify({ status: 'ERROR', error: error.message }, null, 2)}\n`,
  )
  process.exitCode = 1
}
