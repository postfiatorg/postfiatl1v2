//! Comprehensive Wave 3 unit and integration tests for `postfiat-wallet-wasm`:
//! - Golden vector parity with `tasknode-apple#40` (`vectors/pq-unified-bip39-kdf.json`)
//! - Pillar 4 NFT protobuf envelope signing/verification + on-chain L1 execution round-trips
//! - Pillar 6 `identity_set` dual-key (ML-DSA-65 + BIP-340 Schnorr) signing & guards
//! - Pillar 7/8 payload builders & delegated 0-PFT credential acceptance signing

use crate::identity_signing::{
    compute_handle_reservation_commitment, compute_identity_link_id,
    identity_controller_link_preimage, identity_rotate_account_preimage,
    identity_self_unlink_preimage, sign_identity_controller_link_proof,
    sign_identity_rotate_account_cosig, sign_identity_self_unlink_proof, sign_identity_set_mint_v1,
    sign_identity_set_mint_v2, sign_identity_set_rotate_v2, validate_identity_set_operation,
    validate_v1_signing_guard, verify_nostr_binding_proof, verify_signed_identity_set_bundle,
    HandleAttestationV1, IdentityKind, IdentityLinkKind, IdentityMintParams, IdentityRotateMode,
    IdentityRotateParams, IdentitySetActionV1, IdentitySetOperationPayload,
    COLLABORATION_AUTH_PREFIX, IDENTITY_CONTROLLER_LINK_DOMAIN, IDENTITY_ROTATE_ACCOUNT_DOMAIN,
    IDENTITY_SET_TRANSACTION_KIND, IDENTITY_UNLINK_SELF_DOMAIN, NOSTR_V1_DERIVATION_PREFIX,
};
use crate::nft_signing::{
    build_nft_burn_operation, build_nft_mint_operation, build_nft_transfer_operation,
    build_train_a_nft_burn_payload, compute_nft_migrated_serial, compute_onchain_nft_id,
    sign_nft_claim_envelope, sign_nft_metadata_envelope, sign_nft_transaction_fields,
    sign_nft_transaction_quote, verify_signed_public_nft_envelope, NftAttributeInput,
    NftCollectionRefInput, NftMetadataInput, NftMigrationClaimInput, NFT_CLAIM_SCHEMA_NAME,
    NFT_METADATA_SCHEMA_NAME, NFT_MIGRATED_SERIAL_BIT,
};
use crate::pillar78_builders::{
    build_asset_access_set_payload, build_bridge_hold_claim_payload,
    build_bridge_hold_return_payload, build_credential_accept_payload,
    build_credential_create_and_accept_payload, build_credential_delete_payload,
    build_issuer_register_payload, build_nav_fee_claim_payload,
    build_nav_fee_schedule_attach_payload, build_nav_fee_schedule_cancel_payload,
    build_nav_fee_schedule_set_payload, build_offer_prune_ineligible_payload,
    compute_asset_bridge_hold_key, compute_credential_id, compute_credential_issuer_id,
    sign_delegated_credential_accept, verify_delegated_credential_accept, AssetAccessSetPayload,
    AssetApprovalMode, AssetLapsedExitRule, AssetWrapperMode, CredentialPolicyRuleV1,
    CredentialPolicyV1, CredentialRequirementPairV1, DelegatedCredentialAcceptInput,
    NavFeeScheduleInputV1, MAX_OFFER_PRUNE_BATCH, NAV_FEE_MAX_ANNUAL_RATE_BPS,
    NAV_FEE_MAX_FIX_FEE_CAP_BPS, NAV_FEE_MAX_HWM_RATE_BPS, NAV_FEE_MAX_SPREAD_BPS,
    NAV_FEE_MIN_INCREASE_NOTICE_BLOCKS,
};
use crate::unified_kdf::{
    bip39_seed64_from_nfkd, decode_npub, derive_message_key_seed,
    derive_ml_dsa_master_seed_from_bip39, derive_nostr_v2_keypair, derive_nostr_v2_seed,
    derive_pfsh03_material, derive_pq_root_seed_from_bip39,
    derive_pq_root_seed_from_imported_master, derive_unified_public_preview,
    derive_unified_wallet_init, encode_npub, nostr_v1_keypair_from_secret_hex, UNIFIED_KDF_VERSION,
};
use postfiat_crypto_provider::{bytes_to_hex, hex_to_bytes, ml_dsa_65_verify_with_context};
use postfiat_execution::{execute_nft_transaction, genesis_hash};
use postfiat_node::{init, InitOptions};
use postfiat_rpc_sdk::{
    wallet_backup_from_master_seed, wallet_identity_from_backup, RpcResponse, WalletBackupFile,
    WalletIdentity, WalletSignNftTransactionFields,
};
use postfiat_storage::NodeStore;
use postfiat_types::{
    Account, NftBurnOperation, NftMintOperation, NftTransactionOperation, NftTransferOperation,
    NFT_COLLECTION_FLAG_BURN_LOCKED, NFT_COLLECTION_FLAG_TRANSFER_LOCKED, NFT_FLAG_TRANSFERABLE,
};
use serde_json::json;
use std::collections::BTreeMap;

fn test_wallet(
    chain_id: &str,
    seed_byte: u8,
    account_index: u32,
) -> (WalletBackupFile, WalletIdentity) {
    let seed_hex = bytes_to_hex(&[seed_byte; 32]);
    let backup = wallet_backup_from_master_seed(chain_id, seed_hex, account_index).unwrap();
    let identity = wallet_identity_from_backup(&backup).unwrap();
    (backup, identity)
}

// ===========================================================================
// 1. Unified v2 BIP-39 KDF — exact golden vector parity (`pq-unified-bip39-kdf.json`)
// ===========================================================================

struct GoldenKdfVector {
    label: &'static str,
    words: &'static str,
    extra_nfkd: &'static str,
    chain_id: &'static str,
    genesis_hash: &'static str,
    account_index: u32,
    address_index: u32,
    expected_seed64_hex: &'static str,
    expected_ml_dsa_master_hex: &'static str,
    expected_pq_seed_hex: &'static str,
    expected_spend_secret_hex: &'static str,
    expected_spend_dsa_seed_hex: &'static str,
    expected_spend_r_pk_hex: &'static str,
    expected_scan_kem_seed_hex: &'static str,
    expected_message_key_seed_hex: &'static str,
    expected_nostr_v2_seed_hex: &'static str,
}

