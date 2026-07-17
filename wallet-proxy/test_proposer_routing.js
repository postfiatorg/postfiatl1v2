const assert = require('assert');
const { spawnSync } = require('child_process');

const {
  addProxyRouteEvent,
  chooseProposerEndpointCached,
  chooseProposerEndpointFromStatuses,
  chooseProposerEndpointWithRetry,
  chooseSequencedAccountReadEndpoint,
  deterministicProposer,
  endpointStatusMeetsRoute,
  endpointStatusMeetsSequencedReadRoute,
  isFastpayBroadcastMethod,
  isFinalityMethod,
  isSequencedAccountMethod,
  normalizeFastpayBroadcastRequest,
  parseRpcFleet,
  primeNextProposerRouteCacheFromResponse,
  proposerEndpointForHeight,
  recoverFinalityAcrossViews,
  requestWithProxyReadiness,
  shouldUseFirstReadySequencedRead,
  waitForFastpayConvergedGroup,
} = require('./server');

function status(validatorId, height = 479, root = 'root-479') {
  const port = 27650 + Number(validatorId.split('-')[1]);
  return {
    ok: true,
    endpoint: { validatorId, host: `host-${validatorId}`, port },
    status: {
      block_height: height,
      block_tip_hash: `tip-${height}`,
      state_root: root,
    },
  };
}

function testDeterministicProposer() {
  const validators = ['validator-4', 'validator-0', 'validator-2', 'validator-1', 'validator-5', 'validator-3'];
  assert.strictEqual(deterministicProposer(validators, 479, 0), 'validator-5');
  assert.strictEqual(deterministicProposer(validators, 480, 0), 'validator-0');
  assert.strictEqual(deterministicProposer(validators, 480, 1), 'validator-1');
}

function testProposerEndpointForHeightUsesDefaultFleet() {
  const selected = proposerEndpointForHeight(480);
  assert.strictEqual(selected.proposer, 'validator-0');
  assert.strictEqual(selected.endpoint.validatorId, 'validator-0');
}

function testProposerEndpointForHeightRotatesAcrossRecoveryViews() {
  const validators = [
    'validator-0',
    'validator-1',
    'validator-2',
    'validator-3',
    'validator-4',
    'validator-5',
  ];
  const viewZero = proposerEndpointForHeight(480, 0);
  const viewOne = proposerEndpointForHeight(480, 1);

  assert.strictEqual(viewZero.proposer, deterministicProposer(validators, 480, 0));
  assert.strictEqual(viewOne.proposer, deterministicProposer(validators, 480, 1));
  assert.notStrictEqual(
    viewOne.proposer,
    viewZero.proposer,
    'a recovery view must route to the deterministic later-view proposer',
  );
}

async function testFinalityRecoveryCollectsQuorumAndRoutesLaterView() {
  const route = {
    routed: true,
    proposer: 'validator-0',
    height: 480,
    view: 0,
    quorum: 5,
    required_current_height: 479,
    required_state_root: 'root-479',
    required_parent_hash: 'tip-479',
  };
  const fleet = Array.from({ length: 6 }, (_, index) => ({
    validatorId: `validator-${index}`,
    host: '127.0.0.1',
    port: 27650 + index,
  }));
  const timeoutCalls = [];
  const recoveryCalls = [];
  const recovered = await recoverFinalityAcrossViews({
    version: 'postfiat-local-rpc-v1',
    id: 'recover-me',
    method: 'mempool_submit_signed_transfer_finality',
    params: { signed_transfer_json: '{"signed":true}' },
  }, route, {
    fleet,
    maxRecoveryViews: 1,
    collectStatuses: async () => fleet.map((endpoint) => ({
      ok: true,
      endpoint,
      status: {
        block_height: 479,
        block_tip_hash: 'tip-479',
        state_root: 'root-479',
      },
    })),
    requester: async (host, port, request) => {
      timeoutCalls.push({ host, port, request });
      const validator = fleet.find((endpoint) => endpoint.port === port).validatorId;
      return {
        ok: true,
        result: {
          schema: 'postfiat.block_timeout_vote.v1',
          block_height: 480,
          view: 0,
          vote: { validator },
          consensus_v2_vote: { validator },
        },
      };
    },
    requestLine: async (host, port, request) => {
      recoveryCalls.push({ host, port, request });
      return JSON.stringify({ id: request.id, ok: true, result: { recovered: true }, events: [] });
    },
  });

  assert.strictEqual(timeoutCalls.length, 6);
  assert.strictEqual(recoveryCalls.length, 1);
  assert.strictEqual(recoveryCalls[0].port, 27651, 'height 480 view 1 proposer is validator-1');
  assert.strictEqual(recoveryCalls[0].request.params.proxy_consensus_view, 1);
  const recoveryParams = recoveryCalls[0].request.params;
  assert.strictEqual(recoveryParams.proxy_timeout_votes_encoding, 'gzip-base64-chunks-v1');
  const recoveryEncodedVotes = Array.from(
    { length: recoveryParams.proxy_timeout_votes_chunk_count },
    (_, index) => recoveryParams[`proxy_timeout_votes_chunk_${String(index).padStart(4, '0')}`],
  ).join('');
  const recoveryVotes = JSON.parse(
    require('zlib').gunzipSync(Buffer.from(recoveryEncodedVotes, 'base64')).toString('utf8'),
  );
  assert.strictEqual(recoveryVotes.length, 5);
  assert.strictEqual(recovered.route.view, 1);
  assert.strictEqual(recovered.route.proposer, 'validator-1');
  assert.strictEqual(JSON.parse(recovered.line).ok, true);
}

