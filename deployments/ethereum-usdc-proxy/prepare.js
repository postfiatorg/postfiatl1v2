'use strict';

// Relocate the November 2026 G2 descriptors without changing the governed route
// or blessing new bytes at paths that are staying in place.
const fs = require('node:fs');
const path = require('node:path');
const { createHash } = require('node:crypto');
const { normalizeRoute } = require('../../wallet-proxy/trustless-bridge-jobs');
const { validateConfig } = require('../../wallet-proxy/eth-fast-lane-driver');

if (process.argv.length !== 5) {
    throw new Error('usage: node prepare.js G2_SERVICE NEW_CONFIG_DIR REBUILT_VERIFIER');
}
const [service, output, verifier] = process.argv.slice(2).map(p => path.resolve(p));
const checkout = path.resolve(__dirname, '../..');
const oldConfig = path.join(service, 'driver.json');
const newConfig = path.join(output, 'driver.json');
const routesFile = path.join(service, 'routes.json');
const stat = fs.lstatSync(routesFile);
if (!stat.isFile() || stat.uid !== process.getuid() || (stat.mode & 0o022) !== 0) {
    throw new Error('expected an owner-controlled G2 routes file');
}
const rows = JSON.parse(fs.readFileSync(routesFile, 'utf8'));
if (rows.length !== 1 || rows[0].route_id !== 'ethereum-mainnet-usdc-v1') {
    throw new Error('expected exactly the G2 Ethereum route');
}
const route = rows[0];
normalizeRoute(route);
validateConfig(oldConfig);
const driver = JSON.parse(fs.readFileSync(oldConfig, 'utf8'));
for (const [field, expected] of Object.entries(require('./profile.json'))) {
    if (route[field] !== expected || driver[field] !== expected) {
        throw new Error(`G2 route or driver differs from the reviewed profile at ${field}`);
    }
}
const oldStage = path.join(route.cwd, 'scripts/a666-wallet-eth-bridge-stage-gpu.py');
const stage = path.join(checkout, 'scripts/a666-wallet-eth-bridge-stage-gpu.py');
const oldAdapter = path.join(service, 'eth-fast-lane-driver.js');
const adapter = path.join(checkout, 'wallet-proxy/eth-fast-lane-driver.js');
const oldVerifier = path.join(service, 'gpu-qualification/pftl-ingress-groth16-verify');
const moved = new Map([[oldStage, stage], [oldAdapter, adapter], [oldVerifier, verifier],
    [oldConfig, newConfig]]);
for (const name of ['scripts/a666-wallet-eth-bridge-stage-serialized.py',
    'programs/pfusdc-eth-mainnet-ingress/elf/pfusdc-eth-mainnet-ingress-program']) {
    moved.set(path.join(route.cwd, name), path.join(checkout, name));
}
function digest(file) {
    return createHash('sha256').update(fs.readFileSync(file)).digest('hex');
}
for (const file of moved.keys()) {
    const occurrences = pins => pins.filter(pin => pin.path === file).length;
    if (occurrences(route.pinned_files) !== 1
        || (file !== oldConfig && occurrences(driver.pinned_files) !== 1)) {
        throw new Error('missing or duplicate G2 release pin');
    }
}
function relocatePins(pins) {
    return pins.map(pin => {
        const dest = moved.get(pin.path);
        if (!dest) return pin;
        // Only the reviewed adapter, rebuilt verifier and regenerated descriptor
        // are expected to change contents. Keep the existing stage and ELF pins.
        const changed = [oldAdapter, oldVerifier, oldConfig].includes(pin.path);
        return { path: dest, sha256: changed ? digest(dest) : pin.sha256 };
    });
}
function writeNew(file, value) {
    fs.writeFileSync(file, JSON.stringify(value, null, 2) + '\n', { mode: 0o600, flag: 'wx' });
}
for (const command of [driver.readiness, ...driver.stages]) {
    if (command.args[0] !== oldStage) throw new Error('unexpected stage command');
    command.args[0] = stage;
}
for (const args of [route.driver_args, route.readiness_args]) {
    if (args[0] !== oldAdapter || args[2] !== '--config' || args[3] !== oldConfig) {
        throw new Error('unexpected adapter command');
    }
    args[0] = adapter;
    args[3] = newConfig;
}
fs.mkdirSync(output, { mode: 0o700 }); // Fresh release only. Never edit active descriptors.
driver.pinned_files = relocatePins(driver.pinned_files);
writeNew(newConfig, driver);
validateConfig(newConfig);
route.cwd = checkout;
route.pinned_files = relocatePins(route.pinned_files);
normalizeRoute(route);
writeNew(path.join(output, 'routes.json'), rows);
