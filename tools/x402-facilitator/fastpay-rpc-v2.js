'use strict';

// Compact Protobuf v3 FastPay RPC v2 (`postfiat-fastpay-rpc-v2`) client over direct TLS 1.3 TCP.
//
// Wire framing: [u8 version = 2][u32be frame_len][protobuf payload]
// Matches `crates/node/src/fastpay_rpc_v2.rs` field-for-field:
//   - `owned_sign_v3` (~7.5 KB via `owner_is_change_recipient` flag) fits inside 14.6 KB initcwnd
//   - `owned_apply_v3_fast` (~16.7 KB votes-only via validator 256-entry order LRU) with automatic
//     `OrderNotCached` -> `owned_apply_v3_full` (~24.2 KB) fallback.

const tls = require('node:tls');

const FASTPAY_RPC_V2_VERSION = 2;
const MAX_FASTPAY_V2_FRAME_BYTES = 65_536;

const METHOD_OWNED_SIGN_V3 = 1;
const METHOD_OWNED_APPLY_V3_FAST = 2;
const METHOD_OWNED_APPLY_V3_FULL = 3;

const STATUS_OK = 0;
const STATUS_INVALID_INPUT = 1;
const STATUS_UNSUPPORTED_PROTOCOL_VERSION = 2;
const STATUS_ORDER_NOT_CACHED = 3;
const STATUS_RATE_LIMITED = 4;
const STATUS_INTERNAL_ERROR = 5;

// ---------------------------------------------------------------------------
// Canonical Protobuf v3 Wire Primitives
// ---------------------------------------------------------------------------

function encodeVarint(value) {
  let v = typeof value === 'bigint' ? value : BigInt(value);
  const out = [];
  while (v > 0x7fn) {
    out.push(Number((v & 0x7fn) | 0x80n));
    v >>= 7n;
  }
  out.push(Number(v));
  return Buffer.from(out);
}

function decodeVarint(buf, state) {
  let result = 0n;
  let shift = 0n;
  while (true) {
    if (state.offset >= buf.length) throw new Error('truncated protobuf varint');
    const byte = buf[state.offset++];
    if (shift > 63n || (shift === 63n && byte > 1)) throw new Error('protobuf varint overflow');
    result |= BigInt(byte & 0x7f) << shift;
    if ((byte & 0x80) === 0) return result;
    shift += 7n;
  }
}

function pbVarint(fieldNum, value, chunks) {
  const v = typeof value === 'bigint' ? value : BigInt(value || 0);
  if (v === 0n) return;
  chunks.push(encodeVarint(BigInt(fieldNum) << 3n));
  chunks.push(encodeVarint(v));
}

function pbBool(fieldNum, value, chunks) {
  if (value) pbVarint(fieldNum, 1n, chunks);
}

function pbBytes(fieldNum, buf, chunks) {
  if (!buf || buf.length === 0) return;
  chunks.push(encodeVarint((BigInt(fieldNum) << 3n) | 2n));
  chunks.push(encodeVarint(BigInt(buf.length)));
  chunks.push(buf);
}

function pbString(fieldNum, str, chunks) {
  if (!str) return;
  pbBytes(fieldNum, Buffer.from(String(str), 'utf8'), chunks);
}

function hexToBuf(hex) {
  const clean = String(hex || '').trim();
  if (clean.length % 2 !== 0) throw new Error(`odd hex length: ${clean.length}`);
  return Buffer.from(clean, 'hex');
}

function decodeWireFields(buf) {
  const fields = [];
  const state = { offset: 0 };
  while (state.offset < buf.length) {
    const tag = decodeVarint(buf, state);
    const fieldNum = Number(tag >> 3n);
    const wireType = Number(tag & 7n);
    if (fieldNum === 0) throw new Error('invalid protobuf field 0');
    if (wireType === 0) {
      fields.push([fieldNum, 'varint', decodeVarint(buf, state)]);
    } else if (wireType === 2) {
      const len = Number(decodeVarint(buf, state));
      if (state.offset + len > buf.length) throw new Error('truncated length-delimited field');
      fields.push([fieldNum, 'bytes', buf.subarray(state.offset, state.offset + len)]);
      state.offset += len;
    } else if (wireType === 1) {
      state.offset += 8;
    } else if (wireType === 5) {
      state.offset += 4;
    } else {
      throw new Error(`unsupported wire type ${wireType}`);
    }
  }
  return fields;
}

// ---------------------------------------------------------------------------
// Message Encoders / Decoders
// ---------------------------------------------------------------------------