#[test]
fn unified_bip39_kdf_matches_all_five_golden_vectors() {
    const GENESIS: &str =
        "ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad25521aff3ed334da07e150a7233a3e90a9";
    const CHAIN: &str = "postfiat-wan-devnet-2";

    let vectors = [
        GoldenKdfVector {
            label: "bip39-12w-empty-acct0-addr0",
            words: "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
            extra_nfkd: "",
            chain_id: CHAIN,
            genesis_hash: GENESIS,
            account_index: 0,
            address_index: 0,
            expected_seed64_hex: "5eb00bbddcf069084889a8ab9155568165f5c453ccb85e70811aaed6f6da5fc19a5ac40b389cd370d086206dec8aa6c43daea6690f20ad3d8d48b2d2ce9e38e4",
            expected_ml_dsa_master_hex: "07047abf396e1e7562b73d6e1795a2893f1b8140449073b158ab9cb9ab1be9df",
            expected_pq_seed_hex: "ec2619c5d2ad4b2d995ee1141d571bd99a01e99f3be1f547ac5ea023d2dc39d2ec8d3821745cddfd01a202cee8be7ed2",
            expected_spend_secret_hex: "db26ea0148c05f5f6ca039e406a51ad86686c81d06e543a5b24827c241b1831e05d3000d6bb92149a0e8d1c744a09e58",
            expected_spend_dsa_seed_hex: "8f9b341003c57e02e38c1f89ffb0d4cd7cb4b152c7d618eafcb56788719a40c2",
            expected_spend_r_pk_hex: "2aef77339faa9165d16b6685ce46eb55ca83ac700c4f76d07d7352d2de65c2a7ca8dc205481d7f75db10986a97b3190c",
            expected_scan_kem_seed_hex: "0c847451e1834c8d979f5b2c7506487becc19e7070b280b016c21f933f363231958db3dfb78f45b6b6c675a1110dd8640d100aa80830b34bda0105698f782a69",
            expected_message_key_seed_hex: "d76b74b372950c4703f4a63d3460261d4a68aa51282728dec06ce2ad88a54c63",
            expected_nostr_v2_seed_hex: "5fc5a108b07d15c0b1f7f17d45b920ba7895e9029e3012b7804f57ac0d48e859",
        },
        GoldenKdfVector {
            label: "bip39-12w-empty-acct0-addr1",
            words: "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
            extra_nfkd: "",
            chain_id: CHAIN,
            genesis_hash: GENESIS,
            account_index: 0,
            address_index: 1,
            expected_seed64_hex: "5eb00bbddcf069084889a8ab9155568165f5c453ccb85e70811aaed6f6da5fc19a5ac40b389cd370d086206dec8aa6c43daea6690f20ad3d8d48b2d2ce9e38e4",
            expected_ml_dsa_master_hex: "07047abf396e1e7562b73d6e1795a2893f1b8140449073b158ab9cb9ab1be9df",
            expected_pq_seed_hex: "ec2619c5d2ad4b2d995ee1141d571bd99a01e99f3be1f547ac5ea023d2dc39d2ec8d3821745cddfd01a202cee8be7ed2",
            expected_spend_secret_hex: "2cb14d45fff21f29a54b8ae1f8a30513ff355816b59f2fdefef5d6a15a410e82ea6e5392417ccdfdecd80e388282adb2",
            expected_spend_dsa_seed_hex: "02e6d4cb8b489e98f9ad77d33f9cf1d12678840181f1cbfb36aa3676b3be57cf",
            expected_spend_r_pk_hex: "8212cddeb05ff8ed6716ee8c60cb3f734488a63f4f2aa50d5fe3d35356ce3d8042ce37ebfdde35907779ceaf3ad2dc16",
            expected_scan_kem_seed_hex: "6359007cd2d314ddbee181d68ac0ffd70d03b9d8e1bb8f18d00f82e3d48b8dbfbfb694940c856a4c51fe5e80ac51009adbbf913ffc6c9b7d9f10fff02098eb2b",
            expected_message_key_seed_hex: "20b8412c9fa53f31203cef95b6b62606008c1d3ea259b4c2aac96593cc27786c",
            expected_nostr_v2_seed_hex: "e0797ba34d9c3974c060d821a5e4bbb62e010b538360e640027abf62491fbefc",
        },
        GoldenKdfVector {
            label: "bip39-12w-trezor-acct0-addr0",
            words: "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
            extra_nfkd: "TREZOR",
            chain_id: CHAIN,
            genesis_hash: GENESIS,
            account_index: 0,
            address_index: 0,
            expected_seed64_hex: "c55257c360c07c72029aebc1b53c05ed0362ada38ead3e3e9efa3708e53495531f09a6987599d18264c1e1c92f2cf141630c7a3c4ab7c81b2f001698e7463b04",
            expected_ml_dsa_master_hex: "a7b94b31c307e07a4293970464cc7bcad7d6b45a403ae9f0b63c6dff17927f45",
            expected_pq_seed_hex: "12a95a1bfd4f21bf7a034f140b28eeb53dc818b58abd5a414d0421fb15f946bf1fc9faff864faf098e842593d070d76e",
            expected_spend_secret_hex: "ee01f0a43987f124ef8e90fa72ddb77f1c60f93652304253e202170ca9a34a5efe54456563695252d463b2fc68270659",
            expected_spend_dsa_seed_hex: "10879515b5606b95b1c8f57bddd8edd2333a0fbcf119e0872615308eed8245ba",
            expected_spend_r_pk_hex: "653b75dd0aa908f257091f3edde3b809d21b88b39841f15010da65f259bfc8422badd53e23ccea1e52b92f9302cc6529",
            expected_scan_kem_seed_hex: "6c7714ae3e63cb400c73b439033f3ec05b62907912fe039f6d366cd1b6316da095b530aa6acaa4ef64ce7a295d1ff31d4447d2cd3b43565934df92e189821412",
            expected_message_key_seed_hex: "02b19ccf9ae3955c5d8b32635f6b4a2be2672d51559fee51e006c3b29bcbd7a5",
            expected_nostr_v2_seed_hex: "593c47ec5ab52a59f678848dd9a1d72e2006c3d5eb3194c7cbfe2465474cc7ba",
        },
        GoldenKdfVector {
            label: "bip39-12w-unicode-nfkd-acct0-addr0",
            words: "legal winner thank year wave sausage worth useful legal winner thank yellow",
            // NFKD decomposition of U+00E9 ("é")
            extra_nfkd: "e\u{0301}",
            chain_id: CHAIN,
            genesis_hash: GENESIS,
            account_index: 0,
            address_index: 0,
            expected_seed64_hex: "647bdd5206a42ee51e72356561a055fb44af2767874ab14a43cce16ec5e98c320cc0de71495ba0187be1882190977cbc48c3f131f6dc364015e539f0b4f8067f",
            expected_ml_dsa_master_hex: "7562b5d9b23f4ca3eac4af7625993e7c626d9a18a3cc78a7c975075ebdbddb28",
            expected_pq_seed_hex: "2802e421bec2dcf23cbc26a6dca542a9bbdce3125add7f1c1c20e86e09d35868608051b44bda3faad5b437ce65cceac6",
            expected_spend_secret_hex: "cf43b846715c14652ce401c1ca7f980de977090e59c3eb3c970900b9dc499ac23d37336465b2a45d4a16eb8b3b7654db",
            expected_spend_dsa_seed_hex: "e9a292eba87ca68de763e03f502cc887a32e54497849448fd43aa79360230537",
            expected_spend_r_pk_hex: "f9f5beacdc5e0e112d01cc92444dd9604aad6035c6f139f20dc7a4da7d10fbba368a1276304d18a3a424a36f7165e706",
            expected_scan_kem_seed_hex: "a828b8a9fc18825ac3c502213b5d528644392cd4dbf3e0d68bc27e75ad818b6ccabce9ba8e00ee828987206b943b9b919567aae7150aa8ecce44a28c292bc01e",
            expected_message_key_seed_hex: "dc3409ba9e0a5bb8e0406fcb855e267110570243690ef73f0ee4e214373312e5",
            expected_nostr_v2_seed_hex: "d203b06531f57b927c53b8afdbf257e14de06e3049bcd457e07a1127e8f6879c",
        },
        GoldenKdfVector {
            label: "bip39-24w-acct2-addr5",
            words: "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art",
            extra_nfkd: "",
            chain_id: CHAIN,
            genesis_hash: GENESIS,
            account_index: 2,
            address_index: 5,
            expected_seed64_hex: "408b285c123836004f4b8842c89324c1f01382450c0d439af345ba7fc49acf705489c6fc77dbd4e3dc1dd8cc6bc9f043db8ada1e243c4a0eafb290d399480840",
            expected_ml_dsa_master_hex: "fdd5230ab323a8ef00b91b0f3fb9b7eb03b7eae42824457aa7f9b2680c2445d7",
            expected_pq_seed_hex: "bea2722f02bef9e8acf2dd254397763892337641ea9c4a31cbc10d6677c2f40bae7840f2be1e7e624029b654f67b1a98",
            expected_spend_secret_hex: "1e3e5903cb128f9dd2f1b89817ab7f24bc6133d3ca3d99995bd3ad49aa99898a9c29b1f7fcff87b2bcad89dccd547de0",
            expected_spend_dsa_seed_hex: "9ad9f81dc6a7dfeaa1a4e9dad6e99d8d688d4759e6be4dcf80036f1aeaa58973",
            expected_spend_r_pk_hex: "baebb37c03e68ce52ae4b50e5b4dfdd7d1ad0c88fde537ed227975473d2ed3677a27947935fb8db98ca154648045b8ed",
            expected_scan_kem_seed_hex: "f6e2e620fde922e16b1ee878d5df676cafdef16b52887899b5960f916339147e78ef41e9b20a7c40d3cf0991ccf203b368043cd71849915aed367ed4e6dcc9c2",
            expected_message_key_seed_hex: "37e11a67d199bdf8014c6b754fb23b01fea0e4ec236e4b4c92241185b54ed28e",
            expected_nostr_v2_seed_hex: "aab3c30ad4c75b2002be8b2ae97e93976915a00112689ebaa411c29f070558d0",
        },
    ];

    for v in &vectors {
        let seed64 = bip39_seed64_from_nfkd(v.words, v.extra_nfkd).expect(v.label);
        assert_eq!(
            bytes_to_hex(seed64.as_slice()),
            v.expected_seed64_hex,
            "{}: bip39_seed64 mismatch",
            v.label
        );

        let ml_dsa_master = derive_ml_dsa_master_seed_from_bip39(seed64.as_slice()).expect(v.label);
        assert_eq!(
            bytes_to_hex(ml_dsa_master.as_slice()),
            v.expected_ml_dsa_master_hex,
            "{}: ml_dsa_master_seed mismatch",
            v.label
        );

        let pq_seed = derive_pq_root_seed_from_bip39(
            seed64.as_slice(),
            v.chain_id,
            v.genesis_hash,
            v.account_index,
        )
        .expect(v.label);
        assert_eq!(
            bytes_to_hex(pq_seed.as_slice()),
            v.expected_pq_seed_hex,
            "{}: pq_seed mismatch",
            v.label
        );

        let pfsh03 = derive_pfsh03_material(
            &pq_seed,
            v.chain_id,
            v.genesis_hash,
            v.account_index,
            v.address_index,
        )
        .expect(v.label);
        assert_eq!(
            bytes_to_hex(pfsh03.spend_secret.as_slice()),
            v.expected_spend_secret_hex,
            "{}: spend_secret mismatch",
            v.label
        );
        assert_eq!(
            bytes_to_hex(pfsh03.spend_dsa_seed.as_slice()),
            v.expected_spend_dsa_seed_hex,
            "{}: spend_dsa_seed mismatch",
            v.label
        );
        assert_eq!(
            bytes_to_hex(pfsh03.spend_r_pk.as_slice()),
            v.expected_spend_r_pk_hex,
            "{}: spend_r_pk mismatch",
            v.label
        );
        assert_eq!(
            bytes_to_hex(pfsh03.scan_kem_seed.as_slice()),
            v.expected_scan_kem_seed_hex,
            "{}: scan_kem_seed mismatch",
            v.label
        );

        let msg_key = derive_message_key_seed(
            seed64.as_slice(),
            v.chain_id,
            v.account_index,
            v.address_index,
        )
        .expect(v.label);
        assert_eq!(
            bytes_to_hex(msg_key.as_slice()),
            v.expected_message_key_seed_hex,
            "{}: message_key_seed mismatch",
            v.label
        );

        let nostr_v2 = derive_nostr_v2_seed(seed64.as_slice(), v.account_index, v.address_index)
            .expect(v.label);
        assert_eq!(
            bytes_to_hex(nostr_v2.as_slice()),
            v.expected_nostr_v2_seed_hex,
            "{}: nostr_v2_seed mismatch",
            v.label
        );
    }
}

