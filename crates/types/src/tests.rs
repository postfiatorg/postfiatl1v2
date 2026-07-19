use super::*;

include!("atomic_swap_type_tests.rs");
include!("atomic_swap_batch_tests.rs");
include!("pfusdc_tier4_type_tests.rs");

#[test]
fn genesis_round_trip() {
    let genesis = Genesis::new("postfiat-local");
    let json = genesis.to_json().expect("serialize genesis");
    let parsed = Genesis::from_json(&json).expect("parse genesis");
    assert_eq!(genesis, parsed);
}

#[test]
fn genesis_round_trip_preserves_validator_count() {
    let genesis =
        Genesis::try_new_with_validator_count("postfiat-local", 4).expect("valid genesis");
    let json = genesis.to_json().expect("serialize genesis");
    let parsed = Genesis::from_json(&json).expect("parse genesis");
    assert_eq!(genesis, parsed);
    assert_eq!(4, parsed.validator_count);
    assert!(Genesis::try_new_with_validator_count("postfiat-local", 0).is_err());
}

#[test]
fn genesis_bridge_verification_activation_height_is_backward_compatible() {
    let genesis = Genesis::new("postfiat-local");
    let json = genesis.to_json().expect("serialize genesis");
    assert!(json.contains("\"replicated_state_v2_activation_height\": 0"));
    assert!(json.contains("\"native_supply_atoms\": 1000000000"));
    assert!(!json.contains("bridge_verification_activation_height"));

    let legacy_json =
            "{\n  \"chain_id\": \"postfiat-local\",\n  \"protocol_version\": 1,\n  \"validator_count\": 1\n}\n";
    let parsed = Genesis::from_json(legacy_json).expect("parse legacy genesis");
    assert_eq!(parsed.replicated_state_v2_activation_height, None);
    assert_eq!(parsed.native_supply_atoms, None);
    assert_eq!(
        parsed.expected_native_supply_atoms(),
        GENESIS_NATIVE_SUPPLY_ATOMS
    );
    assert_eq!(parsed.bridge_verification_activation_height, None);
    assert_eq!(parsed.atomic_swap_activation_height, None);
    assert_eq!(parsed.consensus_v2_activation_height, None);

    let mut activated = Genesis::new("postfiat-local");
    activated.bridge_verification_activation_height = Some(300);
    let json = activated.to_json().expect("serialize activated genesis");
    assert!(json.contains("\"bridge_verification_activation_height\": 300"));
    let parsed = Genesis::from_json(&json).expect("parse activated genesis");
    assert_eq!(parsed.bridge_verification_activation_height, Some(300));

    let mut atomic = Genesis::new("postfiat-local");
    atomic.atomic_swap_activation_height = Some(512);
    let json = atomic
        .to_json()
        .expect("serialize atomic activation genesis");
    assert!(json.contains("\"atomic_swap_activation_height\": 512"));
    let parsed = Genesis::from_json(&json).expect("parse atomic activation genesis");
    assert_eq!(parsed.atomic_swap_activation_height, Some(512));

    let mut consensus_v2 = Genesis::new("postfiat-local");
    consensus_v2.consensus_v2_activation_height = Some(900);
    let json = consensus_v2
        .to_json()
        .expect("serialize consensus v2 activation genesis");
    assert!(json.contains("\"consensus_v2_activation_height\": 900"));
    let parsed = Genesis::from_json(&json).expect("parse consensus v2 activation genesis");
    assert_eq!(parsed.consensus_v2_activation_height, Some(900));
}

#[test]
fn genesis_validation_rejects_malformed_domain_fields() {
    assert!(Genesis::try_new(" ").is_err());
    assert!(Genesis::try_new(" postfiat-local").is_err());
    assert!(Genesis::try_new("postfiat-local\n").is_err());

    let mut zero_protocol = Genesis::new("postfiat-local");
    zero_protocol.protocol_version = 0;
    assert!(zero_protocol.validate().is_err());

    let mut zero_validators = Genesis::new("postfiat-local");
    zero_validators.validator_count = 0;
    assert!(zero_validators.validate().is_err());

    let mut rewritten_native_supply = Genesis::new("postfiat-local");
    rewritten_native_supply.native_supply_atoms = Some(GENESIS_NATIVE_SUPPLY_ATOMS - 1);
    assert!(rewritten_native_supply.validate().is_err());

    let mut zero_consensus_v2_activation = Genesis::new("postfiat-local");
    zero_consensus_v2_activation.consensus_v2_activation_height = Some(0);
    assert!(zero_consensus_v2_activation.validate().is_err());

    let invalid_json =
        "{\n  \"chain_id\": \" \",\n  \"protocol_version\": 1,\n  \"validator_count\": 1\n}\n";
    assert!(Genesis::from_json(invalid_json).is_err());
}

#[test]
fn genesis_from_json_rejects_trailing_numeric_garbage() {
    let invalid_json = "{\n  \"chain_id\": \"postfiat-local\",\n  \"protocol_version\": 1xxx,\n  \"validator_count\": 1\n}\n";
    assert!(Genesis::from_json(invalid_json).is_err());
}

#[test]
fn node_state_from_json_decodes_escaped_strings_strictly() {
    let json = "{\n  \"node_id\": \"validator-\\\"0\\\"\",\n  \"status\": \"running\\\\ready\",\n  \"last_run_unix\": 7\n}\n";
    let parsed = NodeState::from_json(json).expect("parse escaped node state");

    assert_eq!("validator-\"0\"", parsed.node_id);
    assert_eq!("running\\ready", parsed.status);

    let invalid_json =
            "{\n  \"node_id\": \"validator-0\",\n  \"status\": \"running\",\n  \"last_run_unix\": 7xxx\n}\n";
    assert!(NodeState::from_json(invalid_json).is_err());
}

#[test]
fn status_report_to_json_uses_strict_json_escaping() {
    let report = StatusReport {
            chain_id: "postfiat-local".to_string(),
            genesis_hash: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
            protocol_version: 1,
            rpc_schema: "postfiat-local-rpc-v1".to_string(),
            build_git_revision: "test-revision".to_string(),
            build_profile: "test".to_string(),
            active_nav_profiles: Vec::new(),
            deployment_manifest_sha256: None,
            deployment_validator_id: None,
            deployment_service_artifacts: Vec::new(),
            deployment_runtime_artifacts: None,
            validator_count: 1,
            node_id: "validator-\u{0008}-0".to_string(),
            status: "running\u{000c}ready".to_string(),
            last_run_unix: 7,
            state_root: "root".to_string(),
            block_height: 1,
            block_tip_hash: "tip".to_string(),
            mempool_pending: 0,
        };
    let json = report.to_json().expect("serialize status report");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid status JSON");

    assert_eq!(parsed["node_id"], "validator-\u{0008}-0");
    assert_eq!(parsed["status"], "running\u{000c}ready");
}

#[test]
fn issued_asset_id_is_deterministic_and_domain_separated() {
    let issuer = "pfissuer000000000000000000000000000000000";
    let asset_id = issued_asset_id("postfiat-local", issuer, "USD", 1).expect("asset id");
    assert_eq!(
            "7eb4ab19c010edff936edcff5e4e8c04300d15e1751c102ab91fcec8ac3e0c49738c877dcde38c473050c2d68ecff45f",
            asset_id
        );
    assert_eq!(ISSUED_ASSET_ID_HEX_LEN, asset_id.len());
    assert_ne!(
        asset_id,
        issued_asset_id("postfiat-local", issuer, "USD", 2).expect("versioned id")
    );
    assert_ne!(
        asset_id,
        issued_asset_id("postfiat-other", issuer, "USD", 1).expect("chain id")
    );

    assert!(issued_asset_id("postfiat-local", issuer, " ", 1).is_err());
    assert!(issued_asset_id("postfiat-local", issuer, "A".repeat(33).as_str(), 1).is_err());
    assert!(issued_asset_id("postfiat-local", issuer, "USD", 0).is_err());
}

#[test]
fn asset_definition_and_trustline_validate_deterministic_ids() {
    let chain_id = "postfiat-local";
    let issuer = "pfissuer000000000000000000000000000000000";
    let holder = "pfholder00000000000000000000000000000000";
    let mut asset = AssetDefinition::new(chain_id, issuer, "USD", 1, 6).expect("asset");
    asset.display_name = "US Dollar".to_string();
    asset.max_supply = Some(1_000_000);
    asset.requires_authorization = true;
    asset.validate_for_chain(chain_id).expect("valid asset");
    assert_eq!(
            "7eb4ab19c010edff936edcff5e4e8c04300d15e1751c102ab91fcec8ac3e0c49738c877dcde38c473050c2d68ecff45f",
            asset.asset_id
        );

    let line =
        TrustLine::new(holder, issuer, asset.asset_id.clone(), 5_000, 10).expect("trustline");
    assert_eq!(
            "64311431d94bbfe26b4652a620a0fac3f41ed5b288e5e2d1899a9aa5dd88babbb0cbe7f3e9f688e74f55818da3d51bd4",
            line.trustline_id
        );
    assert_eq!(TRUSTLINE_ID_HEX_LEN, line.trustline_id.len());

    let ledger = LedgerState::new_with_assets(Vec::new(), vec![asset], vec![line]);
    ledger
        .validate_asset_state(chain_id)
        .expect("valid asset ledger state");
}

#[test]
fn ledger_asset_state_rejects_duplicates_and_missing_assets() {
    let chain_id = "postfiat-local";
    let issuer = "pfissuer000000000000000000000000000000000";
    let holder = "pfholder00000000000000000000000000000000";
    let asset = AssetDefinition::new(chain_id, issuer, "USD", 1, 6).expect("asset");
    let line =
        TrustLine::new(holder, issuer, asset.asset_id.clone(), 5_000, 10).expect("trustline");

    let duplicate_assets =
        LedgerState::new_with_assets(Vec::new(), vec![asset.clone(), asset.clone()], Vec::new());
    assert!(duplicate_assets.validate_asset_state(chain_id).is_err());

    let duplicate_lines =
        LedgerState::new_with_assets(Vec::new(), vec![asset.clone()], vec![line.clone(), line]);
    assert!(duplicate_lines.validate_asset_state(chain_id).is_err());

    let missing_asset_line = TrustLine::new(
        holder,
        issuer,
        "a".repeat(ISSUED_ASSET_ID_HEX_LEN),
        5_000,
        10,
    )
    .expect("syntactically valid trustline");
    let missing_asset =
        LedgerState::new_with_assets(Vec::new(), vec![asset], vec![missing_asset_line]);
    assert!(missing_asset.validate_asset_state(chain_id).is_err());
}

#[test]
fn nft_id_and_state_validate_metadata_bounds_and_indexes() {
    let chain_id = "postfiat-local";
    let issuer = "pfissuer000000000000000000000000000000000";
    let owner = "pfowner0000000000000000000000000000000000";
    let collection_id = "ART-2026";
    let metadata_hash = "ab".repeat(32);
    let mut nft = NftDefinition::new(
        chain_id,
        issuer,
        collection_id,
        42,
        owner,
        metadata_hash.clone(),
    )
    .expect("nft");
    nft.metadata_uri = "ipfs://bafybeigdyrzt".to_string();
    nft.flags = NFT_FLAG_TRANSFERABLE | NFT_FLAG_ISSUER_BURNABLE;
    nft.validate_for_chain(chain_id).expect("valid nft");
    assert_eq!(
            "1eab659bd92b86f52685ef5159fcceb9e54ed3e1ccde671c95c1dc21bc55360d61db50a3b666c001f710a018d0c0df31",
            nft.nft_id
        );
    assert_eq!(NFT_ID_HEX_LEN, nft.nft_id.len());
    assert_ne!(
        nft.nft_id,
        nft_id(chain_id, issuer, collection_id, 43).expect("different serial")
    );
    assert_ne!(
        nft.nft_id,
        nft_id("postfiat-other", issuer, collection_id, 42).expect("different chain")
    );
    assert_eq!(
            serde_json::to_string(&nft).expect("serialize nft"),
            format!(
                "{{\"nft_id\":\"{}\",\"issuer\":\"{}\",\"collection_id\":\"{}\",\"serial\":42,\"owner\":\"{}\",\"metadata_hash\":\"{}\",\"metadata_uri\":\"ipfs://bafybeigdyrzt\",\"flags\":3}}",
                nft.nft_id, issuer, collection_id, owner, metadata_hash
            )
        );

    let ledger = LedgerState::new_with_nfts(Vec::new(), vec![nft.clone()]);
    ledger
        .validate_nft_state(chain_id)
        .expect("valid nft ledger state");
    assert!(ledger.nft(&nft.nft_id).is_some());
    let indexes = ledger.nft_indexes(chain_id).expect("nft indexes");
    assert_eq!(
        indexes.by_owner.get(owner).expect("owner index"),
        &vec![nft.nft_id.clone()]
    );
    assert_eq!(
        indexes.by_issuer.get(issuer).expect("issuer index"),
        &vec![nft.nft_id.clone()]
    );
    assert_eq!(
        indexes
            .by_collection
            .get(collection_id)
            .expect("collection index"),
        &vec![nft.nft_id.clone()]
    );

    let mut burned = nft.clone();
    burned.burned = true;
    let burned_ledger = LedgerState::new_with_nfts(Vec::new(), vec![burned]);
    let burned_indexes = burned_ledger.nft_indexes(chain_id).expect("burned indexes");
    assert!(!burned_indexes.by_owner.contains_key(owner));
    assert_eq!(
        burned_indexes
            .by_issuer
            .get(issuer)
            .expect("issuer keeps burned"),
        &vec![nft.nft_id]
    );
}

