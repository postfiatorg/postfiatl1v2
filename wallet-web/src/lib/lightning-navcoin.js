import { assertNoCustodyMaterial } from './custody-boundary.js';
import { sha3_384DomainHex } from './evm.js';
import {
  canonicalQuoteBytes,
  verifySignedQuoteEnvelope,
} from './lightning-quote-signature.js';
import { verifyMainnetBolt11Invoice } from './bolt11-verification.js';
import { LIGHTNING_NAVCOIN_RELEASE_PINS } from './lightning-navcoin-release.js';

export const LIGHTNING_NAVCOIN_API_ROOT = '/api/lightning-navcoin/v1';
export const LIGHTNING_NAVCOIN_STATUS_SCHEMA = 'postfiat.lightning_navcoin.status.v1';
export const LIGHTNING_SUBMARINE_QUOTE_SCHEMA = 'postfiat.lightning_submarine_quote.v1';
export const LIGHTNING_TO_PFTL = 'lightning_to_pftl';
export const PFTL_TO_LIGHTNING = 'pftl_to_lightning';
export const CONTROLLED_CLAIM = 'non-custodial, conditionally atomic, COORDINATOR-TRUSTED timing';

const CONDITION_PREFIX = 'a0258020';
const CONDITION_SUFFIX = '810120';
const FULFILLMENT_PREFIX = 'a0228020';
const PFTL_ADDRESS = /^pf[0-9a-f]{40}$/;
const HEX_32 = /^[0-9a-f]{64}$/;
const HEX_48 = /^[0-9a-f]{96}$/;
const BOLT11_MAINNET = /^lnbc(?:[0-9]+[munp]?)?1[02-9ac-hj-np-z]+$/i;
const COMPRESSED_SECP256K1_KEY = /^(02|03)[0-9a-f]{64}$/;
const DECIMAL_INTEGER = /^(0|[1-9][0-9]*)$/;
const MAX_RESPONSE_BYTES = 256 * 1024;
const MAX_BOLT11_CHARS = 8192;
const MAX_INSPECTION_DEPTH = 32;
const MAX_COLLECTION_ITEMS = 4096;
const DEFAULT_TIMEOUT_MS = 12_000;
const CSRF_SESSION_KEY = 'postfiat.lightning_navcoin.csrf.v1';
const QUOTE_REQUEST_SESSION_KEY = 'postfiat.lightning_navcoin.quote_request.v1';
const ESCROW_CONDITION_HASH_DOMAIN = 'postfiat.escrow_condition_hash.v1';

const FORBIDDEN_COORDINATOR_FIELDS = new Set([
  'backup',
  'backup_json',
  'decrypted_backup',
  'fulfillment',
  'master_seed',
  'master_seed_hex',
  'mnemonic',
  'passphrase',
  'payment_preimage',
  'preimage',
  'preimage_hex',
  'private_key',
  'private_key_hex',
  'secret',
  'secret_key',
  'secret_key_hex',
  'seed',
  'seed_hex',
  'seed_phrase',
  'signing_key',
  'spend_key',
]);