#[test]
fn unified_kdf_preview_init_npub_and_redaction_behave_safely() {
    const GENESIS: &str =
        "ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad25521aff3ed334da07e150a7233a3e90a9";
    let seed64 = bip39_seed64_from_nfkd(
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
        "",
    )
    .unwrap();

    let preview =
        derive_unified_public_preview(seed64.as_slice(), "postfiat-wan-devnet-2", GENESIS, 0, 0)
            .unwrap();
    let init =
        derive_unified_wallet_init(seed64.as_slice(), "postfiat-wan-devnet-2", GENESIS, 0, 0)
            .unwrap();
    assert_eq!(preview.kdf_version, UNIFIED_KDF_VERSION);
    assert_eq!(preview.ml_dsa_address, init.ml_dsa_address);
    assert_eq!(preview.ml_dsa_public_key_hex, init.ml_dsa_public_key_hex);
    assert_eq!(preview.nostr_v2_pubkey_hex, init.nostr_v2_pubkey_hex);
    assert_eq!(preview.nostr_v2_npub, init.nostr_v2_npub);
    assert!(preview.nostr_v2_npub.starts_with("npub1"));
    assert_eq!(
        decode_npub(&preview.nostr_v2_npub).unwrap(),
        preview.nostr_v2_pubkey_hex
    );

    // Imported 32-byte master seed must derive a distinct PQ root seed from the BIP-39 path.
    let imported = derive_pq_root_seed_from_imported_master(
        &[0x42u8; 32],
        "postfiat-wan-devnet-2",
        GENESIS,
        0,
    )
    .unwrap();
    let bip39_pq =
        derive_pq_root_seed_from_bip39(seed64.as_slice(), "postfiat-wan-devnet-2", GENESIS, 0)
            .unwrap();
    assert_ne!(imported.as_slice(), bip39_pq.as_slice());

    // Debug output must redact all secret fields.
    let pfsh03 = derive_pfsh03_material(&bip39_pq, "postfiat-wan-devnet-2", GENESIS, 0, 0).unwrap();
    let pfsh_dbg = format!("{pfsh03:?}");
    assert!(pfsh_dbg.contains("<redacted>"));
    assert!(!pfsh_dbg.contains(&bytes_to_hex(pfsh03.spend_secret.as_slice())));

    let nostr_kp = derive_nostr_v2_keypair(seed64.as_slice(), 0, 0).unwrap();
    let nostr_dbg = format!("{nostr_kp:?}");
    assert!(nostr_dbg.contains("<redacted>"));
    assert!(!nostr_dbg.contains(&bytes_to_hex(nostr_kp.secret_key.as_slice())));

    // Bech32 validation failures: mixed case, corrupted checksum, wrong prefix.
    let mixed = format!("NPUB1{}", &preview.nostr_v2_npub[5..]);
    assert!(decode_npub(&mixed).is_err());
    let mut bad_cs = preview.nostr_v2_npub.clone().into_bytes();
    let last = bad_cs.len() - 1;
    bad_cs[last] = if bad_cs[last] == b'q' { b'p' } else { b'q' };
    assert!(decode_npub(std::str::from_utf8(&bad_cs).unwrap()).is_err());
    assert!(encode_npub("not-hex").is_err());
}