function encodeDomain(domain) {
  const c = [];
  pbString(1, domain.schema, c);
  pbString(2, domain.chain_id, c);
  pbBytes(3, hexToBuf(domain.genesis_hash), c);
  pbVarint(4, domain.protocol_version, c);
  pbBytes(5, hexToBuf(domain.registry_id), c);
  return Buffer.concat(c);
}

function decodeDomain(buf) {
  const out = { schema: '', chain_id: '', genesis_hash: '', protocol_version: 0, registry_id: '' };
  for (const [num, type, val] of decodeWireFields(buf)) {
    if (num === 1 && type === 'bytes') out.schema = val.toString('utf8');
    else if (num === 2 && type === 'bytes') out.chain_id = val.toString('utf8');
    else if (num === 3 && type === 'bytes') out.genesis_hash = val.toString('hex');
    else if (num === 4 && type === 'varint') out.protocol_version = Number(val);
    else if (num === 5 && type === 'bytes') out.registry_id = val.toString('hex');
  }
  return out;
}

function encodeRecovery(rec) {
  const c = [];
  pbString(1, rec.schema, c);
  pbVarint(2, rec.committee_epoch, c);
  pbVarint(3, rec.valid_from_height, c);
  pbVarint(4, rec.expires_at_height, c);
  pbVarint(5, rec.recovery_closes_at_height, c);
  pbBytes(6, hexToBuf(rec.lock_id), c);
  return Buffer.concat(c);
}

function encodeSignedOrder(signed) {
  const c = [];
  const ownerPkLower = String(signed.owner_pubkey_hex || '').toLowerCase();
  pbBytes(1, hexToBuf(ownerPkLower), c);
  pbBytes(2, hexToBuf(signed.owner_signature_hex), c);
  pbBytes(3, encodeDomain(signed.order.domain), c);
  pbBytes(4, encodeRecovery(signed.order.recovery), c);
  for (const inp of signed.order.inputs || []) {
    const ic = [];
    pbString(1, inp.id, ic);
    pbVarint(2, inp.version, ic);
    pbBytes(5, Buffer.concat(ic), c);
  }
  for (const out of signed.order.outputs || []) {
    const oc = [];
    const outPkLower = String(out.owner_pubkey_hex || '').toLowerCase();
    if (outPkLower === ownerPkLower) {
      pbBool(4, true, oc);
    } else {
      pbBytes(1, hexToBuf(outPkLower), oc);
    }
    pbVarint(2, out.value, oc);
    pbString(3, out.asset, oc);
    pbBytes(6, Buffer.concat(oc), c);
  }
  pbVarint(7, signed.order.fee, c);
  pbVarint(8, signed.order.nonce, c);
  for (const memo of signed.order.memos || []) {
    const mc = [];
    pbString(1, memo.memo_type || '', mc);
    pbString(2, memo.memo_format || '', mc);
    pbString(3, memo.memo_data || '', mc);
    pbBytes(9, Buffer.concat(mc), c);
  }
  return Buffer.concat(c);
}

function encodeVote(vote) {
  const c = [];
  pbString(1, vote.validator_id, c);
  pbBytes(2, hexToBuf(vote.signature_hex), c);
  return Buffer.concat(c);
}

function decodeVote(buf) {
  const out = { validator_id: '', signature_hex: '' };
  for (const [num, type, val] of decodeWireFields(buf)) {
    if (num === 1 && type === 'bytes') out.validator_id = val.toString('utf8');
    else if (num === 2 && type === 'bytes') out.signature_hex = val.toString('hex');
  }
  return out;
}

function decodeApplyAck(buf) {
  let schema = '';
  let domain = null;
  let committee_epoch = 0;
  let lock_id = '';
  let order_digest = '';
  let certificate_digest = '';
  let terminal_state_digest = '';
  let validator_id = '';
  let signature_hex = '';
  let signed = false;

  for (const [num, type, val] of decodeWireFields(buf)) {
    if (num === 1 && type === 'bytes') schema = val.toString('utf8');
    else if (num === 2 && type === 'bytes') domain = decodeDomain(val);
    else if (num === 3 && type === 'varint') committee_epoch = Number(val);
    else if (num === 4 && type === 'bytes') lock_id = val.toString('hex');
    else if (num === 5 && type === 'bytes') order_digest = val.toString('hex');
    else if (num === 6 && type === 'bytes') certificate_digest = val.toString('hex');
    else if (num === 7 && type === 'bytes') terminal_state_digest = val.toString('hex');
    else if (num === 8 && type === 'bytes') validator_id = val.toString('utf8');
    else if (num === 9 && type === 'bytes') signature_hex = val.toString('hex');
    else if (num === 10 && type === 'varint') signed = val !== 0n;
  }
  if (!signed) {
    return { schema, validator_id, lock_id, certificate_digest, signed: false };
  }
  return {
    schema,
    domain,
    committee_epoch,
    lock_id,
    order_digest,
    certificate_digest,
    terminal_state_digest,
    validator_id,
    signature_hex,
  };
}