function isRecord(value) {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function requireRecord(value, label) {
  if (!isRecord(value)) throw new Error(`${label} must be an object`);
  return value;
}

function requireString(value, label, maxLength = 4096) {
  if (typeof value !== 'string' || value.length === 0 || value.length > maxLength) {
    throw new Error(`${label} must be a nonempty string no longer than ${maxLength} characters`);
  }
  return value;
}

function requireBoolean(value, label) {
  if (typeof value !== 'boolean') throw new Error(`${label} must be a boolean`);
  return value;
}

function requireSafeInteger(value, label, minimum = 0) {
  if (!Number.isSafeInteger(value) || value < minimum) {
    throw new Error(`${label} must be a safe integer of at least ${minimum}`);
  }
  return value;
}

function requireDecimalInteger(value, label, minimum = 0n) {
  const text = typeof value === 'number'
    ? (Number.isSafeInteger(value) ? String(value) : '')
    : String(value ?? '');
  if (!DECIMAL_INTEGER.test(text) || BigInt(text) < minimum) {
    throw new Error(`${label} must be a canonical nonnegative decimal integer`);
  }
  return text;
}

function requireHex32(value, label) {
  if (typeof value !== 'string' || !HEX_32.test(value)) {
    throw new Error(`${label} must be canonical lowercase 32-byte hex`);
  }
  return value;
}

function requireHex48(value, label) {
  if (typeof value !== 'string' || !HEX_48.test(value)) {
    throw new Error(`${label} must be canonical lowercase 48-byte hex`);
  }
  return value;
}

function requireCompressedSecp256k1Key(value, label) {
  if (typeof value !== 'string' || !COMPRESSED_SECP256K1_KEY.test(value)) {
    throw new Error(`${label} must be a canonical compressed secp256k1 public key`);
  }
  return value;
}

function requirePftlAddress(value, label) {
  if (typeof value !== 'string' || !PFTL_ADDRESS.test(value)) {
    throw new Error(`${label} must be a canonical PFTL address`);
  }
  return value;
}

function normalizedKey(value) {
  return String(value || '')
    .replace(/([a-z0-9])([A-Z])/g, '$1_$2')
    .replace(/[^A-Za-z0-9]+/g, '_')
    .replace(/^_+|_+$/g, '')
    .toLowerCase();
}

/**
 * Poll responses intentionally omit a Lightning preimage and any wallet
 * custody material. The payer supplies the preimage to the browser directly;
 * it is never fetched from or posted to the coordinator.
 */
export function assertSecretFreeCoordinatorValue(value, context = 'coordinator payload') {
  assertNoCustodyMaterial(value, context);
  let inspected = 0;
  const seen = new WeakSet();

  function visit(current, path, depth) {
    inspected += 1;
    if (inspected > MAX_COLLECTION_ITEMS) {
      throw new Error(`${context} exceeds the inspection item limit`);
    }
    if (depth > MAX_INSPECTION_DEPTH) {
      throw new Error(`${context} exceeds the inspection depth limit`);
    }
    if (current === null || typeof current !== 'object') return;
    if (seen.has(current)) return;
    seen.add(current);
    if (Array.isArray(current)) {
      current.forEach((item, index) => visit(item, `${path}[${index}]`, depth + 1));
      return;
    }
    for (const [key, child] of Object.entries(current)) {
      const childPath = `${path}.${key}`;
      if (FORBIDDEN_COORDINATOR_FIELDS.has(normalizedKey(key))) {
        throw new Error(`${context} contains forbidden secret field ${childPath}`);
      }
      visit(child, childPath, depth + 1);
    }
  }

  visit(value, '$', 0);
  return value;
}

function unwrapResponse(payload, label) {
  const envelope = requireRecord(payload, label);
  if (envelope.ok !== true) {
    const message = typeof envelope.error === 'string'
      ? envelope.error
      : envelope.error?.message || envelope.message || `${label} failed`;
    throw new Error(String(message).slice(0, 512));
  }
  return requireRecord(envelope.result, `${label}.result`);
}

function holdReasons(value) {
  if (value === undefined) return [];
  if (!Array.isArray(value) || value.length > 32) {
    throw new Error('hold_reasons must be an array with at most 32 entries');
  }
  return value.map((reason, index) => requireString(reason, `hold_reasons[${index}]`, 512));
}

function normalizeQuorum(value) {
  if (value === undefined || value === null) {
    return { observed: 0, required: 6, validatorCount: 6, converged: false };
  }
  const quorum = requireRecord(value, 'pftl.quorum');
  const observed = requireSafeInteger(
    quorum.observed ?? quorum.agreeing_validator_count ?? 0,
    'pftl.quorum.observed',
  );
  const required = requireSafeInteger(quorum.required ?? 5, 'pftl.quorum.required', 1);
  const validatorCount = requireSafeInteger(
    quorum.validator_count ?? quorum.validatorCount ?? 6,
    'pftl.quorum.validator_count',
    required,
  );
  const converged = requireBoolean(quorum.converged ?? false, 'pftl.quorum.converged');
  if (observed > validatorCount || (converged && observed < required)) {
    throw new Error('PFTL quorum counts are inconsistent');
  }
  return { observed, required, validatorCount, converged };
}

function normalizeReceipt(value) {
  if (value === undefined || value === null) return null;
  const receipt = requireRecord(value, 'pftl.receipt');
  const accepted = requireBoolean(receipt.accepted, 'pftl.receipt.accepted');
  return {
    accepted,
    code: typeof receipt.code === 'string' ? receipt.code.slice(0, 128) : '',
    txId: typeof receipt.tx_id === 'string' ? receipt.tx_id.slice(0, 256) : '',
    reason: typeof receipt.reason === 'string'
      ? receipt.reason.slice(0, 512)
      : typeof receipt.message === 'string' ? receipt.message.slice(0, 512) : '',
  };
}

function normalizePftl(value, quote = null) {
  const pftl = value == null ? {} : requireRecord(value, 'pftl');
  const escrowValue = pftl.escrow == null ? {} : requireRecord(pftl.escrow, 'pftl.escrow');
  const balanceValue = pftl.wallet_balance_atoms ?? pftl.balance_atoms;
  return {
    raw: pftl,
    chainId: pftl.chain_id === undefined
      ? (quote?.pftl_chain_id ?? null)
      : requireString(pftl.chain_id, 'pftl.chain_id', 128),
    genesisHash: pftl.genesis_hash === undefined
      ? (quote?.pftl_genesis_hash ?? null)
      : requireHex48(pftl.genesis_hash, 'pftl.genesis_hash'),
    assetId: pftl.asset_id === undefined
      ? (quote?.pftl_asset_id ?? null)
      : requireHex48(pftl.asset_id, 'pftl.asset_id'),
    navEpoch: pftl.nav_epoch === undefined
      ? (quote?.nav_epoch ?? null)
      : requireSafeInteger(pftl.nav_epoch, 'pftl.nav_epoch', 1),
    navReservePacketHash: pftl.nav_reserve_packet_hash === undefined
      ? (quote?.nav_reserve_packet_hash ?? null)
      : requireHex48(pftl.nav_reserve_packet_hash, 'pftl.nav_reserve_packet_hash'),
    quorum: normalizeQuorum(pftl.quorum),
    receipt: normalizeReceipt(pftl.receipt),
    balanceAtoms: balanceValue === undefined
      ? null
      : requireDecimalInteger(balanceValue, 'pftl.wallet_balance_atoms'),
    height: pftl.height === undefined ? null : requireSafeInteger(pftl.height, 'pftl.height'),
    stateRoot: pftl.state_root === undefined
      ? null
      : requireHex48(pftl.state_root, 'pftl.state_root'),
    blockTipHash: pftl.block_tip_hash === undefined
      ? null
      : requireHex48(pftl.block_tip_hash, 'pftl.block_tip_hash'),
    buildGitRevision: pftl.build_git_revision === undefined
      ? null
      : requireString(
          pftl.build_git_revision,
          'pftl.build_git_revision',
          128,
        ),
    escrow: {
      id: escrowValue.escrow_id ?? quote?.expected_escrow_id ?? null,
      state: typeof escrowValue.state === 'string' ? escrowValue.state : null,
      owner: escrowValue.owner ?? quote?.pftl_owner ?? null,
      recipient: escrowValue.recipient ?? quote?.pftl_recipient ?? null,
      assetId: escrowValue.asset_id ?? quote?.pftl_asset_id ?? null,
      amountAtoms: escrowValue.amount === undefined
        ? (quote?.pftl_amount_atoms === undefined
            ? null
            : requireDecimalInteger(quote.pftl_amount_atoms, 'quote.pftl_amount_atoms', 1n))
        : requireDecimalInteger(escrowValue.amount, 'pftl.escrow.amount', 1n),
      condition: escrowValue.condition ?? quote?.condition ?? null,
      finishAfter: escrowValue.finish_after ?? quote?.finish_after ?? null,
      cancelAfter: escrowValue.cancel_after ?? quote?.cancel_after ?? null,
    },
  };
}

function normalizeValuationBinding(value, pftl, releasePins) {
  const binding = requireRecord(value, 'pftl_valuation_binding');
  if (binding.schema !== 'postfiat.lightning_pftl_valuation_binding.v1') {
    throw new Error('unsupported PFTL valuation-binding evidence schema');
  }
  const digestList = (candidate, label) => {
    if (!Array.isArray(candidate) || candidate.length !== 6) {
      throw new Error(`${label} must contain exactly six validator digests`);
    }
    return candidate.map((digest, index) => requireHex32(
      digest,
      `${label}[${index}]`,
    ));
  };
  const normalized = {
    schema: binding.schema,
    height: requireSafeInteger(binding.height, 'pftl_valuation_binding.height', 1),
    blockTipHash: requireHex48(
      binding.block_tip_hash,
      'pftl_valuation_binding.block_tip_hash',
    ),
    stateRoot: requireHex48(
      binding.state_root,
      'pftl_valuation_binding.state_root',
    ),
    assetId: requireHex48(binding.asset_id, 'pftl_valuation_binding.asset_id'),
    navEpoch: requireSafeInteger(
      binding.nav_epoch,
      'pftl_valuation_binding.nav_epoch',
      1,
    ),
    navPerUnitUsdE8: requireDecimalInteger(
      binding.nav_per_unit_usd_e8,
      'pftl_valuation_binding.nav_per_unit_usd_e8',
      1n,
    ),
    reservePacketHash: requireHex48(
      binding.reserve_packet_hash,
      'pftl_valuation_binding.reserve_packet_hash',
    ),
    valuationUnit: requireString(
      binding.valuation_unit,
      'pftl_valuation_binding.valuation_unit',
      64,
    ),
    valuationScale: requireSafeInteger(
      binding.valuation_scale,
      'pftl_valuation_binding.valuation_scale',
      1,
    ),
    validatorCount: requireSafeInteger(
      binding.validator_count,
      'pftl_valuation_binding.validator_count',
      1,
    ),
    ledgerSha256: digestList(binding.ledger_sha256, 'pftl_valuation_binding.ledger_sha256'),
    chainTipSha256: digestList(
      binding.chain_tip_sha256,
      'pftl_valuation_binding.chain_tip_sha256',
    ),
    stateVerificationSha256: digestList(
      binding.state_verification_sha256,
      'pftl_valuation_binding.state_verification_sha256',
    ),
  };
  if (
    normalized.height !== pftl.height
    || normalized.blockTipHash !== pftl.blockTipHash
    || normalized.stateRoot !== pftl.stateRoot
    || normalized.assetId !== pftl.assetId
    || normalized.assetId !== releasePins.pftlAssetId
    || normalized.navEpoch !== pftl.navEpoch
    || normalized.navEpoch !== releasePins.pftlNavEpoch
    || normalized.reservePacketHash !== pftl.navReservePacketHash
    || normalized.reservePacketHash !== releasePins.pftlNavReservePacketHash
    || normalized.valuationUnit !== 'USD_PER_WHOLE_ASSET_UNIT'
    || normalized.valuationScale !== 100_000_000
    || normalized.validatorCount !== 6
  ) {
    throw new Error('PFTL valuation-binding evidence does not match the six-validator route');
  }
  return Object.freeze({ ...normalized, verified: true });
}

function normalizeProofAssurance(value) {
  const assurance = requireRecord(value, 'pftl_proof_assurance');
  if (
    assurance.schema !== 'postfiat.lightning_pftl_proof_assurance.v1'
    || assurance.profile !== 'multi-fetch-quorum'
    || assurance.attestation_count !== 1
    || assurance.proof_bytes_stored_on_chain !== true
    || assurance.consensus_native_groth16_verification !== false
    || !Array.isArray(assurance.lifecycle)
    || assurance.lifecycle.length !== 3
    || assurance.lifecycle[0] !== 'nav_reserve_submit'
    || assurance.lifecycle[1] !== 'nav_reserve_attest'
    || assurance.lifecycle[2] !== 'nav_epoch_finalize'
  ) {
    throw new Error('PFTL proof-assurance boundary differs from the reviewed CONTROLLED lane');
  }
  return Object.freeze({
    profile: assurance.profile,
    attestationCount: assurance.attestation_count,
    proofBytesStoredOnChain: true,
    consensusNativeGroth16Verification: false,
    lifecycle: Object.freeze([...assurance.lifecycle]),
  });
}

export function validateCoordinatorStatus(
  payload,
  releasePins = LIGHTNING_NAVCOIN_RELEASE_PINS,
) {
  assertSecretFreeCoordinatorValue(payload, 'coordinator status');
  const status = unwrapResponse(payload, 'coordinator status');
  if (status.schema !== LIGHTNING_NAVCOIN_STATUS_SCHEMA) {
    throw new Error(`unsupported coordinator status schema: ${status.schema || '<missing>'}`);
  }
  if (status.lightning_network !== 'bitcoin') {
    throw new Error('coordinator is not bound to Bitcoin mainnet Lightning');
  }
  const mode = requireString(status.mode, 'mode', 32);
  if (!['DRY_RUN', 'HOLD', 'ARMED'].includes(mode)) {
    throw new Error('coordinator mode must be DRY_RUN, HOLD, or ARMED');
  }
  const canExecute = requireBoolean(status.can_execute, 'can_execute');
  if (canExecute && mode !== 'ARMED') {
    throw new Error('coordinator cannot execute unless mode is ARMED');
  }
  if (
    status.trust_class !== 'CONTROLLED'
    || status.atomicity_claim !== CONTROLLED_CLAIM
  ) {
    throw new Error('coordinator status changed the CONTROLLED trust/atomicity claim');
  }
  const quoteSignerPublicKey = requireHex32(
    status.quote_signer_public_key_hex,
    'quote_signer_public_key_hex',
  );
  if (
    quoteSignerPublicKey
    !== releasePins.quoteSignerPublicKeyHex
  ) {
    throw new Error(
      'coordinator quote signer does not match the reviewed wallet release pin',
    );
  }
  const reasons = holdReasons(status.hold_reasons);
  const pftl = normalizePftl(status.pftl);
  const releaseRouteFields = [
    ['chain id', pftl.chainId, releasePins.pftlChainId],
    [
      'genesis hash',
      pftl.genesisHash,
      releasePins.pftlGenesisHash,
    ],
    ['asset id', pftl.assetId, releasePins.pftlAssetId],
    [
      'build revision',
      pftl.buildGitRevision,
      releasePins.pftlBuildGitRevision,
    ],
    ['NAV epoch', pftl.navEpoch, releasePins.pftlNavEpoch],
    [
      'NAV reserve packet',
      pftl.navReservePacketHash,
      releasePins.pftlNavReservePacketHash,
    ],
  ];
  for (const [label, observed, expected] of releaseRouteFields) {
    if (observed !== null && observed !== expected) {
      throw new Error(
        `coordinator PFTL ${label} does not match the reviewed wallet release pin`,
      );
    }
  }
  const lndRaw = isRecord(status.lnd) ? status.lnd : {};
  const lndIdentityPubkey = lndRaw.identity_pubkey === undefined
    ? null
    : requireCompressedSecp256k1Key(
        lndRaw.identity_pubkey,
        'lnd.identity_pubkey',
      );
  const lnd = { ...lndRaw, identityPubkey: lndIdentityPubkey };
  const perRunUsdE8 = status.limits?.per_run_usd_e8 === undefined
    ? null
    : requireDecimalInteger(status.limits.per_run_usd_e8, 'limits.per_run_usd_e8');
  const totalUsdE8 = status.limits?.total_usd_e8 === undefined
    ? null
    : requireDecimalInteger(status.limits.total_usd_e8, 'limits.total_usd_e8');
  const maxAmountMsat = status.limits?.max_amount_msat === undefined
    ? null
    : requireDecimalInteger(status.limits.max_amount_msat, 'limits.max_amount_msat', 1n);
  const maxFeeMsat = status.limits?.max_fee_msat === undefined
    ? null
    : requireDecimalInteger(status.limits.max_fee_msat, 'limits.max_fee_msat');
  const btcUsdE8 = status.pricing?.btc_usd_e8 === undefined
    ? null
    : requireDecimalInteger(status.pricing.btc_usd_e8, 'pricing.btc_usd_e8', 1n);
  const valuationBinding = status.pftl_valuation_binding === undefined
    ? null
    : normalizeValuationBinding(
        status.pftl_valuation_binding,
        pftl,
        releasePins,
      );
  const proofAssurance = status.pftl_proof_assurance === undefined
    ? null
    : normalizeProofAssurance(status.pftl_proof_assurance);
  if (canExecute) {
    if (reasons.length !== 0) throw new Error('executable coordinator status cannot have hold reasons');
    if (lnd.network !== 'mainnet' || lnd.synced_to_chain !== true) {
      throw new Error('executable coordinator status requires a chain-synced mainnet LND');
    }
    if (
      releasePins.lndIdentityPubkeyHex === null
      || lndIdentityPubkey
        !== releasePins.lndIdentityPubkeyHex
    ) {
      throw new Error(
        'executable coordinator LND identity does not match a reviewed wallet release pin',
      );
    }
    if (
      !pftl.chainId
      || !pftl.genesisHash
      || !pftl.assetId
      || !pftl.buildGitRevision
      || !pftl.navEpoch
      || !pftl.navReservePacketHash
      || !pftl.stateRoot
      || !pftl.blockTipHash
      || !pftl.quorum.converged
      || pftl.quorum.observed < pftl.quorum.required
      || pftl.quorum.required !== 6
      || pftl.quorum.observed !== 6
      || pftl.quorum.validatorCount !== 6
    ) {
      throw new Error('executable coordinator status requires proven-NAV binding and converged 6-of-6 PFTL state');
    }
    if (
      perRunUsdE8 === null
      || totalUsdE8 === null
      || maxAmountMsat === null
      || maxFeeMsat === null
      || btcUsdE8 === null
      || valuationBinding === null
      || proofAssurance === null
      || BigInt(perRunUsdE8) < 1n
      || BigInt(perRunUsdE8) > 500_000_000n
      || BigInt(totalUsdE8) < 1n
      || BigInt(totalUsdE8) > 2_000_000_000n
    ) {
      throw new Error('executable coordinator status exceeds or omits the real-value dust caps');
    }
  }
  return {
    raw: status,
    mode,
    canExecute,
    holdReasons: reasons,
    lightningNetwork: status.lightning_network,
    lnd,
    pftl,
    perRunUsdE8,
    totalUsdE8,
    maxAmountMsat,
    maxFeeMsat,
    valuationBinding,
    proofAssurance,
    quoteSignerPublicKey,
    btcUsdE8,
  };
}

export function buildPayerFeeAcknowledgement(
  status,
  principalMsatValue,
  displayedFeeSatsValue,
  acknowledgedAtUnix = Math.floor(Date.now() / 1000),
) {
  if (!status) throw new Error('payer-fee acknowledgement requires coordinator status');
  const principalMsat = requireDecimalInteger(
    principalMsatValue,
    'principal msat',
    1n,
  );
  const displayedFeeSats = requireDecimalInteger(
    displayedFeeSatsValue,
    'displayed payer fee sats',
  );
  const maxFeeMsat = requireDecimalInteger(
    status.maxFeeMsat,
    'coordinator max fee msat',
  );
  const btcUsdE8 = requireDecimalInteger(
    status.btcUsdE8,
    'coordinator BTC/USD e8',
    1n,
  );
  const perRunUsdE8 = requireDecimalInteger(
    status.perRunUsdE8,
    'coordinator per-run USD e8',
    1n,
  );
  const acknowledgedAt = requireSafeInteger(
    acknowledgedAtUnix,
    'payer-fee acknowledgement time',
    1,
  );
  const displayedFeeMsat = BigInt(displayedFeeSats) * 1000n;
  if (displayedFeeMsat > BigInt(maxFeeMsat)) {
    throw new Error('displayed payer-wallet fee exceeds the reserved coordinator fee cap');
  }
  const allInMsat = BigInt(principalMsat) + displayedFeeMsat;
  const scale = 100_000_000_000n;
  const allInUsdE8 = (
    (allInMsat * BigInt(btcUsdE8)) + scale - 1n
  ) / scale;
  if (allInUsdE8 > BigInt(perRunUsdE8)) {
    throw new Error('principal plus displayed payer-wallet fee exceeds the per-run USD cap');
  }
  return Object.freeze({
    schema: 'postfiat.lightning_payer_fee_acknowledgement.v1',
    principal_msat: principalMsat,
    displayed_fee_msat: displayedFeeMsat.toString(),
    coordinator_max_fee_msat: maxFeeMsat,
    all_in_usd_e8: allInUsdE8.toString(),
    per_run_usd_e8: perRunUsdE8,
    acknowledged_at_unix: acknowledgedAt,
  });
}

function validateMainnetInvoice(value) {
  const invoice = requireString(value, 'invoice', MAX_BOLT11_CHARS);
  if (!BOLT11_MAINNET.test(invoice) || invoice.toLowerCase() !== invoice) {
    throw new Error('invoice must be a canonical lowercase Bitcoin-mainnet BOLT11 invoice');
  }
  return invoice;
}

function normalizeQuote(value, releasePins = LIGHTNING_NAVCOIN_RELEASE_PINS) {
  const quote = requireRecord(value, 'quote');
  if (quote.schema !== LIGHTNING_SUBMARINE_QUOTE_SCHEMA) {
    throw new Error(`unsupported quote schema: ${quote.schema || '<missing>'}`);
  }
  const direction = quote.direction;
  if (![LIGHTNING_TO_PFTL, PFTL_TO_LIGHTNING].includes(direction)) {
    throw new Error('quote has an unsupported direction');
  }
  if (quote.lightning_network !== 'bitcoin') {
    throw new Error('quote is not bound to Bitcoin mainnet Lightning');
  }
  const paymentHash = requireHex32(quote.payment_hash, 'quote.payment_hash');
  const condition = requireString(quote.condition, 'quote.condition', 256);
  if (condition !== `${CONDITION_PREFIX}${paymentHash}${CONDITION_SUFFIX}`) {
    throw new Error('quote condition does not canonically bind its Lightning payment hash');
  }
  const invoicePayee = requireCompressedSecp256k1Key(
    quote.invoice_payee,
    'quote.invoice_payee',
  );
  const invoiceAmountMsat = requireDecimalInteger(
    quote.invoice_amount_msat,
    'quote.invoice_amount_msat',
    1n,
  );
  const invoiceExpires = requireSafeInteger(
    quote.invoice_expiry_unix,
    'quote.invoice_expiry_unix',
    1,
  );
  const minFinalCltvDelta = requireSafeInteger(
    quote.min_final_cltv_delta,
    'quote.min_final_cltv_delta',
    1,
  );
  const invoice = validateMainnetInvoice(quote.invoice);
  const decodedInvoice = verifyMainnetBolt11Invoice(invoice, {
    paymentHash,
    amountMsat: invoiceAmountMsat,
    payee: invoicePayee,
    expiryUnix: invoiceExpires,
    minFinalCltvDelta,
  });
  const releasePayee = releasePins.lndIdentityPubkeyHex;
  if (releasePayee !== null && invoicePayee !== releasePayee) {
    throw new Error('signed quote invoice payee does not match the reviewed LND release pin');
  }
  const owner = requirePftlAddress(quote.pftl_owner, 'quote.pftl_owner');
  const recipient = requirePftlAddress(quote.pftl_recipient, 'quote.pftl_recipient');
  if (owner === recipient) throw new Error('quote PFTL owner and recipient must differ');
  if (
    quote.custody_class !== 'NON_CUSTODIAL_HASHLOCK'
    || quote.atomicity_class !== 'CONDITIONAL_HTLC'
    || quote.timeout_clock_class !== 'OFFCHAIN_CROSS_LEDGER_POLICY'
    || quote.asset_control_class !== 'CONTROLLED_ISSUED_ASSET'
  ) {
    throw new Error('quote trust/control classes do not match the CONTROLLED conditional-atomic route');
  }
  const finishAfter = requireSafeInteger(quote.finish_after, 'quote.finish_after');
  const cancelAfter = requireSafeInteger(quote.cancel_after, 'quote.cancel_after', 1);
  if (finishAfter > 0 && cancelAfter <= finishAfter) {
    throw new Error('quote cancel_after must be after finish_after');
  }
  const quoteExpires = requireSafeInteger(quote.quote_expires_unix, 'quote.quote_expires_unix', 1);
  const latestStart = requireSafeInteger(
    quote.latest_lightning_start_unix,
    'quote.latest_lightning_start_unix',
    1,
  );
  if (latestStart > invoiceExpires || quoteExpires > invoiceExpires) {
    throw new Error('quote expiry boundaries are inconsistent with invoice expiry');
  }
  const chainId = requireString(quote.pftl_chain_id, 'quote.pftl_chain_id', 128);
  const genesisHash = requireHex48(
    quote.pftl_genesis_hash,
    'quote.pftl_genesis_hash',
  );
  const assetId = requireHex48(quote.pftl_asset_id, 'quote.pftl_asset_id');
  const navEpoch = requireSafeInteger(quote.nav_epoch, 'quote.nav_epoch', 1);
  const navReservePacketHash = requireHex48(
    quote.nav_reserve_packet_hash,
    'quote.nav_reserve_packet_hash',
  );
  if (
    chainId !== releasePins.pftlChainId
    || genesisHash !== releasePins.pftlGenesisHash
    || assetId !== releasePins.pftlAssetId
    || navEpoch !== releasePins.pftlNavEpoch
    || navReservePacketHash
      !== releasePins.pftlNavReservePacketHash
  ) {
    throw new Error('signed quote does not match the reviewed PFTL release pins');
  }
  if (
    requireSafeInteger(quote.max_total_cltv_delta, 'quote.max_total_cltv_delta', 1)
    < minFinalCltvDelta
  ) {
    throw new Error('quote maximum CLTV delta is below its final-hop delta');
  }
  return {
    raw: quote,
    swapId: requireHex32(quote.swap_id, 'quote.swap_id'),
    direction,
    invoice,
    decodedInvoice,
    invoicePayee,
    paymentHash,
    invoiceAmountMsat,
    quoteExpires,
    invoiceExpires,
    latestStart,
    minFinalCltvDelta,
    maxTotalCltvDelta: quote.max_total_cltv_delta,
    chainId,
    genesisHash,
    assetId,
    amountAtoms: requireDecimalInteger(quote.pftl_amount_atoms, 'quote.pftl_amount_atoms', 1n),
    owner,
    ownerSequence: requireSafeInteger(
      quote.pftl_owner_sequence,
      'quote.pftl_owner_sequence',
      1,
    ),
    recipient,
    escrowId: requireHex48(quote.expected_escrow_id, 'quote.expected_escrow_id'),
    condition,
    finishAfter,
    cancelAfter,
    navEpoch,
    navReservePacketHash,
  };
}

/**
 * Normalize both quote creation and subsequent secret-free status snapshots.
 * The signed quote is immutable; live state lives alongside it.
 */
export function validateSwapSnapshot(
  payload,
  releasePins = LIGHTNING_NAVCOIN_RELEASE_PINS,
) {
  assertSecretFreeCoordinatorValue(payload, 'coordinator swap snapshot');
  const result = unwrapResponse(payload, 'coordinator swap snapshot');
  const quoteEnvelope = isRecord(result.signed_quote) ? result.signed_quote : null;
  const quoteValue = result.quote ?? quoteEnvelope?.quote;
  if (quoteValue === undefined || quoteValue === null) {
    const direction = requireString(result.direction, 'swap direction', 32);
    if (![LIGHTNING_TO_PFTL, PFTL_TO_LIGHTNING].includes(direction)) {
      throw new Error('pending swap direction is unsupported');
    }
    const canExecute = requireBoolean(result.can_execute, 'swap can_execute');
    if (canExecute) {
      throw new Error('a pending swap cannot execute before its signed quote is visible');
    }
    return {
      raw: result,
      quote: null,
      swapId: requireHex32(result.swap_id, 'swap_id'),
      direction,
      invoiceAmountMsat: requireDecimalInteger(
        result.invoice_amount_msat,
        'invoice_amount_msat',
        1n,
      ),
      walletAddress: requirePftlAddress(result.wallet_address, 'wallet_address'),
      pftlAmountAtoms: requireDecimalInteger(
        result.pftl_amount_atoms,
        'pftl_amount_atoms',
        1n,
      ),
      paymentHash: requireHex32(result.payment_hash, 'payment_hash'),
      state: requireString(result.state, 'swap state', 64),
      canExecute,
      holdReasons: holdReasons(result.hold_reasons),
      lightning: {},
      pftl: normalizePftl(result.pftl),
    };
  }
  const quote = normalizeQuote(quoteValue, releasePins);
  if (result.swap_id !== undefined && result.swap_id !== quote.swapId) {
    throw new Error('swap snapshot id does not match signed quote');
  }
  const state = requireString(result.state ?? 'QUOTED', 'swap state', 64);
  const canExecute = requireBoolean(result.can_execute ?? false, 'swap can_execute');
  const pftl = normalizePftl(result.pftl, quote.raw);
  return {
    raw: result,
    quote,
    swapId: quote.swapId,
    direction: quote.direction,
    invoiceAmountMsat: quote.invoiceAmountMsat,
    walletAddress: quote.direction === LIGHTNING_TO_PFTL ? quote.recipient : quote.owner,
    pftlAmountAtoms: quote.amountAtoms,
    paymentHash: quote.paymentHash,
    state,
    canExecute,
    holdReasons: holdReasons(result.hold_reasons),
    lightning: isRecord(result.lightning) ? result.lightning : {},
    pftl,
  };
}

function randomHex(bytes) {
  const cryptoApi = globalThis.crypto;
  if (!cryptoApi?.getRandomValues) throw new Error('secure browser randomness is unavailable');
  const value = new Uint8Array(bytes);
  cryptoApi.getRandomValues(value);
  return Array.from(value, byte => byte.toString(16).padStart(2, '0')).join('');
}

function csrfToken() {
  try {
    const current = globalThis.sessionStorage?.getItem(CSRF_SESSION_KEY);
    if (current && HEX_32.test(current)) return current;
    const token = randomHex(32);
    globalThis.sessionStorage?.setItem(CSRF_SESSION_KEY, token);
    return token;
  } catch (_) {
    return randomHex(32);
  }
}

function quoteRequestBody({
  direction,
  amountMsat,
  walletAddress,
  invoice,
  clientRequestId,
}) {
  if (![LIGHTNING_TO_PFTL, PFTL_TO_LIGHTNING].includes(direction)) {
    throw new Error('direction must be lightning_to_pftl or pftl_to_lightning');
  }
  const body = {
    direction,
    amount_msat: requireDecimalInteger(amountMsat, 'amount_msat', 1n),
    wallet_address: requirePftlAddress(walletAddress, 'wallet_address'),
    client_request_id: requireHex32(clientRequestId, 'client_request_id'),
  };
  if (direction === PFTL_TO_LIGHTNING) {
    body.invoice = validateMainnetInvoice(invoice);
  } else if (invoice !== undefined && invoice !== null && invoice !== '') {
    throw new Error('on-ramp quote requests must not supply an invoice');
  }
  assertSecretFreeCoordinatorValue(body, 'coordinator quote request');
  return body;
}

function quoteRequestTerms(body) {
  return JSON.stringify({
    direction: body.direction,
    amount_msat: body.amount_msat,
    wallet_address: body.wallet_address,
    ...(body.invoice === undefined ? {} : { invoice: body.invoice }),
  });
}

function decodeStoredQuoteRequest(encoded) {
  if (typeof encoded !== 'string' || encoded.length > MAX_BOLT11_CHARS + 1024) {
    throw new Error('durable quote request record is malformed');
  }
  let record;
  try {
    record = JSON.parse(encoded);
  } catch (_) {
    throw new Error('durable quote request record is not valid JSON');
  }
  if (
    !isRecord(record)
    || record.schema !== 'postfiat.lightning_navcoin.quote_request.v1'
    || !isRecord(record.body)
  ) {
    throw new Error('durable quote request record has an unsupported schema');
  }
  const body = quoteRequestBody({
    direction: record.body.direction,
    amountMsat: record.body.amount_msat,
    walletAddress: record.body.wallet_address,
    invoice: record.body.invoice,
    clientRequestId: record.body.client_request_id,
  });
  if (JSON.stringify(body) !== JSON.stringify(record.body)) {
    throw new Error('durable quote request record is non-canonical');
  }
  return body;
}

export class LightningNavcoinClient {
  constructor({
    fetchImpl = globalThis.fetch,
    csrf = '',
    releasePins = LIGHTNING_NAVCOIN_RELEASE_PINS,
    quoteRequestStorage,
  } = {}) {
    if (typeof fetchImpl !== 'function') throw new Error('fetch is unavailable');
    this.fetchImpl = fetchImpl;
    this.csrf = csrf || csrfToken();
    this.releasePins = releasePins;
    this.releaseStatusVerified = false;
    this.quoteRequestStorage = quoteRequestStorage === undefined
      ? (() => {
          try {
            return globalThis.sessionStorage ?? null;
          } catch (_) {
            return null;
          }
        })()
      : quoteRequestStorage;
    this.memoryQuoteRequest = null;
    requireHex32(this.csrf, 'csrf token');
  }

  _storedQuoteRequest() {
    let encoded = this.memoryQuoteRequest;
    if (this.quoteRequestStorage !== null) {
      try {
        encoded = this.quoteRequestStorage.getItem(QUOTE_REQUEST_SESSION_KEY);
      } catch (_) {
        throw new Error('durable browser quote-request storage is unavailable');
      }
    }
    return encoded === null ? null : decodeStoredQuoteRequest(encoded);
  }

  _persistQuoteRequest(body) {
    const encoded = JSON.stringify({
      schema: 'postfiat.lightning_navcoin.quote_request.v1',
      body,
    });
    if (this.quoteRequestStorage !== null) {
      try {
        this.quoteRequestStorage.setItem(QUOTE_REQUEST_SESSION_KEY, encoded);
      } catch (_) {
        throw new Error('durable browser quote-request storage is unavailable');
      }
    }
    this.memoryQuoteRequest = encoded;
  }

  clearQuoteRequest(swapId = null) {
    if (swapId === null) {
      if (this.quoteRequestStorage !== null) {
        try {
          this.quoteRequestStorage.removeItem(QUOTE_REQUEST_SESSION_KEY);
        } catch (_) {
          throw new Error('durable browser quote-request storage is unavailable');
        }
      }
      const hadRecord = this.memoryQuoteRequest !== null;
      this.memoryQuoteRequest = null;
      return hadRecord;
    }
    const stored = this._storedQuoteRequest();
    if (
      stored !== null
      && stored.client_request_id !== requireHex32(swapId, 'swap_id')
    ) {
      return false;
    }
    if (this.quoteRequestStorage !== null) {
      try {
        this.quoteRequestStorage.removeItem(QUOTE_REQUEST_SESSION_KEY);
      } catch (_) {
        throw new Error('durable browser quote-request storage is unavailable');
      }
    }
    this.memoryQuoteRequest = null;
    return stored !== null;
  }

  _durableQuoteRequest(input) {
    const requestedId = input.clientRequestId === undefined
      ? null
      : requireHex32(input.clientRequestId, 'client_request_id');
    const candidate = quoteRequestBody({
      ...input,
      clientRequestId: requestedId ?? randomHex(32),
    });
    const stored = this._storedQuoteRequest();
    if (stored !== null) {
      if (
        quoteRequestTerms(stored) !== quoteRequestTerms(candidate)
        || (requestedId !== null && stored.client_request_id !== requestedId)
      ) {
        throw new Error(
          'a different quote request is already durable; finish it or explicitly reset the run',
        );
      }
      return stored;
    }
    this._persistQuoteRequest(candidate);
    return candidate;
  }

  async _request(method, path, body) {
    if (!path.startsWith('/') || path.startsWith('//') || path.includes('\\')) {
      throw new Error('coordinator API path must be same-origin');
    }
    if (body !== undefined) assertSecretFreeCoordinatorValue(body, 'coordinator request');
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), DEFAULT_TIMEOUT_MS);
    const headers = {
      Accept: 'application/json',
      'X-PostFiat-CSRF': this.csrf,
      'X-Requested-With': 'postfiat-wallet',
    };
    if (body !== undefined) headers['Content-Type'] = 'application/json';
    try {
      const response = await this.fetchImpl(`${LIGHTNING_NAVCOIN_API_ROOT}${path}`, {
        method,
        headers,
        credentials: 'same-origin',
        cache: 'no-store',
        redirect: 'error',
        referrerPolicy: 'same-origin',
        signal: controller.signal,
        ...(body === undefined ? {} : { body: JSON.stringify(body) }),
      });
      const lengthHeader = response.headers?.get?.('content-length');
      if (lengthHeader && Number(lengthHeader) > MAX_RESPONSE_BYTES) {
        throw new Error('coordinator response exceeds the size limit');
      }
      const text = await response.text();
      if (new TextEncoder().encode(text).length > MAX_RESPONSE_BYTES) {
        throw new Error('coordinator response exceeds the size limit');
      }
      let payload;
      try {
        payload = JSON.parse(text);
      } catch (_) {
        throw new Error(`coordinator returned HTTP ${response.status} without valid JSON`);
      }
      assertSecretFreeCoordinatorValue(payload, 'coordinator response');
      if (!response.ok) {
        const message = payload?.error?.message || payload?.error || payload?.message;
        throw new Error(String(message || `coordinator returned HTTP ${response.status}`).slice(0, 512));
      }
      return payload;
    } catch (error) {
      if (error?.name === 'AbortError') throw new Error('coordinator request timed out');
      throw error;
    } finally {
      clearTimeout(timeout);
    }
  }

  async status() {
    const status = validateCoordinatorStatus(
      await this._request('GET', '/status'),
      this.releasePins,
    );
    this.releaseStatusVerified = true;
    return status;
  }

  async _validatedSwap(payload) {
    const swap = validateSwapSnapshot(payload, this.releasePins);
    if (swap.quote !== null) {
      if (!this.releaseStatusVerified) {
        throw new Error(
          'reviewed release pins must be verified against coordinator status before using a signed quote',
        );
      }
      const envelope = swap.raw.signed_quote;
      const verified = await verifySignedQuoteEnvelope(
        envelope,
        this.releasePins.quoteSignerPublicKeyHex,
      );
      const expected = canonicalQuoteBytes(swap.quote.raw);
      const actual = canonicalQuoteBytes(verified);
      if (
        expected.length !== actual.length
        || expected.some((byte, index) => byte !== actual[index])
      ) {
        throw new Error('verified signed quote differs from rendered quote');
      }
    }
    return swap;
  }

  async createQuote(input) {
    // The request id is durable before POST. A response-loss retry therefore
    // addresses the same coordinator swap/invoice and cannot create a hidden
    // second exposure.
    const body = this._durableQuoteRequest(input);
    let payload;
    try {
      payload = await this._request('POST', '/quotes', body);
    } catch (postError) {
      try {
        payload = await this._request(
          'GET',
          `/swaps/${encodeURIComponent(body.client_request_id)}`,
        );
      } catch (_) {
        throw postError;
      }
    }
    const swap = await this._validatedSwap(payload);
    if (
      swap.swapId !== body.client_request_id
      || swap.direction !== body.direction
      || swap.invoiceAmountMsat !== body.amount_msat
    ) {
      throw new Error(
        'coordinator quote substituted the request id, direction, or amount',
      );
    }
    if (swap.walletAddress !== body.wallet_address) {
      throw new Error('coordinator quote substituted the requesting wallet');
    }
    if (
      body.direction === PFTL_TO_LIGHTNING
      && swap.quote
      && swap.quote.invoice !== body.invoice
    ) {
      throw new Error('coordinator quote substituted the Phoenix invoice');
    }
    return swap;
  }

  async swap(swapId) {
    const id = requireHex32(swapId, 'swap_id');
    return this._validatedSwap(
      await this._request('GET', `/swaps/${encodeURIComponent(id)}`),
    );
  }

  async notifyPftlLock(swapId, txId) {
    const id = requireHex32(swapId, 'swap_id');
    const transaction = requireHex48(txId, 'PFTL lock tx_id');
    return this._validatedSwap(await this._request(
      'POST',
      `/swaps/${encodeURIComponent(id)}/pftl-lock`,
      { tx_id: transaction },
    ));
  }

  async notifyPftlFinish(swapId, txId) {
    const id = requireHex32(swapId, 'swap_id');
    const transaction = requireHex48(txId, 'PFTL finish tx_id');
    return this._validatedSwap(await this._request(
      'POST',
      `/swaps/${encodeURIComponent(id)}/pftl-finish`,
      { tx_id: transaction },
    ));
  }

  async notifyPftlCancel(swapId, txId) {
    const id = requireHex32(swapId, 'swap_id');
    const transaction = requireHex48(txId, 'PFTL cancel tx_id');
    return this._validatedSwap(await this._request(
      'POST',
      `/swaps/${encodeURIComponent(id)}/pftl-cancel`,
      { tx_id: transaction },
    ));
  }
}

