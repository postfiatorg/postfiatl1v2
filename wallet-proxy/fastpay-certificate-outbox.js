'use strict';

const crypto = require('crypto');
const fs = require('fs');
const path = require('path');

const LEGACY_SCHEMA = 'postfiat-fastpay-certificate-outbox-v1';
const SCHEMA = 'postfiat-fastpay-certificate-outbox-v2';
const RECORD_SCHEMA = 'postfiat-fastpay-certificate-outbox-record-v2';
const COMPLETED_SCHEMA = 'postfiat-fastpay-certificate-completed-v1';
const MAX_PENDING_CERTIFICATES = 1024;
const MAX_COMPLETED_CERTIFICATES = 1024;
const COMPLETED_RETENTION_MS = 7 * 24 * 60 * 60 * 1000;
const MAX_OUTBOX_BYTES = 16 * 1024 * 1024;

function clone(value) {
  return structuredClone(value);
}

function canonicalJson(value) {
  if (Array.isArray(value)) {
    return `[${value.map(canonicalJson).join(',')}]`;
  }
  if (value && typeof value === 'object') {
    return `{${Object.keys(value).sort().map(key => (
      `${JSON.stringify(key)}:${canonicalJson(value[key])}`
    )).join(',')}}`;
  }
  return JSON.stringify(value);
}

function sha256(domain, value) {
  return crypto.createHash('sha256')
    .update(domain)
    .update('\0')
    .update(value)
    .digest('hex');
}

function operationDigest(record) {
  const certJson = record?.request?.params?.cert_json;
  if (typeof record?.method !== 'string' || typeof certJson !== 'string' || certJson.length === 0) {
    throw new Error('FastPay certificate outbox record requires method and cert_json');
  }
  return sha256(
    'postfiat.fastpay.certificate-outbox-operation.v2',
    `${record.method}\0${certJson}`,
  );
}

function terminalDigest(result) {
  return sha256('postfiat.fastpay.certificate-outbox-terminal.v1', canonicalJson(result));
}

function validateTerminalResult(record, result) {
  if (!result || typeof result !== 'object' || Array.isArray(result)
      || result.schema !== 'postfiat-fastpay-certificate-finality-v1'
      || result.certificate_id !== record.certificate_id
      || result.method !== record.method
      || result.certificate_final !== true) {
    throw new Error('FastPay terminal result is not bound to its certificate operation');
  }
  if (record.method !== 'owned_apply_v3' && record.method !== 'owned_unwrap_apply_v3') return;
  const quorum = result.certificate_quorum;
  const acknowledgements = Array.isArray(result.apply_acknowledgements)
    ? result.apply_acknowledgements : [];
  if (!Number.isSafeInteger(quorum) || quorum <= 0 || acknowledgements.length < quorum) {
    throw new Error('FastPay v3 terminal result lacks its signed acknowledgement quorum');
  }
  const storedByValidator = new Map(
    record.apply_acknowledgements.map(acknowledgement => [
      acknowledgement.validator_id,
      canonicalJson(acknowledgement),
    ]),
  );
  const terminalValidators = new Set();
  for (const acknowledgement of acknowledgements) {
    const validatorId = acknowledgement?.validator_id;
    if (typeof validatorId !== 'string'
        || terminalValidators.has(validatorId)
        || storedByValidator.get(validatorId) !== canonicalJson(acknowledgement)) {
      throw new Error('FastPay v3 terminal result conflicts with its signed acknowledgements');
    }
    terminalValidators.add(validatorId);
  }
}

function normalizedAcknowledgements(record) {
  const acknowledgements = new Map();
  for (const acknowledgement of Array.isArray(record.apply_acknowledgements)
    ? record.apply_acknowledgements : []) {
    if (acknowledgement?.schema !== 'postfiat-fastpay-apply-ack-v1'
        || typeof acknowledgement.validator_id !== 'string') {
      throw new Error('FastPay certificate outbox contains a malformed apply acknowledgement');
    }
    acknowledgements.set(acknowledgement.validator_id, clone(acknowledgement));
  }
  return [...acknowledgements.values()]
    .sort((left, right) => left.validator_id.localeCompare(right.validator_id));
}

