/**
 * Canonical JSON serialization (exact BigInt integer tokens), SHA3-384 hashing,
 * PFTL address parsing/checksum validation, and chain-bound asset/packet IDs.
 * Mirrors python/postfiat_rpc/address.py and python/postfiat_rpc/navcoin_sdk.py.
 */

import { createHash } from "node:crypto";
import {
  ADDRESS_CHECKSUM_DOMAIN,
  ERR_INVALID_RECIPIENT,
  ISSUED_ASSET_ID_DOMAIN,
  NAV_RESERVE_PACKET_ID_DOMAIN,
  NavFeeError,
  U32_MAX,
  U64_MAX,
} from "./constants.mjs";

const ASCII_DIGITS = /^[0-9]+$/;
const HEX_40 = /^[0-9a-fA-F]{40}$/;

/**
 * Parse an exact non-negative integer into BigInt.
 * Rejects booleans, non-integer numbers, and unsafe floats (> MAX_SAFE_INTEGER).
 */
export function parseExactUint(value, name = "value", maxValue = U64_MAX) {
  let parsed;
  if (typeof value === "bigint") {
    parsed = value;
  } else if (typeof value === "number") {
    if (!Number.isSafeInteger(value) || value < 0) {
      throw new RangeError(`${name} must be a non-negative safe integer or BigInt/decimal string`);
    }
    parsed = BigInt(value);
  } else if (typeof value === "string") {
    if (!ASCII_DIGITS.test(value)) {
      throw new RangeError(`${name} must be an unsigned base-10 integer string`);
    }
    parsed = BigInt(value);
  } else {
    throw new TypeError(`${name} must be a BigInt, safe integer, or decimal string`);
  }

  if (parsed < 0n || parsed > maxValue) {
    throw new RangeError(`${name} out of bounds [0, ${maxValue}]: ${parsed}`);
  }
  return parsed;
}

export function parseU32Number(value, name = "value") {
  return Number(parseExactUint(value, name, U32_MAX));
}

export function parseU64(value, name = "value") {
  return parseExactUint(value, name, U64_MAX);
}

/**
 * Serialize any value to canonical JSON with sorted object keys, no whitespace,
 * and BigInt values serialized as exact unquoted integer tokens (matching Python
 * json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)).
 */
export function canonicalJsonString(value) {
  if (value === null) return "null";
  const t = typeof value;
  if (t === "bigint") return value.toString(10);
  if (t === "boolean") return value ? "true" : "false";
  if (t === "number") {
    if (!Number.isFinite(value)) {
      throw new TypeError("non-finite numbers are forbidden in canonical JSON");
    }
    return JSON.stringify(value);
  }
  if (t === "string") return JSON.stringify(value);
  if (Array.isArray(value)) {
    return `[${value.map((item) => canonicalJsonString(item)).join(",")}]`;
  }
  if (t === "object") {
    const keys = Object.keys(value)
      .filter((k) => value[k] !== undefined)
      .sort();
    const parts = keys.map((k) => `${JSON.stringify(k)}:${canonicalJsonString(value[k])}`);
    return `{${parts.join(",")}}`;
  }
  throw new TypeError(`unsupported canonical JSON type: ${t}`);
}

export function canonicalJsonBytes(value) {
  return Buffer.from(canonicalJsonString(value), "utf8");
}

/**
 * Convert an object tree with BigInt fields into standard JSON-safe primitives
 * (BigInts encoded as base-10 strings so JSON.stringify never throws or loses precision).
 */
export function toJsonSafe(value) {
  if (typeof value === "bigint") return value.toString(10);
  if (Array.isArray(value)) return value.map(toJsonSafe);
  if (value !== null && typeof value === "object") {
    const out = {};
    for (const [k, v] of Object.entries(value)) {
      if (v !== undefined) out[k] = toJsonSafe(v);
    }
    return out;
  }
  return value;
}

export function sha3_384Hex(data) {
  return createHash("sha3-384").update(data).digest("hex");
}

