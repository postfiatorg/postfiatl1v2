import assert from 'node:assert/strict';
import { execFileSync, spawn } from 'node:child_process';
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

const CANDIDATE_REVISION = '23b9bffbd7fcf3327ce67615e7abd5dd3265ea9f';
const ASYNC_PROOF_SLOT_SCHEMA = 'postfiat.a666.r4.async-proof-slot.v1';
const ASYNC_PROOF_RUN_ID = 'a666-r4-receipt-prover-pathb-20260804-v3';
const JOURNEY_ROUTE_ID = 'pftl-a666-r4-offline-rehearsal-v1';
const RESERVE_PROOF_SCHEMA = 'postfiat.nav_reserve_proof_status.v1';
const DRY_RUN_STEPS = Object.freeze([1, 2, 3, 4, 5, 6, 9, 10]);
const POLL_INTERVAL_MS = 5_000;
const MAX_ASYNC_WAIT_MS = 12 * 60 * 60 * 1_000;
const FORBIDDEN_FIELD = /seed|mnemonic|private[_-]?key|owner[_-]?key|spend[_-]?auth/i;
const WALLET_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const REPO_ROOT = resolve(WALLET_ROOT, '..');
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

function pinnedCandidatePackage(repoRoot, revision) {
  // Build identity is the pinned candidate git object, never mutable
  // worktree bytes. Argument-safe execFileSync: no shell, no string
  // interpolation into a command line.
  const bytes = execFileSync('git', ['show', `${revision}:wallet-web/package.json`], { cwd: repoRoot });
  const parsed = JSON.parse(bytes.toString('utf8'));
  return {
    revision,
    version: String(parsed.version || ''),
    sha256: createHash('sha256').update(bytes).digest('hex'),
  };
}

function hashPinMatches(pin, actual) {
  const normalizedPin = String(pin || '').trim().toLowerCase();
  if (!/^[0-9a-f]{64}$/.test(normalizedPin)) return false;
  return actual === undefined || actual === null || String(actual).trim().toLowerCase() === normalizedPin;
}

// Mode-symmetric validation predicate registry. Every validation predicate
// reachable by official mode lives here and ONLY here; construction
// preflight and official execution run the identical registry on the same
// normalized inputs before any mode branch. Mode differences begin only at
// mutation execution/receipt capture. Tiers: "shape" predicates evaluate
// scalar input shape (also used by the officialLaunchContract adapter);
// "document" predicates evaluate parsed manifest/pinned-object documents.
const VALIDATION_PREDICATE_REGISTRY = Object.freeze([
  { id: 'fire_control_ready_to_fire', tier: 'shape',
    run: i => (i.fireControl ? i.fireControl.ready_to_fire : i.fireControlReadyToFire) === true },
  { id: 'fire_control_hash_pinned', tier: 'shape',
    run: i => hashPinMatches(i.fireControlSha256Pin, i.fireControlSha256Actual) },
  { id: 'setup_hash_pinned', tier: 'shape',
    run: i => hashPinMatches(i.setupSha256Pin, i.setupSha256Actual) },
  { id: 'deployment_hash_pinned', tier: 'shape',
    run: i => hashPinMatches(i.deploymentSha256Pin, i.deploymentSha256Actual) },
  { id: 'wallet_origin_loopback', tier: 'shape',
    run: i => loopbackHref(i.walletOrigin) },
  { id: 'proxy_origin_loopback', tier: 'shape',
    run: i => loopbackHref(i.proxyOrigin) },
  { id: 'async_budget_bounded', tier: 'shape',
    run: i => i.asyncBudgetMs === null
      || (Number.isSafeInteger(i.asyncBudgetMs) && i.asyncBudgetMs > 0 && i.asyncBudgetMs <= MAX_ASYNC_WAIT_MS) },
  { id: 'distinct_artifact_slots', tier: 'shape',
    run: i => Boolean(i.exportSlotPath) && Boolean(i.returnSlotPath) && i.exportSlotPath !== i.returnSlotPath },
  { id: 'candidate_revision_fire_control', tier: 'document',
    run: i => manifestCandidateRevision(i.fireControl) === CANDIDATE_REVISION },
  { id: 'candidate_revision_setup', tier: 'document',
    run: i => manifestCandidateRevision(i.setup) === CANDIDATE_REVISION },
  { id: 'candidate_revision_deployment', tier: 'document',
    run: i => manifestCandidateRevision(i.deployment) === CANDIDATE_REVISION },
  { id: 'setup_manifest_loopback', tier: 'document',
    run: i => loopbackManifestObservation('setup manifest', i.setup).valid },
  { id: 'deployment_manifest_loopback', tier: 'document',
    run: i => loopbackManifestObservation('deployment manifest', i.deployment).valid },
  { id: 'pinned_candidate_package_object_well_formed', tier: 'document',
    run: i => /^\d+\.\d+\.\d+$/.test(String(i.pinnedPackage?.version || ''))
      && /^[0-9a-f]{64}$/.test(String(i.pinnedPackage?.sha256 || '')) },
  // Wallet identity pins, when carried by the setup manifest, must match the
  // pinned candidate git object exactly. Pin PRESENCE is enforced in both
  // modes by fire_control_ready_to_fire (the fire-control aggregate refuses
  // GREEN without wallet identity pins; see a71a397). Mutable worktree bytes
  // are never consulted for candidate identity.
  { id: 'wallet_package_version_consistent_with_pinned_object', tier: 'document',
    run: i => {
      const pin = String(i.setup?.wallet?.package_version ?? i.setup?.wallet_package_version ?? '');
      return pin === '' ? true : pin === i.pinnedPackage.version;
    } },
  { id: 'wallet_package_json_sha256_consistent_with_pinned_object', tier: 'document',
    run: i => {
      const pin = String(i.setup?.wallet?.package_json_sha256 ?? i.setup?.wallet_package_json_sha256 ?? '')
        .trim().toLowerCase();
      return pin === '' ? true : pin === i.pinnedPackage.sha256;
    } },
  { id: 'report_path_available', tier: 'document',
    run: i => i.reportPathAvailable === true },
  { id: 'production_import_graph', tier: 'document',
    run: i => Object.values(i.productionGraph).every(Boolean) },
]);

function runValidationRegistry(normalizedInputs) {
  // Pure and deterministic: evaluates the shared registry on normalized
  // inputs, launches nothing, invokes nothing, mutates nothing.
  const trace = VALIDATION_PREDICATE_REGISTRY.map(predicate => {
    let ok = false;
    try { ok = Boolean(predicate.run(normalizedInputs)); } catch { ok = false; }
    return { id: predicate.id, ok };
  });
  const failed = trace.filter(entry => !entry.ok).map(entry => entry.id);
  return {
    ok: failed.length === 0,
    trace,
    failed,
    predicate_count: trace.length,
    official_journey_invocations: 0,
    business_mutations: 0,
  };
}