async function testFinalityRecoveryRefusesAParentWithoutQuorum() {
  const fleet = Array.from({ length: 6 }, (_, index) => ({
    validatorId: `validator-${index}`,
    host: '127.0.0.1',
    port: 27650 + index,
  }));
  await assert.rejects(
    recoverFinalityAcrossViews({ id: 'stale', method: 'mempool_submit_signed_transfer_finality' }, {
      height: 480,
      required_current_height: 479,
      required_state_root: 'root-479',
      required_parent_hash: 'tip-479',
    }, {
      fleet,
      maxRecoveryViews: 1,
      collectStatuses: async () => fleet.map((endpoint, index) => ({
        ok: true,
        endpoint,
        status: {
          block_height: index < 4 ? 479 : 480,
          block_tip_hash: index < 4 ? 'tip-479' : 'tip-480',
          state_root: index < 4 ? 'root-479' : 'root-480',
        },
      })),
    }),
    /parent is not held by quorum/,
  );
}

function testFinalityMethodFilter() {
  assert.strictEqual(isFinalityMethod('mempool_submit_signed_transfer_finality'), true);
  assert.strictEqual(isFinalityMethod('mempool_submit_signed_payment_v2_finality'), true);
  assert.strictEqual(isFinalityMethod('mempool_submit_fastlane_primary_finality'), true);
  assert.strictEqual(isFinalityMethod('status'), false);
  assert.strictEqual(isFinalityMethod('mempool_submit_signed_transfer'), false);
}

function testSequencedAccountMethodFilter() {
  assert.strictEqual(isSequencedAccountMethod('transfer_fee_quote'), true);
  assert.strictEqual(isSequencedAccountMethod('asset_fee_quote'), true);
  assert.strictEqual(isSequencedAccountMethod('offer_fee_quote'), true);
  assert.strictEqual(isSequencedAccountMethod('status'), false);
  assert.strictEqual(isSequencedAccountMethod('mempool_submit_signed_transfer_finality'), false);
}

function testFastpayBroadcastMethodFilter() {
  assert.strictEqual(isFastpayBroadcastMethod('wrap_owned'), false);
  assert.strictEqual(isFastpayBroadcastMethod('unwrap_owned'), false);
  assert.strictEqual(isFastpayBroadcastMethod('owned_apply'), true);
  assert.strictEqual(isFastpayBroadcastMethod('owned_unwrap_apply'), true);
  assert.strictEqual(isFastpayBroadcastMethod('owned_objects'), false);
  assert.strictEqual(isFastpayBroadcastMethod('owned_sign'), false);
  assert.strictEqual(isFastpayBroadcastMethod('owned_unwrap_sign'), false);
}

function testRemovedWrapIsNotNormalizedIntoMutation() {
  const normalized = normalizeFastpayBroadcastRequest({
    version: 'postfiat-local-rpc-v1',
    id: 'wrap',
    method: 'wrap_owned',
    params: {
      from_address: 'pfabc',
      owner_pubkey_hex: 'aa',
      amount: 10,
      asset: 'PFT',
    },
  });
  assert.strictEqual(normalized.params.object_id, undefined);

  const preselected = normalizeFastpayBroadcastRequest({
    version: 'postfiat-local-rpc-v1',
    id: 'wrap',
    method: 'wrap_owned',
    params: {
      from_address: 'pfabc',
      owner_pubkey_hex: 'aa',
      amount: 10,
      asset: 'PFT',
      object_id: '11'.repeat(32),
    },
  });
  assert.strictEqual(preselected.params.object_id, '11'.repeat(32));
}

