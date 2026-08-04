import assert from 'node:assert/strict';
import { createHash, randomBytes } from 'node:crypto';
import { readFile, stat, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

import { chromium } from 'playwright';

import {
  loadNavcoinExportReadiness,
  waitForNavcoinExportJob,
} from './navcoin-export-relay.js';
import {
  loadNavcoinReturnReadiness,
  waitForNavcoinReturnJob,
} from './navcoin-return-relay.js';
import {
  loadPftlPrivateReadiness,
  loadPftlPrivateRecoveries,
  pftlPrivateRecoveryKey,
} from './pftl-private-primary.js';

const CANDIDATE_REVISION = '39f7fae3191aa34c376ae1657650a9ec2444f421';
const ASYNC_PROOF_SLOT_SCHEMA = 'postfiat.a666.r4.async-proof-slot.v1';
const ASYNC_PROOF_RUN_ID = 'a666-r4-receipt-prover-pathb-20260804-v3';
const DRY_RUN_STEPS = Object.freeze([1, 2, 3, 4, 5, 6, 9, 10]);
const POLL_INTERVAL_MS = 5_000;
const MAX_ASYNC_WAIT_MS = 12 * 60 * 60 * 1_000;
const FORBIDDEN_FIELD = /seed|mnemonic|private[_-]?key|owner[_-]?key|spend[_-]?auth/i;
const WALLET_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const PRODUCTION_COMPONENT_GRAPH = Object.freeze([
  join(WALLET_ROOT, 'src/App.jsx'),
  join(WALLET_ROOT, 'src/components/Swap.jsx'),
  join(WALLET_ROOT, 'src/components/NavcoinMarket.jsx'),
  join(WALLET_ROOT, 'src/components/NavcoinPrimaryMarket.jsx'),
  join(WALLET_ROOT, 'src/components/PftlPrivatePrimary.jsx'),
]);
const JOURNEY_LABELS = Object.freeze([
  'build identity',
  'browser-controlled connect/create',
  'six sources/proofs/NAV',
  'A666+pfUSDC balances',
  'transparent issue/redeem',
  'private issue/redeem',
  'export',
  'return',
  'restart+reload recovery',
  'public receipt download',
]);
const PRODUCTION_MODULES = Object.freeze({
  loadNavcoinExportReadiness,
  waitForNavcoinExportJob,
  loadNavcoinReturnReadiness,
  waitForNavcoinReturnJob,
  loadPftlPrivateReadiness,
  loadPftlPrivateRecoveries,
  pftlPrivateRecoveryKey,
});

function requiredPath(name) {
  const value = String(process.env[name] || '').trim();
  if (!value) throw new Error(`${name} is required`);
  return resolve(value);
}

function requiredLoopbackOrigin(environmentName, label) {
  const raw = String(process.env[environmentName] || '').trim();
  if (!raw) throw new Error(`${environmentName} is required`);
  const origin = new URL(raw);
  assert.equal(origin.protocol, 'http:', `${label} must use HTTP on loopback`);
  assert.ok(['127.0.0.1', 'localhost', '::1'].includes(origin.hostname), `${label} must be loopback`);
  assert.ok(origin.port, `${label} must include an explicit port`);
  assert.equal(origin.username, '', `${label} must not contain credentials`);
  assert.equal(origin.password, '', `${label} must not contain credentials`);
  return origin;
}

function requiredAsyncBudget() {
  const value = Number(process.env.POSTFIAT_R4_ASYNC_MAX_WAIT_MS);
  assert.ok(Number.isSafeInteger(value) && value > 0, 'POSTFIAT_R4_ASYNC_MAX_WAIT_MS is required');
  assert.ok(value <= MAX_ASYNC_WAIT_MS, 'asynchronous artifact wait exceeds 12 hours');
  return value;
}

async function readJson(path, label) {
  const parsed = JSON.parse(await readFile(path, 'utf8'));
  assert.ok(parsed && typeof parsed === 'object' && !Array.isArray(parsed), `${label} must be a JSON object`);
  return parsed;
}

function manifestCandidateRevision(manifest) {
  return manifest.candidate?.revision
    ?? manifest.candidate_revision
    ?? manifest.aggregation?.current_binding?.candidate_revision
    ?? manifest.bindings?.candidate_revision
    ?? null;
}

async function manifestHashObservation(path, environmentName) {
  const expected = String(process.env[environmentName] || '').trim().toLowerCase();
  const actual = await sha256File(path);
  return {
    path,
    expected_sha256: expected || null,
    actual_sha256: actual,
    matches: /^[0-9a-f]{64}$/.test(expected) && expected === actual,
  };
}

async function readableJsonObservation(path, label) {
  try {
    const value = await readJson(path, label);
    return { path, readable: true, value };
  } catch (error) {
    return { path, readable: false, error: error.code || error.message };
  }
}

function endpointStrings(value, path = 'manifest', output = []) {
  if (Array.isArray(value)) {
    value.forEach((item, index) => endpointStrings(item, `${path}[${index}]`, output));
    return output;
  }
  if (!value || typeof value !== 'object') return output;
  for (const [key, item] of Object.entries(value)) {
    const nextPath = `${path}.${key}`;
    if (typeof item === 'string' && /(url|origin|endpoint|rpc|host)$/i.test(key)) {
      output.push({ path: nextPath, value: item });
    }
    endpointStrings(item, nextPath, output);
  }
  return output;
}

function assertLoopbackManifest(label, manifest) {
  const endpoints = endpointStrings(manifest);
  assert.ok(endpoints.length > 0, `${label} exposes no endpoint bindings`);
  for (const endpoint of endpoints) {
    if (endpoint.value === '127.0.0.1' || endpoint.value === 'localhost' || endpoint.value === '::1') continue;
    let parsed;
    try {
      parsed = new URL(endpoint.value);
    } catch {
      if (/^127\.0\.0\.1:\d+$/.test(endpoint.value)) continue;
      throw new Error(`${endpoint.path} is not an explicit loopback endpoint`);
    }
    assert.ok(['127.0.0.1', 'localhost', '::1'].includes(parsed.hostname), `${endpoint.path} is not loopback`);
    assert.equal(parsed.username, '', `${endpoint.path} contains credentials`);
    assert.equal(parsed.password, '', `${endpoint.path} contains credentials`);
  }
}

function loopbackManifestObservation(label, manifest) {
  try {
    assertLoopbackManifest(label, manifest);
    return { label, valid: true, error: null };
  } catch (error) {
    return { label, valid: false, error: error.message };
  }
}

function loopbackEndpointsObservation(label, value) {
  try {
    const endpoints = endpointStrings(value);
    for (const endpoint of endpoints) {
      if (endpoint.value === '127.0.0.1' || endpoint.value === 'localhost' || endpoint.value === '::1') continue;
      let parsed;
      try {
        parsed = new URL(endpoint.value);
      } catch {
        if (/^127\.0\.0\.1:\d+$/.test(endpoint.value)) continue;
        throw new Error(`${endpoint.path} is not an explicit loopback endpoint`);
      }
      assert.ok(['127.0.0.1', 'localhost', '::1'].includes(parsed.hostname), `${endpoint.path} is not loopback`);
      assert.equal(parsed.username, '', `${endpoint.path} contains credentials`);
      assert.equal(parsed.password, '', `${endpoint.path} contains credentials`);
    }
    return { label, valid: true, endpoint_count: endpoints.length, error: null };
  } catch (error) {
    return { label, valid: false, error: error.message };
  }
}

function resolveRunContract(preflightOnly, classification) {
  // Two-mode classification contract. The preflight branch accepts only
  // construction; the full branch accepts only an explicit official
  // classification. Every other combination refuses before any browser
  // launch or business mutation.
  if (preflightOnly) {
    assert.equal(classification, 'construction',
      'preflight-only mode requires construction classification');
    return { mode: 'preflight_only', classification };
  }
  assert.equal(classification, 'official',
    'full execution requires explicit official classification (POSTFIAT_R4_RUN_CLASSIFICATION=official)');
  return { mode: 'official', classification };
}

function loopbackHref(value) {
  try {
    return ['127.0.0.1', 'localhost', '::1'].includes(new URL(String(value)).hostname);
  } catch {
    return false;
  }
}

function officialLaunchContract(inputs) {
  // Computed complete-input contract for official mode. Pure and
  // deterministic: it inspects only the supplied input map, launches no
  // browser, and issues no business mutation. Every official input must be
  // present and well-shaped; nothing is inferred and no preflight-only
  // default is reused.
  const requiredInputs = [
    'fireControlPath', 'fireControlSha256',
    'setupPath', 'setupSha256',
    'deploymentPath', 'deploymentSha256',
    'walletOrigin', 'proxyOrigin', 'asyncMaxWaitMs',
    'exportSlotPath', 'returnSlotPath',
    'proxyRestartPath', 'proxyPidPath', 'reportPath',
    'fireControlReadyToFire',
  ];
  const missingInputs = requiredInputs
    .filter(name => inputs[name] === undefined || inputs[name] === null || inputs[name] === '');
  const checks = {
    fire_control_ready_to_fire: inputs.fireControlReadyToFire === true,
    fire_control_hash_pinned: /^[0-9a-f]{64}$/.test(String(inputs.fireControlSha256 || '')),
    setup_hash_pinned: /^[0-9a-f]{64}$/.test(String(inputs.setupSha256 || '')),
    deployment_hash_pinned: /^[0-9a-f]{64}$/.test(String(inputs.deploymentSha256 || '')),
    wallet_origin_loopback: loopbackHref(inputs.walletOrigin),
    proxy_origin_loopback: loopbackHref(inputs.proxyOrigin),
    async_budget_bounded: Number.isSafeInteger(inputs.asyncMaxWaitMs)
      && inputs.asyncMaxWaitMs > 0 && inputs.asyncMaxWaitMs <= MAX_ASYNC_WAIT_MS,
    distinct_artifact_slots: Boolean(inputs.exportSlotPath)
      && Boolean(inputs.returnSlotPath)
      && inputs.exportSlotPath !== inputs.returnSlotPath,
  };
  const failedChecks = Object.entries(checks).filter(([, ok]) => !ok).map(([name]) => name);
  return {
    ready_to_launch: missingInputs.length === 0 && failedChecks.length === 0,
    missing_inputs: missingInputs,
    failed_checks: failedChecks,
  };
}

function fallbackFields(label, value, path = label, output = []) {
  if (Array.isArray(value)) {
    value.forEach((item, index) => fallbackFields(label, item, `${path}[${index}]`, output));
    return output;
  }
  if (!value || typeof value !== 'object') return output;
  for (const [key, item] of Object.entries(value)) {
    const nextPath = `${path}.${key}`;
    if (/fallback/i.test(key)) output.push({ path: nextPath, value: Boolean(item) });
    fallbackFields(label, item, nextPath, output);
  }
  return output;
}

function observedStep(step, observations, predicates) {
  assert.equal(JOURNEY_LABELS[step - 1], observations.label, `step ${step} label drift`);
  const checks = Object.fromEntries(Object.entries(predicates).map(([name, predicate]) => [name, Boolean(predicate)]));
  return {
    step,
    label: observations.label,
    observations,
    checks,
    ok: Object.values(checks).length > 0 && Object.values(checks).every(Boolean),
  };
}

function scanForbiddenFields(value, path = 'artifact') {
  if (Array.isArray(value)) {
    value.forEach((item, index) => scanForbiddenFields(item, `${path}[${index}]`));
    return;
  }
  if (!value || typeof value !== 'object') return;
  for (const [key, item] of Object.entries(value)) {
    assert.equal(FORBIDDEN_FIELD.test(key), false, `forbidden custody field ${path}.${key}`);
    scanForbiddenFields(item, `${path}.${key}`);
  }
}

function assertCustodyValuesAbsent(label, value, custodyValues) {
  const serialized = typeof value === 'string' ? value : JSON.stringify(value);
  for (const custodyValue of custodyValues) {
    assert.equal(serialized.includes(custodyValue), false, `${label} contains browser custody material`);
  }
}

async function sha256File(path) {
  return createHash('sha256').update(await readFile(path)).digest('hex');
}

async function pollContentAddressedSlot(path, kind, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let last = null;
  while (Date.now() < deadline) {
    try {
      last = await readJson(path, `${kind} artifact slot`);
      if (last.status === 'accepted') {
        assert.match(String(last.artifact_sha256 || ''), /^[0-9a-f]{64}$/);
        assert.match(String(last.receipt_identity || ''), /^[A-Za-z0-9._:-]{8,256}$/);
        assert.ok(last.observed_chain_state && Number.isSafeInteger(last.observed_chain_state.height));
        assert.match(String(last.observed_chain_state.tip || ''), /^[0-9a-f]{64,96}$/);
        assert.match(String(last.observed_chain_state.state_root || ''), /^[0-9a-f]{64,96}$/);
        assert.equal(last.observed_chain_state.finalized, true);
        const artifactPath = resolve(String(last.artifact_path || ''));
        assert.equal(await sha256File(artifactPath), last.artifact_sha256, `${kind} artifact hash mismatch`);
        return last;
      }
      if (last.status === 'failed') throw new Error(`${kind} relay failed: ${last.failure_code || 'unknown'}`);
    } catch (error) {
      if (error?.code !== 'ENOENT') throw error;
    }
    await new Promise(resolveWait => setTimeout(resolveWait, POLL_INTERVAL_MS));
  }
  const error = new Error(`${kind} content-addressed proof/relay slot timed out`);
  error.code = `a666_r4_${kind}_slot_timeout`;
  error.lastObserved = last;
  throw error;
}

async function verifyProductionGraph() {
  const [app, swap, marketAdapter, market, privatePrimary] = await Promise.all(
    PRODUCTION_COMPONENT_GRAPH.map(path => readFile(path, 'utf8')),
  );
  const checks = {
    app_renders_swap: app.includes("import Swap from './components/Swap.jsx'"),
    app_renders_market_adapter: app.includes("import NavcoinMarket from './components/NavcoinMarket.jsx'"),
    swap_renders_private_primary: swap.includes("import PftlPrivatePrimary from './PftlPrivatePrimary.jsx'"),
    adapter_renders_primary_market: marketAdapter.includes("import PftlUniswapPrimaryMarket from './NavcoinPrimaryMarket.jsx'"),
    primary_uses_export_relay: market.includes("from '../lib/navcoin-export-relay.js'"),
    primary_uses_return_relay: market.includes("from '../lib/navcoin-return-relay.js'"),
    private_uses_production_library: privatePrimary.includes("from '../lib/pftl-private-primary.js'"),
    private_renders_public_receipt_control: privatePrimary.includes('Download public receipt'),
  };
  assert.ok(Object.values(checks).every(Boolean), 'production rendered component import graph changed');
  assert.ok(Object.values(PRODUCTION_MODULES).every(value => typeof value === 'function'));
  return checks;
}

async function runConstructionPreflight({
  walletOrigin, proxyOrigin, fireControl, setup, deployment, productionGraph,
  fireControlPath, setupPath, deploymentPath,
  exportSlotPath, returnSlotPath, asyncProofSlotPath, proxyRestartPath, proxyPidPath, reportPath,
}) {
  const [fireControlHash, setupHash, deploymentHash, exportSlot, returnSlot, asyncProofSlot] = await Promise.all([
    manifestHashObservation(fireControlPath, 'POSTFIAT_R4_FIRE_CONTROL_MANIFEST_SHA256'),
    manifestHashObservation(setupPath, 'POSTFIAT_R4_SETUP_MANIFEST_SHA256'),
    manifestHashObservation(deploymentPath, 'POSTFIAT_R4_DEPLOYMENT_MANIFEST_SHA256'),
    readableJsonObservation(exportSlotPath, 'export artifact slot'),
    readableJsonObservation(returnSlotPath, 'return artifact slot'),
    readableJsonObservation(asyncProofSlotPath, 'async proof slot'),
  ]);
  let walletStatus = null;
  let walletServed = false;
  try {
    const response = await fetch(walletOrigin, { cache: 'no-store' });
    walletStatus = response.status;
    walletServed = response.ok;
  } catch (error) {
    walletStatus = error.code || error.message;
  }
  let proxyStatus = null;
  let proxyReachable = false;
  try {
    const response = await fetch(new URL('/healthz', proxyOrigin), { cache: 'no-store' });
    proxyStatus = response.status;
    proxyReachable = response.ok;
  } catch (error) {
    proxyStatus = error.code || error.message;
  }
  const proxyPidRaw = (await readFile(proxyPidPath, 'utf8').catch(() => '')).trim();
  let proxyPid = Number(proxyPidRaw);
  let proxyPidSource = 'numeric_file';
  if (!(Number.isSafeInteger(proxyPid) && proxyPid > 0)) {
    proxyPidSource = 'unreadable';
    try {
      const readyDocument = JSON.parse(proxyPidRaw);
      const readyPid = Number(readyDocument?.pid);
      if (Number.isSafeInteger(readyPid) && readyPid > 0) {
        proxyPid = readyPid;
        proxyPidSource = 'proxy_ready_json';
      }
    } catch { /* not a proxy-ready JSON document */ }
  }
  const proxyPidReadable = Number.isSafeInteger(proxyPid) && proxyPid > 0;
  const restartDirectoryReadable = await stat(dirname(proxyRestartPath))
    .then(value => value.isDirectory())
    .catch(() => false);
  const manifestHashes = [fireControlHash, setupHash, deploymentHash];
  const loopbackManifests = [
    loopbackManifestObservation('setup manifest', setup),
    loopbackManifestObservation('deployment manifest', deployment),
  ];
  const candidatesMatch = [fireControl, setup, deployment]
    .every(manifest => manifestCandidateRevision(manifest) === CANDIDATE_REVISION);
  const importGraphReady = Object.values(productionGraph).every(Boolean);
  const slotAccepted = slot => slot.readable
    && slot.value?.status === 'accepted'
    && /^[0-9a-f]{64}$/.test(String(slot.value?.artifact_sha256 || ''))
    && Boolean(slot.value?.receipt_identity)
    && slot.value?.observed_chain_state?.finalized === true;
  const asyncProofChecks = {
    readable: asyncProofSlot.readable,
    schema: asyncProofSlot.value?.schema === ASYNC_PROOF_SLOT_SCHEMA,
    status_accepted: asyncProofSlot.value?.status === 'accepted',
    run_id: asyncProofSlot.value?.run_id === ASYNC_PROOF_RUN_ID,
    export_slot_accepted: asyncProofSlot.value?.export_slot_accepted === true,
    first_blocker_null: asyncProofSlot.value?.first_blocker === null,
    inline_proving_forbidden: asyncProofSlot.value?.inline_proving_forbidden === true,
    prover_invocations_zero: asyncProofSlot.value?.prover_invocations === 0,
    business_mutations_zero: asyncProofSlot.value?.business_mutations === 0,
  };
  const asyncProofSlotAccepted = Object.values(asyncProofChecks).every(Boolean);
  const exportArtifactHashValid = slotAccepted(exportSlot) && Boolean(exportSlot.value?.artifact_path)
    && await sha256File(resolve(String(exportSlot.value.artifact_path)))
      .then(hash => hash === exportSlot.value.artifact_sha256)
      .catch(() => false);
  const slotEndpointManifests = [
    loopbackEndpointsObservation('async proof slot', asyncProofSlot.value),
    loopbackEndpointsObservation('export artifact slot', exportSlot.value),
  ];
  const hashesReady = manifestHashes.every(item => item.matches);
  const loopbackReady = loopbackManifests.every(item => item.valid);
  const buildReady = walletServed
    && manifestCandidateRevision(setup) === CANDIDATE_REVISION
    && setupHash.matches
    && importGraphReady;
  const renderedWalletReady = walletServed && importGraphReady;
  const displayReady = walletServed && candidatesMatch && hashesReady && loopbackReady && importGraphReady;
  const mutationReady = displayReady && proxyReachable;
  const readiness = [
    { step: 1, label: JOURNEY_LABELS[0], ready: buildReady },
    { step: 2, label: JOURNEY_LABELS[1], ready: renderedWalletReady },
    { step: 3, label: JOURNEY_LABELS[2], ready: displayReady },
    { step: 4, label: JOURNEY_LABELS[3], ready: displayReady },
    { step: 5, label: JOURNEY_LABELS[4], ready: mutationReady },
    { step: 6, label: JOURNEY_LABELS[5], ready: mutationReady },
    { step: 7, label: JOURNEY_LABELS[6], ready: mutationReady && slotAccepted(exportSlot) },
    { step: 8, label: JOURNEY_LABELS[7], ready: mutationReady && slotAccepted(returnSlot) },
    { step: 9, label: JOURNEY_LABELS[8], ready: walletServed && proxyReachable && proxyPidReadable && restartDirectoryReadable },
    { step: 10, label: JOURNEY_LABELS[9], ready: walletServed && importGraphReady && productionGraph.private_renders_public_receipt_control },
  ].map(item => ({ ...item, executed: false }));
  const mutationRequests = [];
  const loopbackOnly = loopbackManifests.every(item => item.valid)
    && slotEndpointManifests.every(item => item.valid)
    && ['127.0.0.1', 'localhost', '::1'].includes(walletOrigin.hostname)
    && ['127.0.0.1', 'localhost', '::1'].includes(proxyOrigin.hostname);
  const fallbackConfigurations = [
    ...fallbackFields('fire_control', fireControl),
    ...fallbackFields('setup_manifest', setup),
    ...fallbackFields('deployment_manifest', deployment),
    ...fallbackFields('async_proof_slot', asyncProofSlot.value),
    ...fallbackFields('export_artifact_slot', exportSlot.value),
  ];
  const liveChainObservations = {
    fire_control_no_live_chain_dependency: Array.isArray(fireControl.checks)
      ? fireControl.checks.find(check => check?.id === 'no_live_chain_dependency')?.ok ?? null
      : null,
    async_proof_slot_live_chain: asyncProofSlot.value?.live_chain ?? null,
    export_slot_live_chain: exportSlot.value?.live_chain ?? null,
    export_slot_stakehub_interaction: exportSlot.value?.stakehub_interaction ?? null,
    fallback_configurations: fallbackConfigurations,
  };
  const computedChecks = {
    steps_1_6_9_10_dry_run: DRY_RUN_STEPS
      .every(step => readiness.find(item => item.step === step)?.ready === true),
    async_proof_slot_steps_7_8: asyncProofSlotAccepted && exportArtifactHashValid,
    loopback_only: loopbackOnly,
    no_live_chain: loopbackOnly
      && liveChainObservations.async_proof_slot_live_chain !== true
      && liveChainObservations.export_slot_live_chain === false
      && liveChainObservations.export_slot_stakehub_interaction !== true
      && fallbackConfigurations.every(field => !field.value),
    zero_business_mutations: mutationRequests.length === 0
      && readiness.every(item => item.executed === false)
      && asyncProofSlot.value?.business_mutations === 0
      && exportSlot.value?.business_mutations === 0,
  };
  const blockers = [
    ...(!walletServed ? ['candidate_wallet_origin_unreachable'] : []),
    ...(!proxyReachable ? ['candidate_proxy_unreachable'] : []),
    ...(!candidatesMatch ? ['candidate_revision_mismatch'] : []),
    ...(!importGraphReady ? ['production_import_graph_mismatch'] : []),
    ...manifestHashes.filter(item => !item.matches).map(item => `manifest_hash_unverified:${item.path}`),
    ...loopbackManifests.filter(item => !item.valid).map(item => `loopback_manifest_invalid:${item.label}`),
    ...(!exportSlot.readable ? ['export_artifact_slot_unreadable'] : []),
    ...(!returnSlot.readable ? ['return_artifact_slot_unreadable'] : []),
    ...(exportSlot.readable && !slotAccepted(exportSlot) ? ['export_artifact_slot_pending'] : []),
    ...(returnSlot.readable && !slotAccepted(returnSlot) ? ['return_artifact_slot_pending'] : []),
    ...(slotAccepted(exportSlot) && !exportArtifactHashValid ? ['export_artifact_hash_mismatch'] : []),
    ...(!asyncProofChecks.readable ? ['async_proof_slot_unreadable'] : []),
    ...(asyncProofChecks.readable && !asyncProofChecks.schema ? ['async_proof_slot_schema_mismatch'] : []),
    ...(asyncProofChecks.readable && !asyncProofChecks.status_accepted ? ['async_proof_slot_pending'] : []),
    ...(asyncProofChecks.readable && !asyncProofChecks.run_id ? ['async_proof_slot_run_id_mismatch'] : []),
    ...(asyncProofChecks.readable && !asyncProofChecks.export_slot_accepted ? ['async_proof_slot_export_not_accepted'] : []),
    ...(asyncProofChecks.readable && !asyncProofChecks.first_blocker_null ? ['async_proof_slot_first_blocker_present'] : []),
    ...(asyncProofChecks.readable && !asyncProofChecks.inline_proving_forbidden ? ['async_proof_slot_inline_proving_not_forbidden'] : []),
    ...(asyncProofChecks.readable && !asyncProofChecks.prover_invocations_zero ? ['async_proof_slot_prover_invocations_nonzero'] : []),
    ...(asyncProofChecks.readable && !asyncProofChecks.business_mutations_zero ? ['async_proof_slot_business_mutations_nonzero'] : []),
    ...slotEndpointManifests.filter(item => !item.valid).map(item => `loopback_slot_invalid:${item.label}`),
    ...(fallbackConfigurations.some(field => Boolean(field.value)) ? ['live_chain_fallback_configured'] : []),
    ...(!proxyPidReadable ? ['proxy_pid_unreadable'] : []),
    ...(!restartDirectoryReadable ? ['proxy_restart_control_unreadable'] : []),
    ...(fireControl.ready_to_fire === true ? [] : (fireControl.blocker_names ?? ['fire_control_red'])),
  ];
  const report = {
    schema: 'postfiat.a666.r4.offline-ethereum-browser-preflight.v1',
    mode: 'preflight_only',
    candidate_revision: CANDIDATE_REVISION,
    official_pass: readiness.every(item => item.executed && item.ready),
    business_mutations: mutationRequests.length,
    wallet_business_steps_executed: readiness.filter(item => item.executed).length,
    ready_to_fire: readiness.every(item => item.ready) && fireControl.ready_to_fire === true,
    fire_control_ready_observed: fireControl.ready_to_fire === true,
    wallet_origin: {
      value: walletOrigin.href,
      loopback: ['127.0.0.1', 'localhost', '::1'].includes(walletOrigin.hostname),
      served: walletServed,
      status: walletStatus,
    },
    proxy: {
      origin: proxyOrigin.href,
      loopback: ['127.0.0.1', 'localhost', '::1'].includes(proxyOrigin.hostname),
      reachable: proxyReachable,
      status: proxyStatus,
      pid_readable: proxyPidReadable,
      pid_source: proxyPidSource,
    },
    manifest_hashes: manifestHashes,
    loopback_manifests: loopbackManifests,
    production_import_graph: productionGraph,
    artifact_slots: {
      export: {
        path: exportSlot.path,
        readable: exportSlot.readable,
        accepted: slotAccepted(exportSlot),
        artifact_hash_verified: exportArtifactHashValid,
      },
      return: { path: returnSlot.path, readable: returnSlot.readable, accepted: slotAccepted(returnSlot) },
    },
    async_proof_slot: {
      path: asyncProofSlot.path,
      expected_run_id: ASYNC_PROOF_RUN_ID,
      checks: asyncProofChecks,
      accepted: asyncProofSlotAccepted,
    },
    live_chain_observations: liveChainObservations,
    computed_checks: computedChecks,
    step_readiness: readiness,
    blockers: [...new Set(blockers)],
    first_blocker: blockers[0] ?? null,
  };
  scanForbiddenFields(report);
  await writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`, { flag: 'wx', mode: 0o600 });
  assert.equal(report.official_pass, false, 'preflight-only cannot report an official pass');
  assert.equal(report.business_mutations, 0, 'preflight-only cannot issue business mutations');
  assert.ok(readiness.every(item => item.executed === false), 'preflight-only cannot execute wallet business steps');
  return report;
}

async function createBrowserControlledWallet(page, passphrase) {
  await page.getByRole('button', { name: 'Create Wallet', exact: true }).click();
  const seedDisplay = page.locator('.pf-seed-display');
  await seedDisplay.waitFor({ state: 'visible' });
  const seed = String(await seedDisplay.textContent() || '').trim();
  assert.match(seed, /^[0-9a-f]{64}$/);
  await page.locator('.pf-checkbox input').check();
  await page.locator('input[placeholder="Encryption passphrase (min 10 chars)"]').fill(passphrase);
  await page.locator('input[placeholder="Confirm passphrase"]').fill(passphrase);
  await page.getByRole('button', { name: 'Create Wallet', exact: true }).last().click();
  await page.locator('.pf-shell').waitFor();
  const address = await page.evaluate(() => new Promise((resolveAddress, rejectAddress) => {
    const request = indexedDB.open('postfiat-wallet');
    request.onerror = () => rejectAddress(request.error);
    request.onsuccess = () => {
      const database = request.result;
      const transaction = database.transaction('vaults', 'readonly');
      const record = transaction.objectStore('vaults').get('default');
      record.onerror = () => rejectAddress(record.error);
      record.onsuccess = () => {
        const result = record.result?.metadata?.address || '';
        database.close();
        resolveAddress(result);
      };
    };
  }));
  assert.match(String(address), /^pf[0-9a-f]{40}$/);
  return { seed, address: String(address) };
}

async function unlock(page, passphrase) {
  const lock = page.getByText('Wallet locked', { exact: true });
  if (await lock.isVisible().catch(() => false)) {
    await page.locator('input[placeholder="Passphrase"]').fill(passphrase);
    await page.getByRole('button', { name: 'Unlock', exact: true }).click();
  }
  await page.locator('.pf-shell').waitFor();
}

async function openPrimaryMarket(page) {
  await page.locator('.pf-sidebar .pf-nav').filter({ hasText: 'NAV Markets' }).click();
  await page.locator('[data-testid="navcoin-market"]').waitFor();
  return page.locator('[data-testid="navcoin-market"]');
}

async function waitForReserveProofFrame(frames) {
  const deadline = Date.now() + 30_000;
  while (Date.now() < deadline) {
    const match = [...frames].reverse().find(value => {
      const packets = value?.result?.packets ?? value?.result?.result?.packets;
      return Array.isArray(packets);
    });
    if (match) return match;
    await new Promise(resolveWait => setTimeout(resolveWait, 100));
  }
  throw new Error('production wallet did not observe the reserve-proof RPC response');
}

async function marketBalances(market) {
  const rows = await market.locator('.navcoin-primary-balance').allTextContents();
  const observed = Object.fromEntries(rows.map(row => {
    const normalized = row.replace(/\s+/g, ' ').trim();
    const split = normalized.search(/[0-9]/);
    return [normalized.slice(0, split).trim(), normalized.slice(split).trim()];
  }));
  assert.ok(Object.keys(observed).some(key => /pfUSDC/i.test(key)));
  assert.ok(Object.keys(observed).some(key => /A666/i.test(key)));
  return observed;
}

async function completeTransparentRoundTrip(page, market) {
  const before = await marketBalances(market);
  await market.getByRole('button', { name: /Mint A666/ }).first().click();
  await market.locator('#navcoin-amount').fill('1');
  await market.getByRole('button', { name: 'Keep on PFTL', exact: true }).click();
  await market.getByRole('button', { name: /^Mint 1(?:\.0+)? A666$/ }).click();
  await market.getByText('A666 purchase complete', { exact: true }).waitFor();

  await market.getByRole('button', { name: 'Redeem', exact: true }).first().click();
  await market.getByRole('button', { name: 'From PFTL', exact: true }).click();
  await market.locator('#navcoin-amount').fill('1');
  await market.getByRole('button', { name: /^Redeem 1(?:\.0+)? A666$/ }).click();
  await market.getByText('A666 redemption complete', { exact: true }).waitFor();
  const after = await marketBalances(market);
  const progress = await market.locator('.navcoin-primary-progress-step.done').allTextContents();
  return { before, after, progress };
}

async function completePrivateRoundTrip(page) {
  await page.locator('.pf-sidebar .pf-nav').filter({ hasText: 'Process' }).click();
  const panel = page.locator('#private-navcoin-primary');
  await panel.waitFor();
  await panel.locator('select').first().selectOption('issue');
  await panel.locator('input[inputmode="decimal"]').fill('1');
  await panel.getByRole('button', { name: /Sign and issue privately/ }).click();
  await panel.getByText('Private primary swap committed', { exact: true }).first().waitFor();
  await panel.locator('select').first().selectOption('redeem');
  await panel.locator('select').nth(2).selectOption({ index: 1 });
  await panel.getByRole('button', { name: /Sign another redeem|Sign and redeem privately/ }).click();
  await panel.getByText('Private primary swap committed', { exact: true }).first().waitFor();
  return page.evaluate(() => {
    const key = Object.keys(localStorage).find(item => item.startsWith('postfiat.navcoin_private_primary.'));
    return { key, value: key ? JSON.parse(localStorage.getItem(key) || 'null') : null };
  });
}

async function requestProxyRestart(requestPath, pidPath) {
  const before = Number((await readFile(pidPath, 'utf8')).trim());
  assert.ok(Number.isSafeInteger(before) && before > 0);
  await writeFile(requestPath, 'restart\n', { flag: 'wx', mode: 0o600 });
  const deadline = Date.now() + 60_000;
  while (Date.now() < deadline) {
    const after = Number((await readFile(pidPath, 'utf8').catch(() => '')).trim());
    if (Number.isSafeInteger(after) && after > 0 && after !== before) return { before, after };
    await new Promise(resolveWait => setTimeout(resolveWait, 250));
  }
  throw new Error('production wallet proxy did not restart');
}

test('A666 R4 offline Ethereum rehearsal drives the production rendered wallet journey', {
  timeout: MAX_ASYNC_WAIT_MS + 10 * 60 * 1_000,
}, async () => {
  const origin = requiredLoopbackOrigin('POSTFIAT_R4_WALLET_ORIGIN', 'rehearsal wallet origin');
  const proxyOrigin = requiredLoopbackOrigin('POSTFIAT_R4_PROXY_ORIGIN', 'candidate proxy origin');
  const preflightOnly = process.env.POSTFIAT_R4_PREFLIGHT_ONLY === '1';
  const asyncBudgetMs = preflightOnly ? null : requiredAsyncBudget();
  const fireControlPath = requiredPath('POSTFIAT_R4_FIRE_CONTROL_MANIFEST');
  const setupPath = requiredPath('POSTFIAT_R4_SETUP_MANIFEST');
  const deploymentPath = requiredPath('POSTFIAT_R4_DEPLOYMENT_MANIFEST');
  const exportSlotPath = requiredPath('POSTFIAT_R4_EXPORT_ARTIFACT_SLOT');
  const returnSlotPath = requiredPath('POSTFIAT_R4_RETURN_ARTIFACT_SLOT');
  const proxyRestartPath = requiredPath('POSTFIAT_R4_PROXY_RESTART_REQUEST');
  const proxyPidPath = requiredPath('POSTFIAT_R4_PROXY_PID_FILE');
  const reportPath = requiredPath('POSTFIAT_R4_JOURNEY_REPORT');
  const runClassification = String(process.env.POSTFIAT_R4_RUN_CLASSIFICATION || 'construction');
  // Two-mode contract: preflight accepts only construction; full execution
  // accepts only official. POSTFIAT_R4_ASYNC_PROOF_SLOT stays preflight-only;
  // official execution consumes the distinct export/return artifact slots and
  // never the async proof slot.
  const runContract = resolveRunContract(preflightOnly, runClassification);

  const [fireControl, setup, deployment, productionGraph] = await Promise.all([
    readJson(fireControlPath, 'fire-control manifest'),
    readJson(setupPath, 'setup manifest'),
    readJson(deploymentPath, 'deployment manifest'),
    verifyProductionGraph(),
  ]);
  assert.equal(manifestCandidateRevision(setup), CANDIDATE_REVISION);
  assert.equal(manifestCandidateRevision(deployment), CANDIDATE_REVISION);
  assert.equal(manifestCandidateRevision(fireControl), CANDIDATE_REVISION);
  if (preflightOnly) {
    const asyncProofSlotPath = requiredPath('POSTFIAT_R4_ASYNC_PROOF_SLOT');
    await runConstructionPreflight({
      walletOrigin: origin, proxyOrigin, fireControl, setup, deployment, productionGraph,
      fireControlPath, setupPath, deploymentPath,
      exportSlotPath, returnSlotPath, asyncProofSlotPath, proxyRestartPath, proxyPidPath, reportPath,
    });
    return;
  }
  assert.equal(runContract.mode, 'official');
  const launchContract = officialLaunchContract({
    fireControlPath,
    fireControlSha256: String(process.env.POSTFIAT_R4_FIRE_CONTROL_MANIFEST_SHA256 || '').trim(),
    setupPath,
    setupSha256: String(process.env.POSTFIAT_R4_SETUP_MANIFEST_SHA256 || '').trim(),
    deploymentPath,
    deploymentSha256: String(process.env.POSTFIAT_R4_DEPLOYMENT_MANIFEST_SHA256 || '').trim(),
    walletOrigin: origin.href,
    proxyOrigin: proxyOrigin.href,
    asyncMaxWaitMs: asyncBudgetMs,
    exportSlotPath,
    returnSlotPath,
    proxyRestartPath,
    proxyPidPath,
    reportPath,
    fireControlReadyToFire: fireControl.ready_to_fire,
  });
  assert.ok(launchContract.ready_to_launch,
    `official launch contract refused before browser launch: ${JSON.stringify(launchContract)}`);
  const reportAlreadyExists = await stat(reportPath).then(() => true).catch(() => false);
  assert.equal(reportAlreadyExists, false, 'official report path must be unique');
  assertLoopbackManifest('setup manifest', setup);
  assertLoopbackManifest('deployment manifest', deployment);
  assert.equal(fireControl.ready_to_fire, true, 'fire-control must be computed GREEN before execution');
  const executionManifestHashes = await Promise.all([
    manifestHashObservation(fireControlPath, 'POSTFIAT_R4_FIRE_CONTROL_MANIFEST_SHA256'),
    manifestHashObservation(setupPath, 'POSTFIAT_R4_SETUP_MANIFEST_SHA256'),
    manifestHashObservation(deploymentPath, 'POSTFIAT_R4_DEPLOYMENT_MANIFEST_SHA256'),
  ]);
  assert.ok(executionManifestHashes.every(item => item.matches), 'full execution requires pinned manifest hashes');

  const passphrase = randomBytes(24).toString('hex');
  let seed = '';
  const results = [];
  const context = await chromium.launchPersistentContext('', { headless: true, acceptDownloads: true });
  try {
    const page = await context.newPage();
    const rpcFrames = [];
    page.on('websocket', socket => socket.on('framereceived', event => {
      if (typeof event.payload !== 'string') return;
      try { rpcFrames.push(JSON.parse(event.payload)); } catch { /* non-JSON frames are irrelevant */ }
    }));
    page.setDefaultTimeout(30_000);
    const response = await page.goto(origin.href, { waitUntil: 'domcontentloaded' });

    const expectedVersion = String(setup.wallet?.package_version ?? setup.wallet_package_version ?? '');
    assert.match(expectedVersion, /^\d+\.\d+\.\d+$/, 'setup manifest must pin the wallet package version');
    const expectedPackageJsonSha256 = String(
      setup.wallet?.package_json_sha256 ?? setup.wallet_package_json_sha256 ?? '',
    ).trim().toLowerCase();
    let packageJsonSha256 = null;
    if (expectedPackageJsonSha256) {
      packageJsonSha256 = await sha256File(join(WALLET_ROOT, 'package.json'));
      assert.equal(packageJsonSha256, expectedPackageJsonSha256,
        'setup manifest wallet package.json sha256 does not match the served wallet build');
    }
    const versionNode = page.locator('.pf-sidebar').getByText(`v${expectedVersion}`, { exact: true });
    await versionNode.waitFor({ state: 'visible' });
    const visibleVersion = String(await versionNode.textContent() || '').trim();
    results.push(observedStep(1, {
      label: 'build identity',
      response_status: response?.status(),
      visible_version: visibleVersion,
      expected_version: expectedVersion,
      package_json_sha256: packageJsonSha256,
      expected_package_json_sha256: expectedPackageJsonSha256 || null,
      candidate_revision: CANDIDATE_REVISION,
      production_import_graph: productionGraph,
    }, {
      served: response?.status() === 200,
      version_visible: visibleVersion.length > 0 && visibleVersion.includes(expectedVersion),
      package_json_sha256_match: !expectedPackageJsonSha256 || packageJsonSha256 === expectedPackageJsonSha256,
      candidate_bound: manifestCandidateRevision(setup) === CANDIDATE_REVISION,
      production_graph: Object.values(productionGraph).every(Boolean),
    }));

    const created = await createBrowserControlledWallet(page, passphrase);
    seed = created.seed;
    results.push(observedStep(2, {
      label: 'browser-controlled connect/create',
      wallet_address: created.address,
      shell_visible: await page.locator('.pf-shell').isVisible(),
    }, {
      browser_address: /^pf[0-9a-f]{40}$/.test(created.address),
      rendered_wallet: await page.locator('.pf-shell').isVisible(),
    }));

    const market = await openPrimaryMarket(page);
    const marketText = (await market.textContent() || '').replace(/\s+/g, ' ');
    const reserveResponse = await waitForReserveProofFrame(rpcFrames);
    const packets = reserveResponse.result?.packets ?? reserveResponse.result?.result?.packets ?? [];
    const sourceIdentities = packets.flatMap(packet => packet.source_identities ?? packet.sources ?? []);
    const proofIdentities = packets.map(packet => packet.packet_hash ?? packet.reserve_packet_hash).filter(Boolean);
    results.push(observedStep(3, {
      label: 'six sources/proofs/NAV',
      source_identities: sourceIdentities,
      proof_identities: proofIdentities,
      rendered_nav_summary: marketText.match(/Verified NAV[^\n]*/)?.[0] ?? '',
      reserve_response_id: reserveResponse.id ?? null,
    }, {
      six_sources: sourceIdentities.length === 6 && new Set(sourceIdentities).size === 6,
      both_proofs: proofIdentities.length >= 2 && new Set(proofIdentities).size >= 2,
      nav_rendered: /Verified NAV/.test(marketText),
      proof_rendered: /matches finalized PFTL reserve proof/.test(marketText),
    }));

    const initialBalances = await marketBalances(market);
    results.push(observedStep(4, {
      label: 'A666+pfUSDC balances',
      balances: initialBalances,
    }, {
      a666_visible: Object.keys(initialBalances).some(key => /A666/i.test(key)),
      pfusdc_visible: Object.keys(initialBalances).some(key => /pfUSDC/i.test(key)),
      finalized_marker: /YOUR BALANCES\s*finalized/i.test(marketText),
    }));

    const transparent = await completeTransparentRoundTrip(page, market);
    results.push(observedStep(5, {
      label: 'transparent issue/redeem',
      before_balances: transparent.before,
      after_balances: transparent.after,
      finalized_progress: transparent.progress,
    }, {
      issue_and_redeem_receipts: transparent.progress.length >= 2,
      round_trip_conserved: JSON.stringify(transparent.before) === JSON.stringify(transparent.after),
    }));

    const privateRoundTrip = await completePrivateRoundTrip(page);
    const privateRecords = privateRoundTrip.value?.records ?? [];
    const privateReceipts = privateRecords
      .map(record => record.response?.receipt?.receipt_identity ?? record.response?.swap?.certificate_ref)
      .filter(Boolean);
    results.push(observedStep(6, {
      label: 'private issue/redeem',
      recovery_key: privateRoundTrip.key,
      operation_identities: privateRecords.map(record => record.idempotency_key),
      receipt_identities: privateReceipts,
      final_balance_tuples: privateRecords.map(record => record.response?.final_balance_tuple).filter(Boolean),
    }, {
      issue_and_redeem_committed: privateRecords.filter(record => record.status === 'COMMITTED').length >= 2,
      distinct_receipts: privateReceipts.length >= 2 && new Set(privateReceipts).size >= 2,
      durable_recovery: typeof privateRoundTrip.key === 'string',
    }));

    await openPrimaryMarket(page);
    await market.getByRole('button', { name: /Mint A666/ }).first().click();
    await market.getByRole('button', { name: 'Deliver to MetaMask', exact: true }).click();
    await market.locator('#navcoin-amount').fill('1');
    await market.getByRole('button', { name: /^Mint & export 1(?:\.0+)? A666$/ }).click();
    const exportArtifact = await pollContentAddressedSlot(exportSlotPath, 'export', asyncBudgetMs);
    results.push(observedStep(7, {
      label: 'export',
      artifact_sha256: exportArtifact.artifact_sha256,
      receipt_identity: exportArtifact.receipt_identity,
      observed_chain_state: exportArtifact.observed_chain_state,
    }, {
      content_addressed: await sha256File(resolve(exportArtifact.artifact_path)) === exportArtifact.artifact_sha256,
      receipt_observed: Boolean(exportArtifact.receipt_identity),
      finalized_state_observed: exportArtifact.observed_chain_state.finalized === true,
    }));

    await market.getByRole('button', { name: 'Redeem', exact: true }).first().click();
    await market.getByRole('button', { name: 'From MetaMask', exact: true }).click();
    await market.locator('#navcoin-amount').fill('1');
    await market.getByRole('button', { name: /^Return & redeem 1(?:\.0+)? A666$/ }).click();
    const returnArtifact = await pollContentAddressedSlot(returnSlotPath, 'return', asyncBudgetMs);
    results.push(observedStep(8, {
      label: 'return',
      artifact_sha256: returnArtifact.artifact_sha256,
      receipt_identity: returnArtifact.receipt_identity,
      observed_chain_state: returnArtifact.observed_chain_state,
      conservation: returnArtifact.conservation,
    }, {
      content_addressed: await sha256File(resolve(returnArtifact.artifact_path)) === returnArtifact.artifact_sha256,
      receipt_observed: Boolean(returnArtifact.receipt_identity),
      finalized_state_observed: returnArtifact.observed_chain_state.finalized === true,
      exact_conservation: returnArtifact.conservation?.equal === true,
    }));

    const balancesBeforeRestart = await marketBalances(market);
    const receiptIdsBeforeRestart = [...new Set([...privateReceipts, exportArtifact.receipt_identity, returnArtifact.receipt_identity])];
    const restart = await requestProxyRestart(proxyRestartPath, proxyPidPath);
    await page.reload({ waitUntil: 'domcontentloaded' });
    await unlock(page, passphrase);
    const recoveredMarket = await openPrimaryMarket(page);
    const balancesAfterRestart = await marketBalances(recoveredMarket);
    const recovered = await page.evaluate(() => Object.fromEntries(
      Object.keys(localStorage)
        .filter(key => key.startsWith('postfiat.navcoin_private_primary.') || key.includes('navcoin'))
        .map(key => [key, localStorage.getItem(key)]),
    ));
    const recoveredText = JSON.stringify(recovered);
    results.push(observedStep(9, {
      label: 'restart+reload recovery',
      proxy_pid_before: restart.before,
      proxy_pid_after: restart.after,
      balances_before: balancesBeforeRestart,
      balances_after: balancesAfterRestart,
      receipt_identities: receiptIdsBeforeRestart,
    }, {
      actual_proxy_restart: restart.before !== restart.after,
      exact_balances: JSON.stringify(balancesBeforeRestart) === JSON.stringify(balancesAfterRestart),
      receipt_identities_recovered: receiptIdsBeforeRestart.every(identity => recoveredText.includes(identity)),
    }));

    await page.locator('.pf-sidebar .pf-nav').filter({ hasText: 'Process' }).click();
    const downloadControl = page.getByRole('button', { name: 'Download public receipt', exact: true });
    await downloadControl.waitFor({ state: 'visible' });
    const event = page.waitForEvent('download');
    await downloadControl.click();
    const download = await event;
    const downloadPath = await download.path();
    assert.ok(downloadPath);
    const receiptText = await readFile(downloadPath, 'utf8');
    const publicReceipt = JSON.parse(receiptText);
    scanForbiddenFields(publicReceipt);
    assertCustodyValuesAbsent('downloaded public receipt', receiptText, [seed, passphrase]);
    results.push(observedStep(10, {
      label: 'public receipt download',
      suggested_filename: download.suggestedFilename(),
      schema: publicReceipt.schema,
      receipt_identity: publicReceipt.receipt_identity ?? publicReceipt.certificate_ref,
      final_balance_tuple: publicReceipt.final_balance_tuple,
    }, {
      real_download: (await stat(downloadPath)).size > 0,
      versioned_schema: /^postfiat[.-].*public[-_.]receipt.*v[0-9]+$/i.test(String(publicReceipt.schema || '')),
      receipt_bound: receiptIdsBeforeRestart.includes(publicReceipt.receipt_identity ?? publicReceipt.certificate_ref),
      final_balances_present: Array.isArray(publicReceipt.final_balance_tuple),
    }));

    const report = {
      schema: 'postfiat.a666.r4.offline-ethereum-browser-journey.v1',
      candidate_revision: CANDIDATE_REVISION,
      run_classification: runClassification,
      steps: results,
      all_steps_observed: results.length === JOURNEY_LABELS.length && results.every(result => result.ok),
      construction_execution: runClassification === 'construction'
        && results.length === JOURNEY_LABELS.length
        && results.every(result => result.ok),
    };
    scanForbiddenFields(report);
    assertCustodyValuesAbsent('journey report', report, [seed, passphrase]);
    await writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`, { flag: 'wx', mode: 0o600 });
    assert.equal(report.all_steps_observed, true);
  } finally {
    await context.close();
  }
});

