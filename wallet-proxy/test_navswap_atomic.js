'use strict';

const assert = require('assert');
const crypto = require('crypto');

const PFUSDC_ASSET_ID = '34ce77d07099872d5691ead3842bfb3d6cc8678ff62cc68d887dad7f8645128351e72b9ae76f88ed1854a5e8d3372c8b';
const A651_ASSET_ID = '8584aa713209eb8253293c891f7269e35841f004080e06414db019f868610e9cb57dfb7aca3d51427fbe369b6ebde127';
const THIRD_ASSET_ID = '0'.repeat(96);
process.env.PFUSDC_ASSET_ID = PFUSDC_ASSET_ID;
process.env.A651_ASSET_ID = A651_ASSET_ID;

const atomicModule = require('./navswap-atomic');
const {
  RPC_CAPS,
  atomicRpcProxyError,
  executeNavswapCapabilities,
  executeNavswapQuote,
  executeNavswapRun,
  isFinalityMethod,
  isSequencedAccountMethod,
  parseListenHost,
  requestWithProxyReadiness,
} = require('./server');

const OWNER_0 = `pf${'1'.repeat(40)}`;
const OWNER_1 = `pf${'2'.repeat(40)}`;
const ISSUER_0 = `pf${'a'.repeat(40)}`;
const ISSUER_1 = `pf${'b'.repeat(40)}`;
const PARENT_HASH = 'a'.repeat(96);
const PARENT_ROOT = 'b'.repeat(96);
const TX_ID = '6'.repeat(96);
const RPC_FLEET = Array.from({ length: 6 }, (_, index) => ({
  validatorId: `validator-${index}`,
  host: `validator-${index}.test`,
  port: 27650 + index,
}));
const PROXY_CONFIGURATION_HASH = 'd8187c2f1f899834957f7cf7c2eb480a7e59059c87fc46ac092487b13d05d292';

function stableJson(value) {
  if (Array.isArray(value)) return `[${value.map(stableJson).join(',')}]`;
  if (value && typeof value === 'object') {
    return `{${Object.keys(value).sort().map((key) => (
      `${JSON.stringify(key)}:${stableJson(value[key])}`
    )).join(',')}}`;
  }
  return JSON.stringify(value);
}

function testExplicitProxyListenHost() {
  assert.strictEqual(parseListenHost(undefined), '127.0.0.1');
  assert.strictEqual(parseListenHost('127.0.0.1'), '127.0.0.1');
  assert.strictEqual(parseListenHost('::1'), '::1');
  assert.throws(
    () => parseListenHost('localhost'),
    /explicit IPv4 or IPv6 address/,
  );
}

function expectedProxyConfiguration() {
  return {
    schema: 'postfiat-navswap-atomic-proxy-configuration-v1',
    chain_id: 'postfiat-local',
    genesis_hash: 'c'.repeat(96),
    protocol_version: 1,
    assets: {
      a651: A651_ASSET_ID,
      pfusdc: PFUSDC_ASSET_ID,
    },
    rpc_fleet: RPC_FLEET.map((endpoint) => ({
      node_id: endpoint.validatorId,
      host: endpoint.host,
      port: endpoint.port,
    })),
  };
}

function quoteBody(overrides = {}) {
  return {
    route: 'transparent_navswap',
    settlement_mode: atomicModule.ATOMIC_SETTLEMENT_MODE,
    request_id: 'atomic-quote-1',
    rfq_hash: '3'.repeat(96),
    market_envelope_hash: '4'.repeat(96),
    nav_epoch: 7,
    expires_at_height: 900,
    swap_nonce: '5'.repeat(96),
    leg_0_owner: OWNER_0,
    leg_0_recipient: OWNER_1,
    leg_0_issuer: ISSUER_0,
    leg_0_asset_id: PFUSDC_ASSET_ID,
    leg_0_amount: 100,
    leg_1_owner: OWNER_1,
    leg_1_recipient: OWNER_0,
    leg_1_issuer: ISSUER_1,
    leg_1_asset_id: A651_ASSET_ID,
    leg_1_amount: 200,
    ...overrides,
  };
}