function officialLaunchContract(inputs) {
  // Computed complete-input contract for official mode. Pure and
  // deterministic: it inspects only the supplied input map, launches no
  // browser, and issues no business mutation. Every official input must be
  // present and well-shaped; nothing is inferred and no preflight-only
  // default is reused. Shape checks delegate to the shared validation
  // predicate registry so no predicate lives outside it.
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
  const shapeNormalized = {
    fireControlReadyToFire: inputs.fireControlReadyToFire,
    fireControlSha256Pin: inputs.fireControlSha256,
    setupSha256Pin: inputs.setupSha256,
    deploymentSha256Pin: inputs.deploymentSha256,
    walletOrigin: inputs.walletOrigin,
    proxyOrigin: inputs.proxyOrigin,
    asyncBudgetMs: inputs.asyncMaxWaitMs,
    exportSlotPath: inputs.exportSlotPath,
    returnSlotPath: inputs.returnSlotPath,
  };
  const failedChecks = VALIDATION_PREDICATE_REGISTRY
    .filter(predicate => predicate.tier === 'shape')
    .filter(predicate => !predicate.run(shapeNormalized))
    .map(predicate => predicate.id);
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

async function observeStepOneBuildIdentity({
  page, walletOrigin, setup, pinnedPackage, productionGraph, passphrase,
  createWallet = createBrowserControlledWallet,
}) {
  const response = await page.goto(walletOrigin.href, { waitUntil: 'domcontentloaded' });
  assert.equal(response?.status(), 200, 'candidate wallet origin must return HTTP 200');
  const expectedVersion = String(setup.wallet?.package_version ?? setup.wallet_package_version ?? '');
  assert.match(expectedVersion, /^\d+\.\d+\.\d+$/, 'setup manifest must pin the wallet package version');
  const expectedPackageJsonSha256 = String(
    setup.wallet?.package_json_sha256 ?? setup.wallet_package_json_sha256 ?? '',
  ).trim().toLowerCase();
  assert.match(expectedPackageJsonSha256, /^[0-9a-f]{64}$/,
    'setup manifest must pin the wallet package.json sha256');
  assert.equal(pinnedPackage.sha256, expectedPackageJsonSha256,
    'setup manifest wallet package.json sha256 does not match the pinned candidate package object');
  assert.equal(expectedVersion, pinnedPackage.version,
    'setup manifest wallet package version does not match the pinned candidate package object');

  const created = await createWallet(page, passphrase);
  const versionNode = page.locator('.pf-sidebar').getByText(`v${expectedVersion}`, { exact: true });
  await versionNode.waitFor({ state: 'visible' });
  const visibleVersion = String(await versionNode.textContent() || '').trim();
  assert.equal(visibleVersion, `v${expectedVersion}`, 'production sidebar package identity must match exactly');
  const step = observedStep(1, {
    label: 'build identity',
    response_status: response.status(),
    visible_version: visibleVersion,
    expected_version: expectedVersion,
    package_json_sha256: pinnedPackage.sha256,
    expected_package_json_sha256: expectedPackageJsonSha256,
    package_identity_source: `git show ${CANDIDATE_REVISION}:wallet-web/package.json`,
    candidate_revision: CANDIDATE_REVISION,
    production_import_graph: productionGraph,
  }, {
    served: response.status() === 200,
    wallet_created_and_unlocked: Boolean(created?.address),
    version_visible_exact: visibleVersion === `v${expectedVersion}`,
    package_json_sha256_match: pinnedPackage.sha256 === expectedPackageJsonSha256,
    candidate_bound: manifestCandidateRevision(setup) === CANDIDATE_REVISION,
    production_graph: Object.values(productionGraph).every(Boolean),
  });
  assert.equal(step.ok, true, 'production step-1 browser identity predicates must all pass');
  return { step, created };
}

async function observeStepTwoWalletShell(page, created) {
  // Shared step-2 display predicates: identical in construction preflight
  // and official mode. Read-only DOM observation; no mutation.
  const shellVisible = await page.locator('.pf-shell').isVisible();
  return observedStep(2, {
    label: 'browser-controlled connect/create',
    wallet_address: created.address,
    shell_visible: shellVisible,
  }, {
    browser_address: /^pf[0-9a-f]{40}$/.test(created.address),
    rendered_wallet: shellVisible,
  });
}

function reserveProofContract(request, response) {
  assert.equal(request?.method, 'nav_reserve_proof_status', 'reserve proof request method mismatch');
  assert.match(String(request?.id || ''), /^web-[1-9][0-9]*$/, 'reserve proof request id is malformed');
  assert.match(String(request?.params?.asset_id || ''), /^[0-9a-f]{96}$/, 'reserve proof asset_id is malformed');
  assert.equal(response?.id, request.id, 'reserve proof response id does not match its request');
  assert.equal(response?.ok, true, 'reserve proof RPC response is not ok');
  const report = response?.result;
  assert.equal(report?.schema, RESERVE_PROOF_SCHEMA, 'reserve proof response schema mismatch');
  assert.equal(report?.found, true, 'reserve proof response did not find the selected asset');
  assert.equal(report?.asset_id, request.params.asset_id, 'reserve proof response asset_id mismatch');
  assert.ok(Array.isArray(report?.packets) && report.packets.length > 0 && report.packets.length <= 16,
    'reserve proof packet count is outside the bounded contract');
  assert.ok(report.packets.every(packet => typeof packet?.state === 'string' && packet.state.length > 0),
    'reserve proof packet state is missing');
  const finalized = report.packets.filter(packet => packet.state === 'finalized');
  assert.ok(finalized.length >= 2, 'reserve proof response must expose at least two finalized packets');
  assert.ok(finalized.every(packet => Number.isInteger(packet.source_count) && packet.source_count === 6),
    'every finalized reserve packet must declare source_count 6');
  const proofIdentities = finalized
    .map(packet => packet.reserve_packet_hash ?? packet.packet_id)
    .filter(identity => typeof identity === 'string' && identity.length > 0);
  assert.ok(proofIdentities.length >= 2 && new Set(proofIdentities).size >= 2,
    'reserve proof response must expose two distinct packet/proof identities');
  return { report, finalized, proofIdentities };
}

async function waitForMarketText(market, needle, timeoutMs = 30_000) {
  // Poll the market DOM until the exact rendered text appears. Fail-closed:
  // a timeout throws; a pre-data snapshot never satisfies the observation.
  const deadline = Date.now() + timeoutMs;
  let text = '';
  while (Date.now() < deadline) {
    text = ((await market.textContent()) || '').replace(/\s+/g, ' ');
    if (text.includes(needle)) return text;
    await new Promise(resolveWait => setTimeout(resolveWait, 100));
  }
  throw new Error(`market did not render exact text within budget: ${needle}`);
}

async function observeStepThreeReserveProof(page, rpcFrames) {
  // Shared step-3 choreography and predicates are identical in construction
  // and official mode. Select the exact route, accept only the method/id-
  // bound reserve-proof response, then wait for the DOM rerender that the
  // refreshed snapshot drives. Text captured before the bound proof frame
  // can never satisfy the proof-rendered observation. Read-only; no mutation.
  const opened = await openPrimaryMarket(page, rpcFrames);
  const reserveResponse = await waitForReserveProofFrame(rpcFrames);
  const marketText = await waitForMarketText(opened.market, 'matches finalized PFTL reserve proof');
  const proof = reserveProofContract(opened.proofRequest, reserveResponse);
  const packets = reserveResponse.result?.packets ?? reserveResponse.result?.result?.packets ?? [];
  const step = observedStep(3, {
    label: 'six sources/proofs/NAV',
    route_id: JOURNEY_ROUTE_ID,
    request_method: opened.proofRequest.method,
    request_id: opened.proofRequest.id,
    request_asset_id: opened.proofRequest.params.asset_id,
    response_id: reserveResponse.id,
    finalized_packet_count: proof.finalized.length,
    source_counts: proof.finalized.map(packet => packet.source_count),
    proof_identities: proof.proofIdentities,
    rendered_nav_summary: marketText.match(/Verified NAV[^\n]*/)?.[0] ?? '',
  }, {
    response_schema: proof.report.schema === RESERVE_PROOF_SCHEMA,
    packets_bounded: packets.length > 0 && packets.length <= 16,
    finalized_sources_six: proof.finalized.every(packet => packet.source_count === 6),
    both_proofs: new Set(proof.proofIdentities).size >= 2,
    nav_rendered: /Verified NAV/.test(marketText),
    proof_rendered: /matches finalized PFTL reserve proof/.test(marketText),
  });
  return { step, market: opened.market, marketText };
}

async function observeStepFourBalances(market, marketText) {
  // Shared step-4 display predicates: identical in construction preflight
  // and official mode. Read-only DOM observation; no mutation.
  const initialBalances = await marketBalances(market);
  return observedStep(4, {
    label: 'A666+pfUSDC balances',
    balances: initialBalances,
  }, {
    a666_visible: Object.keys(initialBalances).some(key => /A666/i.test(key)),
    pfusdc_visible: Object.keys(initialBalances).some(key => /pfUSDC/i.test(key)),
    finalized_marker: /YOUR BALANCES\s*finalized/i.test(marketText),
  });
}

// Registry of the shared steps 1-4 display observers. The structural
// completeness guard fails the suite if any step 1-4 predicate is
// reachable only in official mode, if construction misses an observer, or
// if a mutation step (5-8) or dependent step (9-10) enters construction.
const SHARED_DISPLAY_OBSERVERS = Object.freeze([
  'observeStepOneBuildIdentity',
  'observeStepTwoWalletShell',
  'observeStepThreeReserveProof',
  'observeStepFourBalances',
]);
const CONSTRUCTION_FORBIDDEN_MUTATION_STEPS = Object.freeze([
  'completeTransparentRoundTrip',
  'completePrivateRoundTrip',
  'pollContentAddressedSlot',
  'requestProxyRestart',
]);

function sharedDisplayCoverageProblems(source) {
  // Pure static completeness check over the runner source. Needles are
  // concatenated so this guard's own source never matches them.
  const problems = [];
  const preflightNeedle = 'async function runConstructionPreflight' + '({';
  const preflightStart = source.indexOf(preflightNeedle);
  const preflightEnd = source.indexOf('\nasync function', preflightStart + 1);
  if (preflightStart === -1 || preflightEnd === -1) return ['construction preflight function not found'];
  const preflight = source.slice(preflightStart, preflightEnd);
  for (const observer of SHARED_DISPLAY_OBSERVERS) {
    if (!preflight.includes(`${observer}(`)) {
      problems.push(`construction preflight missing shared observer: ${observer}`);
    }
  }
  for (const mutation of CONSTRUCTION_FORBIDDEN_MUTATION_STEPS) {
    if (preflight.includes(`${mutation}(`)) {
      problems.push(`mutation/dependent step reachable in construction preflight: ${mutation}`);
    }
  }
  const officialStart = source.indexOf("assert.equal(runContract.mode," + " 'official');");
  const officialEnd = source.indexOf('await completeTransparentRoundTrip' + '(page, market)', officialStart);
  if (officialStart === -1 || officialEnd === -1) {
    problems.push('official steps 1-4 span not found');
    return problems;
  }
  const officialDisplay = source.slice(officialStart, officialEnd);
  const inline = officialDisplay.match(/observedStep\([1-4],/g) ?? [];
  if (inline.length > 0) {
    problems.push(`official steps 1-4 define predicates inline outside the shared observers: ${inline.join(', ')}`);
  }
  for (const observer of SHARED_DISPLAY_OBSERVERS) {
    if (!officialDisplay.includes(`${observer}(`)) {
      problems.push(`official journey missing shared observer: ${observer}`);
    }
  }
  return problems;
}

async function runConstructionPreflight({
  walletOrigin, proxyOrigin, fireControl, setup, deployment, productionGraph, pinnedPackage,
  fireControlPath, setupPath, deploymentPath,
  exportSlotPath, returnSlotPath, asyncProofSlotPath, proxyRestartPath, proxyPidPath, reportPath,
  manifestHashes: sharedManifestHashes, validationTrace,
}) {
  const [fireControlHash, setupHash, deploymentHash] = sharedManifestHashes;
  const [exportSlot, returnSlot, asyncProofSlot] = await Promise.all([
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
  let stepOneBrowser;
  const constructionDisplaySteps = [];
  const recordDisplayStep = async (name, run) => {
    // Construction records step 2-4 display outcomes as findings. A
    // staged-stack RED is preserved exactly, never asserted away and never
    // blocking; zero mutations and zero official invocations are issued.
    try {
      const step = await run();
      constructionDisplaySteps.push({ name, executed: true, ok: step.ok === true, checks: step.checks ?? null });
      return step;
    } catch (error) {
      constructionDisplaySteps.push({
        name,
        executed: true,
        ok: false,
        construction_finding: String(error?.message || error).split('\n')[0].slice(0, 300),
      });
      return null;
    }
  };
  const browser = await chromium.launch({ headless: true });
  try {
    const context = await browser.newContext();
    const page = await context.newPage();
    const rpcTraffic = { sent: [], received: [] };
    page.on('websocket', socket => {
      socket.on('framesent', event => {
        if (typeof event.payload !== 'string') return;
        try { rpcTraffic.sent.push(JSON.parse(event.payload)); } catch { /* non-JSON frames are irrelevant */ }
      });
      socket.on('framereceived', event => {
        if (typeof event.payload !== 'string') return;
        try { rpcTraffic.received.push(JSON.parse(event.payload)); } catch { /* non-JSON frames are irrelevant */ }
      });
    });
    page.setDefaultTimeout(30_000);
    stepOneBrowser = await observeStepOneBuildIdentity({
      page,
      walletOrigin,
      setup,
      pinnedPackage,
      productionGraph,
      passphrase: 'a666-r4-construction-step1',
    });
    await recordDisplayStep('step_2_wallet_shell', () => observeStepTwoWalletShell(page, stepOneBrowser.created));
    let stepThreeObserved = null;
    await recordDisplayStep('step_3_reserve_proof', async () => {
      stepThreeObserved = await observeStepThreeReserveProof(page, rpcTraffic);
      return stepThreeObserved.step;
    });
    if (stepThreeObserved) {
      await recordDisplayStep('step_4_balances', () => observeStepFourBalances(stepThreeObserved.market, stepThreeObserved.marketText));
    } else {
      constructionDisplaySteps.push({
        name: 'step_4_balances',
        executed: false,
        ok: false,
        construction_finding: 'skipped: step 3 reserve-proof observation was not green in construction',
      });
    }
  } finally {
    await browser.close();
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
    && stepOneBrowser.step.ok
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
    step_1_browser_identity_green: stepOneBrowser.step.ok,
    step_1_browser_observation: stepOneBrowser.step,
    steps_2_4_display_observations: constructionDisplaySteps,
    steps_1_4_display_all_green: stepOneBrowser.step.ok
      && constructionDisplaySteps.every(entry => entry.executed && entry.ok),
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
    validation_trace: validationTrace,
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

async function waitForTrafficEntry(entries, predicate, failure, timeoutMs = 30_000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const match = entries.find(predicate);
    if (match) return match;
    await new Promise(resolveWait => setTimeout(resolveWait, 50));
  }
  throw new Error(failure);
}

function matchingRpcResponse(received, request) {
  return received.find(response => response?.id === request?.id) ?? null;
}

async function selectJourneyRoute(selector, timeoutMs = 15_000) {
  // Shared Path-B selection contract. Post-0a9e552 route_live filtering can
  // leave exactly one live route; the product then disables the selector and
  // auto-selects the sole live market (bba3632 recorded the selectOption
  // wall this caused). Enabled selectors take the exact Path-B option;
  // disabled selectors are adapter-ready only when they expose exactly one
  // option and already carry Path-B. The exact-value assertion never weakens.
  await selector.waitFor({ state: 'visible' });
  const optionInfo = await selector.locator('option').evaluateAll(
    (options, routeId) => ({
      values: options.map(option => option.value),
      hasRoute: options.some(option => option.value === routeId),
    }),
    JOURNEY_ROUTE_ID,
  );
  assert.equal(optionInfo.hasRoute, true, `journey route option ${JOURNEY_ROUTE_ID} is absent`);
  if (await selector.isDisabled()) {
    assert.equal(optionInfo.values.length, 1,
      'disabled NAVCoin market selector must expose exactly one option');
    const deadline = Date.now() + timeoutMs;
    let value = '';
    while (Date.now() < deadline) {
      value = await selector.inputValue().catch(() => '');
      if (value === JOURNEY_ROUTE_ID) return;
      await new Promise(resolveWait => setTimeout(resolveWait, 100));
    }
    assert.equal(value, JOURNEY_ROUTE_ID,
      'disabled NAVCoin market selector must already carry the Path-B route');
    return;
  }
  await selector.selectOption(JOURNEY_ROUTE_ID);
  assert.equal(await selector.inputValue(), JOURNEY_ROUTE_ID, 'wrong NAVCoin journey route selected');
}

async function openPrimaryMarket(page, rpcTraffic) {
  const sentStart = rpcTraffic.sent.length;
  await page.locator('.pf-sidebar .pf-nav').filter({ hasText: 'NAV Markets' }).click();
  const market = page.locator('[data-testid="navcoin-market"]');
  await market.waitFor({ state: 'visible' });
  const selector = market.locator('#navcoin-market-select');
  await selectJourneyRoute(selector);

  const routesRequest = await waitForTrafficEntry(
    rpcTraffic.sent,
    request => request?.method === 'navcoin_bridge_routes',
    'production wallet did not emit navcoin_bridge_routes',
  );
  const routesResponse = await waitForTrafficEntry(
    rpcTraffic.received,
    response => response?.id === routesRequest.id,
    'production wallet did not receive its navcoin_bridge_routes response',
  );
  assert.equal(routesResponse?.result?.schema, 'postfiat-pftl-uniswap-routes-status-v2',
    'route registry response schema mismatch');
  const route = routesResponse.result.routes?.find(row => row?.route_id === JOURNEY_ROUTE_ID);
  assert.ok(route, `journey route ${JOURNEY_ROUTE_ID} is missing from route registry response`);
  assert.match(String(route.native_nav_asset_id || ''), /^[0-9a-f]{96}$/,
    'journey route native NAV asset id is malformed');

  const proofRequest = await waitForTrafficEntry(
    rpcTraffic.sent,
    request => rpcTraffic.sent.indexOf(request) >= sentStart
      && request?.method === 'nav_reserve_proof_status'
      && request?.params?.asset_id === route.native_nav_asset_id,
    'production wallet did not emit the selected-route nav_reserve_proof_status request',
  );
  rpcTraffic.reserveProofRequest = proofRequest;
  return { market, proofRequest };
}

async function waitForReserveProofFrame(rpcFrames) {
  const request = rpcFrames.reserveProofRequest;
  assert.equal(request?.method, 'nav_reserve_proof_status',
    'reserve-proof frame wait requires the selected-route method context');
  return waitForTrafficEntry(
    rpcFrames.received,
    response => response?.id === request.id,
    'production wallet did not receive the method/id-bound reserve-proof response',
  );
}

async function marketBalances(market) {
  // Read each balance row by DOM association: the label span and the value
  // strong are separate nodes. Labels are matched as exact known symbols
  // (case-insensitive); digit-bearing symbols like A666 must never be split
  // at their first digit.
  const rows = await market.locator('.navcoin-primary-balance').all();
  const observed = {};
  for (const row of rows) {
    const label = String(await row.locator('span').first().textContent() || '').replace(/\s+/g, ' ').trim();
    const value = String(await row.locator('strong').first().textContent() || '').replace(/\s+/g, ' ').trim();
    if (label) observed[label] = value;
  }
  assert.ok(Object.keys(observed).some(key => key.toLowerCase() === 'pfusdc'),
    'pfUSDC balance row is absent');
  assert.ok(Object.keys(observed).some(key => key.toLowerCase() === 'a666'),
    'A666 balance row is absent');
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
  // Mode-symmetric validation: the identical predicate registry runs on the
  // same normalized inputs before any mode branch. Build identity comes from
  // the pinned candidate git object, never from mutable worktree bytes.
  const pinnedPackage = pinnedCandidatePackage(REPO_ROOT, CANDIDATE_REVISION);
  const [fireControlHash, setupHash, deploymentHash] = await Promise.all([
    manifestHashObservation(fireControlPath, 'POSTFIAT_R4_FIRE_CONTROL_MANIFEST_SHA256'),
    manifestHashObservation(setupPath, 'POSTFIAT_R4_SETUP_MANIFEST_SHA256'),
    manifestHashObservation(deploymentPath, 'POSTFIAT_R4_DEPLOYMENT_MANIFEST_SHA256'),
  ]);
  const normalizedInputs = {
    fireControl,
    setup,
    deployment,
    productionGraph,
    pinnedPackage,
    fireControlSha256Pin: String(process.env.POSTFIAT_R4_FIRE_CONTROL_MANIFEST_SHA256 || '').trim(),
    fireControlSha256Actual: fireControlHash.actual_sha256,
    setupSha256Pin: String(process.env.POSTFIAT_R4_SETUP_MANIFEST_SHA256 || '').trim(),
    setupSha256Actual: setupHash.actual_sha256,
    deploymentSha256Pin: String(process.env.POSTFIAT_R4_DEPLOYMENT_MANIFEST_SHA256 || '').trim(),
    deploymentSha256Actual: deploymentHash.actual_sha256,
    walletOrigin: origin.href,
    proxyOrigin: proxyOrigin.href,
    asyncBudgetMs,
    exportSlotPath,
    returnSlotPath,
    reportPathAvailable: await stat(reportPath).then(() => false).catch(() => true),
  };
  const validation = runValidationRegistry(normalizedInputs);
  assert.ok(validation.ok,
    `validation registry refused before ${preflightOnly ? 'construction preflight' : 'official launch'}: ${validation.failed.join(', ')}`);
  if (preflightOnly) {
    const asyncProofSlotPath = requiredPath('POSTFIAT_R4_ASYNC_PROOF_SLOT');
    await runConstructionPreflight({
      walletOrigin: origin, proxyOrigin, fireControl, setup, deployment, productionGraph, pinnedPackage,
      fireControlPath, setupPath, deploymentPath,
      exportSlotPath, returnSlotPath, asyncProofSlotPath, proxyRestartPath, proxyPidPath, reportPath,
      manifestHashes: [fireControlHash, setupHash, deploymentHash],
      validationTrace: validation.trace,
    });
    return;
  }
  assert.equal(runContract.mode, 'official');

  const passphrase = randomBytes(24).toString('hex');
  let seed = '';
  const results = [];
  const context = await chromium.launchPersistentContext('', { headless: true, acceptDownloads: true });
  try {
    const page = await context.newPage();
    const rpcTraffic = { sent: [], received: [] };
    page.on('websocket', socket => {
      socket.on('framesent', event => {
        if (typeof event.payload !== 'string') return;
        try { rpcTraffic.sent.push(JSON.parse(event.payload)); } catch { /* non-JSON frames are irrelevant */ }
      });
      socket.on('framereceived', event => {
        if (typeof event.payload !== 'string') return;
        try { rpcTraffic.received.push(JSON.parse(event.payload)); } catch { /* non-JSON frames are irrelevant */ }
      });
    });
    page.setDefaultTimeout(30_000);
    const stepOneBrowser = await observeStepOneBuildIdentity({
      page,
      walletOrigin: origin,
      setup,
      pinnedPackage,
      productionGraph,
      passphrase,
    });
    results.push(stepOneBrowser.step);
    const created = stepOneBrowser.created;
    seed = created.seed;
    results.push(await observeStepTwoWalletShell(page, created));

    const stepThree = await observeStepThreeReserveProof(page, rpcTraffic);
    results.push(stepThree.step);
    const market = stepThree.market;
    results.push(await observeStepFourBalances(stepThree.market, stepThree.marketText));

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

test('A666 R4 v3 build identity hashes the pinned candidate package object, never worktree bytes (closes 66a7171/d9871ce)', async () => {
  const setupPath = join(REPO_ROOT, 'docs/evidence/a666-public-reserve-product-20260803/browser/r4-pass1/setup-endpoints-manifest.json');
  const setup = JSON.parse(await readFile(setupPath, 'utf8'));
  const candidateRevision = manifestCandidateRevision(setup);
  assert.equal(candidateRevision, CANDIDATE_REVISION, 'setup must remain pinned to the successor candidate revision');

  const pinned = pinnedCandidatePackage(REPO_ROOT, candidateRevision);
  assert.equal(pinned.sha256, String(setup.wallet.package_json_sha256).trim().toLowerCase(),
    'setup hash pin must equal the pinned candidate package object hash');
  assert.equal(pinned.version, String(setup.wallet.package_version),
    'setup version pin must equal the pinned candidate package object version');

  const worktreePackageSha256 = await sha256File(join(WALLET_ROOT, 'package.json'));
  assert.notEqual(worktreePackageSha256, pinned.sha256,
    'the mutable worktree package bytes diverge from the pinned candidate object; hashing them is the v3 defect');
  const runnerSource = await readFile(fileURLToPath(import.meta.url), 'utf8');
  const implementation = runnerSource.slice(0, runnerSource.indexOf('function syntheticOfficialInputs()'));
  assert.equal(/sha256File\(join\(WALLET_ROOT, 'package\.json'\)\)/.test(implementation), false,
    'no runner implementation path may hash mutable WALLET_ROOT/package.json for candidate identity');
});

test('A666 R4 step 1 creates and unlocks the wallet before observing the production sidebar version (closes v4 0e2cce3)', async () => {
  const source = await readFile(fileURLToPath(import.meta.url), 'utf8');
  const observerStart = source.indexOf('async function observeStepOneBuildIdentity(');
  const observerEnd = source.indexOf('\nasync function runConstructionPreflight(', observerStart);
  assert.ok(observerStart > -1 && observerEnd > observerStart, 'shared step-1 observer must be present');
  const observer = source.slice(observerStart, observerEnd);
  const createComplete = observer.indexOf('const created = await createWallet(page, passphrase)');
  const sidebarVersionWait = observer.indexOf("page.locator('.pf-sidebar')");
  assert.ok(createComplete > -1 && sidebarVersionWait > -1,
    'shared observer must create/unlock and observe the production sidebar identity');
  assert.ok(createComplete < sidebarVersionWait,
    'create/unlock must complete before the exact .pf-sidebar v0.1.2 visibility wait');

  const officialStart = source.indexOf("assert.equal(runContract.mode, 'official');");
  const officialEnd = source.indexOf("label: 'browser-controlled connect/create'", officialStart);
  const officialStepOne = source.slice(officialStart, officialEnd);
  assert.equal(officialStepOne.split('await observeStepOneBuildIdentity(').length - 1, 1,
    'official execution must invoke the shared step-1 observer exactly once');
});

test('A666 R4 construction browser preflight executes the shared production step-1 observer', async () => {
  const source = await readFile(fileURLToPath(import.meta.url), 'utf8');
  const implementation = source.slice(0, source.indexOf("test('A666 R4 offline Ethereum rehearsal"));
  assert.ok(/async function observeStepOneBuildIdentity\s*\(/.test(implementation),
    'a shared production step-1 observer must own create/unlock then exact sidebar version observation');
  const preflightStart = implementation.indexOf('async function runConstructionPreflight(');
  const preflightEnd = source.indexOf('\nasync function createBrowserControlledWallet(', preflightStart);
  assert.ok(preflightStart > -1 && preflightEnd > preflightStart,
    'construction preflight source region must be present');
  const preflight = source.slice(preflightStart, preflightEnd);
  assert.equal(preflight.split('await observeStepOneBuildIdentity(').length - 1, 1,
    'construction browser preflight must execute the same shared step-1 observer exactly once');
  assert.ok(preflight.includes('await browser.close()'),
    'construction browser context must close in finally');
});

function stepOneBehaviorFixture({ versionText = 'v0.1.2', unlocks = true, versionPresent = true } = {}) {
  const state = { unlocked: false, official_journey_invocations: 0, business_mutations: 0 };
  const versionNode = {
    async waitFor() {
      assert.equal(state.unlocked, true, 'sidebar identity checked before wallet unlock');
      assert.equal(versionPresent, true, 'production sidebar version node absent');
    },
    async textContent() { return versionText; },
  };
  const page = {
    async goto() { return { status: () => 200 }; },
    locator(selector) {
      assert.equal(selector, '.pf-sidebar');
      return {
        getByText(text, options) {
          assert.equal(text, 'v0.1.2');
          assert.deepEqual(options, { exact: true });
          return versionNode;
        },
      };
    },
  };
  const createWallet = async () => {
    state.unlocked = unlocks;
    return { address: 'pf' + '1'.repeat(40), seed: '' };
  };
  return { page, createWallet, state };
}

function stepOneFixtureInputs(vector) {
  return {
    ...vector,
    walletOrigin: new URL('http://127.0.0.1:8080'),
    setup: {
      candidate_revision: CANDIDATE_REVISION,
      wallet: { package_version: '0.1.2', package_json_sha256: 'd'.repeat(64) },
    },
    pinnedPackage: { revision: CANDIDATE_REVISION, version: '0.1.2', sha256: 'd'.repeat(64) },
    productionGraph: { rendered_candidate_graph: true },
    passphrase: 'deterministic-construction-passphrase',
  };
}

test('A666 R4 step-1 construction refuses identity observation before wallet unlock with zero mutations', async () => {
  const vector = stepOneBehaviorFixture({ unlocks: false });
  await assert.rejects(observeStepOneBuildIdentity(stepOneFixtureInputs(vector)),
    /sidebar identity checked before wallet unlock/);
  assert.equal(vector.state.official_journey_invocations, 0);
  assert.equal(vector.state.business_mutations, 0);
});

test('A666 R4 step-1 construction refuses an absent production sidebar version node with zero mutations', async () => {
  const vector = stepOneBehaviorFixture({ versionPresent: false });
  await assert.rejects(observeStepOneBuildIdentity(stepOneFixtureInputs(vector)),
    /production sidebar version node absent/);
  assert.equal(vector.state.official_journey_invocations, 0);
  assert.equal(vector.state.business_mutations, 0);
});

test('A666 R4 step-1 construction refuses wrong exact production sidebar version text with zero mutations', async () => {
  const vector = stepOneBehaviorFixture({ versionText: 'v9.9.9' });
  await assert.rejects(observeStepOneBuildIdentity(stepOneFixtureInputs(vector)),
    /production sidebar package identity must match exactly/);
  assert.equal(vector.state.official_journey_invocations, 0);
  assert.equal(vector.state.business_mutations, 0);
});

test('A666 R4 step-1 construction refuses when onboarding never completes with zero mutations', async () => {
  const vector = stepOneBehaviorFixture();
  vector.createWallet = async () => { throw new Error('wallet onboarding did not complete'); };
  await assert.rejects(observeStepOneBuildIdentity(stepOneFixtureInputs(vector)),
    /wallet onboarding did not complete/);
  assert.equal(vector.state.official_journey_invocations, 0);
  assert.equal(vector.state.business_mutations, 0);
});

function syntheticValidationFixture() {
  // Committed synthetic loopback fixture data for the pure validation suite;
  // no external env block is required.
  return {
    fireControl: { candidate_revision: CANDIDATE_REVISION, ready_to_fire: true },
    setup: {
      candidate_revision: CANDIDATE_REVISION,
      wallet: {
        origin_url: 'http://127.0.0.1:8080',
        package_version: '0.1.2',
        package_json_sha256: 'd'.repeat(64),
      },
    },
    deployment: { candidate_revision: CANDIDATE_REVISION, rpc_url: 'http://127.0.0.1:31001' },
    productionGraph: { import_graph_ok: true },
    pinnedPackage: { revision: CANDIDATE_REVISION, version: '0.1.2', sha256: 'd'.repeat(64) },
    fireControlSha256Pin: 'a'.repeat(64),
    setupSha256Pin: 'b'.repeat(64),
    deploymentSha256Pin: 'c'.repeat(64),
    walletOrigin: 'http://127.0.0.1:8080/',
    proxyOrigin: 'http://127.0.0.1:31021/',
    exportSlotPath: '/synthetic/export-artifact-slot.json',
    returnSlotPath: '/synthetic/return-artifact-slot.json',
    reportPathAvailable: true,
  };
}

function normalizeValidationInputs(mode, fixture) {
  assert.ok(['construction', 'official'].includes(mode), `unknown mode ${mode}`);
  return {
    fireControl: fixture.fireControl,
    setup: fixture.setup,
    deployment: fixture.deployment,
    productionGraph: fixture.productionGraph,
    pinnedPackage: fixture.pinnedPackage,
    fireControlSha256Pin: fixture.fireControlSha256Pin,
    fireControlSha256Actual: fixture.fireControlSha256Pin,
    setupSha256Pin: fixture.setupSha256Pin,
    setupSha256Actual: fixture.setupSha256Pin,
    deploymentSha256Pin: fixture.deploymentSha256Pin,
    deploymentSha256Actual: fixture.deploymentSha256Pin,
    walletOrigin: fixture.walletOrigin,
    proxyOrigin: fixture.proxyOrigin,
    asyncBudgetMs: mode === 'official' ? 3_600_000 : null,
    exportSlotPath: fixture.exportSlotPath,
    returnSlotPath: fixture.returnSlotPath,
    reportPathAvailable: fixture.reportPathAvailable,
  };
}

test('A666 R4 RED-FIRST served candidate NAV Markets exposes visible navcoin market container', async () => {
  const walletOrigin = requiredLoopbackOrigin('POSTFIAT_R4_WALLET_ORIGIN', 'served candidate wallet origin');
  const setup = await readJson(
    join(REPO_ROOT, 'docs/evidence/a666-public-reserve-product-20260803/browser/r4-pass1/setup-endpoints-manifest.json'),
    'setup manifest',
  );
  const pinnedPackage = pinnedCandidatePackage(REPO_ROOT, CANDIDATE_REVISION);
  const productionGraph = await verifyProductionGraph();
  const browser = await chromium.launch({ headless: true });
  try {
    const context = await browser.newContext();
    const page = await context.newPage();
    page.setDefaultTimeout(10_000);
    await observeStepOneBuildIdentity({
      page,
      walletOrigin,
      setup,
      pinnedPackage,
      productionGraph,
      passphrase: 'a666-r4-step3-red-browser-vector',
    });
    await page.locator('.pf-sidebar .pf-nav').filter({ hasText: 'NAV Markets' }).click();
    await page.locator('[data-testid="navcoin-market"]').waitFor({ state: 'visible', timeout: 5_000 });
  } finally {
    await browser.close();
  }
});

test('A666 R4 DOM-IDENTITY healthy NAV Markets exposes exactly one navcoin-market testid', async () => {
  // Same-origin candidate staging (31021): identical env/proxy-auth
  // choreography as runConstructionPreflight, which produced the staged
  // count=2 strict-mode blocker. Read-only; zero mutations.
  const walletOrigin = requiredLoopbackOrigin('POSTFIAT_R4_WALLET_ORIGIN', 'served candidate wallet origin');
  const proxyOrigin = requiredLoopbackOrigin('POSTFIAT_R4_PROXY_ORIGIN', 'candidate proxy origin');
  assert.equal(walletOrigin.href, proxyOrigin.href,
    'DOM-identity healthy vector requires same-origin candidate staging');
  const setup = await readJson(
    join(REPO_ROOT, 'docs/evidence/a666-public-reserve-product-20260803/browser/r4-pass1/setup-endpoints-manifest.json'),
    'setup manifest',
  );
  const pinnedPackage = pinnedCandidatePackage(REPO_ROOT, CANDIDATE_REVISION);
  const productionGraph = await verifyProductionGraph();
  const browser = await chromium.launch({ headless: true });
  try {
    const context = await browser.newContext();
    const page = await context.newPage();
    const rpcTraffic = { sent: [], received: [] };
    page.on('websocket', socket => {
      socket.on('framesent', event => {
        if (typeof event.payload !== 'string') return;
        try { rpcTraffic.sent.push(JSON.parse(event.payload)); } catch { /* non-JSON frames are irrelevant */ }
      });
      socket.on('framereceived', event => {
        if (typeof event.payload !== 'string') return;
        try { rpcTraffic.received.push(JSON.parse(event.payload)); } catch { /* non-JSON frames are irrelevant */ }
      });
    });
    page.setDefaultTimeout(30_000);
    const stepOne = await observeStepOneBuildIdentity({
      page,
      walletOrigin,
      setup,
      pinnedPackage,
      productionGraph,
      passphrase: 'a666-r4-dom-identity-vector',
    });
    await observeStepTwoWalletShell(page, stepOne.created);
    await page.locator('.pf-sidebar .pf-nav').filter({ hasText: 'NAV Markets' }).click();
    // Adapter-mounted assertion BEFORE any testid count: the select lives
    // only inside NavcoinPrimaryMarket, never in the NavcoinMarket wrapper.
    // Shared helper handles the single-live-route disabled-select contract.
    const selector = page.locator('#navcoin-market-select');
    await selectJourneyRoute(selector);
    const routesRequest = await waitForTrafficEntry(
      rpcTraffic.sent,
      request => request?.method === 'navcoin_bridge_routes',
      'production wallet did not emit navcoin_bridge_routes',
    );
    const routesResponse = await waitForTrafficEntry(
      rpcTraffic.received,
      response => response?.id === routesRequest.id,
      'production wallet did not receive its navcoin_bridge_routes response',
    );
    assert.equal(routesResponse?.result?.schema, 'postfiat-pftl-uniswap-routes-status-v2',
      'route registry response schema mismatch');
    const market = page.locator('[data-testid="navcoin-market"]');
    assert.equal(await market.count(), 1,
      'healthy branch must expose exactly one navcoin-market testid');
    await market.waitFor({ state: 'visible' });
  } finally {
    await browser.close();
  }
});

test('A666 R4 DOM-IDENTITY fallback NAV Markets exposes exactly one navcoin-market testid', async () => {
  const distPort = 24000 + (process.pid % 2000);
  const distOrigin = `http://127.0.0.1:${distPort}`;
  const server = spawn('python3', ['-m', 'http.server', String(distPort), '--bind', '127.0.0.1', '--directory', join(WALLET_ROOT, 'dist')], { stdio: 'ignore' });
  const started = Date.now();
  let up = false;
  while (Date.now() - started < 10_000) {
    up = await fetch(`${distOrigin}/`).then((r) => r.ok).catch(() => false);
    if (up) break;
    await new Promise((r) => setTimeout(r, 150));
  }
  assert.ok(up, 'static dist server must come up');
  const browser = await chromium.launch({ headless: true });
  try {
    const context = await browser.newContext();
    const page = await context.newPage();
    page.setDefaultTimeout(10_000);
    // No /rpc on the static server: route registry fetch fails, markets stay empty,
    // NavcoinMarket takes the fallback branch (no NavcoinPrimaryMarket mount).
    await page.goto(distOrigin, { waitUntil: 'domcontentloaded' });
    await createBrowserControlledWallet(page, 'a666-r4-dom-identity-fallback');
    await page.locator('.pf-sidebar .pf-nav').filter({ hasText: 'NAV Markets' }).click();
    const market = page.locator('[data-testid="navcoin-market"]');
    await market.first().waitFor({ state: 'visible', timeout: 5_000 });
    assert.equal(await market.count(), 1, 'fallback branch must expose exactly one navcoin-market testid');
  } finally {
    await browser.close();
    server.kill('SIGTERM');
  }
  await new Promise((r) => setTimeout(r, 300));
  const stillUp = await fetch(`${distOrigin}/`).then(() => true).catch(() => false);
  assert.equal(stillUp, false, 'static dist server port must be closed after the test');
});

test('A666 R4 DOM-IDENTITY vector source guard: both branches present, healthy case never skipped', async () => {
  const source = await readFile(fileURLToPath(import.meta.url), 'utf8');
  assert.ok(source.includes('A666 R4 DOM-IDENTITY healthy NAV Markets exposes exactly one navcoin-market testid'));
  assert.ok(source.includes('A666 R4 DOM-IDENTITY fallback NAV Markets exposes exactly one navcoin-market testid'));
  const healthyStart = source.indexOf("test('A666 R4 DOM-IDENTITY healthy");
  const healthyHeader = source.slice(healthyStart, source.indexOf('async () =>', healthyStart));
  assert.equal(healthyHeader.includes('skip:'), false, 'healthy case must stay an active todo, never skipped');
});

function reserveProofFixture() {
  const assetId = 'a'.repeat(96);
  const request = {
    version: 'postfiat-local-rpc-v1',
    id: 'web-7',
    method: 'nav_reserve_proof_status',
    params: { asset_id: assetId },
  };
  const packet = (identity, sourceCount = 6) => ({
    packet_id: identity,
    reserve_packet_hash: identity.repeat(96),
    state: 'finalized',
    source_count: sourceCount,
  });
  const response = {
    version: 'postfiat-local-rpc-v1',
    id: request.id,
    ok: true,
    result: {
      schema: RESERVE_PROOF_SCHEMA,
      found: true,
      asset_id: assetId,
      packets: [packet('b'), packet('c')],
    },
  };
  return { request, response };
}

function stepThreePageFixture({ selectorVisible = true, selectedRoute = JOURNEY_ROUTE_ID, selectorDisabled = false, deferProofResponse = false } = {}) {
  const routeAssetId = 'a'.repeat(96);
  const routesRequest = { id: 'web-1', method: 'navcoin_bridge_routes', params: {} };
  const routesResponse = {
    id: routesRequest.id,
    ok: true,
    result: {
      schema: 'postfiat-pftl-uniswap-routes-status-v2',
      routes: [{ route_id: JOURNEY_ROUTE_ID, native_nav_asset_id: routeAssetId }],
    },
  };
  const traffic = { sent: [routesRequest], received: [routesResponse] };
  const calls = { selectOption: 0 };
  let currentValue = selectorDisabled ? selectedRoute : '';
  let proofPushed = false;
  let deferredProofResponse = null;
  const pushProof = () => {
    if (proofPushed) return;
    proofPushed = true;
    const proof = reserveProofFixture();
    proof.request.params.asset_id = routeAssetId;
    traffic.sent.push(proof.request);
    if (deferProofResponse) {
      deferredProofResponse = proof.response;
    } else {
      traffic.received.push(proof.response);
    }
  };
  const deliverProofResponse = () => {
    if (deferredProofResponse) {
      traffic.received.push(deferredProofResponse);
      deferredProofResponse = null;
    }
  };
  let marketText = 'packet unavailable or mismatched Verified NAV';
  const setMarketText = (text) => { marketText = text; };
  const selector = {
    async waitFor() {
      if (!selectorVisible) throw new Error('primary market adapter selector is absent');
    },
    locator(name) {
      assert.equal(name, 'option');
      return { async evaluateAll(callback, routeId) { return callback([{ value: JOURNEY_ROUTE_ID }], routeId); } };
    },
    async isDisabled() { return selectorDisabled; },
    async selectOption(routeId) {
      calls.selectOption += 1;
      currentValue = selectedRoute;
      pushProof();
      return routeId;
    },
    async inputValue() {
      if (currentValue === JOURNEY_ROUTE_ID) pushProof();
      return currentValue;
    },
  };
  const market = {
    async waitFor() {},
    async textContent() { return marketText; },
    locator(name) {
      assert.equal(name, '#navcoin-market-select');
      return selector;
    },
  };
  const page = {
    locator(name) {
      if (name === '.pf-sidebar .pf-nav') {
        return { filter() { return { async click() {} }; } };
      }
      assert.equal(name, '[data-testid="navcoin-market"]');
      return market;
    },
  };
  return { page, traffic, selectorCalls: calls, setMarketText, deliverProofResponse };
}

function balanceMarketFixture(rows) {
  return {
    locator(name) {
      assert.equal(name, '.navcoin-primary-balance');
      return {
        async all() {
          return rows.map(([label, value]) => ({
            locator(part) {
              return { first() { return { async textContent() { return part === 'span' ? label : value; } }; } };
            },
          }));
        },
      };
    },
  };
}

test('A666 R4 step-3 observer requires the bound proof frame before the exact match text, and the post-proof DOM rerender', async () => {
  // RED-first record: construction sampled market text before the proof
  // response/refresh landed, recording proof_rendered=false against a staged
  // backend whose packet hashes match exactly (diagnosis A).
  const ordered = stepThreePageFixture({ deferProofResponse: true });
  ordered.setMarketText('RESERVE PACKET matches finalized PFTL reserve proof Verified NAV YOUR BALANCES finalized');
  const pending = observeStepThreeReserveProof(ordered.page, ordered.traffic);
  let settled = false;
  pending.then(() => { settled = true; }, () => { settled = true; });
  await new Promise(resolveWait => setTimeout(resolveWait, 300));
  assert.equal(settled, false, 'match text alone cannot satisfy the observer before the bound proof frame');
  ordered.deliverProofResponse();
  const observed = await pending;
  assert.equal(observed.step.checks.proof_rendered, true);
  assert.match(observed.marketText, /matches finalized PFTL reserve proof/);

  // Pre-proof text never satisfies: initial DOM says unavailable, the bound
  // proof arrives, and only the later rerender completes the observation.
  const rerender = stepThreePageFixture();
  rerender.setMarketText('packet unavailable or mismatched Verified NAV');
  const pendingRerender = observeStepThreeReserveProof(rerender.page, rerender.traffic);
  await new Promise(resolveWait => setTimeout(resolveWait, 300));
  rerender.setMarketText('RESERVE PACKET matches finalized PFTL reserve proof Verified NAV YOUR BALANCES finalized');
  const rerendered = await pendingRerender;
  assert.equal(rerendered.step.checks.proof_rendered, true);

  // Fail-closed: text that never rerenders to the exact match is rejected.
  const stale = stepThreePageFixture();
  stale.setMarketText('packet unavailable or mismatched Verified NAV');
  await assert.rejects(
    waitForMarketText(stale.page.locator('[data-testid="navcoin-market"]'), 'matches finalized PFTL reserve proof', 400),
    /market did not render exact text/,
  );
});

test('A666 R4 balance observer reads row labels by DOM association and recognizes digit-bearing symbols', async () => {
  // RED-first record: first-digit label splitting parsed the A666 row label
  // as "A", failing step 4 deterministically (diagnosis A).
  const observed = await marketBalances(balanceMarketFixture([
    ['PFUSDC', '0'], ['A666', '1'], ['wA666 · MetaMask', '0'],
  ]));
  assert.equal(observed['A666'], '1', 'digit-bearing A666 label must be read whole');
  assert.equal(observed['PFUSDC'], '0');
  await assert.rejects(
    marketBalances(balanceMarketFixture([['PFUSDC', '0'], ['A66', '5']])),
    /A666 balance row is absent/,
  );
  await assert.rejects(
    marketBalances(balanceMarketFixture([['PFUSDC', '0'], ['wA666 · MetaMask', '0']])),
    /A666 balance row is absent/,
  );
  await assert.rejects(
    marketBalances(balanceMarketFixture([['A666', '1']])),
    /pfUSDC balance row is absent/,
  );
});

function selectionHelperFixture({ disabled = false, options = [JOURNEY_ROUTE_ID], value = '' } = {}) {
  let currentValue = value;
  const calls = { selectOption: 0 };
  const selector = {
    async waitFor() {},
    locator(name) {
      assert.equal(name, 'option');
      return {
        async evaluateAll(callback, routeId) {
          return callback(options.map(optionValue => ({ value: optionValue })), routeId);
        },
      };
    },
    async isDisabled() { return disabled; },
    async selectOption(routeId) { calls.selectOption += 1; currentValue = routeId; },
    async inputValue() { return currentValue; },
  };
  return { selector, calls };
}

test('A666 R4 selection helper single-live-route contract: disabled exact Path-B ready, wrong value and multi-option rejected, enabled multi-route still selects', async () => {
  // RED-first record: bba3632 documented the selectOption wall when route_live
  // filtering leaves one route and the product correctly disables the
  // selector; the DOM-identity healthy run reproduced it against 31021.
  const ready = selectionHelperFixture({ disabled: true, value: JOURNEY_ROUTE_ID });
  await selectJourneyRoute(ready.selector);
  assert.equal(ready.calls.selectOption, 0, 'disabled selector must never receive selectOption');

  const wrongValue = selectionHelperFixture({ disabled: true, value: 'wrong-route' });
  await assert.rejects(selectJourneyRoute(wrongValue.selector, 200), /must already carry the Path-B route/);
  assert.equal(wrongValue.calls.selectOption, 0);

  const multiOption = selectionHelperFixture({ disabled: true, options: [JOURNEY_ROUTE_ID, 'other-route'], value: JOURNEY_ROUTE_ID });
  await assert.rejects(selectJourneyRoute(multiOption.selector, 200), /exactly one option/);

  const enabled = selectionHelperFixture({ options: ['other-route', JOURNEY_ROUTE_ID] });
  await selectJourneyRoute(enabled.selector);
  assert.equal(enabled.calls.selectOption, 1, 'enabled selector must select exactly once');
  assert.equal(await enabled.selector.inputValue(), JOURNEY_ROUTE_ID);

  const enabledMissing = selectionHelperFixture({ options: ['other-route'] });
  await assert.rejects(selectJourneyRoute(enabledMissing.selector, 200), /is absent/);

  // End-to-end through openPrimaryMarket: disabled single-option Path-B
  // selector is adapter-ready and the proof binding still completes.
  const disabledReady = stepThreePageFixture({ selectorDisabled: true });
  const opened = await openPrimaryMarket(disabledReady.page, disabledReady.traffic);
  assert.equal(disabledReady.selectorCalls.selectOption, 0,
    'openPrimaryMarket must not call selectOption on a disabled selector');
  assert.equal(opened.proofRequest.method, 'nav_reserve_proof_status');
});

test('DEFECT-13 RED-FIRST: fresh self-custody wallet creates pfUSDC and A666 trustlines through production UI', {
  todo: process.env.POSTFIAT_REQUIRE_ADD_ASSET === '1'
    ? false
    : 'gated RED: [data-testid="add-asset"] control is absent on the current candidate; set POSTFIAT_REQUIRE_ADD_ASSET=1 to enforce',
}, async () => {
  // Bindings taken from the committed construction launcher contract: the
  // same-origin candidate stage (template default 127.0.0.1:31021) and the
  // persistent fresh-wallet profile staged by scripts/a666-r4-fresh-wallet-stage
  // (3d548d5). Seed material is never exported, read, or logged; the raw
  // address is asserted by equality and never printed.
  const origin = new URL(process.env.POSTFIAT_R4_WALLET_ORIGIN || 'http://127.0.0.1:31021');
  assert.ok(['127.0.0.1', 'localhost', '::1'].includes(origin.hostname), 'DEFECT-13 origin must be loopback');
  assert.ok(origin.port, 'DEFECT-13 origin must include an explicit port');
  const runDir = '/home/postfiat/.pft/a666-r4-fresh-wallet-v7';
  const ready = JSON.parse(await readFile(join(runDir, 'fresh-wallet-ready.json'), 'utf8'));
  const passphrase = (await readFile(join(runDir, 'wallet-passphrase.local'), 'utf8')).trim();
  assert.match(String(ready.address || ''), /^pf[0-9a-f]{40}$/,
    'funding-target ready file must carry the staged wallet address');
  // Exact staged assets of the rehearsal route registry
  // (pftl-a666-r4-offline-rehearsal-v1; values cross-checked against the
  // committed fleet/route evidence): settlement pfUSDC then native A666.
  const stagedAssets = [
    { symbol: 'pfUSDC', assetId: '02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b' },
    { symbol: 'A666', assetId: '521c6c630bb48d4a37ab4a7bd4900dd2caa2d9e99499e452da3c7ce75b3d74b62d20e18555642bec32174498cbee5e2c' },
  ];
  for (const asset of stagedAssets) {
    assert.match(asset.assetId, /^[0-9a-f]{96}$/, `staged ${asset.symbol} asset id is malformed`);
  }
  const context = await chromium.launchPersistentContext(join(runDir, 'browser-profile'), { headless: true });
  try {
    const page = context.pages()[0] ?? await context.newPage();
    page.setDefaultTimeout(15_000);
    const response = await page.goto(origin.href, { waitUntil: 'domcontentloaded' });
    assert.equal(response?.status(), 200, 'staged candidate wallet origin must return HTTP 200');
    // domcontentloaded resolves before the React lock screen renders; unlock()
    // would otherwise observe a not-yet-visible lock node and skip the
    // passphrase step. Wait for either terminal state first.
    await Promise.race([
      page.locator('input[placeholder="Passphrase"]').waitFor({ state: 'visible' }).catch(() => {}),
      page.locator('.pf-shell').waitFor({ state: 'visible' }).catch(() => {}),
    ]);
    await unlock(page, passphrase);
    const recovered = await page.evaluate(() => new Promise((resolveAddress, rejectAddress) => {
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
    assert.equal(String(recovered), String(ready.address),
      'persisted profile must recover the staged wallet identity (hash-compared, never printed)');
    for (const asset of stagedAssets) {
      const control = page.locator('[data-testid="add-asset"]');
      const controlVisible = await control.first()
        .waitFor({ state: 'visible', timeout: 5_000 }).then(() => true).catch(() => false);
      assert.ok(controlVisible,
        `DEFECT-13: [data-testid="add-asset"] control is absent on the production wallet UI; ${asset.symbol} trustline creation is unreachable for the fresh self-custody wallet`);
      // Future (post product fix): select the exact staged asset by assetId,
      // submit the user-signed trustline through the browser wallet, and
      // observe a finalized trustline receipt. Unreachable on the current
      // candidate; zero submissions and zero ledger mutations today.
    }
  } finally {
    await context.close();
  }
});

test('A666 R4 step-3 choreography rejects wrapper-only readiness and binds proof method/request identity', async () => {
  const historical = execFileSync(
    'git',
    ['show', '84fa197:wallet-web/src/lib/a666-r4-offline-ethereum-rehearsal.e2e.js'],
    { cwd: REPO_ROOT, encoding: 'utf8' },
  );
  const oldStart = historical.indexOf('async function openPrimaryMarket(page)');
  const oldEnd = historical.indexOf('\nasync function waitForReserveProofFrame', oldStart);
  const oldOpen = historical.slice(oldStart, oldEnd);
  assert.ok(oldOpen.includes('[data-testid="navcoin-market"]'), '84fa197 waited for the shared wrapper');
  assert.equal(oldOpen.includes('#navcoin-market-select'), false,
    '84fa197 incorrectly treated wrapper visibility as adapter readiness');

  const fallback = stepThreePageFixture({ selectorVisible: false });
  await assert.rejects(openPrimaryMarket(fallback.page, fallback.traffic), /adapter selector is absent/);
  const wrongRoute = stepThreePageFixture({ selectedRoute: 'wrong-route' });
  await assert.rejects(openPrimaryMarket(wrongRoute.page, wrongRoute.traffic), /wrong NAVCoin journey route selected/);

  const valid = reserveProofFixture();
  const packetShapedUnrelated = structuredClone(valid.response);
  packetShapedUnrelated.id = 'web-unrelated';
  assert.equal(matchingRpcResponse([packetShapedUnrelated], valid.request), null,
    'packet-shaped unrelated frame must be rejected');
  const wrongId = structuredClone(valid.response);
  wrongId.id = 'web-8';
  assert.equal(matchingRpcResponse([wrongId], valid.request), null,
    'wrong request id must be rejected');

  const sourceFive = reserveProofFixture();
  sourceFive.response.result.packets[0].source_count = 5;
  assert.throws(() => reserveProofContract(sourceFive.request, sourceFive.response), /source_count 6/);
  const duplicate = reserveProofFixture();
  duplicate.response.result.packets[1].reserve_packet_hash = duplicate.response.result.packets[0].reserve_packet_hash;
  assert.throws(() => reserveProofContract(duplicate.request, duplicate.response), /two distinct packet\/proof identities/);
  const accepted = reserveProofContract(valid.request, valid.response);
  assert.equal(accepted.finalized.length, 2);
  assert.equal(new Set(accepted.proofIdentities).size, 2);
});

test('A666 R4 mode parity: construction and official execute the identical 18-predicate registry with identical traces', () => {
  const fixture = syntheticValidationFixture();
  const construction = runValidationRegistry(normalizeValidationInputs('construction', fixture));
  const official = runValidationRegistry(normalizeValidationInputs('official', fixture));
  assert.equal(construction.ok, true, `construction refused: ${construction.failed.join(', ')}`);
  assert.equal(official.ok, true, `official refused: ${official.failed.join(', ')}`);
  assert.equal(VALIDATION_PREDICATE_REGISTRY.length, 18);
  assert.equal(construction.trace.length, 18);
  assert.equal(official.trace.length, 18);
  assert.deepEqual(official.trace, construction.trace,
    'construction and official validation traces must have identical predicate id set, order, and results');
  assert.equal(construction.official_journey_invocations, 0);
  assert.equal(official.business_mutations, 0);
});

test('A666 R4 structural guard: official mode introduces no validation outside the shared registry', async () => {
  const source = await readFile(fileURLToPath(import.meta.url), 'utf8');
  const registryBlock = source.match(/const VALIDATION_PREDICATE_REGISTRY = Object\.freeze\(\[([\s\S]*?)\]\);/);
  assert.ok(registryBlock, 'validation predicate registry must exist');
  const ids = [...registryBlock[1].matchAll(/id: '([a-z0-9_]+)'/g)].map(match => match[1]);
  assert.equal(ids.length, 18);
  assert.equal(new Set(ids).size, ids.length, 'duplicate predicate ids');

  const needle = 'runValidationRegistry' + '(normalizedInputs)';
  const registryRun = source.indexOf(`const validation = ${needle};`);
  assert.ok(registryRun > -1, 'the runner must execute the shared registry');
  const modeBranch = source.indexOf('if (preflightOnly) {', registryRun);
  assert.ok(modeBranch > registryRun,
    'the shared registry must execute before the runner mode branch');
  assert.equal(source.split(`const validation = ${needle};`).length - 1, 1,
    'the runner must execute the registry exactly once');

  const officialStart = source.indexOf("assert.equal(runContract.mode, 'official');");
  const browserLaunch = source.indexOf('chromium.launchPersistentContext');
  assert.ok(officialStart > -1 && browserLaunch > officialStart);
  const officialPreLaunch = source.slice(source.indexOf(';', officialStart) + 1, browserLaunch);
  assert.deepEqual(officialPreLaunch.match(/assert\./g) ?? [], [],
    'official pre-launch validation outside the registry is forbidden');

  const implementation = source.slice(0, source.indexOf('function syntheticOfficialInputs()'));
  assert.equal(/sha256File\(join\(WALLET_ROOT, 'package\.json'\)\)/.test(implementation), false,
    'mutable worktree package hashing is forbidden in the runner implementation');
  const fixture = syntheticValidationFixture();
  const traceIds = runValidationRegistry(normalizeValidationInputs('construction', fixture)).trace
    .map(entry => entry.id);
  assert.deepEqual(traceIds, ids, 'construction trace must cover every registry predicate in registry order');
});

test('A666 R4 steps 1-4 display predicates are mode-shared: structural completeness guard with RED proof and negatives', async () => {
  const source = await readFile(fileURLToPath(import.meta.url), 'utf8');
  assert.deepEqual(sharedDisplayCoverageProblems(source), [],
    'current source: every step 1-4 display predicate must execute in both modes');

  // RED proof: the pre-fix source (49e4ce8, step-1-only construction
  // browser preflight) must fail this guard. The v5 official run reached
  // step 3 with zero construction coverage of its predicates (20e0ee8).
  const historical = execFileSync(
    'git',
    ['show', '49e4ce8:wallet-web/src/lib/a666-r4-offline-ethereum-rehearsal.e2e.js'],
    { cwd: REPO_ROOT, encoding: 'utf8' },
  );
  const historicalProblems = sharedDisplayCoverageProblems(historical);
  assert.ok(historicalProblems.some(problem => problem.includes('construction preflight missing shared observer: observeStepTwoWalletShell')),
    'pre-fix source lacked the step-2 construction observer');
  assert.ok(historicalProblems.some(problem => problem.includes('construction preflight missing shared observer: observeStepThreeReserveProof')),
    'pre-fix source lacked the step-3 construction observer');
  assert.ok(historicalProblems.some(problem => problem.includes('construction preflight missing shared observer: observeStepFourBalances')),
    'pre-fix source lacked the step-4 construction observer');
  assert.ok(historicalProblems.some(problem => problem.includes('inline')),
    'pre-fix official journey defined step 2-4 predicates inline');

  // Negative 1: an official-only step 1-4 predicate (inline observedStep in
  // the official display span) fails the guard.
  const officialOnly = source.replace(
    'results.push(await observeStepTwoWalletShell(page, created));',
    "results.push(await observeStepTwoWalletShell(page, created));\n    results.push(observedStep(3, { label: 'x' }, { official_only: true }));",
  );
  assert.ok(sharedDisplayCoverageProblems(officialOnly)
    .some(problem => problem.includes('inline')), 'official-only predicate must fail');

  // Negative 2: a missing construction observer fails the guard.
  const missingObserver = source.replace(
    'await observeStepThreeReserveProof(page, rpcTraffic)',
    'await Promise.resolve(null)',
  );
  assert.ok(sharedDisplayCoverageProblems(missingObserver)
    .some(problem => problem.includes('construction preflight missing shared observer: observeStepThreeReserveProof')),
    'missing construction observer must fail');

  // Negative 3: a mutation step entering construction fails the guard.
  const mutationInConstruction = source.replace(
    'await browser.close();',
    'await completeTransparentRoundTrip(page, market);\n    await browser.close();',
  );
  assert.ok(sharedDisplayCoverageProblems(mutationInConstruction)
    .some(problem => problem.includes('mutation/dependent step reachable in construction preflight')),
    'mutation step in construction must fail');

  // Permanent guard: the browser-level product render vector survives the
  // RED-to-GREEN transition. Its TODO marker may be removed; the vector,
  // shared observer, production navigation click, and exact container wait may not.
  const vectorName = 'A666 R4 RED-FIRST served candidate NAV Markets' + ' exposes visible navcoin market container';
  const vectorStart = source.indexOf(`test('${vectorName}'`);
  const vectorEnd = source.indexOf("\ntest('", vectorStart + 1);
  assert.ok(vectorStart > -1 && vectorEnd > vectorStart,
    'served-candidate navcoin-market browser vector must remain registered');
  const vector = source.slice(vectorStart, vectorEnd);
  assert.equal(/skip\s*:/.test(vector), false, 'served-candidate navcoin-market vector may not be skipped');
  assert.ok(vector.includes('observeStepOneBuildIdentity({'), 'browser vector must reuse the shared step-1 observer');
  assert.ok(vector.includes("hasText: 'NAV Markets'"), 'browser vector must click the production NAV Markets control');
  assert.ok(vector.includes("page.locator('[data-testid=\"navcoin-market\"]')"),
    'browser vector must require the exact production render container');
});

test('A666 R4 build identity negatives refuse before invocation: nonexistent revision/object, wrong hash/version, worktree divergence', async () => {
  assert.throws(() => pinnedCandidatePackage(REPO_ROOT, '0'.repeat(40)),
    /Command failed|spawnSync|exit/i, 'nonexistent revision must refuse');
  assert.throws(() => execFileSync('git', ['show', `${CANDIDATE_REVISION}:wallet-web/does-not-exist.json`], { cwd: REPO_ROOT }),
    /Command failed|exit/i, 'nonexistent git object must refuse');

  const fixture = syntheticValidationFixture();
  const wrongHash = structuredClone(fixture);
  wrongHash.setup.wallet.package_json_sha256 = '0'.repeat(64);
  const hashResult = runValidationRegistry(normalizeValidationInputs('official', wrongHash));
  assert.equal(hashResult.ok, false);
  assert.ok(hashResult.failed.includes('wallet_package_json_sha256_consistent_with_pinned_object'));
  assert.equal(hashResult.official_journey_invocations, 0);
  assert.equal(hashResult.business_mutations, 0);

  const wrongVersion = structuredClone(fixture);
  wrongVersion.setup.wallet.package_version = '9.9.9';
  const versionResult = runValidationRegistry(normalizeValidationInputs('official', wrongVersion));
  assert.equal(versionResult.ok, false);
  assert.ok(versionResult.failed.includes('wallet_package_version_consistent_with_pinned_object'));
  assert.equal(versionResult.official_journey_invocations, 0);

  // Mutable checkout divergence: the setup pin equals the (fake) current
  // worktree hash but the pinned candidate object carries different bytes.
  const diverged = structuredClone(fixture);
  diverged.pinnedPackage = { ...fixture.pinnedPackage, sha256: 'e'.repeat(64) };
  const divergedResult = runValidationRegistry(normalizeValidationInputs('official', diverged));
  assert.equal(divergedResult.ok, false);
  assert.ok(divergedResult.failed.includes('wallet_package_json_sha256_consistent_with_pinned_object'));
  assert.equal(divergedResult.official_journey_invocations, 0);
});

test('A666 R4 construction official-readiness fixture binds current committed inputs before fire (no mutation)', async () => {
  const evRoot = join(REPO_ROOT, 'docs/evidence/a666-public-reserve-product-20260803/browser');
  const paths = {
    fireControl: join(evRoot, 'r4-pass1/journey-fire-control-preflight.json'),
    setup: join(evRoot, 'r4-pass1/setup-endpoints-manifest.json'),
    deployment: join(evRoot, 'r4-construction/ethereum-contract-stage.json'),
  };
  const [fireControl, setup, deployment, fireControlSha256, setupSha256, deploymentSha256] = await Promise.all([
    readJson(paths.fireControl, 'fire-control manifest'),
    readJson(paths.setup, 'setup manifest'),
    readJson(paths.deployment, 'deployment manifest'),
    sha256File(paths.fireControl),
    sha256File(paths.setup),
    sha256File(paths.deployment),
  ]);
  for (const hash of [fireControlSha256, setupSha256, deploymentSha256]) {
    assert.match(hash, /^[0-9a-f]{64}$/, 'current committed input must have a SHA-256 binding');
  }
  assert.equal(fireControl.ready_to_fire, true, 'readiness fixture requires computed GREEN fire control');

  const readinessFixture = {
    fireControl,
    setup,
    deployment,
    productionGraph: { import_graph_ok: true },
    pinnedPackage: pinnedCandidatePackage(REPO_ROOT, CANDIDATE_REVISION),
    fireControlSha256Pin: fireControlSha256,
    setupSha256Pin: setupSha256,
    deploymentSha256Pin: deploymentSha256,
    walletOrigin: 'http://127.0.0.1:8080/',
    proxyOrigin: 'http://127.0.0.1:31021/',
    exportSlotPath: join(evRoot, 'r4-construction/export-artifact-slot.json'),
    returnSlotPath: join(evRoot, 'r4-construction/return-artifact-slot.json'),
    reportPathAvailable: true,
  };
  const construction = runValidationRegistry(normalizeValidationInputs('construction', readinessFixture));
  const official = runValidationRegistry(normalizeValidationInputs('official', readinessFixture));
  assert.equal(construction.ok, true, `readiness fixture refused in construction: ${construction.failed.join(', ')}`);
  assert.equal(official.ok, true, `readiness fixture refused in official: ${official.failed.join(', ')}`);
  assert.deepEqual(official.trace, construction.trace,
    'readiness traces must be identical across modes');
  assert.equal(construction.official_journey_invocations, 0);
  assert.equal(construction.business_mutations, 0);

  // Semantic binding: the dynamic hashes must bind reality, not arbitrary
  // bytes. Candidate revision, loopback shape, and readiness semantics are
  // validated independently of the computed pins.
  assert.equal(manifestCandidateRevision(fireControl), CANDIDATE_REVISION);
  assert.equal(manifestCandidateRevision(setup), CANDIDATE_REVISION);
  assert.equal(manifestCandidateRevision(deployment), CANDIDATE_REVISION);
  assert.equal(loopbackManifestObservation('setup manifest', setup).valid, true);
  assert.equal(loopbackManifestObservation('deployment manifest', deployment).valid, true);
  for (const id of ['candidate_revision_fire_control', 'candidate_revision_setup',
    'candidate_revision_deployment', 'setup_manifest_loopback', 'deployment_manifest_loopback',
    'fire_control_ready_to_fire', 'fire_control_hash_pinned', 'setup_hash_pinned',
    'deployment_hash_pinned', 'report_path_available', 'production_import_graph']) {
    assert.equal(official.trace.find(item => item.id === id)?.ok, true,
      `${id} must bind green on the current committed inputs`);
  }

  // Non-vacuity: the current step-1 inputs carry real wallet identity pins
  // and the package-pin predicates bind them against the pinned git object.
  assert.match(String(setup.wallet.package_version), /^\d+\.\d+\.\d+$/, 'readiness fixture must carry the version pin');
  assert.match(String(setup.wallet.package_json_sha256), /^[0-9a-f]{64}$/, 'readiness fixture must carry the hash pin');
  for (const id of ['wallet_package_version_consistent_with_pinned_object',
    'wallet_package_json_sha256_consistent_with_pinned_object']) {
    const entry = official.trace.find(item => item.id === id);
    assert.equal(entry.ok, true, `${id} must be green on the current committed inputs`);
    const flipped = structuredClone(readinessFixture);
    flipped.setup = structuredClone(setup);
    if (id.startsWith('wallet_package_version')) flipped.setup.wallet.package_version = '9.9.9';
    else flipped.setup.wallet.package_json_sha256 = '0'.repeat(64);
    const flippedResult = runValidationRegistry(normalizeValidationInputs('official', flipped));
    assert.equal(flippedResult.ok, false, `${id} must refuse a flipped pin (non-vacuous)`);
    assert.ok(flippedResult.failed.includes(id), `${id} must be the refusing predicate`);
    assert.equal(flippedResult.official_journey_invocations, 0);
  }
});

test('A666 R4 candidate repin negatives: stale 39 setup, any cross-manifest revision mismatch, and watcher divergence refuse; exact 23 passes', async () => {
  const evRoot = join(REPO_ROOT, 'docs/evidence/a666-public-reserve-product-20260803/browser');
  const [fireControl, setup, deployment, productionGraph] = await Promise.all([
    readJson(join(evRoot, 'r4-pass1/journey-fire-control-preflight.json'), 'fire-control manifest'),
    readJson(join(evRoot, 'r4-pass1/setup-endpoints-manifest.json'), 'setup manifest'),
    readJson(join(evRoot, 'r4-construction/ethereum-contract-stage.json'), 'deployment manifest'),
    verifyProductionGraph(),
  ]);
  const baseFixture = {
    fireControl,
    setup,
    deployment,
    productionGraph,
    pinnedPackage: pinnedCandidatePackage(REPO_ROOT, CANDIDATE_REVISION),
    fireControlSha256Pin: '',
    fireControlSha256Actual: 'a'.repeat(64),
    setupSha256Pin: '',
    setupSha256Actual: 'b'.repeat(64),
    deploymentSha256Pin: '',
    deploymentSha256Actual: 'c'.repeat(64),
    walletOrigin: 'http://127.0.0.1:8080/',
    proxyOrigin: 'http://127.0.0.1:31021/',
    asyncBudgetMs: null,
    exportSlotPath: join(evRoot, 'r4-construction/export-artifact-slot.json'),
    returnSlotPath: join(evRoot, 'r4-construction/return-artifact-slot.json'),
    reportPathAvailable: true,
  };
  // Exact 23 baseline passes (hash pins blank: hash predicates are exercised
  // by the readiness fixture test with live sha256File bindings).
  for (const manifest of [fireControl, setup, deployment]) {
    assert.equal(manifestCandidateRevision(manifest), CANDIDATE_REVISION,
      'committed manifest must pin the exact successor candidate');
  }

  const STALE_39 = '39f7fae3191aa34c376ae1657650a9ec2444f421';
  const mismatchCases = [
    ['candidate_revision_setup', 'setup', 'old39 setup against new23 fire-control'],
    ['candidate_revision_fire_control', 'fireControl', 'new23 setup against old39 fire-control'],
    ['candidate_revision_deployment', 'deployment', 'old39 deployment against new23 runner'],
  ];
  for (const [predicateId, key, label] of mismatchCases) {
    const flipped = structuredClone(baseFixture);
    if (flipped[key].candidate) flipped[key].candidate.revision = STALE_39;
    if (flipped[key].candidate_revision) flipped[key].candidate_revision = STALE_39;
    if (flipped[key].aggregation?.current_binding?.candidate_revision) {
      flipped[key].aggregation.current_binding.candidate_revision = STALE_39;
    }
    if (flipped[key].bindings?.candidate_revision) flipped[key].bindings.candidate_revision = STALE_39;
    assert.equal(manifestCandidateRevision(flipped[key]), STALE_39, `${label}: fixture flip must take effect`);
    const result = runValidationRegistry(normalizeValidationInputs('official', flipped));
    assert.equal(result.ok, false, `${label} must refuse`);
    assert.ok(result.failed.includes(predicateId), `${predicateId} must be the refusing predicate`);
    assert.equal(result.official_journey_invocations, 0);
  }

  // Watcher binding: the deployed restart watcher must pin the identical
  // successor revision or the runner/watcher pair is divergent.
  const watcherSource = await readFile(join(REPO_ROOT, 'scripts/a666-r4-candidate-proxy-restart-watch'), 'utf8');
  assert.ok(watcherSource.includes(`CANDIDATE_REVISION="${CANDIDATE_REVISION}"`),
    'restart watcher must pin the exact successor candidate revision');
  assert.equal(watcherSource.includes(`CANDIDATE_REVISION="${STALE_39}"`), false,
    'restart watcher must not retain the stale 39 pin');
});

test('A666 R4 readiness fixture carries no inline historical artifact hash literals (structural negative)', async () => {
  const source = await readFile(fileURLToPath(import.meta.url), 'utf8');
  const start = source.indexOf("test('A666 R4 construction official-readiness fixture");
  assert.ok(start > -1, 'readiness test must exist');
  const end = source.indexOf("\ntest('", start + 1);
  assert.ok(end > start, 'readiness test span must terminate');
  const span = source.slice(start, end);
  assert.deepEqual(span.match(/'[0-9a-f]{64}'/g) ?? [], [],
    'readiness fixture must bind fire-control/setup/deployment dynamically via sha256File(); inline historical hash literals are forbidden');
  assert.ok(span.split('sha256File(paths.').length - 1 >= 3,
    'all three official input artifacts must be hashed from their committed paths at test time');
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
