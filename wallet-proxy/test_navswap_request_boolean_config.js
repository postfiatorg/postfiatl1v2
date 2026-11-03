'use strict';

const assert = require('node:assert/strict');
const path = require('node:path');
const { isDeepStrictEqual } = require('node:util');
const vectors = require('./fixtures/navswap-request-boolean-vectors.json');

function encode(value) {
    if (typeof value === 'bigint') return { bigint: String(value) };
    if (Array.isArray(value)) return value.map(encode);
    if (value && typeof value === 'object') {
        return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, encode(item)]));
    }
    return value;
}

function bigintFields(value, prefix = '') {
    if (typeof value === 'bigint') return [prefix];
    if (!value || typeof value !== 'object') return [];
    return Object.entries(value).flatMap(([key, item]) => bigintFields(item, prefix ? `${prefix}.${key}` : key));
}

function rawEnv(flag) {
    return {
        present: Object.hasOwn(process.env, flag),
        js_typeof: typeof process.env[flag],
        value: Object.hasOwn(process.env, flag) ? process.env[flag] : null,
    };
}

let nativeTimerAttempts = 0;
const originalEnv = process.env;
const originalFetch = global.fetch;
const originalSetTimeout = global.setTimeout;
const originalSetInterval = global.setInterval;
const counters = { ...vectors.forbidden_effect_counters };
const forbidden = (counter) => () => {
    counters[counter] += 1;
    throw new Error(`${counter} is outside the pure configuration test`);
};
const timerGuard = () => {
    nativeTimerAttempts += 1;
    throw new Error('native timer is outside the pure configuration test');
};