function syntheticOfficialInputs() {
  return {
    fireControlPath: '/synthetic/fire-control.json',
    fireControlSha256: 'a'.repeat(64),
    setupPath: '/synthetic/setup.json',
    setupSha256: 'b'.repeat(64),
    deploymentPath: '/synthetic/deployment.json',
    deploymentSha256: 'c'.repeat(64),
    walletOrigin: 'http://127.0.0.1:8080/',
    proxyOrigin: 'http://127.0.0.1:31021/',
    asyncMaxWaitMs: 3_600_000,
    exportSlotPath: '/synthetic/export-artifact-slot.json',
    returnSlotPath: '/synthetic/return-artifact-slot.json',
    proxyRestartPath: '/synthetic/proxy.restart-request',
    proxyPidPath: '/synthetic/proxy.ready.json',
    reportPath: '/synthetic/official-journey-report.json',
    fireControlReadyToFire: true,
  };
}

test('A666 R4 two-mode classification contract refuses mismatched and unknown modes', () => {
  assert.throws(() => resolveRunContract(true, 'official'), /preflight-only mode requires construction classification/);
  assert.throws(() => resolveRunContract(false, 'construction'), /full execution requires explicit official classification/);
  assert.throws(() => resolveRunContract(false, 'bogus'), /full execution requires explicit official classification/);
  assert.throws(() => resolveRunContract(true, 'bogus'), /preflight-only mode requires construction classification/);
  assert.deepEqual(resolveRunContract(true, 'construction'), { mode: 'preflight_only', classification: 'construction' });
  assert.deepEqual(resolveRunContract(false, 'official'), { mode: 'official', classification: 'official' });
});