function atomicQuoteParams(body = quoteBody()) {
  return Object.fromEntries(
    Object.entries(body).filter(
      ([key]) => !['route', 'settlement_mode', 'request_id'].includes(key),
    ),
  );
}

function unsignedTransaction(body = quoteBody()) {
  return {
    chain_id: 'postfiat-local',
    genesis_hash: 'c'.repeat(96),
    protocol_version: 1,
    address_namespace: 'postfiat',
    signature_algorithm_id: 'ml-dsa-65',
    rfq_hash: body.rfq_hash,
    market_envelope_hash: body.market_envelope_hash,
    nav_epoch: body.nav_epoch,
    expires_at_height: body.expires_at_height,
    swap_nonce: body.swap_nonce,
    leg_0: {
      owner: body.leg_0_owner,
      recipient: body.leg_0_recipient,
      issuer: body.leg_0_issuer,
      asset_id: body.leg_0_asset_id,
      amount: body.leg_0_amount,
      sequence: 3,
      fee: 22,
    },
    leg_1: {
      owner: body.leg_1_owner,
      recipient: body.leg_1_recipient,
      issuer: body.leg_1_issuer,
      asset_id: body.leg_1_asset_id,
      amount: body.leg_1_amount,
      sequence: 5,
      fee: 22,
    },
  };
}

function quoteResponse(request, body = quoteBody()) {
  return {
    version: 'postfiat-local-rpc-v1',
    id: request.id,
    ok: true,
    result: {
      schema: 'postfiat-atomic-swap-fee-quote-v1',
      transaction_kind: 'atomic_swap',
      parent_height: 881,
      parent_hash: PARENT_HASH,
      parent_state_root: PARENT_ROOT,
      quote_height: 882,
      account_reserve: 10,
      transfer_fee_byte_quantum: 512,
      transfer_fee_per_quantum: 1,
      atomic_swap_weight_bytes: 4096,
      leg_0: { owner: OWNER_0 },
      leg_1: { owner: OWNER_1 },
      unsigned_transaction: unsignedTransaction(body),
    },
    error: null,
    events: [],
  };
}

function signedTransaction(body = quoteBody()) {
  return {
    unsigned: unsignedTransaction(body),
    authorization_0: {
      owner: OWNER_0,
      algorithm_id: 'ml-dsa-65',
      public_key_hex: 'd'.repeat(64),
      signature_hex: 'e'.repeat(128),
    },
    authorization_1: {
      owner: OWNER_1,
      algorithm_id: 'ml-dsa-65',
      public_key_hex: 'f'.repeat(64),
      signature_hex: '0'.repeat(128),
    },
  };
}

function finalityResponse(request, body = quoteBody()) {
  const signed = signedTransaction(body);
  const txId = TX_ID;
  return {
    version: 'postfiat-local-rpc-v1',
    id: request.id,
    ok: true,
    result: {
      schema: 'postfiat-rpc-mempool-submit-signed-atomic-swap-finality-v1',
      tx_id: txId,
      finality: {
        chain_id: signed.unsigned.chain_id,
        genesis_hash: signed.unsigned.genesis_hash,
        protocol_version: signed.unsigned.protocol_version,
        tx_id: txId,
        confirmed: true,
        receipt: {
          tx_id: txId,
          accepted: true,
          code: 'accepted',
          atomic_swap_legs: [0, 1].map((index) => {
            const leg = signed.unsigned[`leg_${index}`];
            return {
              owner: leg.owner,
              recipient: leg.recipient,
              asset_id: leg.asset_id,
              amount: leg.amount,
              fee_charged: leg.fee,
              pre_sequence: leg.sequence - 1,
              post_sequence: leg.sequence,
            };
          }),
        },
        receipt_index: 0,
        block: {
          header: {
            height: 882,
            parent_hash: PARENT_HASH,
            block_hash: '7'.repeat(96),
            state_root: '8'.repeat(96),
            certificate_id: '9'.repeat(96),
            certificate: { quorum: 5 },
          },
          receipt_ids: [txId],
        },
      },
    },
    error: null,
    events: [],
  };
}