function testChooseProposerEndpointRequiresMajorityConvergence() {
  const fleet = [0, 1, 2, 3, 4, 5].map((idx) => status(`validator-${idx}`));
  const selected = chooseProposerEndpointFromStatuses(fleet);
  assert.strictEqual(selected.endpoint.validatorId, 'validator-0');
  assert.strictEqual(selected.route.height, 480);
  assert.strictEqual(selected.route.converged_count, 6);

  const divergent = [0, 1, 2, 3, 4, 5].map((idx) => status(`validator-${idx}`, idx));
  assert.throws(
    () => chooseProposerEndpointFromStatuses(divergent),
    /fleet is not converged enough/,
  );
}

async function testChooseProposerEndpointRetriesTransientDivergence() {
  const converged = [0, 1, 2, 3, 4, 5].map((idx) => status(`validator-${idx}`));
  const divergent = [0, 1, 2, 3, 4, 5].map((idx) => status(`validator-${idx}`, idx));
  let attempts = 0;

  const selected = await chooseProposerEndpointWithRetry([], {
    retryMs: 0,
    timeoutMs: 1000,
    collectStatuses: async () => {
      attempts += 1;
      return attempts < 3 ? divergent : converged;
    },
  });

  assert.strictEqual(selected.endpoint.validatorId, 'validator-0');
  assert.strictEqual(selected.route.route_attempts, 3);
}

async function testFastpayConvergenceWaitRefreshesUntilRequiredCount() {
  const fleet = [0, 1, 2, 3, 4, 5]
    .map((idx) => ({ validatorId: `validator-${idx}`, host: `host-${idx}`, port: 27650 + idx }));
  const fourConverged = [
    status('validator-0', 100, 'root-100'),
    status('validator-1', 100, 'root-100'),
    status('validator-2', 100, 'root-100'),
    status('validator-3', 100, 'root-100'),
    status('validator-4', 99, 'root-99'),
    status('validator-5', 99, 'root-99'),
  ];
  const allConverged = [0, 1, 2, 3, 4, 5]
    .map((idx) => status(`validator-${idx}`, 100, 'root-100'));
  let attempts = 0;
  const selected = await waitForFastpayConvergedGroup(fleet, {
    retryMs: 0,
    timeoutMs: 1000,
    requiredCount: 6,
    collectStatuses: async (_fleet, options) => {
      attempts += 1;
      assert.strictEqual(options.forceRefresh, attempts > 1);
      return attempts === 1 ? fourConverged : allConverged;
    },
  });

  assert.strictEqual(selected.majority.length, 6);
  assert.strictEqual(selected.quorum, 5);
  assert.strictEqual(selected.required_count, 6);
  assert.strictEqual(selected.attempts, 2);
}

function testParseFleet() {
  const parsed = parseRpcFleet('validator-0=127.0.0.1:27650,validator-1=127.0.0.2:27651');
  assert.deepStrictEqual(parsed[0], { validatorId: 'validator-0', host: '127.0.0.1', port: 27650 });
  assert.deepStrictEqual(parsed[1], { validatorId: 'validator-1', host: '127.0.0.2', port: 27651 });
}

function testRouteEventInjection() {
  const line = JSON.stringify({ id: 'x', ok: true, result: {}, events: [] });
  const routed = JSON.parse(addProxyRouteEvent(line, {
    routed: true,
    proposer: 'validator-0',
    height: 480,
    view: 0,
    quorum: 5,
    converged_count: 6,
  }));
  assert.strictEqual(routed.proxy_route.proposer, 'validator-0');
  assert.strictEqual(routed.events[0].event_type, 'proxy_proposer_route');

  const fastpay = JSON.parse(addProxyRouteEvent(line, {
    routed: true,
    route_kind: 'fastpay_vote',
    validator: 'validator-3',
    height: 486,
    quorum: 5,
    converged_count: 6,
  }));
  assert.strictEqual(fastpay.proxy_route.validator, 'validator-3');
  assert.strictEqual(fastpay.events[0].event_type, 'proxy_fastpay_vote_route');

  const quote = JSON.parse(addProxyRouteEvent(line, {
    routed: true,
    route_kind: 'sequenced_account_read',
    proposer: 'validator-2',
    height: 487,
    quorum: 5,
    converged_count: 6,
  }));
  assert.strictEqual(quote.proxy_route.proposer, 'validator-2');
  assert.strictEqual(quote.events[0].event_type, 'proxy_sequence_read_route');
}

