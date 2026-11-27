'use strict';

// Deterministic Contract C1 vector round-trips + full-size real PQ (ML-DSA-65 +
// IETF X-Wing ml_kem768_x25519 + XChaCha20-Poly1305) envelope & manifest tests.

const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const mp = require('./message-protocol');

const ADDRESS_VECTORS_PATH = path.resolve(__dirname, '..', 'docs', 'fixtures', 'addresses', 'pftl-address-vectors-v1.json');

// The frozen Contract C1 vectors: the JSON block of docs/specs/message-protocol-v1.md is the one source.
const SPEC_DOC_PATH = path.resolve(__dirname, '..', 'docs', 'specs', 'message-protocol-v1.md');
const C1_DOC = JSON.parse(
  /<!--\s*BEGIN_CANONICAL_TEST_VECTORS_JSON\s*-->\s*```json\s*(\{[\s\S]*?\})\s*```\s*<!--\s*END_CANONICAL_TEST_VECTORS_JSON\s*-->/.exec(
    fs.readFileSync(SPEC_DOC_PATH, 'utf8')
  )[1]
);
const C1 = {
  V1: C1_DOC.V1_POINTER_V5_PUBLIC_TEXT,
  V2A: C1_DOC.V2A_POINTER_V5_WORST_CASE_BUDGET,
  V2B: C1_DOC.V2B_POINTER_V5_INLINE_X402,
  V3: C1_DOC.V3_LEGACY_POINTER_V4_DECODE,
  V4: C1_DOC.V4_ENVELOPE_PUBLIC_TEXT,
  V5: C1_DOC.V5_ENVELOPE_PRIVATE_XWING_REWARD,
  V6: C1_DOC.V6_MANIFEST_MULTICHUNK,
  V7: C1_DOC.V7_X402_HEADERS,
  V8: C1_DOC.V8_NFT_METADATA_AND_MIGRATION,
  V9: C1_DOC.V9_GOVERNANCE_PACKET_AND_LIFECYCLE,
};

// Every protobuf payload of the canonical vectors, as listed by vector_proto_payloads in
// scripts/test-message-protocol-vectors.py: [vector, field, message type].
const CANONICAL_PAYLOADS = [
  ['V1', 'memo_data_hex', 'Pointer'],
  ['V2A', 'memo_data_hex', 'Pointer'],
  ['V2B', 'memo_data_hex', 'Pointer'],
  ['V3', 'memo_data_hex_upper', 'LegacyPointerV4'],
  ['V4', 'envelope_hex', 'Envelope'],
  ['V4', 'header_hex', 'MessageHeader'],
  ['V4', 'body_hex', 'TextMessage'],
  ['V5', 'envelope_hex', 'Envelope'],
  ['V5', 'header_hex', 'MessageHeader'],
  ['V5', 'encryption_header_hex', 'XWingEncryptionHeader'],
  ['V6', 'manifest_hex', 'Manifest'],
  ['V7', 'required_hex', 'X402PaymentRequired'],
  ['V7', 'signature_hex', 'X402PaymentSignature'],
  ['V7', 'response_hex', 'X402PaymentResponse'],
  ['V8', 'claim_hex', null], // postfiat.nft.v1.MigrationClaim: outside this SDK
  ['V8', 'metadata_hex', null], // postfiat.nft.v1.NftMetadata: outside this SDK
  ['V8', 'metadata_envelope_hex', 'Envelope'],
  ['V9', 'packet_proto_hex', null], // postfiat.governance.v1.GovernancePacket: outside this SDK
  ['V9', 'packet_envelope_hex', 'Envelope'],
  ['V9', 'anchor_pointer_v5_hex', 'Pointer'],
  ['V9', 'transcript_hex', null], // postfiat.governance.v1.SessionTranscript: outside this SDK
  ['V9', 'minutes_hex', null], // postfiat.governance.v1.GovernanceMinutes: outside this SDK
  ['V9', 'ruling_hex', null], // postfiat.governance.v1.RulingAnchor: outside this SDK
];