class FastpayCertificateOutbox {
  constructor(filePath, options = {}) {
    if (!filePath || typeof filePath !== 'string') {
      throw new Error('FastPay certificate outbox path is required');
    }
    this.filePath = filePath;
    this.now = typeof options.now === 'function' ? options.now : Date.now;
    this.maxCompletedCertificates = Number.isInteger(options.maxCompletedCertificates)
      && options.maxCompletedCertificates > 0
      ? Math.min(options.maxCompletedCertificates, MAX_COMPLETED_CERTIFICATES)
      : MAX_COMPLETED_CERTIFICATES;
    this.completedTtlMs = Number.isInteger(options.completedTtlMs)
      && options.completedTtlMs > 0
      ? options.completedTtlMs
      : COMPLETED_RETENTION_MS;
    this.records = new Map();
    this.completedRecords = new Map();
    this.load();
  }

  load() {
    let document;
    try {
      const stat = fs.statSync(this.filePath);
      if (!stat.isFile() || stat.size > MAX_OUTBOX_BYTES) {
        throw new Error('outbox is not a bounded regular file');
      }
      document = JSON.parse(fs.readFileSync(this.filePath, 'utf8'));
    } catch (error) {
      if (error.code === 'ENOENT') return;
      throw new Error(`FastPay certificate outbox load failed: ${error.message}`);
    }
    const legacy = document?.schema === LEGACY_SCHEMA;
    if ((!legacy && document?.schema !== SCHEMA) || !Array.isArray(document.records)) {
      throw new Error('FastPay certificate outbox has an invalid schema');
    }
    const completed = legacy ? [] : document.completed;
    if (!Array.isArray(completed)) {
      throw new Error('FastPay certificate outbox has an invalid completed-record set');
    }
    if (document.records.length > MAX_PENDING_CERTIFICATES
        || completed.length > MAX_COMPLETED_CERTIFICATES) {
      throw new Error('FastPay certificate outbox exceeds its record limit');
    }
    for (const value of document.records) {
      const record = this.normalizeRecord(value, false);
      if (this.records.has(record.certificate_id)) {
        throw new Error('FastPay certificate outbox contains a duplicate pending certificate');
      }
      this.records.set(record.certificate_id, record);
    }
    for (const value of completed) {
      const record = this.normalizeRecord(value, true);
      if (this.records.has(record.certificate_id)
          || this.completedRecords.has(record.certificate_id)) {
        throw new Error('FastPay certificate outbox contains a duplicate certificate');
      }
      this.completedRecords.set(record.certificate_id, record);
    }
    const compacted = this.compact(false);
    if (legacy || compacted) this.persist();
  }