// ===========================================================================
// 2. Pillar 4 NFT envelope signing, verification & on-chain L1 execution
// ===========================================================================

#[test]
fn nft_metadata_and_claim_envelopes_round_trip_and_reject_tampering() {
    let (issuer_backup, issuer_id) = test_wallet("postfiat-wan-devnet-2", 0x31, 0);
    let (claimant_backup, claimant_id) = test_wallet("postfiat-wan-devnet-2", 0x32, 0);

    let mut extensions = BTreeMap::new();
    extensions.insert("z_key".to_string(), "last".to_string());
    extensions.insert("a_key".to_string(), "first".to_string());

    let metadata = NftMetadataInput {
        schema_version: 1,
        name: "Post Fiat Profile #1".to_string(),
        description: "Post-quantum profile NFT".to_string(),
        image: "ipfs://bafkreihdwdcefgh4dqkjv67uzcmw7ojee6xedzdetojuzjevtenxquvyku".to_string(),
        image_cid: None,
        media: Vec::new(),
        attributes: vec![NftAttributeInput {
            trait_type: "Tier".to_string(),
            value: "Genesis".to_string(),
            display_type: String::new(),
        }],
        collection: Some(NftCollectionRefInput {
            collection_id: "tasknode-profiles".to_string(),
            issuer_account: issuer_id.address.clone(),
            name: "Task Node Profiles".to_string(),
            description: "Official collection".to_string(),
            external_url: "https://tasknode.postfiat.org".to_string(),
        }),
        content_hashes: Vec::new(),
        prompt_digest_hex: "aa".repeat(48),
        license: "CC-BY-4.0".to_string(),
        external_url: "https://postfiat.org".to_string(),
        origin: None,
        extensions,
    };

    let signed_meta = sign_nft_metadata_envelope(&issuer_backup, &metadata, 1_764_200_000_000)
        .expect("sign metadata envelope");
    assert_eq!(signed_meta.schema_name, NFT_METADATA_SCHEMA_NAME);
    assert_eq!(signed_meta.sender_account, issuer_id.address);
    assert!(signed_meta.metadata_uri.starts_with("ipfs://b"));
    assert_eq!(signed_meta.metadata_hash.len(), 96);

    let env_bytes = hex_to_bytes(&signed_meta.envelope_hex).unwrap();
    let verified = verify_signed_public_nft_envelope(
        &env_bytes,
        NFT_METADATA_SCHEMA_NAME,
        &issuer_id.address,
        Some(&signed_meta.metadata_hash),
    )
    .expect("verify metadata envelope");
    assert!(verified.valid);
    assert_eq!(verified.cid_multibase, signed_meta.cid_multibase);
    assert_eq!(
        verified.metadata_hash_sha3_384_hex,
        signed_meta.metadata_hash
    );

    // Zero re-serialization & one-hash rule checks:
    // 1) Wrong expected sender fails.
    assert!(verify_signed_public_nft_envelope(
        &env_bytes,
        NFT_METADATA_SCHEMA_NAME,
        &claimant_id.address,
        Some(&signed_meta.metadata_hash),
    )
    .is_err());
    // 2) Wrong expected schema fails.
    assert!(verify_signed_public_nft_envelope(
        &env_bytes,
        NFT_CLAIM_SCHEMA_NAME,
        &issuer_id.address,
        Some(&signed_meta.metadata_hash),
    )
    .is_err());
    // 3) Flipping a single byte fails both hash check and ML-DSA signature check.
    let mut tampered = env_bytes.clone();
    let mid = tampered.len() / 2;
    tampered[mid] ^= 0x01;
    assert!(verify_signed_public_nft_envelope(
        &tampered,
        NFT_METADATA_SCHEMA_NAME,
        &issuer_id.address,
        Some(&signed_meta.metadata_hash),
    )
    .is_err());
    assert!(verify_signed_public_nft_envelope(
        &tampered,
        NFT_METADATA_SCHEMA_NAME,
        &issuer_id.address,
        None,
    )
    .is_err());

    // Claim envelope + bit-63 migrated serial parity (case-insensitive NFTokenID).
    let serial_upper =
        compute_nft_migrated_serial("xrpl-postfiat-legacy", "00080000ABCDEF0123456789").unwrap();
    let serial_lower =
        compute_nft_migrated_serial("xrpl-postfiat-legacy", "00080000abcdef0123456789").unwrap();
    assert_eq!(serial_upper, serial_lower);
    assert_ne!(serial_upper & NFT_MIGRATED_SERIAL_BIT, 0);

    let claim = NftMigrationClaimInput {
        schema_version: 1,
        target_chain_id: "postfiat-wan-devnet-2".to_string(),
        source_chain: "xrpl-postfiat-legacy".to_string(),
        nftoken_id: "00080000abcdef0123456789".to_string(),
        old_owner: "rLegacyOwner1111111111111111111111".to_string(),
        new_owner_account: claimant_id.address.clone(),
        target_collection_id: "tasknode-profiles".to_string(),
        migration_issuer_account: issuer_id.address.clone(),
        migrated_serial: 0, // auto-derived
        retirement_mode: 1,
        cutoff_ledger_index: 9_000_000,
        claimed_at_unix_ms: 1_764_200_000_000,
        allowlist_cid: None,
    };
    let signed_claim =
        sign_nft_claim_envelope(&claimant_backup, &claim, 1_764_200_000_000).expect("sign claim");
    assert_eq!(signed_claim.migrated_serial, serial_upper);
    assert_eq!(signed_claim.claim_hash_hex.len(), 64);

    let claim_bytes = hex_to_bytes(&signed_claim.envelope_hex).unwrap();
    let claim_verified = verify_signed_public_nft_envelope(
        &claim_bytes,
        NFT_CLAIM_SCHEMA_NAME,
        &claimant_id.address,
        None,
    )
    .expect("verify claim envelope");
    assert_eq!(
        claim_verified.claim_hash_sha256_hex,
        signed_claim.claim_hash_hex
    );
}

