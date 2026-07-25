import { randomBytes, createHash } from 'node:crypto'
import {
  chmodSync,
  mkdirSync,
  readFileSync,
  writeFileSync,
} from 'node:fs'
import { spawnSync } from 'node:child_process'
import { dirname, join, resolve } from 'node:path'
import * as bitcoin from 'bitcoinjs-lib'
import * as ecc from 'tiny-secp256k1'
import { ECPairFactory } from 'ecpair'

bitcoin.initEccLib(ecc)
const ECPair = ECPairFactory(ecc)
const NETWORK = bitcoin.networks.regtest
const DEFAULT_CORE =
  '/home/postfiat/tmp/bitcoin-core-31.0-download/bitcoin-31.0/bin/bitcoin-cli'
const DEFAULT_DATADIR =
  '/home/postfiat/tmp/pftl-btc-navcoin-regtest-v2-20260725/bitcoin'
const LOCK_FEE_SATS = 500n
const SPEND_FEE_SATS = 500n
const SPLIT_FEE_SATS = 1000n

function assertHex(value, bytes, label) {
  if (
    typeof value !== 'string' ||
    value.length !== bytes * 2 ||
    !/^[0-9a-f]+$/.test(value)
  ) {
    throw new Error(`${label} must be canonical lowercase ${bytes}-byte hex`)
  }
}

function assertUint(value, label) {
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new Error(`${label} must be a non-negative safe integer`)
  }
}

function sha256(bytes) {
  return createHash('sha256').update(bytes).digest()
}