  normalizeRecord(value, completed) {
    if (!value || typeof value.certificate_id !== 'string'
        || typeof value.method !== 'string' || !value.request) {
      throw new Error('FastPay certificate outbox contains a malformed record');
    }
    const digest = operationDigest(value);
    if (value.operation_digest !== undefined && value.operation_digest !== digest) {
      throw new Error('FastPay certificate outbox operation digest mismatch');
    }
    const record = {
      schema: completed ? COMPLETED_SCHEMA : RECORD_SCHEMA,
      certificate_id: value.certificate_id,
      method: value.method,
      request: clone(value.request),
      operation_digest: digest,
      created_at_ms: Number.isSafeInteger(value.created_at_ms) && value.created_at_ms >= 0
        ? value.created_at_ms : 0,
      applied_validators: [...new Set(
        (Array.isArray(value.applied_validators) ? value.applied_validators : [])
          .filter(validator => typeof validator === 'string'),
      )].sort(),
      apply_acknowledgements: normalizedAcknowledgements(value),
      terminal_result: value.terminal_result === undefined || value.terminal_result === null
        ? null : clone(value.terminal_result),
      terminal_digest: value.terminal_digest || null,
    };
    if (record.terminal_result !== null) {
      validateTerminalResult(record, record.terminal_result);
      const digestValue = terminalDigest(record.terminal_result);
      if (record.terminal_digest !== null && record.terminal_digest !== digestValue) {
        throw new Error('FastPay certificate outbox terminal digest mismatch');
      }
      record.terminal_digest = digestValue;
    } else if (record.terminal_digest !== null) {
      throw new Error('FastPay certificate outbox has a terminal digest without a result');
    }
    if (completed) {
      if (record.terminal_result === null
          || !Number.isSafeInteger(value.completed_at_ms)
          || value.completed_at_ms < 0) {
        throw new Error('FastPay certificate outbox contains a malformed completed record');
      }
      record.completed_at_ms = value.completed_at_ms;
    }
    return record;
  }

  assertMatchingOperation(existing, candidate) {
    if (operationDigest(candidate) !== existing.operation_digest
        || candidate.method !== existing.method) {
      throw new Error(`FastPay certificate ${candidate.certificate_id} conflicts with its durable record`);
    }
  }

  pending() {
    return [...this.records.values()].map(clone);
  }

  completed() {
    this.compact();
    return [...this.completedRecords.values()].map(clone);
  }

  terminal(record) {
    this.compact();
    const existing = this.records.get(record.certificate_id)
      || this.completedRecords.get(record.certificate_id);
    if (!existing) return null;
    this.assertMatchingOperation(existing, record);
    if (existing.terminal_result === null) return null;
    validateTerminalResult(existing, existing.terminal_result);
    if (terminalDigest(existing.terminal_result) !== existing.terminal_digest) {
      throw new Error('FastPay certificate outbox terminal digest mismatch');
    }
    return clone(existing.terminal_result);
  }

  enqueue(record) {
    const existing = this.records.get(record.certificate_id)
      || this.completedRecords.get(record.certificate_id);
    if (existing) {
      this.assertMatchingOperation(existing, record);
      return clone(existing);
    }
    if (this.records.size >= MAX_PENDING_CERTIFICATES) {
      throw new Error('FastPay certificate outbox is full');
    }
    const stored = this.normalizeRecord({
      ...record,
      schema: RECORD_SCHEMA,
      operation_digest: operationDigest(record),
      created_at_ms: Number(record.created_at_ms) || this.now(),
      applied_validators: [],
      apply_acknowledgements: [],
      terminal_result: null,
      terminal_digest: null,
    }, false);
    this.records.set(stored.certificate_id, stored);
    try {
      this.persist();
    } catch (error) {
      this.records.delete(stored.certificate_id);
      throw error;
    }
    return clone(stored);
  }

  markApplied(certificateId, validatorId, acknowledgement = null) {
    const record = this.records.get(certificateId);
    if (!record) return null;
    const before = clone(record);
    let changed = false;
    if (!record.applied_validators.includes(validatorId)) {
      record.applied_validators.push(validatorId);
      record.applied_validators.sort();
      changed = true;
    }
    if (acknowledgement !== null) {
      if (acknowledgement?.schema !== 'postfiat-fastpay-apply-ack-v1'
          || acknowledgement.validator_id !== validatorId) {
        throw new Error('FastPay apply acknowledgement does not match its validator');
      }
      const withoutValidator = record.apply_acknowledgements
        .filter(value => value.validator_id !== validatorId);
      withoutValidator.push(clone(acknowledgement));
      withoutValidator.sort((left, right) => left.validator_id.localeCompare(right.validator_id));
      record.apply_acknowledgements = withoutValidator;
      changed = true;
    }
    if (changed) {
      try {
        this.persist();
      } catch (error) {
        this.records.set(certificateId, before);
        throw error;
      }
    }
    return clone(record);
  }

