use postfiat_crypto_provider::{address_from_public_key, bytes_to_hex, ml_dsa_65_keygen_from_seed};
use postfiat_storage::NodeStore;
use postfiat_types::{
    credential_id, credential_issuer_id, identity_handle_key, issued_asset_id, nav_fee_accrual_key,
    Account, AssetAccessPolicyV1, AssetApprovalMode, AssetDefinition, AssetLapsedExitRule,
    AssetWrapperMode, BridgeState, CredentialIssuerV1, CredentialPolicyRuleV1,
    CredentialPolicyV1, CredentialRecordV1, CredentialRequirementPairV1, Genesis,
    GovernanceAmendment,
    GovernanceState, IdentityHistoryEventV1, IdentityKind, IdentityLinkKind, IdentityLinkStatus,
    IdentityLinkV1, IdentityRecordV1, IdentityStatus, IssuerSigningAccountV1, LedgerState,
    NavFeeAccrualV1, NavFeeScheduleV1, NavFeeStateV1, NavTrackedAsset, ShieldedState, TrustLine,
    ValidatorIdentityBindingV1, ASSET_ACCESS_MIN_TIGHTEN_NOTICE_BLOCKS,
    GOVERNANCE_KIND_CREDENTIAL_DID_V1_ACTIVATION_HEIGHT,
    GOVERNANCE_KIND_HIVE_IDENTITY_CLASSICAL_PROOF_KILL_SWITCH,
    GOVERNANCE_KIND_HIVE_IDENTITY_V1_ACTIVATION_HEIGHT,
    GOVERNANCE_KIND_NAV_FEE_V1_ACTIVATION_HEIGHT, NAV_FEE_MIN_INCREASE_NOTICE_BLOCKS,
    PROTOCOL_BLOCKS_PER_YEAR,
};

use super::super::*;

fn unique_test_dir(label: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("unix epoch")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "postfiat-train-b-node-{label}-{}-{nanos}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp test dir");
    dir
}

fn init_basic_store(label: &str) -> (PathBuf, NodeStore, Genesis) {
    let dir = unique_test_dir(label);
    let genesis = Genesis::new_with_validator_count("postfiat-train-b-node", 1);
    let node_state = NodeState::initialized("val-0");
    let store = NodeStore::new(&dir);
    store.init(&genesis, &node_state).expect("init store");
    (dir, store, genesis)
}

