#!/usr/bin/env node

import { createHash } from 'node:crypto'
import { readFileSync } from 'node:fs'
import * as bitcoin from 'bitcoinjs-lib'
import { htlcScript } from './bitcoin_htlc.mjs'

const NETWORK = bitcoin.networks.testnet

function readJson(path) {
  return JSON.parse(readFileSync(path, 'utf8'))
}

function sha256(bytes) {
  return createHash('sha256').update(bytes).digest()
}

function txFromEvidence(record, expectedTxid) {
  const raw = record.explorer.raw_tx
  if (typeof raw !== 'string' || !/^[0-9a-f]+$/.test(raw)) {
    throw new Error(`transaction ${expectedTxid} has invalid raw hex`)
  }
  const tx = bitcoin.Transaction.fromHex(raw)
  if (tx.getId() !== expectedTxid || record.txid !== expectedTxid) {
    throw new Error(`transaction ${expectedTxid} has a txid mismatch`)
  }
  if (
    !record.explorer.transaction.status.confirmed ||
    record.explorer.transaction.txid !== expectedTxid
  ) {
    throw new Error(`transaction ${expectedTxid} is not confirmed`)
  }
  return tx
}

function outpointTxid(input) {
  return Buffer.from(input.hash).reverse().toString('hex')
}

function verifyLock(scenario) {
  const lock = scenario.bitcoin_htlc
  const record = scenario.bitcoin_evidence.lock
  const tx = txFromEvidence(record, lock.txid)
  const accounts = scenario._accounts
  const script = htlcScript({
    digestHex: lock.payment_hash,
    claimPubkeyHex: accounts[lock.recipient].pubkey,
    refundPubkeyHex: accounts[lock.owner].pubkey,
    lockHeight: lock.lock_height,
  })
  if (script.toString('hex') !== lock.witness_script_hex) {
    throw new Error(`${lock.scenario} witness script mismatch`)
  }
  const payment = bitcoin.payments.p2wsh({
    redeem: { output: script, network: NETWORK },
    network: NETWORK,
  })
  if (
    payment.address !== lock.p2wsh_address ||
    !Buffer.from(tx.outs[lock.vout].script).equals(payment.output) ||
    tx.outs[lock.vout].value !== BigInt(lock.amount_sats)
  ) {
    throw new Error(`${lock.scenario} P2WSH output mismatch`)
  }
  if (
    lock.input_value_sats !==
      lock.amount_sats + lock.change_sats + lock.fee_sats ||
    !lock.conservation
  ) {
    throw new Error(`${lock.scenario} lock conservation failed`)
  }
  return { tx, script }
}

function verifyClaim(scenario, lockResult) {
  const lock = scenario.bitcoin_htlc
  const record = scenario.bitcoin_evidence.claim
  const txid = scenario.transactions.bitcoin_claim
  const tx = txFromEvidence(record, txid)
  if (
    tx.ins.length !== 1 ||
    outpointTxid(tx.ins[0]) !== lock.txid ||
    tx.ins[0].index !== lock.vout ||
    tx.ins[0].witness.length !== 4
  ) {
    throw new Error(`${lock.scenario} claim input mismatch`)
  }
  const witness = tx.ins[0].witness.map((item) => Buffer.from(item))
  const preimage = witness[1]
  if (
    preimage.length !== 32 ||
    !sha256(preimage).equals(Buffer.from(lock.payment_hash, 'hex')) ||
    preimage.toString('hex') !== scenario.public_preimage_hex ||
    witness[2].length !== 1 ||
    witness[2][0] !== 1 ||
    !witness[3].equals(lockResult.script)
  ) {
    throw new Error(`${lock.scenario} claim witness mismatch`)
  }
  const built = readJson(
    record._built_path,
  )
  if (
    tx.outs.reduce((sum, output) => sum + output.value, 0n) +
      BigInt(built.fee_sats) !==
      BigInt(lock.amount_sats) ||
    !built.conservation
  ) {
    throw new Error(`${lock.scenario} claim conservation failed`)
  }
  return preimage.toString('hex')
}

function verifyRefund(scenario, lockResult) {
  const lock = scenario.bitcoin_htlc
  const record = scenario.bitcoin_evidence.refund
  const txid = scenario.transactions.bitcoin_refund
  const tx = txFromEvidence(record, txid)
  if (
    tx.ins.length !== 1 ||
    outpointTxid(tx.ins[0]) !== lock.txid ||
    tx.ins[0].index !== lock.vout ||
    tx.ins[0].witness.length !== 3 ||
    tx.locktime < lock.lock_height
  ) {
    throw new Error('refund transaction does not satisfy CLTV structure')
  }
  const witness = tx.ins[0].witness.map((item) => Buffer.from(item))
  if (witness[1].length !== 0 || !witness[2].equals(lockResult.script)) {
    throw new Error('refund transaction selected the wrong script branch')
  }
  const built = readJson(record._built_path)
  if (
    tx.outs.reduce((sum, output) => sum + output.value, 0n) +
      BigInt(built.fee_sats) !==
      BigInt(lock.amount_sats) ||
    !built.conservation
  ) {
    throw new Error('refund transaction conservation failed')
  }
  return true
}