#[test]
fn nft_onchain_mint_transfer_burn_round_trip_executes_and_enforces_lock_guard() {
    let dir = tempfile::tempdir().unwrap();
    init(InitOptions {
        data_dir: dir.path().to_owned(),
        chain_id: "w3-nft-test".into(),
        node_id: "validator-0".into(),
        validator_count: 2,
    })
    .unwrap();
    let store = NodeStore::new(dir.path());
    let genesis = store.read_genesis().unwrap();
    let mut ledger = store.read_ledger().unwrap();
    let gen_hash = genesis_hash(&genesis);

    let (issuer_backup, issuer_id) = test_wallet(&genesis.chain_id, 0x41, 0);
    let (holder_backup, holder_id) = test_wallet(&genesis.chain_id, 0x42, 0);
    let (buyer_backup, buyer_id) = test_wallet(&genesis.chain_id, 0x43, 0);

    for id in [&issuer_id, &holder_id, &buyer_id] {
        ledger.accounts.push(Account::new(
            id.address.clone(),
            10_000_000,
            Some(id.public_key_hex.clone()),
        ));
    }

    // Pillar 4 unsolicited permanent-lock guard: reject unsolicited mint to another account
    // when both BURN_LOCKED and non-transferable.
    let spam_mint = NftMintOperation {
        issuer: issuer_id.address.clone(),
        owner: holder_id.address.clone(),
        collection_id: "spam".to_string(),
        serial: 1,
        metadata_uri: "ipfs://bafkreihdwdcefgh4dqkjv67uzcmw7ojee6xedzdetojuzjevtenxquvyku"
            .to_string(),
        metadata_hash: "11".repeat(48),
        flags: 0, // non-transferable
        collection_flags: NFT_COLLECTION_FLAG_BURN_LOCKED | NFT_COLLECTION_FLAG_TRANSFER_LOCKED,
        issuer_transfer_fee: 0,
    };
    assert!(build_nft_mint_operation(spam_mint.clone()).is_err());
    assert!(sign_nft_transaction_fields(
        &issuer_backup,
        WalletSignNftTransactionFields {
            chain_id: genesis.chain_id.clone(),
            genesis_hash: gen_hash.clone(),
            protocol_version: genesis.protocol_version,
            source: issuer_id.address.clone(),
            fee: 100_000,
            sequence: 1,
            operation: NftTransactionOperation::NftMint(spam_mint.clone()),
        },
    )
    .is_err());

    // Quote path must also reject unsolicited permanent-lock spam mints.
    let spam_quote = RpcResponse {
        version: "1".to_string(),
        id: "q1".to_string(),
        ok: true,
        result: Some(json!({
            "chain_id": genesis.chain_id,
            "genesis_hash": gen_hash,
            "protocol_version": genesis.protocol_version,
            "source": issuer_id.address,
            "minimum_fee": 100_000,
            "sequence": 1,
            "operation": NftTransactionOperation::NftMint(spam_mint),
        })),
        error: None,
        events: Vec::new(),
    };
    assert!(sign_nft_transaction_quote(&issuer_backup, &spam_quote).is_err());

    // Valid transferable mint via signed metadata envelope -> execute on L1 ledger.
    let metadata = NftMetadataInput {
        schema_version: 1,
        name: "Profile #42".to_string(),
        description: "Verified profile NFT".to_string(),
        image: "ipfs://bafkreihdwdcefgh4dqkjv67uzcmw7ojee6xedzdetojuzjevtenxquvyku".to_string(),
        image_cid: None,
        media: Vec::new(),
        attributes: Vec::new(),
        collection: None,
        content_hashes: Vec::new(),
        prompt_digest_hex: String::new(),
        license: String::new(),
        external_url: String::new(),
        origin: None,
        extensions: BTreeMap::new(),
    };
    let signed_env =
        sign_nft_metadata_envelope(&issuer_backup, &metadata, 1_764_200_000_000).unwrap();

    let mint_op = build_nft_mint_operation(NftMintOperation {
        issuer: issuer_id.address.clone(),
        owner: holder_id.address.clone(),
        collection_id: "tasknode-profiles".to_string(),
        serial: 42,
        metadata_uri: signed_env.metadata_uri.clone(),
        metadata_hash: signed_env.metadata_hash.clone(),
        flags: NFT_FLAG_TRANSFERABLE,
        collection_flags: 0,
        issuer_transfer_fee: 0,
    })
    .unwrap();
    let expected_nft_id = compute_onchain_nft_id(
        &genesis.chain_id,
        &issuer_id.address,
        "tasknode-profiles",
        42,
    )
    .unwrap();

    let signed_mint = sign_nft_transaction_fields(
        &issuer_backup,
        WalletSignNftTransactionFields {
            chain_id: genesis.chain_id.clone(),
            genesis_hash: gen_hash.clone(),
            protocol_version: genesis.protocol_version,
            source: issuer_id.address.clone(),
            fee: 100_000,
            sequence: 1,
            operation: NftTransactionOperation::NftMint(mint_op),
        },
    )
    .unwrap();
    let mint_receipt = execute_nft_transaction(&genesis, &mut ledger, &signed_mint);
    assert!(mint_receipt.accepted, "{mint_receipt:?}");
    assert_eq!(ledger.nfts.len(), 1);
    assert_eq!(ledger.nfts[0].nft_id, expected_nft_id);

    // Transfer from holder to buyer -> execute on L1 ledger.
    let transfer_op = build_nft_transfer_operation(NftTransferOperation {
        nft_id: expected_nft_id.clone(),
        from: holder_id.address.clone(),
        to: buyer_id.address.clone(),
        issuer: issuer_id.address.clone(),
        issuer_transfer_fee: 0,
    })
    .unwrap();
    let signed_transfer = sign_nft_transaction_fields(
        &holder_backup,
        WalletSignNftTransactionFields {
            chain_id: genesis.chain_id.clone(),
            genesis_hash: gen_hash.clone(),
            protocol_version: genesis.protocol_version,
            source: holder_id.address.clone(),
            fee: 100_000,
            sequence: 1,
            operation: NftTransactionOperation::NftTransfer(transfer_op),
        },
    )
    .unwrap();
    let transfer_receipt = execute_nft_transaction(&genesis, &mut ledger, &signed_transfer);
    assert!(transfer_receipt.accepted, "{transfer_receipt:?}");
    assert_eq!(ledger.nfts[0].owner, buyer_id.address);

    // Burn by buyer -> execute on L1 ledger.
    let burn_op = build_nft_burn_operation(NftBurnOperation {
        nft_id: expected_nft_id.clone(),
        owner: buyer_id.address.clone(),
        issuer: String::new(),
    })
    .unwrap();
    let signed_burn = sign_nft_transaction_fields(
        &buyer_backup,
        WalletSignNftTransactionFields {
            chain_id: genesis.chain_id.clone(),
            genesis_hash: gen_hash,
            protocol_version: genesis.protocol_version,
            source: buyer_id.address.clone(),
            fee: 100_000,
            sequence: 1,
            operation: NftTransactionOperation::NftBurn(burn_op),
        },
    )
    .unwrap();
    let burn_receipt = execute_nft_transaction(&genesis, &mut ledger, &signed_burn);
    assert!(burn_receipt.accepted, "{burn_receipt:?}");

    // Train A issuer-burn payload builder check.
    let train_a_burn = build_train_a_nft_burn_payload(
        &expected_nft_id,
        &buyer_id.address,
        Some(&issuer_id.address),
    )
    .unwrap();
    assert_eq!(train_a_burn.operation, "nft_burn");
    assert_eq!(train_a_burn.issuer, issuer_id.address);
}