function hexToBytes(value) {
  return Uint8Array.from(
    { length: value.length / 2 },
    (_, index) => Number.parseInt(value.slice(index * 2, index * 2 + 2), 16),
  );
}

function bytesToHex(value) {
  return Array.from(value, byte => byte.toString(16).padStart(2, '0')).join('');
}

export async function verifyPhoenixPreimage(preimageValue, expectedPaymentHash, subtle = globalThis.crypto?.subtle) {
  const preimage = String(preimageValue || '').trim();
  requireHex32(preimage, 'Phoenix payment preimage');
  const paymentHash = requireHex32(expectedPaymentHash, 'payment_hash');
  if (!subtle?.digest) throw new Error('WebCrypto SHA-256 is unavailable');
  const digest = bytesToHex(new Uint8Array(await subtle.digest('SHA-256', hexToBytes(preimage))));
  if (digest !== paymentHash) {
    throw new Error('Phoenix preimage does not match the quote payment hash');
  }
  return {
    paymentHash,
    fulfillment: `${FULFILLMENT_PREFIX}${preimage}`,
  };
}

export function buildEscrowFinishOperation(snapshot, fulfillment, walletAddress) {
  const swap = snapshot?.quote ? snapshot : validateSwapSnapshot(snapshot);
  const quote = swap.quote;
  if (quote.direction !== LIGHTNING_TO_PFTL) {
    throw new Error('only the Lightning-to-PFTL recipient locally finishes this escrow');
  }
  const recipient = requirePftlAddress(walletAddress, 'wallet address');
  if (recipient !== quote.recipient) throw new Error('wallet is not the quoted escrow recipient');
  const encoded = requireString(fulfillment, 'fulfillment', 256);
  if (!encoded.startsWith(FULFILLMENT_PREFIX) || encoded.length !== FULFILLMENT_PREFIX.length + 64) {
    throw new Error('fulfillment is not canonical PREIMAGE-SHA-256');
  }
  return {
    operation: 'escrow_finish',
    escrow_id: quote.escrowId,
    owner: quote.owner,
    recipient,
    fulfillment: encoded,
  };
}