function runBody(overrides = {}) {
  return {
    route: 'transparent_navswap',
    settlement_mode: atomicModule.ATOMIC_SETTLEMENT_MODE,
    idempotency_key: 'atomic-run-once-1',
    request_id: 'atomic-run-1',
    expected_tx_id: TX_ID,
    signed_atomic_swap_transaction_json: JSON.stringify(signedTransaction()),
    quote_binding: {
      parent_height: 881,
      parent_hash: PARENT_HASH,
      parent_state_root: PARENT_ROOT,
    },
    ...overrides,
  };
}

function fakeRuntime(handler, overrides = {}) {
  const calls = [];
  const legacy = {
    quoteCalls: 0,
    runCalls: 0,
  };
  const runtime = {
    PFUSDC_ASSET_ID,
    A651_ASSET_ID,
    RPC_FLEET,
    RPC_HOST: 'primary.test',
    RPC_PORT: 27650,
    TCP_TIMEOUT_MS: 1000,
    requestWithProxyReadiness(request) {
      return request;
    },
    async resolveRpcTarget() {
      return {
        endpoint: RPC_FLEET[0],
        route: { routed: true, required_current_height: 999 },
      };
    },
    async rpcTcpRequest(host, port, request, timeoutMs) {
      calls.push({ host, port, request, timeoutMs });
      return handler(request, calls.length);
    },
    async executeNavswapQuote(body) {
      legacy.quoteCalls += 1;
      return { ok: true, legacy: true, body };
    },
    async executeTransparentNavswapRun(body) {
      legacy.runCalls += 1;
      return { ok: true, legacy: true, body };
    },
    async executeNavswapCapabilities() {
      return {
        ok: true,
        routes: {
          transparent_navswap: {
            enabled: true,
            settlement_modes: ['two_transfer_v1'],
          },
        },
      };
    },
    ...overrides,
  };
  return { api: atomicModule.create(runtime), calls, legacy, runtime };
}

async function testAtomicQuoteBranchesAndBindsExactParent() {
  const body = quoteBody();
  const { api, calls, legacy } = fakeRuntime((request) => quoteResponse(request, body));
  const result = await api.executeNavswapQuoteWithAtomic(body);

  assert.strictEqual(result.ok, true);
  assert.strictEqual(result.schema, 'postfiat-navswap-atomic-quote-v1');
  assert.strictEqual(result.settlement_mode, 'atomic_swap_v1');
  assert.deepStrictEqual(result.quote_binding, {
    parent_height: 881,
    parent_hash: PARENT_HASH,
    parent_state_root: PARENT_ROOT,
  });
  assert.strictEqual(calls.length, 1);
  assert.strictEqual(calls[0].request.method, atomicModule.ATOMIC_QUOTE_METHOD);
  assert.strictEqual(calls[0].request.params.rfq_hash, body.rfq_hash);
  assert.strictEqual(legacy.quoteCalls, 0);

  const legacyResult = await api.executeNavswapQuoteWithAtomic({ route: 'transparent_navswap' });
  assert.strictEqual(legacyResult.legacy, true);
  assert.strictEqual(legacy.quoteCalls, 1);
}

async function testAtomicQuoteRejectsSubstitutedResponse() {
  const body = quoteBody();
  const { api } = fakeRuntime((request) => {
    const response = quoteResponse(request, body);
    response.result.unsigned_transaction.leg_1.amount += 1;
    return response;
  });
  const result = await api.executeAtomicNavswapQuote(body);
  assert.strictEqual(result.ok, false);
  assert.strictEqual(result.code, 'atomic_navswap_quote_response_mismatch');
}

async function testProxyConfigurationIsExactAndStable() {
  const normal = fakeRuntime((request) => quoteResponse(request));
  const quote = await normal.api.executeAtomicNavswapQuote(quoteBody());
  assert.strictEqual(quote.ok, true);
  assert.deepStrictEqual(quote.proxy_configuration, expectedProxyConfiguration());
  assert.strictEqual(quote.proxy_configuration_hash, PROXY_CONFIGURATION_HASH);

  const reordered = fakeRuntime(
    (request) => quoteResponse(request),
    { RPC_FLEET: [...RPC_FLEET].reverse() },
  );
  const reorderedQuote = await reordered.api.executeAtomicNavswapQuote(quoteBody());
  assert.strictEqual(reorderedQuote.ok, true);
  assert.deepStrictEqual(reorderedQuote.proxy_configuration, quote.proxy_configuration);
  assert.strictEqual(reorderedQuote.proxy_configuration_hash, quote.proxy_configuration_hash);

  const finality = fakeRuntime((request) => finalityResponse(request));
  const run = await finality.api.executeAtomicNavswapRun(runBody());
  assert.strictEqual(run.ok, true);
  assert.deepStrictEqual(run.proxy_configuration, quote.proxy_configuration);
  assert.strictEqual(run.proxy_configuration_hash, quote.proxy_configuration_hash);
}