// ===========================================================================
// 3. Pillar 6 `identity_set` dual-key signing, co-signatures & security guards
// ===========================================================================

#[test]
fn identity_set_dual_key_mint_rotate_cosigs_and_guards_round_trip() {
    // Pillar 6 §1 client-side v1 signing guard.
    assert!(validate_v1_signing_guard(
        "collaboration",
        &format!("{COLLABORATION_AUTH_PREFIX}\nnonce=abc")
    )
    .is_ok());
    assert!(validate_v1_signing_guard(
        "collaboration",
        &format!("{NOSTR_V1_DERIVATION_PREFIX}\naccount=r123")
    )
    .is_err());
    assert!(validate_v1_signing_guard(
        "collaboration",
        &format!("{COLLABORATION_AUTH_PREFIX} {NOSTR_V1_DERIVATION_PREFIX}")
    )
    .is_err());
    assert!(validate_v1_signing_guard(
        "nostr_v1_derivation",
        &format!("{NOSTR_V1_DERIVATION_PREFIX}\naccount=r123")
    )
    .is_ok());
    assert!(validate_v1_signing_guard("unknown_purpose", "hello").is_err());

    let (backup, identity) = test_wallet("postfiat-wan-devnet-2", 0x51, 0);
    let (new_backup, new_id) = test_wallet("postfiat-wan-devnet-2", 0x52, 0);
    let (ctrl_backup, ctrl_id) = test_wallet("postfiat-wan-devnet-2", 0x53, 0);
    let (registrar_backup, registrar_id) = test_wallet("postfiat-wan-devnet-2", 0x54, 0);
    let _ = registrar_backup;

    let seed64 = bip39_seed64_from_nfkd(
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
        "",
    )
    .unwrap();
    let seed64_hex = bytes_to_hex(seed64.as_slice());

    // Mint v2 dual-key bundle (ML-DSA-65 + BIP-340 Schnorr).
    let mint_bundle = sign_identity_set_mint_v2(
        &backup,
        &seed64_hex,
        IdentityMintParams {
            tx_sequence: 1,
            key_index: 0,
            identity_kind: IdentityKind::Human,
            handle_attestation: Some(HandleAttestationV1 {
                namespace: "tasknode.postfiat.org".to_string(),
                handle: "goodalexander".to_string(),
                account: identity.address.clone(),
                expires_at_height: 500_000,
                registrar_account: registrar_id.address.clone(),
                registrar_pubkey_hex: registrar_id.public_key_hex.clone(),
                registrar_signature_hex: "aa".repeat(64),
            }),
            reservation_salt_hex: Some("bb".repeat(16)),
            relays: vec!["wss://relay.postfiat.org".to_string()],
            avatar_nft_id: Some("cc".repeat(48)),
            message_key: Some("dd".repeat(1216)),
            pq_receive_address: Some("pfpq1testaddress".to_string()),
        },
    )
    .expect("sign mint v2");
    assert!(verify_signed_identity_set_bundle(&mint_bundle).unwrap());
    assert!(verify_nostr_binding_proof(
        &mint_bundle.nostr_pubkey_hex,
        &mint_bundle.nostr_proof_sig_hex,
        "mint",
        "postfiat-wan-devnet-2",
        &identity.address,
        1,
    )
    .unwrap());

    // Tampering with tx_sequence invalidates verification.
    let mut tampered = mint_bundle.clone();
    tampered.tx_sequence = 2;
    assert!(!verify_signed_identity_set_bundle(&tampered).unwrap());

    // Mint v1 preserving an existing secp256k1 Nostr key.
    let v1_scalar_hex = bytes_to_hex(&[0x17u8; 32]);
    let v1_kp = nostr_v1_keypair_from_secret_hex(&v1_scalar_hex, 0).unwrap();
    let mint_v1 = sign_identity_set_mint_v1(
        &backup,
        &v1_scalar_hex,
        IdentityMintParams {
            tx_sequence: 2,
            key_index: 0,
            identity_kind: IdentityKind::Human,
            handle_attestation: None,
            reservation_salt_hex: None,
            relays: vec!["wss://relay.postfiat.org".to_string()],
            avatar_nft_id: None,
            message_key: None,
            pq_receive_address: None,
        },
    )
    .expect("sign mint v1");
    assert_eq!(mint_v1.npub, v1_kp.npub);
    assert!(verify_signed_identity_set_bundle(&mint_v1).unwrap());

    // Rotate v2 (planned & compromised).
    let rotate_bundle = sign_identity_set_rotate_v2(
        &backup,
        &seed64_hex,
        IdentityRotateParams {
            tx_sequence: 3,
            new_key_index: 1,
            mode: IdentityRotateMode::Planned,
        },
    )
    .expect("sign rotate v2");
    assert_ne!(rotate_bundle.nostr_pubkey_hex, mint_bundle.nostr_pubkey_hex);
    assert!(verify_signed_identity_set_bundle(&rotate_bundle).unwrap());

    // Salted handle reservation commitment & link_id determinism.
    let commit1 = compute_handle_reservation_commitment(
        "goodalexander",
        &mint_bundle.nostr_pubkey_hex,
        &"12".repeat(16),
    )
    .unwrap();
    let commit2 = compute_handle_reservation_commitment(
        "goodalexander",
        &mint_bundle.nostr_pubkey_hex,
        &"34".repeat(16),
    )
    .unwrap();
    assert_eq!(commit1.len(), 96);
    assert_ne!(commit1, commit2);
    assert!(compute_handle_reservation_commitment(
        ".badhandle",
        &mint_bundle.nostr_pubkey_hex,
        &"12".repeat(16),
    )
    .is_err());

    let link_id = compute_identity_link_id(
        &identity.address,
        IdentityLinkKind::AgentController,
        &ctrl_id.address,
        1,
    );
    assert_eq!(link_id.len(), 96);

    // Co-signatures: rotate_account, agent_controller link, linked-key self-unlink.
    let rot_cosig = sign_identity_rotate_account_cosig(&new_backup, &identity.address, 4).unwrap();
    assert_eq!(rot_cosig.signer_account, new_id.address);
    assert!(ml_dsa_65_verify_with_context(
        &hex_to_bytes(&rot_cosig.public_key_hex).unwrap(),
        &identity_rotate_account_preimage(
            "postfiat-wan-devnet-2",
            &identity.address,
            &new_id.address,
            4,
        ),
        &hex_to_bytes(&rot_cosig.signature_hex).unwrap(),
        IDENTITY_ROTATE_ACCOUNT_DOMAIN.as_bytes(),
    ));

    let ctrl_proof =
        sign_identity_controller_link_proof(&ctrl_backup, &identity.address, 5).unwrap();
    assert!(ml_dsa_65_verify_with_context(
        &hex_to_bytes(&ctrl_proof.public_key_hex).unwrap(),
        &identity_controller_link_preimage(
            "postfiat-wan-devnet-2",
            &identity.address,
            &ctrl_id.address,
            5,
        ),
        &hex_to_bytes(&ctrl_proof.signature_hex).unwrap(),
        IDENTITY_CONTROLLER_LINK_DOMAIN.as_bytes(),
    ));

    let unlink_proof = sign_identity_self_unlink_proof(&ctrl_backup, &link_id).unwrap();
    assert!(ml_dsa_65_verify_with_context(
        &hex_to_bytes(&unlink_proof.public_key_hex).unwrap(),
        &identity_self_unlink_preimage("postfiat-wan-devnet-2", &link_id),
        &hex_to_bytes(&unlink_proof.signature_hex).unwrap(),
        IDENTITY_UNLINK_SELF_DOMAIN.as_bytes(),
    ));

    // Validate operation payload bounds (reject invalid relay scheme and self-rotation).
    let bad_op = IdentitySetOperationPayload {
        operation: IDENTITY_SET_TRANSACTION_KIND.to_string(),
        account: identity.address.clone(),
        payload: IdentitySetActionV1::Update {
            relays: Some(vec!["http://insecure.example".to_string()]),
            avatar_nft_id: None,
            message_key: None,
            pq_receive_address: None,
            handle_attestation: None,
            reservation_salt_hex: None,
        },
    };
    assert!(validate_identity_set_operation(&bad_op).is_err());

    let self_rot = IdentitySetOperationPayload {
        operation: IDENTITY_SET_TRANSACTION_KIND.to_string(),
        account: identity.address.clone(),
        payload: IdentitySetActionV1::RotateAccount {
            new_account: identity.address.clone(),
            new_account_pubkey_hex: identity.public_key_hex.clone(),
            new_account_sig_hex: "aa".repeat(64),
        },
    };
    assert!(validate_identity_set_operation(&self_rot).is_err());
}