export function buildEscrowCreateOperation(snapshot, walletAddress) {
  const swap = snapshot?.quote ? snapshot : validateSwapSnapshot(snapshot);
  const quote = swap.quote;
  if (quote.direction !== PFTL_TO_LIGHTNING) {
    throw new Error('only a PFTL-to-Lightning quote has a wallet-owned create leg');
  }
  const owner = requirePftlAddress(walletAddress, 'wallet address');
  if (owner !== quote.owner) throw new Error('wallet is not the quoted escrow owner');
  return {
    operation: 'escrow_create',
    owner,
    recipient: quote.recipient,
    asset_id: quote.assetId,
    amount: quote.amountAtoms,
    condition: quote.condition,
    finish_after: quote.finishAfter,
    cancel_after: quote.cancelAfter,
  };
}

export function assertOfframpLockExecutable(
  snapshot,
  nowUnix = Math.floor(Date.now() / 1000),
) {
  const swap = snapshot?.quote ? snapshot : validateSwapSnapshot(snapshot);
  if (swap.quote.direction !== PFTL_TO_LIGHTNING) {
    throw new Error('execution-window check applies only to a PFTL-to-Lightning quote');
  }
  requireSafeInteger(nowUnix, 'current time');
  if (swap.state !== 'PFTL_LOCK_SUBMITTED' || swap.canExecute !== true) {
    throw new Error('off-ramp is not executable before local PFTL signing');
  }
  if (
    nowUnix >= swap.quote.quoteExpires
    || nowUnix >= swap.quote.latestStart
    || nowUnix >= swap.quote.invoiceExpires
  ) {
    throw new Error('off-ramp execution window expired before local PFTL signing');
  }
  return swap;
}