#[test]
fn ledger_nft_state_rejects_duplicates_and_malformed_metadata() {
    let chain_id = "postfiat-local";
    let issuer = "pfissuer000000000000000000000000000000000";
    let owner = "pfowner0000000000000000000000000000000000";
    let metadata_hash = "cd".repeat(32);
    let nft =
        NftDefinition::new(chain_id, issuer, "COLLECT", 1, owner, metadata_hash).expect("nft");

    let duplicate = LedgerState::new_with_nfts(Vec::new(), vec![nft.clone(), nft.clone()]);
    assert!(duplicate.validate_nft_state(chain_id).is_err());

    let mut wrong_id = nft.clone();
    wrong_id.nft_id = "a".repeat(NFT_ID_HEX_LEN);
    assert!(wrong_id.validate_for_chain(chain_id).is_err());

    let mut uppercase_hash = nft.clone();
    uppercase_hash.metadata_hash = "AB".repeat(32);
    assert!(uppercase_hash.validate().is_err());

    let mut long_uri = nft.clone();
    long_uri.metadata_uri = "u".repeat(MAX_NFT_METADATA_URI_BYTES + 1);
    assert!(long_uri.validate().is_err());

    let mut unsupported_flags = nft.clone();
    unsupported_flags.flags = NFT_ALLOWED_FLAGS | 0x8000_0000;
    assert!(unsupported_flags.validate().is_err());

    let mut unsupported_collection_flags = nft.clone();
    unsupported_collection_flags.collection_flags = NFT_COLLECTION_ALLOWED_FLAGS | 0x8000_0000;
    assert!(unsupported_collection_flags.validate().is_err());

    let mut collection_policy_a = nft.clone();
    collection_policy_a.collection_flags = NFT_COLLECTION_FLAG_TRANSFER_LOCKED;
    let mut collection_policy_b =
        NftDefinition::new(chain_id, issuer, "COLLECT", 2, owner, "aa").expect("nft");
    collection_policy_b.collection_flags = NFT_COLLECTION_FLAG_BURN_LOCKED;
    let mismatched_collection_policy =
        LedgerState::new_with_nfts(Vec::new(), vec![collection_policy_a, collection_policy_b]);
    assert!(mismatched_collection_policy
        .validate_nft_state(chain_id)
        .is_err());

    assert!(NftDefinition::new(chain_id, issuer, "C".repeat(65), 1, owner, "aa").is_err());
    assert!(NftDefinition::new(chain_id, issuer, "COLLECT", 0, owner, "aa").is_err());
    assert!(NftDefinition::new(chain_id, issuer, "COLLECT", 1, owner, "").is_err());
}