function checksumHex(lowercaseHex) {
  const digest = createHash("sha3-256")
    .update(Buffer.from(`${ADDRESS_CHECKSUM_DOMAIN}\0${lowercaseHex}`, "ascii"))
    .digest();
  let out = "";
  for (let i = 0; i < lowercaseHex.length; i += 1) {
    const ch = lowercaseHex[i];
    const byte = digest[i >> 1];
    const nibble = i % 2 === 0 ? byte >> 4 : byte & 0x0f;
    out += ch >= "a" && ch <= "f" && nibble >= 8 ? ch.toUpperCase() : ch;
  }
  return out;
}

export function parseAddress(value) {
  if (typeof value !== "string" || value.length < 2) {
    throw new Error("address must start with `pf`");
  }
  const prefix = value.slice(0, 2);
  if (prefix !== "pf") {
    if (prefix.toLowerCase() === "pf") {
      throw new Error("address prefix must be lowercase `pf`");
    }
    throw new Error("address must start with `pf`");
  }
  if (value.length !== 42) {
    throw new Error("address must be `pf` followed by exactly 40 hex characters");
  }
  const hexPart = value.slice(2);
  if (!HEX_40.test(hexPart)) {
    throw new Error("address must be `pf` followed by 40 hex characters (0-9, a-f)");
  }
  const lower = hexPart.toLowerCase();
  if (hexPart !== lower && checksumHex(lower) !== hexPart) {
    throw new Error("address checksum does not match");
  }
  return `pf${lower}`;
}

export function normalizeAccount(address, fieldName = "account") {
  try {
    return parseAddress(address);
  } catch (err) {
    if (fieldName === "recipient") {
      throw new NavFeeError(ERR_INVALID_RECIPIENT, `invalid recipient address: ${err.message}`);
    }
    throw new Error(`invalid ${fieldName}: ${err.message}`);
  }
}

export function validateAssetCode(code) {
  if (typeof code !== "string" || code.length === 0 || code !== code.trim()) {
    throw new Error("code must be nonempty without leading, trailing, or control whitespace");
  }
  for (let i = 0; i < code.length; i += 1) {
    const ch = code.charCodeAt(i);
    if (ch < 32 || ch === 127) {
      throw new Error("code must be nonempty without leading, trailing, or control whitespace");
    }
  }
  if (Buffer.byteLength(code, "utf8") > 32) {
    throw new Error("code must not exceed 32 bytes");
  }
}

export function derivedAssetId(chainId, issuer, code, version = 1) {
  if (!chainId || typeof chainId !== "string") {
    throw new Error("chain_id is required");
  }
  if (!issuer || typeof issuer !== "string") {
    throw new Error("issuer is required");
  }
  validateAssetCode(code);
  const ver = parseU32Number(version, "version");
  if (ver < 1) {
    throw new Error("version must be >= 1");
  }
  const codeBytes = Buffer.from(code, "utf8");
  const preimage = Buffer.from(
    `chain_id=${chainId}\nissuer=${issuer}\ncode_bytes=${codeBytes.length}\ncode=${code}\nversion=${ver}\n`,
    "utf8",
  );
  return createHash("sha3-384")
    .update(Buffer.from(ISSUED_ASSET_ID_DOMAIN, "utf8"))
    .update(Buffer.from([0]))
    .update(preimage)
    .digest("hex");
}

export function navReservePacketId(assetId, epoch, reservePacketHash) {
  const epochU64 = parseU64(epoch, "epoch");
  const preimage = Buffer.from(
    `asset_id=${assetId}\nepoch=${epochU64.toString(10)}\nreserve_packet_hash=${reservePacketHash}\n`,
    "utf8",
  );
  return createHash("sha3-384")
    .update(Buffer.from(NAV_RESERVE_PACKET_ID_DOMAIN, "utf8"))
    .update(Buffer.from([0]))
    .update(preimage)
    .digest("hex");
}