export function buildEscrowCancelOperation(snapshot, walletAddress) {
  const swap = snapshot?.quote ? snapshot : validateSwapSnapshot(snapshot);
  const quote = swap.quote;
  if (quote.direction !== PFTL_TO_LIGHTNING) {
    throw new Error('only the PFTL-to-Lightning owner locally cancels this escrow');
  }
  if (swap.state !== 'REFUND_ELIGIBLE') {
    throw new Error('off-ramp escrow is not refund eligible');
  }
  const owner = requirePftlAddress(walletAddress, 'wallet address');
  if (owner !== quote.owner) throw new Error('wallet is not the quoted escrow owner');
  if (swap.pftl.height === null || swap.pftl.height < quote.cancelAfter) {
    throw new Error('PFTL cancel_after height has not finalized');
  }
  return {
    operation: 'escrow_cancel',
    escrow_id: quote.escrowId,
    owner,
  };
}

function normalizedIndependentPftlStatus(payload, label) {
  const envelope = requireRecord(payload, `${label} response`);
  if (envelope.ok !== true) {
    throw new Error(envelope.error?.message || `${label} failed`);
  }
  const value = requireRecord(envelope.result, `${label} result`);
  const status = requireString(value.status, `${label}.status`, 32).toLowerCase();
  if (!['running', 'active', 'validator'].includes(status)) {
    throw new Error(`${label} validator is not active`);
  }
  const route = {
    chainId: requireString(value.chain_id, `${label}.chain_id`, 128),
    genesisHash: requireHex48(value.genesis_hash, `${label}.genesis_hash`),
    buildGitRevision: requireString(
      value.build_git_revision,
      `${label}.build_git_revision`,
      128,
    ),
    height: requireSafeInteger(value.block_height, `${label}.block_height`),
    blockTipHash: requireHex48(
      value.block_tip_hash,
      `${label}.block_tip_hash`,
    ),
    stateRoot: requireHex48(value.state_root, `${label}.state_root`),
    validatorCount: requireSafeInteger(
      value.validator_count,
      `${label}.validator_count`,
      1,
    ),
  };
  if (
    route.chainId !== LIGHTNING_NAVCOIN_RELEASE_PINS.pftlChainId
    || route.genesisHash !== LIGHTNING_NAVCOIN_RELEASE_PINS.pftlGenesisHash
    || route.buildGitRevision
      !== LIGHTNING_NAVCOIN_RELEASE_PINS.pftlBuildGitRevision
    || route.validatorCount !== 6
  ) {
    throw new Error(
      `${label} does not match the reviewed PFTL release pins`,
    );
  }
  return route;
}

