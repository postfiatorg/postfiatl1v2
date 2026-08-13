const assert = require('assert');
const fs = require('fs');
const path = require('path');

const root = path.resolve(__dirname, '..');
const capture = fs.readFileSync(
    path.join(root, 'tools/eth-l1-mainnet-fast-lane-p0/src/main.rs'),
    'utf8',
);
const stage = fs.readFileSync(
    path.join(root, 'scripts/a666-wallet-eth-bridge-stage-serialized.py'),
    'utf8',
);
const relay = fs.readFileSync(
    path.join(root, 'scripts/a666-mainnet-pfusdc-relay.sh'),
    'utf8',
);
const bridge = fs.readFileSync(
    path.join(root, 'wallet-web/src/components/Bridge.jsx'),
    'utf8',
);

assert.match(capture, /let depositor = receipt_sender\(&receipt\)\?/);
assert.match(capture, /depositor: format!\("\{depositor:#x\}"\)/);
assert.doesNotMatch(
    capture,
    /depositor:\s*"0x1455bd7fbfbf92a171ef36025e13959e3b0ad8c0"/,
);

assert.match(relay, /sponsor_recipient_if_needed/);
assert.match(relay, /transfer-peer-certified-mempool-round|transport-peer-certified-mempool-round/);
assert.match(relay, /finalize\.ops\.json/);
assert.match(relay, /claim\.ops\.json/);
assert.doesNotMatch(
    relay,
    /submit_round "\$run\/finalize-claim\.ops\.json"/,
);
assert.match(relay, /submit_round "\$run\/finalize\.ops\.json" "\$finalize_height" 1/);
assert.match(relay, /submit_round "\$run\/claim\.ops\.json" "\$claim_height" 1/);
assert.match(relay, /chown postfiat:postfiat '\$remote_ops'/);
assert.match(relay, /runuser -u postfiat -- '\$node' pftl-submit-certified-asset-ops/);
assert.match(relay, /runuser -u postfiat -- '\$node' transport-peer-certified-mempool-round/);
assert.match(relay, /--local-apply-before-certified-send \\\n+        --resume/);

assert.match(relay, /--route-epoch '\$route_epoch'/);
assert.ok(relay.includes('.asset_id == \\$family or .asset_family_id == \\$family'));
assert.match(stage, /skip_finalize=deposit\.get\("status"\) == "finalized"/);
assert.match(stage, /ROUTE_EPOCH = 6/);
assert.match(stage, /"PFTL_ROUTE_EPOCH": str\(ROUTE_EPOCH\)/);
assert.match(stage, /eth-l1-mainnet-fast-lane-p0-depositor-fix-20260731/);
assert.match(bridge, /rpc\.accountAssets\(address\)/);
assert.match(bridge, /function pfusdcLabel/);
assert.match(bridge, /<strong>\{pfusdcLabel\(pfusdcBalance\)\}<\/strong>/);
assert.match(bridge, /await waitForBridgeReadiness\(active\)/);
assert.doesNotMatch(bridge, /\bloadBridgeReadiness\b/);

console.log('live Ethereum pfUSDC browser-path regression passed');
