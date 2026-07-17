// Transaction builder - orchestrates quote -> sign -> submit -> poll.

import { rpcErrorMessage } from './rpc-client.js';

export const PAYMENT_MEMO_LIMITS = {
  memo_type: 64,
  memo_format: 64,
  memo_data: 256,
  total: 512,
};

const MEMO_KEYS = ['memo_type', 'memo_format', 'memo_data'];
const MEMO_LABELS = {
  memo_type: 'Memo Type',
  memo_format: 'Memo Format',
  memo_data: 'Memo Data',
};

async function getDefaultWasm() {
  const { getWasm } = await import('./wasm-loader.js');
  return getWasm();
}

function memoInputValue(memos, key) {
  const value = memos?.[key];
  if (value === undefined || value === null) return '';
  if (typeof value !== 'string') throw new Error(`${MEMO_LABELS[key]} must be a string`);
  return value;
}

function utf8Bytes(value) {
  return new TextEncoder().encode(value);
}

function bytesToHex(bytes) {
  return Array.from(bytes, b => b.toString(16).padStart(2, '0')).join('');
}

function strictHexToBytes(value, expectedBytes, label) {
  const hex = String(value || '').toLowerCase();
  if (hex.length !== expectedBytes * 2 || !/^[0-9a-f]+$/.test(hex)) {
    throw new Error(`${label} must be ${expectedBytes} bytes of lowercase hex`);
  }
  return Array.from({ length: expectedBytes }, (_, index) => Number.parseInt(hex.slice(index * 2, index * 2 + 2), 16));
}

const INTEGER_OPERATION_FIELDS = new Set([
  'amount',
  'cancel_after',
  'epoch',
  'fee',
  'finish_after',
  'limit',
  'mint_amount',
  'mint_amount_atoms',
  'nav_epoch',
  'redeem_amount',
  'redeem_amount_atoms',
  'sequence',
  'settlement_amount',
  'settlement_amount_atoms',
]);

function stableOperationJson(value, key = null) {
  if (INTEGER_OPERATION_FIELDS.has(key)) {
    if (typeof value === 'number' && Number.isSafeInteger(value) && value >= 0) {
      return JSON.stringify(String(value));
    }
    if (typeof value === 'bigint' && value >= 0n) {
      return JSON.stringify(value.toString());
    }
    if (typeof value === 'string' && /^(0|[1-9][0-9]*)$/.test(value)) {
      return JSON.stringify(BigInt(value).toString());
    }
  }
  if (value === null || typeof value !== 'object') return JSON.stringify(value);
  if (Array.isArray(value)) return `[${value.map(item => stableOperationJson(item, null)).join(',')}]`;
  return `{${Object.keys(value).sort().map(childKey => `${JSON.stringify(childKey)}:${stableOperationJson(value[childKey], childKey)}`).join(',')}}`;
}

function receiptFromFinalityResult(txId, result) {
  const finality = result?.finality;
  if (finality?.receipt) {
    return finality.receipt;
  }

  const hotReceipts = finality?.local_hot_finality || result?.local_hot_finality || [];
  const match = hotReceipts.find(r => r.receipt?.tx_id === txId);
  if (match?.receipt) {
    return match.receipt;
  }

  if (finality?.confirmed && finality?.tx_id === txId) {
    return { accepted: true, confirmed: true, tx_id: txId };
  }

  if (finality?.block?.header?.certificate || result?.round_ok === true) {
    return { accepted: true, certified: true, tx_id: txId };
  }

  return null;
}

export function hasMemoFields(memos = {}) {
  return MEMO_KEYS.some(key => {
    const value = memos?.[key];
    return value !== undefined && value !== null && value !== '';
  });
}

export function encodePaymentMemoFields(memos = {}) {
  const encoded = {
    memo_type: '',
    memo_format: '',
    memo_data: '',
  };

  let totalBytes = 0;
  for (const key of MEMO_KEYS) {
    const value = memoInputValue(memos, key);
    const bytes = utf8Bytes(value);
    if (bytes.length > PAYMENT_MEMO_LIMITS[key]) {
      throw new Error(`${MEMO_LABELS[key]} must be ${PAYMENT_MEMO_LIMITS[key]} bytes or less`);
    }
    totalBytes += bytes.length;
    encoded[key] = bytesToHex(bytes);
  }

  if (totalBytes > PAYMENT_MEMO_LIMITS.total) {
    throw new Error(`Memo fields must be ${PAYMENT_MEMO_LIMITS.total} bytes or less in total`);
  }

  return encoded;
}

export class FastPayRecoveryPendingError extends Error {
  constructor(message, recovery) {
    super(message);
    this.name = 'FastPayRecoveryPendingError';
    this.code = 'fastpay_recovery_pending';
    this.recovery = recovery;
  }
}

export class TxBuilder {
  constructor(rpcClient, wasmProvider = getDefaultWasm) {
    this.rpc = rpcClient;
    this.getWasm = wasmProvider;
  }

  async signOwnedTransferOrder(wasm, backupJson, orderJson) {
    return wasm.wallet_sign_owned_transfer(backupJson, orderJson);
  }

  async signOwnedUnwrapOrder(wasm, backupJson, orderJson) {
    return wasm.wallet_sign_owned_unwrap(backupJson, orderJson);
  }

  async fastPayRecoveryCapabilities() {
    if (typeof this.rpc.ownedRecoveryCapabilities !== 'function') return null;
    const response = await this.rpc.ownedRecoveryCapabilities();
    if (!response?.ok || !response.result) {
      throw new Error(`FastPay recovery capability is unavailable: ${rpcErrorMessage(response)}`);
    }
    return response.result;
  }

  fastPayRecoveryWindow(capabilities) {
    const current = Number(capabilities?.current_height);
    const validity = Number(capabilities?.policy?.max_validity_blocks);
    const recovery = Number(capabilities?.policy?.max_recovery_blocks);
    const committeeEpoch = Number(capabilities?.committee_epoch);
    const expires = current + validity;
    const closes = expires + recovery;
    if (
      capabilities?.schema !== 'postfiat-fastpay-recovery-capabilities-v1'
      || capabilities?.domain?.schema !== 'postfiat-owned-certificate-domain-v3'
      || capabilities?.policy?.schema !== 'postfiat-fastpay-recovery-policy-v1'
      || ![current, validity, recovery, committeeEpoch, expires, closes].every(Number.isSafeInteger)
      || current <= 0
      || validity <= 0
      || recovery <= 0
      || committeeEpoch <= 0
      || current < Number(capabilities.policy.activation_height)
    ) {
      throw new Error('FastPay recovery capability is invalid or inactive');
    }
    return {
      schema: 'postfiat-fastpay-order-recovery-v1',
      committee_epoch: committeeEpoch,
      lock_id: '0'.repeat(96),
      valid_from_height: current,
      expires_at_height: expires,
      recovery_closes_at_height: closes,
    };
  }