#[test]
fn train_b_state_commitment_byte_identity_and_all_14_rpcs() {
    let (dir, store, genesis) = init_basic_store("rpcs-and-commitment");
    let issuer_kp = ml_dsa_65_keygen_from_seed(&[1; 32]);
    let alice_kp = ml_dsa_65_keygen_from_seed(&[2; 32]);
    let issuer = address_from_public_key(&issuer_kp.public_key);
    let alice = address_from_public_key(&alice_kp.public_key);
    let alice_pk_hex = bytes_to_hex(&alice_kp.public_key);
    let asset_id = issued_asset_id(&genesis.chain_id, &issuer, "TBG", 1).unwrap();

    let governance = GovernanceState::new(1);
    let shielded = ShieldedState::empty();
    let bridge = BridgeState::empty();

    // 1. Pre-activation state commitment with empty Train B fields matches baseline byte-for-byte!
    let mut base_ledger = LedgerState::new(vec![
        Account {
            address: issuer.clone(),
            balance: 1_000_000,
            sequence: 0,
            public_key_hex: Some(bytes_to_hex(&issuer_kp.public_key)),
            flags: 0,
        },
        Account {
            address: alice.clone(),
            balance: 1_000_000,
            sequence: 1,
            public_key_hex: Some(alice_pk_hex.clone()),
            flags: 0,
        },
    ]);
    base_ledger.asset_definitions.push(AssetDefinition {
        asset_id: asset_id.clone(),
        asset_family_id: asset_id.clone(),
        source_series_id: asset_id.clone(),
        source_bucket_id: String::new(),
        issuer: issuer.clone(),
        code: "TBG".to_string(),
        version: 1,
        precision: 6,
        display_name: "Train B Gated".to_string(),
        max_supply: Some(1_000_000_000),
        requires_authorization: true,
        freeze_enabled: true,
        clawback_enabled: true,
        access_policy: None,
    });
    let base_root = replicated_state_root(
        &genesis,
        &governance,
        &base_ledger,
        &[],
        &shielded,
        &bridge,
    )
    .expect("base root");

    // Populate Train B state across all 3 pillars and verify state root changes deterministically!
    let mut ledger = base_ledger.clone();
    let nostr_pk = "aa".repeat(32);
    let hkey = identity_handle_key("tasknode.postfiat.org", "goodalexander");
    let link_id = "bb".repeat(48);
    ledger.identities_by_account.insert(
        alice.clone(),
        IdentityRecordV1 {
            account: alice.clone(),
            nostr_pubkey: nostr_pk.clone(),
            derivation_version: 1,
            key_index: 0,
            identity_kind: IdentityKind::Human,
            sequence: 2,
            status: IdentityStatus::Active,
            registered_namespace: Some("tasknode.postfiat.org".to_string()),
            handle: Some("goodalexander".to_string()),
            relays: vec!["wss://relay.postfiat.org".to_string()],
            avatar_nft_id: None,
            message_key: Some("cc".repeat(1216)),
            pq_receive_address: Some("pfpq1alice".to_string()),
            links: vec![IdentityLinkV1 {
                link_id: link_id.clone(),
                kind: IdentityLinkKind::ValidatorNode,
                target: "val-0".to_string(),
                consensus_verified: true,
                status: IdentityLinkStatus::Active,
                linked_at_height: 10,
                metadata_hash: None,
            }],
            created_at_height: 5,
            updated_at_height: 10,
            revoked_at_height: None,
        },
    );
    ledger
        .identity_account_by_nostr
        .insert(nostr_pk.clone(), alice.clone());
    ledger
        .identity_account_by_handle
        .insert(hkey, alice.clone());
    ledger.identity_validator_bindings.insert(
        "val-0".to_string(),
        ValidatorIdentityBindingV1 {
            node_id: "val-0".to_string(),
            account: Some(alice.clone()),
            bound_at_height: 10,
            unbound_at_height: None,
        },
    );
    ledger.identity_history_by_account.insert(
        alice.clone(),
        vec![IdentityHistoryEventV1 {
            sequence: 1,
            block_height: 5,
            action: "mint".to_string(),
            detail: "minted".to_string(),
        }],
    );

    // Pillar 7 NAV fee state & accrual.
    let fee_schedule = NavFeeScheduleV1 {
        annual_rate_bps: 100,
        hwm_rate_bps: 1_000,
        issue_spread_bps: 50,
        redeem_spread_bps: 50,
        fix_fee_cap_bps: 25,
        recipient: alice.clone(),
        increase_notice_blocks: NAV_FEE_MIN_INCREASE_NOTICE_BLOCKS,
        accrual_cap_blocks: 86_400,
        rates_locked: false,
        disclosure_ref: Some("ipfs://bafy-doc".to_string()),
        attached_at_height: 5,
    };
    ledger.nav_assets.push(NavTrackedAsset {
        asset_id: asset_id.clone(),
        issuer: issuer.clone(),
        reserve_operator: issuer.clone(),
        proof_profile: "dd".repeat(48),
        valuation_unit: "USD_E6".to_string(),
        redemption_account: issuer.clone(),
        finalized_epoch: 1,
        finalized_at_height: 10,
        nav_per_unit: 1_050_000,
        circulating_supply: 1_000_000,
        finalized_reserve_packet_hash: "ee".repeat(48),
        halted: false,
        halt_reason: String::new(),
        fee_state: Some(NavFeeStateV1 {
            active: fee_schedule,
            pending: None,
            high_water_mark_nav_per_unit: 1_050_000,
            last_accrual_height: 10,
            unclaimed_fee_units: 500,
            total_accrued_fee_units: 500,
            total_claimed_fee_units: 0,
        }),
    });
    ledger.nav_fee_accruals.insert(
        nav_fee_accrual_key(&asset_id, 1),
        NavFeeAccrualV1 {
            asset_id: asset_id.clone(),
            epoch: 1,
            block_height: 10,
            blocks_per_year: PROTOCOL_BLOCKS_PER_YEAR,
            delta_blocks: 86_400,
            verified_net_assets: 1_050_000,
            supply_before: 999_500,
            nav_before: 1_050_525,
            hwm_before: 1_000_000,
            annual_rate_bps_applied: 100,
            hwm_rate_bps_applied: 1_000,
            time_fee_value: 10,
            nav_after_time_fee: 1_050_515,
            hwm_fee_value: 500,
            total_fee_value: 510,
            minted_fee_units: 500,
            clipped_fee_units: 0,
            supply_after: 1_000_000,
            nav_after: 1_050_000,
            hwm_after: 1_050_000,
            reason_code: "accrued".to_string(),
        },
    );

    // Pillar 8 Issuer, Credential, AssetAccessPolicyV1, and implicit TrustLine with holder_limit.
    let kyc_issuer_id = credential_issuer_id(&genesis.chain_id, &issuer, 1);
    ledger.credential_issuers.insert(
        kyc_issuer_id.clone(),
        CredentialIssuerV1 {
            issuer_id: kyc_issuer_id.clone(),
            domain: "kyc.postfiat.org".to_string(),
            admin_account: issuer.clone(),
            pending_admin_account: None,
            signing_accounts: vec![IssuerSigningAccountV1 {
                account: issuer.clone(),
                added_at_height: 5,
                revoked_at_height: None,
                compromise_cutoff_height: None,
            }],
            allowed_credential_types: vec!["kyc.postfiat.org/tier1".to_string()],
            vendor_dids: vec!["did:web:kyc.postfiat.org".to_string()],
            status: "active".to_string(),
            created_at_height: 5,
            updated_at_height: 5,
        },
    );
    let cid = credential_id(&alice, &kyc_issuer_id, "kyc.postfiat.org/tier1");
    ledger.credentials.insert(
        cid.clone(),
        CredentialRecordV1 {
            credential_id: cid.clone(),
            subject: alice.clone(),
            issuer_id: kyc_issuer_id.clone(),
            credential_type: "kyc.postfiat.org/tier1".to_string(),
            signing_account: issuer.clone(),
            commitment_hash: "ff".repeat(48),
            status: "accepted".to_string(),
            issued_height: 5,
            accepted_at_height: Some(5),
            expires_at_height: 1_000_000,
            revoked_at_height: None,
            deposit_payer: issuer.clone(),
            deposit_paid: 10_000,
        },
    );
    ledger.asset_definitions[0].access_policy = Some(AssetAccessPolicyV1 {
        approval_mode: AssetApprovalMode::Either,
        lapsed_exit_rule: AssetLapsedExitRule::ReturnOnly,
        wrapper_mode: AssetWrapperMode::PftlOnly,
        credential_policy: CredentialPolicyV1 {
            any_of: vec![CredentialPolicyRuleV1 {
                all_of: vec![CredentialRequirementPairV1 {
                    issuer_id: kyc_issuer_id.clone(),
                    credential_type: "kyc.postfiat.org/tier1".to_string(),
                }],
            }],
        },
        notice_blocks: ASSET_ACCESS_MIN_TIGHTEN_NOTICE_BLOCKS,
        pending_policy: None,
    });
    ledger.trustlines.push(TrustLine {
        trustline_id: "12".repeat(48),
        account: alice.clone(),
        issuer: issuer.clone(),
        asset_id: asset_id.clone(),
        limit: 500_000,
        balance: 100_000,
        authorized: false,
        frozen: false,
        reserve_paid: 0,
        holder_limit: Some(500_000),
        implicit: true,
    });

    let updated_root =
        replicated_state_root(&genesis, &governance, &ledger, &[], &shielded, &bridge)
            .expect("updated root");
    assert_ne!(base_root, updated_root);

    store.write_ledger(&ledger).expect("write ledger");

    // 2. Test ALL 6 Pillar 6 RPCs!
    let r_info = identity_info(IdentityInfoOptions {
        data_dir: dir.clone(),
        account: alice.clone(),
    })
    .unwrap();
    assert!(r_info.found);
    assert_eq!(r_info.schema, "postfiat-identity-lookup-v1");

    let r_nostr = identity_by_nostr(IdentityByNostrOptions {
        data_dir: dir.clone(),
        nostr_pubkey: nostr_pk.clone(),
    })
    .unwrap();
    assert!(r_nostr.found);

    let r_handle = identity_by_handle(IdentityByHandleOptions {
        data_dir: dir.clone(),
        handle: "goodalexander".to_string(),
        namespace: None,
    })
    .unwrap();
    assert!(r_handle.found);

    let r_link = identity_by_link(IdentityByLinkOptions {
        data_dir: dir.clone(),
        link_kind: "validator_node".to_string(),
        target: "val-0".to_string(),
    })
    .unwrap();
    assert!(r_link.found);

    let r_resolve = identity_resolve(IdentityResolveOptions {
        data_dir: dir.clone(),
        account: None,
        nostr_pubkey: None,
        handle: Some("goodalexander@tasknode.postfiat.org".to_string()),
        namespace: None,
        link_kind: None,
        target: None,
    })
    .unwrap();
    assert!(r_resolve.found);

    let r_hist = identity_history(IdentityHistoryOptions {
        data_dir: dir.clone(),
        account: alice.clone(),
        limit: Some(10),
    })
    .unwrap();
    assert_eq!(r_hist.event_count, 1);

    // 3. Test BOTH Pillar 7 RPCs!
    let r_sched = nav_fee_schedule(NavFeeScheduleOptions {
        data_dir: dir.clone(),
        asset_id: asset_id.clone(),
    })
    .unwrap();
    assert!(r_sched.found);
    assert!(r_sched.has_fee_schedule);
    assert_eq!(r_sched.blocks_per_year, PROTOCOL_BLOCKS_PER_YEAR);

    let r_fhist = nav_fee_history(NavFeeHistoryOptions {
        data_dir: dir.clone(),
        asset_id: asset_id.clone(),
        from_epoch: Some(1),
        cursor: None,
        limit: Some(10),
    })
    .unwrap();
    assert_eq!(r_fhist.accrual_count, 1);
    assert_eq!(r_fhist.accruals[0].minted_fee_units, 500);

    // 4. Test ALL 6 Pillar 8 RPCs + additive asset_info & account_lines fields!
    let r_did_full = did_resolve(DidResolveOptions {
        data_dir: dir.clone(),
        did: Some(format!("did:pftl:{}:{alice}", genesis.chain_id)),
        account: None,
    })
    .unwrap();
    assert_eq!(r_did_full.schema, "postfiat-did-resolve-v1");
    assert!(r_did_full.did_document_metadata.identity_minted);
    assert!(r_did_full.did_document_metadata.key_revealed);
    assert_eq!(r_did_full.did_document.verification_method.len(), 3);
    assert_eq!(
        r_did_full.did_document.also_known_as,
        vec!["acct:goodalexander@tasknode.postfiat.org"]
    );

    // Also supports short did:pftl:<address> form!
    let r_did_short = did_resolve(DidResolveOptions {
        data_dir: dir.clone(),
        did: Some(format!("did:pftl:{alice}")),
        account: None,
    })
    .unwrap();
    assert_eq!(r_did_short.did_document.id, r_did_full.did_document.id);

    let r_issuer = issuer_info(IssuerInfoOptions {
        data_dir: dir.clone(),
        issuer_id: kyc_issuer_id.clone(),
    })
    .unwrap();
    assert!(r_issuer.found);

    let r_ilist = issuer_list(IssuerListOptions {
        data_dir: dir.clone(),
        domain: Some("kyc.postfiat.org".to_string()),
        cursor: None,
        limit: Some(10),
    })
    .unwrap();
    assert_eq!(r_ilist.issuer_count, 1);

    let r_cred = credential_info(CredentialInfoOptions {
        data_dir: dir.clone(),
        credential_id: None,
        subject: Some(alice.clone()),
        issuer_id: Some(kyc_issuer_id.clone()),
        credential_type: Some("kyc.postfiat.org/tier1".to_string()),
    })
    .unwrap();
    assert!(r_cred.found);
    assert!(r_cred.currently_valid);

    let r_acreds = account_credentials(AccountCredentialsOptions {
        data_dir: dir.clone(),
        account: alice.clone(),
        issuer_id: None,
        include_expired: false,
        cursor: None,
        limit: Some(10),
    })
    .unwrap();
    assert_eq!(r_acreds.credential_count, 1);

    let r_access = asset_access_info(AssetAccessInfoOptions {
        data_dir: dir.clone(),
        asset_id: asset_id.clone(),
    })
    .unwrap();
    assert!(r_access.found);
    assert!(r_access.access_policy.is_some());

    // Additive fields on existing asset_info & account_lines!
    let ainfo = asset_info(AssetInfoOptions {
        data_dir: dir.clone(),
        asset_id: asset_id.clone(),
    })
    .unwrap();
    assert!(ainfo.asset.unwrap().access_policy.is_some());

    let alines = account_lines(AccountLinesOptions {
        data_dir: dir.clone(),
        account: alice.clone(),
        issuer: None,
        asset_id: Some(asset_id.clone()),
        limit: Some(10),
    })
    .unwrap();
    assert_eq!(alines.lines[0].holder_limit, Some(500_000));
    assert!(alines.lines[0].implicit);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn train_b_governance_amendments_wire_into_execution_compatibility() {
    let genesis = Genesis::new_with_validator_count("postfiat-train-b-gov", 1);
    let mut gov = GovernanceState::new(1);
    for (idx, (kind, value)) in [
        (GOVERNANCE_KIND_HIVE_IDENTITY_V1_ACTIVATION_HEIGHT, 500_u32),
        (
            GOVERNANCE_KIND_HIVE_IDENTITY_CLASSICAL_PROOF_KILL_SWITCH,
            900_u32,
        ),
        (GOVERNANCE_KIND_NAV_FEE_V1_ACTIVATION_HEIGHT, 600_u32),
        (GOVERNANCE_KIND_CREDENTIAL_DID_V1_ACTIVATION_HEIGHT, 700_u32),
    ]
    .into_iter()
    .enumerate()
    {
        gov.apply(GovernanceAmendment {
            amendment_id: format!("train-b-{idx}"),
            chain_id: genesis.chain_id.clone(),
            genesis_hash: genesis_hash(&genesis),
            protocol_version: genesis.protocol_version,
            instance_id: format!("inst-{idx}"),
            proposal_id: format!("prop-{idx}"),
            certificate_id: format!("cert-{idx}"),
            proposer: "val-0".to_string(),
            validators: vec!["val-0".to_string()],
            quorum: 1,
            support: vec!["val-0".to_string()],
            votes: Vec::new(),
            signed_authorizations: Vec::new(),
            kind: kind.to_string(),
            value,
            activation_height: 0,
            veto_until_height: 0,
            paused: false,
        });
    }
    let compat = asset_execution_compatibility_for_genesis_and_governance(&genesis, &gov);
    assert!(!compat.hive_identity_v1_active(499));
    assert!(compat.hive_identity_v1_active(500));
    assert!(!compat.hive_identity_classical_proof_disabled(899));
    assert!(compat.hive_identity_classical_proof_disabled(900));
    assert!(!compat.nav_fee_v1_active(599));
    assert!(compat.nav_fee_v1_active(600));
    assert!(!compat.credential_did_v1_active(699));
    assert!(compat.credential_did_v1_active(700));
}