function testEndpointStatusReadiness() {
  const route = {
    required_current_height: 479,
    required_state_root: 'root-479',
  };
  assert.strictEqual(endpointStatusMeetsRoute({ block_height: 478, state_root: 'root-478' }, route), false);
  assert.strictEqual(endpointStatusMeetsRoute({ block_height: 479, state_root: 'wrong' }, route), false);
  assert.strictEqual(endpointStatusMeetsRoute({ block_height: 479, state_root: 'root-479' }, route), true);
  assert.strictEqual(endpointStatusMeetsRoute({ block_height: 480, state_root: 'root-480' }, route), false);
  assert.strictEqual(endpointStatusMeetsSequencedReadRoute({ block_height: 478, state_root: 'root-478' }, route), false);
  assert.strictEqual(endpointStatusMeetsSequencedReadRoute({ block_height: 479, state_root: 'wrong' }, route), false);
  assert.strictEqual(endpointStatusMeetsSequencedReadRoute({ block_height: 479, state_root: 'root-479' }, route), true);
  assert.strictEqual(endpointStatusMeetsSequencedReadRoute({ block_height: 480, state_root: 'root-480' }, route), true);
}

async function testDeferredFinalityPrimesNextProposerWithSingleEndpointReadiness() {
  const line = JSON.stringify({
    id: 'submit',
    ok: true,
    result: {
      certified_sends_deferred: true,
      finality: {
        block: {
          header: {
            height: 479,
            state_root: 'root-479',
          },
        },
      },
    },
  });
  primeNextProposerRouteCacheFromResponse(line, {
    height: 479,
    proposer: 'validator-5',
    view: 0,
  });

  let statusCalls = 0;
  const selected = await chooseProposerEndpointCached([], {
    routeKind: 'sequenced_account_read',
    readyRetryMs: 0,
    statusRequester: async () => {
      statusCalls += 1;
      return statusCalls === 1
        ? { block_height: 478, state_root: 'root-478' }
        : { block_height: 479, state_root: 'root-479' };
    },
  });

  assert.strictEqual(selected.endpoint.validatorId, 'validator-0');
  assert.strictEqual(selected.route.route_cache_hit, true);
  assert.strictEqual(selected.route.route_kind, 'sequenced_account_read');
  assert.strictEqual(selected.route.route_source, 'post_finality_deferred_cache');
  assert.strictEqual(selected.route.required_current_height, 479);
  assert.strictEqual(selected.route.ready_observed_height, 479);
  assert.strictEqual(selected.route.route_attempts, 2);
  assert.strictEqual(statusCalls, 2);

  const submitRoute = await chooseProposerEndpointCached([], {
    statusRequester: async () => {
      statusCalls += 1;
      return { block_height: 479, state_root: 'root-479' };
    },
  });
  assert.strictEqual(submitRoute.endpoint.validatorId, 'validator-0');
  assert.strictEqual(submitRoute.route.route_cache_hit, true);
  assert.strictEqual(submitRoute.route.route_wait_ms, 0);
  assert.strictEqual(submitRoute.route.route_attempts, 0);
  assert.strictEqual(statusCalls, 2);
}