function writeJson(path, value, mode = 0o644) {
  mkdirSync(dirname(path), { recursive: true })
  writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`, { mode })
  chmodSync(path, mode)
}

function readJson(path) {
  return JSON.parse(readFileSync(path, 'utf8'))
}

function walletPath(runtimeRoot, role) {
  if (!['user', 'coordinator'].includes(role)) {
    throw new Error('Bitcoin role must be user or coordinator')
  }
  return join(runtimeRoot, 'private', 'bitcoin', `${role}.wallet.json`)
}

function paymentForKey(keyPair) {
  const payment = bitcoin.payments.p2wpkh({
    pubkey: Buffer.from(keyPair.publicKey),
    network: NETWORK,
  })
  if (!payment.address || !payment.output) {
    throw new Error('failed to derive P2WPKH payment')
  }
  return payment
}

function loadWallet(runtimeRoot, role) {
  const record = readJson(walletPath(runtimeRoot, role))
  const keyPair = ECPair.fromWIF(record.wif, NETWORK)
  const payment = paymentForKey(keyPair)
  if (
    payment.address !== record.address ||
    Buffer.from(keyPair.publicKey).toString('hex') !== record.pubkey
  ) {
    throw new Error(`${role} wallet record does not match its WIF`)
  }
  return { keyPair, payment, record }
}

function varInt(value) {
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new Error('varint value is invalid')
  }
  if (value < 0xfd) return Buffer.from([value])
  if (value <= 0xffff) {
    const output = Buffer.alloc(3)
    output[0] = 0xfd
    output.writeUInt16LE(value, 1)
    return output
  }
  if (value <= 0xffffffff) {
    const output = Buffer.alloc(5)
    output[0] = 0xfe
    output.writeUInt32LE(value, 1)
    return output
  }
  throw new Error('witness item count exceeds uint32')
}

function serializeWitness(stack) {
  const parts = [varInt(stack.length)]
  for (const item of stack) {
    const bytes = Buffer.from(item)
    parts.push(varInt(bytes.length), bytes)
  }
  return Buffer.concat(parts)
}

export function htlcScript({
  digestHex,
  claimPubkeyHex,
  refundPubkeyHex,
  lockHeight,
}) {
  assertHex(digestHex, 32, 'payment hash')
  assertHex(claimPubkeyHex, 33, 'claim pubkey')
  assertHex(refundPubkeyHex, 33, 'refund pubkey')
  assertUint(lockHeight, 'lock height')
  if (lockHeight === 0 || lockHeight >= 500_000_000) {
    throw new Error('CLTV lock height must be a positive block height')
  }
  return bitcoin.script.compile([
    bitcoin.opcodes.OP_IF,
    bitcoin.opcodes.OP_SHA256,
    Buffer.from(digestHex, 'hex'),
    bitcoin.opcodes.OP_EQUALVERIFY,
    Buffer.from(claimPubkeyHex, 'hex'),
    bitcoin.opcodes.OP_CHECKSIG,
    bitcoin.opcodes.OP_ELSE,
    bitcoin.script.number.encode(lockHeight),
    bitcoin.opcodes.OP_CHECKLOCKTIMEVERIFY,
    bitcoin.opcodes.OP_DROP,
    Buffer.from(refundPubkeyHex, 'hex'),
    bitcoin.opcodes.OP_CHECKSIG,
    bitcoin.opcodes.OP_ENDIF,
  ])
}

function p2wshForScript(witnessScript) {
  const payment = bitcoin.payments.p2wsh({
    redeem: { output: witnessScript, network: NETWORK },
    network: NETWORK,
  })
  if (!payment.address || !payment.output) {
    throw new Error('failed to derive P2WSH payment')
  }
  return payment
}

function signP2wpkhInput(psbt, inputIndex, wallet, value) {
  psbt.updateInput(inputIndex, {
    witnessUtxo: {
      script: wallet.payment.output,
      value: BigInt(value),
    },
  })
  psbt.signInput(inputIndex, wallet.keyPair)
}

function finalizeHtlc(psbt, stackBuilder) {
  psbt.finalizeInput(0, (_index, input) => {
    if (!input.partialSig || input.partialSig.length !== 1) {
      throw new Error('HTLC input lacks exactly one partial signature')
    }
    return {
      finalScriptWitness: serializeWitness(
        stackBuilder(Buffer.from(input.partialSig[0].signature)),
      ),
    }
  })
  return psbt.extractTransaction()
}

export function initWallets(runtimeRootInput) {
  const runtimeRoot = resolve(runtimeRootInput)
  const publicAccounts = {}
  for (const role of ['user', 'coordinator']) {
    const path = walletPath(runtimeRoot, role)
    let record
    try {
      record = readJson(path)
    } catch (error) {
      if (error.code !== 'ENOENT') throw error
      const keyPair = ECPair.makeRandom({ network: NETWORK, rng: randomBytes })
      const payment = paymentForKey(keyPair)
      record = {
        schema: 'postfiat.bitcoin_regtest_wallet.private.v1',
        network: 'regtest',
        role,
        address: payment.address,
        pubkey: Buffer.from(keyPair.publicKey).toString('hex'),
        wif: keyPair.toWIF(),
      }
      writeJson(path, record, 0o600)
    }
    const loaded = loadWallet(runtimeRoot, role)
    publicAccounts[role] = {
      address: loaded.record.address,
      pubkey: loaded.record.pubkey,
    }
  }
  const output = {
    schema: 'postfiat.bitcoin_regtest_accounts.v1',
    network: 'regtest',
    accounts: publicAccounts,
  }
  writeJson(join(runtimeRoot, 'public', 'bitcoin-regtest-accounts.json'), output)
  return output
}

export function buildSplit(runtimeRootInput, fundingUtxo) {
  const runtimeRoot = resolve(runtimeRootInput)
  const user = loadWallet(runtimeRoot, 'user')
  const coordinator = loadWallet(runtimeRoot, 'coordinator')
  assertHex(fundingUtxo.txid, 32, 'funding txid')
  assertUint(fundingUtxo.vout, 'funding vout')
  assertUint(fundingUtxo.valueSats, 'funding value')
  const input = BigInt(fundingUtxo.valueSats)
  const allocation = 25_000n
  const allocated = allocation * 3n
  const change = input - allocated - SPLIT_FEE_SATS
  if (change < 546n) {
    throw new Error('funding UTXO is too small for three scenario allocations')
  }
  const psbt = new bitcoin.Psbt({ network: NETWORK })
  psbt.addInput({ hash: fundingUtxo.txid, index: fundingUtxo.vout })
  psbt.addOutput({ address: user.record.address, value: allocation })
  psbt.addOutput({ address: coordinator.record.address, value: allocation })
  psbt.addOutput({ address: user.record.address, value: allocation })
  psbt.addOutput({ address: user.record.address, value: change })
  signP2wpkhInput(psbt, 0, user, fundingUtxo.valueSats)
  psbt.finalizeAllInputs()
  const tx = psbt.extractTransaction()
  return {
    schema: 'postfiat.bitcoin_regtest_split.v1',
    network: 'regtest',
    raw_tx: tx.toHex(),
    txid: tx.getId(),
    input: fundingUtxo,
    outputs: [
      { role: 'user', scenario: 'btc_to_nav', vout: 0, value_sats: 25_000 },
      {
        role: 'coordinator',
        scenario: 'nav_to_btc',
        vout: 1,
        value_sats: 25_000,
      },
      { role: 'user', scenario: 'refund', vout: 2, value_sats: 25_000 },
      {
        role: 'user',
        scenario: 'change',
        vout: 3,
        value_sats: Number(change),
      },
    ],
    fee_sats: Number(SPLIT_FEE_SATS),
    conservation:
      fundingUtxo.valueSats === Number(allocated + change + SPLIT_FEE_SATS),
  }
}

export function buildLock(runtimeRootInput, request) {
  const runtimeRoot = resolve(runtimeRootInput)
  const owner = loadWallet(runtimeRoot, request.owner)
  const recipient = loadWallet(runtimeRoot, request.recipient)
  assertHex(request.inputTxid, 32, 'lock input txid')
  assertUint(request.inputVout, 'lock input vout')
  assertUint(request.inputValueSats, 'lock input value')
  assertUint(request.amountSats, 'lock amount')
  const input = BigInt(request.inputValueSats)
  const amount = BigInt(request.amountSats)
  const change = input - amount - LOCK_FEE_SATS
  if (amount < 546n || change < 546n) {
    throw new Error('lock amount or change would be dust')
  }
  const witnessScript = htlcScript({
    digestHex: request.digestHex,
    claimPubkeyHex: recipient.record.pubkey,
    refundPubkeyHex: owner.record.pubkey,
    lockHeight: request.lockHeight,
  })
  const htlc = p2wshForScript(witnessScript)
  const psbt = new bitcoin.Psbt({ network: NETWORK })
  psbt.addInput({ hash: request.inputTxid, index: request.inputVout })
  psbt.addOutput({ address: htlc.address, value: amount })
  psbt.addOutput({ address: owner.record.address, value: change })
  signP2wpkhInput(psbt, 0, owner, request.inputValueSats)
  psbt.finalizeAllInputs()
  const tx = psbt.extractTransaction()
  return {
    schema: 'postfiat.bitcoin_regtest_p2wsh_htlc.v1',
    network: 'regtest',
    scenario: request.scenario,
    owner: request.owner,
    recipient: request.recipient,
    owner_address: owner.record.address,
    recipient_address: recipient.record.address,
    payment_hash: request.digestHex,
    lock_height: request.lockHeight,
    p2wsh_address: htlc.address,
    witness_script_hex: witnessScript.toString('hex'),
    raw_tx: tx.toHex(),
    txid: tx.getId(),
    vout: 0,
    amount_sats: request.amountSats,
    input_value_sats: request.inputValueSats,
    change_sats: Number(change),
    fee_sats: Number(LOCK_FEE_SATS),
    conservation:
      request.inputValueSats ===
      request.amountSats + Number(change) + Number(LOCK_FEE_SATS),
  }
}

function loadLock(runtimeRoot, lock) {
  if (lock.network !== 'regtest') throw new Error('HTLC is not on regtest')
  const owner = loadWallet(runtimeRoot, lock.owner)
  const recipient = loadWallet(runtimeRoot, lock.recipient)
  const witnessScript = htlcScript({
    digestHex: lock.payment_hash,
    claimPubkeyHex: recipient.record.pubkey,
    refundPubkeyHex: owner.record.pubkey,
    lockHeight: lock.lock_height,
  })
  if (witnessScript.toString('hex') !== lock.witness_script_hex) {
    throw new Error('HTLC witness script does not match lock record')
  }
  const htlc = p2wshForScript(witnessScript)
  if (htlc.address !== lock.p2wsh_address) {
    throw new Error('HTLC P2WSH address does not match lock record')
  }
  return { owner, recipient, witnessScript, htlc }
}

export function buildClaim(runtimeRootInput, lock, preimageHex) {
  const runtimeRoot = resolve(runtimeRootInput)
  assertHex(preimageHex, 32, 'preimage')
  const loaded = loadLock(runtimeRoot, lock)
  const outputValue = BigInt(lock.amount_sats) - SPEND_FEE_SATS
  if (outputValue < 546n) throw new Error('claim output would be dust')
  const psbt = new bitcoin.Psbt({ network: NETWORK })
  psbt.addInput({
    hash: lock.txid,
    index: lock.vout,
    witnessUtxo: {
      script: loaded.htlc.output,
      value: BigInt(lock.amount_sats),
    },
    witnessScript: loaded.witnessScript,
  })
  psbt.addOutput({
    address: loaded.recipient.record.address,
    value: outputValue,
  })
  psbt.signInput(0, loaded.recipient.keyPair)
  const tx = finalizeHtlc(psbt, (signature) => [
    signature,
    Buffer.from(preimageHex, 'hex'),
    Buffer.from([1]),
    loaded.witnessScript,
  ])
  return {
    schema: 'postfiat.bitcoin_regtest_htlc_claim.v1',
    network: 'regtest',
    lock_txid: lock.txid,
    raw_tx: tx.toHex(),
    txid: tx.getId(),
    wtxid: tx.getHash(true).reverse().toString('hex'),
    output_value_sats: Number(outputValue),
    fee_sats: Number(SPEND_FEE_SATS),
    public_preimage_hex: preimageHex,
    conservation:
      lock.amount_sats === Number(outputValue + SPEND_FEE_SATS),
  }
}

export function buildRefund(runtimeRootInput, lock) {
  const runtimeRoot = resolve(runtimeRootInput)
  const loaded = loadLock(runtimeRoot, lock)
  const outputValue = BigInt(lock.amount_sats) - SPEND_FEE_SATS
  if (outputValue < 546n) throw new Error('refund output would be dust')
  const psbt = new bitcoin.Psbt({ network: NETWORK })
  psbt.setLocktime(lock.lock_height)
  psbt.addInput({
    hash: lock.txid,
    index: lock.vout,
    sequence: 0xfffffffe,
    witnessUtxo: {
      script: loaded.htlc.output,
      value: BigInt(lock.amount_sats),
    },
    witnessScript: loaded.witnessScript,
  })
  psbt.addOutput({ address: loaded.owner.record.address, value: outputValue })
  psbt.signInput(0, loaded.owner.keyPair)
  const tx = finalizeHtlc(psbt, (signature) => [
    signature,
    Buffer.alloc(0),
    loaded.witnessScript,
  ])
  return {
    schema: 'postfiat.bitcoin_regtest_htlc_refund.v1',
    network: 'regtest',
    lock_txid: lock.txid,
    lock_height: lock.lock_height,
    raw_tx: tx.toHex(),
    txid: tx.getId(),
    wtxid: tx.getHash(true).reverse().toString('hex'),
    output_value_sats: Number(outputValue),
    fee_sats: Number(SPEND_FEE_SATS),
    conservation:
      lock.amount_sats === Number(outputValue + SPEND_FEE_SATS),
  }
}

export function extractClaimPreimage(rawTx, expectedDigestHex) {
  assertHex(expectedDigestHex, 32, 'expected payment hash')
  const tx = bitcoin.Transaction.fromHex(rawTx)
  if (tx.ins.length !== 1 || tx.ins[0].witness.length !== 4) {
    throw new Error('transaction is not a canonical demo HTLC claim')
  }
  const preimage = Buffer.from(tx.ins[0].witness[1])
  if (preimage.length !== 32) {
    throw new Error('claim witness preimage is not 32 bytes')
  }
  if (!sha256(preimage).equals(Buffer.from(expectedDigestHex, 'hex'))) {
    throw new Error('claim witness preimage does not satisfy payment hash')
  }
  return preimage.toString('hex')
}

function coreCommand(args) {
  const binary = process.env.BITCOIN_CLI || DEFAULT_CORE
  const datadir = process.env.BITCOIN_DATADIR || DEFAULT_DATADIR
  const completed = spawnSync(
    binary,
    ['-regtest', `-datadir=${datadir}`, ...args],
    {
      encoding: 'utf8',
      maxBuffer: 4 * 1024 * 1024,
    },
  )
  if (completed.error) throw completed.error
  if (completed.status !== 0) {
    throw new Error(
      `bitcoin-cli ${args[0]} failed: ${completed.stderr.trim()}`,
    )
  }
  const output = completed.stdout.trim()
  try {
    return JSON.parse(output)
  } catch {
    return output
  }
}

export function coreStatus() {
  const chain = coreCommand(['getblockchaininfo'])
  if (chain.chain !== 'regtest') throw new Error('Bitcoin Core is not on regtest')
  return chain
}

export function testMempool(rawTx) {
  const result = coreCommand(['testmempoolaccept', JSON.stringify([rawTx])])
  if (!Array.isArray(result) || result.length !== 1) {
    throw new Error('testmempoolaccept returned an invalid result')
  }
  return result[0]
}

export function broadcast(rawTx) {
  const preflight = testMempool(rawTx)
  if (!preflight.allowed) {
    throw new Error(
      `regtest transaction rejected: ${preflight['reject-reason'] || 'unknown'}`,
    )
  }
  const txid = coreCommand(['sendrawtransaction', rawTx])
  const parsed = bitcoin.Transaction.fromHex(rawTx)
  if (txid !== parsed.getId()) {
    throw new Error('Bitcoin Core returned an unexpected txid')
  }
  return { txid, preflight }
}

export function coreTx(txid) {
  assertHex(txid, 32, 'txid')
  return coreCommand(['getrawtransaction', txid, 'true'])
}