  markTerminal(certificateId, terminalResultValue) {
    const record = this.records.get(certificateId);
    if (!record) return null;
    if (!terminalResultValue || typeof terminalResultValue !== 'object'
        || Array.isArray(terminalResultValue)) {
      throw new Error('FastPay terminal result must be an object');
    }
    const before = clone(record);
    validateTerminalResult(record, terminalResultValue);
    const digest = terminalDigest(terminalResultValue);
    if (record.terminal_result !== null && record.terminal_digest !== digest) {
      throw new Error('FastPay certificate terminal result conflicts with its durable record');
    }
    record.terminal_result = clone(terminalResultValue);
    record.terminal_digest = digest;
    try {
      this.persist();
    } catch (error) {
      this.records.set(certificateId, before);
      throw error;
    }
    return clone(record);
  }

  complete(certificateId) {
    if (this.completedRecords.has(certificateId)) return true;
    const record = this.records.get(certificateId);
    if (!record || record.terminal_result === null) return false;
    const pendingBefore = new Map(this.records);
    const completedBefore = new Map(this.completedRecords);
    const completed = {
      ...clone(record),
      schema: COMPLETED_SCHEMA,
      completed_at_ms: this.now(),
    };
    this.records.delete(certificateId);
    this.completedRecords.set(certificateId, completed);
    this.compact(false);
    try {
      this.persist();
    } catch (error) {
      this.records = pendingBefore;
      this.completedRecords = completedBefore;
      throw error;
    }
    return true;
  }

  compact(persist = true) {
    const now = this.now();
    const ordered = [...this.completedRecords.values()].sort((left, right) => (
      left.completed_at_ms - right.completed_at_ms
        || left.certificate_id.localeCompare(right.certificate_id)
    ));
    let changed = false;
    for (const record of ordered) {
      if (record.completed_at_ms + this.completedTtlMs <= now) {
        this.completedRecords.delete(record.certificate_id);
        changed = true;
      }
    }
    const retained = ordered.filter(record => this.completedRecords.has(record.certificate_id));
    while (retained.length > this.maxCompletedCertificates) {
      const record = retained.shift();
      this.completedRecords.delete(record.certificate_id);
      changed = true;
    }
    if (changed && persist) this.persist();
    return changed;
  }

  persist() {
    const parent = path.dirname(this.filePath);
    fs.mkdirSync(parent, { recursive: true, mode: 0o700 });
    const body = `${JSON.stringify({
      schema: SCHEMA,
      records: [...this.records.values()],
      completed: [...this.completedRecords.values()],
    }, null, 2)}\n`;
    if (Buffer.byteLength(body, 'utf8') > MAX_OUTBOX_BYTES) {
      throw new Error('FastPay certificate outbox exceeds its byte limit');
    }
    const temp = `${this.filePath}.tmp-${process.pid}-${this.now()}`;
    let fd;
    try {
      fd = fs.openSync(temp, 'wx', 0o600);
      fs.writeFileSync(fd, body, 'utf8');
      fs.fsyncSync(fd);
      fs.closeSync(fd);
      fd = undefined;
      fs.renameSync(temp, this.filePath);
    } catch (error) {
      if (fd !== undefined) fs.closeSync(fd);
      fs.rmSync(temp, { force: true });
      throw error;
    }
    const dirFd = fs.openSync(parent, 'r');
    try {
      fs.fsyncSync(dirFd);
    } finally {
      fs.closeSync(dirFd);
    }
  }
}

module.exports = {
  COMPLETED_RETENTION_MS,
  FastpayCertificateOutbox,
  MAX_COMPLETED_CERTIFICATES,
  MAX_OUTBOX_BYTES,
  MAX_PENDING_CERTIFICATES,
  SCHEMA,
};
