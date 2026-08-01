#!/usr/bin/env node

/**
 * Build and locally validate a Bitcoin-mainnet P2WSH HTLC plan.
 *
 * This module deliberately has no RPC broadcast function. It emits unsigned
 * PSBTs only. Fully signed transactions are constructed transiently to measure
 * vsize and validate both claim/refund witnesses, then discarded.
 */

import { createHash } from 'node:crypto'
import { readFileSync, writeFileSync } from 'node:fs'
import * as bitcoin from 'bitcoinjs-lib'
import * as ecc from 'tiny-secp256k1'
import { ECPairFactory } from 'ecpair'
import { htlcScript } from './bitcoin_htlc.mjs'

bitcoin.initEccLib(ecc)
const ECPair = ECPairFactory(ecc)
const NETWORK = bitcoin.networks.bitcoin

function readJson(path) {
  return JSON.parse(readFileSync(path, 'utf8'))
}

function p2wpkh(keyPair) {
  const payment = bitcoin.payments.p2wpkh({
    pubkey: Buffer.from(keyPair.publicKey),
    network: NETWORK,
  })
  if (!payment.address || !payment.output) throw new Error('P2WPKH derivation failed')
  return payment
}

function serializeWitness(stack) {
  const varInt = (value) => {
    if (value < 0xfd) return Buffer.from([value])
    if (value <= 0xffff) {
      const output = Buffer.alloc(3)
      output[0] = 0xfd
      output.writeUInt16LE(value, 1)
      return output
    }
    throw new Error('witness item is unexpectedly large')
  }
  const parts = [varInt(stack.length)]
  for (const item of stack) {
    const bytes = Buffer.from(item)
    parts.push(varInt(bytes.length), bytes)
  }
  return Buffer.concat(parts)
}

function finalizedHtlc(psbt, stackBuilder) {
  psbt.finalizeInput(0, (_index, input) => ({
    finalScriptWitness: serializeWitness(
      stackBuilder(Buffer.from(input.partialSig[0].signature)),
    ),
  }))
  return psbt.extractTransaction()
}

const [inputPath, outputPath] = process.argv.slice(2)
if (!inputPath || !outputPath) {
  throw new Error('usage: mainnet_plan.mjs INPUT_JSON OUTPUT_JSON')
}
const input = readJson(inputPath)
const sourceRecord = readJson(input.source_key_file)
const coordinatorRecord = readJson(input.coordinator_key_file)
const secrets = readJson(input.secrets_file)
const preimage = Buffer.from(secrets.secrets.btc, 'hex')
const digest = createHash('sha256').update(preimage).digest()
if (digest.toString('hex') !== input.payment_hash) {
  throw new Error('stored BTC secret does not match the public payment hash')
}
const sourceKey = ECPair.fromWIF(sourceRecord.private_key_wif, NETWORK)
const coordinatorKey = ECPair.fromWIF(
  coordinatorRecord.private_key_wif,
  NETWORK,
)
const source = p2wpkh(sourceKey)
const coordinator = p2wpkh(coordinatorKey)
if (source.address !== sourceRecord.address) {
  throw new Error('source key does not derive the configured mainnet address')
}
if (coordinator.address !== coordinatorRecord.address) {
  throw new Error('coordinator key does not derive its configured mainnet address')
}
if (input.utxo.confirmations < 1) {
  throw new Error('BTC funding UTXO requires at least one confirmation')
}

const witnessScript = htlcScript({
  digestHex: input.payment_hash,
  claimPubkeyHex: Buffer.from(coordinatorKey.publicKey).toString('hex'),
  refundPubkeyHex: Buffer.from(sourceKey.publicKey).toString('hex'),
  lockHeight: input.lock_height,
})
const htlc = bitcoin.payments.p2wsh({
  redeem: { output: witnessScript, network: NETWORK },
  network: NETWORK,
})
if (!htlc.address || !htlc.output) throw new Error('P2WSH derivation failed')

const amount = BigInt(input.amount_sats)
const inputValue = BigInt(input.utxo.value_sats)
const lockFee = BigInt(input.lock_fee_sats)
const claimFee = BigInt(input.claim_fee_sats)
const refundFee = BigInt(input.refund_fee_sats)
const change = inputValue - amount - lockFee
if (amount <= claimFee || amount <= refundFee || change < 546n) {
  throw new Error('BTC fee plan would create a non-positive or dust output')
}

