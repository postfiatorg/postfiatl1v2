    use super::execution_actions::asset_orchard_egress_policy_refusal;
    use postfiat_privacy_orchard::ASSET_ORCHARD_PRIVATE_EGRESS_CIRCUIT_ID_V1;
    use postfiat_types::{
        AssetOrchardEgressPolicyRecordV1, AssetOrchardEgressPolicyRegistry,
        ASSET_ORCHARD_EGRESS_DISCLOSURE_SCHEMA_ID_V1,
        GOVERNANCE_KIND_ASSET_ORCHARD_EGRESS_POLICY_ACTIVATION_HEIGHT,
        GOVERNANCE_KIND_ASSET_ORCHARD_EGRESS_POLICY_PREFIX_V1,
    };

    const EGRESS_POLICY_ID: &str = "a666.private-egress.v1";

    fn egress_schema_hash() -> String {
        hash_hex("postfiat.test.egress_disclosure_schema", b"a666 public exit v1")
    }

    fn egress_policy_record(asset_id: &str, effective_height: u64) -> AssetOrchardEgressPolicyRecordV1 {
        AssetOrchardEgressPolicyRecordV1::new(
            asset_id,
            EGRESS_POLICY_ID,
            egress_schema_hash(),
            effective_height,
        )
        .expect("egress policy record")
    }

    /// A private egress payload whose proof fields are not real, so execution
    /// past the governed policy check stops at payload decoding.
    fn egress_policy_payload(
        asset_id: &str,
        policy_id: &str,
        disclosure_hash: &str,
    ) -> AssetOrchardPrivateEgressActionPayload {
        let tag = AssetTag::derive(asset_id).expect("asset tag");
        AssetOrchardPrivateEgressActionPayload {
            version: 1,
            schema: "postfiat.asset_orchard.private_egress.v1".to_string(),
            pool_id: ASSET_ORCHARD_POOL_ID_V1.to_string(),
            to: address_from_public_key(&[3u8; 1952]),
            asset_id: asset_id.to_string(),
            amount: 5,
            fee: 0,
            policy_id: policy_id.to_string(),
            disclosure_hash: disclosure_hash.to_string(),
            proof_system_id: ASSET_ORCHARD_PROOF_SYSTEM_ID_V1.to_string(),
            circuit_id: ASSET_ORCHARD_PRIVATE_EGRESS_CIRCUIT_ID_V1.to_string(),
            pool_domain: "11".repeat(32),
            anchor: "22".repeat(32),
            nullifier: "33".repeat(32),
            randomized_verification_key: "44".repeat(32),
            asset_tag_lo: tag.lo,
            asset_tag_hi: tag.hi,
            exit_binding_hash: "55".repeat(64),
            proof: "not-a-proof".to_string(),
            spend_authorization_signature: "66".repeat(64),
        }
    }

    fn egress_policy_receipt(
        fixture: &PricingReadFixture,
        registry: &AssetOrchardEgressPolicyRegistry,
        payload: &AssetOrchardPrivateEgressActionPayload,
        height: u64,
    ) -> Receipt {
        let store = NodeStore::new(&fixture.data_dir);
        let genesis = store.read_genesis().expect("genesis");
        let mut ledger = store.read_ledger().expect("ledger");
        let mut shielded = store.read_shielded().expect("shielded");
        let (ledger_before, shielded_before) = (ledger.clone(), shielded.clone());
        let receipt = execute_asset_orchard_private_egress_action(
            &genesis,
            &mut ledger,
            &mut shielded,
            "egress-policy-batch",
            height,
            0,
            payload,
            registry,
            false,
        );
        assert!(!receipt.accepted);
        assert_eq!(ledger, ledger_before, "a refused egress changes no ledger state");
        assert_eq!(shielded, shielded_before, "a refused egress changes no shielded state");
        receipt
    }

    fn governance_with(amendments: &[(String, u32)]) -> GovernanceState {
        let mut governance = GovernanceState::new(1);
        for (kind, value) in amendments {
            governance.apply(pricing_enforcement_amendment(kind, *value));
        }
        governance
    }

    #[test]
    fn asset_orchard_egress_policy_is_enforced_from_the_record_effective_height() {
        let fixture = pricing_read_fixture(700, |_, _| {});
        let asset = fixture.base_asset_id.clone();
        let record = egress_policy_record(&asset, 750);
        let registry = governance_with(&[(record.governance_kind(), 750)])
            .asset_orchard_egress_policy_registry();
        let governed = egress_policy_payload(&asset, EGRESS_POLICY_ID, &egress_schema_hash());
        let no_policy = AssetOrchardEgressPolicyRegistry::default();

        // Before the effective height the receipt is byte-identical to an
        // ungoverned chain for any policy and disclosure values.
        for payload in [
            governed.clone(),
            egress_policy_payload(&asset, "tnapp-owned-v3", &"0".repeat(64)),
        ] {
            let ungoverned = egress_policy_receipt(&fixture, &no_policy, &payload, 749);
            assert_eq!(ungoverned.code, "asset_orchard_private_egress_bad_payload");
            assert_eq!(
                serde_json::to_vec(&egress_policy_receipt(&fixture, &registry, &payload, 749))
                    .expect("receipt json"),
                serde_json::to_vec(&ungoverned).expect("receipt json")
            );
        }

        for height in [750, 751] {
            // The governed binding passes the policy check and reaches payload decoding.
            assert_eq!(
                egress_policy_receipt(&fixture, &registry, &governed, height).code,
                "asset_orchard_private_egress_bad_payload"
            );
            let mut tampered = egress_schema_hash();
            tampered.replace_range(0..1, if tampered.starts_with('0') { "1" } else { "0" });
            for (policy_id, disclosure_hash) in [
                ("tnapp-owned-v3", egress_schema_hash()),
                (EGRESS_POLICY_ID, tampered),
                (EGRESS_POLICY_ID, "0".repeat(96)),
                (EGRESS_POLICY_ID, "0".repeat(64)),
            ] {
                let payload = egress_policy_payload(&asset, policy_id, &disclosure_hash);
                let receipt = egress_policy_receipt(&fixture, &registry, &payload, height);
                assert_eq!(
                    receipt.code, "asset_orchard_egress_policy_mismatch",
                    "{policy_id}: {}",
                    receipt.message
                );
            }
        }
        fs::remove_dir_all(&fixture.data_dir).expect("cleanup");
    }

    #[test]
    fn asset_orchard_egress_without_a_record_is_refused_after_the_global_activation() {
        let fixture = pricing_read_fixture(700, |_, _| {});
        let record = egress_policy_record(&fixture.base_asset_id, 750);
        let registry = governance_with(&[
            (record.governance_kind(), 750),
            (
                GOVERNANCE_KIND_ASSET_ORCHARD_EGRESS_POLICY_ACTIVATION_HEIGHT.to_string(),
                800,
            ),
        ])
        .asset_orchard_egress_policy_registry();
        let quote = egress_policy_payload(&fixture.quote_asset_id, "anything", "anything");
        assert_eq!(
            egress_policy_receipt(&fixture, &registry, &quote, 799).code,
            "asset_orchard_private_egress_bad_payload"
        );
        for height in [800, 801] {
            assert_eq!(
                egress_policy_receipt(&fixture, &registry, &quote, height).code,
                "asset_orchard_egress_policy_not_governed"
            );
        }
        let governed =
            egress_policy_payload(&fixture.base_asset_id, EGRESS_POLICY_ID, &egress_schema_hash());
        assert_eq!(
            egress_policy_receipt(&fixture, &registry, &governed, 800).code,
            "asset_orchard_private_egress_bad_payload"
        );
        fs::remove_dir_all(&fixture.data_dir).expect("cleanup");
    }

    #[test]
    fn asset_orchard_egress_policy_records_are_registered_by_governance() {
        let data_dir = unique_read_fixture_dir("postfiat-egress-policy-governance");
        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-egress-policy-governance".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 3,
        })
        .expect("init node");
        let validators = local_validator_ids(3).expect("validators");
        let asset = "ab".repeat(48);
        let apply = |name: &str, kind: String, value: u32| {
            let amendment_file = data_dir.join(format!("{name}.amendment.json"));
            ratify_governance(RatifyGovernanceOptions {
                data_dir: data_dir.clone(),
                validators: validators.clone(),
                support: validators.clone(),
                kind,
                value,
                activation_height: 0,
                veto_until_height: 0,
                paused: false,
                amendment_file: amendment_file.clone(),
            })?;
            let batch_file = data_dir.join(format!("{name}.batch.json"));
            create_governance_batch(GovernanceBatchOptions {
                data_dir: data_dir.clone(),
                amendment_file: Some(amendment_file),
                registry_update_file: None,
                batch_file: batch_file.clone(),
            })?;
            apply_unsigned_governance_fixture_for_test(ApplyBatchOptions {
                data_dir: data_dir.clone(),
                batch_file,
                certificate_file: None,
            })
        };
        let store = NodeStore::new(&data_dir);
        let root_before = store.read_chain_tip().ok().map(|tip| tip.state_root);
        let record = egress_policy_record(&asset, 50);
        let receipts = apply("record", record.governance_kind(), 50).expect("apply record");
        assert!(receipts[0].accepted, "{receipts:?}");
        let activation = apply(
            "activation",
            GOVERNANCE_KIND_ASSET_ORCHARD_EGRESS_POLICY_ACTIVATION_HEIGHT.to_string(),
            60,
        )
        .expect("apply activation");
        assert!(activation[0].accepted, "{activation:?}");
        let governance = store.read_governance().expect("governance");
        let registry = governance.asset_orchard_egress_policy_registry();
        assert_eq!(registry.records, vec![record.clone()]);
        assert_eq!(registry.activation_height, Some(60));
        assert_ne!(store.read_chain_tip().ok().map(|tip| tip.state_root), root_before);

        // A record that is already effective, or that binds an all-zero
        // schema hash, never becomes governed state.
        let too_early = apply("early", egress_policy_record(&asset, 1).governance_kind(), 1);
        let zero_hash_kind = format!(
            "{GOVERNANCE_KIND_ASSET_ORCHARD_EGRESS_POLICY_PREFIX_V1}:{asset}:{}:{EGRESS_POLICY_ID}",
            "0".repeat(96)
        );
        let zero_hash = apply("zero", zero_hash_kind, 90);
        for outcome in [too_early, zero_hash] {
            let rejected = match &outcome {
                Ok(receipts) => !receipts[0].accepted,
                Err(_) => true,
            };
            assert!(rejected, "{outcome:?}");
        }
        assert_eq!(
            store
                .read_governance()
                .expect("governance")
                .asset_orchard_egress_policy_registry()
                .records,
            vec![record]
        );
        assert_eq!(
            governance_amendment_lifecycle_rejection(
                &pricing_enforcement_amendment(&egress_policy_record(&asset, 9).governance_kind(), 9),
                9
            )
            .expect("not after the amendment block")
            .0,
            "invalid_asset_orchard_egress_policy_effective_height"
        );
        fs::remove_dir_all(&data_dir).expect("cleanup");
    }

    #[test]
    fn asset_orchard_private_egress_binding_reads_the_governed_record_consensus_enforces() {
        let fixture = pricing_read_fixture(700, |ledger, governance| {
            let base = ledger.nav_assets[0].asset_id.clone();
            let record = egress_policy_record(&base, 750);
            governance.apply(pricing_enforcement_amendment(&record.governance_kind(), 750));
        });
        let before = asset_orchard_private_egress_binding(pricing_read_options(&fixture, 749))
            .expect("binding before");
        assert_eq!(before.status, "ungoverned");
        assert!(!before.bindings[0].governed);
        let after = asset_orchard_private_egress_binding(pricing_read_options(&fixture, 750))
            .expect("binding after");
        let base = &after.bindings[0];
        assert!(base.governed);
        assert_eq!(base.policy_id.as_deref(), Some(EGRESS_POLICY_ID));
        assert_eq!(base.disclosure_hash.as_deref(), Some(egress_schema_hash().as_str()));
        assert_eq!(
            base.disclosure_schema_id.as_deref(),
            Some(ASSET_ORCHARD_EGRESS_DISCLOSURE_SCHEMA_ID_V1)
        );
        assert_eq!(base.effective_height, Some(750));
        assert!(!after.bindings[1].governed, "the quote asset has no record");
        assert_eq!(after.status, "ungoverned");

        // What the read returns is exactly what consensus accepts.
        let registry = NodeStore::new(&fixture.data_dir)
            .read_governance()
            .expect("governance")
            .asset_orchard_egress_policy_registry();
        let payload = egress_policy_payload(
            &fixture.base_asset_id,
            base.policy_id.as_deref().expect("policy"),
            base.disclosure_hash.as_deref().expect("disclosure"),
        );
        assert_eq!(asset_orchard_egress_policy_refusal(&registry, &payload, 750), None);

        let snapshot = private_pool_snapshot(PrivatePoolSnapshotOptions {
            data_dir: fixture.data_dir.clone(),
            pool_id: ASSET_ORCHARD_POOL_ID_V1.to_string(),
            address: None,
            base_asset_id: Some(fixture.base_asset_id.clone()),
            quote_asset_id: Some(fixture.quote_asset_id.clone()),
            inclusion_height: Some(750),
        })
        .expect("snapshot");
        assert_eq!(snapshot.policy_id.as_deref(), Some(EGRESS_POLICY_ID));
        assert_eq!(snapshot.disclosure_hash, Some(egress_schema_hash()));
        fs::remove_dir_all(&fixture.data_dir).expect("cleanup");
    }

    #[test]
    fn asset_orchard_private_egress_binding_is_ok_when_both_assets_are_governed() {
        let fixture = pricing_read_fixture(700, |ledger, governance| {
            for asset_id in [
                ledger.nav_assets[0].asset_id.clone(),
                ledger
                    .asset_definitions
                    .iter()
                    .find(|asset| asset.code == "PFUSDC")
                    .expect("quote")
                    .asset_id
                    .clone(),
            ] {
                let record = egress_policy_record(&asset_id, 720);
                governance.apply(pricing_enforcement_amendment(&record.governance_kind(), 720));
            }
        });
        let report = asset_orchard_private_egress_binding(pricing_read_options(&fixture, 721))
            .expect("binding");
        assert_eq!(report.status, "ok", "{report:?}");
        assert!(report.refusal.is_none());
        assert!(report.bindings.iter().all(|binding| binding.governed));
        fs::remove_dir_all(&fixture.data_dir).expect("cleanup");
    }