test('A666 R4 official launch contract refuses with one required input missing (no browser, no mutation)', () => {
  const inputs = syntheticOfficialInputs();
  delete inputs.returnSlotPath;
  const contract = officialLaunchContract(inputs);
  assert.equal(contract.ready_to_launch, false);
  assert.deepEqual(contract.missing_inputs, ['returnSlotPath']);
  const unready = { ...syntheticOfficialInputs(), fireControlReadyToFire: false };
  const refused = officialLaunchContract(unready);
  assert.equal(refused.ready_to_launch, false);
  assert.deepEqual(refused.failed_checks, ['fire_control_ready_to_fire']);
});

test('A666 R4 official launch contract accepts a complete synthetic input map (ready_to_launch, no Chromium)', () => {
  const contract = officialLaunchContract(syntheticOfficialInputs());
  assert.deepEqual(contract, { ready_to_launch: true, missing_inputs: [], failed_checks: [] });
});

test('A666 R4 official input contract matches the runner accessor surface exactly (strict 1:1)', async () => {
  const { execFileSync } = await import('node:child_process');
  const repoRoot = resolve(WALLET_ROOT, '..');
  const output = execFileSync(
    join(repoRoot, 'scripts/a666-r4-official-input-contract-extract'),
    ['check'],
    { cwd: repoRoot, encoding: 'utf8' },
  );
  const result = JSON.parse(output);
  assert.equal(result.ok, true, `input contract drift: ${output}`);
  assert.equal(result.mismatch_count, 0);
  assert.ok(result.accessor_count > 0);
});