  fastPayValidatorContext(validators, capabilities) {
    if (!Array.isArray(validators)) throw new Error('No FastPay validators available');
    const byId = new Map();
    for (const validator of validators) {
      const validatorId = validator?.node_id || validator?.validator_id || validator?.id;
      const publicKeyHex = validator?.public_key_hex;
      if (!validatorId || !publicKeyHex || byId.has(validatorId)) {
        throw new Error('FastPay validator roster is incomplete or non-distinct');
      }
      byId.set(validatorId, publicKeyHex);
    }
    if (
      byId.size !== Number(capabilities?.validator_count)
      || !Number.isSafeInteger(Number(capabilities?.quorum))
      || Number(capabilities.quorum) <= 0
      || Number(capabilities.quorum) > byId.size
    ) {
      throw new Error('FastPay validator roster does not match the governed recovery capability');
    }
    return { byId, quorum: Number(capabilities.quorum) };
  }

  async collectFastPayV3Votes(signedOrder, validators, quorum, unwrap = false) {
    const signedOrderJson = JSON.stringify(signedOrder);
    const pending = new Set(validators.map((validator) => {
      const validatorId = validator.node_id || validator.validator_id || validator.id;
      const request = (unwrap ? this.rpc.ownedUnwrapSignV3 : this.rpc.ownedSignV3)
        .call(this.rpc, signedOrderJson, validatorId)
        .then(resp => ({ validatorId, resp }))
        .catch(error => ({ validatorId, error }));
      return { validatorId, request };
    }));
    const votes = [];
    const failures = [];
    const seen = new Set();
    while (pending.size > 0 && votes.length < quorum) {
      const { item, outcome } = await Promise.race(
        [...pending].map(item => item.request.then(outcome => ({ item, outcome }))),
      );
      pending.delete(item);
      const vote = outcome.resp?.result;
      if (
        !outcome.error
        && outcome.resp?.ok
        && vote?.validator_id === outcome.validatorId
        && typeof vote.signature_hex === 'string'
        && vote.signature_hex.length > 0
        && !seen.has(vote.validator_id)
      ) {
        seen.add(vote.validator_id);
        votes.push({ validator_id: vote.validator_id, signature_hex: vote.signature_hex });
      } else {
        failures.push({
          validator_id: outcome.validatorId,
          message: outcome.error?.message || outcome.resp?.error?.message || 'invalid FastPay vote',
        });
      }
    }
    return { votes, failures };
  }

  verifyFastPayV3Apply(wasm, response, certificate, certificateDigest, capabilities, validatorContext) {
    const rows = Array.isArray(response?.result?.validators) ? response.result.validators : [];
    const accepted = [];
    const seen = new Set();
    let terminalStateDigest = null;
    let orderDigest = null;
    for (const row of rows) {
      const ack = row?.result;
      const validatorId = row?.validator_id;
      const publicKeyHex = validatorContext.byId.get(validatorId);
      if (!row?.ok || !ack || !publicKeyHex || seen.has(validatorId)) continue;
      if (
        ack.validator_id !== validatorId
        || ack.schema !== 'postfiat-fastpay-apply-ack-v1'
        || JSON.stringify(ack.domain) !== JSON.stringify(capabilities.domain)
        || Number(ack.committee_epoch) !== Number(capabilities.committee_epoch)
        || ack.lock_id !== certificate.order.recovery.lock_id
        || ack.certificate_digest !== certificateDigest
        || (terminalStateDigest !== null && ack.terminal_state_digest !== terminalStateDigest)
        || (orderDigest !== null && ack.order_digest !== orderDigest)
      ) continue;
      try {
        if (!wasm.wallet_verify_fastpay_apply_ack(JSON.stringify(ack), publicKeyHex)) continue;
      } catch (_) {
        continue;
      }
      seen.add(validatorId);
      terminalStateDigest = ack.terminal_state_digest;
      orderDigest = ack.order_digest;
      accepted.push(ack);
    }
    if (!response?.ok || accepted.length < validatorContext.quorum) {
      throw new FastPayRecoveryPendingError(
        `FastPay apply has ${accepted.length}/${validatorContext.quorum} authenticated durable acknowledgements`,
        {
          certificate,
          signed_order: this.fastPaySignedOrderFromCertificate(certificate),
          accepted_acknowledgements: accepted,
        },
      );
    }
    return accepted;
  }

  fastPaySignedOrderFromCertificate(certificate) {
    const operation = Object.hasOwn(certificate?.order || {}, 'to_address') ? 'unwrap' : 'transfer';
    return {
      operation,
      signed_order: {
        order: certificate.order,
        owner_pubkey_hex: certificate.owner_pubkey_hex,
        owner_signature_hex: certificate.owner_signature_hex,
      },
    };
  }

  async recoverFastPay(pending) {
    const certificate = pending?.certificate || null;
    const signedOrder = pending?.signed_order
      || (certificate ? this.fastPaySignedOrderFromCertificate(certificate) : null);
    const recovery = signedOrder?.signed_order?.order?.recovery;
    if (!signedOrder || !recovery?.lock_id) {
      throw new Error('FastPay recovery requires the locally retained signed order');
    }
    const [capabilities, statusResponse] = await Promise.all([
      this.fastPayRecoveryCapabilities(),
      this.rpc.ownedRecoveryStatus(recovery.lock_id),
    ]);
    const status = statusResponse?.ok ? statusResponse.result : null;
    if (status?.status === 'confirmed' || status?.status === 'cancelled') {
      return { status: status.status, recovery_status: status, receipt: null };
    }
    const currentHeight = Number(capabilities.current_height);
    if (currentHeight <= Number(recovery.expires_at_height)) {
      throw new FastPayRecoveryPendingError(
        `FastPay recovery opens after height ${recovery.expires_at_height}`,
        pending,
      );
    }
    if (certificate && currentHeight < Number(recovery.recovery_closes_at_height)) {
      const reveal = {
        operation: {
          kind: 'fast_pay_recovery_reveal',
          certificate: {
            operation: signedOrder.operation,
            certificate,
          },
        },
      };
      const submit = await this.rpc.submitFastlanePrimary(JSON.stringify(reveal));
      if (!submit?.ok || !submit.result?.tx_id) {
        throw new FastPayRecoveryPendingError(
          `FastPay recovery reveal submit failed: ${rpcErrorMessage(submit)}`,
          pending,
        );
      }
      const receipt = await this.pollReceipt(submit.result.tx_id, 30000);
      if (!receipt?.accepted || receipt.code !== 'fastpay_recovery_certificate_revealed') {
        throw new FastPayRecoveryPendingError(
          `FastPay recovery reveal was not accepted: ${receipt?.code || 'unknown'}`,
          pending,
        );
      }
      return {
        status: 'certificate_revealed',
        recovery_status: status,
        receipt,
        next_action_height: recovery.recovery_closes_at_height,
      };
    }
    const submittedAtHeight = currentHeight + 1;
    if (!Number.isSafeInteger(submittedAtHeight)) {
      throw new Error('FastPay recovery decision height overflow');
    }
    const decision = {
      operation: {
        kind: 'fast_pay_recovery_decision',
        request: {
          schema: 'postfiat-fastpay-recovery-decision-request-v1',
          submitted_at_height: submittedAtHeight,
          signed_order: signedOrder,
        },
      },
    };
    const submit = await this.rpc.submitFastlanePrimary(JSON.stringify(decision));
    if (!submit?.ok || !submit.result?.tx_id) {
      throw new FastPayRecoveryPendingError(
        `FastPay recovery decision submit failed: ${rpcErrorMessage(submit)}`,
        pending,
      );
    }
    const receipt = await this.pollReceipt(submit.result.tx_id, 30000);
    if (
      !receipt?.accepted
      || !['fastpay_recovery_confirmed', 'fastpay_recovery_cancelled'].includes(receipt.code)
    ) {
      throw new FastPayRecoveryPendingError(
        `FastPay recovery decision was not accepted: ${receipt?.code || 'unknown'}`,
        pending,
      );
    }
    return {
      status: receipt.code === 'fastpay_recovery_confirmed' ? 'confirmed_by_recovery' : 'cancelled',
      recovery_status: status,
      receipt,
    };
  }