// ===========================================================================
// 4. Pillar 7 & Pillar 8 transaction builders & delegated 0-PFT acceptance
// ===========================================================================

#[test]
fn pillar7_nav_fee_builders_enforce_protocol_caps_and_round_trip() {
    let (_, issuer) = test_wallet("postfiat-wan-devnet-2", 0x61, 0);
    let (_, recipient) = test_wallet("postfiat-wan-devnet-2", 0x62, 0);
    let asset_id = "ab".repeat(48);

    let valid_schedule = NavFeeScheduleInputV1 {
        annual_rate_bps: 100,
        hwm_rate_bps: 1_000,
        issue_spread_bps: 25,
        redeem_spread_bps: 25,
        fix_fee_cap_bps: 50,
        recipient: recipient.address.clone(),
        increase_notice_blocks: NAV_FEE_MIN_INCREASE_NOTICE_BLOCKS,
        accrual_cap_blocks: 86_400,
        rates_locked: false,
        disclosure_ref: Some("ipfs://bafkreidisclosure".to_string()),
    };

    let attach =
        build_nav_fee_schedule_attach_payload(&issuer.address, &asset_id, valid_schedule.clone())
            .unwrap();
    assert_eq!(attach.operation, "nav_fee_schedule_attach");

    let set =
        build_nav_fee_schedule_set_payload(&issuer.address, &asset_id, valid_schedule.clone())
            .unwrap();
    assert_eq!(set.operation, "nav_fee_schedule_set");

    let cancel = build_nav_fee_schedule_cancel_payload(&issuer.address, &asset_id).unwrap();
    assert_eq!(cancel.operation, "nav_fee_schedule_cancel");

    let claim = build_nav_fee_claim_payload(&recipient.address, &asset_id, 50_000).unwrap();
    assert_eq!(claim.operation, "nav_fee_claim");
    assert!(build_nav_fee_claim_payload(&recipient.address, &asset_id, 0).is_err());

    // Cap & invariant violations must fail closed.
    let mut bad = valid_schedule.clone();
    bad.recipient = issuer.address.clone();
    assert!(build_nav_fee_schedule_attach_payload(&issuer.address, &asset_id, bad).is_err());

    let mut bad = valid_schedule.clone();
    bad.annual_rate_bps = NAV_FEE_MAX_ANNUAL_RATE_BPS + 1;
    assert!(build_nav_fee_schedule_set_payload(&issuer.address, &asset_id, bad).is_err());

    let mut bad = valid_schedule.clone();
    bad.hwm_rate_bps = NAV_FEE_MAX_HWM_RATE_BPS + 1;
    assert!(build_nav_fee_schedule_set_payload(&issuer.address, &asset_id, bad).is_err());

    let mut bad = valid_schedule.clone();
    bad.issue_spread_bps = NAV_FEE_MAX_SPREAD_BPS + 1;
    assert!(build_nav_fee_schedule_set_payload(&issuer.address, &asset_id, bad).is_err());

    let mut bad = valid_schedule.clone();
    bad.fix_fee_cap_bps = NAV_FEE_MAX_FIX_FEE_CAP_BPS + 1;
    assert!(build_nav_fee_schedule_set_payload(&issuer.address, &asset_id, bad).is_err());

    let mut bad = valid_schedule;
    bad.increase_notice_blocks = NAV_FEE_MIN_INCREASE_NOTICE_BLOCKS - 1;
    assert!(build_nav_fee_schedule_set_payload(&issuer.address, &asset_id, bad).is_err());
}