async function testSequencedReadUsesFirstReadyMinHeightEndpoint() {
  const line = JSON.stringify({
    id: 'submit',
    ok: true,
    result: {
      certified_sends_deferred: true,
      finality: {
        block: {
          header: {
            height: 479,
            state_root: 'root-479',
          },
        },
      },
    },
  });
  primeNextProposerRouteCacheFromResponse(line, {
    height: 479,
    proposer: 'validator-5',
    view: 0,
  });

  const selected = await chooseSequencedAccountReadEndpoint([
    { validatorId: 'validator-0', host: 'host-0', port: 27650 },
    { validatorId: 'validator-2', host: 'host-2', port: 27652 },
  ], {
    skipBackgroundReadinessProbe: true,
    statusRequester: async (endpoint) => {
      if (endpoint.validatorId === 'validator-0') {
        return { block_height: 480, state_root: 'root-480' };
      }
      return { block_height: 478, state_root: 'root-478' };
    },
  });

  assert.strictEqual(selected.endpoint.validatorId, 'validator-0');
  assert.strictEqual(selected.route.route_kind, 'sequenced_account_read');
  assert.strictEqual(selected.route.read_validator, 'validator-0');
  assert.strictEqual(selected.route.proposer, 'validator-0');
  assert.strictEqual(selected.route.route_attempts, 1);
  assert.strictEqual(selected.route.ready_observed_height, 480);
  assert.strictEqual(selected.route.readiness_check, 'proxy_min_height_sequenced_read_route');
  assert.strictEqual(selected.route.read_source, 'first_ready_min_height');
}

async function testFinalityResponseCanWarmNextProposerReadiness() {
  const line = JSON.stringify({
    id: 'submit',
    ok: true,
    result: {
      finality: {
        block: {
          header: {
            height: 479,
            state_root: 'root-479',
          },
        },
      },
    },
  });
  let statusCalls = 0;
  const selection = primeNextProposerRouteCacheFromResponse(line, {
    height: 479,
    proposer: 'validator-5',
    view: 0,
  }, {
    warmReadiness: true,
    readyRetryMs: 0,
    statusRequester: async () => {
      statusCalls += 1;
      return { block_height: 479, state_root: 'root-479' };
    },
  });

  while (selection.ready_in_flight) {
    await new Promise((resolve) => setTimeout(resolve, 0));
  }
  assert.strictEqual(selection.ready_observed_status.block_height, 479);

  const selected = await chooseProposerEndpointCached([], {
    routeKind: 'sequenced_account_read',
    statusRequester: async () => {
      throw new Error('cached warm readiness should avoid a foreground status request');
    },
  });

  assert.strictEqual(selection.endpoint.validatorId, 'validator-0');
  assert.strictEqual(statusCalls, 1);
  assert.strictEqual(selected.endpoint.validatorId, 'validator-0');
  assert.strictEqual(selected.route.route_cache_hit, true);
  assert.strictEqual(selected.route.route_wait_ms, 0);
  assert.strictEqual(selected.route.route_attempts, 0);
  assert.strictEqual(selected.route.ready_observed_height, 479);
}

async function testCachedFinalityRouteUsesRpcParentWaitByDefault() {
  const line = JSON.stringify({
    id: 'submit',
    ok: true,
    result: {
      finality: {
        block: {
          header: {
            height: 479,
            state_root: 'root-479',
          },
        },
      },
    },
  });
  primeNextProposerRouteCacheFromResponse(line, {
    height: 479,
    proposer: 'validator-5',
    view: 0,
  });

  const selected = await chooseProposerEndpointCached([], {
    statusRequester: async () => {
      throw new Error('node-side parent wait should avoid foreground status probing');
    },
  });

  assert.strictEqual(selected.endpoint.validatorId, 'validator-0');
  assert.strictEqual(selected.route.route_cache_hit, true);
  assert.strictEqual(selected.route.route_wait_ms, 0);
  assert.strictEqual(selected.route.route_attempts, 0);
  assert.strictEqual(selected.route.readiness_check, 'rpc_parent_wait_finality_route');
}

function testCachedFinalityRouteCanBeOptimisticWhenEnabled() {
  const script = `
    const {chooseProposerEndpointCached,primeNextProposerRouteCacheFromResponse}=require('./wallet-proxy/server');
    const line = JSON.stringify({id:'submit',ok:true,result:{finality:{block:{header:{height:479,state_root:'root-479'}}}}});
    primeNextProposerRouteCacheFromResponse(line,{height:479,proposer:'validator-5',view:0});
    chooseProposerEndpointCached([], {
      statusRequester: async () => { throw new Error('foreground status probe should be skipped'); }
    }).then((selected) => {
      process.exit(selected.route.readiness_check === 'optimistic_cached_finality_route' ? 0 : 1);
    }).catch((error) => {
      console.error(error);
      process.exit(1);
    });
  `;
  const result = spawnSync(
    process.execPath,
    ['-e', script],
    {
      cwd: require('path').join(__dirname, '..'),
      env: {
        ...process.env,
        ENABLE_RPC_PARENT_WAIT_FINALITY_ROUTE: 'false',
        OPTIMISTIC_CACHED_FINALITY_ROUTE: 'true',
      },
      encoding: 'utf8',
    },
  );
  assert.strictEqual(result.status, 0, result.stderr || result.stdout);
}