  async sendTransfer(backupJson, fromAddress, toAddress, amount, memos = undefined, reviewedQuote = null) {
    const memoFields = encodePaymentMemoFields(memos);
    if (hasMemoFields(memoFields)) {
      return this.sendPaymentV2(backupJson, fromAddress, toAddress, amount, memoFields, reviewedQuote);
    }

    // 1. Get fee quote
    const quote = reviewedQuote || await this.quoteTransfer(fromAddress, toAddress, amount);

    // 2. Validate
    if (quote.sender_meets_reserve_after_transfer === false) {
      throw new Error('Insufficient balance after transfer. Balance after: ' + quote.sender_balance_after_amount_and_fee);
    }

    // 3. Sign with WASM
    const wasm = await this.getWasm();
    const signed = wasm.wallet_sign_transfer(backupJson, JSON.stringify(quote));

    // 4. Submit through the peer-certified finality RPC. The demo wallet must
    // not silently downgrade to a mempool-only submit.
    const signedJson = JSON.stringify(signed);
    const submitResp = await this.rpc.submitSignedTransferFinality(signedJson);
    if (!submitResp.ok) throw new Error('Finality submit failed: ' + (submitResp.error?.message || 'unknown'));
    const txId = submitResp.result.tx_id;

    const finalityReceipt = receiptFromFinalityResult(txId, submitResp.result);
    if (finalityReceipt) {
      return { txId, receipt: finalityReceipt, quote, signed, finality: submitResp.result.finality };
    }

    // 5. Poll for receipt
    const receipt = await this.pollReceipt(txId, 30000);

    return { txId, receipt, quote, signed };
  }

  /**
   * Publish the wallet's public key to the ledger so other wallets can resolve
   * it for FastPay recipient verification.
   *
   * The L1 execution engine (crates/execution/src/lib_parts/entrypoints.rs)
   * records `public_key_hex` onto the sender's Account on the first Account-lane
   * transfer or payment v2 the sender submits. Wrapping to FastPay
   * (`wrap_owned`) and receiving funds do NOT publish it, so a wallet that has
   * only wrapped/received has `account.public_key_hex === null` and cannot be
   * addressed by FastPay senders.
   *
   * This signs a minimal self-transfer of 1 atom (the smallest non-zero amount)
   * through the standard Account-lane finality path. The execution engine
   * records the signed transaction's `public_key_hex` onto the sender Account.
   * The 1 atom returns to the sender; the only net cost is the transfer fee
   * (`minimum_fee`, typically 1 atom = 0.000001 PFT).
   *
   * @param {string} backupJson - WalletBackupFile JSON
   * @param {string} fromAddress - the wallet's own pf address (also the recipient)
   * @returns {Promise<{txId, receipt, quote, signed}>} the transfer result
   */
  async publishPublicKey(backupJson, fromAddress) {
    if (!backupJson) throw new Error('Wallet not unlocked');
    if (!fromAddress) throw new Error('Wallet address is missing');
    // 1 atom is the smallest representable amount (>0). A self-transfer at
    // 1 atom costs only the fee and returns the atom to the sender.
    const PUBLISH_AMOUNT = 1;
    const result = await this.sendTransfer(backupJson, fromAddress, fromAddress, PUBLISH_AMOUNT);
    if (result.receipt?.accepted !== true || result.receipt?.code !== 'accepted') {
      throw new Error(
        `Public-key activation requires an explicit accepted receipt code; received ${result.receipt?.code || 'none'}`,
      );
    }
    return result;
  }

  /**
   * Ensure the wallet's public key is published on the ledger before a
   * FastPay-related action (wrap/unwrap/owned-transfer). Wrapping and
   * receiving do NOT publish the account public key (only an Account-lane
   * transfer/payment does — see entrypoints.rs:341/589), so a wallet that has
   * only ever wrapped would be invisible to FastPay senders. This makes
   * "I wrapped, why can't anyone FastPay me?" impossible by chaining a minimal
   * 1-atom self-transfer before the wrap whenever the key is not yet on-chain.
   *
   * Idempotent: when `publishedPublicKey` is truthy this is a no-op.
   *
   * @param {string} backupJson - WalletBackupFile JSON
   * @param {string} fromAddress - the wallet's own pf address
   * @param {boolean|string|null} publishedPublicKey - account.public_key_hex
   *        from the account RPC. falsy => not published yet => publish now.
   * @returns {Promise<{published: boolean, result?: object}>}
   */
  async ensurePublicKeyPublished(backupJson, fromAddress, publishedPublicKey) {
    if (publishedPublicKey) return { published: true };
    const result = await this.publishPublicKey(backupJson, fromAddress);
    return { published: true, result };
  }