#[test]
fn pillar8_builders_and_delegated_credential_accept_round_trip() {
    let (_, admin) = test_wallet("postfiat-wan-devnet-2", 0x71, 0);
    let (_, signing_acc) = test_wallet("postfiat-wan-devnet-2", 0x72, 0);
    let (subject_backup, subject) = test_wallet("postfiat-wan-devnet-2", 0x73, 0);
    let asset_id = "cd".repeat(48);
    let genesis_hash = "ef".repeat(48);

    // Bridge holds & DEX anti-poisoning offer pruning.
    let hold_key = compute_asset_bridge_hold_key(&asset_id, &subject.address).unwrap();
    assert_eq!(hold_key.len(), 96);
    assert_eq!(
        build_bridge_hold_claim_payload(&subject.address, &asset_id, 1_000)
            .unwrap()
            .operation,
        "bridge_hold_claim"
    );
    assert_eq!(
        build_bridge_hold_return_payload(&subject.address, &asset_id, 1_000, true)
            .unwrap()
            .operation,
        "bridge_hold_return"
    );

    let prune = build_offer_prune_ineligible_payload(
        &admin.address,
        vec!["01".repeat(48), "02".repeat(48)],
    )
    .unwrap();
    assert_eq!(prune.offer_ids.len(), 2);
    // Reject duplicate offer IDs and oversized batches.
    assert!(build_offer_prune_ineligible_payload(
        &admin.address,
        vec!["01".repeat(48), "01".repeat(48)],
    )
    .is_err());
    let too_many: Vec<String> = (0..=MAX_OFFER_PRUNE_BATCH)
        .map(|i| format!("{i:096x}"))
        .collect();
    assert!(build_offer_prune_ineligible_payload(&admin.address, too_many).is_err());

    // Issuer registration + deterministic IDs + asset access policy.
    let issuer_reg = build_issuer_register_payload(
        &admin.address,
        "kyc.example.org",
        vec![signing_acc.address.clone()],
        vec!["kyc.example.org/kyc.basic".to_string()],
        vec!["did:web:kyc.example.org".to_string()],
        1,
    )
    .unwrap();
    assert_eq!(issuer_reg.operation, "issuer_register");
    // Un-namespaced credential type must fail.
    assert!(build_issuer_register_payload(
        &admin.address,
        "kyc.example.org",
        vec![signing_acc.address.clone()],
        vec!["other.org/kyc.basic".to_string()],
        Vec::new(),
        1,
    )
    .is_err());

    let issuer_id =
        compute_credential_issuer_id("postfiat-wan-devnet-2", &admin.address, 1).unwrap();
    let cred_id =
        compute_credential_id(&subject.address, &issuer_id, "kyc.example.org/kyc.basic").unwrap();
    assert_eq!(issuer_id.len(), 96);
    assert_eq!(cred_id.len(), 96);

    let access = build_asset_access_set_payload(AssetAccessSetPayload {
        operation: String::new(),
        issuer: admin.address.clone(),
        asset_id: asset_id.clone(),
        disable_authorization_permanently: false,
        approval_mode: Some(AssetApprovalMode::Either),
        lapsed_exit_rule: Some(AssetLapsedExitRule::ReturnOnly),
        wrapper_mode: Some(AssetWrapperMode::PftlOnly),
        credential_policy: Some(CredentialPolicyV1 {
            any_of: vec![CredentialPolicyRuleV1 {
                all_of: vec![CredentialRequirementPairV1 {
                    issuer_id: issuer_id.clone(),
                    credential_type: "kyc.example.org/kyc.basic".to_string(),
                }],
            }],
        }),
        notice_blocks: Some(NAV_FEE_MIN_INCREASE_NOTICE_BLOCKS),
        emergency_remove_issuer_id: None,
    })
    .unwrap();
    assert_eq!(access.operation, "asset_access_set");

    assert_eq!(
        build_credential_accept_payload(&subject.address, &cred_id)
            .unwrap()
            .operation,
        "credential_accept"
    );
    assert_eq!(
        build_credential_delete_payload(&subject.address, &cred_id)
            .unwrap()
            .operation,
        "credential_delete"
    );

    // Delegated 0-PFT credential_create_and_accept signing & verification round-trip.
    let accept_input = DelegatedCredentialAcceptInput {
        chain_id: "postfiat-wan-devnet-2".to_string(),
        genesis_hash,
        subject: subject.address.clone(),
        issuer_id,
        credential_type: "kyc.example.org/kyc.basic".to_string(),
        commitment_hash: "99".repeat(48),
        expires_at_height: 1_000_000,
        subject_accept_nonce: 7,
    };
    let signed_accept = sign_delegated_credential_accept(&subject_backup, &accept_input).unwrap();
    assert!(verify_delegated_credential_accept(&accept_input, &signed_accept).unwrap());

    let create_and_accept = build_credential_create_and_accept_payload(
        &signing_acc.address,
        &accept_input,
        &signed_accept,
    )
    .unwrap();
    assert_eq!(create_and_accept.operation, "credential_create_and_accept");
    assert_eq!(create_and_accept.subject_accept_nonce, 7);

    // Tampering with nonce invalidates verification and blocks payload construction.
    let mut tampered_input = accept_input;
    tampered_input.subject_accept_nonce = 8;
    assert!(!verify_delegated_credential_accept(&tampered_input, &signed_accept).unwrap());
    assert!(build_credential_create_and_accept_payload(
        &signing_acc.address,
        &tampered_input,
        &signed_accept,
    )
    .is_err());
}