export function assertFinalizedEscrowMatches(
  snapshot,
  rpcResult,
  walletAddress,
  coordinatorStatus,
  rpcStatusBefore,
  rpcStatusAfter,
) {
  const swap = snapshot?.quote ? snapshot : validateSwapSnapshot(snapshot);
  const quote = swap.quote;
  const coordinatorRoute = coordinatorStatus?.pftl;
  if (
    !coordinatorRoute
    || coordinatorRoute.chainId
      !== LIGHTNING_NAVCOIN_RELEASE_PINS.pftlChainId
    || coordinatorRoute.genesisHash
      !== LIGHTNING_NAVCOIN_RELEASE_PINS.pftlGenesisHash
    || coordinatorRoute.assetId
      !== LIGHTNING_NAVCOIN_RELEASE_PINS.pftlAssetId
    || coordinatorRoute.buildGitRevision
      !== LIGHTNING_NAVCOIN_RELEASE_PINS.pftlBuildGitRevision
    || coordinatorRoute.navEpoch
      !== LIGHTNING_NAVCOIN_RELEASE_PINS.pftlNavEpoch
    || coordinatorRoute.navReservePacketHash
      !== LIGHTNING_NAVCOIN_RELEASE_PINS.pftlNavReservePacketHash
    || coordinatorRoute.quorum?.observed !== 6
    || coordinatorRoute.quorum?.required !== 6
    || coordinatorRoute.quorum?.validatorCount !== 6
    || coordinatorRoute.quorum?.converged !== true
    || coordinatorRoute.height === null
    || coordinatorRoute.stateRoot === null
    || coordinatorRoute.blockTipHash === null
  ) {
    throw new Error(
      'coordinator route is not a release-pinned converged PFTL view',
    );
  }
  const before = normalizedIndependentPftlStatus(
    rpcStatusBefore,
    'independent PFTL status before escrow read',
  );
  const after = normalizedIndependentPftlStatus(
    rpcStatusAfter,
    'independent PFTL status after escrow read',
  );
  for (const field of [
    'chainId',
    'genesisHash',
    'buildGitRevision',
    'height',
    'blockTipHash',
    'stateRoot',
    'validatorCount',
  ]) {
    if (before[field] !== after[field]) {
      throw new Error(
        'independent PFTL consensus view changed around the escrow read',
      );
    }
  }
  if (
    before.chainId !== coordinatorRoute.chainId
    || before.genesisHash !== coordinatorRoute.genesisHash
    || before.buildGitRevision !== coordinatorRoute.buildGitRevision
    || before.height !== coordinatorRoute.height
    || before.blockTipHash !== coordinatorRoute.blockTipHash
    || before.stateRoot !== coordinatorRoute.stateRoot
  ) {
    throw new Error(
      'independent PFTL escrow endpoint is not on the coordinator six-validator consensus view',
    );
  }
  const result = requireRecord(rpcResult, 'escrow_info response');
  if (result.ok !== true) throw new Error(result.error?.message || 'escrow_info failed');
  const info = requireRecord(result.result, 'escrow_info result');
  if (info.found !== true) throw new Error('quoted escrow is not in finalized PFTL state');
  const escrow = requireRecord(info.escrow, 'finalized escrow');
  const conditionHash = sha3_384DomainHex(ESCROW_CONDITION_HASH_DOMAIN, quote.condition);
  if (
    escrow.escrow_id !== quote.escrowId
    || escrow.owner !== quote.owner
    || escrow.recipient !== quote.recipient
    || escrow.asset_id !== quote.assetId
    || requireDecimalInteger(escrow.amount, 'finalized escrow amount', 1n) !== quote.amountAtoms
    || escrow.condition_hash !== conditionHash
    || requireSafeInteger(escrow.finish_after, 'finalized escrow finish_after') !== quote.finishAfter
    || requireSafeInteger(escrow.cancel_after, 'finalized escrow cancel_after', 1) !== quote.cancelAfter
    || escrow.state !== 'open'
  ) {
    throw new Error('finalized PFTL escrow does not match the signed quote');
  }
  const address = requirePftlAddress(walletAddress, 'wallet address');
  if (
    (quote.direction === LIGHTNING_TO_PFTL && quote.recipient !== address)
    || (quote.direction === PFTL_TO_LIGHTNING && quote.owner !== address)
  ) {
    throw new Error('finalized escrow is not bound to this wallet');
  }
  return escrow;
}