async function testMalformedProxyConfigurationFailsClosedBeforeForwarding() {
  const malformedFleets = [
    RPC_FLEET.slice(0, 5),
    RPC_FLEET.map((endpoint, index) => (
      index === 5 ? { ...endpoint, validatorId: 'validator-6' } : endpoint
    )),
    RPC_FLEET.map((endpoint, index) => (
      index === 5 ? { ...endpoint, host: RPC_FLEET[0].host, port: RPC_FLEET[0].port } : endpoint
    )),
    RPC_FLEET.map((endpoint, index) => (
      index === 5 ? { ...endpoint, port: 65536 } : endpoint
    )),
  ];
  for (const fleet of malformedFleets) {
    const quoteHarness = fakeRuntime(
      () => { throw new Error('malformed proxy configuration must not be forwarded'); },
      { RPC_FLEET: fleet },
    );
    const quote = await quoteHarness.api.executeAtomicNavswapQuote(quoteBody());
    assert.strictEqual(quote.ok, false);
    assert.strictEqual(quote.code, 'atomic_navswap_configuration_invalid');
    assert.strictEqual(quoteHarness.calls.length, 0);
  }

  const runHarness = fakeRuntime(
    () => { throw new Error('malformed proxy configuration must not submit'); },
    { RPC_FLEET: RPC_FLEET.slice(0, 5) },
  );
  const run = await runHarness.api.executeAtomicNavswapRun(runBody());
  assert.strictEqual(run.ok, false);
  assert.strictEqual(run.code, 'atomic_navswap_configuration_invalid');
  assert.strictEqual(run.state, 'not_submitted');
  assert.strictEqual(runHarness.calls.length, 0);

  const directError = atomicModule.atomicRpcProxyError({
    version: 'postfiat-local-rpc-v1',
    id: 'malformed-fleet-direct-quote',
    method: atomicModule.ATOMIC_QUOTE_METHOD,
    params: atomicQuoteParams(),
  }, {
    PFUSDC_ASSET_ID,
    A651_ASSET_ID,
    RPC_FLEET: RPC_FLEET.slice(0, 5),
  });
  assert.strictEqual(directError.code, 'atomic_navswap_configuration_invalid');

  const wrongAssets = fakeRuntime(
    () => { throw new Error('wrong asset authority must not be forwarded'); },
    { PFUSDC_ASSET_ID: THIRD_ASSET_ID },
  );
  const wrongAssetQuote = await wrongAssets.api.executeAtomicNavswapQuote(quoteBody());
  assert.strictEqual(wrongAssetQuote.ok, false);
  assert.strictEqual(wrongAssetQuote.code, 'atomic_navswap_pair_not_supported');
  assert.strictEqual(wrongAssets.calls.length, 0);
}

async function testConfiguredPairRejectsThirdAssetBeforeForwarding() {
  const unsupported = quoteBody({ leg_1_asset_id: THIRD_ASSET_ID });
  const quoteHarness = fakeRuntime(() => {
    throw new Error('unsupported quote must not be forwarded');
  });
  const quote = await quoteHarness.api.executeAtomicNavswapQuote(unsupported);
  assert.strictEqual(quote.ok, false);
  assert.strictEqual(quote.code, 'atomic_navswap_pair_not_supported');
  assert.strictEqual(quoteHarness.calls.length, 0);

  const runHarness = fakeRuntime(() => {
    throw new Error('unsupported signed run must not be forwarded');
  });
  const run = await runHarness.api.executeAtomicNavswapRun(runBody({
    signed_atomic_swap_transaction_json: JSON.stringify(signedTransaction(unsupported)),
  }));
  assert.strictEqual(run.ok, false);
  assert.strictEqual(run.code, 'atomic_navswap_pair_not_supported');
  assert.strictEqual(run.state, 'not_submitted');
  assert.strictEqual(run.rpc_request, null);
  assert.strictEqual(runHarness.calls.length, 0);
}