#[test]
fn offer_id_and_state_validate_assets_and_indexes() {
    let chain_id = "postfiat-local";
    let owner = "pfowner0000000000000000000000000000000000";
    let issuer = "pfissuer000000000000000000000000000000000";
    let asset = AssetDefinition::new(chain_id, issuer, "USD", 1, 6).expect("asset");
    let offer = Offer::new(
        chain_id,
        owner,
        11,
        "PFT",
        125,
        asset.asset_id.clone(),
        50,
        9,
        25,
    )
    .expect("offer");
    assert_eq!(OFFER_ID_HEX_LEN, offer.offer_id.len());
    assert_ne!(
        offer.offer_id,
        offer_id(chain_id, owner, 12).expect("different sequence")
    );
    assert_ne!(
        offer.offer_id,
        offer_id("postfiat-other", owner, 11).expect("different chain")
    );
    assert_eq!(
        offer_book_key("PFT", &asset.asset_id).expect("book key"),
        format!("PFT->{}", asset.asset_id)
    );
    assert!(offer.is_open());

    let ledger = LedgerState::new_with_offers(
        Vec::new(),
        vec![asset.clone()],
        Vec::new(),
        vec![offer.clone()],
    );
    ledger
        .validate_offer_state(chain_id)
        .expect("valid offer ledger state");
    assert!(ledger.offer(&offer.offer_id).is_some());
    let indexes = ledger.offer_indexes(chain_id).expect("offer indexes");
    assert_eq!(
        indexes.by_owner.get(owner).expect("owner index"),
        &vec![offer.offer_id.clone()]
    );
    assert_eq!(
        indexes
            .by_book
            .get(&format!("PFT->{}", asset.asset_id))
            .expect("book index"),
        &vec![offer.offer_id.clone()]
    );
    assert_eq!(
        indexes.by_state.get(OFFER_STATE_OPEN).expect("state index"),
        &vec![offer.offer_id.clone()]
    );
    assert_eq!(
        indexes
            .by_expiration_height
            .get(&25)
            .expect("expiration index"),
        &vec![offer.offer_id]
    );

    let legacy: LedgerState =
        serde_json::from_str(r#"{"accounts":[]}"#).expect("parse legacy ledger");
    assert!(legacy.offers.is_empty());
    assert!(legacy.fastpay_recovery_policy.is_none());
    assert!(legacy.fastpay_recovery_committees.is_empty());
    assert!(legacy.fastpay_recovery_reveals.is_empty());
    assert!(legacy.fastpay_version_fences.is_empty());
    let json = serde_json::to_string(&LedgerState::empty()).expect("serialize empty ledger");
    assert!(!json.contains("offers"));
    assert!(!json.contains("fastpay_recovery_policy"));
    assert!(!json.contains("fastpay_recovery_committees"));
    assert!(!json.contains("fastpay_recovery_reveals"));
    assert!(!json.contains("fastpay_version_fences"));
}

#[test]
fn ledger_offer_state_rejects_duplicates_missing_assets_and_malformed_offers() {
    let chain_id = "postfiat-local";
    let owner = "pfowner0000000000000000000000000000000000";
    let issuer = "pfissuer000000000000000000000000000000000";
    let asset = AssetDefinition::new(chain_id, issuer, "USD", 1, 6).expect("asset");
    let offer = Offer::new(
        chain_id,
        owner,
        11,
        "PFT",
        125,
        asset.asset_id.clone(),
        50,
        9,
        25,
    )
    .expect("offer");

    let duplicate = LedgerState::new_with_offers(
        Vec::new(),
        vec![asset.clone()],
        Vec::new(),
        vec![offer.clone(), offer.clone()],
    );
    assert!(duplicate.validate_offer_state(chain_id).is_err());

    let missing_asset_offer = Offer::new(
        chain_id,
        owner,
        12,
        "PFT",
        125,
        "aa".repeat(ISSUED_ASSET_ID_HEX_LEN / 2),
        50,
        9,
        25,
    )
    .expect("syntactically valid offer");
    let missing_asset = LedgerState::new_with_offers(
        Vec::new(),
        vec![asset],
        Vec::new(),
        vec![missing_asset_offer],
    );
    assert!(missing_asset.validate_offer_state(chain_id).is_err());

    assert!(Offer::new(chain_id, owner, 13, "PFT", 0, "PFT", 50, 9, 25).is_err());
    assert!(Offer::new(chain_id, owner, 14, "PFT", 125, "PFT", 50, 9, 25).is_err());

    let mut wrong_id = offer.clone();
    wrong_id.offer_id = "bb".repeat(OFFER_ID_HEX_LEN / 2);
    assert!(wrong_id.validate_for_chain(chain_id).is_err());

    let mut filled = offer;
    filled.state = OFFER_STATE_FILLED.to_string();
    assert!(filled.validate().is_err());
    filled.taker_gets_amount_remaining = 0;
    filled.taker_pays_amount_remaining = 0;
    assert!(filled.validate().is_ok());
}

#[test]
fn escrow_id_and_state_validate_deterministic_ids() {
    let chain_id = "postfiat-local";
    let owner = "pfowner0000000000000000000000000000000000";
    let recipient = "bridge-recipient-000000000000000000000000";
    let escrow = Escrow::new(
        chain_id,
        owner,
        7,
        recipient,
        "PFT",
        125,
        1,
        "time_lock",
        10,
        20,
        3,
    )
    .expect("escrow");
    assert_eq!(
            "c7da2391e2c9d15690d0933503c5f3bdbe72ccf479a9aa0cb66465497961eda73770c040e33b2ec9e59dd75acac79c2f",
            escrow.escrow_id
        );
    assert_eq!(ESCROW_ID_HEX_LEN, escrow.escrow_id.len());
    assert_ne!(
        escrow.escrow_id,
        escrow_id(chain_id, owner, 8).expect("different sequence")
    );
    assert_ne!(
        escrow.escrow_id,
        escrow_id("postfiat-other", owner, 7).expect("different chain")
    );
    escrow.validate_for_chain(chain_id).expect("valid escrow");
    assert_eq!(
            serde_json::to_string(&escrow).expect("serialize escrow"),
            format!(
                "{{\"escrow_id\":\"{}\",\"owner\":\"{}\",\"owner_sequence\":7,\"recipient\":\"{}\",\"asset_id\":\"PFT\",\"amount\":125,\"fee\":1,\"condition\":\"time_lock\",\"finish_after\":10,\"cancel_after\":20,\"state\":\"open\",\"created_height\":3}}",
                escrow.escrow_id, owner, recipient
            )
        );

    let ledger = LedgerState::new_with_ledger_objects(
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![escrow.clone()],
    );
    ledger
        .validate_escrow_state(chain_id)
        .expect("valid escrow ledger state");
    assert!(ledger.escrow(&escrow.escrow_id).is_some());
}

#[test]
fn atomic_settlement_template_id_is_symmetric_and_builds_escrow_creates() {
    let chain_id = "postfiat-local";
    let pft_owner = "pfpftowner0000000000000000000000000000000";
    let issued_owner = "pfissuedowner000000000000000000000000000";
    let asset_id = "01".repeat(ISSUED_ASSET_ID_HEX_LEN / 2);
    let template = AtomicSettlementTemplate {
        left: AtomicSettlementTemplateLeg {
            owner: pft_owner.to_string(),
            recipient: issued_owner.to_string(),
            asset_id: "PFT".to_string(),
            amount: 125,
            owner_sequence: 7,
        },
        right: AtomicSettlementTemplateLeg {
            owner: issued_owner.to_string(),
            recipient: pft_owner.to_string(),
            asset_id: asset_id.clone(),
            amount: 40,
            owner_sequence: 3,
        },
        condition: "atomic-secret".to_string(),
        finish_after: 10,
        cancel_after: 20,
    };
    template.validate().expect("valid template");
    let settlement_id = atomic_settlement_template_id(chain_id, &template).expect("settlement id");
    assert_eq!(
            "1d4d5fe12a7f63874c512a210e14f17a3906dcfa24924bc37d0ab82e577de8f0cb558fff0f4cf222aaa9a1a3fbda074c",
            settlement_id
        );
    assert_eq!(ATOMIC_SETTLEMENT_TEMPLATE_ID_HEX_LEN, settlement_id.len());
    assert_ne!(
        settlement_id,
        atomic_settlement_template_id("postfiat-other", &template).expect("chain id")
    );

    let swapped = AtomicSettlementTemplate {
        left: template.right.clone(),
        right: template.left.clone(),
        ..template.clone()
    };
    assert_eq!(
        settlement_id,
        atomic_settlement_template_id(chain_id, &swapped).expect("symmetric id")
    );

    let (left, right) = template
        .escrow_create_operations()
        .expect("escrow create operations");
    assert_eq!(left.owner, pft_owner);
    assert_eq!(left.recipient, issued_owner);
    assert_eq!(left.asset_id, "PFT");
    assert_eq!(right.owner, issued_owner);
    assert_eq!(right.recipient, pft_owner);
    assert_eq!(right.asset_id, asset_id);
    assert_eq!(left.condition, "atomic-secret");
    assert_eq!(right.condition, "atomic-secret");
}

#[test]
fn atomic_settlement_template_rejects_non_reciprocal_or_non_swap_legs() {
    let pft_owner = "pfpftowner0000000000000000000000000000000";
    let issued_owner = "pfissuedowner000000000000000000000000000";
    let other = "pfother00000000000000000000000000000000000";
    let asset_id = "02".repeat(ISSUED_ASSET_ID_HEX_LEN / 2);
    let valid = AtomicSettlementTemplate {
        left: AtomicSettlementTemplateLeg {
            owner: pft_owner.to_string(),
            recipient: issued_owner.to_string(),
            asset_id: "PFT".to_string(),
            amount: 1,
            owner_sequence: 1,
        },
        right: AtomicSettlementTemplateLeg {
            owner: issued_owner.to_string(),
            recipient: pft_owner.to_string(),
            asset_id,
            amount: 1,
            owner_sequence: 1,
        },
        condition: "secret".to_string(),
        finish_after: 0,
        cancel_after: 5,
    };
    assert!(valid.validate().is_ok());

    let mut non_reciprocal = valid.clone();
    non_reciprocal.right.recipient = other.to_string();
    assert!(non_reciprocal.validate().is_err());

    let mut both_pft = valid.clone();
    both_pft.right.asset_id = "PFT".to_string();
    assert!(both_pft.validate().is_err());

    let mut missing_cancel = valid.clone();
    missing_cancel.cancel_after = 0;
    assert!(missing_cancel.validate().is_err());

    let mut empty_condition = valid;
    empty_condition.condition.clear();
    assert!(empty_condition.validate().is_err());
}

#[test]
fn escrow_indexes_group_deterministically_by_owner_recipient_condition_and_expiry() {
    let chain_id = "postfiat-local";
    let owner_a = "pfownera000000000000000000000000000000000";
    let owner_b = "pfownerb000000000000000000000000000000000";
    let recipient_a = "pfrecipienta0000000000000000000000000000";
    let recipient_b = "pfrecipientb0000000000000000000000000000";
    let escrow_later = Escrow::new(
        chain_id,
        owner_a,
        2,
        recipient_b,
        "PFT",
        50,
        1,
        "hashlock",
        5,
        20,
        3,
    )
    .expect("later escrow");
    let escrow_finish_only =
        Escrow::new(chain_id, owner_b, 1, recipient_a, "PFT", 10, 1, "", 5, 0, 1)
            .expect("finish-only escrow");
    let escrow_earlier = Escrow::new(
        chain_id,
        owner_a,
        1,
        recipient_a,
        "PFT",
        25,
        1,
        "hashlock",
        0,
        10,
        2,
    )
    .expect("earlier escrow");
    let ledger = LedgerState::new_with_ledger_objects(
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![
            escrow_later.clone(),
            escrow_finish_only.clone(),
            escrow_earlier.clone(),
        ],
    );

    let indexes = ledger.escrow_indexes(chain_id).expect("escrow indexes");
    let mut owner_a_ids = vec![
        escrow_later.escrow_id.clone(),
        escrow_earlier.escrow_id.clone(),
    ];
    owner_a_ids.sort();
    assert_eq!(
        indexes.by_owner.get(owner_a).expect("owner a index"),
        &owner_a_ids
    );
    let mut recipient_a_ids = vec![
        escrow_finish_only.escrow_id.clone(),
        escrow_earlier.escrow_id.clone(),
    ];
    recipient_a_ids.sort();
    assert_eq!(
        indexes
            .by_recipient
            .get(recipient_a)
            .expect("recipient a index"),
        &recipient_a_ids
    );

    let condition_hash = escrow_condition_hash("hashlock").expect("condition hash");
    assert_eq!(ESCROW_CONDITION_HASH_HEX_LEN, condition_hash.len());
    let mut condition_ids = vec![
        escrow_later.escrow_id.clone(),
        escrow_earlier.escrow_id.clone(),
    ];
    condition_ids.sort();
    assert_eq!(
        indexes
            .by_condition_hash
            .get(&condition_hash)
            .expect("condition index"),
        &condition_ids
    );
    let empty_condition_hash = escrow_condition_hash("").expect("empty condition hash");
    assert!(!indexes
        .by_condition_hash
        .contains_key(&empty_condition_hash));
    assert_eq!(
        indexes.by_expiry_height.get(&10).expect("expiry 10"),
        &vec![escrow_earlier.escrow_id.clone()]
    );
    assert_eq!(
        indexes.by_expiry_height.get(&20).expect("expiry 20"),
        &vec![escrow_later.escrow_id.clone()]
    );
    assert!(!indexes.by_expiry_height.contains_key(&0));
}

#[test]
fn ledger_escrow_state_rejects_duplicates_and_malformed_objects() {
    let chain_id = "postfiat-local";
    let owner = "pfowner0000000000000000000000000000000000";
    let recipient = "bridge-recipient-000000000000000000000000";
    let escrow = Escrow::new(
        chain_id,
        owner,
        7,
        recipient,
        "PFT",
        125,
        1,
        "time_lock",
        10,
        20,
        3,
    )
    .expect("escrow");

    let duplicate_escrows = LedgerState::new_with_ledger_objects(
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![escrow.clone(), escrow.clone()],
    );
    assert!(duplicate_escrows.validate_escrow_state(chain_id).is_err());

    let mut wrong_id = escrow.clone();
    wrong_id.escrow_id = "a".repeat(ESCROW_ID_HEX_LEN);
    let wrong_id_ledger =
        LedgerState::new_with_ledger_objects(Vec::new(), Vec::new(), Vec::new(), vec![wrong_id]);
    assert!(wrong_id_ledger.validate_escrow_state(chain_id).is_err());

    let mut invalid_state = escrow.clone();
    invalid_state.state = "released".to_string();
    assert!(invalid_state.validate_for_chain(chain_id).is_err());

    let mut invalid_timing = escrow.clone();
    invalid_timing.cancel_after = invalid_timing.finish_after;
    assert!(invalid_timing.validate_for_chain(chain_id).is_err());

    assert!(escrow_id(chain_id, owner, 0).is_err());
    assert!(Escrow::new(chain_id, owner, 1, owner, "PFT", 1, 1, "time_lock", 0, 1, 1).is_err());
}

#[test]
fn ledger_state_preserves_legacy_empty_serialization() {
    let ledger = LedgerState::empty();
    let json = serde_json::to_string(&ledger).expect("serialize ledger");
    assert_eq!(r#"{"accounts":[]}"#, json);

    let parsed: LedgerState = serde_json::from_str(r#"{"accounts":[]}"#).expect("parse ledger");
    assert!(parsed.asset_definitions.is_empty());
    assert!(parsed.trustlines.is_empty());
    assert!(parsed.escrows.is_empty());
    assert!(parsed.nfts.is_empty());
    assert_eq!(
        json,
        serde_json::to_string(&parsed).expect("serialize parsed")
    );
}

#[test]
fn unsigned_transfer_validation_rejects_malformed_fields() {
    let mut transfer = UnsignedTransfer {
        chain_id: "postfiat-local".to_string(),
        genesis_hash: "a".repeat(96),
        protocol_version: 1,
        address_namespace: ADDRESS_NAMESPACE.to_string(),
        transaction_kind: TRANSFER_TRANSACTION_KIND.to_string(),
        signature_algorithm_id: "ML-DSA-65".to_string(),
        from: "pfsender00000000000000000000000000000000".to_string(),
        to: "bridge-recipient-000000000000000000000000".to_string(),
        amount: 1,
        fee: 1,
        sequence: 1,
    };
    assert!(transfer.validate().is_ok());

    transfer.chain_id = " postfiat-local".to_string();
    assert!(transfer.validate().is_err());
    transfer.chain_id = "postfiat-local".to_string();

    transfer.genesis_hash = "A".repeat(96);
    assert!(transfer.validate().is_err());
    transfer.genesis_hash = "a".repeat(96);

    transfer.protocol_version = 0;
    assert!(transfer.validate().is_err());
    transfer.protocol_version = 1;

    transfer.address_namespace = " ".to_string();
    assert!(transfer.validate().is_err());
    transfer.address_namespace = ADDRESS_NAMESPACE.to_string();

    transfer.from = "\npfsender00000000000000000000000000000000".to_string();
    assert!(transfer.validate().is_err());
    transfer.from = "p".repeat(MAX_TEXT_FIELD_BYTES + 1);
    assert!(transfer.validate().is_err());
    transfer.from = "pfsender00000000000000000000000000000000".to_string();
}

#[test]
fn signed_transfer_validation_rejects_malformed_or_oversized_hex() {
    let unsigned = UnsignedTransfer {
        chain_id: "postfiat-local".to_string(),
        genesis_hash: "a".repeat(96),
        protocol_version: 1,
        address_namespace: ADDRESS_NAMESPACE.to_string(),
        transaction_kind: TRANSFER_TRANSACTION_KIND.to_string(),
        signature_algorithm_id: "ML-DSA-65".to_string(),
        from: "pfsender00000000000000000000000000000000".to_string(),
        to: "bridge-recipient-000000000000000000000000".to_string(),
        amount: 1,
        fee: 1,
        sequence: 1,
    };
    let mut transfer = SignedTransfer {
        unsigned,
        algorithm_id: "ML-DSA-65".to_string(),
        public_key_hex: "aa".to_string(),
        signature_hex: "bb".to_string(),
    };
    assert!(transfer.validate().is_ok());

    transfer.public_key_hex = "AA".to_string();
    assert!(transfer.validate().is_err());
    transfer.public_key_hex = "a".repeat(MAX_TRANSFER_PUBLIC_KEY_HEX_LEN + 2);
    assert!(transfer.validate().is_err());
    transfer.public_key_hex = "aa".to_string();

    transfer.signature_hex = "b".to_string();
    assert!(transfer.validate().is_err());
    transfer.signature_hex = "b".repeat(MAX_TRANSFER_SIGNATURE_HEX_LEN + 2);
    assert!(transfer.validate().is_err());
}

fn payment_v2_with_memos(memos: Vec<PaymentMemo>) -> UnsignedPaymentV2 {
    UnsignedPaymentV2 {
        chain_id: "postfiat-local".to_string(),
        genesis_hash: "a".repeat(96),
        protocol_version: 1,
        address_namespace: ADDRESS_NAMESPACE.to_string(),
        transaction_kind: PAYMENT_V2_TRANSACTION_KIND.to_string(),
        signature_algorithm_id: "ML-DSA-65".to_string(),
        from: "pfsender00000000000000000000000000000000".to_string(),
        to: "bridge-recipient-000000000000000000000000".to_string(),
        amount: 25,
        fee: 3,
        sequence: 7,
        memos,
    }
}

fn asset_create_unsigned() -> UnsignedAssetTransaction {
    UnsignedAssetTransaction {
        chain_id: "postfiat-local".to_string(),
        genesis_hash: "a".repeat(96),
        protocol_version: 1,
        address_namespace: ADDRESS_NAMESPACE.to_string(),
        transaction_kind: ASSET_CREATE_TRANSACTION_KIND.to_string(),
        signature_algorithm_id: "ML-DSA-65".to_string(),
        source: "pfissuer000000000000000000000000000000000".to_string(),
        fee: 5,
        sequence: 3,
        operation: AssetTransactionOperation::AssetCreate(AssetCreateOperation {
            issuer: "pfissuer000000000000000000000000000000000".to_string(),
            code: "USD".to_string(),
            version: 1,
            precision: 6,
            display_name: "US Dollar".to_string(),
            max_supply: Some(1_000_000),
            requires_authorization: true,
            freeze_enabled: true,
            clawback_enabled: false,
        }),
    }
}

fn escrow_create_unsigned() -> UnsignedEscrowTransaction {
    UnsignedEscrowTransaction {
        chain_id: "postfiat-local".to_string(),
        genesis_hash: "a".repeat(96),
        protocol_version: 1,
        address_namespace: ADDRESS_NAMESPACE.to_string(),
        transaction_kind: ESCROW_CREATE_TRANSACTION_KIND.to_string(),
        signature_algorithm_id: "ML-DSA-65".to_string(),
        source: "pfowner0000000000000000000000000000000000".to_string(),
        fee: 5,
        sequence: 7,
        operation: EscrowTransactionOperation::EscrowCreate(EscrowCreateOperation {
            owner: "pfowner0000000000000000000000000000000000".to_string(),
            recipient: "bridge-recipient-000000000000000000000000".to_string(),
            asset_id: "PFT".to_string(),
            amount: 125,
            condition: "time_lock".to_string(),
            finish_after: 10,
            cancel_after: 20,
        }),
    }
}

fn nft_mint_unsigned() -> UnsignedNftTransaction {
    UnsignedNftTransaction {
        chain_id: "postfiat-local".to_string(),
        genesis_hash: "a".repeat(96),
        protocol_version: 1,
        address_namespace: ADDRESS_NAMESPACE.to_string(),
        transaction_kind: NFT_MINT_TRANSACTION_KIND.to_string(),
        signature_algorithm_id: "ML-DSA-65".to_string(),
        source: "pfissuer000000000000000000000000000000000".to_string(),
        fee: 5,
        sequence: 9,
        operation: NftTransactionOperation::NftMint(NftMintOperation {
            issuer: "pfissuer000000000000000000000000000000000".to_string(),
            collection_id: "ART-2026".to_string(),
            serial: 42,
            owner: "pfowner0000000000000000000000000000000000".to_string(),
            metadata_hash: "ab".repeat(32),
            metadata_uri: "ipfs://bafybeigdyrzt".to_string(),
            flags: NFT_FLAG_TRANSFERABLE,
            collection_flags: 0,
            issuer_transfer_fee: 0,
        }),
    }
}

fn offer_create_unsigned() -> UnsignedOfferTransaction {
    UnsignedOfferTransaction {
        chain_id: "postfiat-local".to_string(),
        genesis_hash: "a".repeat(96),
        protocol_version: 1,
        address_namespace: ADDRESS_NAMESPACE.to_string(),
        transaction_kind: OFFER_CREATE_TRANSACTION_KIND.to_string(),
        signature_algorithm_id: "ML-DSA-65".to_string(),
        source: "pfowner0000000000000000000000000000000000".to_string(),
        fee: 5,
        sequence: 11,
        operation: OfferTransactionOperation::OfferCreate(OfferCreateOperation {
            owner: "pfowner0000000000000000000000000000000000".to_string(),
            taker_gets_asset_id: "PFT".to_string(),
            taker_gets_amount: 125,
            taker_pays_asset_id: "01".repeat(ISSUED_ASSET_ID_HEX_LEN / 2),
            taker_pays_amount: 50,
            expiration_height: 25,
        }),
    }
}

#[test]
fn legacy_transfer_signing_bytes_are_unchanged() {
    let transfer = UnsignedTransfer {
        chain_id: "postfiat-local".to_string(),
        genesis_hash: "a".repeat(96),
        protocol_version: 1,
        address_namespace: ADDRESS_NAMESPACE.to_string(),
        transaction_kind: TRANSFER_TRANSACTION_KIND.to_string(),
        signature_algorithm_id: "ML-DSA-65".to_string(),
        from: "pfsender00000000000000000000000000000000".to_string(),
        to: "bridge-recipient-000000000000000000000000".to_string(),
        amount: 25,
        fee: 3,
        sequence: 7,
    };

    let expected = format!(
            "postfiat.transfer.v1\nchain_id=postfiat-local\ngenesis_hash={}\nprotocol_version=1\naddress_namespace={}\ntransaction_kind={}\nsignature_algorithm_id=ML-DSA-65\nfrom=pfsender00000000000000000000000000000000\nto=bridge-recipient-000000000000000000000000\namount=25\nfee=3\nsequence=7\n",
            "a".repeat(96),
            ADDRESS_NAMESPACE,
            TRANSFER_TRANSACTION_KIND
        );
    assert_eq!(expected.into_bytes(), transfer.signing_bytes());
}

#[test]
fn payment_v2_signing_bytes_are_canonical_and_memo_bounded() {
    let payment = payment_v2_with_memos(vec![PaymentMemo {
        memo_type: "7061796d656e74".to_string(),
        memo_format: "746578742f706c61696e".to_string(),
        memo_data: "68656c6c6f".to_string(),
    }]);

    assert!(payment.validate().is_ok());
    assert_eq!(22, payment.memo_bytes());
    let expected = format!(
            "postfiat.payment.v2\nchain_id=postfiat-local\ngenesis_hash={}\nprotocol_version=1\naddress_namespace={}\ntransaction_kind={}\nsignature_algorithm_id=ML-DSA-65\nfrom=pfsender00000000000000000000000000000000\nto=bridge-recipient-000000000000000000000000\namount=25\nfee=3\nsequence=7\nmemo_count=1\nmemo[0].type_bytes=7\nmemo[0].type=7061796d656e74\nmemo[0].format_bytes=10\nmemo[0].format=746578742f706c61696e\nmemo[0].data_bytes=5\nmemo[0].data=68656c6c6f\n",
            "a".repeat(96),
            ADDRESS_NAMESPACE,
            PAYMENT_V2_TRANSACTION_KIND
        );
    assert_eq!(expected.into_bytes(), payment.signing_bytes());
}

#[test]
fn payment_v2_rejects_malformed_memos_and_zero_amount() {
    let valid_memo = PaymentMemo {
        memo_type: "74797065".to_string(),
        memo_format: "666d74".to_string(),
        memo_data: "00ff".to_string(),
    };
    assert!(payment_v2_with_memos(vec![valid_memo.clone()])
        .validate()
        .is_ok());

    let mut zero_amount = payment_v2_with_memos(vec![valid_memo.clone()]);
    zero_amount.amount = 0;
    assert!(zero_amount.validate().is_err());

    let mut odd_hex = valid_memo.clone();
    odd_hex.memo_data = "abc".to_string();
    assert!(payment_v2_with_memos(vec![odd_hex]).validate().is_err());

    let mut uppercase_hex = valid_memo.clone();
    uppercase_hex.memo_type = "AA".to_string();
    assert!(payment_v2_with_memos(vec![uppercase_hex])
        .validate()
        .is_err());

    let mut oversized_data = valid_memo.clone();
    oversized_data.memo_data = "aa".repeat(MAX_PAYMENT_MEMO_DATA_BYTES + 1);
    assert!(payment_v2_with_memos(vec![oversized_data])
        .validate()
        .is_err());

    let empty_memo = PaymentMemo {
        memo_type: String::new(),
        memo_format: String::new(),
        memo_data: String::new(),
    };
    assert!(payment_v2_with_memos(vec![empty_memo]).validate().is_err());
}

#[test]
fn payment_v2_rejects_memo_count_and_total_byte_overflow() {
    let memos = (0..=MAX_PAYMENT_MEMOS)
        .map(|_| PaymentMemo {
            memo_type: String::new(),
            memo_format: String::new(),
            memo_data: "aa".to_string(),
        })
        .collect::<Vec<_>>();
    assert!(payment_v2_with_memos(memos).validate().is_err());

    let oversized_total_memos = (0..3)
        .map(|_| PaymentMemo {
            memo_type: String::new(),
            memo_format: String::new(),
            memo_data: "aa".repeat(180),
        })
        .collect::<Vec<_>>();
    assert!(payment_v2_with_memos(oversized_total_memos)
        .validate()
        .is_err());
}

#[test]
fn signed_payment_v2_preimage_is_stable() {
    let signed = SignedPaymentV2 {
        unsigned: payment_v2_with_memos(Vec::new()),
        algorithm_id: "ML-DSA-65".to_string(),
        public_key_hex: "aa".to_string(),
        signature_hex: "bb".to_string(),
    };
    assert!(signed.validate().is_ok());

    let expected = format!(
            "postfiat.payment.v2\nchain_id=postfiat-local\ngenesis_hash={}\nprotocol_version=1\naddress_namespace={}\ntransaction_kind={}\nsignature_algorithm_id=ML-DSA-65\nfrom=pfsender00000000000000000000000000000000\nto=bridge-recipient-000000000000000000000000\namount=25\nfee=3\nsequence=7\nmemo_count=0\nalgorithm=ML-DSA-65\npublic_key=aa\nsignature=bb\n",
            "a".repeat(96),
            ADDRESS_NAMESPACE,
            PAYMENT_V2_TRANSACTION_KIND
        );
    assert_eq!(expected.into_bytes(), signed.tx_id_preimage_bytes());
}

#[test]
fn asset_transaction_create_signing_bytes_are_canonical() {
    let unsigned = asset_create_unsigned();
    assert!(unsigned.validate().is_ok());

    let expected = format!(
            "postfiat.asset_transaction.v1\nchain_id=postfiat-local\ngenesis_hash={}\nprotocol_version=1\naddress_namespace={}\ntransaction_kind={}\nsignature_algorithm_id=ML-DSA-65\nsource=pfissuer000000000000000000000000000000000\nfee=5\nsequence=3\noperation={}\nissuer=pfissuer000000000000000000000000000000000\ncode_bytes=3\ncode=USD\nversion=1\nprecision=6\ndisplay_name_bytes=9\ndisplay_name=US Dollar\nmax_supply_present=true\nmax_supply=1000000\nrequires_authorization=true\nfreeze_enabled=true\nclawback_enabled=false\n",
            "a".repeat(96),
            ADDRESS_NAMESPACE,
            ASSET_CREATE_TRANSACTION_KIND,
            ASSET_CREATE_TRANSACTION_KIND
        );
    assert_eq!(expected.into_bytes(), unsigned.signing_bytes());

    let json = serde_json::to_string(&unsigned).expect("serialize asset transaction");
    assert!(json.contains(r#""operation":"asset_create""#));
    assert!(json.contains(r#""issuer":"pfissuer000000000000000000000000000000000""#));
}

#[test]
fn signed_asset_transaction_preimage_is_stable() {
    let signed = SignedAssetTransaction {
        unsigned: asset_create_unsigned(),
        algorithm_id: "ML-DSA-65".to_string(),
        public_key_hex: "aa".to_string(),
        signature_hex: "bb".to_string(),
    };
    assert!(signed.validate().is_ok());

    let expected = format!(
            "postfiat.asset_transaction.v1\nchain_id=postfiat-local\ngenesis_hash={}\nprotocol_version=1\naddress_namespace={}\ntransaction_kind={}\nsignature_algorithm_id=ML-DSA-65\nsource=pfissuer000000000000000000000000000000000\nfee=5\nsequence=3\noperation={}\nissuer=pfissuer000000000000000000000000000000000\ncode_bytes=3\ncode=USD\nversion=1\nprecision=6\ndisplay_name_bytes=9\ndisplay_name=US Dollar\nmax_supply_present=true\nmax_supply=1000000\nrequires_authorization=true\nfreeze_enabled=true\nclawback_enabled=false\nalgorithm=ML-DSA-65\npublic_key=aa\nsignature=bb\n",
            "a".repeat(96),
            ADDRESS_NAMESPACE,
            ASSET_CREATE_TRANSACTION_KIND,
            ASSET_CREATE_TRANSACTION_KIND
        );
    assert_eq!(expected.into_bytes(), signed.tx_id_preimage_bytes());
}

#[test]
fn asset_transaction_validation_covers_all_asset_operations() {
    let issuer = "pfissuer000000000000000000000000000000000";
    let holder = "pfholder00000000000000000000000000000000";
    let recipient = "bridge-recipient-000000000000000000000000";
    let asset_id = issued_asset_id("postfiat-local", issuer, "USD", 1).expect("asset id");

    let trust_set = UnsignedAssetTransaction {
        chain_id: "postfiat-local".to_string(),
        genesis_hash: "a".repeat(96),
        protocol_version: 1,
        address_namespace: ADDRESS_NAMESPACE.to_string(),
        transaction_kind: TRUST_SET_TRANSACTION_KIND.to_string(),
        signature_algorithm_id: "ML-DSA-65".to_string(),
        source: holder.to_string(),
        fee: 4,
        sequence: 1,
        operation: AssetTransactionOperation::TrustSet(TrustSetOperation {
            account: holder.to_string(),
            issuer: issuer.to_string(),
            asset_id: asset_id.clone(),
            limit: 500,
            authorized: false,
            frozen: false,
            reserve_paid: 10,
        }),
    };
    assert!(trust_set.validate().is_ok());

    let issued_payment = UnsignedAssetTransaction {
        transaction_kind: ISSUED_PAYMENT_TRANSACTION_KIND.to_string(),
        source: holder.to_string(),
        operation: AssetTransactionOperation::IssuedPayment(IssuedPaymentOperation {
            from: holder.to_string(),
            to: recipient.to_string(),
            issuer: issuer.to_string(),
            asset_id: asset_id.clone(),
            amount: 25,
        }),
        ..trust_set.clone()
    };
    assert!(issued_payment.validate().is_ok());

    let asset_burn = UnsignedAssetTransaction {
        transaction_kind: ASSET_BURN_TRANSACTION_KIND.to_string(),
        source: holder.to_string(),
        operation: AssetTransactionOperation::AssetBurn(AssetBurnOperation {
            owner: holder.to_string(),
            issuer: issuer.to_string(),
            asset_id: asset_id.clone(),
            amount: 10,
        }),
        ..trust_set
    };
    assert!(asset_burn.validate().is_ok());

    let asset_clawback = UnsignedAssetTransaction {
        transaction_kind: ASSET_CLAWBACK_TRANSACTION_KIND.to_string(),
        source: issuer.to_string(),
        operation: AssetTransactionOperation::AssetClawback(AssetClawbackOperation {
            owner: holder.to_string(),
            issuer: issuer.to_string(),
            asset_id: asset_id.clone(),
            amount: 5,
        }),
        ..asset_burn.clone()
    };
    assert!(asset_clawback.validate().is_ok());
    let expected_clawback_signing_bytes = format!(
            "postfiat.asset_transaction.v1\nchain_id=postfiat-local\ngenesis_hash={}\nprotocol_version=1\naddress_namespace={}\ntransaction_kind={}\nsignature_algorithm_id=ML-DSA-65\nsource={}\nfee=4\nsequence=1\noperation={}\nowner={}\nissuer={}\nasset_id={}\namount=5\n",
            "a".repeat(96),
            ADDRESS_NAMESPACE,
            ASSET_CLAWBACK_TRANSACTION_KIND,
            issuer,
            ASSET_CLAWBACK_TRANSACTION_KIND,
            holder,
            issuer,
            asset_id
        );
    assert_eq!(
        expected_clawback_signing_bytes.into_bytes(),
        asset_clawback.signing_bytes()
    );

    let wrong_clawback_source = UnsignedAssetTransaction {
        source: holder.to_string(),
        ..asset_clawback.clone()
    };
    assert!(wrong_clawback_source.validate().is_err());

    let native_pft_clawback = UnsignedAssetTransaction {
        operation: AssetTransactionOperation::AssetClawback(AssetClawbackOperation {
            owner: holder.to_string(),
            issuer: issuer.to_string(),
            asset_id: "PFT".to_string(),
            amount: 5,
        }),
        ..asset_clawback
    };
    assert!(native_pft_clawback.validate().is_err());

    let mut wrong_kind = asset_create_unsigned();
    wrong_kind.transaction_kind = ISSUED_PAYMENT_TRANSACTION_KIND.to_string();
    assert!(wrong_kind.validate().is_err());

    let mut wrong_source = asset_create_unsigned();
    wrong_source.source = holder.to_string();
    assert!(wrong_source.validate().is_err());

    let mut zero_amount = issued_payment;
    if let AssetTransactionOperation::IssuedPayment(operation) = &mut zero_amount.operation {
        operation.amount = 0;
    }
    assert!(zero_amount.validate().is_err());
}

#[test]
fn escrow_transaction_create_signing_bytes_are_canonical() {
    let unsigned = escrow_create_unsigned();
    assert!(unsigned.validate().is_ok());

    let expected = format!(
            "postfiat.escrow_transaction.v1\nchain_id=postfiat-local\ngenesis_hash={}\nprotocol_version=1\naddress_namespace={}\ntransaction_kind={}\nsignature_algorithm_id=ML-DSA-65\nsource=pfowner0000000000000000000000000000000000\nfee=5\nsequence=7\noperation={}\nowner=pfowner0000000000000000000000000000000000\nrecipient=bridge-recipient-000000000000000000000000\nasset_id=PFT\namount=125\ncondition_bytes=9\ncondition=time_lock\nfinish_after=10\ncancel_after=20\n",
            "a".repeat(96),
            ADDRESS_NAMESPACE,
            ESCROW_CREATE_TRANSACTION_KIND,
            ESCROW_CREATE_TRANSACTION_KIND
        );
    assert_eq!(expected.into_bytes(), unsigned.signing_bytes());
}

#[test]
fn signed_escrow_transaction_preimage_is_stable() {
    let signed = SignedEscrowTransaction {
        unsigned: escrow_create_unsigned(),
        algorithm_id: "ML-DSA-65".to_string(),
        public_key_hex: "aa".to_string(),
        signature_hex: "bb".to_string(),
    };
    assert!(signed.validate().is_ok());

    let expected = format!(
            "postfiat.escrow_transaction.v1\nchain_id=postfiat-local\ngenesis_hash={}\nprotocol_version=1\naddress_namespace={}\ntransaction_kind={}\nsignature_algorithm_id=ML-DSA-65\nsource=pfowner0000000000000000000000000000000000\nfee=5\nsequence=7\noperation={}\nowner=pfowner0000000000000000000000000000000000\nrecipient=bridge-recipient-000000000000000000000000\nasset_id=PFT\namount=125\ncondition_bytes=9\ncondition=time_lock\nfinish_after=10\ncancel_after=20\nalgorithm=ML-DSA-65\npublic_key=aa\nsignature=bb\n",
            "a".repeat(96),
            ADDRESS_NAMESPACE,
            ESCROW_CREATE_TRANSACTION_KIND,
            ESCROW_CREATE_TRANSACTION_KIND
        );
    assert_eq!(expected.into_bytes(), signed.tx_id_preimage_bytes());
}

#[test]
fn escrow_transaction_validation_covers_all_operations() {
    let owner = "pfowner0000000000000000000000000000000000";
    let recipient = "bridge-recipient-000000000000000000000000";
    let escrow_id = escrow_id("postfiat-local", owner, 7).expect("escrow id");

    let create = escrow_create_unsigned();
    assert!(create.validate().is_ok());
    let mut issued_create = escrow_create_unsigned();
    if let EscrowTransactionOperation::EscrowCreate(operation) = &mut issued_create.operation {
        operation.asset_id = "01".repeat(ISSUED_ASSET_ID_HEX_LEN / 2);
    }
    assert!(issued_create.validate().is_ok());

    let finish = UnsignedEscrowTransaction {
        transaction_kind: ESCROW_FINISH_TRANSACTION_KIND.to_string(),
        source: recipient.to_string(),
        operation: EscrowTransactionOperation::EscrowFinish(EscrowFinishOperation {
            escrow_id: escrow_id.clone(),
            owner: owner.to_string(),
            recipient: recipient.to_string(),
            fulfillment: "preimage".to_string(),
        }),
        ..create.clone()
    };
    assert!(finish.validate().is_ok());

    let cancel = UnsignedEscrowTransaction {
        transaction_kind: ESCROW_CANCEL_TRANSACTION_KIND.to_string(),
        source: owner.to_string(),
        operation: EscrowTransactionOperation::EscrowCancel(EscrowCancelOperation {
            escrow_id,
            owner: owner.to_string(),
        }),
        ..create
    };
    assert!(cancel.validate().is_ok());

    let mut wrong_kind = escrow_create_unsigned();
    wrong_kind.transaction_kind = ESCROW_CANCEL_TRANSACTION_KIND.to_string();
    assert!(wrong_kind.validate().is_err());

    let mut wrong_source = escrow_create_unsigned();
    wrong_source.source = recipient.to_string();
    assert!(wrong_source.validate().is_err());

    let mut zero_amount = escrow_create_unsigned();
    if let EscrowTransactionOperation::EscrowCreate(operation) = &mut zero_amount.operation {
        operation.amount = 0;
    }
    assert!(zero_amount.validate().is_err());

    let mut bad_asset_id = escrow_create_unsigned();
    if let EscrowTransactionOperation::EscrowCreate(operation) = &mut bad_asset_id.operation {
        operation.asset_id = "USD".to_string();
    }
    assert!(bad_asset_id.validate().is_err());
}

#[test]
fn nft_transaction_mint_signing_bytes_are_canonical() {
    let unsigned = nft_mint_unsigned();
    assert!(unsigned.validate().is_ok());

    let expected = format!(
            "postfiat.nft_transaction.v1\nchain_id=postfiat-local\ngenesis_hash={}\nprotocol_version=1\naddress_namespace={}\ntransaction_kind={}\nsignature_algorithm_id=ML-DSA-65\nsource=pfissuer000000000000000000000000000000000\nfee=5\nsequence=9\noperation={}\nissuer=pfissuer000000000000000000000000000000000\ncollection_id_bytes=8\ncollection_id=ART-2026\nserial=42\nowner=pfowner0000000000000000000000000000000000\nmetadata_hash_bytes=32\nmetadata_hash={}\nmetadata_uri_bytes=20\nmetadata_uri=ipfs://bafybeigdyrzt\nflags=1\n",
            "a".repeat(96),
            ADDRESS_NAMESPACE,
            NFT_MINT_TRANSACTION_KIND,
            NFT_MINT_TRANSACTION_KIND,
            "ab".repeat(32)
        );
    assert_eq!(expected.into_bytes(), unsigned.signing_bytes());

    let json = serde_json::to_string(&unsigned).expect("serialize nft transaction");
    assert!(json.contains(r#""operation":"nft_mint""#));
    assert!(json.contains(r#""collection_id":"ART-2026""#));
}

#[test]
fn nft_mint_collection_flags_signing_bytes_are_canonical() {
    let mut unsigned = nft_mint_unsigned();
    if let NftTransactionOperation::NftMint(operation) = &mut unsigned.operation {
        operation.collection_flags =
            NFT_COLLECTION_FLAG_TRANSFER_LOCKED | NFT_COLLECTION_FLAG_BURN_LOCKED;
    }
    assert!(unsigned.validate().is_ok());
    let signing_bytes = String::from_utf8(unsigned.signing_bytes()).expect("utf8 preimage");
    assert!(signing_bytes.contains("flags=1\ncollection_flags=3\n"));

    if let NftTransactionOperation::NftMint(operation) = &mut unsigned.operation {
        operation.collection_flags = NFT_COLLECTION_ALLOWED_FLAGS | 0x8000_0000;
    }
    assert!(unsigned.validate().is_err());
}

#[test]
fn nft_transfer_issuer_fee_signing_bytes_are_canonical() {
    let issuer = "pfissuer000000000000000000000000000000000";
    let owner = "pfowner0000000000000000000000000000000000";
    let recipient = "bridge-recipient-000000000000000000000000";
    let nft_id = nft_id("postfiat-local", issuer, "ART-2026", 42).expect("nft id");
    let unsigned = UnsignedNftTransaction {
        transaction_kind: NFT_TRANSFER_TRANSACTION_KIND.to_string(),
        source: owner.to_string(),
        operation: NftTransactionOperation::NftTransfer(NftTransferOperation {
            nft_id: nft_id.clone(),
            from: owner.to_string(),
            to: recipient.to_string(),
            issuer: issuer.to_string(),
            issuer_transfer_fee: 7,
        }),
        ..nft_mint_unsigned()
    };
    assert!(unsigned.validate().is_ok());

    let expected = format!(
            "postfiat.nft_transaction.v1\nchain_id=postfiat-local\ngenesis_hash={}\nprotocol_version=1\naddress_namespace={}\ntransaction_kind={}\nsignature_algorithm_id=ML-DSA-65\nsource={}\nfee=5\nsequence=9\noperation={}\nnft_id={}\nfrom={}\nto={}\nissuer={}\nissuer_transfer_fee=7\n",
            "a".repeat(96),
            ADDRESS_NAMESPACE,
            NFT_TRANSFER_TRANSACTION_KIND,
            owner,
            NFT_TRANSFER_TRANSACTION_KIND,
            nft_id,
            owner,
            recipient,
            issuer
        );
    assert_eq!(expected.into_bytes(), unsigned.signing_bytes());
}

#[test]
fn signed_nft_transaction_preimage_is_stable() {
    let signed = SignedNftTransaction {
        unsigned: nft_mint_unsigned(),
        algorithm_id: "ML-DSA-65".to_string(),
        public_key_hex: "aa".to_string(),
        signature_hex: "bb".to_string(),
    };
    assert!(signed.validate().is_ok());

    let expected = format!(
            "postfiat.nft_transaction.v1\nchain_id=postfiat-local\ngenesis_hash={}\nprotocol_version=1\naddress_namespace={}\ntransaction_kind={}\nsignature_algorithm_id=ML-DSA-65\nsource=pfissuer000000000000000000000000000000000\nfee=5\nsequence=9\noperation={}\nissuer=pfissuer000000000000000000000000000000000\ncollection_id_bytes=8\ncollection_id=ART-2026\nserial=42\nowner=pfowner0000000000000000000000000000000000\nmetadata_hash_bytes=32\nmetadata_hash={}\nmetadata_uri_bytes=20\nmetadata_uri=ipfs://bafybeigdyrzt\nflags=1\nalgorithm=ML-DSA-65\npublic_key=aa\nsignature=bb\n",
            "a".repeat(96),
            ADDRESS_NAMESPACE,
            NFT_MINT_TRANSACTION_KIND,
            NFT_MINT_TRANSACTION_KIND,
            "ab".repeat(32)
        );
    assert_eq!(expected.into_bytes(), signed.tx_id_preimage_bytes());
}

#[test]
fn nft_transaction_validation_covers_all_operations() {
    let issuer = "pfissuer000000000000000000000000000000000";
    let owner = "pfowner0000000000000000000000000000000000";
    let recipient = "bridge-recipient-000000000000000000000000";
    let nft_id = nft_id("postfiat-local", issuer, "ART-2026", 42).expect("nft id");

    let mint = nft_mint_unsigned();
    assert!(mint.validate().is_ok());

    let transfer = UnsignedNftTransaction {
        transaction_kind: NFT_TRANSFER_TRANSACTION_KIND.to_string(),
        source: owner.to_string(),
        operation: NftTransactionOperation::NftTransfer(NftTransferOperation {
            nft_id: nft_id.clone(),
            from: owner.to_string(),
            to: recipient.to_string(),
            issuer: String::new(),
            issuer_transfer_fee: 0,
        }),
        ..mint.clone()
    };
    assert!(transfer.validate().is_ok());

    let burn = UnsignedNftTransaction {
        transaction_kind: NFT_BURN_TRANSACTION_KIND.to_string(),
        source: owner.to_string(),
        operation: NftTransactionOperation::NftBurn(NftBurnOperation {
            nft_id,
            owner: owner.to_string(),
        }),
        ..mint.clone()
    };
    assert!(burn.validate().is_ok());

    let mut wrong_kind = nft_mint_unsigned();
    wrong_kind.transaction_kind = NFT_TRANSFER_TRANSACTION_KIND.to_string();
    assert!(wrong_kind.validate().is_err());

    let mut wrong_source = nft_mint_unsigned();
    wrong_source.source = owner.to_string();
    assert!(wrong_source.validate().is_err());

    let mut oversized_uri = nft_mint_unsigned();
    if let NftTransactionOperation::NftMint(operation) = &mut oversized_uri.operation {
        operation.metadata_uri = "u".repeat(MAX_NFT_METADATA_URI_BYTES + 1);
    }
    assert!(oversized_uri.validate().is_err());

    let mut same_recipient = transfer;
    if let NftTransactionOperation::NftTransfer(operation) = &mut same_recipient.operation {
        operation.to = operation.from.clone();
    }
    assert!(same_recipient.validate().is_err());

    let mut bad_nft_id = burn;
    if let NftTransactionOperation::NftBurn(operation) = &mut bad_nft_id.operation {
        operation.nft_id = "AA".repeat(NFT_ID_HEX_LEN / 2);
    }
    assert!(bad_nft_id.validate().is_err());
}

#[test]
fn offer_transaction_create_signing_bytes_are_canonical() {
    let unsigned = offer_create_unsigned();
    assert!(unsigned.validate().is_ok());

    let expected = format!(
            "postfiat.offer_transaction.v1\nchain_id=postfiat-local\ngenesis_hash={}\nprotocol_version=1\naddress_namespace={}\ntransaction_kind={}\nsignature_algorithm_id=ML-DSA-65\nsource=pfowner0000000000000000000000000000000000\nfee=5\nsequence=11\noperation={}\nowner=pfowner0000000000000000000000000000000000\ntaker_gets_asset_id=PFT\ntaker_gets_amount=125\ntaker_pays_asset_id={}\ntaker_pays_amount=50\nexpiration_height=25\n",
            "a".repeat(96),
            ADDRESS_NAMESPACE,
            OFFER_CREATE_TRANSACTION_KIND,
            OFFER_CREATE_TRANSACTION_KIND,
            "01".repeat(ISSUED_ASSET_ID_HEX_LEN / 2)
        );
    assert_eq!(expected.into_bytes(), unsigned.signing_bytes());

    let json = serde_json::to_string(&unsigned).expect("serialize offer transaction");
    assert!(json.contains(r#""operation":"offer_create""#));
    assert!(json.contains(r#""taker_gets_asset_id":"PFT""#));
}

#[test]
fn signed_offer_transaction_preimage_is_stable() {
    let signed = SignedOfferTransaction {
        unsigned: offer_create_unsigned(),
        algorithm_id: "ML-DSA-65".to_string(),
        public_key_hex: "aa".to_string(),
        signature_hex: "bb".to_string(),
    };
    assert!(signed.validate().is_ok());

    let expected = format!(
            "postfiat.offer_transaction.v1\nchain_id=postfiat-local\ngenesis_hash={}\nprotocol_version=1\naddress_namespace={}\ntransaction_kind={}\nsignature_algorithm_id=ML-DSA-65\nsource=pfowner0000000000000000000000000000000000\nfee=5\nsequence=11\noperation={}\nowner=pfowner0000000000000000000000000000000000\ntaker_gets_asset_id=PFT\ntaker_gets_amount=125\ntaker_pays_asset_id={}\ntaker_pays_amount=50\nexpiration_height=25\nalgorithm=ML-DSA-65\npublic_key=aa\nsignature=bb\n",
            "a".repeat(96),
            ADDRESS_NAMESPACE,
            OFFER_CREATE_TRANSACTION_KIND,
            OFFER_CREATE_TRANSACTION_KIND,
            "01".repeat(ISSUED_ASSET_ID_HEX_LEN / 2)
        );
    assert_eq!(expected.into_bytes(), signed.tx_id_preimage_bytes());
}

#[test]
fn offer_transaction_validation_covers_create_and_cancel() {
    let owner = "pfowner0000000000000000000000000000000000";
    let recipient = "bridge-recipient-000000000000000000000000";
    let offer_id = offer_id("postfiat-local", owner, 11).expect("offer id");

    let create = offer_create_unsigned();
    assert!(create.validate().is_ok());

    let cancel = UnsignedOfferTransaction {
        transaction_kind: OFFER_CANCEL_TRANSACTION_KIND.to_string(),
        source: owner.to_string(),
        operation: OfferTransactionOperation::OfferCancel(OfferCancelOperation {
            offer_id,
            owner: owner.to_string(),
        }),
        ..create.clone()
    };
    assert!(cancel.validate().is_ok());

    let mut wrong_kind = offer_create_unsigned();
    wrong_kind.transaction_kind = OFFER_CANCEL_TRANSACTION_KIND.to_string();
    assert!(wrong_kind.validate().is_err());

    let mut wrong_source = offer_create_unsigned();
    wrong_source.source = recipient.to_string();
    assert!(wrong_source.validate().is_err());

    let mut zero_amount = offer_create_unsigned();
    if let OfferTransactionOperation::OfferCreate(operation) = &mut zero_amount.operation {
        operation.taker_gets_amount = 0;
    }
    assert!(zero_amount.validate().is_err());

    let mut same_asset = offer_create_unsigned();
    if let OfferTransactionOperation::OfferCreate(operation) = &mut same_asset.operation {
        operation.taker_pays_asset_id = "PFT".to_string();
    }
    assert!(same_asset.validate().is_err());

    let mut bad_offer_id = cancel;
    if let OfferTransactionOperation::OfferCancel(operation) = &mut bad_offer_id.operation {
        operation.offer_id = "AA".repeat(OFFER_ID_HEX_LEN / 2);
    }
    assert!(bad_offer_id.validate().is_err());
}

#[test]
fn nav_profile_register_omits_default_sp1_fields_for_legacy_serialization() {
    let legacy_json = r#"{"registrant":"pfissuer","verifier_kind":"placeholder","source_class":"a651-interim","max_snapshot_age_blocks":100000,"challenge_window_blocks":1,"max_epoch_gap_blocks":100000,"settle_deadline_blocks":0,"min_challenge_bond":0,"min_attestations":0,"tolerance_bp":0,"valuation_policy_hash":""}"#;
    let op: NavProfileRegisterOperation =
        serde_json::from_str(legacy_json).expect("parse legacy nav profile register");
    let json = serde_json::to_string(&op).expect("serialize nav profile register");
    assert_eq!(legacy_json, json);
}

#[test]
fn nav_profile_register_binds_governed_route_separately_from_sp1_policy() {
    let route_hash = "33".repeat(NAV_PROFILE_ID_HEX_LEN / 2);
    let operation = NavProfileRegisterOperation {
        registrant: "pfissuer".to_string(),
        verifier_kind: NAV_PROFILE_VERIFIER_SP1_GROTH16.to_string(),
        source_class: "vault_bridge:erc20_bridge_vault:42161:vault:token".to_string(),
        max_snapshot_age_blocks: 100,
        challenge_window_blocks: 2,
        max_epoch_gap_blocks: 1_000,
        settle_deadline_blocks: 1_000,
        min_challenge_bond: 1,
        min_attestations: 0,
        tolerance_bp: 0,
        bridge_observer_min_confirmations: 0,
        valuation_policy_hash: "44".repeat(NAV_SP1_POLICY_HASH_HEX_LEN / 2),
        vault_bridge_route_policy_hash: route_hash.clone(),
        sp1_program_vkey: format!("0x{}", "55".repeat(32)),
        sp1_proof_encoding: NAV_SP1_PROOF_ENCODING_GROTH16.to_string(),
        max_proof_bytes: 512,
        max_public_values_bytes: 256,
    };
    operation.validate().expect("route-bound SP1 profile");
    let json = serde_json::to_value(&operation).expect("route-bound profile JSON");
    assert_eq!(
        json["vault_bridge_route_policy_hash"],
        serde_json::json!(route_hash)
    );

    let mut non_bridge = operation;
    non_bridge.source_class = "a651-sp1".to_string();
    assert!(non_bridge
        .validate()
        .expect_err("route hash outside vault bridge must reject")
        .contains("requires vault_bridge source_class"));
}

#[test]
fn nav_sp1_profile_requires_policy_hash_and_omits_default_limits() {
    let sp1_vkey = format!("0x{}", "11".repeat(32));
    let missing_policy = NavProfileRegisterOperation {
        registrant: "pfissuer".to_string(),
        verifier_kind: NAV_PROFILE_VERIFIER_SP1_GROTH16.to_string(),
        source_class: "a651-sp1".to_string(),
        max_snapshot_age_blocks: 100_000,
        challenge_window_blocks: 1,
        max_epoch_gap_blocks: 100_000,
        settle_deadline_blocks: 0,
        min_challenge_bond: 0,
        min_attestations: 0,
        tolerance_bp: 0,
        bridge_observer_min_confirmations: 0,
        valuation_policy_hash: String::new(),
        vault_bridge_route_policy_hash: String::new(),
        sp1_program_vkey: sp1_vkey.clone(),
        sp1_proof_encoding: NAV_SP1_PROOF_ENCODING_GROTH16.to_string(),
        max_proof_bytes: 0,
        max_public_values_bytes: 0,
    };
    let error = missing_policy
        .validate()
        .expect_err("sp1 profile without policy hash must reject");
    assert!(error.contains("valuation_policy_hash is required"));

    let profile = NavProofProfile::new(
        "pfissuer",
        NAV_PROFILE_VERIFIER_SP1_GROTH16,
        "a651-sp1",
        100_000,
        1,
        100_000,
        0,
        0,
        0,
        0,
        "22".repeat(NAV_SP1_POLICY_HASH_HEX_LEN / 2),
        &sp1_vkey,
        NAV_SP1_PROOF_ENCODING_GROTH16,
        0,
        0,
    )
    .expect("valid sp1 profile");
    let json = serde_json::to_string(&profile).expect("serialize sp1 profile");
    assert!(!json.contains("max_proof_bytes"));
    assert!(!json.contains("max_public_values_bytes"));
}

fn sample_market_ops_envelope() -> MarketOpsEnvelope {
    MarketOpsEnvelope {
        encoding_version: 1,
        chain_id: 1,
        adapter_address: [0x11; 20],
        vault_address: [0x12; 20],
        mint_controller_address: [0x13; 20],
        asset_id: [0x21; 32],
        epoch: 42,
        program_id: [0x31; 32],
        policy_hash: [0x32; 32],
        parameter_hash: [0x33; 32],
        reserve_packet_hash: [0x34; 32],
        supply_packet_hash: [0x35; 32],
        evidence_root: [0x36; 32],
        previous_market_state_hash: [0x37; 32],
        venue_id: [0x41; 32],
        pool_config_hash: [0x42; 32],
        hook_code_hash: [0x43; 32],
        nav_floor_usd_e8: 500_000_000,
        valid_global_supply_atoms: 1_000_000_000_000_000_000_000_000,
        verified_net_assets_usd_e8: 5_000_000_000_000_000,
        funded_alignment_reserve_usd_e8: 15_000_000_000_000,
        required_alignment_reserve_usd_e8: 13_500_000_000_000,
        max_reserve_deploy_usd_e8: 2_587_500_000_000,
        max_mint_atoms: 8_300_000_000_000_000_000_000,
        discount_trigger_bps: 250,
        premium_trigger_bps: 1_000,
        data_window_start: 1_800_000_000,
        data_window_end: 1_801_209_600,
        valid_after: 1_801_210_200,
        expires_at: 1_801_213_800,
        cooldown_seconds: 3_600,
        nonce: [0x51; 32],
    }
}

#[test]
fn market_ops_envelope_hash_is_deterministic_and_covers_every_field() {
    let base = sample_market_ops_envelope();
    let base_hash = base.envelope_hash();
    assert_eq!(base_hash, base.clone().envelope_hash());

    macro_rules! assert_hash_changes {
        ($field:literal, $mutate:expr) => {{
            let mut changed = base.clone();
            $mutate(&mut changed);
            assert_ne!(
                base_hash,
                changed.envelope_hash(),
                "{} did not affect envelope_hash",
                $field
            );
        }};
    }

    assert_hash_changes!("encoding_version", |e: &mut MarketOpsEnvelope| e
        .encoding_version +=
        1);
    assert_hash_changes!("chain_id", |e: &mut MarketOpsEnvelope| e.chain_id += 1);
    assert_hash_changes!("adapter_address", |e: &mut MarketOpsEnvelope| e
        .adapter_address[0] ^=
        0x01);
    assert_hash_changes!("vault_address", |e: &mut MarketOpsEnvelope| e
        .vault_address[0] ^=
        0x01);
    assert_hash_changes!("mint_controller_address", |e: &mut MarketOpsEnvelope| e
        .mint_controller_address[0] ^=
        0x01);
    assert_hash_changes!("asset_id", |e: &mut MarketOpsEnvelope| e.asset_id[0] ^=
        0x01);
    assert_hash_changes!("epoch", |e: &mut MarketOpsEnvelope| e.epoch += 1);
    assert_hash_changes!("program_id", |e: &mut MarketOpsEnvelope| e.program_id[0] ^=
        0x01);
    assert_hash_changes!("policy_hash", |e: &mut MarketOpsEnvelope| e.policy_hash
        [0] ^= 0x01);
    assert_hash_changes!("parameter_hash", |e: &mut MarketOpsEnvelope| e
        .parameter_hash[0] ^=
        0x01);
    assert_hash_changes!("reserve_packet_hash", |e: &mut MarketOpsEnvelope| e
        .reserve_packet_hash[0] ^=
        0x01);
    assert_hash_changes!("supply_packet_hash", |e: &mut MarketOpsEnvelope| e
        .supply_packet_hash[0] ^=
        0x01);
    assert_hash_changes!("evidence_root", |e: &mut MarketOpsEnvelope| e
        .evidence_root[0] ^=
        0x01);
    assert_hash_changes!("previous_market_state_hash", |e: &mut MarketOpsEnvelope| {
        e.previous_market_state_hash[0] ^= 0x01
    });
    assert_hash_changes!("venue_id", |e: &mut MarketOpsEnvelope| e.venue_id[0] ^=
        0x01);
    assert_hash_changes!("pool_config_hash", |e: &mut MarketOpsEnvelope| e
        .pool_config_hash[0] ^=
        0x01);
    assert_hash_changes!("hook_code_hash", |e: &mut MarketOpsEnvelope| e
        .hook_code_hash[0] ^=
        0x01);
    assert_hash_changes!("nav_floor_usd_e8", |e: &mut MarketOpsEnvelope| e
        .nav_floor_usd_e8 +=
        1);
    assert_hash_changes!("valid_global_supply_atoms", |e: &mut MarketOpsEnvelope| {
        e.valid_global_supply_atoms += 1
    });
    assert_hash_changes!("verified_net_assets_usd_e8", |e: &mut MarketOpsEnvelope| {
        e.verified_net_assets_usd_e8 += 1
    });
    assert_hash_changes!(
        "funded_alignment_reserve_usd_e8",
        |e: &mut MarketOpsEnvelope| e.funded_alignment_reserve_usd_e8 += 1
    );
    assert_hash_changes!(
        "required_alignment_reserve_usd_e8",
        |e: &mut MarketOpsEnvelope| e.required_alignment_reserve_usd_e8 += 1
    );
    assert_hash_changes!("max_reserve_deploy_usd_e8", |e: &mut MarketOpsEnvelope| {
        e.max_reserve_deploy_usd_e8 += 1
    });
    assert_hash_changes!("max_mint_atoms", |e: &mut MarketOpsEnvelope| e
        .max_mint_atoms +=
        1);
    assert_hash_changes!("discount_trigger_bps", |e: &mut MarketOpsEnvelope| e
        .discount_trigger_bps +=
        1);
    assert_hash_changes!("premium_trigger_bps", |e: &mut MarketOpsEnvelope| e
        .premium_trigger_bps +=
        1);
    assert_hash_changes!("data_window_start", |e: &mut MarketOpsEnvelope| e
        .data_window_start +=
        1);
    assert_hash_changes!("data_window_end", |e: &mut MarketOpsEnvelope| e
        .data_window_end +=
        1);
    assert_hash_changes!("valid_after", |e: &mut MarketOpsEnvelope| e.valid_after +=
        1);
    assert_hash_changes!("expires_at", |e: &mut MarketOpsEnvelope| e.expires_at += 1);
    assert_hash_changes!("cooldown_seconds", |e: &mut MarketOpsEnvelope| e
        .cooldown_seconds +=
        1);
    assert_hash_changes!("nonce", |e: &mut MarketOpsEnvelope| e.nonce[0] ^= 0x01);
}

#[test]
fn transaction_batch_omits_empty_payment_v2_for_legacy_serialization() {
    let batch = TransactionBatch::new("batch-1", Vec::new());
    let json = serde_json::to_string(&batch).expect("serialize batch");
    assert_eq!(r#"{"batch_id":"batch-1","transactions":[]}"#, json);
    let parsed: TransactionBatch = serde_json::from_str(&json).expect("parse batch");
    assert!(parsed.payments_v2.is_empty());
    assert!(parsed.asset_transactions.is_empty());
    assert!(parsed.escrow_transactions.is_empty());
    assert!(parsed.nft_transactions.is_empty());
    assert_eq!(0, parsed.transaction_count());
}

#[test]
fn transaction_batch_counts_asset_transactions() {
    let signed = SignedAssetTransaction {
        unsigned: asset_create_unsigned(),
        algorithm_id: "ML-DSA-65".to_string(),
        public_key_hex: "aa".to_string(),
        signature_hex: "bb".to_string(),
    };
    let batch = TransactionBatch::new_with_asset_transactions(
        "batch-assets",
        Vec::new(),
        Vec::new(),
        vec![signed],
    );
    assert_eq!(1, batch.transaction_count());
    assert!(!batch.is_empty());
    let json = serde_json::to_string(&batch).expect("serialize asset batch");
    assert!(json.contains(r#""asset_transactions""#));
    let parsed: TransactionBatch = serde_json::from_str(&json).expect("parse asset batch");
    assert_eq!(1, parsed.asset_transactions.len());
}

#[test]
fn transaction_batch_counts_escrow_transactions() {
    let signed = SignedEscrowTransaction {
        unsigned: escrow_create_unsigned(),
        algorithm_id: "ML-DSA-65".to_string(),
        public_key_hex: "aa".to_string(),
        signature_hex: "bb".to_string(),
    };
    let batch = TransactionBatch::new_with_escrow_transactions(
        "batch-escrows",
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![signed],
    );
    assert_eq!(1, batch.transaction_count());
    assert!(!batch.is_empty());
    let json = serde_json::to_string(&batch).expect("serialize escrow batch");
    assert!(json.contains(r#""escrow_transactions""#));
    let parsed: TransactionBatch = serde_json::from_str(&json).expect("parse escrow batch");
    assert_eq!(1, parsed.escrow_transactions.len());
}

#[test]
fn transaction_batch_counts_nft_transactions() {
    let signed = SignedNftTransaction {
        unsigned: nft_mint_unsigned(),
        algorithm_id: "ML-DSA-65".to_string(),
        public_key_hex: "aa".to_string(),
        signature_hex: "bb".to_string(),
    };
    let batch = TransactionBatch::new_with_nft_transactions(
        "batch-nfts",
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![signed],
    );
    assert_eq!(1, batch.transaction_count());
    assert!(!batch.is_empty());
    let json = serde_json::to_string(&batch).expect("serialize nft batch");
    assert!(json.contains(r#""nft_transactions""#));
    let parsed: TransactionBatch = serde_json::from_str(&json).expect("parse nft batch");
    assert_eq!(1, parsed.nft_transactions.len());
}

#[test]
fn transaction_batch_counts_offer_transactions() {
    let signed = SignedOfferTransaction {
        unsigned: offer_create_unsigned(),
        algorithm_id: "ML-DSA-65".to_string(),
        public_key_hex: "aa".to_string(),
        signature_hex: "bb".to_string(),
    };
    let batch = TransactionBatch::new_with_offer_transactions(
        "batch-offers",
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![signed],
    );
    assert_eq!(1, batch.transaction_count());
    assert!(!batch.is_empty());
    let json = serde_json::to_string(&batch).expect("serialize offer batch");
    assert!(json.contains(r#""offer_transactions""#));
    let parsed: TransactionBatch = serde_json::from_str(&json).expect("parse offer batch");
    assert_eq!(1, parsed.offer_transactions.len());
}

#[test]
fn mempool_state_omits_empty_payment_v2_for_legacy_serialization() {
    let mempool = MempoolState::empty();
    let json = serde_json::to_string(&mempool).expect("serialize mempool");
    assert_eq!(r#"{"pending":[]}"#, json);
    let parsed: MempoolState = serde_json::from_str(r#"{"pending":[]}"#).expect("parse");
    assert!(parsed.is_empty());
    assert!(parsed.pending_nft_transactions.is_empty());
    assert!(parsed.pending_offer_transactions.is_empty());

    let signed = SignedOfferTransaction {
        unsigned: offer_create_unsigned(),
        algorithm_id: "ML-DSA-65".to_string(),
        public_key_hex: "aa".to_string(),
        signature_hex: "bb".to_string(),
    };
    let mut mempool = MempoolState::empty();
    mempool
        .pending_offer_transactions
        .push(MempoolOfferTransactionEntry::new("offer-tx", signed));
    assert_eq!(1, mempool.len());
    assert!(mempool.has_sender_sequence("pfowner0000000000000000000000000000000000", 11));
}

#[test]
fn node_state_round_trip() {
    let mut state = NodeState::initialized("validator-0");
    state.mark_running();
    let json = state.to_json().expect("serialize state");
    let parsed = NodeState::from_json(&json).expect("parse state");
    assert_eq!(state, parsed);
}

#[test]
fn nav_reserve_collateralization_accepts_over_collateralized_floor_nav() {
    let verified_net_assets = 2_032_945_386_170u64;
    let circulating_supply = 4_000u64;
    let nav_per_unit = nav_per_unit_floor(verified_net_assets, circulating_supply).expect("floor");
    assert_eq!(nav_per_unit, 508_236_346);
    assert!(validate_nav_reserve_collateralization(
        verified_net_assets,
        circulating_supply,
        nav_per_unit
    )
    .is_ok());
    let collateralized = circulating_supply * nav_per_unit;
    assert!(verified_net_assets > collateralized);
}

#[test]
fn nav_reserve_collateralization_rejects_under_collateralized_nav() {
    let verified_net_assets = 2_032_945_386_170u64;
    let circulating_supply = 4_000u64;
    let nav_per_unit =
        nav_per_unit_floor(verified_net_assets, circulating_supply).expect("floor") + 1;
    assert!(validate_nav_reserve_collateralization(
        verified_net_assets,
        circulating_supply,
        nav_per_unit
    )
    .is_err());
}

#[test]
fn nav_reserve_collateralization_accepts_vault_bridge_atom_scaled_unit_nav() {
    assert!(
        validate_nav_reserve_collateralization(5_000_000, 5_000_000, VAULT_BRIDGE_UNIT).is_ok()
    );
    assert!(
        validate_nav_reserve_collateralization(4_999_999, 5_000_000, VAULT_BRIDGE_UNIT).is_err()
    );
}

#[test]
fn nav_reserve_collateralization_accepts_precision_scaled_fractional_nav() {
    assert_eq!(
        nav_per_unit_floor_with_unit_scale(5_000_005, 5_000_000, 1_000_000).expect("scaled nav"),
        1_000_001
    );
    assert!(validate_nav_reserve_collateralization_with_unit_scale(
        5_000_005,
        5_000_000,
        VAULT_BRIDGE_UNIT + 1,
        u128::from(VAULT_BRIDGE_UNIT),
    )
    .is_ok());
    assert!(validate_nav_reserve_collateralization_with_unit_scale(
        5_000_004,
        5_000_000,
        VAULT_BRIDGE_UNIT + 1,
        u128::from(VAULT_BRIDGE_UNIT),
    )
    .is_err());
}

#[test]
fn nav_redemption_claim_accepts_precision_scaled_fractional_nav() {
    let chain_id = "postfiat-local";
    let redemption = NavRedemption::new_with_unit_scale(
        chain_id,
        "pfowner0000000000000000000000000000000000",
        "issuer",
        "aa".repeat(48),
        7,
        500_000,
        3,
        696_184_909,
        1_000_000,
        "bb".repeat(48),
    )
    .expect("scaled redemption");
    assert_eq!(redemption.unit_scale, 1_000_000);
    assert_eq!(redemption.redemption_claim, 348_092_455);
    redemption
        .validate_for_chain(chain_id)
        .expect("valid scaled redemption");
}

#[test]
fn nav_reserve_packet_accepts_over_collateralized_floor_nav() {
    let verified_net_assets = 2_032_945_386_170u64;
    let circulating_supply = 4_000u64;
    let nav_per_unit = nav_per_unit_floor(verified_net_assets, circulating_supply).expect("floor");
    let packet = NavReservePacket::new(
        "aa".repeat(48),
        "issuer",
        "submitter",
        2,
        nav_per_unit,
        circulating_supply,
        verified_net_assets,
        "profile",
        "bb".repeat(48),
        "cc".repeat(48),
        "dd".repeat(48),
    )
    .expect("packet");
    assert_eq!(packet.nav_per_unit, nav_per_unit);
}

#[test]
fn vault_bridge_receipt_bucket_and_allocation_ids_are_deterministic() {
    assert_eq!(
        vault_bridge_route_binding(&"11".repeat(48), 7).expect("route binding"),
        "bceb5f7d7b32245250a394adb9f4a29c83e8806f805d6427caa4e055aa17473a"
    );
    assert_ne!(
        vault_bridge_route_binding(&"12".repeat(48), 7).expect("changed route binding"),
        vault_bridge_route_binding(&"11".repeat(48), 7).expect("baseline route binding")
    );
    assert!(vault_bridge_route_binding(&"11".repeat(48), 0).is_err());
    let solidity_vector = VaultBridgeDepositEvidence {
        source_chain_id: 42_161,
        vault_address: "0x1455bd7fbfbf92a171ef36025e13959e3b0ad8c0".to_string(),
        token_address: "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        depositor: "0x1111111111111111111111111111111111111111".to_string(),
        pftl_recipient: "unused-for-deposit-id".to_string(),
        pftl_recipient_hash: "66".repeat(32),
        amount_atoms: 10_000_099,
        nonce: "22".repeat(32),
        route_binding: vault_bridge_route_binding(&"11".repeat(48), 7).expect("route binding"),
        deposit_id: String::new(),
        block_hash: "00".repeat(32),
        tx_hash: "00".repeat(32),
        log_index: 0,
    };
    assert_eq!(
        vault_bridge_deposit_id(&solidity_vector).expect("v2 Solidity ABI deposit id"),
        "319368bc7bae3f5806a0a0ba8920f5b6c7e865df6ea460c9e8b291a32fc74dee"
    );
    let chain_id = "postfiat-local";
    let asset_id = "aa".repeat(48);
    let policy_hash = "42".repeat(48);
    let pftl_recipient = "bridge-recipient-000000000000000000000000".to_string();
    let pftl_recipient_hash =
        vault_bridge_pftl_recipient_hash(&pftl_recipient).expect("recipient hash");
    let mut evidence = VaultBridgeDepositEvidence {
        source_chain_id: 42_161,
        vault_address: "0x1455bd7fbfbf92a171ef36025e13959e3b0ad8c0".to_string(),
        token_address: "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        depositor: "0x1111111111111111111111111111111111111111".to_string(),
        pftl_recipient,
        pftl_recipient_hash,
        amount_atoms: 10_000_099,
        nonce: "22".repeat(32),
        route_binding: String::new(),
        deposit_id: "33".repeat(32),
        block_hash: "44".repeat(32),
        tx_hash: "55".repeat(32),
        log_index: 7,
    };
    evidence.deposit_id = vault_bridge_deposit_id(&evidence).expect("deposit id");
    let source_domain = evidence.source_domain();
    let receipt = VaultBridgeReceipt::new(
        chain_id,
        asset_id.clone(),
        source_domain.clone(),
        evidence.source_asset_ref(),
        VAULT_BRIDGE_CLAIM_TYPE_BRIDGE_DEPOSIT,
        10_000_099,
        evidence.source_tx_or_attestation(),
        evidence.finality_ref(),
        evidence.vault_id(),
        policy_hash.clone(),
        10,
        1_000,
        Some(evidence.clone()),
    )
    .expect("receipt");
    assert_eq!(
        vault_bridge_deposit_evidence_root(&evidence)
            .expect("evidence root")
            .len(),
        96
    );
    assert_eq!(
        receipt.receipt_id,
        vault_bridge_receipt_id(
            chain_id,
            &asset_id,
            &source_domain,
            &evidence.source_tx_or_attestation(),
            &evidence.finality_ref(),
            10_000_099,
            &policy_hash,
        )
        .expect("receipt id")
    );
    assert_eq!(
        receipt.bucket_id,
        vault_bridge_bucket_id(&asset_id, &source_domain, &policy_hash).expect("bucket id")
    );
    let sp1_policy_hash = "24".repeat(NAV_SP1_POLICY_HASH_HEX_LEN / 2);
    assert!(vault_bridge_bucket_id(&asset_id, &source_domain, &sp1_policy_hash).is_ok());
    assert!(vault_bridge_receipt_id(
        chain_id,
        &asset_id,
        &source_domain,
        &evidence.source_tx_or_attestation(),
        &evidence.finality_ref(),
        10_000_099,
        &sp1_policy_hash,
    )
    .is_ok());
    assert!(vault_bridge_bucket_id(&asset_id, &source_domain, &"24".repeat(31)).is_err());
    assert_eq!(
        vault_bridge_deposit_public_values_hash(
            &evidence,
            &vault_bridge_deposit_evidence_root(&evidence).expect("evidence root"),
            &sp1_policy_hash,
        )
        .expect("public values hash")
        .len(),
        96
    );

    let changed = vault_bridge_receipt_id(
        chain_id,
        &asset_id,
        &source_domain,
        "erc20_bridge_deposit:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        &evidence.finality_ref(),
        10_000_099,
        &policy_hash,
    )
    .expect("changed receipt id");
    assert_ne!(receipt.receipt_id, changed);

    let allocation = VaultBridgeAllocation::new(
        chain_id,
        receipt.receipt_id.clone(),
        asset_id.clone(),
        receipt.bucket_id.clone(),
        5_000_000,
        VAULT_BRIDGE_ALLOCATION_PURPOSE_SUPPLY,
        "vault_bridge_supply:7:0",
        12,
    )
    .expect("allocation");
    assert_eq!(
        allocation.allocation_id,
        vault_bridge_allocation_id(
            chain_id,
            &receipt.receipt_id,
            &asset_id,
            &receipt.bucket_id,
            5_000_000,
            VAULT_BRIDGE_ALLOCATION_PURPOSE_SUPPLY,
            "vault_bridge_supply:7:0",
        )
        .expect("allocation id")
    );
}

#[test]
fn pftl_uniswap_return_burn_id_binds_burn_height() {
    let native_nav_asset_id = "65".repeat(48);
    let burn_id = pftl_uniswap_return_burn_id_from_fields(
        1,
        "0x1111111111111111111111111111111111111111",
        "0x3333333333333333333333333333333333333333",
        &native_nav_asset_id,
        "0x5555555555555555555555555555555555555555",
        "pf124071fd53a12ca4556b7aa1f5ec98b585e73468",
        40,
        &"ab".repeat(32),
        20,
    )
    .expect("return burn id");
    let different_height = pftl_uniswap_return_burn_id_from_fields(
        1,
        "0x1111111111111111111111111111111111111111",
        "0x3333333333333333333333333333333333333333",
        &native_nav_asset_id,
        "0x5555555555555555555555555555555555555555",
        "pf124071fd53a12ca4556b7aa1f5ec98b585e73468",
        40,
        &"ab".repeat(32),
        21,
    )
    .expect("return burn id");
    assert_eq!(burn_id.len(), 64);
    assert_ne!(burn_id, different_height);
}

#[test]
fn pftl_uniswap_non_consumption_proof_hash_binds_refund_height() {
    let packet_hash = "66".repeat(48);
    let commitment = pftl_uniswap_non_consumption_proof_hash("pftl-uniswap-a651", &packet_hash, 13)
        .expect("commitment");
    let different_height =
        pftl_uniswap_non_consumption_proof_hash("pftl-uniswap-a651", &packet_hash, 14)
            .expect("commitment");
    assert_eq!(commitment.len(), 96);
    assert_ne!(commitment, different_height);
}

#[test]
fn legacy_vault_bridge_withdrawal_packet_without_domain_binding_loads_but_is_not_valid() {
    let json = format!(
        r#"{{
                "pftl_chain_id": 13299438584480955342,
                "vault_bridge_asset_id": "{asset_id}",
                "burn_tx_id": "{burn_tx_id}",
                "withdrawal_id": "{withdrawal_id}",
                "recipient": "0x1455bd7fbfbf92a171ef36025e13959e3b0ad8c0",
                "amount_atoms": 1,
                "source_bucket_id": "{bucket_id}",
                "destination_hash": "{destination_hash}",
                "finalized_height": 46,
                "evidence_root": "{evidence_root}"
            }}"#,
        asset_id = "aa".repeat(48),
        burn_tx_id = "bb".repeat(48),
        withdrawal_id = "cc".repeat(48),
        bucket_id = "dd".repeat(48),
        destination_hash = "ee".repeat(48),
        evidence_root = "ff".repeat(48),
    );
    let packet: VaultBridgeWithdrawalPacket =
        serde_json::from_str(&json).expect("legacy packet loads");

    assert_eq!(packet.source_chain_id, 0);
    assert!(packet.vault_address.is_empty());
    assert!(packet.token_address.is_empty());
    assert!(packet.is_legacy_domainless());
    assert!(packet.validate().is_err());
    assert!(vault_bridge_withdrawal_packet_hash(&packet).is_err());
    assert!(vault_bridge_withdrawal_packet_evm_digest(&packet).is_err());
}

#[test]
fn legacy_vault_bridge_redemption_domainless_packet_validates_as_state_record_only() {
    let chain_id = "postfiat-local";
    let issuer = "pfissuer000000000000000000000000000000000";
    let owner = "pfholder00000000000000000000000000000000";
    let asset = AssetDefinition::new(chain_id, issuer, "USDC", 1, 6).expect("asset");
    let source_domain = "erc20_bridge_vault:42161:0x1111111111111111111111111111111111111111:0x2222222222222222222222222222222222222222";
    let bucket =
        VaultBridgeBucketState::new(asset.asset_id.clone(), source_domain, "11".repeat(48), 10)
            .expect("bucket");
    let mut redemption = VaultBridgeRedemption::new(
        chain_id,
        owner,
        issuer,
        asset.asset_id.clone(),
        bucket.bucket_id.clone(),
        source_domain,
        7,
        1_000_000,
        1,
        "22".repeat(48),
        "evm-erc20:42161:0x3333333333333333333333333333333333333333",
        "44".repeat(48),
        11,
    )
    .expect("redemption");

    redemption.withdrawal_packet.source_chain_id = 0;
    redemption.withdrawal_packet.vault_address.clear();
    redemption.withdrawal_packet.token_address.clear();
    redemption.withdrawal_packet_hash = "55".repeat(48);
    redemption.withdrawal_packet_evm_digest = "66".repeat(32);

    assert!(redemption.withdrawal_packet.is_legacy_domainless());
    assert!(redemption.withdrawal_packet.validate().is_err());
    assert!(vault_bridge_withdrawal_packet_hash(&redemption.withdrawal_packet).is_err());
    assert!(vault_bridge_withdrawal_packet_evm_digest(&redemption.withdrawal_packet).is_err());
    redemption
        .validate_for_chain(chain_id)
        .expect("legacy record remains state-valid");

    let mut ledger = LedgerState::new_with_assets(Vec::new(), vec![asset], Vec::new());
    ledger.vault_bridge_bucket_states.push(bucket);
    ledger.vault_bridge_redemptions.push(redemption);
    ledger
        .validate_asset_state(chain_id)
        .expect("legacy redemption must not brick unrelated state transitions");
}

#[test]
fn vault_bridge_withdrawal_packet_evm_digest_matches_solidity_abi_vector() {
    let packet = VaultBridgeWithdrawalPacket {
        pftl_chain_id: 65_100,
        source_chain_id: 42_161,
        vault_address: "0x1111111111111111111111111111111111111111".to_string(),
        token_address: "0x3333333333333333333333333333333333333333".to_string(),
        vault_bridge_asset_id: "aa".repeat(48),
        burn_tx_id: "bb".repeat(48),
        withdrawal_id: "cc".repeat(48),
        recipient: "0x2222222222222222222222222222222222222222".to_string(),
        amount_atoms: 1_000_000,
        source_bucket_id: "dd".repeat(48),
        destination_hash: "ee".repeat(48),
        finalized_height: 77,
        evidence_root: "11".repeat(48),
    };
    let digest = vault_bridge_withdrawal_packet_evm_digest(&packet).expect("evm digest");
    assert_eq!(
        digest,
        "faf77ea9f7590b08fdaa1ce11263a0d952781118a867e0dbfe99c34e31c8e0c3"
    );
}

#[test]
fn nav_mint_vault_bridge_settlement_signing_bytes_are_optional_and_committed() {
    let legacy = NavMintAtNavOperation {
        issuer: "pfissuer000000000000000000000000000000000".to_string(),
        to: "pfsubscriber000000000000000000000000000000".to_string(),
        asset_id: "aa".repeat(48),
        amount: 5,
        epoch: 1,
        reserve_packet_hash: "bb".repeat(48),
        settlement_asset_id: String::new(),
        settlement_bucket_id: String::new(),
        settlement_allocation_id: String::new(),
        settlement_amount_atoms: 0,
    };
    legacy.validate().expect("legacy mint validates");
    assert!(!legacy.has_vault_bridge_settlement());
    let legacy_bytes = String::from_utf8(legacy.signing_bytes()).expect("legacy bytes");
    assert!(!legacy_bytes.contains("settlement_asset_id"));

    let mut settled = legacy.clone();
    settled.settlement_asset_id = "cc".repeat(48);
    settled.settlement_bucket_id = "dd".repeat(48);
    settled.settlement_allocation_id = "ee".repeat(48);
    settled.settlement_amount_atoms = 5_000_000;
    settled.validate().expect("settled mint validates");
    assert!(settled.has_vault_bridge_settlement());
    let settled_bytes = String::from_utf8(settled.signing_bytes()).expect("settled bytes");
    assert_ne!(legacy_bytes, settled_bytes);
    assert!(settled_bytes.contains("settlement_asset_id="));
    assert!(settled_bytes.contains("settlement_amount_atoms=5000000"));
}

#[test]
fn vault_bridge_nav_subscription_allocate_operation_commits_kind_and_source() {
    let operation = VaultBridgeNavSubscriptionAllocateOperation {
        operator: "pfissuer000000000000000000000000000000000".to_string(),
        nav_asset_id: "aa".repeat(48),
        settlement_asset_id: "bb".repeat(48),
        settlement_bucket_id: "cc".repeat(48),
        settlement_receipt_id: "dd".repeat(48),
        settlement_amount_atoms: 5_000_000,
        consume_supply_owner: None,
        consume_supply_allocation_id: None,
        nav_recipient: None,
        subscription_id: None,
    };
    operation.validate().expect("operation validates");
    let asset_operation = AssetTransactionOperation::VaultBridgeNavSubscriptionAllocate(operation);
    assert_eq!(
        asset_operation.transaction_kind(),
        VAULT_BRIDGE_NAV_SUBSCRIPTION_ALLOCATE_TRANSACTION_KIND
    );
    assert!(asset_operation.source_matches("pfissuer000000000000000000000000000000000"));
    let signing_bytes = String::from_utf8(asset_operation.signing_bytes()).expect("signing bytes");
    assert!(signing_bytes.contains("operation=vault_bridge_nav_subscription_allocate"));
    assert!(signing_bytes.contains("settlement_amount_atoms=5000000"));

    let consume_supply_operation = AssetTransactionOperation::VaultBridgeNavSubscriptionAllocate(
        VaultBridgeNavSubscriptionAllocateOperation {
            operator: "pfissuer000000000000000000000000000000000".to_string(),
            nav_asset_id: "aa".repeat(48),
            settlement_asset_id: "bb".repeat(48),
            settlement_bucket_id: "cc".repeat(48),
            settlement_receipt_id: "dd".repeat(48),
            settlement_amount_atoms: 5_000_000,
            consume_supply_owner: Some("pfholder000000000000000000000000000000000".to_string()),
            consume_supply_allocation_id: Some("ee".repeat(48)),
            nav_recipient: Some("pfholder000000000000000000000000000000000".to_string()),
            subscription_id: Some("navsub-0001".to_string()),
        },
    );
    consume_supply_operation
        .validate()
        .expect("consume supply operation validates");
    assert!(consume_supply_operation.source_matches("pfholder000000000000000000000000000000000"));
    assert!(!consume_supply_operation.source_matches("pfissuer000000000000000000000000000000000"));
    let consume_signing_bytes =
        String::from_utf8(consume_supply_operation.signing_bytes()).expect("consume signing bytes");
    assert!(consume_signing_bytes.contains("consume_supply_owner="));
    assert!(consume_signing_bytes.contains("consume_supply_allocation_id="));
    assert!(consume_signing_bytes.contains("nav_recipient="));
    assert!(consume_signing_bytes.contains("subscription_id=navsub-0001"));
}

#[test]
fn vault_bridge_source_root_is_sorted_and_commits_bucket_fields() {
    let asset_id = "aa".repeat(48);
    let policy_hash = "42".repeat(48);
    let mut left = VaultBridgeBucketState::new(
        asset_id.clone(),
        "erc20_bridge_primary_source",
        policy_hash.clone(),
        10,
    )
    .expect("left bucket");
    left.gross_receipt_atoms = 10_000_099;
    left.counted_value_atoms = 9_975_098;
    let mut right = VaultBridgeBucketState::new(
        asset_id.clone(),
        "erc20_bridge_secondary_source",
        policy_hash,
        11,
    )
    .expect("right bucket");
    right.gross_receipt_atoms = 1_000_000;
    right.counted_value_atoms = 1_000_000;

    let sorted_root = vault_bridge_source_root_for_asset(&[left.clone(), right.clone()], &asset_id)
        .expect("sorted root");
    let reversed_root = vault_bridge_source_root_for_asset(&[right, left.clone()], &asset_id)
        .expect("reversed root");
    assert_eq!(sorted_root, reversed_root);

    left.outstanding_vault_bridge_atoms = 5_000_000;
    let changed_root =
        vault_bridge_source_root_for_asset(&[left], &asset_id).expect("changed root");
    assert_ne!(sorted_root, changed_root);
}

#[test]
fn vault_bridge_bucket_impairment_factor_is_deterministic() {
    let asset_id = "aa".repeat(48);
    let policy_hash = "42".repeat(48);
    let mut bucket =
        VaultBridgeBucketState::new(asset_id, "erc20_bridge_primary_source", policy_hash, 10)
            .expect("bucket");
    bucket.counted_value_atoms = 3_000_000;
    bucket.outstanding_vault_bridge_atoms = 4_000_000;
    assert!(bucket.validate().is_err());

    bucket.status = VAULT_BRIDGE_BUCKET_STATUS_IMPAIRED.to_string();
    bucket.impairment_factor_bps = 7_501;
    assert!(bucket.validate().is_err());

    bucket.impairment_factor_bps = 7_500;
    bucket
        .validate()
        .expect("impaired bucket with exact factor");
}

fn pfusdc_ingress_public_values_fixture() -> PfUsdcIngressPublicValuesV3 {
    let mut values = PfUsdcIngressPublicValuesV3 {
        schema: PFUSDC_INGRESS_PUBLIC_VALUES_SCHEMA_V3.to_string(),
        proof_program_version: 3,
        pftl_chain_id: "postfiat-devnet".to_string(),
        pftl_genesis_hash: "01".repeat(48),
        pftl_protocol_version: 1,
        route_profile_hash: "02".repeat(48),
        route_epoch: 7,
        ethereum_chain_id: 1,
        prior_ethereum_finalized_beacon_root: "15".repeat(32),
        prior_ethereum_finalized_slot: 12_344,
        ethereum_finalized_beacon_root: "03".repeat(32),
        ethereum_finalized_slot: 12_345,
        arbitrum_chain_id: 42_161,
        arbitrum_rollup_address: "0x1111111111111111111111111111111111111111".to_string(),
        arbitrum_rollup_runtime_code_hash: "16".repeat(32),
        rollup_latest_confirmed_storage_slot: "17".repeat(32),
        arbitrum_assertion_hash: "04".repeat(32),
        assertion_l2_block_hash: "05".repeat(32),
        assertion_l2_state_root: "18".repeat(32),
        assertion_send_root: "06".repeat(32),
        output_index: 3,
        output_item_hash: "07".repeat(32),
        output_l2_block_number: 98_765,
        output_l1_block_number: 22_345_678,
        output_timestamp: 1_700_000_000,
        output_sender: "0x2222222222222222222222222222222222222222".to_string(),
        output_destination: "0x5555555555555555555555555555555555555555".to_string(),
        ingress_anchor_runtime_code_hash: "19".repeat(32),
        output_calldata_hash: "08".repeat(32),
        vault_address: "0x2222222222222222222222222222222222222222".to_string(),
        vault_runtime_code_hash: "09".repeat(32),
        token_address: "0x3333333333333333333333333333333333333333".to_string(),
        token_runtime_code_hash: "0a".repeat(32),
        depositor: "0x4444444444444444444444444444444444444444".to_string(),
        pftl_recipient: "pfrecipient000000000000000000000000000000000".to_string(),
        pftl_recipient_hash: "0b".repeat(32),
        amount_atoms: 1_000_000,
        nonce: "0c".repeat(32),
        route_binding: "0d".repeat(32),
        deposit_id: "0e".repeat(32),
        evidence_root: "0f".repeat(48),
        public_values_commitment: String::new(),
    };
    values.seal().expect("seal ingress fixture");
    values
}

#[test]
fn pfusdc_ingress_public_values_round_trip_is_canonical_and_strict() {
    let values = pfusdc_ingress_public_values_fixture();
    let bytes = values
        .canonical_bytes_without_commitment()
        .expect("canonical ingress bytes");
    let decoded = PfUsdcIngressPublicValuesV3::from_canonical_bytes(&bytes)
        .expect("strict ingress decode");
    assert_eq!(decoded, values);

    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(PfUsdcIngressPublicValuesV3::from_canonical_bytes(&trailing).is_err());

    let mut wrong_tag = bytes.clone();
    let first_tag = b"PFTL-PFUSDC-TIER4".len()
        + 4
        + PFUSDC_INGRESS_PUBLIC_VALUES_SCHEMA_V3.len();
    wrong_tag[first_tag + 1] = 2;
    assert!(PfUsdcIngressPublicValuesV3::from_canonical_bytes(&wrong_tag).is_err());
}

#[test]
fn pfusdc_ingress_commitment_changes_for_each_field_class() {
    let original = pfusdc_ingress_public_values_fixture();
    let commitment = original.expected_commitment().expect("original commitment");
    let mut mutations = Vec::new();

    let mut value = original.clone();
    value.route_epoch += 1;
    mutations.push(value);
    let mut value = original.clone();
    value.ethereum_finalized_beacon_root = "10".repeat(32);
    mutations.push(value);
    let mut value = original.clone();
    value.assertion_send_root = "11".repeat(32);
    mutations.push(value);
    let mut value = original.clone();
    value.arbitrum_rollup_runtime_code_hash = "1a".repeat(32);
    mutations.push(value);
    let mut value = original.clone();
    value.rollup_latest_confirmed_storage_slot = "1b".repeat(32);
    mutations.push(value);
    let mut value = original.clone();
    value.assertion_l2_state_root = "1c".repeat(32);
    mutations.push(value);
    let mut value = original.clone();
    value.output_sender = "0x6666666666666666666666666666666666666666".to_string();
    mutations.push(value);
    let mut value = original.clone();
    value.ingress_anchor_runtime_code_hash = "1d".repeat(32);
    mutations.push(value);
    let mut value = original.clone();
    value.vault_address = "0x5555555555555555555555555555555555555555".to_string();
    mutations.push(value);
    let mut value = original.clone();
    value.output_item_hash = "12".repeat(32);
    mutations.push(value);
    let mut value = original.clone();
    value.amount_atoms += 1;
    mutations.push(value);
    let mut value = original.clone();
    value.pftl_recipient.push('1');
    mutations.push(value);
    let mut value = original.clone();
    value.route_binding = "13".repeat(32);
    mutations.push(value);
    let mut value = original.clone();
    value.evidence_root = "14".repeat(48);
    mutations.push(value);

    for mut value in mutations {
        value.public_values_commitment.clear();
        assert_ne!(
            value.expected_commitment().expect("mutated commitment"),
            commitment
        );
    }
}

#[test]
fn pfusdc_finality_state_requires_retained_ancestry_and_monotonic_advance() {
    let values = pfusdc_ingress_public_values_fixture();
    let initial = EthereumArbitrumCheckpointV1 {
        ethereum_finalized_beacon_root: values
            .prior_ethereum_finalized_beacon_root
            .clone(),
        ethereum_finalized_slot: values.prior_ethereum_finalized_slot,
        arbitrum_assertion_hash: "20".repeat(32),
        assertion_l2_block_hash: "21".repeat(32),
        assertion_send_root: "22".repeat(32),
    };
    let mut state = EthereumArbitrumFinalityStateV2 {
        schema: ETHEREUM_ARBITRUM_FINALITY_STATE_SCHEMA_V2.to_string(),
        route_profile_hash: values.route_profile_hash.clone(),
        route_epoch: values.route_epoch,
        ethereum_chain_id: values.ethereum_chain_id,
        arbitrum_chain_id: values.arbitrum_chain_id,
        arbitrum_rollup_address: values.arbitrum_rollup_address.clone(),
        arbitrum_rollup_runtime_code_hash: format!(
            "0x{}",
            values.arbitrum_rollup_runtime_code_hash
        ),
        rollup_latest_confirmed_storage_slot: values
            .rollup_latest_confirmed_storage_slot
            .clone(),
        vault_address: values.vault_address.clone(),
        vault_runtime_code_hash: format!("0x{}", values.vault_runtime_code_hash),
        token_address: values.token_address.clone(),
        token_runtime_code_hash: format!("0x{}", values.token_runtime_code_hash),
        ethereum_ingress_anchor_address: values.output_destination.clone(),
        ethereum_ingress_anchor_runtime_code_hash: format!(
            "0x{}",
            values.ingress_anchor_runtime_code_hash
        ),
        latest: initial.clone(),
        retained: vec![initial],
    };
    let before = state
        .state_commitment_bytes()
        .expect("initial state commitment");
    state
        .verify_and_advance(&values)
        .expect("valid finality advance");
    assert_eq!(state.latest.ethereum_finalized_slot, values.ethereum_finalized_slot);
    assert_ne!(
        state
            .state_commitment_bytes()
            .expect("advanced state commitment"),
        before
    );

    // An idempotent proof of the already-retained result remains admissible.
    state
        .verify_and_advance(&values)
        .expect("retained checkpoint replay is idempotent");

    let mut unknown_parent = values.clone();
    unknown_parent.prior_ethereum_finalized_beacon_root = "25".repeat(32);
    assert!(state.verify_and_advance(&unknown_parent).is_err());

    let mut conflict = values.clone();
    conflict.assertion_send_root = "26".repeat(32);
    assert!(state.verify_and_advance(&conflict).is_err());

    let mut route_mutations = Vec::new();
    let mut value = values.clone();
    value.arbitrum_rollup_runtime_code_hash = "27".repeat(32);
    route_mutations.push(value);
    let mut value = values.clone();
    value.rollup_latest_confirmed_storage_slot = "28".repeat(32);
    route_mutations.push(value);
    let mut value = values.clone();
    value.vault_address = "0x6666666666666666666666666666666666666666".to_string();
    route_mutations.push(value);
    let mut value = values.clone();
    value.output_sender = "0x6666666666666666666666666666666666666666".to_string();
    route_mutations.push(value);
    let mut value = values.clone();
    value.vault_runtime_code_hash = "29".repeat(32);
    route_mutations.push(value);
    let mut value = values.clone();
    value.token_address = "0x7777777777777777777777777777777777777777".to_string();
    route_mutations.push(value);
    let mut value = values.clone();
    value.token_runtime_code_hash = "2a".repeat(32);
    route_mutations.push(value);
    let mut value = values.clone();
    value.output_destination = "0x8888888888888888888888888888888888888888".to_string();
    route_mutations.push(value);
    let mut value = values;
    value.ingress_anchor_runtime_code_hash = "2b".repeat(32);
    route_mutations.push(value);
    assert!(route_mutations
        .iter()
        .all(|mutation| state.clone().verify_and_advance(mutation).is_err()));
}
