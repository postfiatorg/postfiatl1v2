import { governedRouteBinding, sha3_384DomainHex } from './evm.js';

const ROUTE_PROFILE_SCHEMA = 'postfiat.vault_bridge.route_profile.v1';
const ROUTE_REPORT_SCHEMA = 'postfiat.vault_bridge.route_report.v1';
const ROUTE_PROFILE_HASH_DOMAIN = 'postfiat.vault_bridge.route_profile_hash.v1';
const RETIRED_VAULTS = new Set(['0x1a15e6103d6af4e88924f748e13b829d3948dea9']);

function object(value, label) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error(`${label} is missing`);
  }
  return value;
}

function exactString(value, label) {
  if (typeof value !== 'string' || value.length === 0) throw new Error(`${label} is missing`);
  return value;
}

function canonicalAddress(value, label) {
  const original = exactString(value, label);
  const normalized = original.toLowerCase();
  if (!/^0x[0-9a-f]{40}$/.test(normalized)) throw new Error(`${label} is not a canonical EVM address`);
  if (original !== normalized) throw new Error(`${label} is not lowercase canonical form`);
  return normalized;
}

function runtimeCodeHash(value, label) {
  const original = exactString(value, label);
  const normalized = original.toLowerCase();
  if (!/^0x[0-9a-f]{64}$/.test(normalized) || /^0x0{64}$/.test(normalized)) {
    throw new Error(`${label} is not a nonzero 32-byte runtime code hash`);
  }
  if (original !== normalized) throw new Error(`${label} is not lowercase canonical form`);
  return normalized;
}

function positiveInteger(value, label) {
  if (!Number.isSafeInteger(value) || value <= 0) throw new Error(`${label} must be a positive integer`);
  return value;
}

function nonNegativeInteger(value, label) {
  if (!Number.isSafeInteger(value) || value < 0) throw new Error(`${label} must be a nonnegative integer`);
  return value;
}

function lowerHex(value, bytes, label, prefix = false) {
  if (typeof value !== 'string') throw new Error(`${label} is missing`);
  const pattern = prefix
    ? new RegExp(`^0x[0-9a-f]{${bytes * 2}}$`)
    : new RegExp(`^[0-9a-f]{${bytes * 2}}$`);
  if (!pattern.test(value)) throw new Error(`${label} is not canonical lowercase hex`);
  return value;
}

function canonicalRouteProfile(value) {
  const profile = object(value, 'vault bridge route profile');
  if (profile.schema !== ROUTE_PROFILE_SCHEMA) throw new Error('vault bridge route profile schema mismatch');
  const routeId = exactString(profile.route_id, 'vault bridge route id');
  if (!/^[a-z0-9._-]{1,64}$/.test(routeId)) throw new Error('vault bridge route id is not canonical');
  const assetId = exactString(profile.asset_id, 'vault bridge route asset id');
  if (!/^[0-9a-f]{96}$/.test(assetId)) throw new Error('vault bridge route asset id is malformed');
  const canonical = {
    ...profile,
    source_chain_id: positiveInteger(profile.source_chain_id, 'vault bridge route source chain'),
    vault_address: canonicalAddress(profile.vault_address, 'vault bridge vault address'),
    vault_runtime_code_hash: runtimeCodeHash(profile.vault_runtime_code_hash, 'vault bridge vault code hash'),
    token_address: canonicalAddress(profile.token_address, 'vault bridge token address'),
    token_runtime_code_hash: runtimeCodeHash(profile.token_runtime_code_hash, 'vault bridge token code hash'),
    route_epoch: positiveInteger(profile.route_epoch, 'vault bridge route epoch'),
    verifier_policy_hash: profile.verifier_policy_hash ?? '',
    verifier_program_vkey: profile.verifier_program_vkey ?? '',
    verifier_proof_encoding: profile.verifier_proof_encoding ?? '',
    max_proof_bytes: nonNegativeInteger(profile.max_proof_bytes, 'vault bridge proof byte bound'),
    max_public_values_bytes: nonNegativeInteger(
      profile.max_public_values_bytes,
      'vault bridge public-values byte bound',
    ),
    max_snapshot_age_blocks: positiveInteger(profile.max_snapshot_age_blocks, 'vault bridge snapshot age bound'),
    challenge_window_blocks: positiveInteger(profile.challenge_window_blocks, 'vault bridge challenge window'),
    max_epoch_gap_blocks: positiveInteger(profile.max_epoch_gap_blocks, 'vault bridge epoch gap bound'),
    settle_deadline_blocks: positiveInteger(profile.settle_deadline_blocks, 'vault bridge settlement deadline'),
    min_challenge_bond: nonNegativeInteger(profile.min_challenge_bond, 'vault bridge challenge bond'),
    min_attestations: nonNegativeInteger(profile.min_attestations, 'vault bridge attestation threshold'),
    minimum_confirmations: nonNegativeInteger(profile.minimum_confirmations, 'vault bridge confirmation threshold'),
    activation_height: positiveInteger(profile.activation_height, 'vault bridge route activation height'),
    expires_at_height: positiveInteger(profile.expires_at_height, 'vault bridge route expiry height'),
  };
  exactString(canonical.verifier_kind, 'vault bridge verifier kind');
  exactString(canonical.evidence_tier, 'vault bridge evidence tier');
  return canonical;
}