async function testPrivateMaterialFailsBeforeForwarding() {
  const quoteHarness = fakeRuntime(() => {
    throw new Error('must not forward');
  });
  const quote = await quoteHarness.api.executeAtomicNavswapQuote(quoteBody({
    owner_key_file: '/forbidden/wallet.json',
  }));
  assert.strictEqual(quote.ok, false);
  assert.strictEqual(quote.code, 'atomic_navswap_private_material_rejected');
  assert.strictEqual(quoteHarness.calls.length, 0);

  const runHarness = fakeRuntime(() => {
    throw new Error('must not forward');
  });
  const run = await runHarness.api.executeAtomicNavswapRun(runBody({
    backup_json: '{"forbidden":true}',
  }));
  assert.strictEqual(run.ok, false);
  assert.strictEqual(run.code, 'atomic_navswap_private_material_rejected');
  assert.strictEqual(run.state, 'not_submitted');
  assert.strictEqual(runHarness.calls.length, 0);
}

async function testFinalityIsSingleShotAndPreservesExactPins() {
  const { api, calls } = fakeRuntime((request) => finalityResponse(request));
  const result = await api.executeTransparentNavswapRunWithAtomic(runBody());

  assert.strictEqual(result.ok, true);
  assert.strictEqual(result.state, 'finalized');
  assert.strictEqual(result.mutation_policy, 'single_shot_no_retry');
  assert.strictEqual(result.rpc_request, undefined);
  assert.strictEqual(
    result.rpc_request_hash,
    crypto.createHash('sha256')
      .update(stableJson(calls[0].request))
      .digest('hex'),
  );
  assert.strictEqual(calls.length, 1);
  assert.strictEqual(calls[0].request.method, atomicModule.ATOMIC_FINALITY_METHOD);
  assert.deepStrictEqual(
    {
      parent_height: calls[0].request.params.proxy_required_current_height,
      parent_hash: calls[0].request.params.proxy_required_parent_hash,
      parent_state_root: calls[0].request.params.proxy_required_state_root,
    },
    runBody().quote_binding,
  );
}

async function testWrappedFinalityTransportRoutesToDeterministicProposer() {
  const proposer = RPC_FLEET[4];
  let routedMethod = null;
  let observedRoute = null;
  const harness = fakeRuntime(
    (request) => finalityResponse(request),
    {
      async resolveRpcTarget(method) {
        routedMethod = method;
        return {
          endpoint: proposer,
          route: {
            routed: true,
            route_kind: 'finality_proposer',
            proposer: proposer.validatorId,
            height: 882,
            view: 0,
            required_current_height: 881,
            required_state_root: PARENT_ROOT,
          },
        };
      },
      requestWithProxyReadiness(request, route) {
        observedRoute = route;
        return request;
      },
    },
  );
  const wrappedRpcRequest = (...args) => harness.runtime.rpcTcpRequest(...args);
  atomicModule.markAtomicProxyRoutableTransport(wrappedRpcRequest);

  const result = await harness.api.executeAtomicNavswapRun(
    runBody({ request_id: 'atomic-run-wrapped-route' }),
    wrappedRpcRequest,
  );

  assert.strictEqual(result.ok, true);
  assert.strictEqual(result.state, 'finalized');
  assert.strictEqual(result.mutation_policy, 'single_shot_no_retry');
  assert.strictEqual(routedMethod, atomicModule.ATOMIC_FINALITY_METHOD);
  assert.strictEqual(harness.calls.length, 1);
  assert.strictEqual(harness.calls[0].host, proposer.host);
  assert.strictEqual(harness.calls[0].port, proposer.port);
  assert.notStrictEqual(harness.calls[0].host, RPC_FLEET[0].host);
  assert.strictEqual(observedRoute.proposer, proposer.validatorId);
  assert.strictEqual(result.proxy_route.proposer, proposer.validatorId);
}

