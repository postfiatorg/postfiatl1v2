import { readFileSync } from 'node:fs';
import assert from 'node:assert/strict';
import { ml_dsa65 } from '@noble/post-quantum/ml-dsa.js';
const hex = (value, bytes) => {
  if (typeof value !== 'string' || value.length !== bytes * 2 || !/^[0-9a-f]+$/.test(value)) throw new Error('Invalid hex');
  return Uint8Array.from(Buffer.from(value, 'hex'));
};
export function verify(publicKey, message, signature, context = 'postfiat-l1-v2/tasknode-validator-binding/v1') {
  try {
    return ml_dsa65.verify(hex(signature, 3309), new TextEncoder().encode(message), hex(publicKey, 1952), { context: new TextEncoder().encode(context) });
  } catch { return false; }
}
const data = JSON.parse(readFileSync(process.argv[2] || new URL('./l1-vectors.json', import.meta.url), 'utf8'));
for (const v of data.vectors) {
  assert.equal(v.context, 'postfiat-l1-v2/tasknode-validator-binding/v1');
  // Production verification fixes its own context; callers cannot select it.
  const actual = verify(v.ml_dsa_public_key, v.message, v.signature);
  assert.equal(actual, v.expected_valid, v.name);
  console.log(`${v.name}: PASS (${actual})`);
}
console.log(`PASS: ${data.vectors.length} Rust crypto_provider -> Node @noble/post-quantum vectors`);
