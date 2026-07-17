// WASM module loader — imports and initializes the PostFiat wallet WASM.
// Uses Vite's ?url import for reliable WASM binary resolution in dev + prod.
import wasmUrl from '../wasm/postfiat_wallet_wasm_bg.wasm?url';
import wasmInit, * as wasmMod from '../wasm/postfiat_wallet_wasm.js';

let initialized = false;
let wasm = null;

export async function initWasm() {
  if (initialized) return wasm;

  try {
    // __wbg_init (default export) fetches and instantiates the WASM module.
    // It already knows the correct URL via its own import.meta.url reference.
    await wasmInit();
  } catch (e) {
    // Fallback: manually fetch + initSync
    try {
      const response = await fetch(wasmUrl);
      if (!response.ok) throw new Error(`HTTP ${response.status}`);
      const buffer = await response.arrayBuffer();
      wasmMod.initSync({ module: buffer });
    } catch (e2) {
      throw new Error(`WASM load failed (${e.message}; fallback: ${e2.message})`);
    }
  }

  wasm = wasmMod;
  initialized = true;
  return wasm;
}

export function getWasm() {
  if (!initialized) throw new Error('WASM not initialized. Call initWasm() first.');
  return wasm;
}