export function safeToRevealInvoice(
  status,
  swap,
  nowUnix = Math.floor(Date.now() / 1000),
  releasePins = LIGHTNING_NAVCOIN_RELEASE_PINS,
) {
  if (!status || !swap || swap.quote.direction !== LIGHTNING_TO_PFTL) return false;
  const quorum = swap.pftl.quorum;
  return Boolean(
    status.mode === 'ARMED'
    && status.canExecute
    && swap.canExecute
    && swap.state === 'PFTL_LOCK_FINAL'
    && quorum.converged
    && quorum.observed >= quorum.required
    && quorum.required === 6
    && quorum.observed === 6
    && quorum.validatorCount === 6
    && swap.pftl.receipt?.accepted === true
    && swap.pftl.receipt?.code === 'accepted'
    && releasePins.lndIdentityPubkeyHex !== null
    && status.lnd.identityPubkey
      === releasePins.lndIdentityPubkeyHex
    && swap.quote.invoicePayee === status.lnd.identityPubkey
    && swap.quote.decodedInvoice.payee === status.lnd.identityPubkey
    && status.valuationBinding?.verified === true
    && status.proofAssurance?.profile === 'multi-fetch-quorum'
    && status.proofAssurance?.consensusNativeGroth16Verification === false
    && status.pftl.chainId === swap.quote.chainId
    && status.pftl.genesisHash === swap.quote.genesisHash
    && status.pftl.assetId === swap.quote.assetId
    && status.pftl.navEpoch === swap.quote.navEpoch
    && status.pftl.navReservePacketHash === swap.quote.navReservePacketHash
    && nowUnix < swap.quote.latestStart
    && nowUnix < swap.quote.invoiceExpires,
  );
}