try {
    // The builder vectors cover the legacy certification, kept behind its switch for one release (W-1 S3.3).
    process.env = { ...vectors.fixed_environment, WALLET_PROXY_LEGACY_SHIELDED_CERTIFICATION: 'on' };
    global.fetch = forbidden('network');
    global.setTimeout = timerGuard;
    global.setInterval = timerGuard;
    for (const name of vectors.absent_key_variables) assert.equal(Object.hasOwn(process.env, name), false);

    const literals = Object.fromEntries(Object.entries(vectors.runtime_literals).map(([key, value]) => [
        key, value && typeof value === 'object' && Object.hasOwn(value, 'bigint') ? BigInt(value.bigint) : value,
    ]));
    const runtime = new Proxy({
        ...literals, path,
        fs: new Proxy({}, { get(_target, key) {
            return forbidden(/^(write|append|mkdir|rename|unlink|rm|truncate)/.test(String(key))
                ? 'runtime_fs_write' : 'runtime_fs_read');
        } }),
        execFileAsync: forbidden('exec_async'), execFileSync: forbidden('exec_sync'), spawn: forbidden('spawn'),
        http: new Proxy({}, { get: () => forbidden('network') }),
        net: new Proxy({}, { get: () => forbidden('network') }),
        readNavswapKeyFileAddress: forbidden('key_address_read'),
        presentEnv(name) {
            const value = process.env[name];
            return value === undefined || String(value).trim() === '' ? null : String(value).trim();
        },
    }, { get(target, key) {
        if (Object.hasOwn(target, key)) return target[key];
        const name = String(key);
        if (/sign/i.test(name)) return forbidden('sign');
        if (/send/i.test(name)) return forbidden('send');
        if (/ssh/i.test(name)) return forbidden('ssh');
        if (/rpc|requestWith|routed/i.test(name)) return forbidden('rpc');
        if (/fetch|network/i.test(name)) return forbidden('network');
        return forbidden('other_runtime_action');
    } });

    const bridgePath = require.resolve('./navswap-config-bridge');
    const shieldedPath = require.resolve('./navswap-shielded');
    let constructorCount = 0;
    const bridge = require(bridgePath).create(runtime);
    constructorCount += 1;
    const shielded = require(shieldedPath).create(runtime);
    constructorCount += 1;
    const originalBridge = bridge;
    const originalShielded = shielded;
    const records = [];
    for (const vector of vectors.observations) {
        if (vector.flag_input.kind === 'unset') delete process.env[vector.flag];
        else {
            assert.equal(vector.flag_input.kind, 'string');
            process.env[vector.flag] = vector.flag_input.value;
        }
        const rawBefore = rawEnv(vector.flag);
        assert.deepStrictEqual(rawBefore, vector.expected_raw_env, vector.id);
        for (const name of vectors.absent_key_variables) assert.equal(Object.hasOwn(process.env, name), false);
        const environmentBefore = { ...process.env };
        const input = vector.builder_config === null ? null : structuredClone(vector.builder_config);
        const inputBefore = input === null ? null : structuredClone(input);
        const api = vector.module.endsWith('navswap-config-bridge.js') ? bridge : shielded;
        const observed = {
            kind: 'return', value: null, return_js_typeof: null, return_is_array: null,
            bigint_fields: [], exception_class: null, exception_message: null,
            input_unchanged: null, raw_env_unchanged: true, forbidden_effects: null,
        };
        try {
            const value = input === null ? api[vector.method]()
                : api[vector.method](input, vector.batch_file, vector.artifact_dir);
            observed.value = encode(value);
            observed.return_js_typeof = typeof value;
            observed.return_is_array = Array.isArray(value);
            observed.bigint_fields = bigintFields(value);
        } catch (error) {
            observed.kind = 'error';
            observed.exception_class = error.constructor.name;
            observed.exception_message = error.message;
        }
        observed.input_unchanged = input === null ? null : isDeepStrictEqual(input, inputBefore);
        observed.raw_env_unchanged = isDeepStrictEqual(rawEnv(vector.flag), rawBefore);
        observed.forbidden_effects = { ...counters };
        const record = {
            id: vector.id, api_id: vector.api_id, method: vector.method, module: vector.module,
            scenario: vector.scenario, sequence_index: vector.sequence_index, flag: vector.flag,
            flag_input: vector.flag_input, raw_env_before: rawBefore, raw_env_after: rawEnv(vector.flag),
            environment_before: environmentBefore, environment_after: { ...process.env },
            builder_input_before: inputBefore, builder_input_after: input,
            batch_file: vector.batch_file, artifact_dir: vector.artifact_dir,
            observed, same_instances: bridge === originalBridge && shielded === originalShielded,
            matches_base: isDeepStrictEqual(observed, vectors.outcomes[vector.expectedBase]),
            matches_fixed: isDeepStrictEqual(observed, vectors.outcomes[vector.expectedFixed]),
        };
        records.push(record);
        assert.deepStrictEqual(counters, vectors.forbidden_effect_counters);
        assert.equal(nativeTimerAttempts, 0);
        assert.deepStrictEqual(process.env, environmentBefore, 'pure reads must not rewrite any environment setting');
    }
    const summary = {
        runtime: process.version, oracle_sha256: vectors.independent_oracle_sha256,
        observations: records.length, constructor_count: constructorCount,
        dynamic_same_instances: records.filter((record) => record.scenario === 'dynamic').every((record) => record.same_instances),
        returns: records.filter((record) => record.observed.kind === 'return').length,
        errors: records.filter((record) => record.observed.kind === 'error').length,
        base_mismatches: records.filter((record) => !record.matches_base).map((record) => record.id),
        fixed_mismatches: records.filter((record) => !record.matches_fixed).map((record) => record.id),
        forbidden_effects: counters, native_timer_attempts: nativeTimerAttempts,
    };
    assert.equal(summary.constructor_count, 2);
    assert.equal(summary.dynamic_same_instances, true);
    assert.deepStrictEqual(summary.fixed_mismatches, [], 'request-time boolean configuration contract');
    console.log('NavSwap request-time boolean configuration tests passed (73/73)');
} finally {
    process.env = originalEnv;
    global.fetch = originalFetch;
    global.setTimeout = originalSetTimeout;
    global.setInterval = originalSetInterval;
}