function testRequestWithProxyReadinessAnnotatesFinalityOnly() {
  const route = {
    required_current_height: 479,
    required_state_root: 'root-479',
  };
  const annotated = requestWithProxyReadiness({
    id: 'submit',
    method: 'mempool_submit_signed_transfer_finality',
    params: {
      signed_transfer_json: '{"ok":true}',
    },
  }, route);

  assert.strictEqual(annotated.params.signed_transfer_json, '{"ok":true}');
  assert.strictEqual(annotated.params.proxy_required_current_height, 479);
  assert.strictEqual(annotated.params.proxy_required_state_root, 'root-479');
  assert.strictEqual(Number.isFinite(annotated.params.proxy_readiness_timeout_ms), true);

  const quoteRequest = {
    id: 'quote',
    method: 'transfer_fee_quote',
    params: {
      from: 'pf1',
      to: 'pf2',
      amount: 1,
    },
  };
  assert.strictEqual(requestWithProxyReadiness(quoteRequest, route), quoteRequest);

  const readOnly = {
    id: 'status',
    method: 'status',
    params: {},
  };
  assert.strictEqual(requestWithProxyReadiness(readOnly, route), readOnly);
}

function testRpcTcpRequestReusesKeepAliveConnectionWhenEnabled() {
  const script = `
    const net = require('net');
    const { closeUpstreamRpcConnections, rpcTcpRequest } = require('./wallet-proxy/server');
    let connectionCount = 0;
    const server = net.createServer((socket) => {
      connectionCount += 1;
      let buffer = '';
      socket.on('data', (chunk) => {
        buffer += chunk.toString('utf8');
        let idx;
        while ((idx = buffer.indexOf('\\n')) >= 0) {
          const line = buffer.slice(0, idx);
          buffer = buffer.slice(idx + 1);
          const request = JSON.parse(line);
          socket.write(JSON.stringify({
            version: 'postfiat-local-rpc-v1',
            id: request.id,
            ok: true,
            result: { method: request.method },
            error: null,
            events: [],
          }) + '\\n');
        }
      });
    });
    server.listen(0, '127.0.0.1', async () => {
      const port = server.address().port;
      try {
        const first = await rpcTcpRequest('127.0.0.1', port, {
          version: 'postfiat-local-rpc-v1',
          id: 'first',
          method: 'status',
          params: {},
        }, 1000);
        const second = await rpcTcpRequest('127.0.0.1', port, {
          version: 'postfiat-local-rpc-v1',
          id: 'second',
          method: 'server_info',
          params: {},
        }, 1000);
        const ok = first.result.method === 'status'
          && second.result.method === 'server_info'
          && connectionCount === 1;
        closeUpstreamRpcConnections();
        server.close(() => process.exit(ok ? 0 : 1));
      } catch (error) {
        console.error(error);
        closeUpstreamRpcConnections();
        server.close(() => process.exit(1));
      }
    });
  `;
  const result = spawnSync(
    process.execPath,
    ['-e', script],
    {
      cwd: require('path').join(__dirname, '..'),
      env: { ...process.env, ENABLE_UPSTREAM_KEEPALIVE: 'true' },
      encoding: 'utf8',
    },
  );
  assert.strictEqual(result.status, 0, result.stderr || result.stdout);
}

