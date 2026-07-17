const MAX_INTENT_BYTES = 128 * 1024;
const MAX_STRING_BYTES = 4096;
const MAX_ASSET_INPUTS = 16;
const MAX_FEE_INPUTS = 4;
const textEncoder = new TextEncoder();

function bytes(value, length, name) {
  let result;
  if (value instanceof Uint8Array) result = value;
  else if (Array.isArray(value)) result = Uint8Array.from(value);
  else if (typeof value === 'string' && /^[0-9a-fA-F]*$/.test(value) && value.length % 2 === 0) {
    result = Uint8Array.from(value.match(/../g) || [], (pair) => Number.parseInt(pair, 16));
  } else throw new TypeError(`${name} must be bytes or even-length hex`);
  if (length !== null && result.length !== length) {
    throw new RangeError(`${name} must be exactly ${length} bytes`);
  }
  return result;
}

function boundedU64(value, name) {
  const parsed = BigInt(value);
  if (parsed < 0n || parsed > 0xffff_ffff_ffff_ffffn) {
    throw new RangeError(`${name} is outside u64`);
  }
  return parsed;
}

function compareBytes(left, right) {
  const length = Math.min(left.length, right.length);
  for (let index = 0; index < length; index += 1) {
    if (left[index] !== right[index]) return left[index] - right[index];
  }
  return left.length - right.length;
}

function compareKeys(left, right) {
  return compareBytes(bytes(left.object_id, 32, 'object_id'), bytes(right.object_id, 32, 'object_id'))
    || (boundedU64(left.version, 'object version') < boundedU64(right.version, 'object version') ? -1
      : boundedU64(left.version, 'object version') > boundedU64(right.version, 'object version') ? 1 : 0);
}

class Encoder {
  constructor() { this.parts = []; this.length = 0; }
  fixed(value) {
    if (this.length + value.length > MAX_INTENT_BYTES) throw new RangeError('canonical intent too large');
    this.parts.push(value); this.length += value.length;
  }
  u8(value) { this.fixed(Uint8Array.of(value)); }
  u16(value) { const part = new Uint8Array(2); new DataView(part.buffer).setUint16(0, value); this.fixed(part); }
  u32(value) { const part = new Uint8Array(4); new DataView(part.buffer).setUint32(0, value); this.fixed(part); }
  u64(value, name) {
    const part = new Uint8Array(8);
    new DataView(part.buffer).setBigUint64(0, boundedU64(value, name));
    this.fixed(part);
  }
  lengthBytes(value) { this.u32(value.length); this.fixed(value); }
  string(value, name) {
    if (typeof value !== 'string' || value.length === 0) throw new TypeError(`${name} must be non-empty`);
    const encoded = textEncoder.encode(value);
    if (encoded.length > MAX_STRING_BYTES) throw new RangeError(`${name} too large`);
    this.lengthBytes(encoded);
  }
  finish() {
    const result = new Uint8Array(this.length);
    let offset = 0;
    for (const part of this.parts) { result.set(part, offset); offset += part.length; }
    return result;
  }
}

function encodeKeys(encoder, keys, maximum, name) {
  if (!Array.isArray(keys) || keys.length > maximum) throw new RangeError(`${name} too large`);
  for (let index = 1; index < keys.length; index += 1) {
    if (compareKeys(keys[index - 1], keys[index]) >= 0) throw new Error(`${name} must be sorted and unique`);
  }
  encoder.u16(keys.length);
  for (const key of keys) {
    encoder.fixed(bytes(key.object_id, 32, `${name}.object_id`));
    encoder.u64(key.version, `${name}.version`);
  }
}

function partyOrderKey(party) {
  return [
    bytes(party.offered_asset_id, 48, 'offered_asset_id'),
    textEncoder.encode(party.owner_address),
    bytes(party.owner_pubkey, null, 'owner_pubkey'),
  ];
}

function compareParty(left, right) {
  const a = partyOrderKey(left); const b = partyOrderKey(right);
  return compareBytes(a[0], b[0]) || compareBytes(a[1], b[1]) || compareBytes(a[2], b[2]);
}