  // Fund the FastPay owned-object lane through a source-signed,
  // sequence-bound transaction committed by normal consensus.
  async depositToFastPay(backupJson, sourceAddress, sourcePubkeyHex, amountPft) {
    if (!backupJson) throw new Error('Wallet not unlocked');
    const amountAtoms = Math.round(Number(amountPft) * 1_000_000);
    if (!Number.isSafeInteger(amountAtoms) || amountAtoms <= 0) {
      throw new Error('FastPay deposit amount must be a positive safe amount');
    }
    if (!globalThis.crypto?.getRandomValues) {
      throw new Error('Secure browser randomness is unavailable');
    }
    const [capabilities, accountResp] = await Promise.all([
      this.rpc.serverCapabilities(),
      this.rpc.account(sourceAddress),
    ]);
    if (!capabilities?.owned_lane_enabled || !capabilities?.mempool_submit_enabled) {
      throw new Error('Signed FastPay deposit is unavailable from this endpoint');
    }
    if (!capabilities.chain_id || !capabilities.genesis_hash || !capabilities.protocol_version) {
      throw new Error('FastPay deposit chain domain is unavailable');
    }
    if (!accountResp?.ok || !accountResp.result) {
      throw new Error('FastPay deposit account state is unavailable');
    }
    const account = accountResp.result.account || accountResp.result;
    const sequence = Number(account.sequence ?? 0) + 1;
    const feePft = 1;
    const balance = Number(account.balance ?? 0);
    if (!Number.isSafeInteger(sequence) || !Number.isSafeInteger(balance)) {
      throw new Error('FastPay deposit account values exceed browser-safe integers');
    }
    if (balance < amountAtoms + feePft) {
      throw new Error(`Insufficient Account balance for ${amountAtoms} atoms plus fee`);
    }
    const nonce = new Uint8Array(32);
    globalThis.crypto.getRandomValues(nonce);
    const deposit = {
      domain: {
        chain_id: capabilities.chain_id,
        genesis_hash: strictHexToBytes(capabilities.genesis_hash, 48, 'genesis hash'),
        protocol_version: capabilities.protocol_version,
      },
      source_address: sourceAddress,
      source_pubkey: strictHexToBytes(sourcePubkeyHex, 1952, 'wallet public key'),
      sequence,
      fee_pft: feePft,
      destination_owner_pubkey: strictHexToBytes(sourcePubkeyHex, 1952, 'FastPay owner public key'),
      asset: 'PFT',
      amount_atoms: amountAtoms,
      valid_through_height: Number(capabilities.block_height) + 100,
      nonce: Array.from(nonce),
    };
    const wasm = await this.getWasm();
    const transaction = wasm.wallet_sign_owned_deposit(backupJson, JSON.stringify(deposit));
    const submit = await this.rpc.submitFastlanePrimary(JSON.stringify(transaction));
    if (!submit?.ok || !submit.result?.tx_id) {
      throw new Error('Signed FastPay deposit submit failed: ' + rpcErrorMessage(submit));
    }
    const receipt = await this.pollReceipt(submit.result.tx_id, 30000);
    if (receipt?.accepted !== true || receipt?.code !== 'owned_deposit_applied') {
      throw new Error(`FastPay deposit requires accepted receipt code owned_deposit_applied; received ${receipt?.code || 'none'}`);
    }
    return { txId: submit.result.tx_id, receipt, deposit, transaction };
  }

  async quoteTransfer(fromAddress, toAddress, amount, memoFields = undefined) {
    const quoteResp = memoFields === undefined
      ? await this.rpc.transferFeeQuote(fromAddress, toAddress, amount)
      : await this.rpc.transferFeeQuote(fromAddress, toAddress, amount, memoFields);
    if (!quoteResp.ok) throw new Error('Fee quote failed: ' + (quoteResp.error?.message || 'unknown'));
    return quoteResp.result;
  }

  async sendPaymentV2(backupJson, fromAddress, toAddress, amount, memoFields, reviewedQuote = null) {
    const quote = reviewedQuote || await this.quoteTransfer(fromAddress, toAddress, amount, memoFields);

    if (quote.sender_meets_reserve_after_transfer === false) {
      throw new Error('Insufficient balance after transfer. Balance after: ' + quote.sender_balance_after_amount_and_fee);
    }

    const fields = {
      chain_id: quote.chain_id,
      genesis_hash: quote.genesis_hash,
      protocol_version: quote.protocol_version,
      to: toAddress,
      amount,
      fee: quote.minimum_fee,
      sequence: quote.sequence,
      memos: [memoFields],
    };

    const wasm = await this.getWasm();
    const signed = wasm.wallet_sign_payment_v2(backupJson, JSON.stringify(fields));
    const signedJson = JSON.stringify(signed);
    const submitResp = await this.rpc.submitSignedPaymentV2Finality(signedJson);
    if (!submitResp.ok) throw new Error('Finality submit failed: ' + (submitResp.error?.message || 'unknown'));
    const txId = submitResp.result.tx_id;

    const finalityReceipt = receiptFromFinalityResult(txId, submitResp.result);
    if (finalityReceipt) {
      return { txId, receipt: finalityReceipt, quote, signed, paymentV2: true, memos: memoFields, finality: submitResp.result.finality };
    }

    const receipt = await this.pollReceipt(txId, 30000);

    return { txId, receipt, quote, signed, paymentV2: true, memos: memoFields };
  }

  async sendAssetTransfer(backupJson, sourceAddress, fields) {
    const { quote, signed } = await this.signAssetTransaction(backupJson, sourceAddress, fields);

    // Submit
    const signedJson = JSON.stringify(signed);
    const submitAsset = typeof this.rpc.submitSignedAssetTransactionFinality === 'function'
      ? this.rpc.submitSignedAssetTransactionFinality.bind(this.rpc)
      : this.rpc.submitSignedAssetTransaction.bind(this.rpc);
    const submitResp = await submitAsset(signedJson);
    if (!submitResp.ok) throw new Error('Asset submit failed: ' + (submitResp.error?.message || 'unknown'));
    const txId = submitResp.result?.tx_id;

    const finalityReceipt = receiptFromFinalityResult(txId, submitResp.result);
    if (finalityReceipt) {
      return { txId, receipt: finalityReceipt, quote, signed, finality: submitResp.result.finality };
    }

    const receipt = await this.pollReceipt(txId, 30000);

    return { txId, receipt, quote, signed };
  }

  async signAssetTransaction(backupJson, sourceAddress, fields) {
    // For asset transactions we use wallet_sign_asset_transaction_fields.
    // fields: { chain_id, genesis_hash, protocol_version, source, fee, sequence, operation }
    const wasm = await this.getWasm();

    const operationJson = JSON.stringify(fields.operation);
    const quoteResp = await this.rpc.assetFeeQuote(sourceAddress, operationJson);
    if (!quoteResp.ok) throw new Error('Asset fee quote failed: ' + (quoteResp.error?.message || 'unknown'));
    const quote = quoteResp.result;

    if (quote.sender_meets_reserve_after_fee === false) {
      throw new Error('Insufficient balance for asset transaction fee.');
    }
    if (quote.source && quote.source !== sourceAddress) {
      throw new Error('Asset fee quote source does not match the wallet address.');
    }
    if (quote.operation && stableOperationJson(quote.operation) !== stableOperationJson(fields.operation)) {
      throw new Error('Asset fee quote operation does not match the reviewed action.');
    }

    const signFields = {
      chain_id: quote.chain_id,
      genesis_hash: quote.genesis_hash,
      protocol_version: quote.protocol_version,
      source: quote.source,
      fee: quote.minimum_fee,
      sequence: quote.sequence,
      operation: quote.operation || fields.operation,
    };

    const signed = wasm.wallet_sign_asset_transaction_fields(backupJson, JSON.stringify(signFields));
    return {
      quote,
      signed,
      signedJson: JSON.stringify(signed),
      signFields,
    };
  }