const lock = new bitcoin.Psbt({ network: NETWORK })
lock.addInput({
  hash: input.utxo.txid,
  index: input.utxo.vout,
  sequence: 0xfffffffd,
  witnessUtxo: { script: source.output, value: inputValue },
})
lock.addOutput({ address: htlc.address, value: amount })
lock.addOutput({ address: source.address, value: change })
const unsignedLockPsbt = lock.toBase64()
lock.signInput(0, sourceKey)
lock.finalizeAllInputs()
const signedLock = lock.extractTransaction()

const claimOutput = amount - claimFee
const claim = new bitcoin.Psbt({ network: NETWORK })
claim.addInput({
  hash: signedLock.getId(),
  index: 0,
  witnessUtxo: { script: htlc.output, value: amount },
  witnessScript,
})
claim.addOutput({ address: coordinator.address, value: claimOutput })
const unsignedClaimPsbt = claim.toBase64()
claim.signInput(0, coordinatorKey)
const signedClaim = finalizedHtlc(claim, (signature) => [
  signature,
  preimage,
  Buffer.from([1]),
  witnessScript,
])

const refundOutput = amount - refundFee
const refund = new bitcoin.Psbt({ network: NETWORK })
refund.setLocktime(input.lock_height)
refund.addInput({
  hash: signedLock.getId(),
  index: 0,
  sequence: 0xfffffffe,
  witnessUtxo: { script: htlc.output, value: amount },
  witnessScript,
})
refund.addOutput({ address: source.address, value: refundOutput })
const unsignedRefundPsbt = refund.toBase64()
refund.signInput(0, sourceKey)
const signedRefund = finalizedHtlc(refund, (signature) => [
  signature,
  Buffer.alloc(0),
  witnessScript,
])

const result = {
  schema: 'postfiat.real_money_htlc.bitcoin_mainnet_plan.v1',
  network: 'bitcoin-mainnet',
  broadcast_capability: false,
  source_address: source.address,
  coordinator_address: coordinator.address,
  coordinator_public_key: Buffer.from(coordinatorKey.publicKey).toString('hex'),
  payment_hash: input.payment_hash,
  p2wsh_address: htlc.address,
  witness_script_hex: witnessScript.toString('hex'),
  lock_height: input.lock_height,
  funding_utxo: input.utxo,
  amount_sats: input.amount_sats,
  unsigned: {
    lock_psbt_base64: unsignedLockPsbt,
    lock_txid_if_signed_without_output_changes: signedLock.getId(),
    claim_psbt_base64: unsignedClaimPsbt,
    refund_psbt_base64: unsignedRefundPsbt,
  },
  fees: {
    lock_fee_sats: input.lock_fee_sats,
    lock_vsize: signedLock.virtualSize(),
    lock_feerate_sat_vb: Number(lockFee) / signedLock.virtualSize(),
    claim_fee_sats: input.claim_fee_sats,
    claim_vsize: signedClaim.virtualSize(),
    claim_feerate_sat_vb: Number(claimFee) / signedClaim.virtualSize(),
    refund_fee_sats: input.refund_fee_sats,
    refund_vsize: signedRefund.virtualSize(),
    refund_feerate_sat_vb: Number(refundFee) / signedRefund.virtualSize(),
  },
  deltas: {
    source_lock_principal_sats: -input.amount_sats,
    source_lock_miner_fee_sats: -input.lock_fee_sats,
    source_change_sats: Number(change),
    coordinator_claim_receives_sats: Number(claimOutput),
    source_refund_receives_sats: Number(refundOutput),
  },
  validation: {
    source_key_rederived: true,
    coordinator_key_rederived: true,
    claim_path_signed_locally: true,
    refund_path_signed_locally: true,
    public_output_contains_no_signed_transaction: true,
    confirmed_funding_gate: true,
  },
}
writeFileSync(outputPath, `${JSON.stringify(result, null, 2)}\n`, {
  encoding: 'utf8',
  flag: 'wx',
  mode: 0o644,
})
process.stdout.write(
  `${JSON.stringify({
    result: 'PASS',
    p2wsh_address: result.p2wsh_address,
    amount_sats: result.amount_sats,
    claim_receives_sats: result.deltas.coordinator_claim_receives_sats,
  })}\n`,
)