function testCanonicalVectors() {
  // V1 Pointer v5
  const memo1 = mp.encodePointerV5Memo({
    cid_bin: Buffer.from(C1.V1.cid_bin_hex, 'hex'),
    target: mp.PointerTarget.CONTENT_BLOB,
    kind: mp.ContentKind.CHAT,
    schema: mp.SCHEMA_ID.TEXT_V1,
    thread_id: 'thread-c1-001',
    flags: mp.PointerFlag.PUBLIC,
  });
  assert.equal(memo1.memo_type, C1.V1.memo_type_hex);
  assert.equal(memo1.memo_format, C1.V1.memo_format_hex);
  assert.equal(memo1.memo_data, C1.V1.memo_data_hex);
  assert.equal(mp.cidBinToMultibase(Buffer.from(C1.V1.cid_bin_hex, 'hex')), C1.V1.cid_multibase);
  assert.equal(mp.multibaseToCidBin(C1.V1.cid_multibase).toString('hex'), C1.V1.cid_bin_hex);
  assert.equal(mp.encodePointer(mp.decodePointerV5Memo(memo1, { enforceCanonicalOrder: true })).toString('hex'), C1.V1.memo_data_hex);

  // V2A worst-case 256-byte budget + V2B inline x402
  const ptr2a = mp.decodePointer(Buffer.from(C1.V2A.memo_data_hex, 'hex'), { enforceCanonicalOrder: true });
  assert.equal(mp.encodePointer(ptr2a).toString('hex'), C1.V2A.memo_data_hex);
  assert.equal(ptr2a.schema_name, 'org.example.custom.packet.v1');

  const ptr2b = mp.decodePointer(Buffer.from(C1.V2B.memo_data_hex, 'hex'), { enforceCanonicalOrder: true });
  assert.equal(mp.encodePointer(ptr2b).toString('hex'), C1.V2B.memo_data_hex);
  assert.equal(ptr2b.inline_payload.toString('hex'), C1.V2B.request_hash_hex);

  // V3 legacy v4 uppercase hex
  const legacy = mp.decodeLegacyPointerV4Memo({
    memo_type: C1.V3.memo_type_hex,
    memo_format: C1.V3.memo_format_hex,
    memo_data: C1.V3.memo_data_hex_upper,
  });
  assert.equal(legacy.task_id, 'task-legacy-0042');
  assert.equal(mp.encodeLegacyPointerV4(legacy).toString('hex').toUpperCase(), C1.V3.memo_data_hex_upper);

  // V4 public Envelope
  const env4 = mp.decodeEnvelope(Buffer.from(C1.V4.envelope_hex, 'hex'), { enforceCanonicalOrder: true });
  assert.equal(mp.encodeEnvelope(env4).toString('hex'), C1.V4.envelope_hex);
  assert.equal(
    mp.sha256(mp.envelopeSigningPreimage(env4.version, env4.header_bytes, env4.encryption_header_bytes, env4.body_bytes)).toString('hex'),
    C1.V4.signing_preimage_sha256_hex
  );
  const cid4 = mp.cidv1RawFromBytes(Buffer.from(C1.V4.envelope_hex, 'hex'));
  assert.equal(cid4.cidBin.toString('hex'), C1.V4.cid_bin_hex);
  assert.equal(cid4.multibaseCid, C1.V4.cid_multibase);
  assert.equal(mp.decodeTextMessage(env4.body_bytes).text, 'Hello Post Fiat L1v2!');

  // V5 private X-Wing Envelope + HKDF + AAD + KeyBinding preimage
  const env5 = mp.decodeEnvelope(Buffer.from(C1.V5.envelope_hex, 'hex'), { enforceCanonicalOrder: true });
  assert.equal(mp.encodeEnvelope(env5).toString('hex'), C1.V5.envelope_hex);
  assert.equal(
    mp.sha256(mp.envelopeSigningPreimage(env5.version, env5.header_bytes, env5.encryption_header_bytes, env5.body_bytes)).toString('hex'),
    C1.V5.signing_preimage_sha256_hex
  );
  const cid5 = mp.cidv1RawFromBytes(Buffer.from(C1.V5.envelope_hex, 'hex'));
  assert.equal(cid5.cidBin.toString('hex'), C1.V5.cid_bin_hex);
  assert.equal(cid5.multibaseCid, C1.V5.cid_multibase);
  const aad5 = mp.computeAadContext('postfiat-devnet-2', env5.header_bytes, 0);
  assert.equal(aad5.toString('hex'), C1.V5.aad_context_hex);
  assert.equal(mp.deriveMessageKemSeed(Buffer.from('10'.repeat(32), 'hex'), 'postfiat-devnet-2').toString('hex'), C1.V5.derived_kem_seed_hex);
  const kid = mp.computeRecipientKeyId(Buffer.from('22'.repeat(32), 'hex'));
  assert.equal(kid.toString('hex'), C1.V5.recipient_key_id_hex);
  assert.equal(mp.deriveXWingWrapKek(Buffer.from('33'.repeat(32), 'hex'), aad5, kid).toString('hex'), C1.V5.derived_wrap_kek_hex);
  const kbPre = mp.computeKeyBindingPreimage({
    chain_id: 'postfiat-devnet-2',
    account: 'pf1recip0000000000000000000000000000000000',
    xwing_public_key: Buffer.from('22'.repeat(32), 'hex'),
    key_id: kid,
    created_at_unix_ms: 1764201600000,
    key_epoch: 1,
  });
  assert.equal(mp.sha256(kbPre).toString('hex'), C1.V5.key_binding_preimage_sha256_hex);

  // V6 Manifest + V7 x402
  const manifest = mp.decodeManifest(Buffer.from(C1.V6.manifest_hex, 'hex'), { enforceCanonicalOrder: true });
  assert.equal(mp.encodeManifest(manifest).toString('hex'), C1.V6.manifest_hex);

  const req = mp.decodeX402PaymentRequired(Buffer.from(C1.V7.required_hex, 'hex'), { enforceCanonicalOrder: true });
  assert.equal(mp.encodeX402PaymentRequired(req).toString('hex'), C1.V7.required_hex);
  const sig = mp.decodeX402PaymentSignature(Buffer.from(C1.V7.signature_hex, 'hex'), { enforceCanonicalOrder: true });
  assert.equal(mp.encodeX402PaymentSignature(sig).toString('hex'), C1.V7.signature_hex);
  const resp = mp.decodeX402PaymentResponse(Buffer.from(C1.V7.response_hex, 'hex'), { enforceCanonicalOrder: true });
  assert.equal(mp.encodeX402PaymentResponse(resp).toString('hex'), C1.V7.response_hex);
  const b64 = mp.encodeX402Header(Buffer.from(C1.V7.required_hex, 'hex'));
  assert.equal(mp.decodeX402Header(b64).toString('hex'), C1.V7.required_hex);
}