function setupProvenanceGate(setup, expectedWallet) {
  // Required fire-control behavior for candidate_revision_and_provenance:
  // the candidate-bound setup manifest pins the exact package object that
  // the official journey hashes at step 1. Pure and deterministic; invokes
  // nothing.
  const missingInputs = [];
  if (manifestCandidateRevision(setup) !== CANDIDATE_REVISION) missingInputs.push('candidate_revision');
  const version = String(setup?.wallet?.package_version ?? setup?.wallet_package_version ?? '');
  const packageJsonSha256 = String(
    setup?.wallet?.package_json_sha256 ?? setup?.wallet_package_json_sha256 ?? '',
  ).trim().toLowerCase();
  if (!/^\d+\.\d+\.\d+$/.test(version) || version !== expectedWallet.version) {
    missingInputs.push('wallet.package_version');
  }
  if (!/^[0-9a-f]{64}$/.test(packageJsonSha256)
      || packageJsonSha256 !== expectedWallet.packageJsonSha256) {
    missingInputs.push('wallet.package_json_sha256');
  }
  const ok = missingInputs.length === 0;
  return {
    candidate_revision_and_provenance: ok,
    ready_to_fire: ok,
    missing_inputs: missingInputs,
    official_journey_invocations: 0,
  };
}

function officialInputContractGate(accessors, expectedAccessorIds, targetStatuses) {
  const ids = accessors.map(entry => entry?.id);
  const runtimeKinds = new Set(['runtime_observe', 'runtime_slot_read']);
  const expectedIds = new Set(expectedAccessorIds);
  const uniqueIds = new Set(ids);
  const invalid = [];
  for (const entry of accessors) {
    const runtime = runtimeKinds.has(entry?.access_kind);
    const expectedInputClass = runtime ? 'runtime_derived' : 'prelaunch_external';
    const target = entry?.fire_control_target;
    const binding = entry?.enforcement?.producer_binding;
    const targetOk = Object.prototype.hasOwnProperty.call(targetStatuses, target)
      && targetStatuses[target] === true;
    const bindingOk = runtime
      ? typeof binding === 'string' && binding.startsWith('runner:')
      : typeof binding === 'string' && binding.startsWith('fire_control:');
    if (!entry?.id || !expectedIds.has(entry.id)
        || !entry.validation_predicate || !target
        || entry.enforcement?.validation_target !== target
        || entry.enforcement?.input_class !== expectedInputClass
        || !bindingOk || entry.currently_enforced_in_fire_control !== true || !targetOk) {
      invalid.push(entry?.id ?? '<missing-id>');
    }
  }
  const exactSet = ids.length === expectedAccessorIds.length
    && uniqueIds.size === expectedAccessorIds.length
    && expectedAccessorIds.every(id => uniqueIds.has(id));
  const ready = exactSet && invalid.length === 0;
  return {
    ready_to_fire: ready,
    official_journey_invocations: 0,
    invalid_accessor_ids: invalid,
  };
}

