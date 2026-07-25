/**
 * Reviewed public release pins for the real-value Lightning/NAVcoin demo.
 *
 * These values are deliberately tracked with the wallet release. They must
 * not be replaced by runtime environment variables or learned from the
 * coordinator endpoint that supplies quotes.
 */
export const LIGHTNING_NAVCOIN_RELEASE_PINS = Object.freeze({
  quoteSignerPublicKeyHex:
    'f8a3f40774327ebf0d26246b830d89a4e171cafda3c8430e750b314e01dd4d0d',
  // Filled only after the founder performs interactive mainnet LND wallet
  // creation and the public identity is reviewed. A null pin makes every
  // executable status fail closed and prevents invoice presentation.
  lndIdentityPubkeyHex: null,
  pftlChainId: 'local-pftl-proven-nav-v2-20260724',
  pftlGenesisHash:
    '817e1d2426faf4d6ffe000cf43bb9642f33d3736d754d7c8fc255530829fed3de0720c1fa01c2aff5058f80e33fe94c0',
  pftlAssetId:
    'f912599013445352dc064b8b07be3815db5f494eff7e7097b2d6a72ff333bbfcaf51954e35fe28558525541f5fb945b5',
  pftlBuildGitRevision: 'ae3c53c9',
  pftlNavEpoch: 1,
  pftlNavReservePacketHash:
    '02eaa97346cb1a3adfcb3d10446b18f9b0244ef21e3877e5bd5429854cbb72d4061937901e50fc4b2ad2d2f427e22144',
});