function testEveryCanonicalPayloadEncodesByteForByte() {
  let checked = 0;
  for (const [vector, field, type] of CANONICAL_PAYLOADS) {
    const hex = C1[vector][field];
    assert.equal(typeof hex, 'string', `${vector}.${field} missing from the canonical vectors`);
    if (type === null) continue;
    const raw = Buffer.from(hex, 'hex');
    const opts = type === 'LegacyPointerV4' ? undefined : { enforceCanonicalOrder: true };
    const reencoded = mp[`encode${type}`](mp[`decode${type}`](raw, opts));
    assert.equal(reencoded.toString('hex'), raw.toString('hex'), `${vector}.${field} ${type} does not re-encode byte for byte`);
    checked += 1;
  }
  assert.equal(checked, 17);
}

function testPaymentRefHashesAre48Bytes() {
  assert.equal(mp.CHAIN_HASH_BYTES, 48);
  const h48 = Buffer.alloc(48, 0x11);
  const h32 = Buffer.alloc(32, 0x11);
  const wireBytesField = (number, value) => Buffer.concat([Buffer.from([(number << 3) | 2, value.length]), value]);
  const ref = { lane: mp.PaymentLane.FASTPAY, order_hash: h48, certificate_digest: h48 };
  const decodedRef = mp.decodeFastPayPaymentRef(mp.encodeFastPayPaymentRef(ref));
  assert.deepEqual(decodedRef.order_hash, h48);
  assert.deepEqual(decodedRef.certificate_digest, h48);
  const decodedSig = mp.decodeX402PaymentSignature(mp.encodeX402PaymentSignature({ order_hash: h48, tx_hash: h48 }));
  assert.deepEqual(decodedSig.order_hash, h48);
  assert.deepEqual(decodedSig.tx_hash, h48);
  for (const [field, number] of [['order_hash', 3], ['certificate_digest', 4], ['tx_hash', 5]]) {
    const message = new RegExp(`FastPayPaymentRef\\.${field} must be 48 bytes`);
    assert.throws(() => mp.encodeFastPayPaymentRef({ lane: mp.PaymentLane.FASTPAY, [field]: h32 }), message);
    assert.throws(() => mp.decodeFastPayPaymentRef(wireBytesField(number, h32)), message);
  }
  for (const [field, number] of [['order_hash', 7], ['tx_hash', 8]]) {
    const message = new RegExp(`X402PaymentSignature\\.${field} must be 48 bytes`);
    assert.throws(() => mp.encodeX402PaymentSignature({ [field]: h32 }), message);
    assert.throws(() => mp.decodeX402PaymentSignature(wireBytesField(number, h32)), message);
  }
}