test('A666 R4 fire-control input gate rejects missing wallet identity pins before official launch', async () => {
  const repoRoot = resolve(WALLET_ROOT, '..');
  const setupPath = join(repoRoot, 'docs/evidence/a666-public-reserve-product-20260803/browser/r4-pass1/setup-endpoints-manifest.json');
  const fixture = JSON.parse(await readFile(setupPath, 'utf8'));
  const expectedWallet = {
    version: fixture.wallet.package_version,
    packageJsonSha256: fixture.wallet.package_json_sha256,
  };

  const missingVersion = structuredClone(fixture);
  delete missingVersion.wallet.package_version;
  const versionGate = setupProvenanceGate(missingVersion, expectedWallet);
  assert.equal(versionGate.ready_to_fire, false);
  assert.equal(versionGate.official_journey_invocations, 0);
  assert.ok(versionGate.missing_inputs.includes('wallet.package_version'));

  const hashMismatch = structuredClone(fixture);
  hashMismatch.wallet.package_json_sha256 = '0'.repeat(64);
  const hashGate = setupProvenanceGate(hashMismatch, expectedWallet);
  assert.equal(hashGate.ready_to_fire, false);
  assert.equal(hashGate.official_journey_invocations, 0);
  assert.ok(hashGate.missing_inputs.includes('wallet.package_json_sha256'));
});