function wrapV2Frame(body) {
  if (body.length === 0 || body.length > MAX_FASTPAY_V2_FRAME_BYTES) {
    throw new Error(`FastPay v2 request frame size ${body.length} exceeds cap ${MAX_FASTPAY_V2_FRAME_BYTES}`);
  }
  const hdr = Buffer.allocUnsafe(5);
  hdr[0] = FASTPAY_RPC_V2_VERSION;
  hdr.writeUInt32BE(body.length, 1);
  return Buffer.concat([hdr, body]);
}

function encodeSignRequestFrame({ requestId, validatorId, signedOrder }) {
  const c = [];
  pbVarint(1, requestId, c);
  pbVarint(2, METHOD_OWNED_SIGN_V3, c);
  pbString(3, validatorId, c);
  pbBytes(4, encodeSignedOrder(signedOrder), c);
  return wrapV2Frame(Buffer.concat(c));
}

function encodeApplyFastRequestFrame({ requestId, validatorId, lockId, votes }) {
  const inner = [];
  pbBytes(1, hexToBuf(lockId), inner);
  for (const v of votes) pbBytes(2, encodeVote(v), inner);
  const c = [];
  pbVarint(1, requestId, c);
  pbVarint(2, METHOD_OWNED_APPLY_V3_FAST, c);
  pbString(3, validatorId, c);
  pbBytes(5, Buffer.concat(inner), c);
  return wrapV2Frame(Buffer.concat(c));
}

function encodeApplyFullRequestFrame({ requestId, validatorId, certificate }) {
  const inner = [];
  pbBytes(
    1,
    encodeSignedOrder({
      order: certificate.order,
      owner_pubkey_hex: certificate.owner_pubkey_hex,
      owner_signature_hex: certificate.owner_signature_hex,
    }),
    inner
  );
  for (const v of certificate.votes) pbBytes(2, encodeVote(v), inner);
  const c = [];
  pbVarint(1, requestId, c);
  pbVarint(2, METHOD_OWNED_APPLY_V3_FULL, c);
  pbString(3, validatorId, c);
  pbBytes(6, Buffer.concat(inner), c);
  return wrapV2Frame(Buffer.concat(c));
}

function decodeResponseBody(buf) {
  const resp = {
    requestId: 0,
    status: STATUS_OK,
    errorMessage: '',
    vote: null,
    applyAck: null,
  };
  for (const [num, type, val] of decodeWireFields(buf)) {
    if (num === 1 && type === 'varint') resp.requestId = Number(val);
    else if (num === 2 && type === 'varint') resp.status = Number(val);
    else if (num === 3 && type === 'bytes') resp.errorMessage = val.toString('utf8');
    else if (num === 4 && type === 'bytes') resp.vote = decodeVote(val);
    else if (num === 5 && type === 'bytes') resp.applyAck = decodeApplyAck(val);
  }
  return resp;
}

// ---------------------------------------------------------------------------
// Persistent Direct TLS 1.3 Client (`TCP_NODELAY = true`)
// ---------------------------------------------------------------------------

class FastPayRpcV2Client {
  constructor({ host, port, tlsOptions = {}, timeoutMs = 15_000 }) {
    this.host = host;
    this.port = Number(port);
    this.tlsOptions = tlsOptions;
    this.timeoutMs = timeoutMs;
    this.socket = null;
    this.connectPromise = null;
    this.recvBuf = Buffer.alloc(0);
    this.nextRequestId = 1;
    this.pendingQueue = [];
  }

  async ensureConnected() {
    if (this.socket && !this.socket.destroyed && this.socket.writable) return;
    if (this.connectPromise) return this.connectPromise;
    this.connectPromise = new Promise((resolve, reject) => {
      const sock = tls.connect(
        {
          host: this.host,
          port: this.port,
          minVersion: 'TLSv1.3',
          maxVersion: 'TLSv1.3',
          ...this.tlsOptions,
        },
        () => {
          sock.setNoDelay(true);
          sock.setKeepAlive(true, 10_000);
          this.socket = sock;
          this.connectPromise = null;
          resolve();
        }
      );
      sock.on('data', (chunk) => this._onData(chunk));
      sock.on('error', (err) => {
        this.connectPromise = null;
        this._failAll(err);
        reject(err);
      });
      sock.on('close', () => {
        this.socket = null;
        this.connectPromise = null;
        this._failAll(new Error(`FastPay v2 TLS socket closed (${this.host}:${this.port})`));
      });
    });
    return this.connectPromise;
  }