function encodeParty(encoder, party) {
  encoder.string(party.owner_address, 'owner_address');
  const owner = bytes(party.owner_pubkey, null, 'owner_pubkey');
  if (owner.length === 0) throw new RangeError('owner_pubkey must be non-empty');
  encoder.lengthBytes(owner);
  encoder.fixed(bytes(party.offered_asset_id, 48, 'offered_asset_id'));
  encoder.fixed(bytes(party.offered_asset_rule_hash, 48, 'offered_asset_rule_hash'));
  encoder.u64(party.offered_amount, 'offered_amount');
  encoder.fixed(bytes(party.receives_asset_id, 48, 'receives_asset_id'));
  encoder.fixed(bytes(party.receives_asset_rule_hash, 48, 'receives_asset_rule_hash'));
  if (party.receives_holder_permit_id === null || party.receives_holder_permit_id === undefined) encoder.u8(0);
  else { encoder.u8(1); encoder.fixed(bytes(party.receives_holder_permit_id, 48, 'holder_permit_id')); }
  encoder.u64(party.receives_amount, 'receives_amount');
  encodeKeys(encoder, party.asset_inputs, MAX_ASSET_INPUTS, 'asset_inputs');
  encodeKeys(encoder, party.fee_inputs, MAX_FEE_INPUTS, 'fee_inputs');
  encoder.u64(party.asset_change, 'asset_change');
  encoder.u64(party.fee_change, 'fee_change');
  encoder.u64(party.fee_burn_pft, 'fee_burn_pft');
}

export function canonicalFastSwapIntentBytes(intent) {
  const domain = intent.domain;
  const validatorCount = Number(domain.validator_count);
  if (!Number.isInteger(validatorCount) || validatorCount < 4 || validatorCount > 64) {
    throw new RangeError('validator_count outside 4..=64');
  }
  if (Number(domain.quorum) !== Math.floor((2 * validatorCount) / 3) + 1) {
    throw new Error('non-canonical committee quorum');
  }
  if (Number(domain.fastswap_schema_version) !== 1) throw new Error('unsupported FastSwap schema');
  if (compareParty(intent.party_0, intent.party_1) >= 0) throw new Error('non-canonical party order');
  if (compareBytes(bytes(intent.party_0.offered_asset_id, 48, 'party_0 asset'), bytes(intent.party_1.offered_asset_id, 48, 'party_1 asset')) === 0) {
    throw new Error('parties must offer distinct assets');
  }
  if (compareBytes(bytes(intent.party_0.owner_pubkey, null, 'party_0 owner'), bytes(intent.party_1.owner_pubkey, null, 'party_1 owner')) === 0) {
    throw new Error('parties must have distinct owners');
  }

  const encoder = new Encoder();
  encoder.fixed(textEncoder.encode('PFFASTSWAPINTENT'));
  encoder.string(domain.chain.chain_id, 'chain_id');
  encoder.fixed(bytes(domain.chain.genesis_hash, 48, 'genesis_hash'));
  encoder.u32(Number(domain.chain.protocol_version));
  encoder.u32(Number(domain.fastswap_schema_version));
  encoder.u64(domain.committee_epoch, 'committee_epoch');
  encoder.fixed(bytes(domain.committee_root, 48, 'committee_root'));
  encoder.u16(validatorCount);
  encoder.u16(Number(domain.quorum));
  encoder.fixed(bytes(intent.policy_hash, 48, 'policy_hash'));
  encoder.fixed(bytes(intent.rfq_hash, 48, 'rfq_hash'));
  encoder.fixed(bytes(intent.market_envelope_hash, 48, 'market_envelope_hash'));
  encoder.u64(intent.nav_epoch, 'nav_epoch');
  encoder.u64(intent.expires_at_height, 'expires_at_height');
  encoder.fixed(bytes(intent.nonce, 32, 'nonce'));
  encodeParty(encoder, intent.party_0);
  encodeParty(encoder, intent.party_1);
  return encoder.finish();
}

export function bytesToHex(value) {
  return Array.from(value, (byte) => byte.toString(16).padStart(2, '0')).join('');
}