async function testRealPqRoundtrips() {
  const { mlDsa65 } = await mp.loadPqModules();
  const senderKeys = mlDsa65.keygen(crypto.randomBytes(32));
  assert.equal(senderKeys.publicKey.length, mp.ML_DSA_65_PUBKEY_BYTES);

  const recip1 = await mp.deriveMessageKemKeypair(crypto.randomBytes(32), 'postfiat-devnet-2');
  const recip2 = await mp.deriveMessageKemKeypair(crypto.randomBytes(32), 'postfiat-devnet-2');
  assert.equal(recip1.publicKey.length, mp.XWING_PUBKEY_BYTES);
  assert.equal(recip1.keyId.length, 32);

  // 1. MessageKeyBinding real ML-DSA-65 sign & verify
  const { bytes: kbBytes } = await mp.signMessageKeyBinding({
    chain_id: 'postfiat-devnet-2',
    account: mp.deriveAccountAddress(senderKeys.publicKey),
    xwing_public_key: recip1.publicKey,
    created_at_unix_ms: 1764201600000,
    key_epoch: 1,
    account_pubkey: senderKeys.publicKey,
    account_secret_key: senderKeys.secretKey,
  });
  const verifiedBinding = await mp.verifyMessageKeyBinding(kbBytes);
  assert.equal(verifiedBinding.signature.length, mp.ML_DSA_65_SIGNATURE_BYTES);
  assert.equal(verifiedBinding.xwing_public_key.length, mp.XWING_PUBKEY_BYTES);

  // 2. Real PRIVATE multi-recipient X-Wing + XChaCha20-Poly1305 + ML-DSA-65 Envelope
  const senderAccount = mp.deriveAccountAddress(senderKeys.publicKey);
  const rewardPlaintext = mp.encodeRewardNotice({
    reward_id: 'reward-live-001',
    task_id: 'task-live-001',
    recipient_account: 'pf1recip0000000000000000000000000000000000',
    amount_atoms: 1_000_000,
    asset_id: 'PFT',
    summary: 'Verified post-quantum FastPay reward',
  });
  const sealed = await mp.sealEnvelope({
    header: {
      protocol_version: 1,
      chain_id: 'postfiat-devnet-2',
      schema_name: 'pf.reward.v1',
      schema_id: mp.SCHEMA_ID.REWARD_V1,
      kind: mp.ContentKind.REWARD,
      visibility: mp.Visibility.PRIVATE,
      body_encoding: mp.BodyEncoding.PROTOBUF,
      cipher_suite: mp.CipherSuite.XWING_XCHACHA20_POLY1305,
      sender_account: senderAccount,
      created_at_unix_ms: 1764201700000,
      task_id: 'task-live-001',
    },
    bodyPlaintext: rewardPlaintext,
    senderPubkey: senderKeys.publicKey,
    senderSecretKey: senderKeys.secretKey,
    recipients: [
      { xwingPublicKey: recip2.publicKey, account: senderAccount },
      { xwingPublicKey: recip1.publicKey, account: 'pf1recip0000000000000000000000000000000000' },
    ],
  });
  assert.equal(sealed.envelope.sender_pubkey.length, mp.ML_DSA_65_PUBKEY_BYTES);
  assert.equal(sealed.envelope.signature.length, mp.ML_DSA_65_SIGNATURE_BYTES);
  const decodedEnc = mp.decodeXWingEncryptionHeader(sealed.envelope.encryption_header_bytes);
  assert.equal(decodedEnc.recipients.length, 2);
  assert.equal(decodedEnc.recipients[0].xwing_ciphertext.length, mp.XWING_CIPHERTEXT_BYTES);
  assert.equal(decodedEnc.recipients[0].wrapped_cek.length, mp.WRAPPED_CEK_BYTES);
  assert.ok(Buffer.compare(decodedEnc.recipients[0].recipient_key_id, decodedEnc.recipients[1].recipient_key_id) < 0);

  // Both recipients can decrypt identically with full anchor-binding checks enforced inside openEnvelope()
  for (const r of [recip1, recip2]) {
    const opened = await mp.openEnvelope(sealed.envelopeBytes, {
      expectedCidBin: sealed.cidBin,
      expectedMultibaseCid: sealed.multibaseCid,
      expectedChainId: 'postfiat-devnet-2',
      expectedTxSender: senderAccount,
      recipientKeyId: r.keyId,
      recipientXWingSecretKey: r.secretKey,
      enforceCanonicalOrder: true,
    });
    assert.deepEqual(opened.plaintextBody, rewardPlaintext);
    assert.equal(mp.decodeRewardNotice(opened.plaintextBody).reward_id, 'reward-live-001');
  }

  // Anchor-binding negative checks enforced inside openEnvelope():
  await assert.rejects(
    () =>
      mp.openEnvelope(sealed.envelopeBytes, {
        expectedChainId: 'postfiat-mainnet-1',
        expectedTxSender: senderAccount,
        recipientKeyId: recip1.keyId,
        recipientXWingSecretKey: recip1.secretKey,
      }),
    /chain_id mismatch/
  );
  await assert.rejects(
    () =>
      mp.openEnvelope(sealed.envelopeBytes, {
        expectedChainId: 'postfiat-devnet-2',
        expectedTxSender: 'pf0000000000000000000000000000000000000000',
        recipientKeyId: recip1.keyId,
        recipientXWingSecretKey: recip1.secretKey,
      }),
    /Foreign re-anchor rejected/
  );
  const outsiderKeys = mlDsa65.keygen(crypto.randomBytes(32));
  const spoofed = await mp.sealEnvelope({
    header: {
      protocol_version: 1,
      chain_id: 'postfiat-devnet-2',
      schema_name: 'pf.reward.v1',
      schema_id: mp.SCHEMA_ID.REWARD_V1,
      kind: mp.ContentKind.REWARD,
      visibility: mp.Visibility.PRIVATE,
      body_encoding: mp.BodyEncoding.PROTOBUF,
      cipher_suite: mp.CipherSuite.XWING_XCHACHA20_POLY1305,
      sender_account: senderAccount,
      created_at_unix_ms: 1764201700000,
    },
    bodyPlaintext: rewardPlaintext,
    senderPubkey: outsiderKeys.publicKey,
    senderSecretKey: outsiderKeys.secretKey,
    recipients: [{ xwingPublicKey: recip1.publicKey, account: 'pf1recip0000000000000000000000000000000000' }],
  });
  await assert.rejects(
    () =>
      mp.openEnvelope(spoofed.envelopeBytes, {
        recipientKeyId: recip1.keyId,
        recipientXWingSecretKey: recip1.secretKey,
      }),
    /sender_pubkey address/
  );

  // Tampering any byte breaks CID & signature verification
  const tampered = Buffer.from(sealed.envelopeBytes);
  tampered[tampered.length - 1] ^= 0x01;
  await assert.rejects(
    () =>
      mp.openEnvelope(tampered, {
        recipientKeyId: recip1.keyId,
        recipientXWingSecretKey: recip1.secretKey,
      }),
    mp.MessageProtocolError
  );

  // 3. SEALED visibility omits recipient_account
  const sealedAnon = await mp.sealEnvelope({
    header: {
      protocol_version: 1,
      chain_id: 'postfiat-devnet-2',
      schema_name: 'pf.message.text.v1',
      schema_id: mp.SCHEMA_ID.TEXT_V1,
      kind: mp.ContentKind.CHAT,
      visibility: mp.Visibility.SEALED,
      body_encoding: mp.BodyEncoding.RAW_UTF8,
      cipher_suite: mp.CipherSuite.XWING_XCHACHA20_POLY1305,
      sender_account: senderAccount,
      created_at_unix_ms: 1764201700000,
    },
    bodyPlaintext: Buffer.from('anonymous sealed payload', 'utf8'),
    senderPubkey: senderKeys.publicKey,
    senderSecretKey: senderKeys.secretKey,
    recipients: [{ xwingPublicKey: recip1.publicKey, account: 'must-be-stripped' }],
  });
  const openedAnon = await mp.openEnvelope(sealedAnon.envelopeBytes, {
    expectedCidBin: sealedAnon.cidBin,
    recipientKeyId: recip1.keyId,
    recipientXWingSecretKey: recip1.secretKey,
  });
  assert.equal(openedAnon.plaintextBody.toString('utf8'), 'anonymous sealed payload');
  assert.equal(openedAnon.encryptionHeader.recipients[0].recipient_account, '');

  // 4. Multi-chunk encrypted Manifest round-trip + chunk reorder rejection
  const cek = crypto.randomBytes(32);
  const headerBytes = mp.encodeMessageHeader({
    protocol_version: 1,
    chain_id: 'postfiat-devnet-2',
    schema_name: 'pf.document.bundle.v1',
    schema_id: mp.SCHEMA_ID.CUSTOM,
    kind: mp.ContentKind.DOCUMENT,
    visibility: mp.Visibility.PRIVATE,
    body_encoding: mp.BodyEncoding.PROTOBUF,
    cipher_suite: mp.CipherSuite.XWING_XCHACHA20_POLY1305,
    sender_account: 'pf1sender000000000000000000000000000000000',
    created_at_unix_ms: 1764201700000,
  });
  const payload = crypto.randomBytes(10_500);
  const { manifest, blocks } = await mp.splitManifestChunks(payload, {
    chainId: 'postfiat-devnet-2',
    headerBytes,
    cek,
    chunkSizeBytes: 4096,
  });
  assert.equal(blocks.length, 3);
  const reassembled = await mp.assembleManifestChunks(manifest, blocks, {
    chainId: 'postfiat-devnet-2',
    headerBytes,
    cek,
  });
  assert.deepEqual(reassembled, payload);

  await assert.rejects(
    () => mp.assembleManifestChunks(manifest, [blocks[1], blocks[0], blocks[2]], { chainId: 'postfiat-devnet-2', headerBytes, cek }),
    mp.MessageProtocolError
  );

  // 5. Negative pointer validation
  assert.throws(() => mp.validatePointerV5Flags(mp.PointerFlag.PUBLIC | mp.PointerFlag.SEALED), mp.MessageProtocolError);
  assert.throws(() => mp.validateL1PaymentMemo('70662E707472', '7635', '00'), mp.MessageProtocolError);
}