  _failAll(err) {
    while (this.pendingQueue.length > 0) {
      const item = this.pendingQueue.shift();
      clearTimeout(item.timer);
      item.reject(err);
    }
  }

  _onData(chunk) {
    this.recvBuf = this.recvBuf.length === 0 ? chunk : Buffer.concat([this.recvBuf, chunk]);
    while (this.recvBuf.length >= 5) {
      const version = this.recvBuf[0];
      const frameLen = this.recvBuf.readUInt32BE(1);
      if (this.recvBuf.length < 5 + frameLen) break;
      const body = this.recvBuf.subarray(5, 5 + frameLen);
      this.recvBuf = this.recvBuf.subarray(5 + frameLen);
      const item = this.pendingQueue.shift();
      if (!item) continue;
      clearTimeout(item.timer);
      try {
        if (version !== FASTPAY_RPC_V2_VERSION) {
          item.reject(new Error(`UnsupportedProtocolVersion: ${version}`));
        } else {
          item.resolve(decodeResponseBody(body));
        }
      } catch (err) {
        item.reject(err);
      }
    }
  }

  async _sendFrame(frameBuf) {
    await this.ensureConnected();
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        const idx = this.pendingQueue.findIndex((p) => p.timer === timer);
        if (idx >= 0) this.pendingQueue.splice(idx, 1);
        reject(new Error(`FastPay v2 RPC timeout after ${this.timeoutMs}ms (${this.host}:${this.port})`));
      }, this.timeoutMs);
      this.pendingQueue.push({ resolve, reject, timer });
      this.socket.write(frameBuf);
    });
  }

  async ownedSignV3(validatorId, signedOrder) {
    const requestId = this.nextRequestId++;
    const frame = encodeSignRequestFrame({ requestId, validatorId, signedOrder });
    const resp = await this._sendFrame(frame);
    if (resp.status !== STATUS_OK) {
      const err = new Error(resp.errorMessage || `FastPay v2 sign error status=${resp.status}`);
      err.v2Status = resp.status;
      throw err;
    }
    return { vote: resp.vote, wireBytes: frame.length };
  }

  async ownedApplyV3FastOrFallback(validatorId, certificate) {
    const requestId = this.nextRequestId++;
    const fastFrame = encodeApplyFastRequestFrame({
      requestId,
      validatorId,
      lockId: certificate.order.recovery.lock_id,
      votes: certificate.votes,
    });
    const resp = await this._sendFrame(fastFrame);
    if (resp.status === STATUS_OK) {
      return { ack: resp.applyAck, usedFastPath: true, wireBytes: fastFrame.length };
    }
    if (resp.status === STATUS_ORDER_NOT_CACHED) {
      const fallbackReqId = this.nextRequestId++;
      const fullFrame = encodeApplyFullRequestFrame({
        requestId: fallbackReqId,
        validatorId,
        certificate,
      });
      const fullResp = await this._sendFrame(fullFrame);
      if (fullResp.status !== STATUS_OK) {
        const err = new Error(fullResp.errorMessage || `FastPay v2 full apply error status=${fullResp.status}`);
        err.v2Status = fullResp.status;
        throw err;
      }
      return { ack: fullResp.applyAck, usedFastPath: false, wireBytes: fullFrame.length };
    }
    const err = new Error(resp.errorMessage || `FastPay v2 fast apply error status=${resp.status}`);
    err.v2Status = resp.status;
    throw err;
  }

  close() {
    if (this.socket && !this.socket.destroyed) {
      this.socket.destroy();
    }
    this.socket = null;
  }
}

module.exports = {
  FASTPAY_RPC_V2_VERSION,
  MAX_FASTPAY_V2_FRAME_BYTES,
  METHOD_OWNED_SIGN_V3,
  METHOD_OWNED_APPLY_V3_FAST,
  METHOD_OWNED_APPLY_V3_FULL,
  STATUS_OK,
  STATUS_INVALID_INPUT,
  STATUS_UNSUPPORTED_PROTOCOL_VERSION,
  STATUS_ORDER_NOT_CACHED,
  STATUS_RATE_LIMITED,
  STATUS_INTERNAL_ERROR,
  encodeSignRequestFrame,
  encodeApplyFastRequestFrame,
  encodeApplyFullRequestFrame,
  decodeResponseBody,
  FastPayRpcV2Client,
};