test('TODO RED-FIRST: v3 build identity must hash pinned candidate package object, not current worktree bytes', {
  todo: 'official v3 step 1 hashes WALLET_ROOT/package.json instead of candidate:wallet-web/package.json',
}, async () => {
  const { execFileSync } = await import('node:child_process');
  const repoRoot = resolve(WALLET_ROOT, '..');
  const setupPath = join(repoRoot, 'docs/evidence/a666-public-reserve-product-20260803/browser/r4-pass1/setup-endpoints-manifest.json');
  const setup = JSON.parse(await readFile(setupPath, 'utf8'));
  const candidateRevision = manifestCandidateRevision(setup);
  const expectedPackageJsonSha256 = String(setup.wallet?.package_json_sha256 ?? '').trim().toLowerCase();

  assert.equal(candidateRevision, CANDIDATE_REVISION, 'setup must remain pinned to the v3 candidate revision');
  assert.match(expectedPackageJsonSha256, /^[0-9a-f]{64}$/, 'setup must pin a package.json hash');
  const pinnedCandidatePackage = execFileSync(
    'git',
    ['show', `${candidateRevision}:wallet-web/package.json`],
    { cwd: repoRoot },
  );
  const pinnedCandidatePackageSha256 = createHash('sha256').update(pinnedCandidatePackage).digest('hex');
  assert.equal(pinnedCandidatePackageSha256, expectedPackageJsonSha256,
    'required behavior: setup hash must match the pinned candidate package object');

  const worktreePackageSha256 = await sha256File(join(WALLET_ROOT, 'package.json'));
  assert.notEqual(worktreePackageSha256, expectedPackageJsonSha256,
    'RED fixture requires intentionally different current worktree package bytes');
  const runnerSource = await readFile(fileURLToPath(import.meta.url), 'utf8');
  assert.match(runnerSource,
    /packageJsonSha256 = await sha256File\(join\(WALLET_ROOT, 'package\.json'\)\);/,
    'current step-1 implementation must be the captured worktree-hash behavior');
  assert.equal(worktreePackageSha256, expectedPackageJsonSha256,
    'required behavior: build identity must hash candidate:wallet-web/package.json, not WALLET_ROOT/package.json');
});