// A binding says "this account uses this encryption key". It counts only when the key that
// signed it is the key of the account it names.
async function testKeyBindingIsSignedByTheNamedAccount() {
  const { mlDsa65 } = await mp.loadPqModules();

  // The derivation is the chain's: checked against the published address vectors.
  const vectors = JSON.parse(fs.readFileSync(ADDRESS_VECTORS_PATH, 'utf8'));
  const derived = vectors.valid.filter((entry) => entry.ml_dsa_65_seed_hex);
  assert.ok(derived.length >= 4);
  for (const entry of derived) {
    const { publicKey } = mlDsa65.keygen(Buffer.from(entry.ml_dsa_65_seed_hex, 'hex'));
    assert.equal(mp.deriveAccountAddress(publicKey), entry.canonical, entry.description);
  }
  assert.throws(() => mp.deriveAccountAddress(Buffer.alloc(32)), mp.MessageProtocolError);

  const owner = mlDsa65.keygen(crypto.randomBytes(32));
  const outsider = mlDsa65.keygen(crypto.randomBytes(32));
  const outsiderKem = await mp.deriveMessageKemKeypair(crypto.randomBytes(32), 'postfiat-devnet-2');
  const ownerAccount = mp.deriveAccountAddress(owner.publicKey);
  const bindingSignedBy = (signer, account) =>
    mp.signMessageKeyBinding({
      chain_id: 'postfiat-devnet-2',
      account,
      xwing_public_key: outsiderKem.publicKey,
      created_at_unix_ms: 1764201600000,
      key_epoch: 7,
      account_pubkey: signer.publicKey,
      account_secret_key: signer.secretKey,
    });
  const refusedAsNotTheAccountKey = (err) =>
    err instanceof mp.MessageProtocolError && /not the key of the named account/.test(err.message);

  // Another account's binding, signed by a key that has nothing to do with that account.
  const forOwnerByOutsider = await bindingSignedBy(outsider, ownerAccount);
  await assert.rejects(() => mp.verifyMessageKeyBinding(forOwnerByOutsider.bytes), refusedAsNotTheAccountKey);
  await assert.rejects(() => mp.verifyMessageKeyBinding(forOwnerByOutsider.binding), refusedAsNotTheAccountKey);

  // The checksummed spelling of the signer's own account is not the form a binding carries.
  const checksummedAccount = `pf${ownerAccount.slice(2).toUpperCase()}`;
  const forChecksummedSpelling = await bindingSignedBy(owner, checksummedAccount);
  await assert.rejects(() => mp.verifyMessageKeyBinding(forChecksummedSpelling.bytes), refusedAsNotTheAccountKey);

  const forOwnerByOwner = await bindingSignedBy(owner, ownerAccount);
  const verified = await mp.verifyMessageKeyBinding(forOwnerByOwner.bytes);
  assert.equal(verified.account, ownerAccount);

  // The owner's binding with the signature of another message is still refused.
  const tampered = { ...forOwnerByOwner.binding, key_epoch: 8 };
  await assert.rejects(() => mp.verifyMessageKeyBinding(tampered), /signature invalid/);
}