  async sendEscrowTransaction(backupJson, sourceAddress, fields, options = {}) {
    const wasm = await this.getWasm();
    const operationJson = JSON.stringify(fields.operation);
    const quoteResp = await this.rpc.escrowFeeQuote(sourceAddress, operationJson, options.sequence);
    if (!quoteResp.ok) throw new Error('Escrow fee quote failed: ' + (quoteResp.error?.message || 'unknown'));
    const quote = quoteResp.result;

    if (quote.sender_meets_reserve_after_fee === false) {
      throw new Error('Insufficient balance for escrow transaction fee.');
    }
    if (quote.source && quote.source !== sourceAddress) {
      throw new Error('Escrow fee quote source does not match the wallet address.');
    }
    if (options.sequence !== undefined && options.sequence !== null && quote.sequence !== options.sequence) {
      throw new Error('Escrow fee quote sequence does not match the reviewed template.');
    }
    if (quote.operation && stableOperationJson(quote.operation) !== stableOperationJson(fields.operation)) {
      throw new Error('Escrow fee quote operation does not match the reviewed template.');
    }

    const signFields = {
      chain_id: quote.chain_id,
      genesis_hash: quote.genesis_hash,
      protocol_version: quote.protocol_version,
      source: quote.source,
      fee: quote.minimum_fee,
      sequence: quote.sequence,
      operation: quote.operation || fields.operation,
    };

    const signed = wasm.wallet_sign_escrow_transaction_fields(backupJson, JSON.stringify(signFields));
    const signedJson = JSON.stringify(signed);
    const submitEscrow = typeof this.rpc.submitSignedEscrowTransactionFinality === 'function'
      ? this.rpc.submitSignedEscrowTransactionFinality.bind(this.rpc)
      : this.rpc.submitSignedEscrowTransaction.bind(this.rpc);
    const submitResp = await submitEscrow(signedJson);
    if (!submitResp.ok) throw new Error('Escrow submit failed: ' + (submitResp.error?.message || 'unknown'));
    const txId = submitResp.result?.tx_id;

    const finalityReceipt = receiptFromFinalityResult(txId, submitResp.result);
    if (finalityReceipt) {
      return { txId, receipt: finalityReceipt, quote, signed, finality: submitResp.result.finality };
    }

    const receipt = await this.pollReceipt(txId, 30000);
    return { txId, receipt, quote, signed };
  }

  async sendOfferTransaction(backupJson, sourceAddress, fields) {
    const wasm = await this.getWasm();

    // Get the offer fee quote
    const operationJson = JSON.stringify(fields.operation);
    const quoteResp = await this.rpc.offerFeeQuote(sourceAddress, operationJson);
    if (!quoteResp.ok) throw new Error('Offer fee quote failed: ' + (quoteResp.error?.message || 'unknown'));
    const quote = quoteResp.result;

    if (quote.sender_meets_reserve_after_fee === false) {
      throw new Error('Insufficient balance for offer transaction fee.');
    }

    // Build fields for signing
    const signFields = {
      chain_id: quote.chain_id,
      genesis_hash: quote.genesis_hash,
      protocol_version: quote.protocol_version,
      source: quote.source,
      fee: quote.minimum_fee,
      sequence: quote.sequence,
      operation: fields.operation,
    };

    const signed = wasm.wallet_sign_offer_transaction_fields(backupJson, JSON.stringify(signFields));

    // Submit
    const signedJson = JSON.stringify(signed);
    const submitResp = await this.rpc.submitSignedOfferTransaction(signedJson);
    if (!submitResp.ok) throw new Error('Offer submit failed: ' + (submitResp.error?.message || 'unknown'));
    const txId = submitResp.result?.tx_id;

    const receipt = await this.pollReceipt(txId, 30000);
    return { txId, receipt, quote, signed };
  }

  /**
   * FastPay owned-transfer: sign an OwnedTransferOrder with the wallet key,
   * collect validator votes via owned_sign RPC, assemble a certificate, and
   * apply it via owned_apply RPC.
   *
   * @param {string} backupJson - WalletBackupFile JSON
   * @param {string} ownerPubkeyHex - wallet public key hex
   * @param {Object} ownedObjects - array of owned objects owned by this wallet
   * @param {string} recipientPubkeyHex - recipient's public key hex
   * @param {number} amountPft - amount in PFT (human-readable, will be converted to atoms)
   * @param {number} feePft - fee in PFT
   * @param {Array} validators - array of { node_id, public_key_hex } from validators RPC
   */
  async sendOwnedTransfer(backupJson, ownerPubkeyHex, ownedObjects, recipientPubkeyHex, amountPft, feePft, validators) {
    const wasm = await this.getWasm();
    const recoveryCapabilities = await this.fastPayRecoveryCapabilities();
    if (recoveryCapabilities) {
      return this.sendOwnedTransferV3(
        wasm,
        backupJson,
        ownerPubkeyHex,
        ownedObjects,
        recipientPubkeyHex,
        amountPft,
        feePft,
        validators,
        recoveryCapabilities,
      );
    }
    const capabilities = await this.rpc.serverCapabilities();
    const ownedDomain = capabilities?.owned_certificate_domain;
    if (!capabilities?.owned_lane_enabled || !ownedDomain) {
      throw new Error('FastPay certificate domain is unavailable');
    }
    const amountAtoms = Math.round(amountPft * 1_000_000);
    const feeAtoms = Math.round(feePft * 1_000_000);
    const requiredAtoms = amountAtoms + feeAtoms;

    if (!ownedObjects || ownedObjects.length === 0) {
      throw new Error('No owned objects available for FastPay transfer');
    }
    if (!Array.isArray(validators) || validators.length === 0) {
      throw new Error('No FastPay validators available');
    }

    const candidates = ownedObjects.filter(o => Number(o?.value ?? 0) >= requiredAtoms);
    if (candidates.length === 0) {
      const largest = ownedObjects.reduce((max, obj) => Math.max(max, Number(obj?.value ?? 0)), 0);
      throw new Error(`Insufficient owned object balance. Need ${requiredAtoms} atoms, largest object has ${largest}`);
    }

    const validatorCount = Array.isArray(validators) ? validators.length : 0;
    const quorum = Math.floor((validatorCount * 2) / 3) + 1;
    const attempts = [];

    for (let attemptIndex = 0; attemptIndex < candidates.length; attemptIndex += 1) {
      const input = candidates[attemptIndex];
      const inputValue = Number(input.value);
      const change = inputValue - requiredAtoms;
      const nonce = (Date.now() * 1000) + attemptIndex;

      // Build the order: 1 input, 1 output to recipient (+ optional change output)
      const outputs = [
        { owner_pubkey_hex: recipientPubkeyHex, value: amountAtoms, asset: input.asset },
      ];
      if (change > 0) {
        outputs.push({ owner_pubkey_hex: ownerPubkeyHex, value: change, asset: input.asset });
      }

      const order = {
        domain: ownedDomain,
        inputs: [{ id: input.id, version: input.version }],
        outputs,
        fee: feeAtoms,
        nonce,
        memos: [],
      };

      // 1. Sign the order with the wallet owner's key (WASM)
      const orderJson = JSON.stringify(order);
      const signResult = await this.signOwnedTransferOrder(wasm, backupJson, orderJson);
      const ownerSignatureHex = signResult.owner_signature_hex;
      const signedOwnerPubkeyHex = signResult.owner_pubkey_hex;
      const signedOrderEnvelopeJson = JSON.stringify({
        order,
        owner_pubkey_hex: signedOwnerPubkeyHex,
        owner_signature_hex: ownerSignatureHex,
      });

      // 2. Collect validator votes via owned_sign RPC. Request votes in
      // parallel and stop at quorum so one slow validator does not stall a
      // complete FastPay certificate.
      const votes = [];
      const voteFailures = [];
      const voteRequests = validators
        .map(validator => {
          const validatorId = validator.node_id || validator.validator_id || validator.id;
          if (!validatorId) return null;
          const promise = this.rpc.ownedSign(signedOrderEnvelopeJson, validatorId)
            .then(resp => ({ validatorId, resp }))
            .catch(error => ({ validatorId, error }));
          return { validatorId, promise };
        })
        .filter(Boolean);
      const pendingVotes = new Set(voteRequests);

      while (pendingVotes.size > 0 && votes.length < quorum) {
        const { request, result } = await Promise.race(
          [...pendingVotes].map(request => request.promise.then(result => ({ request, result }))),
        );
        pendingVotes.delete(request);
        if (result.error) {
          const message = result.error.message || String(result.error);
          voteFailures.push({ validator_id: result.validatorId, message });
          console.warn(`owned_sign failed for ${result.validatorId}:`, message);
          continue;
        }
        const voteResp = result.resp;
        if (voteResp?.ok && voteResp.result) {
          votes.push({
            validator_id: voteResp.result.validator_id,
            signature_hex: voteResp.result.signature_hex,
          });
        } else {
          const message = voteResp?.error?.message || voteResp?.error?.code || 'owned_sign refused';
          voteFailures.push({ validator_id: result.validatorId, message });
          console.warn(`owned_sign refused for ${result.validatorId}:`, message);
        }
      }

      const attempt = {
        input_id: input.id,
        votes: votes.length,
        failures: voteFailures,
      };
      attempts.push(attempt);

      if (votes.length < quorum) {
        continue;
      }

      // 3. Assemble the certificate
      const cert = {
        order,
        owner_pubkey_hex: signedOwnerPubkeyHex,
        owner_signature_hex: ownerSignatureHex,
        votes,
      };
      const certJson = JSON.stringify(cert);

      // 4. Apply the certificate via owned_apply RPC. A quorum-voted
      // certificate that fails apply is not treated as a stale-input retry,
      // because the object may already be locked by the collected votes.
      const applyResp = await this.rpc.ownedApply(certJson);
      if (!applyResp.ok) {
        const validatorDetails = Array.isArray(applyResp.error?.validators)
          ? ` (${applyResp.error.validators.slice(0, 3).map(v => `${v.validator_id || v.validatorId}: ${v.error?.message || v.error?.code || 'failed'}`).join('; ')})`
          : '';
        throw new Error('Owned apply failed: ' + (applyResp.error?.message || 'unknown') + validatorDetails);
      }

      return {
        cert,
        applyResult: applyResp.result,
        order,
        votes,
        input,
        attempts,
      };
    }

    const summarizeAttempt = attempt => {
      const id = String(attempt.input_id || '').slice(0, 12) || 'unknown-input';
      const details = attempt.failures
        .slice(0, 3)
        .map(failure => `${failure.validator_id}: ${failure.message}`)
        .join('; ');
      return details
        ? `${id} collected ${attempt.votes}/${quorum} votes (${details})`
        : `${id} collected ${attempt.votes}/${quorum} votes`;
    };

    if (attempts.length === 1) {
      const [attempt] = attempts;
      const details = attempt.failures.length > 0
        ? ` (${attempt.failures.slice(0, 3).map(failure => `${failure.validator_id}: ${failure.message}`).join('; ')})`
        : '';
      throw new Error(`FastPay collected ${attempt.votes} validator votes, need ${quorum}${details}`);
    }

    throw new Error(`FastPay could not collect ${quorum} validator votes from ${attempts.length} owned objects: ${attempts.map(summarizeAttempt).join(' | ')}`);
  }