async function testSuccessfulAtomicFinalityPrimesNextProposerRoute() {
  const remembered = [];
  const primed = [];
  const proposer = RPC_FLEET[4];
  const harness = fakeRuntime(
    (request) => finalityResponse(request),
    {
      async resolveRpcTarget() {
        return {
          endpoint: proposer,
          route: {
            routed: true,
            proposer: proposer.validatorId,
            height: 882,
            required_current_height: 881,
            required_state_root: PARENT_ROOT,
          },
        };
      },
      rememberFinalizedReadEndpoint(line, selection) {
        remembered.push({ response: JSON.parse(line), selection });
      },
      primeNextProposerRouteCacheFromResponse(line, route, options) {
        primed.push({ response: JSON.parse(line), route, options });
        return { endpoint: RPC_FLEET[5] };
      },
    },
  );

  const result = await harness.api.executeAtomicNavswapRun(runBody());

  assert.strictEqual(result.ok, true);
  assert.strictEqual(result.next_proposer_route_cache_primed, true);
  assert.strictEqual(remembered.length, 1);
  assert.strictEqual(remembered[0].selection.endpoint, proposer);
  assert.strictEqual(remembered[0].response.result.tx_id, TX_ID);
  assert.strictEqual(primed.length, 1);
  assert.strictEqual(primed[0].route.proposer, proposer.validatorId);
  assert.deepStrictEqual(primed[0].options, { warmReadiness: true });
}

async function testFinalitySuccessBindsBothSignedLegsAndDomain() {
  const substitutedLeg = fakeRuntime((request) => {
    const response = finalityResponse(request);
    response.result.finality.receipt.atomic_swap_legs[1].amount += 1;
    return response;
  });
  const legResult = await substitutedLeg.api.executeAtomicNavswapRun(runBody());
  assert.strictEqual(legResult.ok, false);
  assert.strictEqual(legResult.code, 'atomic_navswap_finality_response_mismatch');
  assert.strictEqual(legResult.state, 'submitted_unknown');

  const wrongDomain = fakeRuntime((request) => {
    const response = finalityResponse(request);
    response.result.finality.genesis_hash = 'f'.repeat(96);
    return response;
  });
  const domainResult = await wrongDomain.api.executeAtomicNavswapRun(runBody());
  assert.strictEqual(domainResult.ok, false);
  assert.strictEqual(domainResult.code, 'atomic_navswap_finality_response_mismatch');
  assert.strictEqual(domainResult.state, 'submitted_unknown');

  const substitutedTxId = fakeRuntime((request) => {
    const response = finalityResponse(request);
    const wrongTxId = '1'.repeat(96);
    response.result.tx_id = wrongTxId;
    response.result.finality.tx_id = wrongTxId;
    response.result.finality.receipt.tx_id = wrongTxId;
    response.result.finality.block.receipt_ids[0] = wrongTxId;
    return response;
  });
  const txIdResult = await substitutedTxId.api.executeAtomicNavswapRun(runBody());
  assert.strictEqual(txIdResult.ok, false);
  assert.strictEqual(txIdResult.code, 'atomic_navswap_finality_response_mismatch');
  assert.strictEqual(txIdResult.state, 'submitted_unknown');

  const rejectedCode = fakeRuntime((request) => {
    const response = finalityResponse(request);
    response.result.finality.receipt.code = 'asset_orchard_pricing_off_band';
    return response;
  });
  const rejectedCodeResult = await rejectedCode.api.executeAtomicNavswapRun(runBody());
  assert.strictEqual(rejectedCodeResult.ok, false);
  assert.strictEqual(
    rejectedCodeResult.code,
    'atomic_navswap_finality_response_mismatch',
  );
  assert.strictEqual(rejectedCodeResult.state, 'submitted_unknown');
}

async function testFinalityTransportErrorIsUnknownAndNeverRetried() {
  const { api, calls } = fakeRuntime(() => {
    throw new Error('connection lost after write');
  });
  const result = await api.executeAtomicNavswapRun(runBody());
  assert.strictEqual(result.ok, false);
  assert.strictEqual(result.state, 'submitted_unknown');
  assert.strictEqual(result.mutation_policy, 'single_shot_no_retry');
  assert.strictEqual(calls.length, 1);
}