async function testCekCommitmentBlindedSealedAndEpochRotation() {
  const { mlDsa65, xwing } = await mp.loadPqModules();
  const sender = mlDsa65.keygen(crypto.randomBytes(32));
  const senderAddr = mp.deriveAccountAddress(sender.publicKey);
  const bob = await mp.generateRotatableXWingKeypair();
  const eve = await mp.generateRotatableXWingKeypair();

  // 1) SEALED envelope emits 32B cek_commitment and blinded unlinkable recipient tags
  const header = {
    protocol_version: 1,
    chain_id: 'postfiat-devnet-2',
    sender_account: senderAddr,
    created_at_unix_ms: 1764201600000,
    kind: mp.ContentKind.CHAT,
    visibility: mp.Visibility.SEALED,
    cipher_suite: mp.CipherSuite.XWING_XCHACHA20_POLY1305,
    body_encoding: mp.BodyEncoding.PROTOBUF,
    schema_id: mp.SCHEMA_ID.TEXT_V1,
    schema_name: 'pf.msg.text.v1',
  };
  const sealed = await mp.sealEnvelope({
    header,
    bodyPlaintext: Buffer.from('js-blinded-sealed-hello'),
    senderPubkey: sender.publicKey,
    senderSecretKey: sender.secretKey,
    recipients: [{ xwingPublicKey: bob.publicKey, account: 'pf_ignored' }],
  });
  const encHdr = mp.decodeXWingEncryptionHeader(sealed.envelope.encryption_header_bytes);
  assert.equal(encHdr.cek_commitment.length, 32);
  for (const w of encHdr.recipients) {
    assert.notDeepEqual(w.recipient_key_id, bob.keyId);
    assert.equal(w.recipient_account, '');
  }

  // Bob opens blinded SEALED envelope with real X-Wing trial-decapsulation
  const opened = await mp.openEnvelope(sealed.envelopeBytes, {
    expectedCidBin: sealed.cidBin,
    expectedChainId: 'postfiat-devnet-2',
    expectedTxSender: senderAddr,
    recipientKeyId: bob.keyId,
    recipientXWingSecretKey: bob.secretKey,
    requireCekCommitment: true,
  });
  assert.equal(opened.plaintextBody.toString('utf8'), 'js-blinded-sealed-hello');

  // Eve cannot open blinded SEALED envelope
  await assert.rejects(
    () =>
      mp.openEnvelope(sealed.envelopeBytes, {
        expectedCidBin: sealed.cidBin,
        recipientKeyId: eve.keyId,
        recipientXWingSecretKey: eve.secretKey,
      }),
    /no matching RecipientWrap/
  );

  // 2) Key-commitment negative control: malicious sender wraps evil CEK while header commits to real CEK
  const realCek = crypto.randomBytes(32);
  const evilCek = crypto.randomBytes(32);
  const hdrBytes = mp.encodeMessageHeader(header);
  const aad = mp.computeAadContext('postfiat-devnet-2', hdrBytes, 0);
  const bodyNonce = crypto.randomBytes(24);
  const bodyCt = await mp.xchacha20Poly1305Encrypt(evilCek, bodyNonce, Buffer.from('evil-msg'), aad);
  const encap = xwing.encapsulate(bob.publicKey);
  const wrapNonce = crypto.randomBytes(24);
  const kek = mp.deriveXWingWrapKek(Buffer.from(encap.sharedSecret), aad, bob.keyId);
  const evilWrap = {
    recipient_key_id: bob.keyId,
    recipient_account: '',
    xwing_ciphertext: Buffer.from(encap.cipherText),
    wrap_nonce: wrapNonce,
    wrapped_cek: await mp.xchacha20Poly1305Encrypt(kek, wrapNonce, evilCek, aad),
  };
  const forgedEncHdrBytes = mp.encodeXWingEncryptionHeader({
    cipher_suite: mp.CipherSuite.XWING_XCHACHA20_POLY1305,
    aead_nonce: bodyNonce,
    aad_context: aad,
    chunk_index: 0,
    kdf_info: mp.DOMAIN_XWING_WRAP_INFO,
    recipients: [evilWrap],
    cek_commitment: mp.computeCekCommitment(realCek, aad),
  });
  const preimage = mp.envelopeSigningPreimage(1, hdrBytes, forgedEncHdrBytes, bodyCt);
  const forgedEnv = mp.encodeEnvelope({
    version: 1,
    header_bytes: hdrBytes,
    encryption_header_bytes: forgedEncHdrBytes,
    body_bytes: bodyCt,
    signature_algorithm: mp.SignatureAlgorithm.ML_DSA_65,
    sender_pubkey: Buffer.from(sender.publicKey),
    signature: Buffer.from(mlDsa65.sign(preimage, sender.secretKey)),
  });
  await assert.rejects(
    () =>
      mp.openEnvelope(forgedEnv, {
        recipientKeyId: bob.keyId,
        recipientXWingSecretKey: bob.secretKey,
        requireCekCommitment: true,
      }),
    /cek_commitment mismatch/
  );

  // 3) Non-seed epoch rotation & monotonic anti-downgrade verification
  const bootstrap = await mp.signMessageKeyBinding({
    chain_id: 'postfiat-devnet-2',
    account: senderAddr,
    xwing_public_key: bob.publicKey,
    created_at_unix_ms: 1764201600000,
    key_epoch: 0,
    account_pubkey: sender.publicKey,
    account_secret_key: sender.secretKey,
  });
  const rotated = await mp.rotateMessageKeyBinding({
    chain_id: 'postfiat-devnet-2',
    account: senderAddr,
    new_xwing_public_key: eve.publicKey,
    previous_key_epoch: 0,
    created_at_unix_ms: 1764201700000,
    account_pubkey: sender.publicKey,
    account_secret_key: sender.secretKey,
  });
  assert.equal(rotated.binding.key_epoch, 1);
  const v1 = await mp.verifyMessageKeyBinding(rotated.bytes, {
    expectedChainId: 'postfiat-devnet-2',
    expectedAccount: senderAddr,
    minKnownEpoch: 1,
  });
  assert.equal(v1.key_epoch, 1);
  await assert.rejects(
    () =>
      mp.verifyMessageKeyBinding(bootstrap.bytes, {
        expectedChainId: 'postfiat-devnet-2',
        expectedAccount: senderAddr,
        minKnownEpoch: 1,
      }),
    /key_epoch rollback rejected/
  );
}

async function main() {
  testCanonicalVectors();
  testEveryCanonicalPayloadEncodesByteForByte();
  testPaymentRefHashesAre48Bytes();
  await testRealPqRoundtrips();
  await testKeyBindingIsSignedByTheNamedAccount();
  await testCekCommitmentBlindedSealedAndEpochRotation();
  console.log('test_message_protocol: all C1 vectors, real PQ, cek_commitment, blinded SEALED & epoch-rotation tests passed');
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