async function publicTransactionMatches(record) {
  let lastError
  for (let attempt = 1; attempt <= 5; attempt += 1) {
    try {
      const response = await fetch(
        `https://mempool.space/signet/api/tx/${record.txid}/hex`,
      )
      if (!response.ok) {
        throw new Error(`HTTP ${response.status}`)
      }
      const raw = (await response.text()).trim()
      if (raw !== record.explorer.raw_tx) {
        throw new Error(
          `public Signet raw transaction mismatch for ${record.txid}`,
        )
      }
      return
    } catch (error) {
      lastError = error
      if (attempt < 5) {
        await new Promise((resolve) => setTimeout(resolve, attempt * 1_000))
      }
    }
  }
  throw new Error(
    `public Signet lookup failed for ${record.txid}: ${lastError.message}`,
  )
}

async function main() {
  const [reportPath] = process.argv.slice(2)
  if (!reportPath) {
    throw new Error('usage: verify_bitcoin_evidence.mjs LIVE_DEMO_REPORT')
  }
  const report = readJson(reportPath)
  if (
    report.schema !== 'postfiat.bitcoin_signet_navcoin.live_demo.v1' ||
    report.result !== 'PASS' ||
    report.networks.bitcoin.network !== 'signet'
  ) {
    throw new Error('live report is not a PASS Bitcoin Signet report')
  }
  const accounts = report.networks.bitcoin
  const scenarios = report.scenarios
  const publicRecords = []
  const preimages = []
  for (const name of ['btc_to_nav_happy', 'nav_to_btc_happy']) {
    const scenario = scenarios[name]
    scenario._accounts = {
      user: accounts.user,
      coordinator: accounts.coordinator,
    }
    scenario.bitcoin_evidence.claim._built_path = reportPath.replace(
      'live-demo-report.json',
      `bitcoin/${name === 'btc_to_nav_happy' ? 'onramp-04-btc-claim' : 'offramp-03-btc-claim'}.built.json`,
    )
    const lockResult = verifyLock(scenario)
    preimages.push(verifyClaim(scenario, lockResult))
    publicRecords.push(
      scenario.bitcoin_evidence.lock,
      scenario.bitcoin_evidence.claim,
    )
  }
  const refund = scenarios.refund
  refund._accounts = {
    user: accounts.user,
    coordinator: accounts.coordinator,
  }
  refund.bitcoin_evidence.refund._built_path = reportPath.replace(
    'live-demo-report.json',
    'bitcoin/refund-04-btc-refund.built.json',
  )
  const refundLock = verifyLock(refund)
  verifyRefund(refund, refundLock)
  publicRecords.push(refund.bitcoin_evidence.lock, refund.bitcoin_evidence.refund)

  const splitRecord = readJson(
    reportPath.replace(
      'live-demo-report.json',
      'bitcoin/00-faucet-split.confirmed.json',
    ),
  )
  txFromEvidence(splitRecord, report.networks.bitcoin.split_txid)
  const splitBuilt = readJson(
    reportPath.replace(
      'live-demo-report.json',
      'bitcoin/00-faucet-split.built.json',
    ),
  )
  if (
    !splitBuilt.conservation ||
    splitBuilt.input.valueSats !==
      splitBuilt.outputs.reduce((sum, output) => sum + output.value_sats, 0) +
        splitBuilt.fee_sats
  ) {
    throw new Error('faucet split conservation failed')
  }
  publicRecords.push(splitRecord)
  await Promise.all(publicRecords.map(publicTransactionMatches))

  const conservation = report.conservation
  if (
    !conservation.bitcoin_exact ||
    conservation.faucet_input_sats !==
      conservation.final_user_coordinator_controlled_sats +
        conservation.miner_fee_sats
  ) {
    throw new Error('global Bitcoin satoshi conservation failed')
  }
  if (
    new Set(preimages).size !== 2 ||
    report.public_preimages_authenticated !== 2 ||
    report.refund_preimage_revealed
  ) {
    throw new Error('public preimage disclosure accounting failed')
  }
  process.stdout.write(
    `${JSON.stringify(
      {
        network: 'signet',
        confirmed_transactions: publicRecords.map((record) => record.txid),
        public_preimages_authenticated: preimages.length,
        refund_preimage_revealed: false,
        faucet_input_sats: conservation.faucet_input_sats,
        final_controlled_sats:
          conservation.final_user_coordinator_controlled_sats,
        miner_fee_sats: conservation.miner_fee_sats,
        exact_conservation: true,
      },
      null,
      2,
    )}\n`,
  )
}

main().catch((error) => {
  process.stderr.write(`${error.stack || error.message}\n`)
  process.exitCode = 1
})