  async sendOwnedTransferV3(
    wasm,
    backupJson,
    ownerPubkeyHex,
    ownedObjects,
    recipientPubkeyHex,
    amountPft,
    feePft,
    validators,
    capabilities,
  ) {
    const amountAtoms = Math.round(amountPft * 1_000_000);
    const feeAtoms = Math.round(feePft * 1_000_000);
    const requiredAtoms = amountAtoms + feeAtoms;
    if (
      ![amountAtoms, feeAtoms, requiredAtoms].every(Number.isSafeInteger)
      || amountAtoms <= 0
      || feeAtoms < 0
    ) {
      throw new Error('FastPay amount is outside the safe integer range');
    }
    const validatorContext = this.fastPayValidatorContext(validators, capabilities);
    const input = (ownedObjects || []).find(object => (
      Number.isSafeInteger(Number(object?.value))
      && Number(object.value) >= requiredAtoms
      && object?.id
      && object?.version !== undefined
    ));
    if (!input) throw new Error(`Insufficient owned object balance. Need ${requiredAtoms} atoms`);
    const outputs = [{
      owner_pubkey_hex: recipientPubkeyHex,
      value: amountAtoms,
      asset: input.asset,
    }];
    const change = Number(input.value) - requiredAtoms;
    if (change > 0) {
      outputs.push({ owner_pubkey_hex: ownerPubkeyHex, value: change, asset: input.asset });
    }
    const order = {
      domain: capabilities.domain,
      recovery: this.fastPayRecoveryWindow(capabilities),
      inputs: [{ id: input.id, version: input.version }],
      outputs,
      fee: feeAtoms,
      nonce: Date.now() * 1000,
      memos: [],
    };
    order.recovery.lock_id = wasm.wallet_fastpay_transfer_lock_id(JSON.stringify(order));
    const signedOrder = wasm.wallet_sign_owned_transfer_v3(
      backupJson,
      JSON.stringify(order),
      JSON.stringify(capabilities),
    );
    if (
      signedOrder?.owner_pubkey_hex !== ownerPubkeyHex
      || signedOrder?.order?.recovery?.lock_id !== order.recovery.lock_id
    ) {
      throw new Error('Wallet signer returned a different FastPay v3 owner or lock');
    }
    const { votes, failures } = await this.collectFastPayV3Votes(
      signedOrder,
      validators,
      validatorContext.quorum,
      false,
    );
    if (votes.length < validatorContext.quorum) {
      throw new FastPayRecoveryPendingError(
        `FastPay collected ${votes.length}/${validatorContext.quorum} votes; locked inputs require ordered recovery`,
        { signed_order: { operation: 'transfer', signed_order: signedOrder }, failures },
      );
    }
    const certificate = {
      order: signedOrder.order,
      owner_pubkey_hex: signedOrder.owner_pubkey_hex,
      owner_signature_hex: signedOrder.owner_signature_hex,
      votes,
    };
    const certificateJson = JSON.stringify(certificate);
    const certificateDigest = wasm.wallet_fastpay_transfer_certificate_digest(certificateJson);
    let applyResponse;
    try {
      applyResponse = await this.rpc.ownedApplyV3(certificateJson);
    } catch (error) {
      throw new FastPayRecoveryPendingError(
        `FastPay apply outcome is unknown: ${error?.message || error}`,
        {
          certificate,
          signed_order: this.fastPaySignedOrderFromCertificate(certificate),
          accepted_acknowledgements: [],
        },
      );
    }
    const acknowledgements = this.verifyFastPayV3Apply(
      wasm,
      applyResponse,
      certificate,
      certificateDigest,
      capabilities,
      validatorContext,
    );
    return {
      status: 'finalized',
      order: signedOrder.order,
      cert: certificate,
      votes,
      acknowledgements,
      applyResult: applyResponse.result,
      input,
    };
  }