async function testTerminalStaleIsReturnedWithoutRetry() {
  const { api, calls } = fakeRuntime((request) => ({
    version: 'postfiat-local-rpc-v1',
    id: request.id,
    ok: false,
    result: null,
    error: {
      code: 'rpc_finality_parent_stale',
      message: 'parent tuple is terminally stale',
    },
    events: [],
  }));
  const result = await api.executeAtomicNavswapRun(runBody());
  assert.strictEqual(result.state, 'terminal_stale');
  assert.strictEqual(calls.length, 1);
}

async function testMalformedSuccessIsUnknownAndNeverRetried() {
  const { api, calls } = fakeRuntime((request) => ({
    version: 'postfiat-local-rpc-v1',
    id: request.id,
    ok: true,
    result: { schema: 'wrong-finality-schema' },
    error: null,
    events: [],
  }));
  const result = await api.executeAtomicNavswapRun(runBody());
  assert.strictEqual(result.ok, false);
  assert.strictEqual(result.code, 'atomic_navswap_finality_response_mismatch');
  assert.strictEqual(result.state, 'submitted_unknown');
  assert.strictEqual(calls.length, 1);
}

async function testCapabilitiesDeclareFinalityOnlyAndNoTrustlineOperation() {
  const { api } = fakeRuntime(() => null);
  const result = await api.executeNavswapCapabilitiesWithAtomic();
  const atomic = result.routes.transparent_navswap.atomic_swap_v1;
  assert.strictEqual(atomic.enabled, true);
  assert.strictEqual(atomic.raw_submit_enabled, false);
  assert.strictEqual(atomic.mutation_policy, 'single_shot_no_retry');
  assert.strictEqual(atomic.submit_method, atomicModule.ATOMIC_FINALITY_METHOD);
  const serialized = JSON.stringify(atomic);
  assert.doesNotMatch(serialized, /trustline|trust_set|line_create/);
  assert.doesNotMatch(serialized, /recovery/);
}

function testProxyRoutingAndRawDenial() {
  assert.strictEqual(isSequencedAccountMethod(atomicModule.ATOMIC_QUOTE_METHOD), true);
  assert.strictEqual(isFinalityMethod(atomicModule.ATOMIC_FINALITY_METHOD), true);
  assert.strictEqual(isFinalityMethod(atomicModule.ATOMIC_RAW_SUBMIT_METHOD), false);
  assert.strictEqual(RPC_CAPS.atomic_swap_fee_quote_enabled, true);
  assert.strictEqual(RPC_CAPS.mempool_submit_atomic_swap_finality_enabled, true);
  assert.strictEqual(RPC_CAPS.mempool_submit_atomic_swap_enabled, false);

  const rawError = atomicRpcProxyError({
    version: 'postfiat-local-rpc-v1',
    id: 'raw-submit',
    method: atomicModule.ATOMIC_RAW_SUBMIT_METHOD,
    params: { signed_atomic_swap_transaction_json: '{}' },
  });
  assert.strictEqual(rawError.code, 'proxy_atomic_swap_raw_submit_disabled');

  const quotePairError = atomicRpcProxyError({
    version: 'postfiat-local-rpc-v1',
    id: 'unsupported-direct-quote',
    method: atomicModule.ATOMIC_QUOTE_METHOD,
    params: atomicQuoteParams(quoteBody({ leg_1_asset_id: THIRD_ASSET_ID })),
  });
  assert.strictEqual(quotePairError.code, 'atomic_navswap_pair_not_supported');

  const finalityPairError = atomicRpcProxyError({
    version: 'postfiat-local-rpc-v1',
    id: 'unsupported-direct-finality',
    method: atomicModule.ATOMIC_FINALITY_METHOD,
    params: {
      signed_atomic_swap_transaction_json: JSON.stringify(
        signedTransaction(quoteBody({ leg_1_asset_id: THIRD_ASSET_ID })),
      ),
      proxy_required_current_height: 881,
      proxy_required_parent_hash: PARENT_HASH,
      proxy_required_state_root: PARENT_ROOT,
    },
  });
  assert.strictEqual(finalityPairError.code, 'atomic_navswap_pair_not_supported');
}