export function vaultBridgeRouteProfileHash(value) {
  const profile = canonicalRouteProfile(value);
  const preimage = `schema=${profile.schema}\nroute_id=${profile.route_id}\nasset_id=${profile.asset_id}\nsource_chain_id=${profile.source_chain_id}\nvault_address=${profile.vault_address}\nvault_runtime_code_hash=${profile.vault_runtime_code_hash}\ntoken_address=${profile.token_address}\ntoken_runtime_code_hash=${profile.token_runtime_code_hash}\nroute_epoch=${profile.route_epoch}\nverifier_kind=${profile.verifier_kind}\nevidence_tier=${profile.evidence_tier}\nverifier_policy_hash=${profile.verifier_policy_hash}\nverifier_program_vkey=${profile.verifier_program_vkey}\nverifier_proof_encoding=${profile.verifier_proof_encoding}\nmax_proof_bytes=${profile.max_proof_bytes}\nmax_public_values_bytes=${profile.max_public_values_bytes}\nmax_snapshot_age_blocks=${profile.max_snapshot_age_blocks}\nchallenge_window_blocks=${profile.challenge_window_blocks}\nmax_epoch_gap_blocks=${profile.max_epoch_gap_blocks}\nsettle_deadline_blocks=${profile.settle_deadline_blocks}\nmin_challenge_bond=${profile.min_challenge_bond}\nmin_attestations=${profile.min_attestations}\nminimum_confirmations=${profile.minimum_confirmations}\nactivation_height=${profile.activation_height}\nexpires_at_height=${profile.expires_at_height}\n`;
  return sha3_384DomainHex(ROUTE_PROFILE_HASH_DOMAIN, preimage);
}

