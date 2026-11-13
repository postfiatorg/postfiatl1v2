'use strict';
const assert = require('node:assert/strict');
const { isFinalityMethod, chooseProposerEndpointFromStatuses, requestWithProxyReadiness } = require('./server');

const method = 'mempool_submit_signed_offer_transaction_finality';
assert.equal(isFinalityMethod(method), true, 'offer finality must enter proposer routing');
assert.equal(isFinalityMethod('mempool_submit_signed_offer_transaction'), false);
const statuses = Array.from({ length: 6 }, (_, index) => ({
  ok: true, endpoint: { validatorId: `validator-${index}`, host: '127.0.0.1', port: 39650 + index },
  status: { block_height: 1256, block_tip_hash: 'parent', state_root: 'state' },
}));
const selected = chooseProposerEndpointFromStatuses(statuses);
assert.equal(selected.route.proposer, 'validator-3');
assert.equal(selected.endpoint.port, 39653);
const request = { version: 'postfiat-local-rpc-v1', id: 'test', method,
  params: { signed_offer_transaction_json: '{"synthetic":true}' } };
const routed = requestWithProxyReadiness(request, selected.route);
assert.equal(routed.method, method);
assert.equal(routed.params.signed_offer_transaction_json, request.params.signed_offer_transaction_json);
assert.equal(routed.params.proxy_required_current_height, 1256);
assert.equal(routed.params.proxy_required_state_root, 'state');
console.log('PASS: offer finality selects the height-1257 proposer and preserves the signed request; no network or chain writes');