  /**
   * Standard FastPay unwrap: sign an OwnedUnwrapOrder with the wallet key,
   * collect validator votes via owned_unwrap_sign RPC, assemble a certificate,
   * and apply it via owned_unwrap_apply RPC. The UI supplies an amount; object
   * selection and change are internal wallet details.
   */
  async unwrapOwnedTransfer(backupJson, ownerPubkeyHex, ownedObjects, toAddress, amountPft, feePft, validators) {
    const wasm = await this.getWasm();
    const recoveryCapabilities = await this.fastPayRecoveryCapabilities();
    if (recoveryCapabilities) {
      return this.unwrapOwnedTransferV3(
        wasm,
        backupJson,
        ownerPubkeyHex,
        ownedObjects,
        toAddress,
        amountPft,
        feePft,
        validators,
        recoveryCapabilities,
      );
    }
    const capabilities = await this.rpc.serverCapabilities();
    const ownedDomain = capabilities?.owned_certificate_domain;
    if (!capabilities?.owned_lane_enabled || !ownedDomain) {
      throw new Error('FastPay certificate domain is unavailable');
    }
    const amountAtoms = Math.round(amountPft * 1_000_000);
    const feeAtoms = Math.round(feePft * 1_000_000);
    const requiredAtoms = amountAtoms + feeAtoms;
    const maxInputs = 2048;

    if (!ownedObjects || ownedObjects.length === 0) {
      throw new Error('No owned objects available for FastPay unwrap');
    }
    if (!Array.isArray(validators) || validators.length === 0) {
      throw new Error('No FastPay validators available');
    }
    if (!toAddress) {
      throw new Error('Account address is required for FastPay unwrap');
    }
    if (amountAtoms <= 0) {
      throw new Error('Unwrap amount must be positive');
    }
    if (feeAtoms < 0) {
      throw new Error('Unwrap fee cannot be negative');
    }
    if (!Number.isSafeInteger(amountAtoms) || !Number.isSafeInteger(feeAtoms) || !Number.isSafeInteger(requiredAtoms)) {
      throw new Error('Unwrap amount is outside the safe integer range');
    }
    const requiredAtomsBig = BigInt(requiredAtoms);

    const usableObjects = ownedObjects
      .map((object, index) => {
        try {
          return { object, index, value: BigInt(object?.value ?? 0) };
        } catch (_) {
          return null;
        }
      })
      .filter(item => item && item.value > 0n && item.object?.id && item.object?.version !== undefined && (item.object?.asset || 'PFT') === 'PFT');
    const totalUsable = usableObjects.reduce((sum, item) => sum + item.value, 0n);
    if (totalUsable < requiredAtomsBig) {
      throw new Error(`Insufficient FastPay balance. Need ${requiredAtoms} atoms, available objects total ${totalUsable.toString()}`);
    }

    const singleCoverSets = usableObjects
      .filter(item => item.value >= requiredAtomsBig)
      .sort((a, b) => {
        if (a.value < b.value) return -1;
        if (a.value > b.value) return 1;
        return a.index - b.index;
      })
      .map(item => [item.object]);

    const largestFirst = [...usableObjects].sort((a, b) => {
      if (a.value > b.value) return -1;
      if (a.value < b.value) return 1;
      return a.index - b.index;
    });
    const multiCoverSets = [];
    for (let start = 0; start < largestFirst.length; start += 1) {
      const group = [];
      let total = 0n;
      for (let index = start; index < largestFirst.length && group.length < maxInputs; index += 1) {
        group.push(largestFirst[index].object);
        total += largestFirst[index].value;
        if (total >= requiredAtomsBig) {
          if (group.length > 1) multiCoverSets.push(group);
          break;
        }
      }
    }

    const seenInputSets = new Set();
    const candidates = [];
    for (const group of [...singleCoverSets, ...multiCoverSets]) {
      const key = group.map(object => `${object.id}:${object.version}`).sort().join('|');
      if (seenInputSets.has(key)) continue;
      seenInputSets.add(key);
      candidates.push(group);
    }
    if (candidates.length === 0) {
      throw new Error(`No combination of up to ${maxInputs} FastPay objects covers ${requiredAtoms} atoms`);
    }

    const validatorCount = validators.length;
    const quorum = Math.floor((validatorCount * 2) / 3) + 1;
    const attempts = [];

    for (let attemptIndex = 0; attemptIndex < candidates.length; attemptIndex += 1) {
      const inputs = candidates[attemptIndex];
      const nonce = (Date.now() * 1000) + attemptIndex;
      const order = {
        domain: ownedDomain,
        inputs: inputs.map(input => ({ id: input.id, version: input.version })),
        to_address: toAddress,
        amount: amountAtoms,
        asset: inputs[0]?.asset || 'PFT',
        fee: feeAtoms,
        nonce,
        memos: [],
      };

      const orderJson = JSON.stringify(order);
      const signResult = await this.signOwnedUnwrapOrder(wasm, backupJson, orderJson);
      const ownerSignatureHex = signResult.owner_signature_hex;
      const signedOwnerPubkeyHex = signResult.owner_pubkey_hex;
      if (
        ownerPubkeyHex
        && signedOwnerPubkeyHex
        && String(signedOwnerPubkeyHex).toLowerCase() !== String(ownerPubkeyHex).toLowerCase()
      ) {
        throw new Error('Wallet signer returned a different FastPay owner public key');
      }
      const signedOrderEnvelopeJson = JSON.stringify({
        order,
        owner_pubkey_hex: signedOwnerPubkeyHex,
        owner_signature_hex: ownerSignatureHex,
      });

      const votes = [];
      const voteFailures = [];
      const voteRequests = validators
        .map(validator => {
          const validatorId = validator.node_id || validator.validator_id || validator.id;
          if (!validatorId) return null;
          const promise = this.rpc.ownedUnwrapSign(signedOrderEnvelopeJson, validatorId)
            .then(resp => ({ validatorId, resp }))
            .catch(error => ({ validatorId, error }));
          return { validatorId, promise };
        })
        .filter(Boolean);
      const pendingVotes = new Set(voteRequests);

      while (pendingVotes.size > 0 && votes.length < quorum) {
        const { request, result } = await Promise.race(
          [...pendingVotes].map(request => request.promise.then(result => ({ request, result }))),
        );
        pendingVotes.delete(request);
        if (result.error) {
          const message = result.error.message || String(result.error);
          voteFailures.push({ validator_id: result.validatorId, message });
          console.warn(`owned_unwrap_sign failed for ${result.validatorId}:`, message);
          continue;
        }
        const voteResp = result.resp;
        if (voteResp?.ok && voteResp.result) {
          votes.push({
            validator_id: voteResp.result.validator_id,
            signature_hex: voteResp.result.signature_hex,
          });
        } else {
          const message = voteResp?.error?.message || voteResp?.error?.code || 'owned_unwrap_sign refused';
          voteFailures.push({ validator_id: result.validatorId, message });
          console.warn(`owned_unwrap_sign refused for ${result.validatorId}:`, message);
        }
      }

      const attempt = {
        input_id: inputs.map(input => input.id).join(','),
        input_ids: inputs.map(input => input.id),
        votes: votes.length,
        failures: voteFailures,
      };
      attempts.push(attempt);

      if (votes.length < quorum) {
        continue;
      }

      const cert = {
        order,
        owner_pubkey_hex: signedOwnerPubkeyHex,
        owner_signature_hex: ownerSignatureHex,
        votes,
      };
      const certJson = JSON.stringify(cert);
      const applyResp = await this.rpc.ownedUnwrapApply(certJson);
      if (!applyResp.ok) {
        const validatorDetails = Array.isArray(applyResp.error?.validators)
          ? ` (${applyResp.error.validators.slice(0, 3).map(v => `${v.validator_id || v.validatorId}: ${v.error?.message || v.error?.code || 'failed'}`).join('; ')})`
          : '';
        throw new Error('Owned unwrap apply failed: ' + (applyResp.error?.message || 'unknown') + validatorDetails);
      }

      return {
        cert,
        applyResult: applyResp.result,
        order,
        votes,
        input: inputs[0],
        inputs,
        attempts,
      };
    }

    const summarizeAttempt = attempt => {
      const id = String(attempt.input_id || '').slice(0, 12) || 'unknown-input';
      const details = attempt.failures
        .slice(0, 3)
        .map(failure => `${failure.validator_id}: ${failure.message}`)
        .join('; ');
      return details
        ? `${id} collected ${attempt.votes}/${quorum} votes (${details})`
        : `${id} collected ${attempt.votes}/${quorum} votes`;
    };

    if (attempts.length === 1) {
      const [attempt] = attempts;
      const details = attempt.failures.length > 0
        ? ` (${attempt.failures.slice(0, 3).map(failure => `${failure.validator_id}: ${failure.message}`).join('; ')})`
        : '';
      throw new Error(`FastPay unwrap collected ${attempt.votes} validator votes, need ${quorum}${details}`);
    }

    throw new Error(`FastPay unwrap could not collect ${quorum} validator votes from ${attempts.length} owned objects: ${attempts.map(summarizeAttempt).join(' | ')}`);
  }

