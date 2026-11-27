'use strict';

// Deterministic Contract C1 vector round-trips + full-size real PQ (ML-DSA-65 +
// IETF X-Wing ml_kem768_x25519 + XChaCha20-Poly1305) envelope & manifest tests.

const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const mp = require('./message-protocol');

const C1 = {
  V1: {
    memo_type_hex: '70662e707472',
    memo_format_hex: '7635',
    memo_data_hex:
      '0a24015512202acc209da9af312de8e9d20175c075a22b31b329a3e6d856dec9cf66436d523d100118042064320d7468726561642d63312d3030314002',
    cid_bin_hex: '015512202acc209da9af312de8e9d20175c075a22b31b329a3e6d856dec9cf66436d523d',
    cid_multibase: 'bafkreibkzqqj3knpgew6r2osaf24a5ncfmy3gknd43mfnxwjz5teg3kshu',
  },
  V2A: {
    memo_data_hex:
      '0a24015512202acc209da9af312de8e9d20175c075a22b31b329a3e6d856dec9cf66436d523d1002180c20012a287461736b2d323032362d31312d32372d3030303030303030303030303030303030303030303030313228676f762d73657373696f6e2d323032362d31312d32372d30303030303030303030303030303030313a286374782d736e617073686f742d323032362d31312d32372d303030303030303030303030303030314012521c6f72672e6578616d706c652e637573746f6d2e7061636b65742e7631',
  },
  V2B: {
    request_hash_hex: 'ee159abe7875173448aac81f2ac10748b46f3f8261f8d78d2b2d98b063a2bb56',
    memo_data_hex: '1003180e20920340064a20ee159abe7875173448aac81f2ac10748b46f3f8261f8d78d2b2d98b063a2bb56',
  },
  V3: {
    memo_type_hex: '70662E707472',
    memo_format_hex: '7634',
    memo_data_hex_upper:
      '0A3B6261666B726569676832616B69736361696C646371616273796733646672366368753366677072656769796D73636B376537617161347335327A791001180620012A107461736B2D6C65676163792D303034324001',
  },
  V4: {
    header_hex:
      '08011211706f7374666961742d6465766e65742d321a1270662e6d6573736167652e746578742e7631206428043001380140014a2a70663173656e6465723030303030303030303030303030303030303030303030303030303030303030305080a8f094ac33620d7468726561642d63312d303031',
    body_hex: '0a1548656c6c6f20506f73742046696174204c31763221120a746578742f706c61696e',
    signing_preimage_sha256_hex: 'e9d50d27fa79f160fda793f8133fc9ce19f5a507ed539e2a4bbea19d67c53ba7',
    envelope_hex:
      '0801127508011211706f7374666961742d6465766e65742d321a1270662e6d6573736167652e746578742e7631206428043001380140014a2a70663173656e6465723030303030303030303030303030303030303030303030303030303030303030305080a8f094ac33620d7468726561642d63312d30303122230a1548656c6c6f20506f73742046696174204c31763221120a746578742f706c61696e28013220a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a13a20b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2',
    cid_bin_hex: '015512202acc209da9af312de8e9d20175c075a22b31b329a3e6d856dec9cf66436d523d',
    cid_multibase: 'bafkreibkzqqj3knpgew6r2osaf24a5ncfmy3gknd43mfnxwjz5teg3kshu',
  },
  V5: {
    header_hex:
      '08011211706f7374666961742d6465766e65742d321a0c70662e7265776172642e763120c90128063002380140024a2a7066317461736b6e6f64653030303030303030303030303030303030303030303030303030303030303050e0fcf394ac335a107461736b2d7265776172642d303030317abf0108011211706f7374666961742d6465766e65742d321a201fa9e64486fcda633292370aa97c47fa1ffa1463b8bc3804503e4d9a03c940942220986cb4a5f6ae7ce3b980118f577d54a1a32347ac8d467394479f43d340e04ef832035046543a2a7066317461736b6e6f646530303030303030303030303030303030303030303030303030303030303030422a7066317265636970303030303030303030303030303030303030303030303030303030303030303030304880cab5ee01508208',
    aad_context_hex: '34df597e69504a383a97eba1f106a9286b71496de75c7a66af953d4cec167c70',
    derived_kem_seed_hex: '430e4662271619e3e43878bddafd267013540a0e72aec34a4eeb1225af12199a',
    recipient_key_id_hex: '9f72ea0cf49536e3c66c787f705186df9a4378083753ae9536d65b3ad7fcddc4',
    derived_wrap_kek_hex: '3f70600a1a973a8b38b1868852cf8db36c2517f58aa389c6b02369678154f9eb',
    key_binding_preimage_sha256_hex: '6c4a3977bfc8b2f67e028c5b648d375fe21fe2dda86832e83ae6e5d0545560e4',
    encryption_header_hex:
      '080212187777777777777777777777777777777777777777777777771a2034df597e69504a383a97eba1f106a9286b71496de75c7a66af953d4cec167c702a1c706f7374666961742d6c312d76322f7877696e672d777261702f763132bc010a209f72ea0cf49536e3c66c787f705186df9a4378083753ae9536d65b3ad7fcddc4122a7066317265636970303030303030303030303030303030303030303030303030303030303030303030301a20444444444444444444444444444444444444444444444444444444444444444422185555555555555555555555555555555555555555555555552a30666666666666666666666666666666666666666666666666666666666666666666666666666666666666666666666666',
    signing_preimage_sha256_hex: '32442d9bd94a1edb029e6293d6814ab5eaf58d2eaefc25c6d89fe5b3b09386b4',
    envelope_hex:
      '080112b50208011211706f7374666961742d6465766e65742d321a0c70662e7265776172642e763120c90128063002380140024a2a7066317461736b6e6f64653030303030303030303030303030303030303030303030303030303030303050e0fcf394ac335a107461736b2d7265776172642d303030317abf0108011211706f7374666961742d6465766e65742d321a201fa9e64486fcda633292370aa97c47fa1ffa1463b8bc3804503e4d9a03c940942220986cb4a5f6ae7ce3b980118f577d54a1a32347ac8d467394479f43d340e04ef832035046543a2a7066317461736b6e6f646530303030303030303030303030303030303030303030303030303030303030422a7066317265636970303030303030303030303030303030303030303030303030303030303030303030304880cab5ee015082081a9b02080212187777777777777777777777777777777777777777777777771a2034df597e69504a383a97eba1f106a9286b71496de75c7a66af953d4cec167c702a1c706f7374666961742d6c312d76322f7877696e672d777261702f763132bc010a209f72ea0cf49536e3c66c787f705186df9a4378083753ae9536d65b3ad7fcddc4122a7066317265636970303030303030303030303030303030303030303030303030303030303030303030301a20444444444444444444444444444444444444444444444444444444444444444422185555555555555555555555555555555555555555555555552a3066666666666666666666666666666666666666666666666666666666666666666666666666666666666666666666666622408888888888888888888888888888888888888888888888888888888888888888888888888888888888888888888888888888888888888888888888888888888828013220a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a13a20b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2',
    cid_bin_hex: '0155122073a90001a7ea8b4f5a317c76d97b06049fbd647300aafd4ab0d6ca1e72d100aa',
    cid_multibase: 'bafkreidtveaadj7krnhvuml4o3mxwbqet66wi4yavl6uvmgwziphfuiavi',
  },
  V6: {
    manifest_hex:
      '0801108080601880804022186170706c69636174696f6e2f6f637465742d73747265616d2a1570662e646f63756d656e742e62756e646c652e76313004380242830112670a24015512200147e682a1ce4cf491afa19c10b08a40f6559f657576398a7788c37c60fcf88a109080401a3b6261666b72656961626937746966696f6f6a74326a646c35627471696c62637361367a6b7a367a6c766f79347975353469796e36676237687972691a18909090909090909090909090909090909090909090909090428501080112670a24015512205b914ae0bd580f1a94824cfc2cff2cf61c2988f46c36e00aa87c5e46ecb584c2109080201a3b6261666b72656963337366666f62706b7962346e6a6a61736d37717770366c6877647175797235646d67337161766b64346c7a646f7a6e6d6579691a18919191919191919191919191919191919191919191919191',
  },
  V7: {
    required_hex:
      '080210011a11706f7374666961743a6465766e65742d3220012a0350465430a08d063a2a7066317265636970303030303030303030303030303030303030303030303030303030303030303030304220ee159abe7875173448aac81f2ac10748b46f3f8261f8d78d2b2d98b063a2bb564a10c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c150e0cf8295ac335a2568747470733a2f2f6170692e706f7374666961742e6f72672f76312f696e666572656e6365621968747470733a2f2f783430322e706f7374666961742e6f7267',
    signature_hex:
      '080210011a11706f7374666961743a6465766e65742d3220012a20ee159abe7875173448aac81f2ac10748b46f3f8261f8d78d2b2d98b063a2bb563210c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c13a2083367ad31fc90e2e2fcba5b711ffd80f264f92c5c68aca0241cb89477bc522e24a20d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2',
    response_hex:
      '080210011abc0108011211706f7374666961742d6465766e65742d321a2083367ad31fc90e2e2fcba5b711ffd80f264f92c5c68aca0241cb89477bc522e22220c09c70d2488ee1e63c12bad892f3be484f363e34181e8c58d1a6dde8eda315f432035046543a2a70663173656e646572303030303030303030303030303030303030303030303030303030303030303030422a70663172656369703030303030303030303030303030303030303030303030303030303030303030303048a08d06501a20faa9f094ac332a17706f7374666961742d666163696c697461746f722d7631',
  },
};

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
    account: 'pf1recip0000000000000000000000000000000000',
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
      sender_account: 'pf1tasknode0000000000000000000000000000000',
      created_at_unix_ms: 1764201700000,
      task_id: 'task-live-001',
    },
    bodyPlaintext: rewardPlaintext,
    senderPubkey: senderKeys.publicKey,
    senderSecretKey: senderKeys.secretKey,
    recipients: [
      { xwingPublicKey: recip2.publicKey, account: 'pf1tasknode0000000000000000000000000000000' },
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

  // Both recipients can decrypt identically
  for (const r of [recip1, recip2]) {
    const opened = await mp.openEnvelope(sealed.envelopeBytes, {
      expectedCidBin: sealed.cidBin,
      expectedMultibaseCid: sealed.multibaseCid,
      recipientKeyId: r.keyId,
      recipientXWingSecretKey: r.secretKey,
      enforceCanonicalOrder: true,
    });
    assert.deepEqual(opened.plaintextBody, rewardPlaintext);
    assert.equal(mp.decodeRewardNotice(opened.plaintextBody).reward_id, 'reward-live-001');
  }

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
      sender_account: 'pf1sender000000000000000000000000000000000',
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

async function main() {
  testCanonicalVectors();
  await testRealPqRoundtrips();
  console.log('test_message_protocol: all C1 vectors and real PQ (ML-DSA-65 + X-Wing + XChaCha20-Poly1305) tests passed');
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