function testProxyReadinessNeverRewritesAtomicParentTuple() {
  const request = {
    version: 'postfiat-local-rpc-v1',
    id: 'atomic-run-1',
    method: atomicModule.ATOMIC_FINALITY_METHOD,
    params: {
      signed_atomic_swap_transaction_json: JSON.stringify(signedTransaction()),
      proxy_required_current_height: 881,
      proxy_required_parent_hash: PARENT_HASH,
      proxy_required_state_root: PARENT_ROOT,
      proxy_readiness_timeout_ms: 1234,
    },
  };
  const route = {
    required_current_height: 999,
    required_state_root: '9'.repeat(96),
  };
  assert.strictEqual(requestWithProxyReadiness(request, route), request);
  assert.strictEqual(request.params.proxy_required_current_height, 881);
  assert.strictEqual(request.params.proxy_required_parent_hash, PARENT_HASH);
  assert.strictEqual(request.params.proxy_required_state_root, PARENT_ROOT);
  assert.strictEqual(request.params.proxy_readiness_timeout_ms, 1234);
}

function testDirectRpcRejectsPrivateMaterial() {
  const request = {
    version: 'postfiat-local-rpc-v1',
    id: 'atomic-run-private',
    method: atomicModule.ATOMIC_FINALITY_METHOD,
    params: {
      signed_atomic_swap_transaction_json: JSON.stringify(signedTransaction()),
      proxy_required_current_height: 881,
      proxy_required_parent_hash: PARENT_HASH,
      proxy_required_state_root: PARENT_ROOT,
      signer_key_path: '/forbidden/key',
    },
  };
  const error = atomicRpcProxyError(request);
  assert.strictEqual(error.code, 'atomic_navswap_private_material_rejected');
}

async function testServerHttpDelegatesUseAtomicBranch() {
  let quoteCalls = 0;
  const body = quoteBody({ request_id: 'server-atomic-quote' });
  const quote = await executeNavswapQuote(body, async (_host, _port, request) => {
    quoteCalls += 1;
    return quoteResponse(request, body);
  });
  assert.strictEqual(quote.ok, true);
  assert.strictEqual(quote.settlement_mode, 'atomic_swap_v1');
  assert.strictEqual(quoteCalls, 1);

  let finalityCalls = 0;
  const run = await executeNavswapRun(
    runBody({ request_id: 'server-atomic-run' }),
    async (_host, _port, request) => {
      finalityCalls += 1;
      return finalityResponse(request);
    },
  );
  assert.strictEqual(run.ok, true);
  assert.strictEqual(run.settlement_mode, 'atomic_swap_v1');
  assert.strictEqual(finalityCalls, 1);

  const capabilities = await executeNavswapCapabilities();
  assert.strictEqual(
    capabilities.routes.transparent_navswap.atomic_swap_v1.submit_method,
    atomicModule.ATOMIC_FINALITY_METHOD,
  );
}

async function main() {
  testExplicitProxyListenHost();
  await testAtomicQuoteBranchesAndBindsExactParent();
  await testAtomicQuoteRejectsSubstitutedResponse();
  await testProxyConfigurationIsExactAndStable();
  await testMalformedProxyConfigurationFailsClosedBeforeForwarding();
  await testConfiguredPairRejectsThirdAssetBeforeForwarding();
  await testPrivateMaterialFailsBeforeForwarding();
  await testFinalityIsSingleShotAndPreservesExactPins();
  await testWrappedFinalityTransportRoutesToDeterministicProposer();
  await testSuccessfulAtomicFinalityPrimesNextProposerRoute();
  await testFinalitySuccessBindsBothSignedLegsAndDomain();
  await testFinalityTransportErrorIsUnknownAndNeverRetried();
  await testTerminalStaleIsReturnedWithoutRetry();
  await testMalformedSuccessIsUnknownAndNeverRetried();
  await testCapabilitiesDeclareFinalityOnlyAndNoTrustlineOperation();
  testProxyRoutingAndRawDenial();
  testProxyReadinessNeverRewritesAtomicParentTuple();
  testDirectRpcRejectsPrivateMaterial();
  await testServerHttpDelegatesUseAtomicBranch();
  console.log('navswap atomic proxy tests passed');
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