test('A666 R4 120-entry official input contract rejects every incomplete enforcement shape', async () => {
  const repoRoot = resolve(WALLET_ROOT, '..');
  const contractPath = join(repoRoot, 'docs/evidence/a666-public-reserve-product-20260803/browser/r4-construction/official-input-contract.json');
  const contract = JSON.parse(await readFile(contractPath, 'utf8'));
  const entries = contract.accessors;
  const ids = entries.map(entry => entry.id);
  const targets = Object.fromEntries(ids.map(id => {
    const target = entries.find(entry => entry.id === id).fire_control_target;
    return [target, true];
  }));
  const green = officialInputContractGate(entries, ids, targets);
  assert.equal(entries.length, 120);
  assert.equal(green.ready_to_fire, true);
  assert.equal(green.official_journey_invocations, 0);

  const vectors = [
    ['missing_contract_entry', entries.slice(1)],
    ['stale_contract_entry', entries.map((entry, index) => index === 0 ? { ...entry, id: 'stale:entry' } : entry)],
    ['required_entry_without_predicate', entries.map((entry, index) => index === 0 ? { ...entry, validation_predicate: '' } : entry)],
    ['required_entry_without_target', entries.map((entry, index) => index === 0 ? { ...entry, fire_control_target: '', enforcement: { ...entry.enforcement, validation_target: '' } } : entry)],
    ['runtime_entry_without_producer_binding', entries.map(entry => entry.access_kind === 'runtime_observe'
      ? { ...entry, enforcement: { ...entry.enforcement, producer_binding: '' } }
      : entry)],
  ];
  for (const [label, mutated] of vectors) {
    const gate = officialInputContractGate(mutated, ids, targets);
    assert.equal(gate.ready_to_fire, false, label);
    assert.equal(gate.official_journey_invocations, 0, label);
    assert.ok(gate.invalid_accessor_ids.length > 0 || label === 'missing_contract_entry', label);
  }
});