function testSequencedReadCanUseFinalityResponderWhenEnabled() {
  const script = `
    const {
      chooseSequencedAccountReadEndpoint,
      primeNextProposerRouteCacheFromResponse,
      rememberFinalizedReadEndpoint
    } = require('./wallet-proxy/server');
    const line = JSON.stringify({
      id: 'submit',
      ok: true,
      result: { finality: { block: { header: { height: 479, state_root: 'root-479' } } } }
    });
    rememberFinalizedReadEndpoint(line, {
      endpoint: { validatorId: 'validator-5', host: 'host-5', port: 27655 },
    });
    primeNextProposerRouteCacheFromResponse(line, {
      height: 479,
      proposer: 'validator-5',
      view: 0,
    });
    chooseSequencedAccountReadEndpoint([], {
      statusRequester: async () => { throw new Error('finality responder cache should avoid status probing'); },
    }).then((selected) => {
      const ok = selected.endpoint.validatorId === 'validator-5'
        && selected.route.route_wait_ms === 0
        && selected.route.route_attempts === 0
        && selected.route.read_source === 'finality_response_endpoint'
        && selected.route.ready_observed_height === 479
        && selected.route.ready_observed_state_root === 'root-479';
      process.exit(ok ? 0 : 1);
    }).catch((error) => {
      console.error(error);
      process.exit(1);
    });
  `;
  const result = spawnSync(
    process.execPath,
    ['-e', script],
    {
      cwd: require('path').join(__dirname, '..'),
      env: {
        ...process.env,
        ENABLE_RPC_PARENT_WAIT_SEQUENCED_READ_ROUTE: 'false',
        ENABLE_FINALITY_RESPONDER_READ_CACHE: 'true',
      },
      encoding: 'utf8',
    },
  );
  assert.strictEqual(result.status, 0, result.stderr || result.stdout);
}

function testFirstReadySequencedReadsDefaultOn() {
  primeNextProposerRouteCacheFromResponse(JSON.stringify({
    id: 'submit',
    ok: true,
    result: {
      finality: {
        block: {
          header: {
            height: 479,
            state_root: 'root-479',
          },
        },
      },
    },
  }), {
    height: 479,
    proposer: 'validator-5',
    view: 0,
  });
  assert.strictEqual(shouldUseFirstReadySequencedRead(), true);

  primeNextProposerRouteCacheFromResponse(JSON.stringify({
    id: 'submit',
    ok: true,
    result: {
      finality: {
        block: {
          header: {
            height: 480,
            state_root: 'root-480',
          },
        },
      },
    },
  }), {
    height: 480,
    proposer: 'validator-0',
    view: 0,
  });
  assert.strictEqual(shouldUseFirstReadySequencedRead(), true);
}

function testFirstReadySequencedReadsCanBeDisabled() {
  const result = spawnSync(
    process.execPath,
    ['-e', "const {shouldUseFirstReadySequencedRead}=require('./wallet-proxy/server'); if (shouldUseFirstReadySequencedRead()) process.exit(1);"],
    {
      cwd: require('path').join(__dirname, '..'),
      env: { ...process.env, ENABLE_FIRST_READY_SEQUENCED_READ: 'false' },
      encoding: 'utf8',
    },
  );
  assert.strictEqual(result.status, 0, result.stderr || result.stdout);
}

testDeterministicProposer();
testProposerEndpointForHeightUsesDefaultFleet();
testProposerEndpointForHeightRotatesAcrossRecoveryViews();
testFinalityMethodFilter();
testSequencedAccountMethodFilter();
testFastpayBroadcastMethodFilter();
testRemovedWrapIsNotNormalizedIntoMutation();
testChooseProposerEndpointRequiresMajorityConvergence();
testChooseProposerEndpointRetriesTransientDivergence()
  .then(() => {
    return testFastpayConvergenceWaitRefreshesUntilRequiredCount();
  })
  .then(() => testFinalityRecoveryCollectsQuorumAndRoutesLaterView())
  .then(() => testFinalityRecoveryRefusesAParentWithoutQuorum())
  .then(() => {
    testParseFleet();
    testRouteEventInjection();
    testEndpointStatusReadiness();
    return testDeferredFinalityPrimesNextProposerWithSingleEndpointReadiness();
  })
  .then(() => {
    return testSequencedReadUsesFirstReadyMinHeightEndpoint();
  })
  .then(() => {
    return testFinalityResponseCanWarmNextProposerReadiness();
  })
  .then(() => {
    return testCachedFinalityRouteUsesRpcParentWaitByDefault();
  })
  .then(() => {
    testCachedFinalityRouteCanBeOptimisticWhenEnabled();
    testRequestWithProxyReadinessAnnotatesFinalityOnly();
  })
  .then(() => {
    testRpcTcpRequestReusesKeepAliveConnectionWhenEnabled();
  })
  .then(() => {
    testSequencedReadCanUseFinalityResponderWhenEnabled();
  })
  .then(() => {
    testFirstReadySequencedReadsDefaultOn();
    testFirstReadySequencedReadsCanBeDisabled();
  })
  .then(() => {
    console.log('proposer routing tests passed');
  })
  .catch((error) => {
    console.error(error);
    process.exit(1);
  });