export function parseGovernedVaultBridgeRoute(response, expected = {}) {
  if (!response?.ok) {
    throw new Error(response?.error?.message || 'governed vault bridge route RPC failed');
  }
  const report = object(response.result, 'vault bridge route report');
  if (report.schema !== ROUTE_REPORT_SCHEMA) throw new Error('vault bridge route report schema mismatch');
  if (report.active !== true) throw new Error('vault bridge route is not active');
  if (expected.chainId && report.chain_id !== expected.chainId) throw new Error('vault bridge route chain mismatch');
  if (expected.genesisHash && report.genesis_hash !== expected.genesisHash) {
    throw new Error('vault bridge route genesis mismatch');
  }
  const currentHeight = positiveInteger(report.current_height, 'vault bridge route current height');
  const profile = canonicalRouteProfile(report.profile);
  const assetId = exactString(profile.asset_id, 'vault bridge route asset id');
  if (!/^[0-9a-f]{96}$/.test(assetId)) throw new Error('vault bridge route asset id is malformed');
  if (expected.assetId && assetId !== expected.assetId) throw new Error('vault bridge route asset mismatch');
  const sourceChainId = positiveInteger(profile.source_chain_id, 'vault bridge route source chain');
  if (expected.sourceChainId && sourceChainId !== expected.sourceChainId) {
    throw new Error('vault bridge route source chain mismatch');
  }
  const vaultAddress = canonicalAddress(profile.vault_address, 'vault bridge vault address');
  if (RETIRED_VAULTS.has(vaultAddress)) throw new Error('vault bridge route identifies a retired vault');
  const tokenAddress = canonicalAddress(profile.token_address, 'vault bridge token address');
  if (expected.tokenAddress && tokenAddress !== expected.tokenAddress.toLowerCase()) {
    throw new Error('vault bridge route token mismatch');
  }
  const profileHash = exactString(report.profile_hash, 'vault bridge route profile hash');
  if (!/^[0-9a-f]{96}$/.test(profileHash)) throw new Error('vault bridge route profile hash is malformed');
  if (vaultBridgeRouteProfileHash(profile) !== profileHash) {
    throw new Error('vault bridge route profile preimage does not match its authenticated hash');
  }
  if (report.nav_profile_policy_hash !== profileHash) {
    throw new Error('vault bridge route NAV policy hash mismatch');
  }
  const routeEpoch = positiveInteger(profile.route_epoch, 'vault bridge route epoch');
  if (report.governance_route_epoch !== routeEpoch) throw new Error('vault bridge route epoch mismatch');
  const routeBinding = exactString(report.route_binding, 'vault bridge route binding').toLowerCase();
  if (!/^[0-9a-f]{64}$/.test(routeBinding)) throw new Error('vault bridge route binding is malformed');
  if (`0x${routeBinding}` !== governedRouteBinding(profileHash, routeEpoch)) {
    throw new Error('vault bridge route binding does not match profile hash and epoch');
  }
  const activationHeight = profile.activation_height;
  const expiresAtHeight = profile.expires_at_height;
  if (activationHeight > currentHeight || expiresAtHeight <= currentHeight) {
    throw new Error('vault bridge route is inactive or expired at the reported height');
  }
  const verifierKind = profile.verifier_kind;
  const evidenceTier = profile.evidence_tier;
  const expectedTier = verifierKind === 'multi-fetch-quorum'
    ? 'independently-observed'
    : verifierKind === 'sp1-groth16'
      ? 'receipt-proven'
      : '';
  if (!expectedTier || evidenceTier !== expectedTier) {
    throw new Error('vault bridge route evidence tier does not match its verifier');
  }
  if (verifierKind === 'multi-fetch-quorum'
      && (profile.min_attestations === 0 || profile.minimum_confirmations === 0)) {
    throw new Error('independently observed route has an invalid observer threshold');
  }
  if (verifierKind === 'multi-fetch-quorum'
      && (profile.verifier_policy_hash !== ''
        || profile.verifier_program_vkey !== ''
        || profile.verifier_proof_encoding !== ''
        || profile.max_proof_bytes !== 0
        || profile.max_public_values_bytes !== 0)) {
    throw new Error('independently observed route must not carry proof-verifier fields');
  }
  if (verifierKind === 'sp1-groth16') {
    if (profile.min_attestations !== 0 || profile.minimum_confirmations !== 0) {
      throw new Error('receipt-proven route must not require observer attestations or confirmations');
    }
    lowerHex(profile.verifier_policy_hash, 32, 'vault bridge verifier policy');
    lowerHex(profile.verifier_program_vkey, 32, 'vault bridge verifier key', true);
    if (profile.verifier_proof_encoding !== 'groth16') {
      throw new Error('receipt-proven route proof encoding mismatch');
    }
  }
  if (report.nav_profile_verifier_kind !== verifierKind) {
    throw new Error('vault bridge route NAV verifier mismatch');
  }
  return Object.freeze({
    report,
    profile: Object.freeze(profile),
    profileHash,
    routeEpoch,
    routeBinding: `0x${routeBinding}`,
    currentHeight,
    expiresAtHeight,
    remainingBlocks: expiresAtHeight - currentHeight,
    vaultAddress,
    vaultRuntimeCodeHash: runtimeCodeHash(profile.vault_runtime_code_hash, 'vault bridge vault code hash'),
    tokenAddress,
    tokenRuntimeCodeHash: runtimeCodeHash(profile.token_runtime_code_hash, 'vault bridge token code hash'),
    evidenceTier,
    verifierKind,
  });
}

export async function loadGovernedVaultBridgeRoute(rpc, expected) {
  if (!rpc || typeof rpc.vaultBridgeRoute !== 'function') {
    throw new Error('wallet RPC does not support governed vault bridge route discovery');
  }
  return parseGovernedVaultBridgeRoute(await rpc.vaultBridgeRoute(expected.assetId), expected);
}