  async unwrapOwnedTransferV3(
    wasm,
    backupJson,
    ownerPubkeyHex,
    ownedObjects,
    toAddress,
    amountPft,
    feePft,
    validators,
    capabilities,
  ) {
    const amountAtoms = Math.round(amountPft * 1_000_000);
    const feeAtoms = Math.round(feePft * 1_000_000);
    const requiredAtoms = amountAtoms + feeAtoms;
    if (
      !toAddress
      || ![amountAtoms, feeAtoms, requiredAtoms].every(Number.isSafeInteger)
      || amountAtoms <= 0
      || feeAtoms < 0
    ) {
      throw new Error('FastPay unwrap destination or amount is invalid');
    }
    const validatorContext = this.fastPayValidatorContext(validators, capabilities);
    const candidates = (ownedObjects || [])
      .filter(object => object?.id && object?.version !== undefined && (object.asset || 'PFT') === 'PFT')
      .map(object => ({ object, value: BigInt(object.value) }))
      .filter(candidate => candidate.value > 0n)
      .sort((left, right) => (left.value > right.value ? -1 : left.value < right.value ? 1 : 0));
    const inputs = [];
    let total = 0n;
    for (const candidate of candidates) {
      inputs.push(candidate.object);
      total += candidate.value;
      if (total >= BigInt(requiredAtoms)) break;
      if (inputs.length >= 2048) break;
    }
    if (total < BigInt(requiredAtoms)) {
      throw new Error(`Insufficient FastPay balance. Need ${requiredAtoms} atoms, available ${total}`);
    }
    const order = {
      domain: capabilities.domain,
      recovery: this.fastPayRecoveryWindow(capabilities),
      inputs: inputs.map(input => ({ id: input.id, version: input.version })),
      to_address: toAddress,
      amount: amountAtoms,
      asset: 'PFT',
      fee: feeAtoms,
      nonce: Date.now() * 1000,
      memos: [],
    };
    order.recovery.lock_id = wasm.wallet_fastpay_unwrap_lock_id(JSON.stringify(order));
    const signedOrder = wasm.wallet_sign_owned_unwrap_v3(
      backupJson,
      JSON.stringify(order),
      JSON.stringify(capabilities),
    );
    if (
      signedOrder?.owner_pubkey_hex !== ownerPubkeyHex
      || signedOrder?.order?.recovery?.lock_id !== order.recovery.lock_id
    ) {
      throw new Error('Wallet signer returned a different FastPay v3 owner or lock');
    }
    const { votes, failures } = await this.collectFastPayV3Votes(
      signedOrder,
      validators,
      validatorContext.quorum,
      true,
    );
    if (votes.length < validatorContext.quorum) {
      throw new FastPayRecoveryPendingError(
        `FastPay unwrap collected ${votes.length}/${validatorContext.quorum} votes; locked inputs require ordered recovery`,
        { signed_order: { operation: 'unwrap', signed_order: signedOrder }, failures },
      );
    }
    const certificate = {
      order: signedOrder.order,
      owner_pubkey_hex: signedOrder.owner_pubkey_hex,
      owner_signature_hex: signedOrder.owner_signature_hex,
      votes,
    };
    const certificateJson = JSON.stringify(certificate);
    const certificateDigest = wasm.wallet_fastpay_unwrap_certificate_digest(certificateJson);
    let applyResponse;
    try {
      applyResponse = await this.rpc.ownedUnwrapApplyV3(certificateJson);
    } catch (error) {
      throw new FastPayRecoveryPendingError(
        `FastPay unwrap apply outcome is unknown: ${error?.message || error}`,
        {
          certificate,
          signed_order: this.fastPaySignedOrderFromCertificate(certificate),
          accepted_acknowledgements: [],
        },
      );
    }
    const acknowledgements = this.verifyFastPayV3Apply(
      wasm,
      applyResponse,
      certificate,
      certificateDigest,
      capabilities,
      validatorContext,
    );
    return {
      status: 'finalized',
      order: signedOrder.order,
      cert: certificate,
      votes,
      acknowledgements,
      applyResult: applyResponse.result,
      input: inputs[0],
      inputs,
    };
  }

  async pollReceipt(txId, timeoutMs) {
    const deadline = Date.now() + timeoutMs;
    while (Date.now() < deadline) {
      await new Promise(r => setTimeout(r, 2000));
      try {
        const resp = await this.rpc.receipts(txId);
        if (resp.ok && resp.result && Array.isArray(resp.result) && resp.result.length > 0) {
          return resp.result[0];
        }
      } catch (e) {
        // keep polling
      }
    }
    return { accepted: null, message: 'timeout' };
  }
}
