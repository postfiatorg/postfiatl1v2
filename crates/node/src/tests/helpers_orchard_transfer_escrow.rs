    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use postfiat_execution::{
        minimum_asset_transaction_fee_for_ledger, minimum_escrow_transaction_fee_for_ledger,
        minimum_nft_transaction_fee_for_ledger, minimum_offer_transaction_fee_for_ledger,
        ACCOUNT_RESERVE,
    };
    use postfiat_privacy_orchard::{
        asset_orchard_accounting_record as build_asset_orchard_accounting_record,
        hash_to_pallas_base, hash_to_pallas_scalar_nonzero, orchard_empty_anchor,
        shielded_swap_authorization_proof,
        shielded_swap_build_action_test_vector, AssetOrchardBoundedBytes,
        AssetOrchardFieldElement, AssetOrchardPricingClaim, AssetOrchardPricingClaimProvenance,
        AssetOrchardProofBytes,
        AssetOrchardSpendAuthSignature,
        AssetOrchardSwapAccountingRecord, AssetOrchardSwapBindingHash, OrchardAnchor, AssetTag,
        ShieldedSwapAction, ShieldedSwapCommitment, ShieldedSwapPrivateInput,
        ShieldedSwapPrivateOutput, ASSET_ORCHARD_ACTION_SCHEMA_V1,
        ASSET_ORCHARD_ACTION_VERSION_V1, ASSET_ORCHARD_CIRCUIT_ID_V1,
        ASSET_ORCHARD_ENCRYPTED_OUTPUT_MAX_BYTES, ASSET_ORCHARD_POOL_ID_V1,
        ASSET_ORCHARD_PROOF_SYSTEM_ID_V1, VerifiedAssetOrchardPricingClaim,
    };
    use postfiat_types::{
        AssetBurnOperation, AssetClawbackOperation, AssetCreateOperation, EscrowCancelOperation,
        EscrowCreateOperation, EscrowFinishOperation, EscrowTransactionOperation,
        IssuedPaymentOperation, NftBurnOperation, NftMintOperation, NftTransferOperation,
        OfferCancelOperation, OfferCreateOperation, OfferTransactionOperation,
        SignedEscrowTransaction, SignedNftTransaction, SignedOfferTransaction, TrustSetOperation,
        UnsignedEscrowTransaction, UnsignedNftTransaction, UnsignedOfferTransaction,
        ASSET_BURN_TRANSACTION_KIND, ASSET_CLAWBACK_TRANSACTION_KIND, ASSET_CREATE_TRANSACTION_KIND,
        DEFAULT_SHIELDED_ASSET_ID, ESCROW_CANCEL_TRANSACTION_KIND, ESCROW_CREATE_TRANSACTION_KIND,
        ESCROW_FINISH_TRANSACTION_KIND, ESCROW_STATE_CANCELED, ESCROW_STATE_FINISHED,
        ESCROW_STATE_OPEN, ISSUED_PAYMENT_TRANSACTION_KIND, NFT_BURN_TRANSACTION_KIND,
        NFT_COLLECTION_FLAG_BURN_LOCKED, NFT_COLLECTION_FLAG_TRANSFER_LOCKED,
        NFT_FLAG_TRANSFERABLE, NFT_MINT_TRANSACTION_KIND, NFT_TRANSFER_TRANSACTION_KIND,
        OFFER_CANCEL_TRANSACTION_KIND, OFFER_CREATE_TRANSACTION_KIND, OFFER_OBJECT_RESERVE,
        OFFER_STATE_CANCELED, OFFER_STATE_FILLED, OFFER_STATE_OPEN, TRUST_SET_TRANSACTION_KIND,
    };

    use super::*;

    fn unique_test_dir(prefix: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "{prefix}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time")
                .as_nanos()
            ))
    }

    fn assert_orchard_public_artifact_redacted(
        label: &str,
        artifact: &str,
        private_values: &[&str],
    ) {
        for forbidden_field in [
            "rho",
            "rseed",
            "witness_auth_path",
            "spending_key_hex",
            "full_viewing_key_hex",
        ] {
            assert!(
                !artifact.contains(&format!("\"{forbidden_field}\"")),
                "public Orchard artifact {label} leaked private field {forbidden_field}"
            );
        }
        for private_value in private_values {
            assert!(!private_value.is_empty(), "private marker must not be empty");
            assert!(
                !artifact.contains(private_value),
                "public Orchard artifact {label} leaked private note or spend-authority material"
            );
        }
    }

    #[cfg(unix)]
    fn assert_private_asset_orchard_note_modes(note_file: &Path) {
        use std::os::unix::fs::PermissionsExt;

        let file_mode = fs::metadata(note_file)
            .expect("note file metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(file_mode, 0o600, "{} mode should be 0600", note_file.display());

        let parent = note_file.parent().expect("note file parent");
        let parent_mode = fs::metadata(parent)
            .expect("note parent metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(
            parent_mode,
            0o700,
            "{} mode should be 0700",
            parent.display()
        );
    }

    fn shielded_swap_test_dir(prefix: &str) -> (PathBuf, Genesis) {
        let data_dir = unique_test_dir(prefix);
        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-local".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 1,
        })
        .expect("init shielded swap test node");
        let genesis = NodeStore::new(&data_dir)
            .read_genesis()
            .expect("read shielded swap genesis");
        (data_dir, genesis)
    }

    fn valid_shielded_swap_action_for_genesis(
        genesis: &Genesis,
        anchor: OrchardAnchor,
        binding_nonce: &str,
    ) -> ShieldedSwapAction {
        let domain = orchard_authorizing_domain(genesis, "orchard-swap")
            .expect("shielded swap authorizing domain");
        shielded_swap_build_action_test_vector(
            &domain,
            "orchard-swap",
            anchor,
            valid_shielded_swap_inputs(),
            valid_shielded_swap_outputs(),
            binding_nonce,
            0,
        )
        .expect("build shielded swap action")
    }

    fn valid_shielded_swap_inputs() -> [ShieldedSwapPrivateInput; 2] {
        [
            shielded_swap_input("asset-a", 50, "input-a"),
            shielded_swap_input("asset-b", 70, "input-b"),
        ]
    }

    fn valid_shielded_swap_outputs() -> [ShieldedSwapPrivateOutput; 2] {
        [
            shielded_swap_output("asset-b", 70, "output-b"),
            shielded_swap_output("asset-a", 50, "output-a"),
        ]
    }

    fn shielded_swap_input(asset_id: &str, value: u64, tag: &str) -> ShieldedSwapPrivateInput {
        let authorization_secret = format!("{tag}-auth-secret");
        ShieldedSwapPrivateInput {
            asset_id: asset_id.to_string(),
            value,
            asset_blinding: format!("{tag}-asset-blinding"),
            value_blinding: format!("{tag}-value-blinding"),
            authorization_proof: shielded_swap_authorization_proof(
                asset_id,
                value,
                &authorization_secret,
            )
            .expect("shielded swap authorization proof"),
            authorization_secret,
        }
    }

    fn shielded_swap_output(asset_id: &str, value: u64, tag: &str) -> ShieldedSwapPrivateOutput {
        ShieldedSwapPrivateOutput {
            asset_id: asset_id.to_string(),
            value,
            asset_blinding: format!("{tag}-asset-blinding"),
            value_blinding: format!("{tag}-value-blinding"),
        }
    }

    fn write_shielded_swap_action_file(
        data_dir: &Path,
        name: &str,
        action: &ShieldedSwapAction,
    ) -> PathBuf {
        let path = data_dir.join(name);
        let json = serde_json::to_string_pretty(action).expect("serialize swap action");
        atomic_write(&path, format!("{json}\n")).expect("write shielded swap action");
        path
    }

    fn apply_raw_shielded_swap_payload(
        data_dir: &Path,
        genesis: &Genesis,
        name: &str,
        swap_json: String,
    ) -> Vec<Receipt> {
        let batch = build_shielded_action_batch(
            genesis,
            vec![ShieldedAction::ShieldedSwapV1(ShieldedSwapActionPayload {
                swap_json,
            })],
        )
        .expect("build raw shielded swap batch");
        let batch_file = data_dir.join(name);
        write_shielded_action_batch_file(&batch_file, &batch)
            .expect("write raw shielded swap batch");
        apply_shielded_batch(ApplyBatchOptions {
            data_dir: data_dir.to_path_buf(),
            batch_file,
            certificate_file: None,
        })
        .expect("apply raw shielded swap batch")
    }

    #[allow(clippy::too_many_arguments)]
    fn signed_asset_transaction_for_test(
        genesis: &Genesis,
        ledger: &LedgerState,
        source: &str,
        public_key_hex: &str,
        private_key_hex: &str,
        transaction_kind: &str,
        sequence: u64,
        operation: AssetTransactionOperation,
    ) -> SignedAssetTransaction {
        let private_key = hex_to_bytes(private_key_hex).expect("private key bytes");
        let mut fee = MIN_TRANSFER_FEE;
        for _ in 0..8 {
            let unsigned = UnsignedAssetTransaction {
                chain_id: genesis.chain_id.clone(),
                genesis_hash: genesis_hash(genesis),
                protocol_version: genesis.protocol_version,
                address_namespace: postfiat_types::ADDRESS_NAMESPACE.to_string(),
                transaction_kind: transaction_kind.to_string(),
                signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
                source: source.to_string(),
                fee,
                sequence,
                operation: operation.clone(),
            };
            let signature =
                ml_dsa_65_sign(&private_key, &unsigned.signing_bytes()).expect("sign asset tx");
            let signed = SignedAssetTransaction {
                unsigned,
                algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
                public_key_hex: public_key_hex.to_string(),
                signature_hex: bytes_to_hex(&signature),
            };
            let minimum_fee = minimum_asset_transaction_fee_for_ledger(ledger, &signed);
            if fee >= minimum_fee {
                return signed;
            }
            fee = minimum_fee;
        }
        panic!("minimum asset transaction fee did not converge");
    }

    #[allow(clippy::too_many_arguments)]
    fn signed_escrow_transaction_for_test(
        genesis: &Genesis,
        ledger: &LedgerState,
        source: &str,
        public_key_hex: &str,
        private_key_hex: &str,
        transaction_kind: &str,
        sequence: u64,
        operation: EscrowTransactionOperation,
    ) -> SignedEscrowTransaction {
        let private_key = hex_to_bytes(private_key_hex).expect("private key bytes");
        let mut fee = MIN_TRANSFER_FEE;
        for _ in 0..8 {
            let unsigned = UnsignedEscrowTransaction {
                chain_id: genesis.chain_id.clone(),
                genesis_hash: genesis_hash(genesis),
                protocol_version: genesis.protocol_version,
                address_namespace: postfiat_types::ADDRESS_NAMESPACE.to_string(),
                transaction_kind: transaction_kind.to_string(),
                signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
                source: source.to_string(),
                fee,
                sequence,
                operation: operation.clone(),
            };
            let signature = ml_dsa_65_sign(&private_key, &unsigned.signing_bytes())
                .expect("sign escrow tx");
            let signed = SignedEscrowTransaction {
                unsigned,
                algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
                public_key_hex: public_key_hex.to_string(),
                signature_hex: bytes_to_hex(&signature),
            };
            let minimum_fee = minimum_escrow_transaction_fee_for_ledger(ledger, &signed);
            if fee >= minimum_fee {
                return signed;
            }
            fee = minimum_fee;
        }
        panic!("minimum escrow transaction fee did not converge");
    }

    #[allow(clippy::too_many_arguments)]
    fn signed_nft_transaction_for_test(
        genesis: &Genesis,
        ledger: &LedgerState,
        source: &str,
        public_key_hex: &str,
        private_key_hex: &str,
        transaction_kind: &str,
        sequence: u64,
        operation: NftTransactionOperation,
    ) -> SignedNftTransaction {
        let private_key = hex_to_bytes(private_key_hex).expect("private key bytes");
        let mut fee = MIN_TRANSFER_FEE;
        for _ in 0..8 {
            let unsigned = UnsignedNftTransaction {
                chain_id: genesis.chain_id.clone(),
                genesis_hash: genesis_hash(genesis),
                protocol_version: genesis.protocol_version,
                address_namespace: postfiat_types::ADDRESS_NAMESPACE.to_string(),
                transaction_kind: transaction_kind.to_string(),
                signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
                source: source.to_string(),
                fee,
                sequence,
                operation: operation.clone(),
            };
            let signature =
                ml_dsa_65_sign(&private_key, &unsigned.signing_bytes()).expect("sign nft tx");
            let signed = SignedNftTransaction {
                unsigned,
                algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
                public_key_hex: public_key_hex.to_string(),
                signature_hex: bytes_to_hex(&signature),
            };
            let minimum_fee = minimum_nft_transaction_fee_for_ledger(ledger, &signed);
            if fee >= minimum_fee {
                return signed;
            }
            fee = minimum_fee;
        }
        panic!("minimum nft transaction fee did not converge");
    }

    #[allow(clippy::too_many_arguments)]
    fn signed_offer_transaction_for_test(
        genesis: &Genesis,
        ledger: &LedgerState,
        source: &str,
        public_key_hex: &str,
        private_key_hex: &str,
        transaction_kind: &str,
        sequence: u64,
        operation: OfferTransactionOperation,
    ) -> SignedOfferTransaction {
        signed_offer_transaction_for_test_at_height(
            genesis,
            ledger,
            source,
            public_key_hex,
            private_key_hex,
            transaction_kind,
            sequence,
            operation,
            1,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn signed_offer_transaction_for_test_at_height(
        genesis: &Genesis,
        ledger: &LedgerState,
        source: &str,
        public_key_hex: &str,
        private_key_hex: &str,
        transaction_kind: &str,
        sequence: u64,
        operation: OfferTransactionOperation,
        block_height: u64,
    ) -> SignedOfferTransaction {
        let private_key = hex_to_bytes(private_key_hex).expect("private key bytes");
        let mut fee = MIN_TRANSFER_FEE;
        for _ in 0..8 {
            let unsigned = UnsignedOfferTransaction {
                chain_id: genesis.chain_id.clone(),
                genesis_hash: genesis_hash(genesis),
                protocol_version: genesis.protocol_version,
                address_namespace: postfiat_types::ADDRESS_NAMESPACE.to_string(),
                transaction_kind: transaction_kind.to_string(),
                signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
                source: source.to_string(),
                fee,
                sequence,
                operation: operation.clone(),
            };
            let signature =
                ml_dsa_65_sign(&private_key, &unsigned.signing_bytes()).expect("sign offer tx");
            let signed = SignedOfferTransaction {
                unsigned,
                algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
                public_key_hex: public_key_hex.to_string(),
                signature_hex: bytes_to_hex(&signature),
            };
            let minimum_fee =
                minimum_offer_transaction_fee_for_ledger(ledger, &signed, block_height);
            if fee >= minimum_fee {
                return signed;
            }
            fee = minimum_fee;
        }
        panic!("minimum offer transaction fee did not converge");
    }

    fn assert_asset_invariants_for_test(
        genesis: &Genesis,
        ledger: &LedgerState,
        asset_id: &str,
        expected_supply: u64,
        expected_balances: &[(&str, u64)],
    ) {
        ledger
            .validate_asset_state(&genesis.chain_id)
            .expect("valid issued asset state");
        let asset = ledger
            .asset_definition(asset_id)
            .expect("asset definition exists");
        let mut observed_supply = 0_u64;
        let mut holder_count = 0_u64;
        for line in ledger
            .trustlines
            .iter()
            .filter(|line| line.asset_id == asset_id)
        {
            assert_eq!(line.issuer, asset.issuer);
            assert!(line.balance <= line.limit);
            assert_eq!(
                line.trustline_id,
                postfiat_types::trustline_id(&line.account, &line.issuer, &line.asset_id)
                    .expect("deterministic trustline id")
            );
            if line.balance > 0 {
                holder_count += 1;
            }
            observed_supply = observed_supply
                .checked_add(line.balance)
                .expect("supply total does not overflow");
        }
        for escrow in ledger
            .escrows
            .iter()
            .filter(|escrow| escrow.asset_id == asset_id && escrow.state == ESCROW_STATE_OPEN)
        {
            observed_supply = observed_supply
                .checked_add(escrow.amount)
                .expect("supply total does not overflow");
        }
        for offer in ledger.offers.iter().filter(|offer| {
            offer.taker_gets_asset_id == asset_id && offer.state == OFFER_STATE_OPEN
        }) {
            observed_supply = observed_supply
                .checked_add(offer.taker_gets_amount_remaining)
                .expect("supply total does not overflow");
        }
        assert_eq!(observed_supply, expected_supply);
        assert_eq!(
            holder_count,
            expected_balances
                .iter()
                .filter(|(_, balance)| *balance > 0)
                .count() as u64
        );
        if let Some(max_supply) = asset.max_supply {
            assert!(observed_supply <= max_supply);
        }
        for (account, expected_balance) in expected_balances {
            assert_eq!(
                ledger
                    .trustline_for_account_asset(account, asset_id)
                    .expect("expected trustline")
                    .balance,
                *expected_balance
            );
        }
    }

    fn native_pft_account_offer_total_for_test(ledger: &LedgerState) -> u64 {
        let account_total = ledger.accounts.iter().fold(0_u64, |total, account| {
            total
                .checked_add(account.balance)
                .expect("account PFT total does not overflow")
        });
        let open_offer_sell_side_total = ledger
            .offers
            .iter()
            .filter(|offer| {
                offer.state == OFFER_STATE_OPEN
                    && offer.taker_gets_asset_id == postfiat_execution::NATIVE_PFT_ESCROW_ASSET_ID
            })
            .fold(0_u64, |total, offer| {
                total
                    .checked_add(offer.taker_gets_amount_remaining)
                    .expect("open offer PFT total does not overflow")
            });
        let offer_reserve_total = ledger.offers.iter().fold(0_u64, |total, offer| {
            total
                .checked_add(offer.reserve_paid)
                .expect("offer reserve total does not overflow")
        });
        account_total
            .checked_add(open_offer_sell_side_total)
            .and_then(|total| total.checked_add(offer_reserve_total))
            .expect("native PFT conservation total does not overflow")
    }

    fn assert_offer_conservation_for_test(
        genesis: &Genesis,
        ledger: &LedgerState,
        asset_id: &str,
        expected_supply: u64,
        expected_balances: &[(&str, u64)],
        expected_native_pft_after_burn: u64,
    ) {
        ledger
            .validate_offer_state(&genesis.chain_id)
            .expect("valid offer state");
        assert_asset_invariants_for_test(
            genesis,
            ledger,
            asset_id,
            expected_supply,
            expected_balances,
        );
        assert_eq!(
            expected_native_pft_after_burn,
            native_pft_account_offer_total_for_test(ledger)
        );
    }

    fn assert_orchard_scan_witnesses(
        report: &OrchardWalletScanReport,
        latest_root: &str,
        latest_output_count: u64,
    ) {
        assert_eq!(report.latest_retained_root, latest_root);
        assert_eq!(report.latest_retained_output_count, latest_output_count);
        for output in &report.outputs {
            assert_eq!(
                usize::try_from(output.merkle_position),
                Ok(output.output_index)
            );
            assert_eq!(output.witness_anchor, latest_root);
            assert_eq!(output.witness_output_count, latest_output_count);
            assert_eq!(output.witness_auth_path.len(), 32);
            assert!(output
                .witness_auth_path
                .iter()
                .all(|node| node.len() == ORCHARD_COMMITMENT_BYTES * 2));
        }
    }

    fn test_orchard_action_for_genesis(
        genesis: &Genesis,
    ) -> postfiat_privacy_orchard::OrchardShieldedAction {
        use orchard::{
            builder::{Builder, BundleType},
            circuit::ProvingKey,
            keys::{FullViewingKey, Scope, SpendingKey},
            value::NoteValue,
            Anchor,
        };
        use postfiat_privacy_orchard::{
            orchard_action_from_authorized_bundle, orchard_authorizing_sighash,
            OrchardAuthorizingDomain,
        };
        use rand::rngs::OsRng;

        let domain = OrchardAuthorizingDomain::new(
            genesis.chain_id.clone(),
            genesis_hash(genesis),
            genesis.protocol_version,
            "orchard-v1",
        )
        .expect("Orchard authorizing domain");
        let spending_key = SpendingKey::from_bytes([7u8; 32]).unwrap();
        let recipient = FullViewingKey::from(&spending_key).address_at(0u32, Scope::External);
        let mut builder = Builder::new(BundleType::DEFAULT, Anchor::empty_tree());
        builder
            .add_output(None, recipient, NoteValue::from_raw(0), [0u8; 512])
            .expect("add Orchard output");

        let (unsigned_bundle, _) = builder
            .build::<i64>(OsRng)
            .expect("build Orchard bundle")
            .expect("bundle should be present");
        let sighash = orchard_authorizing_sighash(&domain, 0, &unsigned_bundle)
            .expect("Orchard authorizing sighash");
        let proving_key = ProvingKey::build();
        let bundle = unsigned_bundle
            .create_proof(&proving_key, OsRng)
            .expect("create Orchard proof")
            .apply_signatures(OsRng, sighash, &[])
            .expect("apply Orchard signatures");
        orchard_action_from_authorized_bundle("orchard-v1", 0, &bundle)
            .expect("serialize Orchard action")
    }

    fn signed_test_operator_manifest(
        chain_id: &str,
        network: &str,
        validator_id: &str,
        hot_public_key_hex: &str,
        master_seed: [u8; 32],
        operator: &str,
    ) -> OperatorManifest {
        signed_test_operator_manifest_with_cobalt_trust(
            chain_id,
            network,
            validator_id,
            hot_public_key_hex,
            master_seed,
            operator,
            None,
        )
    }

    fn signed_test_operator_manifest_with_cobalt_trust(
        chain_id: &str,
        network: &str,
        validator_id: &str,
        hot_public_key_hex: &str,
        master_seed: [u8; 32],
        operator: &str,
        cobalt_trust: Option<OperatorCobaltTrustBinding>,
    ) -> OperatorManifest {
        let master_key = ml_dsa_65_keygen_from_seed(&master_seed);
        let mut manifest = OperatorManifest {
            schema: OPERATOR_MANIFEST_FILE_SCHEMA.to_string(),
            chain_id: chain_id.to_string(),
            network: network.to_string(),
            validator_id: validator_id.to_string(),
            master_public_key_hex: bytes_to_hex(&master_key.public_key),
            hot_public_key_hex: hot_public_key_hex.to_string(),
            algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            key_role: "validator-hot".to_string(),
            operator: operator.to_string(),
            contact: format!("{validator_id}@operators.example"),
            infrastructure: OperatorInfrastructureLabels {
                provider_group: format!("{validator_id}-provider"),
                region_group: format!("{validator_id}-region"),
                jurisdiction_group: format!("{validator_id}-jurisdiction"),
                legal_domain_group: format!("{validator_id}-legal"),
                funding_domain_group: format!("{validator_id}-funding"),
            },
            rotation_state: "active".to_string(),
            effective_height: 0,
            cobalt_trust,
            manifest_signing_key_hex: bytes_to_hex(&master_key.public_key),
            signature_hex: String::new(),
            manifest_hash: String::new(),
        };
        let payload =
            operator_manifest_signing_payload_bytes(&manifest).expect("operator manifest payload");
        let signature = ml_dsa_65_sign_with_context_seed(
            &master_key.private_key,
            &payload,
            OPERATOR_MANIFEST_SIGNATURE_CONTEXT,
            &[19u8; 32],
        )
        .expect("operator manifest signature");
        manifest.signature_hex = bytes_to_hex(&signature);
        manifest.manifest_hash = operator_manifest_hash(&manifest).expect("operator manifest hash");
        manifest
    }

    fn test_cobalt_trust_binding(
        graph_label: &str,
        graph_version: u64,
        view_label: &str,
        view_version: u64,
    ) -> OperatorCobaltTrustBinding {
        OperatorCobaltTrustBinding {
            trust_graph_root: hash_hex(
                "postfiat.test.operator_manifest.cobalt.trust_graph",
                graph_label.as_bytes(),
            ),
            trust_graph_version: graph_version,
            trust_view_id: hash_hex(
                "postfiat.test.operator_manifest.cobalt.trust_view",
                view_label.as_bytes(),
            ),
            trust_view_version: view_version,
        }
    }

    fn write_test_operator_manifest(path: &Path, manifest: &OperatorManifest) {
        let json = serde_json::to_string_pretty(manifest).expect("serialize operator manifest");
        atomic_write(path, format!("{json}\n")).expect("write operator manifest");
    }

    fn write_test_master_key(path: &Path, seed: [u8; 32]) -> DevKeyFile {
        let key_pair = ml_dsa_65_keygen_from_seed(&seed);
        let key_file = DevKeyFile {
            algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            address: address_from_public_key(&key_pair.public_key),
            public_key_hex: bytes_to_hex(&key_pair.public_key),
            private_key_hex: bytes_to_hex(&key_pair.private_key),
        };
        write_key_file(path, &key_file).expect("write test master key");
        key_file
    }

    #[test]
    fn replicated_state_root_commits_to_chain_domain() {
        let governance = GovernanceState::new(1);
        let ledger = LedgerState::empty();
        let ordered_batches = Vec::<String>::new();
        let shielded = ShieldedState::empty();
        let bridge = BridgeState::empty();
        let local_genesis = Genesis::new("postfiat-local");
        let other_genesis = Genesis::new("postfiat-other");
        let legacy_genesis = Genesis::from_json(
            "{\n  \"chain_id\": \"postfiat-vector-test\",\n  \"protocol_version\": 1,\n  \"validator_count\": 5\n}\n",
        )
        .expect("legacy genesis JSON");
        assert_eq!(
            genesis_hash(&legacy_genesis),
            "84372116390c0426768a038a3f15f12b0d7fade112004732fdf1ea9834a025b22f07ad3c5737dce008a1e5112cfcb60a",
            "adding the optional v2 activation must not change a legacy genesis hash"
        );

        let local_root = replicated_state_root(
            &local_genesis,
            &governance,
            &ledger,
            &ordered_batches,
            &shielded,
            &bridge,
        )
        .expect("local state root");
        let other_root = replicated_state_root(
            &other_genesis,
            &governance,
            &ledger,
            &ordered_batches,
            &shielded,
            &bridge,
        )
        .expect("other state root");

        assert_eq!(local_root.len(), 96);
        assert_eq!(
            local_root,
            "d71ff8b0dce55a3a2aba436c40f5b618f7dc9c5a00d8876e6e298bbfb99ef7201470b0114775bf7fddb007d410d6e393"
        );
        assert_ne!(local_root, other_root);
    }

    #[test]
    fn replicated_state_root_commits_to_orchard_pool_pause() {
        let genesis = Genesis::new("postfiat-local");
        let mut governance = GovernanceState::new(1);
        let ledger = LedgerState::empty();
        let ordered_batches = Vec::<String>::new();
        let shielded = ShieldedState::empty();
        let bridge = BridgeState::empty();

        let active_root = replicated_state_root(
            &genesis,
            &governance,
            &ledger,
            &ordered_batches,
            &shielded,
            &bridge,
        )
        .expect("active orchard state root");
        governance.orchard_pool_paused = true;
        let paused_root = replicated_state_root(
            &genesis,
            &governance,
            &ledger,
            &ordered_batches,
            &shielded,
            &bridge,
        )
        .expect("paused orchard state root");
        governance.orchard_pool_paused = false;
        let resumed_root = replicated_state_root(
            &genesis,
            &governance,
            &ledger,
            &ordered_batches,
            &shielded,
            &bridge,
        )
        .expect("resumed orchard state root");

        assert_ne!(active_root, paused_root);
        assert_eq!(active_root, resumed_root);
    }

    #[test]
    fn replicated_state_root_commits_every_fastlane_ledger_field() {
        use postfiat_types::{
            FastAssetDefinitionHashV1, FastAssetIdV1, FastAssetRuleHashV1, FastAssetRuleV1,
            FastHolderPermitIdV1, FastHolderPermitV1, FastLaneCheckpointCertificateV1,
            FastLaneCheckpointV1, FastLaneCheckpointVoteV1, FastLaneDepositReceiptV1,
            FastLanePrepareFenceV1, FastLaneReserveBalanceV1, FastObjectIdV1, FastObjectKeyV1,
            FastSwapChainDomainV1, FastSwapCommitteeDomainV1, FastSwapCommitteeRootV1,
            FastSwapCommitteeV1, FastSwapExitClaimIdV1, FastSwapMarketEnvelopeHashV1,
            FastSwapOpaqueHashV1, FastSwapPolicyHashV1, FastSwapPolicySnapshotV1,
            FastSwapQuoteRoundingV1, FastSwapValidatorV1, GovernanceAmendment,
            FastPayCertificateV1, FastPayOperationKindV1, FastPayOrderRecoveryV1,
            FastPayRecoveryCommitteeV1, FastPayRecoveryDecisionV1, FastPayRecoveryPolicyV1,
            FastPayRecoveryRevealV1, FastPayVersionFenceV1, OwnedCertificateDomain,
            OwnedObjectRef, OwnedOutputSpec, OwnedTransferCertificateV3, OwnedTransferOrderV3,
            GOVERNANCE_KIND_REPLICATED_STATE_V2_ACTIVATION_HEIGHT, FASTSWAP_SCHEMA_VERSION_V1,
            FASTPAY_ORDER_RECOVERY_SCHEMA_V1, FASTPAY_RECOVERY_POLICY_SCHEMA_V1,
            FASTPAY_RECOVERY_REVEAL_SCHEMA_V1, FASTPAY_VERSION_FENCE_SCHEMA_V1,
            OWNED_CERTIFICATE_DOMAIN_SCHEMA_V3,
        };

        let genesis = Genesis::new("postfiat-fastlane-root-test");
        let governance = GovernanceState::new(4);
        let ordered_batches = Vec::<String>::new();
        let shielded = ShieldedState::empty();
        let bridge = BridgeState::empty();
        let root = |ledger: &LedgerState| {
            replicated_state_root(
                &genesis,
                &governance,
                ledger,
                &ordered_batches,
                &shielded,
                &bridge,
            )
            .expect("replicated state root")
        };
        let baseline = root(&LedgerState::empty());

        let chain = FastSwapChainDomainV1 {
            chain_id: genesis.chain_id.clone(),
            genesis_hash: FastSwapOpaqueHashV1([3; 48]),
            protocol_version: genesis.protocol_version,
        };
        let mut committee = FastSwapCommitteeV1 {
            domain: FastSwapCommitteeDomainV1 {
                chain: chain.clone(),
                fastswap_schema_version: FASTSWAP_SCHEMA_VERSION_V1,
                committee_epoch: 1,
                committee_root: FastSwapCommitteeRootV1::ZERO,
                validator_count: 4,
                quorum: 3,
            },
            validators: (0..4)
                .map(|index| FastSwapValidatorV1 {
                    validator_id: format!("validator-{index}"),
                    public_key: vec![index as u8 + 1],
                })
                .collect(),
        };
        committee.domain.committee_root = committee.computed_root().expect("committee root");

        let root_asset_a =
            AssetDefinition::new(&genesis.chain_id, "pfissuer0", "ROOTA", 1, 0)
                .expect("root asset A");
        let root_asset_b =
            AssetDefinition::new(&genesis.chain_id, "pfissuer1", "ROOTB", 1, 0)
                .expect("root asset B");
        let (asset_definition_0, asset_definition_1) =
            if root_asset_a.asset_id < root_asset_b.asset_id {
                (root_asset_a, root_asset_b)
            } else {
                (root_asset_b, root_asset_a)
            };
        let asset_0 = FastAssetIdV1(
            hex_to_bytes(&asset_definition_0.asset_id)
                .expect("root asset A id hex")
                .try_into()
                .expect("root asset A id length"),
        );
        let asset_1 = FastAssetIdV1(
            hex_to_bytes(&asset_definition_1.asset_id)
                .expect("root asset B id hex")
                .try_into()
                .expect("root asset B id length"),
        );
        let rule_hash_0 = FastAssetRuleHashV1([12; 48]);
        let rule_hash_1 = FastAssetRuleHashV1([13; 48]);
        let mut permit = FastHolderPermitV1 {
            permit_id: FastHolderPermitIdV1::ZERO,
            asset_id: asset_0,
            owner_pubkey: vec![14; 32],
            valid_from_height: 1,
            valid_through_height: 100,
            consensus_receipt_digest: FastSwapOpaqueHashV1([15; 48]),
        };
        permit.permit_id = permit.computed_id().expect("permit id");
        let mut policy = FastSwapPolicySnapshotV1 {
            domain: chain,
            policy_epoch: 1,
            policy_hash: FastSwapPolicyHashV1::ZERO,
            pair_asset_0: asset_0,
            pair_asset_1: asset_1,
            asset_rule_hash_0: rule_hash_0,
            asset_rule_hash_1: rule_hash_1,
            price_numerator: 8,
            price_denominator: 1,
            rounding: FastSwapQuoteRoundingV1::Exact,
            nav_epoch: 1,
            market_envelope_hash: FastSwapMarketEnvelopeHashV1([16; 48]),
            valid_from_height: 1,
            valid_through_height: 100,
            fee_schedule_hash: FastSwapOpaqueHashV1([17; 48]),
            max_inputs_per_party: 2,
            max_outputs: 2,
            paused: false,
        };
        policy.policy_hash = policy.computed_hash().expect("policy hash");
        let checkpoint = FastLaneCheckpointV1 {
            previous_checkpoint_id: None,
            committee: committee.domain.clone(),
            live_object_root: FastSwapOpaqueHashV1([18; 48]),
            live_object_totals: Vec::new(),
            exit_claim_root: FastSwapOpaqueHashV1([19; 48]),
            exit_claim_totals: Vec::new(),
            pending_fee_burn_totals: Vec::new(),
            terminal_root: FastSwapOpaqueHashV1([20; 48]),
            highest_wal_sequence: 1,
            active_policy_hashes: vec![policy.policy_hash],
            imported_deposit_root: FastSwapOpaqueHashV1([21; 48]),
            redeemed_exit_claim_root: FastSwapOpaqueHashV1([22; 48]),
            drain_ready: true,
            fenced_policy_epochs: vec![1],
        };

        let mut cases = Vec::<(&str, LedgerState)>::new();
        let mut ledger = LedgerState::empty();
        ledger.asset_definitions = vec![asset_definition_0.clone()];
        ledger.fast_lane_reserves.push(FastLaneReserveBalanceV1 {
            asset_id: asset_0,
            amount_atoms: 1,
        });
        cases.push(("reserves", ledger));
        let mut ledger = LedgerState::empty();
        ledger.fast_lane_deposit_receipts.push(FastLaneDepositReceiptV1 {
            deposit_id: postfiat_types::FastSwapDepositIdV1([23; 48]),
            accepted: true,
            code: "fastlane_deposit_accepted".to_owned(),
            destination_owner_pubkey: vec![24; 32],
            asset_id: asset_0,
            asset_rule_hash: rule_hash_0,
            amount_atoms: 1,
            initial_object_key: FastObjectKeyV1 {
                object_id: FastObjectIdV1([25; 32]),
                version: 1,
            },
        });
        cases.push(("deposit receipts", ledger));
        let mut ledger = LedgerState::empty();
        ledger
            .redeemed_fast_lane_exit_claims
            .push(FastSwapExitClaimIdV1([26; 48]));
        cases.push(("redeemed exits", ledger));
        let mut ledger = LedgerState::empty();
        ledger.fast_lane_asset_rules.push(FastAssetRuleV1 {
            asset_id: FastAssetIdV1::native_pft(),
            asset_definition_hash: FastAssetDefinitionHashV1::ZERO,
            issuer_address: "native".to_owned(),
            issuer_control_pubkey: vec![27; 32],
            requires_authorization: false,
            freeze_enabled: false,
            clawback_enabled: false,
            fast_lane_enabled: true,
            valid_from_height: 1,
            valid_through_height: 100,
        });
        cases.push(("asset rules", ledger));
        let mut ledger = LedgerState::empty();
        ledger.fast_lane_holder_permits.push(permit);
        cases.push(("holder permits", ledger));
        let mut ledger = LedgerState::empty();
        ledger.fastswap_policy_snapshots.push(policy);
        cases.push(("policy snapshots", ledger));
        let mut ledger = LedgerState::empty();
        ledger.fastswap_committees.push(committee.clone());
        cases.push(("committees", ledger));
        let mut ledger = LedgerState::empty();
        ledger.fast_lane_prepare_fences.push(FastLanePrepareFenceV1 {
            committee_epoch: 1,
            policy_epoch: 1,
            finalized_primary_height: 10,
        });
        cases.push(("prepare fences", ledger));
        let mut ledger = LedgerState::empty();
        ledger
            .fast_lane_checkpoint_anchors
            .push(FastLaneCheckpointCertificateV1 {
                votes: vec![FastLaneCheckpointVoteV1 {
                    checkpoint,
                    validator_id: "validator-0".to_owned(),
                    signature: vec![28],
                }],
            });
        cases.push(("checkpoint anchors", ledger));
        let mut ledger = LedgerState::empty();
        ledger.fastswap_activation_height = Some(10);
        cases.push(("activation height", ledger));

        let recovery_policy = FastPayRecoveryPolicyV1 {
            schema: FASTPAY_RECOVERY_POLICY_SCHEMA_V1.to_string(),
            activation_height: 90,
            max_validity_blocks: 20,
            max_recovery_blocks: 20,
        };
        let mut ledger = LedgerState::empty();
        ledger.fastpay_recovery_policy = Some(recovery_policy);
        cases.push(("FastPay recovery policy", ledger));
        let mut ledger = LedgerState::empty();
        ledger.fastpay_recovery_committees.push(
            FastPayRecoveryCommitteeV1::from_public_keys(
                genesis.chain_id.clone(),
                genesis_hash(&genesis),
                genesis.protocol_version,
                1,
                90,
                120,
                (0..4)
                    .map(|index| {
                        (
                            format!("validator-{index}"),
                            format!("{:02x}", index + 1).repeat(32),
                        )
                    })
                    .collect(),
            )
            .expect("FastPay recovery committee"),
        );
        cases.push(("FastPay recovery committees", ledger));

        let input = OwnedObjectRef {
            id: "31".repeat(32),
            version: 9,
        };
        let lock_id = "32".repeat(48);
        let transfer_certificate = OwnedTransferCertificateV3 {
            order: OwnedTransferOrderV3 {
                domain: OwnedCertificateDomain {
                    schema: OWNED_CERTIFICATE_DOMAIN_SCHEMA_V3.to_string(),
                    chain_id: genesis.chain_id.clone(),
                    genesis_hash: "33".repeat(48),
                    protocol_version: genesis.protocol_version,
                    registry_id: "34".repeat(48),
                },
                recovery: FastPayOrderRecoveryV1 {
                    schema: FASTPAY_ORDER_RECOVERY_SCHEMA_V1.to_string(),
                    committee_epoch: 1,
                    lock_id: lock_id.clone(),
                    valid_from_height: 100,
                    expires_at_height: 110,
                    recovery_closes_at_height: 120,
                },
                inputs: vec![input.clone()],
                outputs: vec![OwnedOutputSpec {
                    owner_pubkey_hex: "35".repeat(32),
                    value: 1,
                    asset: "PFT".to_string(),
                }],
                fee: 0,
                nonce: 1,
                memos: Vec::new(),
            },
            owner_pubkey_hex: "35".repeat(32),
            owner_signature_hex: "36".repeat(32),
            votes: Vec::new(),
        };
        let mut ledger = LedgerState::empty();
        ledger.fastpay_recovery_reveals.push(FastPayRecoveryRevealV1 {
            schema: FASTPAY_RECOVERY_REVEAL_SCHEMA_V1.to_string(),
            lock_id: lock_id.clone(),
            order_digest: "37".repeat(48),
            certificate_digest: "38".repeat(48),
            revealed_at_height: 111,
            certificate: FastPayCertificateV1::Transfer(transfer_certificate),
        });
        cases.push(("FastPay recovery reveals", ledger));

        let mut ledger = LedgerState::empty();
        ledger.fastpay_version_fences.push(FastPayVersionFenceV1 {
            schema: FASTPAY_VERSION_FENCE_SCHEMA_V1.to_string(),
            operation: FastPayOperationKindV1::Transfer,
            origin: postfiat_types::FastPayFenceOriginV1::OrderedRecovery,
            committee_epoch: 1,
            registry_root: "34".repeat(48),
            lock_id,
            inputs: vec![input.clone()],
            decision: FastPayRecoveryDecisionV1::Cancelled,
            certificate: None,
            decided_at_height: 120,
            next_versions: vec![OwnedObjectRef {
                id: input.id,
                version: 10,
            }],
        });
        cases.push(("FastPay version fences", ledger));

        for (label, ledger) in cases {
            assert_ne!(baseline, root(&ledger), "state root omitted {label}");
        }

        let reserve_a = FastLaneReserveBalanceV1 {
            asset_id: asset_0,
            amount_atoms: 7,
        };
        let reserve_b = FastLaneReserveBalanceV1 {
            asset_id: asset_1,
            amount_atoms: 9,
        };
        let mut ordered = LedgerState::empty();
        ordered.asset_definitions = vec![asset_definition_0.clone(), asset_definition_1.clone()];
        ordered.fast_lane_reserves = vec![reserve_a.clone(), reserve_b.clone()];
        let mut reversed = LedgerState::empty();
        reversed.asset_definitions = vec![asset_definition_0, asset_definition_1];
        reversed.fast_lane_reserves = vec![reserve_b, reserve_a];
        assert_eq!(root(&ordered), root(&reversed));
        assert_eq!(
            root(&ordered),
            "382bbc70ec041628dff59bf00bf84068244b4b91db81494bef4c81507fe89abbf0af5dafe982e89e656929186f017c97"
        );
        reversed.fast_lane_reserves[0].amount_atoms += 1;
        assert_ne!(root(&ordered), root(&reversed));

        let mut activation_genesis = Genesis::new("postfiat-fastlane-root-activation-test");
        activation_genesis.replicated_state_v2_activation_height = Some(2);
        let before_activation = vec!["batch-1".to_owned()];
        let at_activation = vec!["batch-1".to_owned(), "batch-2".to_owned()];
        let mut no_fastlane = ordered.clone();
        no_fastlane.fast_lane_reserves.clear();
        assert_eq!(
            replicated_state_root(
                &activation_genesis,
                &governance,
                &no_fastlane,
                &before_activation,
                &shielded,
                &bridge,
            )
            .expect("legacy empty root before activation"),
            replicated_state_root(
                &activation_genesis,
                &governance,
                &ordered,
                &before_activation,
                &shielded,
                &bridge,
            )
            .expect("legacy FastLane root before activation"),
            "FastLane commitment must not alter pre-activation history"
        );
        assert_ne!(
            replicated_state_root(
                &activation_genesis,
                &governance,
                &no_fastlane,
                &at_activation,
                &shielded,
                &bridge,
            )
            .expect("v2 empty root at activation"),
            replicated_state_root(
                &activation_genesis,
                &governance,
                &ordered,
                &at_activation,
                &shielded,
                &bridge,
            )
            .expect("v2 FastLane root at activation"),
            "FastLane commitment must activate exactly at the configured height"
        );

        let mut legacy_genesis = Genesis::new("postfiat-fastlane-root-governed-migration-test");
        legacy_genesis.replicated_state_v2_activation_height = None;
        let amendment = GovernanceAmendment {
            amendment_id: "state-root-v2-at-2".to_owned(),
            chain_id: legacy_genesis.chain_id.clone(),
            genesis_hash: genesis_hash(&legacy_genesis),
            protocol_version: legacy_genesis.protocol_version,
            instance_id: "state-root-migration".to_owned(),
            proposal_id: "state-root-migration-proposal".to_owned(),
            certificate_id: "state-root-migration-certificate".to_owned(),
            proposer: "validator-0".to_owned(),
            validators: vec!["validator-0".to_owned()],
            quorum: 1,
            kind: GOVERNANCE_KIND_REPLICATED_STATE_V2_ACTIVATION_HEIGHT.to_owned(),
            value: 2,
            activation_height: 0,
            veto_until_height: 0,
            paused: false,
            support: vec!["validator-0".to_owned()],
            votes: Vec::new(),
            signed_authorizations: Vec::new(),
        };
        assert!(governance_amendment_lifecycle_rejection(&amendment, 1).is_none());
        assert_eq!(
            governance_amendment_lifecycle_rejection(&amendment, 2)
                .map(|rejection| rejection.0),
            Some("invalid_replicated_state_v2_activation_height")
        );
        let mut migration_governance = GovernanceState::new(1);
        migration_governance.apply(amendment);
        assert_eq!(
            replicated_state_root(
                &legacy_genesis,
                &migration_governance,
                &no_fastlane,
                &before_activation,
                &shielded,
                &bridge,
            )
            .expect("governed legacy empty root before activation"),
            replicated_state_root(
                &legacy_genesis,
                &migration_governance,
                &ordered,
                &before_activation,
                &shielded,
                &bridge,
            )
            .expect("governed legacy FastLane root before activation")
        );
        assert_ne!(
            replicated_state_root(
                &legacy_genesis,
                &migration_governance,
                &no_fastlane,
                &at_activation,
                &shielded,
                &bridge,
            )
            .expect("governed v2 empty root at activation"),
            replicated_state_root(
                &legacy_genesis,
                &migration_governance,
                &ordered,
                &at_activation,
                &shielded,
                &bridge,
            )
            .expect("governed v2 FastLane root at activation")
        );
    }

    #[test]
    fn native_supply_oracle_counts_each_live_custody_lane_once_and_checks_overflow() {
        let mut ledger = LedgerState::new(vec![Account::new("pfaccount", 100, None)]);
        ledger.escrows.push(
            Escrow::new(
                "postfiat-native-supply-test",
                "pfescrowowner",
                1,
                "pfescrowrecipient",
                postfiat_execution::NATIVE_PFT_ESCROW_ASSET_ID,
                7,
                1,
                "condition",
                0,
                0,
                1,
            )
            .expect("native escrow"),
        );
        ledger.offers.push(
            Offer::new(
                "postfiat-native-supply-test",
                "pfofferowner",
                1,
                postfiat_execution::NATIVE_PFT_ESCROW_ASSET_ID,
                11,
                "a".repeat(96),
                5,
                1,
                0,
            )
            .expect("native offer"),
        );
        ledger.owned_objects.push(OwnedObject {
            id: "owned-native".to_owned(),
            version: 1,
            owner_pubkey_hex: "owner-key".to_owned(),
            value: 13,
            asset: postfiat_execution::OWNED_NATIVE_ASSET.to_owned(),
        });
        ledger
            .fast_lane_reserves
            .push(postfiat_types::FastLaneReserveBalanceV1 {
            asset_id: postfiat_types::FastAssetIdV1::native_pft(),
            amount_atoms: 17,
        });
        let mut shielded = ShieldedState::empty();
        let mut orchard = OrchardPoolState::empty("native-orchard");
        orchard.turnstile_deposit_total = 23;
        orchard.fee_burn_total = 2;
        orchard.withdraw_total = 3;
        orchard.value_balance_total = -18;
        shielded.orchard = Some(orchard);

        assert_eq!(
            native_pft_live_total(&ledger, &shielded).expect("native live total"),
            176
        );

        let mut overflow = LedgerState::new(vec![Account::new("pfaccount", 1, None)]);
        overflow
            .fast_lane_reserves
            .push(postfiat_types::FastLaneReserveBalanceV1 {
                asset_id: postfiat_types::FastAssetIdV1::native_pft(),
                amount_atoms: u128::MAX,
            });
        assert!(native_pft_live_total(&overflow, &ShieldedState::empty()).is_err());

        for duplicate_lane in ["account", "escrow", "offer", "owned", "reserve"] {
            let mut duplicate_ledger = ledger.clone();
            match duplicate_lane {
                "account" => duplicate_ledger.accounts.push(duplicate_ledger.accounts[0].clone()),
                "escrow" => duplicate_ledger.escrows.push(duplicate_ledger.escrows[0].clone()),
                "offer" => duplicate_ledger.offers.push(duplicate_ledger.offers[0].clone()),
                "owned" => duplicate_ledger
                    .owned_objects
                    .push(duplicate_ledger.owned_objects[0].clone()),
                "reserve" => duplicate_ledger
                    .fast_lane_reserves
                    .push(duplicate_ledger.fast_lane_reserves[0].clone()),
                _ => unreachable!(),
            }
            let error = native_pft_live_total(&duplicate_ledger, &shielded)
                .expect_err("duplicate custody row must fail closed");
            assert!(error.to_string().contains("duplicate native custody"));
        }

        let mut non_native = LedgerState::new(vec![Account::new("pfaccount", 1, None)]);
        non_native.owned_objects.push(OwnedObject {
            id: "issued-owned".to_string(),
            version: 1,
            owner_pubkey_hex: "owner-key".to_string(),
            value: u64::MAX,
            asset: "pfUSDC".to_string(),
        });
        non_native
            .fast_lane_reserves
            .push(postfiat_types::FastLaneReserveBalanceV1 {
                asset_id: postfiat_types::FastAssetIdV1([9; 48]),
                amount_atoms: u128::MAX,
            });
        assert_eq!(
            native_pft_live_total(&non_native, &ShieldedState::empty())
                .expect("non-native lanes are explicitly excluded"),
            1
        );

        for iteration in 0_u64..256 {
            let account_value = iteration;
            let owned_value = iteration.saturating_mul(3).saturating_add(1);
            let reserve_value = u128::from(iteration).saturating_mul(5);
            let orchard_value = iteration.saturating_mul(7);
            let mut property_ledger =
                LedgerState::new(vec![Account::new("property-account", account_value, None)]);
            property_ledger.owned_objects.push(OwnedObject {
                id: format!("property-owned-{iteration}"),
                version: 1,
                owner_pubkey_hex: "owner-key".to_string(),
                value: owned_value,
                asset: postfiat_execution::OWNED_NATIVE_ASSET.to_string(),
            });
            property_ledger
                .fast_lane_reserves
                .push(postfiat_types::FastLaneReserveBalanceV1 {
                    asset_id: postfiat_types::FastAssetIdV1::native_pft(),
                    amount_atoms: reserve_value,
                });
            let mut property_pool = OrchardPoolState::empty("property-pool");
            property_pool.turnstile_deposit_total = orchard_value;
            let mut property_shielded = ShieldedState::empty();
            property_shielded.orchard = Some(property_pool);
            assert_eq!(
                native_pft_live_total(&property_ledger, &property_shielded)
                    .expect("native custody property case"),
                u128::from(account_value)
                    + u128::from(owned_value)
                    + reserve_value
                    + u128::from(orchard_value)
            );
        }

        let burned = Receipt::accepted("native-burn", "fee burned")
            .with_fee_policy(2, 2, 1, 0);
        verify_native_pft_transition(1, 100, 98, std::slice::from_ref(&burned))
            .expect("exact receipt burn reconciles");
        let mismatch = verify_native_pft_transition(1, 100, 99, &[burned])
            .expect_err("unreported native destruction must fail replay");
        assert!(
            mismatch
                .to_string()
                .contains("native supply conservation failed"),
            "{mismatch}"
        );
    }

    #[test]
    fn asset_orchard_outputs_validate_and_affect_replicated_state_root() {
        let genesis = Genesis::new("postfiat-local");
        let governance = GovernanceState::new(1);
        let ledger = LedgerState::empty();
        let ordered_batches = Vec::<String>::new();
        let bridge = BridgeState::empty();
        let commitment = postfiat_privacy_orchard::AssetOrchardFieldElement::from_field(
            postfiat_privacy_orchard::hash_to_pallas_base(
                "postfiat.node.test.asset_orchard_output",
                b"output-0",
            )
            .expect("commitment field"),
        )
        .as_hex()
        .to_string();
        let commitment_wrapper =
            OrchardOutputCommitment::parse_hex(commitment.clone()).expect("commitment wrapper");
        let root = orchard_anchor_from_commitments(&[commitment_wrapper])
            .expect("asset-orchard output root")
            .as_hex()
            .to_string();
        let mut pool = OrchardPoolState::empty("asset-orchard-v1");
        pool.output_commitments.push(commitment.clone());
        pool.asset_orchard_outputs
            .push(AssetOrchardEncryptedOutputRecord {
                output_commitment: commitment,
                encrypted_output: "ab".repeat(16),
            });
        pool.root_history.push(OrchardRootRecord {
            root: orchard_empty_root_hex(),
            output_count: 0,
        });
        pool.root_history.push(OrchardRootRecord {
            root,
            output_count: 1,
        });
        let mut shielded = ShieldedState::empty();
        shielded.orchard = Some(pool);

        verify_shielded_state(&shielded).expect("asset-orchard output state verifies");
        let root_before = replicated_state_root(
            &genesis,
            &governance,
            &ledger,
            &ordered_batches,
            &shielded,
            &bridge,
        )
        .expect("state root before");

        let mut tampered = shielded.clone();
        tampered
            .orchard
            .as_mut()
            .expect("pool")
            .asset_orchard_outputs[0]
            .encrypted_output = "cd".repeat(16);
        verify_shielded_state(&tampered).expect("tampered blob shape still verifies");
        let root_after = replicated_state_root(
            &genesis,
            &governance,
            &ledger,
            &ordered_batches,
            &tampered,
            &bridge,
        )
        .expect("state root after");

        assert_ne!(root_before, root_after);
    }

    #[test]
    fn replicated_state_root_rejects_issued_supply_hidden_in_orchard_custody() {
        let genesis = Genesis::new("postfiat-local");
        let governance = GovernanceState::new(1);
        let issuer = "pfissuer";
        let holder = "pfholder";
        let mut asset =
            AssetDefinition::new(&genesis.chain_id, issuer, "CAPTEST", 1, 0).expect("asset");
        asset.max_supply = Some(10);
        let mut line = TrustLine::new(holder, issuer, asset.asset_id.clone(), 10, 10)
            .expect("trustline");
        line.balance = 10;
        line.authorized = true;
        let mut ledger = LedgerState::new(vec![
            Account::new(issuer, 1, None),
            Account::new(holder, 1, None),
        ]);
        ledger.asset_definitions.push(asset.clone());
        ledger.trustlines.push(line);

        let mut pool = OrchardPoolState::empty("asset-orchard-v1");
        pool.asset_orchard_balances.push(AssetOrchardAssetBalance {
            asset_id: asset.asset_id,
            ingress_total: 1,
            egress_total: 0,
            live_total: 1,
        });
        let mut shielded = ShieldedState::empty();
        shielded.orchard = Some(pool);

        let error = replicated_state_root(
            &genesis,
            &governance,
            &ledger,
            &[],
            &shielded,
            &BridgeState::empty(),
        )
        .expect_err("global issued supply above max_supply must fail closed");
        assert!(
            error.to_string().contains("issued asset supply cap exceeded"),
            "{error}"
        );

        ledger.trustlines[0].balance = 9;
        replicated_state_root(
            &genesis,
            &governance,
            &ledger,
            &[],
            &shielded,
            &BridgeState::empty(),
        )
        .expect("global issued supply exactly at max_supply remains valid");

        ledger.asset_definitions[0].max_supply = None;
        let mut nav_asset = NavTrackedAsset::new(
            ledger.asset_definitions[0].asset_id.clone(),
            issuer,
            issuer,
            "ledger-transparent",
            "USD",
            issuer,
        )
        .expect("NAV asset");
        nav_asset.finalized_epoch = 1;
        nav_asset.circulating_supply = 9;
        ledger.nav_assets.push(nav_asset);
        let error = replicated_state_root(
            &genesis,
            &governance,
            &ledger,
            &[],
            &shielded,
            &BridgeState::empty(),
        )
        .expect_err("private custody must count against finalized NAV supply");
        assert!(
            error
                .to_string()
                .contains("exceeds finalized NAV circulating supply"),
            "{error}"
        );
    }

    #[test]
    fn global_issued_supply_inventory_rejects_duplicate_unknown_and_unsupported_lanes() {
        let genesis = Genesis::new("postfiat-issued-inventory-test");
        let governance = GovernanceState::new(1);
        let bridge = BridgeState::empty();
        let issuer = "pfissuer";
        let holder = "pfholder";
        let asset = AssetDefinition::new(&genesis.chain_id, issuer, "INV", 1, 0)
            .expect("issued asset");
        let state_root_error = |ledger: &LedgerState, shielded: &ShieldedState| {
            replicated_state_root(
                &genesis,
                &governance,
                ledger,
                &[],
                shielded,
                &bridge,
            )
            .expect_err("invalid issued custody inventory must fail state commitment")
        };

        let mut duplicate_definition = LedgerState::empty();
        duplicate_definition.asset_definitions = vec![asset.clone(), asset.clone()];
        assert!(state_root_error(&duplicate_definition, &ShieldedState::empty())
            .to_string()
            .contains("duplicate issued asset definition"));

        let mut line = TrustLine::new(holder, issuer, asset.asset_id.clone(), 10, 5)
            .expect("trustline");
        line.authorized = true;
        let mut duplicate_trustline = LedgerState::empty();
        duplicate_trustline.asset_definitions.push(asset.clone());
        duplicate_trustline.trustlines = vec![line.clone(), line];
        assert!(state_root_error(&duplicate_trustline, &ShieldedState::empty())
            .to_string()
            .contains("duplicate issued trustline"));

        let mut duplicate_reserve = LedgerState::empty();
        duplicate_reserve.asset_definitions.push(asset.clone());
        let issued_fast_id = postfiat_types::FastAssetIdV1(
            postfiat_crypto_provider::hex_to_bytes(&asset.asset_id)
                .expect("asset hex")
                .try_into()
                .expect("asset id width"),
        );
        let reserve = postfiat_types::FastLaneReserveBalanceV1 {
            asset_id: issued_fast_id,
            amount_atoms: 1,
        };
        duplicate_reserve.fast_lane_reserves = vec![reserve.clone(), reserve];
        assert!(state_root_error(&duplicate_reserve, &ShieldedState::empty())
            .to_string()
            .contains("duplicate FastLane reserve"));

        let mut unsupported_owned = LedgerState::empty();
        unsupported_owned.asset_definitions.push(asset.clone());
        unsupported_owned.owned_objects.push(OwnedObject {
            id: "issued-owned-object".to_string(),
            version: 1,
            owner_pubkey_hex: "owner-key".to_string(),
            value: 1,
            asset: asset.asset_id.clone(),
        });
        assert!(state_root_error(&unsupported_owned, &ShieldedState::empty())
            .to_string()
            .contains("unsupported issued owned-object custody"));

        let mut unknown_trustline = LedgerState::empty();
        unknown_trustline.trustlines.push(
            TrustLine::new(holder, issuer, "ab".repeat(48), 10, 1)
                .expect("unknown-asset trustline"),
        );
        assert!(state_root_error(&unknown_trustline, &ShieldedState::empty())
            .to_string()
            .contains("trustline references unknown issued asset"));
    }

    #[test]
    fn issued_supply_complete_customer_custody_flow_counts_all_lanes_together() {
        let genesis = Genesis::new("postfiat-issued-customer-flow-test");
        let issuer_key = ml_dsa_65_keygen_from_seed(&[0xa1; 32]);
        let holder_key = ml_dsa_65_keygen_from_seed(&[0xa2; 32]);
        let issuer = address_from_public_key(&issuer_key.public_key);
        let holder = address_from_public_key(&holder_key.public_key);

        let mut asset =
            AssetDefinition::new(&genesis.chain_id, &issuer, "FLOW", 1, 0).expect("asset");
        asset.max_supply = Some(100);
        let settlement = AssetDefinition::new(&genesis.chain_id, &issuer, "USD", 1, 0)
            .expect("settlement asset");
        let mut holder_line =
            TrustLine::new(&holder, &issuer, asset.asset_id.clone(), 200, 30)
                .expect("holder trustline");
        holder_line.balance = 30;
        holder_line.authorized = true;

        let mut ledger = LedgerState::new(vec![
            Account::new(
                issuer.clone(),
                10_000,
                Some(bytes_to_hex(&issuer_key.public_key)),
            ),
            Account::new(
                holder.clone(),
                10_000,
                Some(bytes_to_hex(&holder_key.public_key)),
            ),
        ]);
        ledger.asset_definitions = vec![asset.clone(), settlement.clone()];
        ledger.trustlines.push(holder_line);
        ledger.nav_assets.push(
            postfiat_types::NavTrackedAsset::new(
                asset.asset_id.clone(),
                issuer.clone(),
                issuer.clone(),
                "issued-customer-flow".to_string(),
                "USD".to_string(),
                issuer.clone(),
            )
            .expect("NAV asset"),
        );
        let fast_asset_id = postfiat_types::FastAssetIdV1(
            hex_to_bytes(&asset.asset_id)
                .expect("asset id hex")
                .try_into()
                .expect("48-byte asset id"),
        );
        ledger
            .fast_lane_reserves
            .push(postfiat_types::FastLaneReserveBalanceV1 {
                asset_id: fast_asset_id,
                amount_atoms: 20,
            });
        ledger
            .pftl_uniswap_routes
            .push(postfiat_types::PftlUniswapConsensusRouteState {
                route_id: "issued-customer-flow-route".to_string(),
                route_family: postfiat_types::PFTL_UNISWAP_ROUTE_FAMILY_PRIMARY_MINT.to_string(),
                route_config_digest: "11".repeat(48),
                route_trust_class: "BFT_CHECKPOINT".to_string(),
                native_nav_asset_id: asset.asset_id.clone(),
                settlement_asset_id: settlement.asset_id,
                handoff_controller: "0x1111111111111111111111111111111111111111".to_string(),
                settlement_adapter: "0x2222222222222222222222222222222222222222".to_string(),
                wrapped_navcoin_token: "0x3333333333333333333333333333333333333333".to_string(),
                ethereum_chain_id: 1,
                route_supply_cap_atoms: 100,
                packet_notional_cap_atoms: 100,
                latest_finalized_nav_epoch: 0,
                return_finality_blocks: 12,
                ethereum_verification_policy: None,
                authorized_valid_supply_atoms: 25,
                pftl_spendable_supply_atoms: 0,
                native_spendable_balances_atoms: std::collections::BTreeMap::new(),
                ethereum_spendable_supply_atoms: 25,
                other_registered_venue_supply_atoms: 0,
                outstanding_bridge_claims_atoms: 0,
                pending_return_import_claims_atoms: 0,
                settlement_reserve_atoms: 0,
                primary_subscription_nonces: std::collections::BTreeMap::new(),
                export_packets: std::collections::BTreeMap::new(),
                export_nonces: std::collections::BTreeMap::new(),
                return_imports: std::collections::BTreeMap::new(),
                paused: false,
            });

        let mut pool = OrchardPoolState::empty(ASSET_ORCHARD_POOL_ID_V1);
        pool.asset_orchard_balances.push(AssetOrchardAssetBalance {
            asset_id: asset.asset_id.clone(),
            ingress_total: 25,
            egress_total: 0,
            live_total: 25,
        });
        let mut shielded = ShieldedState::empty();
        shielded.orchard = Some(pool);

        let assert_exact_cap = |ledger: &LedgerState, shielded: &ShieldedState| {
            assert_eq!(
                global_issued_asset_supply(ledger, shielded, &asset.asset_id)
                    .expect("complete issued custody supply"),
                100
            );
            verify_global_issued_asset_supply_caps(ledger, shielded)
                .expect("complete issued custody remains exactly at cap");
        };
        assert_exact_cap(&ledger, &shielded);

        // Exercise the exact mixed-custody mint-admission boundary. Execution
        // alone sees public + FastLane + external custody (75), while node
        // admission must add the 25 private atoms and reject 101 before the
        // candidate ledger can replace canonical state.
        let issue = signed_asset_transaction_for_test(
            &genesis,
            &ledger,
            &issuer,
            &bytes_to_hex(&issuer_key.public_key),
            &bytes_to_hex(&issuer_key.private_key),
            ISSUED_PAYMENT_TRANSACTION_KIND,
            1,
            AssetTransactionOperation::IssuedPayment(IssuedPaymentOperation {
                from: issuer.clone(),
                to: holder,
                issuer: issuer.clone(),
                asset_id: asset.asset_id.clone(),
                amount: 1,
            }),
        );
        let canonical_before = ledger.clone();
        let mut dry_run = ledger.clone();
        let execution_receipt = execute_asset_transaction(&genesis, &mut dry_run, &issue, 1);
        assert!(execution_receipt.accepted, "{execution_receipt:?}");
        let cap_error = verify_global_issued_asset_supply_caps(&dry_run, &shielded)
            .expect_err("private custody must close false mint headroom");
        assert!(
            cap_error.to_string().contains("issued asset supply cap exceeded"),
            "{cap_error}"
        );
        assert_eq!(ledger, canonical_before, "failed admission must not mutate canonical state");

        // Compose the four supported customer custody lanes. Each move is
        // supply-neutral and the global oracle remains exact at every step;
        // the individual production transition tests guard the corresponding
        // signed deposit/redeem, encrypted ingress/egress, and BFT-checkpoint
        // export/return boundaries.
        ledger.trustlines[0].balance -= 1;
        ledger.fast_lane_reserves[0].amount_atoms += 1;
        assert_exact_cap(&ledger, &shielded);

        ledger.fast_lane_reserves[0].amount_atoms -= 1;
        shielded
            .orchard
            .as_mut()
            .expect("orchard")
            .asset_orchard_balances[0]
            .live_total += 1;
        assert_exact_cap(&ledger, &shielded);

        shielded
            .orchard
            .as_mut()
            .expect("orchard")
            .asset_orchard_balances[0]
            .live_total -= 1;
        ledger.pftl_uniswap_routes[0].ethereum_spendable_supply_atoms += 1;
        ledger.pftl_uniswap_routes[0].authorized_valid_supply_atoms += 1;
        assert_exact_cap(&ledger, &shielded);

        ledger.pftl_uniswap_routes[0].ethereum_spendable_supply_atoms -= 1;
        ledger.pftl_uniswap_routes[0].authorized_valid_supply_atoms -= 1;
        ledger.trustlines[0].balance += 1;
        assert_exact_cap(&ledger, &shielded);
    }

    #[test]
    fn orchard_frontier_cache_does_not_affect_replicated_state_root() {
        let genesis = Genesis::new("postfiat-local");
        let governance = GovernanceState::new(1);
        let ledger = LedgerState::empty();
        let ordered_batches = Vec::<String>::new();
        let bridge = BridgeState::empty();
        let commitment = postfiat_privacy_orchard::AssetOrchardFieldElement::from_field(
            hash_to_pallas_base(
                "postfiat.node.test.orchard_frontier_cache_root",
                b"output-0",
            )
            .expect("commitment field"),
        )
        .as_hex()
        .to_string();
        let commitment_wrapper =
            OrchardOutputCommitment::parse_hex(commitment.clone()).expect("commitment wrapper");
        let root = orchard_anchor_from_commitments(&[commitment_wrapper])
            .expect("asset-orchard output root")
            .as_hex()
            .to_string();
        let mut pool = OrchardPoolState::empty("asset-orchard-v1");
        pool.output_commitments.push(commitment.clone());
        pool.asset_orchard_outputs
            .push(AssetOrchardEncryptedOutputRecord {
                output_commitment: commitment,
                encrypted_output: "ab".repeat(16),
            });
        pool.root_history.push(OrchardRootRecord {
            root: orchard_empty_root_hex(),
            output_count: 0,
        });
        pool.root_history.push(OrchardRootRecord {
            root: root.clone(),
            output_count: 1,
        });
        let mut shielded = ShieldedState::empty();
        shielded.orchard = Some(pool);

        let root_without_cache = replicated_state_root(
            &genesis,
            &governance,
            &ledger,
            &ordered_batches,
            &shielded,
            &bridge,
        )
        .expect("state root without cache");

        let mut cached = shielded.clone();
        cached.orchard.as_mut().expect("pool").frontier_cache =
            Some(OrchardFrontierCache {
                output_count: 1,
                root,
                latest_leaf: Some("11".repeat(32)),
                ommers: vec!["22".repeat(32)],
            });
        let root_with_cache = replicated_state_root(
            &genesis,
            &governance,
            &ledger,
            &ordered_batches,
            &cached,
            &bridge,
        )
        .expect("state root with cache");

        assert_eq!(root_without_cache, root_with_cache);
    }

    #[test]
    fn orchard_frontier_cache_malformed_parts_fall_back_to_full_recompute() {
        let commitment_a = postfiat_privacy_orchard::AssetOrchardFieldElement::from_field(
            hash_to_pallas_base(
                "postfiat.node.test.orchard_frontier_cache_fallback",
                b"output-a",
            )
            .expect("commitment field a"),
        )
        .as_hex()
        .to_string();
        let commitment_b = postfiat_privacy_orchard::AssetOrchardFieldElement::from_field(
            hash_to_pallas_base(
                "postfiat.node.test.orchard_frontier_cache_fallback",
                b"output-b",
            )
            .expect("commitment field b"),
        )
        .as_hex()
        .to_string();
        let commitment_a_wrapper =
            OrchardOutputCommitment::parse_hex(commitment_a.clone()).expect("commitment a");
        let commitment_b_wrapper =
            OrchardOutputCommitment::parse_hex(commitment_b.clone()).expect("commitment b");
        let first_root = orchard_anchor_from_commitments(std::slice::from_ref(&commitment_a_wrapper))
            .expect("first root")
            .as_hex()
            .to_string();
        let second_root =
            orchard_anchor_from_commitments(&[commitment_a_wrapper, commitment_b_wrapper])
                .expect("second root")
                .as_hex()
                .to_string();

        let mut pool = OrchardPoolState::empty("asset-orchard-v1");
        pool.output_commitments.push(commitment_a);
        pool.root_history.push(OrchardRootRecord {
            root: orchard_empty_root_hex(),
            output_count: 0,
        });
        pool.root_history.push(OrchardRootRecord {
            root: first_root.clone(),
            output_count: 1,
        });
        pool.frontier_cache = Some(OrchardFrontierCache {
            output_count: 1,
            root: first_root,
            latest_leaf: Some("zz".repeat(32)),
            ommers: Vec::new(),
        });
        pool.output_commitments.push(commitment_b);

        append_orchard_current_root(&mut pool).expect("append with malformed cache fallback");

        assert_eq!(
            pool.root_history.last().expect("latest root").root,
            second_root
        );
        let cache = pool.frontier_cache.expect("fallback writes fresh cache");
        assert_eq!(cache.output_count, 2);
        assert_eq!(cache.root, second_root);
        assert!(cache.latest_leaf.is_some());
    }

    #[test]
    fn orchard_frontier_cache_ahead_of_outputs_is_rejected() {
        let commitment = postfiat_privacy_orchard::AssetOrchardFieldElement::from_field(
            hash_to_pallas_base(
                "postfiat.node.test.orchard_frontier_cache_ahead",
                b"output-0",
            )
            .expect("commitment field"),
        )
        .as_hex()
        .to_string();
        let commitment_wrapper =
            OrchardOutputCommitment::parse_hex(commitment.clone()).expect("commitment wrapper");
        let root = orchard_anchor_from_commitments(&[commitment_wrapper])
            .expect("asset-orchard output root")
            .as_hex()
            .to_string();
        let mut pool = OrchardPoolState::empty("asset-orchard-v1");
        pool.output_commitments.push(commitment);
        pool.root_history.push(OrchardRootRecord {
            root: orchard_empty_root_hex(),
            output_count: 0,
        });
        pool.root_history.push(OrchardRootRecord {
            root: root.clone(),
            output_count: 1,
        });
        pool.frontier_cache = Some(OrchardFrontierCache {
            output_count: 2,
            root: "11".repeat(32),
            latest_leaf: Some("22".repeat(32)),
            ommers: Vec::new(),
        });

        assert!(!orchard_frontier_cache_matches_history(
            &pool,
            pool.frontier_cache.as_ref().expect("cache")
        ));
        assert_eq!(
            orchard_pool_current_root(&pool).expect("current root falls back"),
            root
        );
    }

    #[test]
    fn orchard_pool_current_root_appends_suffix_for_stale_valid_cache() {
        let commitment_a = postfiat_privacy_orchard::AssetOrchardFieldElement::from_field(
            hash_to_pallas_base(
                "postfiat.node.test.orchard_frontier_cache_suffix",
                b"output-a",
            )
            .expect("commitment field a"),
        )
        .as_hex()
        .to_string();
        let commitment_b = postfiat_privacy_orchard::AssetOrchardFieldElement::from_field(
            hash_to_pallas_base(
                "postfiat.node.test.orchard_frontier_cache_suffix",
                b"output-b",
            )
            .expect("commitment field b"),
        )
        .as_hex()
        .to_string();
        let commitment_a_wrapper =
            OrchardOutputCommitment::parse_hex(commitment_a.clone()).expect("commitment a");
        let commitment_b_wrapper =
            OrchardOutputCommitment::parse_hex(commitment_b.clone()).expect("commitment b");
        let first_snapshot = orchard_frontier_snapshot_from_commitments(std::slice::from_ref(
            &commitment_a_wrapper,
        ))
        .expect("first snapshot");
        let first_root = first_snapshot.root.clone();
        let second_root =
            orchard_anchor_from_commitments(&[commitment_a_wrapper, commitment_b_wrapper])
                .expect("second root")
                .as_hex()
                .to_string();
        let mut pool = OrchardPoolState::empty("asset-orchard-v1");
        pool.output_commitments.push(commitment_a);
        pool.output_commitments.push(commitment_b);
        pool.root_history.push(OrchardRootRecord {
            root: orchard_empty_root_hex(),
            output_count: 0,
        });
        pool.root_history.push(OrchardRootRecord {
            root: first_root,
            output_count: 1,
        });
        pool.frontier_cache = Some(orchard_frontier_cache_from_snapshot(first_snapshot));

        assert_eq!(
            orchard_pool_current_root(&pool).expect("current root appends suffix"),
            second_root
        );
    }

    fn asset_orchard_test_field(label: &[u8]) -> AssetOrchardFieldElement {
        AssetOrchardFieldElement::from_field(
            hash_to_pallas_base("postfiat.node.test.asset_orchard_apply", label)
                .expect("asset-orchard test field"),
        )
    }

    fn asset_orchard_accounting_record(
        output_commitment: &AssetOrchardFieldElement,
        asset_id: &str,
        amount: u64,
        blinding_seed: &[u8],
    ) -> AssetOrchardSwapAccountingRecord {
        let tag = AssetTag::derive(asset_id).expect("asset tag");
        let blinding = hash_to_pallas_scalar_nonzero(
            "postfiat.node.test.asset_orchard_accounting_blinding",
            blinding_seed,
        )
        .expect("accounting blinding");
        build_asset_orchard_accounting_record(output_commitment, tag, amount, blinding)
            .expect("accounting record")
    }

    fn asset_orchard_conserving_accounting(
        output_commitments: &[AssetOrchardFieldElement],
    ) -> (
        Vec<AssetOrchardSwapAccountingRecord>,
        Vec<AssetOrchardSwapAccountingRecord>,
    ) {
        (
            vec![
                asset_orchard_accounting_record(
                    &asset_orchard_test_field(b"input-0"),
                    "a651",
                    5,
                    b"a651",
                ),
                asset_orchard_accounting_record(
                    &asset_orchard_test_field(b"input-1"),
                    "pfUSDC",
                    9,
                    b"pfUSDC",
                ),
            ],
            vec![
                asset_orchard_accounting_record(
                    &output_commitments[0],
                    "pfUSDC",
                    9,
                    b"pfUSDC",
                ),
                asset_orchard_accounting_record(&output_commitments[1], "a651", 5, b"a651"),
            ],
        )
    }

    fn asset_orchard_swapped_tag_unswapped_value_accounting(
        output_commitments: &[AssetOrchardFieldElement],
    ) -> Vec<AssetOrchardSwapAccountingRecord> {
        vec![
            asset_orchard_accounting_record(&output_commitments[0], "pfUSDC", 5, b"a651"),
            asset_orchard_accounting_record(&output_commitments[1], "a651", 9, b"pfUSDC"),
        ]
    }

    fn asset_orchard_test_encrypted_outputs() -> Vec<AssetOrchardBoundedBytes> {
        vec![
            AssetOrchardBoundedBytes::from_bytes(
                b"asset-orchard-output-0",
                ASSET_ORCHARD_ENCRYPTED_OUTPUT_MAX_BYTES,
            )
            .expect("encrypted output 0"),
            AssetOrchardBoundedBytes::from_bytes(
                b"asset-orchard-output-1",
                ASSET_ORCHARD_ENCRYPTED_OUTPUT_MAX_BYTES,
            )
            .expect("encrypted output 1"),
        ]
    }

    fn asset_orchard_test_action_and_verified(
        encrypted_outputs: Vec<AssetOrchardBoundedBytes>,
    ) -> (AssetOrchardSwapAction, VerifiedAssetOrchardSwap) {
        let pool_domain = asset_orchard_test_field(b"pool-domain");
        let anchor =
            AssetOrchardFieldElement::parse_hex(orchard_empty_root_hex()).expect("empty anchor");
        let nullifiers = vec![
            asset_orchard_test_field(b"nullifier-0"),
            asset_orchard_test_field(b"nullifier-1"),
        ];
        let output_commitments = vec![
            asset_orchard_test_field(b"output-0"),
            asset_orchard_test_field(b"output-1"),
        ];
        let (accounting_inputs, accounting_outputs) =
            asset_orchard_conserving_accounting(&output_commitments);
        let swap_binding_hash = AssetOrchardSwapBindingHash::from_bytes(&[7u8; 64]);
        let action = AssetOrchardSwapAction {
            version: ASSET_ORCHARD_ACTION_VERSION_V1,
            schema: ASSET_ORCHARD_ACTION_SCHEMA_V1.to_string(),
            pool_id: ASSET_ORCHARD_POOL_ID_V1.to_string(),
            proof_system_id: ASSET_ORCHARD_PROOF_SYSTEM_ID_V1.to_string(),
            circuit_id: ASSET_ORCHARD_CIRCUIT_ID_V1.to_string(),
            pool_domain: pool_domain.clone(),
            anchor: anchor.clone(),
            nullifiers: nullifiers.clone(),
            randomized_verification_keys: Vec::new(),
            output_commitments: output_commitments.clone(),
            encrypted_outputs: encrypted_outputs.clone(),
            accounting_inputs: accounting_inputs.clone(),
            accounting_outputs: accounting_outputs.clone(),
            pricing_claim: AssetOrchardPricingClaim {
                nav_epoch: 59,
                reserve_packet_hash: "ab".repeat(48),
                ratio_numerator: 9,
                ratio_denominator: 5,
                mode: "at_nav_with_band".to_string(),
                band_bps: 0,
                base_asset_tag_lo: AssetTag::derive("a651").unwrap().lo,
                base_asset_tag_hi: AssetTag::derive("a651").unwrap().hi,
                quote_asset_tag_lo: AssetTag::derive("pfUSDC").unwrap().lo,
                quote_asset_tag_hi: AssetTag::derive("pfUSDC").unwrap().hi,
            },
            swap_binding_hash: swap_binding_hash.clone(),
            fee: 0,
            proof: AssetOrchardProofBytes::from_bytes(&[1u8]).expect("proof placeholder"),
            spend_authorization_signatures: Vec::<AssetOrchardSpendAuthSignature>::new(),
        };
        let verified = VerifiedAssetOrchardSwap {
            proof_system_id: ASSET_ORCHARD_PROOF_SYSTEM_ID_V1.to_string(),
            circuit_id: ASSET_ORCHARD_CIRCUIT_ID_V1.to_string(),
            pool_domain,
            anchor,
            nullifiers,
            randomized_verification_keys: Vec::new(),
            output_commitments,
            encrypted_outputs,
            accounting_inputs,
            accounting_outputs,
            pricing: VerifiedAssetOrchardPricingClaim {
                claim: action.pricing_claim.clone(),
                action_binding_hash: swap_binding_hash.clone(),
                provenance: AssetOrchardPricingClaimProvenance::CircuitProven,
            },
            swap_binding_hash,
            fee: 0,
        };
        (action, verified)
    }

    #[test]
    fn asset_orchard_swap_accounting_rejects_per_asset_total_change_even_if_verified() {
        let genesis = Genesis::new("postfiat-local");
        let mut shielded = ShieldedState::empty();
        shielded.orchard = Some(OrchardPoolState::empty(ASSET_ORCHARD_POOL_ID_V1));
        verify_shielded_state(&shielded).expect("initial state verifies");
        let before = shielded.clone();

        let (mut action, mut verified) =
            asset_orchard_test_action_and_verified(asset_orchard_test_encrypted_outputs());
        let nonconserving =
            asset_orchard_swapped_tag_unswapped_value_accounting(&verified.output_commitments);
        action.accounting_outputs = nonconserving.clone();
        verified.accounting_outputs = nonconserving;

        let receipt = apply_verified_asset_orchard_swap_action_to_state(
            &genesis,
            &mut shielded,
            &action,
            &verified,
        )
        .expect("non-conserved accounting returns a rejection receipt");

        assert!(!receipt.accepted);
        assert_eq!(receipt.code, "asset_orchard_accounting_not_conserved");
        assert_eq!(
            shielded, before,
            "rejected asset-orchard accounting must not mutate shielded state"
        );
        verify_shielded_state(&shielded).expect("rejected state still verifies");
    }

    #[test]
    fn asset_orchard_swap_accounting_accepts_conserving_verified_swap() {
        let genesis = Genesis::new("postfiat-local");
        let mut shielded = ShieldedState::empty();
        shielded.orchard = Some(OrchardPoolState::empty(ASSET_ORCHARD_POOL_ID_V1));
        verify_shielded_state(&shielded).expect("initial state verifies");

        let (action, verified) =
            asset_orchard_test_action_and_verified(asset_orchard_test_encrypted_outputs());
        let receipt = apply_verified_asset_orchard_swap_action_to_state(
            &genesis,
            &mut shielded,
            &action,
            &verified,
        )
        .expect("conserved accounting applies");

        assert!(receipt.accepted);
        assert_eq!(receipt.code, "accepted");
        verify_shielded_state(&shielded).expect("accepted state verifies");
    }

    #[test]
    fn asset_orchard_swap_action_accounting_hides_cleartext_asset_tags_and_amounts() {
        let (action, _) =
            asset_orchard_test_action_and_verified(asset_orchard_test_encrypted_outputs());
        let json = serde_json::to_value(&action).expect("action json");
        for side in ["accounting_inputs", "accounting_outputs"] {
            let records = json
                .get(side)
                .and_then(serde_json::Value::as_array)
                .expect("accounting array");
            for record in records {
                let object = record.as_object().expect("accounting record object");
                assert!(object.contains_key("output_commitment"));
                assert!(object.contains_key("value_commitment"));
                assert!(
                    !object.contains_key("asset_commitment"),
                    "{side} must not serialize deterministic asset_commitment"
                );
                assert!(
                    !object.contains_key("asset_tag_lo"),
                    "{side} must not serialize cleartext asset_tag_lo"
                );
                assert!(
                    !object.contains_key("asset_tag_hi"),
                    "{side} must not serialize cleartext asset_tag_hi"
                );
                assert!(
                    !object.contains_key("amount"),
                    "{side} must not serialize cleartext amount"
                );
            }
        }
    }

    #[test]
    fn asset_orchard_swap_apply_failure_rolls_back_state() {
        let genesis = Genesis::new("postfiat-local");
        let mut shielded = ShieldedState::empty();
        shielded.orchard = Some(OrchardPoolState::empty(ASSET_ORCHARD_POOL_ID_V1));
        verify_shielded_state(&shielded).expect("initial state verifies");
        let before = shielded.clone();

        let (malformed_action, malformed_verified) =
            asset_orchard_test_action_and_verified(Vec::new());
        let error = apply_verified_asset_orchard_swap_action_to_state(
            &genesis,
            &mut shielded,
            &malformed_action,
            &malformed_verified,
        )
        .expect_err("missing encrypted output records must fail state verification");
        assert!(
            error
                .to_string()
                .contains("encrypted output records do not cover output commitment count"),
            "unexpected state verification error: {error}"
        );
        assert_eq!(
            shielded, before,
            "failed asset-orchard apply must restore shielded state"
        );
        verify_shielded_state(&shielded).expect("rolled-back state verifies");

        let (valid_action, valid_verified) =
            asset_orchard_test_action_and_verified(asset_orchard_test_encrypted_outputs());
        let receipt = apply_verified_asset_orchard_swap_action_to_state(
            &genesis,
            &mut shielded,
            &valid_action,
            &valid_verified,
        )
        .expect("subsequent valid apply must not be poisoned");
        assert!(receipt.accepted);
        verify_shielded_state(&shielded).expect("accepted state verifies");
    }

    #[test]
    fn replicated_state_root_commits_nav_profiles_and_packet_evidence() {
        let genesis = Genesis::new("postfiat-local");
        let governance = GovernanceState::new(1);
        let ordered_batches = Vec::<String>::new();
        let shielded = ShieldedState::empty();
        let bridge = BridgeState::empty();
        let root_for = |ledger: &LedgerState| {
            replicated_state_root(
                &genesis,
                &governance,
                ledger,
                &ordered_batches,
                &shielded,
                &bridge,
            )
            .expect("state root")
        };

        let empty_root = root_for(&LedgerState::empty());

        let mut with_profile = LedgerState::empty();
        with_profile.nav_proof_profiles.push(
            NavProofProfile::new(
                "pfissuer",
                "sp1-groth16",
                "a651-sp1",
                100_000,
                1,
                100_000,
                0,
                0,
                0,
                0,
                "22".repeat(32),
                format!("0x{}", "11".repeat(32)),
                "groth16",
                0,
                0,
            )
            .expect("sp1 profile"),
        );
        assert_ne!(empty_root, root_for(&with_profile));

        let mut base_packet = NavReservePacket {
            packet_id: "01".repeat(48),
            asset_id: "02".repeat(48),
            issuer: "pfissuer".to_string(),
            submitter: "pfissuer".to_string(),
            epoch: 7,
            nav_per_unit: 3,
            circulating_supply: 10,
            verified_net_assets: 31,
            proof_profile: "03".repeat(48),
            source_root: "04".repeat(48),
            attestor_root: "05".repeat(48),
            reserve_packet_hash: "06".repeat(48),
            state: "pending".to_string(),
            challenge_hash: String::new(),
            submitted_at_height: 0,
            reserve_accounts: Vec::new(),
            challenger: String::new(),
            challenge_bond: 0,
            attestations: Vec::new(),
            sp1_proof_bytes: Vec::new(),
            sp1_public_values: Vec::new(),
        };
        let mut without_evidence = LedgerState::empty();
        without_evidence.nav_reserve_packets.push(base_packet.clone());
        let without_evidence_root = root_for(&without_evidence);

        base_packet.submitted_at_height = 9;
        base_packet.sp1_proof_bytes = vec![1, 2, 3];
        base_packet.sp1_public_values = vec![4, 5, 6];
        let mut with_evidence = LedgerState::empty();
        with_evidence.nav_reserve_packets.push(base_packet);
        assert_ne!(without_evidence_root, root_for(&with_evidence));
    }

    #[test]
    fn shielded_tree_root_commits_to_chain_domain() {
        let shielded = ShieldedState::empty();
        let local_genesis = Genesis::new("postfiat-local");
        let other_genesis = Genesis::new("postfiat-other");
        let local_root =
            chain_bound_shielded_tree_root(&local_genesis, &shielded).expect("local shielded root");
        let other_root =
            chain_bound_shielded_tree_root(&other_genesis, &shielded).expect("other shielded root");

        assert_eq!(local_root.len(), 96);
        assert_ne!(local_root, other_root);
    }

    #[test]
    fn direct_shielded_mint_creator_commits_to_chain_domain() {
        let local_genesis = Genesis::new("postfiat-local");
        let other_genesis = Genesis::new("postfiat-other");
        let local_creator = direct_shielded_mint_creator(&local_genesis);
        let other_creator = direct_shielded_mint_creator(&other_genesis);
        assert_ne!(local_creator, other_creator);

        let mut local_state = ShieldedState::empty();
        let mut other_state = ShieldedState::empty();
        let local_note = mint_debug_note_with_creator(
            &mut local_state,
            "alice",
            "POSTFIAT",
            10,
            "memo",
            local_creator,
        )
        .expect("local mint");
        let other_note = mint_debug_note_with_creator(
            &mut other_state,
            "alice",
            "POSTFIAT",
            10,
            "memo",
            other_creator,
        )
        .expect("other mint");

        assert_ne!(local_note.note_id, other_note.note_id);
        assert_ne!(
            debug_nullifier(&local_note.note_id),
            debug_nullifier(&other_note.note_id)
        );
    }

    #[test]
    fn orchard_operator_policy_reports_limits_and_warnings() {
        let data_dir = unique_test_dir("postfiat-orchard-operator-policy-test");
        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-orchard-operator-policy".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 1,
        })
        .expect("init operator policy test");

        let report = orchard_operator_policy(OrchardOperatorPolicyOptions {
            data_dir: data_dir.clone(),
            privacy_enabled: true,
            max_concurrent_verifiers: DEFAULT_ORCHARD_VERIFIER_MAX_CONCURRENCY,
            verifier_timeout_ms: DEFAULT_ORCHARD_VERIFIER_TIMEOUT_MS,
            root_retention_roots: DEFAULT_ORCHARD_ROOT_RETENTION,
            indexing_role: "local".to_string(),
        })
        .expect("operator policy report");
        assert_eq!(report.schema, ORCHARD_OPERATOR_POLICY_REPORT_SCHEMA);
        assert_eq!(report.chain_id, "postfiat-orchard-operator-policy");
        assert!(report.privacy_enabled);
        assert_eq!(
            report.protocol_limits.max_proof_bytes,
            ORCHARD_PROOF_MAX_BYTES
        );
        assert_eq!(
            report.protocol_limits.max_ciphertext_blob_bytes,
            ORCHARD_CIPHERTEXT_MAX_BYTES
        );
        assert_eq!(
            report.protocol_limits.max_actions_per_orchard_bundle,
            DEFAULT_MAX_ORCHARD_ACTIONS
        );
        assert!(report.enforcement.protocol_size_bounds_enforced);
        assert!(report.enforcement.verifier_runs_in_process);
        assert!(!report.enforcement.verifier_timeout_enforced_in_process);
        assert!(
            report
                .enforcement
                .rpc_child_timeout_available_for_remote_batch_create
        );
        assert!(report.enforcement.remote_batch_create_requires_action_json);
        assert!(
            report
                .enforcement
                .remote_batch_create_uses_server_controlled_spool
        );
        assert!(report.enforcement.remote_batch_create_rate_limited);
        assert!(report.enforcement.remote_batch_create_concurrency_limited);
        assert!(report
            .warnings
            .iter()
            .any(|warning| warning.contains("do not expose public write RPC")));

        let bad_policy = orchard_operator_policy(OrchardOperatorPolicyOptions {
            data_dir: data_dir.clone(),
            privacy_enabled: false,
            max_concurrent_verifiers: 0,
            verifier_timeout_ms: DEFAULT_ORCHARD_VERIFIER_TIMEOUT_MS,
            root_retention_roots: DEFAULT_ORCHARD_ROOT_RETENTION,
            indexing_role: "local".to_string(),
        })
        .expect_err("zero verifier concurrency must fail");
        assert!(bad_policy
            .to_string()
            .contains("max concurrent Orchard verifiers"));

        std::fs::remove_dir_all(data_dir).expect("cleanup operator policy test");
    }

    #[test]
    fn orchard_fee_resource_policy_reports_schedule_and_bounds() {
        let data_dir = unique_test_dir("postfiat-orchard-fee-resource-policy-test");
        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-orchard-fee-resource-policy".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 1,
        })
        .expect("init fee resource policy test");

        let report = orchard_fee_resource_policy(OrchardFeeResourcePolicyOptions {
            data_dir: data_dir.clone(),
        })
        .expect("fee resource policy report");

        assert_eq!(report.schema, ORCHARD_FEE_RESOURCE_POLICY_REPORT_SCHEMA);
        assert_eq!(report.chain_id, "postfiat-orchard-fee-resource-policy");
        assert_eq!(report.pool_id, ORCHARD_DEFAULT_POOL_ID);
        assert!(report.passed);
        assert_eq!(
            report
                .orchard_resource_fee_schedule
                .minimum_orchard_resource_fee,
            ORCHARD_FEE_BURN_MIN_FEE
        );
        assert_eq!(
            report
                .orchard_resource_fee_schedule
                .orchard_fee_byte_quantum,
            ORCHARD_FEE_BURN_BYTE_QUANTUM as u64
        );
        assert_eq!(
            report.resource_bounds.max_actions_per_orchard_bundle,
            DEFAULT_MAX_ORCHARD_ACTIONS
        );
        assert_eq!(
            report.resource_bounds.max_proof_bytes,
            ORCHARD_PROOF_MAX_BYTES
        );
        assert_eq!(report.flow_fee_schedule.len(), 3);
        assert!(report
            .flow_fee_schedule
            .iter()
            .any(|flow| flow.operation == "transparent_to_orchard_deposit"));
        assert!(!report.anti_spam_policy.public_write_edge_allowed);
        assert!(
            report
                .anti_spam_policy
                .remote_batch_create_concurrency_limited
        );
        assert!(report.checks.public_write_edge_closed);
        assert!(report.checks.transparent_fee_schedule_visible);

        std::fs::remove_dir_all(data_dir).expect("cleanup fee resource policy test");
    }

    #[test]
    fn orchard_pool_report_exposes_only_public_bounds() {
        let data_dir = unique_test_dir("postfiat-orchard-pool-report-test");
        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-orchard-pool-report".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 1,
        })
        .expect("init pool report test");

        let report = orchard_pool_report(OrchardPoolReportOptions {
            data_dir: data_dir.clone(),
        })
        .expect("pool report");

        assert_eq!(report.schema, ORCHARD_POOL_REPORT_SCHEMA);
        assert_eq!(report.chain_id, "postfiat-orchard-pool-report");
        assert_eq!(report.pool_id, ORCHARD_DEFAULT_POOL_ID);
        assert!(report.passed);
        assert!(!report.counters.pool_initialized);
        assert_eq!(report.counters.output_count, 0);
        assert_eq!(report.counters.nullifier_count, 0);
        assert_eq!(report.active_note_bounds.conservative_public_floor, 0);
        assert_eq!(report.active_note_bounds.public_upper_bound, 0);
        assert!(
            !report
                .active_note_bounds
                .exact_active_note_count_publicly_available
        );
        assert!(report.checks.no_private_material_fields);
        assert!(report
            .privacy_claim
            .not_claimed
            .iter()
            .any(|claim| claim == "Zcash-equivalent anonymity set"));

        std::fs::remove_dir_all(data_dir).expect("cleanup pool report test");
    }

    #[test]
    fn orchard_action_gate_verifies_applies_and_rejects_duplicate_nullifiers() {
        let data_dir = unique_test_dir("postfiat-orchard-action-test");
        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-local".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 1,
        })
        .expect("init node");

        let store = NodeStore::new(&data_dir);
        let genesis = store.read_genesis().expect("read genesis");
        let action = test_orchard_action_for_genesis(&genesis);
        let action_file = data_dir.join("orchard-action.json");
        let action_json = serde_json::to_string_pretty(&action).expect("serialize action");
        atomic_write(&action_file, format!("{action_json}\n")).expect("write action");

        let dry_run = verify_or_apply_orchard_action(OrchardActionOptions {
            data_dir: data_dir.clone(),
            action_file: action_file.clone(),
            apply: false,
        })
        .expect("verify Orchard action");
        assert!(dry_run.verified);
        assert!(!dry_run.applied);
        assert_eq!(dry_run.pool_id, "orchard-v1");
        assert_eq!(dry_run.action_count, 2);
        assert_eq!(dry_run.nullifier_count, 2);
        assert_eq!(dry_run.output_count, 2);
        assert_eq!(dry_run.value_balance, 0);
        assert!(store
            .read_shielded()
            .expect("read dry-run shielded state")
            .orchard
            .is_none());

        let domain = orchard_authorizing_domain(&genesis, &action.pool_id).expect("domain");
        let mut nonzero_verified =
            verify_serialized_orchard_action_with_built_key(&action, &domain)
                .expect("verified action");
        assert_eq!(
            orchard_minimum_fee_for_action(&action, &nonzero_verified),
            0
        );
        let mut underpriced_action = action.clone();
        underpriced_action.fee = ORCHARD_FEE_BURN_MIN_FEE - 1;
        let mut underpriced_verified = nonzero_verified.clone();
        underpriced_verified.value_balance =
            i64::try_from(underpriced_action.fee).expect("test Orchard fee fits value balance");
        let underpriced_policy = orchard_fee_burn_amount_for_apply(
            &genesis,
            &ShieldedState::empty(),
            &underpriced_action,
            &underpriced_verified,
        )
        .expect("underpriced fee policy result");
        let underpriced_receipt =
            underpriced_policy.expect_err("underpriced Orchard fee must be rejected");
        assert_eq!(underpriced_receipt.code, "orchard_fee_too_low");
        assert_eq!(underpriced_receipt.minimum_fee, ORCHARD_FEE_BURN_MIN_FEE);
        nonzero_verified.value_balance = -1;
        let mut shielded = store.read_shielded().expect("read shielded state");
        let nonzero_receipt = apply_verified_orchard_action_to_shielded_state(
            &genesis,
            &mut shielded,
            &action,
            &nonzero_verified,
        )
        .expect("nonzero value balance receipt");
        assert!(!nonzero_receipt.accepted);
        assert_eq!(nonzero_receipt.code, "turnstile_insufficient_deposit");
        assert!(shielded.orchard.is_none());

        let mut unretained_verified =
            verify_serialized_orchard_action_with_built_key(&action, &domain)
                .expect("verified action");
        unretained_verified.anchor =
            orchard_anchor_from_commitments(&action.output_commitments).expect("output root");
        let mut shielded = store.read_shielded().expect("read shielded state");
        let unretained_receipt = apply_verified_orchard_action_to_shielded_state(
            &genesis,
            &mut shielded,
            &action,
            &unretained_verified,
        )
        .expect("unretained anchor receipt");
        assert!(!unretained_receipt.accepted);
        assert_eq!(unretained_receipt.code, "unretained_orchard_anchor");
        assert!(shielded.orchard.is_none());

        let applied = verify_or_apply_orchard_action(OrchardActionOptions {
            data_dir: data_dir.clone(),
            action_file: action_file.clone(),
            apply: true,
        })
        .expect("apply Orchard action");
        assert!(applied.verified);
        assert!(applied.applied);
        assert_eq!(applied.receipt.code, "accepted");

        let shielded = store.read_shielded().expect("read applied shielded state");
        let orchard = shielded.orchard.as_ref().expect("Orchard pool state");
        assert_eq!(orchard.pool_id, "orchard-v1");
        assert_eq!(orchard.nullifiers.len(), applied.nullifier_count);
        assert_eq!(orchard.output_commitments.len(), applied.output_count);
        assert_eq!(orchard.encrypted_outputs.len(), applied.output_count);
        assert_eq!(orchard.accepted_anchors.len(), 1);
        assert_eq!(orchard.root_history.len(), 2);
        assert_eq!(orchard.root_history[0].root, orchard_empty_root_hex());
        assert_eq!(orchard.root_history[0].output_count, 0);
        assert_ne!(orchard.root_history[1].root, orchard_empty_root_hex());
        assert_eq!(orchard.root_history[1].output_count, 2);
        let direct_report = verify_shielded(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("verify shielded state with Orchard pool");
        assert_eq!(direct_report.orchard_pool_id, "orchard-v1");
        assert_eq!(direct_report.orchard_nullifier_count, 2);
        assert_eq!(direct_report.orchard_output_count, 2);
        assert_eq!(direct_report.orchard_anchor_count, 1);
        assert_eq!(direct_report.orchard_root_count, 2);
        assert_eq!(
            direct_report.orchard_latest_root,
            orchard.root_history[1].root
        );
        let scan = orchard_wallet_scan(OrchardWalletScanOptions {
            data_dir: data_dir.clone(),
            spending_key_hex: Some(bytes_to_hex(&[7u8; 32])),
            key_file: None,
            view_key_file: None,
        })
        .expect("scan Orchard outputs");
        assert_eq!(scan.pool_id, "orchard-v1");
        assert_eq!(scan.output_count, 2);
        assert_eq!(scan.decrypted_count, 1);
        assert_eq!(scan.spent_count, 0);
        assert_orchard_scan_witnesses(&scan, &orchard.root_history[1].root, 2);
        assert_eq!(scan.outputs[0].value, 0);
        assert_eq!(scan.outputs[0].memo_hex.len(), 1024);
        assert!(orchard
            .output_commitments
            .iter()
            .any(|commitment| commitment == &scan.outputs[0].commitment));
        let wrong_scan = orchard_wallet_scan(OrchardWalletScanOptions {
            data_dir: data_dir.clone(),
            spending_key_hex: Some(bytes_to_hex(&[8u8; 32])),
            key_file: None,
            view_key_file: None,
        })
        .expect("scan Orchard outputs with wrong key");
        assert_eq!(wrong_scan.output_count, 2);
        assert_eq!(wrong_scan.decrypted_count, 0);
        assert_orchard_scan_witnesses(&wrong_scan, &orchard.root_history[1].root, 2);
        let orchard_key_file = data_dir.join("orchard-wallet.json");
        let orchard_key_report = orchard_wallet_keygen(OrchardWalletKeygenOptions {
            master_seed_hex: bytes_to_hex(&[3u8; 32]),
            account_index: 0,
            key_file: orchard_key_file.clone(),
            overwrite: false,
        })
        .expect("generate Orchard wallet key file");
        assert_eq!(orchard_key_report.schema, ORCHARD_WALLET_KEY_REPORT_SCHEMA);
        let key_file_scan = orchard_wallet_scan(OrchardWalletScanOptions {
            data_dir: data_dir.clone(),
            spending_key_hex: None,
            key_file: Some(orchard_key_file.clone()),
            view_key_file: None,
        })
        .expect("scan Orchard outputs from key file");
        assert_eq!(key_file_scan.output_count, 2);
        assert_eq!(key_file_scan.decrypted_count, 0);
        assert_orchard_scan_witnesses(&key_file_scan, &orchard.root_history[1].root, 2);
        let view_key_file = data_dir.join("orchard-view-key.json");
        let view_key_report = orchard_view_key_export(OrchardViewKeyExportOptions {
            key_file: orchard_key_file,
            view_key_file: view_key_file.clone(),
            overwrite: false,
        })
        .expect("export Orchard view key file");
        assert_eq!(view_key_report.schema, ORCHARD_VIEW_KEY_REPORT_SCHEMA);
        assert!(!view_key_report.spend_authority_exported);
        let view_key_json =
            std::fs::read_to_string(&view_key_file).expect("read Orchard view key file");
        assert!(!view_key_json.contains("spending_key_hex"));
        let view_key_scan = orchard_wallet_scan(OrchardWalletScanOptions {
            data_dir: data_dir.clone(),
            spending_key_hex: None,
            key_file: None,
            view_key_file: Some(view_key_file),
        })
        .expect("scan Orchard outputs from view key file");
        assert_eq!(view_key_scan.output_count, 2);
        assert_eq!(view_key_scan.decrypted_count, 0);
        assert_orchard_scan_witnesses(&view_key_scan, &orchard.root_history[1].root, 2);
        let correct_orchard_key_file = data_dir.join("orchard-wallet-correct.json");
        write_orchard_wallet_key_file(
            &correct_orchard_key_file,
            &OrchardWalletKeyFile {
                schema: ORCHARD_WALLET_FILE_SCHEMA.to_string(),
                kdf: ORCHARD_WALLET_DERIVATION_KDF.to_string(),
                derivation_domain: ORCHARD_WALLET_DERIVATION_DOMAIN.to_string(),
                account_index: 0,
                spending_key_hex: bytes_to_hex(&[7u8; 32]),
                address_raw_hex: orchard_default_address_from_spending_key([7u8; 32])
                    .expect("correct Orchard address"),
            },
        )
        .expect("write correct Orchard key file");
        let correct_view_key_file = data_dir.join("orchard-view-key-correct.json");
        let correct_view_key_report = orchard_view_key_export(OrchardViewKeyExportOptions {
            key_file: correct_orchard_key_file,
            view_key_file: correct_view_key_file.clone(),
            overwrite: false,
        })
        .expect("export correct Orchard view key file");
        assert!(!correct_view_key_report.spend_authority_exported);
        let correct_view_key_json =
            std::fs::read_to_string(&correct_view_key_file).expect("read correct view key file");
        assert!(!correct_view_key_json.contains("spending_key_hex"));
        let change_orchard_key_file = data_dir.join("orchard-wallet-change.json");
        write_orchard_wallet_key_file(
            &change_orchard_key_file,
            &OrchardWalletKeyFile {
                schema: ORCHARD_WALLET_FILE_SCHEMA.to_string(),
                kdf: ORCHARD_WALLET_DERIVATION_KDF.to_string(),
                derivation_domain: ORCHARD_WALLET_DERIVATION_DOMAIN.to_string(),
                account_index: 0,
                spending_key_hex: bytes_to_hex(&[9u8; 32]),
                address_raw_hex: orchard_default_address_from_spending_key([9u8; 32])
                    .expect("change Orchard address"),
            },
        )
        .expect("write change Orchard key file");
        let change_view_key_file = data_dir.join("orchard-view-key-change.json");
        orchard_view_key_export(OrchardViewKeyExportOptions {
            key_file: change_orchard_key_file,
            view_key_file: change_view_key_file.clone(),
            overwrite: false,
        })
        .expect("export change Orchard view key file");
        let correct_view_key_scan = orchard_wallet_scan(OrchardWalletScanOptions {
            data_dir: data_dir.clone(),
            spending_key_hex: None,
            key_file: None,
            view_key_file: Some(correct_view_key_file.clone()),
        })
        .expect("scan Orchard outputs from correct view key file");
        assert_eq!(correct_view_key_scan.output_count, 2);
        assert_eq!(correct_view_key_scan.decrypted_count, 1);
        assert_orchard_scan_witnesses(&correct_view_key_scan, &orchard.root_history[1].root, 2);
        assert_eq!(
            correct_view_key_scan.outputs[0].commitment,
            scan.outputs[0].commitment
        );

        let duplicate = verify_or_apply_orchard_action(OrchardActionOptions {
            data_dir: data_dir.clone(),
            action_file,
            apply: true,
        })
        .expect("duplicate Orchard action rejected");
        assert!(duplicate.verified);
        assert!(!duplicate.applied);
        assert_eq!(duplicate.receipt.code, "duplicate_nullifier");

        let shielded_after_duplicate = store
            .read_shielded()
            .expect("read duplicate shielded state");
        let orchard_after_duplicate = shielded_after_duplicate
            .orchard
            .as_ref()
            .expect("Orchard pool state after duplicate");
        assert_eq!(orchard_after_duplicate.nullifiers.len(), 2);
        assert_eq!(orchard_after_duplicate.output_commitments.len(), 2);
        assert_eq!(orchard_after_duplicate.encrypted_outputs.len(), 2);
        assert_eq!(orchard_after_duplicate.accepted_anchors.len(), 1);
        assert_eq!(orchard_after_duplicate.root_history.len(), 2);

        let wallet_created_action_file = data_dir.join("orchard-wallet-created-action.json");
        let wallet_created_report = create_orchard_output_action(OrchardOutputActionOptions {
            data_dir: data_dir.clone(),
            recipient_address_raw_hex: None,
            recipient_key_file: None,
            recipient_view_key_file: Some(correct_view_key_file.clone()),
            memo_hex: None,
            value: 0,
            fee: 0,
            action_file: wallet_created_action_file.clone(),
            overwrite: false,
        })
        .expect("create wallet Orchard output action");
        assert_eq!(
            wallet_created_report.schema,
            ORCHARD_OUTPUT_ACTION_REPORT_SCHEMA
        );
        assert_eq!(
            wallet_created_report.anchor,
            orchard_after_duplicate.root_history[1].root
        );
        assert_eq!(wallet_created_report.value, 0);
        assert_eq!(wallet_created_report.output_count, 2);

        let wallet_created_apply = verify_or_apply_orchard_action(OrchardActionOptions {
            data_dir: data_dir.clone(),
            action_file: wallet_created_action_file,
            apply: true,
        })
        .expect("apply wallet-created Orchard action");
        assert!(wallet_created_apply.applied, "{wallet_created_apply:?}");
        assert_eq!(wallet_created_apply.receipt.code, "accepted");
        let shielded_after_wallet_created = store
            .read_shielded()
            .expect("read wallet-created shielded state");
        let orchard_after_wallet_created = shielded_after_wallet_created
            .orchard
            .as_ref()
            .expect("Orchard pool state after wallet-created action");
        assert_eq!(orchard_after_wallet_created.output_commitments.len(), 4);
        assert_eq!(orchard_after_wallet_created.encrypted_outputs.len(), 4);
        assert_eq!(orchard_after_wallet_created.root_history.len(), 3);
        assert_eq!(orchard_after_wallet_created.root_history[2].output_count, 4);
        let wallet_created_scan = orchard_wallet_scan(OrchardWalletScanOptions {
            data_dir: data_dir.clone(),
            spending_key_hex: None,
            key_file: None,
            view_key_file: Some(correct_view_key_file.clone()),
        })
        .expect("scan wallet-created Orchard output");
        assert_eq!(wallet_created_scan.output_count, 4);
        assert_eq!(wallet_created_scan.decrypted_count, 2);
        assert_orchard_scan_witnesses(
            &wallet_created_scan,
            &orchard_after_wallet_created.root_history[2].root,
            4,
        );

        let genesis = store.read_genesis().expect("read migration-test genesis");
        let genesis_hash_hex = genesis_hash(&genesis);
        let mut historical_shielded = store
            .read_shielded()
            .expect("read historical migration fixture state");
        let migrated_note = mint_debug_note_with_creator_for_chain(
            &mut historical_shielded,
            postfiat_privacy::ShieldedChainContext {
                chain_id: &genesis.chain_id,
                genesis_hash: &genesis_hash_hex,
            },
            "alice",
            DEFAULT_SHIELDED_ASSET_ID,
            7,
            "orchard-deposit-budget",
            direct_shielded_mint_creator(&genesis),
        )
        .expect("seed authenticated historical note for migration test");
        store
            .write_shielded(&historical_shielded)
            .expect("persist historical migration fixture");
        let migration_batch_file = data_dir.join("orchard-migration.batch.json");
        create_shielded_migrate_batch(ShieldMigrateBatchOptions {
            data_dir: data_dir.clone(),
            note_id: migrated_note.note_id.clone(),
            target_pool: ORCHARD_DEFAULT_POOL_ID.to_string(),
            memo: "migrate-to-orchard".to_string(),
            batch_file: migration_batch_file.clone(),
        })
        .expect("create Orchard migration batch");
        let migration_receipts = apply_shielded_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: migration_batch_file,
            certificate_file: None,
        })
        .expect("apply Orchard migration batch");
        assert_eq!(migration_receipts.len(), 1);
        assert!(migration_receipts[0].accepted, "{migration_receipts:?}");

        let nonzero_action_file = data_dir.join("orchard-nonzero-output-action.json");
        let nonzero_report = create_orchard_output_action(OrchardOutputActionOptions {
            data_dir: data_dir.clone(),
            recipient_address_raw_hex: None,
            recipient_key_file: None,
            recipient_view_key_file: Some(correct_view_key_file.clone()),
            memo_hex: None,
            value: 7,
            fee: 0,
            action_file: nonzero_action_file.clone(),
            overwrite: false,
        })
        .expect("create nonzero Orchard output action");
        assert_eq!(nonzero_report.value, 7);
        let nonzero_apply = verify_or_apply_orchard_action(OrchardActionOptions {
            data_dir: data_dir.clone(),
            action_file: nonzero_action_file,
            apply: true,
        })
        .expect("apply nonzero Orchard output action");
        assert!(nonzero_apply.applied, "{nonzero_apply:?}");
        assert_eq!(nonzero_apply.value_balance, -7);
        let shielded_after_nonzero = store.read_shielded().expect("read nonzero Orchard state");
        let orchard_after_nonzero = shielded_after_nonzero
            .orchard
            .as_ref()
            .expect("Orchard pool state after nonzero action");
        assert_eq!(orchard_after_nonzero.turnstile_deposit_total, 7);
        assert_eq!(orchard_after_nonzero.value_balance_total, -7);
        assert_eq!(orchard_after_nonzero.output_commitments.len(), 6);
        assert_eq!(orchard_after_nonzero.root_history.len(), 4);
        let nonzero_scan = orchard_wallet_scan(OrchardWalletScanOptions {
            data_dir: data_dir.clone(),
            spending_key_hex: None,
            key_file: None,
            view_key_file: Some(correct_view_key_file.clone()),
        })
        .expect("scan nonzero Orchard output");
        assert_eq!(nonzero_scan.output_count, 6);
        assert_eq!(nonzero_scan.decrypted_count, 3);
        assert_orchard_scan_witnesses(
            &nonzero_scan,
            &orchard_after_nonzero.root_history[3].root,
            6,
        );
        assert!(nonzero_scan.outputs.iter().any(|output| output.value == 7));
        let spend_input = nonzero_scan
            .outputs
            .iter()
            .find(|output| output.value == 7 && !output.spent)
            .expect("unspent migrated-value Orchard note");
        let spend_action_file = data_dir.join("orchard-spend-action.json");
        let spend_report = create_orchard_spend_action(OrchardSpendActionOptions {
            data_dir: data_dir.clone(),
            spending_key_hex: Some(bytes_to_hex(&[7u8; 32])),
            key_file: None,
            input_output_index: spend_input.output_index,
            amount: Some(3),
            recipient_address_raw_hex: None,
            recipient_key_file: None,
            recipient_view_key_file: Some(correct_view_key_file.clone()),
            change_address_raw_hex: None,
            change_key_file: None,
            change_view_key_file: Some(change_view_key_file.clone()),
            memo_hex: None,
            fee: 2,
            action_file: spend_action_file.clone(),
            overwrite: false,
        })
        .expect("create Orchard spend action");
        assert_eq!(spend_report.schema, ORCHARD_SPEND_ACTION_REPORT_SCHEMA);
        assert_eq!(spend_report.input_output_index, spend_input.output_index);
        assert_eq!(spend_report.input_nullifier, spend_input.nullifier);
        assert_eq!(spend_report.input_value, 7);
        assert_eq!(spend_report.output_value, 5);
        assert_eq!(spend_report.recipient_value, 3);
        assert_eq!(spend_report.change_value, 2);
        assert_eq!(
            spend_report.change_address_raw_hex,
            orchard_default_address_from_spending_key([9u8; 32]).expect("change address")
        );
        assert_eq!(spend_report.fee, 2);
        assert_eq!(spend_report.minimum_fee, ORCHARD_FEE_BURN_MIN_FEE);
        assert_eq!(spend_report.value_balance, 2);
        assert!(spend_report.output_count >= 2);
        let spend_action_json =
            fs::read_to_string(&spend_action_file).expect("read private transfer action");
        let spend_batch_file = data_dir.join("orchard-spend.batch.json");
        create_orchard_action_batch(OrchardActionBatchOptions {
            data_dir: data_dir.clone(),
            action_file: spend_action_file,
            batch_file: spend_batch_file.clone(),
        })
        .expect("create private transfer batch");
        let spend_batch_json =
            fs::read_to_string(&spend_batch_file).expect("read private transfer batch");
        let spend_receipts = apply_shielded_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: spend_batch_file,
            certificate_file: None,
        })
        .expect("apply private transfer batch");
        assert_eq!(spend_receipts.len(), 1);
        assert!(spend_receipts[0].accepted, "{spend_receipts:?}");
        assert_eq!(spend_receipts[0].code, "accepted");
        assert_eq!(spend_receipts[0].fee_burned, 2);
        assert_eq!(spend_receipts[0].minimum_fee, ORCHARD_FEE_BURN_MIN_FEE);
        let shielded_after_spend = store.read_shielded().expect("read spent Orchard state");
        let orchard_after_spend = shielded_after_spend
            .orchard
            .as_ref()
            .expect("Orchard pool state after spend action");
        assert_eq!(
            orchard_after_spend.output_commitments.len(),
            orchard_after_nonzero.output_commitments.len() + spend_report.output_count
        );
        assert_eq!(orchard_after_spend.turnstile_deposit_total, 7);
        assert_eq!(orchard_after_spend.fee_burn_total, 2);
        assert_eq!(orchard_after_spend.value_balance_total, -5);
        assert_eq!(orchard_after_spend.root_history.len(), 5);
        let post_spend_scan = orchard_wallet_scan(OrchardWalletScanOptions {
            data_dir: data_dir.clone(),
            spending_key_hex: None,
            key_file: None,
            view_key_file: Some(correct_view_key_file),
        })
        .expect("scan after Orchard spend");
        assert_eq!(
            post_spend_scan.output_count,
            orchard_after_spend.output_commitments.len()
        );
        assert_eq!(post_spend_scan.spent_count, 1);
        assert!(post_spend_scan
            .outputs
            .iter()
            .any(|output| output.nullifier == spend_input.nullifier && output.spent));
        assert!(post_spend_scan
            .outputs
            .iter()
            .any(|output| output.value == 3 && !output.spent));
        assert!(!post_spend_scan
            .outputs
            .iter()
            .any(|output| output.value == 2 && !output.spent));
        let withdraw_input = post_spend_scan
            .outputs
            .iter()
            .find(|output| output.value == 3 && !output.spent)
            .expect("unspent Orchard note for transparent withdraw");
        let withdraw_to = store
            .read_ledger()
            .expect("read ledger before withdraw")
            .accounts
            .first()
            .expect("faucet account exists")
            .address
            .clone();
        let withdraw_balance_before = store
            .read_ledger()
            .expect("read ledger balance before withdraw")
            .account(&withdraw_to)
            .expect("withdraw recipient exists before withdraw")
            .balance;
        let withdraw_action_file = data_dir.join("orchard-withdraw-action.json");
        let withdraw_report = create_orchard_withdraw_action(OrchardWithdrawActionOptions {
            data_dir: data_dir.clone(),
            spending_key_hex: Some(bytes_to_hex(&[7u8; 32])),
            key_file: None,
            input_output_index: withdraw_input.output_index,
            to: withdraw_to.clone(),
            amount: 1,
            change_address_raw_hex: None,
            change_key_file: None,
            change_view_key_file: None,
            memo_hex: None,
            fee: 2,
            policy_id: None,
            disclosure_hash: None,
            action_file: withdraw_action_file.clone(),
            overwrite: false,
        })
        .expect("create Orchard withdraw action");
        assert_eq!(
            withdraw_report.schema,
            ORCHARD_WITHDRAW_ACTION_REPORT_SCHEMA
        );
        assert_eq!(
            withdraw_report.input_output_index,
            withdraw_input.output_index
        );
        assert_eq!(withdraw_report.input_nullifier, withdraw_input.nullifier);
        assert_eq!(withdraw_report.input_value, 3);
        assert_eq!(withdraw_report.withdraw_amount, 1);
        assert_eq!(withdraw_report.change_value, 0);
        assert_eq!(withdraw_report.to, withdraw_to);
        assert_eq!(withdraw_report.fee, 2);
        assert_eq!(withdraw_report.minimum_fee, ORCHARD_FEE_BURN_MIN_FEE);
        assert_eq!(withdraw_report.state_expansion_fee, 0);
        assert_eq!(withdraw_report.policy_id, ORCHARD_WITHDRAW_POLICY_ID);
        assert_eq!(withdraw_report.value_balance, 3);
        assert_eq!(withdraw_report.external_binding_hash.len(), 96);
        let withdraw_action_json =
            fs::read_to_string(&withdraw_action_file).expect("read private egress action");
        let bad_withdraw_batch =
            create_orchard_withdraw_action_batch(OrchardWithdrawActionBatchOptions {
                data_dir: data_dir.clone(),
                action_file: withdraw_action_file.clone(),
                to: withdraw_to.clone(),
                amount: 2,
                fee: 2,
                policy_id: None,
                disclosure_hash: None,
                batch_file: data_dir.join("orchard-withdraw-bad.batch.json"),
            })
            .expect_err("mismatched withdraw envelope must not batch");
        assert!(bad_withdraw_batch.to_string().contains("external binding"));
        let withdraw_batch_file = data_dir.join("orchard-withdraw.batch.json");
        let withdraw_batch =
            create_orchard_withdraw_action_batch(OrchardWithdrawActionBatchOptions {
                data_dir: data_dir.clone(),
                action_file: withdraw_action_file,
                to: withdraw_to.clone(),
                amount: 1,
                fee: 2,
                policy_id: None,
                disclosure_hash: None,
                batch_file: withdraw_batch_file.clone(),
            })
            .expect("create Orchard withdraw shielded batch");
        assert!(matches!(
            withdraw_batch.actions.first(),
            Some(ShieldedAction::OrchardWithdrawV1(_))
        ));
        let withdraw_batch_json =
            fs::read_to_string(&withdraw_batch_file).expect("read private egress batch");
        let withdraw_receipts = apply_shielded_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: withdraw_batch_file.clone(),
            certificate_file: None,
        })
        .expect("apply Orchard withdraw batch");
        assert_eq!(withdraw_receipts.len(), 1);
        assert!(withdraw_receipts[0].accepted, "{withdraw_receipts:?}");
        assert_eq!(withdraw_receipts[0].fee_charged, 2);
        assert_eq!(withdraw_receipts[0].fee_burned, 2);
        assert_eq!(withdraw_receipts[0].minimum_fee, ORCHARD_FEE_BURN_MIN_FEE);
        let withdraw_balance_after = store
            .read_ledger()
            .expect("read ledger balance after withdraw")
            .account(&withdraw_to)
            .expect("withdraw recipient exists after withdraw")
            .balance;
        assert_eq!(withdraw_balance_after, withdraw_balance_before + 1);
        let private_markers = [
            spend_input.rho.as_str(),
            spend_input.rseed.as_str(),
            "0707070707070707070707070707070707070707070707070707070707070707",
        ];
        let public_artifacts = [
            ("private-transfer-action", spend_action_json),
            ("private-transfer-batch", spend_batch_json),
            ("private-egress-action", withdraw_action_json),
            ("private-egress-batch", withdraw_batch_json),
            (
                "batch-archive",
                serde_json::to_string(&store.read_batch_archive().expect("read batch archive"))
                    .expect("serialize batch archive"),
            ),
            (
                "block-log",
                serde_json::to_string(&store.read_blocks().expect("read block log"))
                    .expect("serialize block log"),
            ),
            (
                "receipt-log",
                serde_json::to_string(&store.read_receipts().expect("read receipt log"))
                    .expect("serialize receipt log"),
            ),
        ];
        for (label, artifact) in &public_artifacts {
            assert_orchard_public_artifact_redacted(label, artifact, &private_markers);
        }
        let shielded_after_withdraw = store.read_shielded().expect("read withdrawn Orchard state");
        let orchard_after_withdraw = shielded_after_withdraw
            .orchard
            .as_ref()
            .expect("Orchard pool state after withdraw");
        assert_eq!(orchard_after_withdraw.turnstile_deposit_total, 7);
        assert_eq!(orchard_after_withdraw.fee_burn_total, 4);
        assert_eq!(orchard_after_withdraw.withdraw_total, 1);
        assert_eq!(orchard_after_withdraw.value_balance_total, -2);
        assert_eq!(orchard_after_withdraw.root_history.len(), 6);
        let post_withdraw_scan = orchard_wallet_scan(OrchardWalletScanOptions {
            data_dir: data_dir.clone(),
            spending_key_hex: Some(bytes_to_hex(&[7u8; 32])),
            key_file: None,
            view_key_file: None,
        })
        .expect("scan after Orchard withdraw");
        assert_eq!(post_withdraw_scan.spent_count, 2);
        assert!(post_withdraw_scan
            .outputs
            .iter()
            .any(|output| output.nullifier == withdraw_input.nullifier && output.spent));
        let change_scan = orchard_wallet_scan(OrchardWalletScanOptions {
            data_dir: data_dir.clone(),
            spending_key_hex: None,
            key_file: None,
            view_key_file: Some(change_view_key_file.clone()),
        })
        .expect("scan change Orchard output");
        assert!(change_scan
            .outputs
            .iter()
            .any(|output| output.value == 2 && !output.spent));
        let change_output = change_scan
            .outputs
            .iter()
            .find(|output| output.value == 2 && !output.spent)
            .expect("unspent change note for disclosure");
        let disclosure_file = data_dir.join("orchard-disclosure.json");
        let disclosure = orchard_disclosure_packet(OrchardDisclosureOptions {
            data_dir: data_dir.clone(),
            spending_key_hex: None,
            key_file: None,
            view_key_file: Some(change_view_key_file),
            output_index: change_output.output_index,
            packet_file: disclosure_file.clone(),
            overwrite: false,
        })
        .expect("write Orchard disclosure packet");
        assert_eq!(disclosure.schema, ORCHARD_DISCLOSURE_PACKET_SCHEMA);
        assert_eq!(disclosure.pool_id, ORCHARD_DEFAULT_POOL_ID);
        assert_eq!(disclosure.output_index, change_output.output_index);
        assert_eq!(disclosure.commitment, change_output.commitment);
        assert_eq!(disclosure.nullifier, change_output.nullifier);
        assert_eq!(disclosure.value, 2);
        assert!(!disclosure.spent);
        assert!(disclosure.private_witness_redacted);
        let disclosure_finality = disclosure
            .finality
            .as_ref()
            .expect("ordered private transfer disclosure must include finality");
        assert_eq!(disclosure_finality.batch_kind, BATCH_KIND_SHIELDED);
        assert!(disclosure_finality
            .receipt_ids
            .iter()
            .any(|receipt_id| receipt_id == &spend_receipts[0].tx_id));
        assert_eq!(disclosure.disclosure_hash.len(), 96);
        let disclosure_json =
            std::fs::read_to_string(&disclosure_file).expect("read disclosure packet");
        assert!(!disclosure_json.contains("rseed"));
        assert!(!disclosure_json.contains("\"witness_auth_path\""));
        assert!(!disclosure_json.contains("\"spending_key_hex\""));
        assert!(!disclosure_json.contains("\"full_viewing_key_hex\""));
        let disclosure_from_disk: OrchardDisclosurePacket =
            serde_json::from_str(&disclosure_json).expect("parse disclosure packet");
        assert_eq!(disclosure_from_disk, disclosure);
        let disclosure_verify = orchard_disclosure_verify(OrchardDisclosureVerifyOptions {
            data_dir: data_dir.clone(),
            packet_file: disclosure_file.clone(),
        })
        .expect("verify direct Orchard disclosure packet");
        assert_eq!(
            disclosure_verify.schema,
            ORCHARD_DISCLOSURE_VERIFY_REPORT_SCHEMA
        );
        assert!(disclosure_verify.verified);
        assert!(disclosure_verify.packet_hash_verified);
        assert!(disclosure_verify.local_context_verified);
        assert!(disclosure_verify.finality_verified);
        let mut tampered_disclosure = disclosure.clone();
        tampered_disclosure.value += 1;
        let tampered_disclosure_file = data_dir.join("orchard-disclosure-tampered.json");
        let tampered_json =
            serde_json::to_string_pretty(&tampered_disclosure).expect("serialize tampered packet");
        atomic_write(&tampered_disclosure_file, format!("{tampered_json}\n"))
            .expect("write tampered disclosure packet");
        let tampered_verify = orchard_disclosure_verify(OrchardDisclosureVerifyOptions {
            data_dir: data_dir.clone(),
            packet_file: tampered_disclosure_file,
        })
        .expect_err("tampered disclosure hash must fail");
        assert_eq!(tampered_verify.kind(), io::ErrorKind::InvalidData);
        let duplicate_disclosure = orchard_disclosure_packet(OrchardDisclosureOptions {
            data_dir: data_dir.clone(),
            spending_key_hex: None,
            key_file: None,
            view_key_file: Some(data_dir.join("orchard-view-key-change.json")),
            output_index: change_output.output_index,
            packet_file: disclosure_file,
            overwrite: false,
        })
        .expect_err("duplicate disclosure packet requires overwrite");
        assert_eq!(duplicate_disclosure.kind(), io::ErrorKind::AlreadyExists);
        let turnstile_after_nonzero = shield_turnstile(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("turnstile after nonzero Orchard action");
        assert_eq!(turnstile_after_nonzero.migration_total, 7);
        let shielded_report = verify_shielded(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("verify shielded state after Orchard withdraw");
        assert_eq!(shielded_report.orchard_turnstile_deposit_total, 7);
        assert_eq!(shielded_report.orchard_fee_burn_total, 4);
        assert_eq!(shielded_report.orchard_withdraw_total, 1);
        assert_eq!(shielded_report.orchard_value_balance_total, -2);

        let batch_data_dir = unique_test_dir("postfiat-orchard-action-batch-test");
        init(InitOptions {
            data_dir: batch_data_dir.clone(),
            chain_id: "postfiat-local".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 1,
        })
        .expect("init batch node");
        let batch_action_file = batch_data_dir.join("orchard-action.json");
        atomic_write(&batch_action_file, format!("{action_json}\n")).expect("write batch action");
        let batch_file = batch_data_dir.join("orchard-shielded-batch.json");
        let batch = create_orchard_action_batch(OrchardActionBatchOptions {
            data_dir: batch_data_dir.clone(),
            action_file: batch_action_file,
            batch_file: batch_file.clone(),
        })
        .expect("create Orchard shielded batch");
        assert_eq!(batch.actions.len(), 1);
        assert!(matches!(
            batch.actions.first(),
            Some(ShieldedAction::OrchardV1(_))
        ));
        let batch_receipts = apply_shielded_batch(ApplyBatchOptions {
            data_dir: batch_data_dir.clone(),
            batch_file,
            certificate_file: None,
        })
        .expect("apply Orchard shielded batch");
        assert_eq!(batch_receipts.len(), 1);
        assert!(batch_receipts[0].accepted, "{batch_receipts:?}");
        let batch_scan = orchard_wallet_scan(OrchardWalletScanOptions {
            data_dir: batch_data_dir.clone(),
            spending_key_hex: Some(bytes_to_hex(&[7u8; 32])),
            key_file: None,
            view_key_file: None,
        })
        .expect("scan ordered Orchard batch output");
        assert_eq!(batch_scan.decrypted_count, 1);
        let batch_output = batch_scan
            .outputs
            .first()
            .expect("ordered batch decrypted output");
        let batch_disclosure_file = batch_data_dir.join("orchard-ordered-disclosure.json");
        let batch_disclosure = orchard_disclosure_packet(OrchardDisclosureOptions {
            data_dir: batch_data_dir.clone(),
            spending_key_hex: Some(bytes_to_hex(&[7u8; 32])),
            key_file: None,
            view_key_file: None,
            output_index: batch_output.output_index,
            packet_file: batch_disclosure_file.clone(),
            overwrite: false,
        })
        .expect("write ordered Orchard disclosure packet");
        let batch_finality = batch_disclosure
            .finality
            .as_ref()
            .expect("ordered disclosure includes finality");
        assert_eq!(batch_finality.batch_kind, BATCH_KIND_SHIELDED);
        assert_eq!(batch_finality.batch_id, batch.batch_id);
        assert!(batch_finality
            .receipt_ids
            .iter()
            .any(|receipt_id| receipt_id == &batch_receipts[0].tx_id));
        let batch_disclosure_verify = orchard_disclosure_verify(OrchardDisclosureVerifyOptions {
            data_dir: batch_data_dir.clone(),
            packet_file: batch_disclosure_file,
        })
        .expect("verify ordered Orchard disclosure packet");
        assert!(batch_disclosure_verify.verified);
        assert!(batch_disclosure_verify.packet_hash_verified);
        assert!(batch_disclosure_verify.local_context_verified);
        assert!(batch_disclosure_verify.finality_verified);

        let batch_store = NodeStore::new(&batch_data_dir);
        let batch_shielded = batch_store
            .read_shielded()
            .expect("read batch shielded state");
        let batch_orchard = batch_shielded
            .orchard
            .as_ref()
            .expect("batch Orchard pool state");
        assert_eq!(batch_orchard.nullifiers.len(), 2);
        assert_eq!(batch_orchard.output_commitments.len(), 2);
        assert_eq!(batch_orchard.encrypted_outputs.len(), 2);
        assert_eq!(batch_orchard.root_history.len(), 2);
        assert_eq!(batch_orchard.root_history[0].root, orchard_empty_root_hex());
        assert_eq!(batch_orchard.root_history[0].output_count, 0);
        assert_eq!(batch_orchard.root_history[1].output_count, 2);
        let batch_report = verify_shielded(NodeOptions {
            data_dir: batch_data_dir,
        })
        .expect("verify batch shielded state");
        assert_eq!(batch_report.orchard_pool_id, "orchard-v1");
        assert_eq!(batch_report.orchard_nullifier_count, 2);
        assert_eq!(batch_report.orchard_output_count, 2);
        assert_eq!(batch_report.orchard_anchor_count, 1);
        assert_eq!(batch_report.orchard_root_count, 2);
        assert_eq!(
            batch_report.orchard_latest_root,
            batch_orchard.root_history[1].root
        );
    }

    #[test]
    fn shielded_swap_action_batch_fails_closed_without_consensus_conservation_proof() {
        let (data_dir, genesis) =
            shielded_swap_test_dir("postfiat-shielded-swap-action-batch-test");
        let action =
            valid_shielded_swap_action_for_genesis(&genesis, orchard_empty_anchor(), "swap-good");
        let action_file = write_shielded_swap_action_file(&data_dir, "swap-action.json", &action);
        let batch_file = data_dir.join("swap.batch.json");
        let batch_error = create_shielded_swap_action_batch(ShieldedSwapActionBatchOptions {
            data_dir: data_dir.clone(),
            swap_file: action_file,
            batch_file: batch_file.clone(),
        })
        .expect_err("batch creation must fail without consensus proof verifier");
        assert_eq!(batch_error.kind(), io::ErrorKind::InvalidData);
        assert!(
            batch_error
                .to_string()
                .contains("shielded_swap_proof_verifier_unimplemented"),
            "{batch_error}"
        );

        let raw_batch = build_shielded_action_batch(
            &genesis,
            vec![ShieldedAction::ShieldedSwapV1(ShieldedSwapActionPayload {
                swap_json: serde_json::to_string(&action).expect("swap json"),
            })],
        )
        .expect("build raw shielded swap batch");
        write_shielded_action_batch_file(&batch_file, &raw_batch)
            .expect("write raw shielded swap batch");
        let receipts = apply_shielded_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file,
            certificate_file: None,
        })
        .expect("apply fail-closed shielded swap batch");
        assert_eq!(receipts.len(), 1);
        assert!(!receipts[0].accepted, "{receipts:?}");
        assert_eq!(
            receipts[0].code,
            "shielded_swap_proof_verifier_unimplemented"
        );

        let store = NodeStore::new(&data_dir);
        let shielded = store.read_shielded().expect("read shielded state");
        assert!(shielded.orchard.is_none());
        assert!(
            find_orchard_disclosure_finality(&store, action.output_commitments[0].as_hex())
                .expect("lookup rejected swap archive")
                .is_none()
        );

        let report = verify_shielded(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("verify fail-closed shielded state");
        assert!(report.verified);
        assert_eq!(report.orchard_nullifier_count, 0);
        assert_eq!(report.orchard_output_count, 0);
        assert_eq!(report.orchard_root_count, 0);

        fs::remove_dir_all(data_dir).expect("cleanup shielded swap action batch test");
    }
    #[test]
    fn shielded_swap_fails_closed_before_duplicate_state_checks() {
        let (data_dir, genesis) =
            shielded_swap_test_dir("postfiat-shielded-swap-duplicate-test");

        let mut duplicate_nullifier =
            valid_shielded_swap_action_for_genesis(&genesis, orchard_empty_anchor(), "swap-dupe-nf");
        duplicate_nullifier.nullifiers[1] = duplicate_nullifier.nullifiers[0].clone();
        let duplicate_nullifier_receipts = apply_raw_shielded_swap_payload(
            &data_dir,
            &genesis,
            "duplicate-nullifier.batch.json",
            serde_json::to_string(&duplicate_nullifier).expect("duplicate nullifier json"),
        );
        assert_eq!(duplicate_nullifier_receipts.len(), 1);
        assert!(!duplicate_nullifier_receipts[0].accepted);
        assert_eq!(
            duplicate_nullifier_receipts[0].code,
            "shielded_swap_proof_verifier_unimplemented"
        );

        let mut duplicate_output =
            valid_shielded_swap_action_for_genesis(&genesis, orchard_empty_anchor(), "swap-dupe-out");
        duplicate_output.output_commitments[1] = duplicate_output.output_commitments[0].clone();
        let duplicate_output_receipts = apply_raw_shielded_swap_payload(
            &data_dir,
            &genesis,
            "duplicate-output.batch.json",
            serde_json::to_string(&duplicate_output).expect("duplicate output json"),
        );
        assert_eq!(duplicate_output_receipts.len(), 1);
        assert!(!duplicate_output_receipts[0].accepted);
        assert_eq!(
            duplicate_output_receipts[0].code,
            "shielded_swap_proof_verifier_unimplemented"
        );

        let well_formed =
            valid_shielded_swap_action_for_genesis(&genesis, orchard_empty_anchor(), "swap-formed");
        let well_formed_receipts = apply_raw_shielded_swap_payload(
            &data_dir,
            &genesis,
            "well-formed.batch.json",
            serde_json::to_string(&well_formed).expect("well formed json"),
        );
        assert_eq!(well_formed_receipts.len(), 1);
        assert!(!well_formed_receipts[0].accepted);
        assert_eq!(
            well_formed_receipts[0].code,
            "shielded_swap_proof_verifier_unimplemented"
        );

        fs::remove_dir_all(data_dir).expect("cleanup shielded swap duplicate test");
    }

    #[test]
    fn shielded_swap_rejects_wrong_anchor_wrong_domain_tampered_binding_and_oversized_payload() {
        let (data_dir, genesis) =
            shielded_swap_test_dir("postfiat-shielded-swap-adversarial-test");

        let wrong_anchor = OrchardAnchor::parse_hex("22".repeat(32)).expect("wrong anchor");
        let wrong_anchor_action =
            valid_shielded_swap_action_for_genesis(&genesis, wrong_anchor, "swap-wrong-anchor");
        let wrong_anchor_receipts = apply_raw_shielded_swap_payload(
            &data_dir,
            &genesis,
            "wrong-anchor.batch.json",
            serde_json::to_string(&wrong_anchor_action).expect("wrong anchor json"),
        );
        assert_eq!(wrong_anchor_receipts.len(), 1);
        assert!(!wrong_anchor_receipts[0].accepted);
        assert_eq!(
            wrong_anchor_receipts[0].code,
            "shielded_swap_proof_verifier_unimplemented"
        );

        let wrong_domain = postfiat_privacy_orchard::OrchardAuthorizingDomain::new(
            genesis.chain_id.clone(),
            "33".repeat(48),
            genesis.protocol_version,
            "orchard-swap",
        )
        .expect("wrong swap domain");
        let wrong_domain_action = shielded_swap_build_action_test_vector(
            &wrong_domain,
            "orchard-swap",
            orchard_empty_anchor(),
            valid_shielded_swap_inputs(),
            valid_shielded_swap_outputs(),
            "swap-wrong-domain",
            0,
        )
        .expect("wrong-domain action");
        let wrong_domain_receipts = apply_raw_shielded_swap_payload(
            &data_dir,
            &genesis,
            "wrong-domain.batch.json",
            serde_json::to_string(&wrong_domain_action).expect("wrong domain json"),
        );
        assert_eq!(wrong_domain_receipts.len(), 1);
        assert!(!wrong_domain_receipts[0].accepted);
        assert_eq!(
            wrong_domain_receipts[0].code,
            "shielded_swap_proof_verifier_unimplemented"
        );

        let mut tampered_binding =
            valid_shielded_swap_action_for_genesis(&genesis, orchard_empty_anchor(), "swap-tamper");
        tampered_binding.swap_binding_hash =
            ShieldedSwapCommitment::parse_hex("44".repeat(48)).expect("tampered binding");
        let tampered_receipts = apply_raw_shielded_swap_payload(
            &data_dir,
            &genesis,
            "tampered-binding.batch.json",
            serde_json::to_string(&tampered_binding).expect("tampered binding json"),
        );
        assert_eq!(tampered_receipts.len(), 1);
        assert!(!tampered_receipts[0].accepted);
        assert_eq!(
            tampered_receipts[0].code,
            "shielded_swap_proof_verifier_unimplemented"
        );

        let oversized_batch = build_shielded_action_batch(
            &genesis,
            vec![ShieldedAction::ShieldedSwapV1(ShieldedSwapActionPayload {
                swap_json: "x".repeat((MAX_LOCAL_JSON_FILE_BYTES as usize) + 1),
            })],
        );
        let oversized_batch = oversized_batch.expect("build oversized in-memory swap batch");
        let store = NodeStore::new(&data_dir);
        let mut ledger = store.read_ledger().expect("read ledger for oversized test");
        let mut shielded = store
            .read_shielded()
            .expect("read shielded state for oversized test");
        let oversized_receipts =
            execute_shielded_batch(
                &genesis,
                &mut ledger,
                &mut shielded,
                &oversized_batch,
                1,
                AssetExecutionCompatibility::strict(),
                false,
                false,
            );
        assert_eq!(oversized_receipts.len(), 1);
        assert!(!oversized_receipts[0].accepted);
        assert_eq!(oversized_receipts[0].code, "shielded_swap_too_large");

        fs::remove_dir_all(data_dir).expect("cleanup shielded swap adversarial test");
    }

    #[test]
    fn governance_pause_rejects_all_shielded_actions_before_mutation() {
        let (data_dir, genesis) = shielded_swap_test_dir("postfiat-orchard-pause");
        let mut governance = GovernanceState::new(1);
        let pause = GovernanceAmendment {
            amendment_id: "orchard-pause-1".to_string(),
            chain_id: genesis.chain_id.clone(),
            genesis_hash: genesis_hash(&genesis),
            protocol_version: genesis.protocol_version,
            instance_id: "orchard-pause-instance".to_string(),
            proposal_id: "orchard-pause-proposal".to_string(),
            certificate_id: "orchard-pause-certificate".to_string(),
            proposer: "validator-0".to_string(),
            validators: vec!["validator-0".to_string()],
            quorum: 1,
            kind: GOVERNANCE_KIND_ORCHARD_POOL_PAUSE.to_string(),
            value: 1,
            activation_height: 0,
            veto_until_height: 0,
            paused: false,
            support: vec!["validator-0".to_string()],
            votes: Vec::new(),
            signed_authorizations: Vec::new(),
        };
        let governance_receipts = execute_governance_batch(
            &mut governance,
            None,
            &GovernanceActionBatch::new("orchard-pause-batch", vec![pause]),
            1,
        );
        assert_eq!(governance_receipts.len(), 1);
        assert!(governance_receipts[0].accepted);
        assert!(governance.orchard_pool_paused);

        let store = NodeStore::new(&data_dir);
        let mut ledger = store.read_ledger().expect("read paused ledger");
        let mut shielded = store.read_shielded().expect("read paused shielded state");
        let ledger_before = ledger.clone();
        let shielded_before = shielded.clone();
        let batch = ShieldedActionBatch::new(
            "paused-actions",
            vec![
                ShieldedAction::Mint(ShieldMintAction {
                    owner: "owner".to_string(),
                    asset_id: DEFAULT_SHIELDED_ASSET_ID.to_string(),
                    amount: 1,
                    memo: String::new(),
                }),
                ShieldedAction::ShieldedSwapV1(ShieldedSwapActionPayload {
                    swap_json: "not-a-proof".to_string(),
                }),
            ],
        );
        let receipts = execute_shielded_batch(
            &genesis,
            &mut ledger,
            &mut shielded,
            &batch,
            2,
            AssetExecutionCompatibility::strict(),
            governance.orchard_pool_paused,
            false,
        );
        assert_eq!(receipts.len(), 2);
        assert!(receipts.iter().all(|receipt| !receipt.accepted));
        assert!(receipts
            .iter()
            .all(|receipt| receipt.code == "orchard_pool_paused"));
        assert_eq!(ledger, ledger_before);
        assert_eq!(shielded, shielded_before);

        let mut invalid = governance.amendments[0].clone();
        invalid.amendment_id = "orchard-pause-invalid".to_string();
        invalid.value = 2;
        let invalid_receipts = execute_governance_batch(
            &mut governance,
            None,
            &GovernanceActionBatch::new("orchard-pause-invalid-batch", vec![invalid]),
            3,
        );
        assert_eq!(invalid_receipts[0].code, "invalid_orchard_pool_pause_value");
        assert!(governance.orchard_pool_paused);

        let mut resume = governance.amendments[0].clone();
        resume.amendment_id = "orchard-resume-1".to_string();
        resume.instance_id = "orchard-resume-instance".to_string();
        resume.proposal_id = "orchard-resume-proposal".to_string();
        resume.certificate_id = "orchard-resume-certificate".to_string();
        resume.value = 0;
        let resume_receipts = execute_governance_batch(
            &mut governance,
            None,
            &GovernanceActionBatch::new("orchard-resume-batch", vec![resume]),
            4,
        );
        assert!(resume_receipts[0].accepted);
        assert!(!governance.orchard_pool_paused);

        fs::remove_dir_all(data_dir).expect("cleanup orchard pause test");
    }

    #[test]
    fn legacy_cleartext_shielded_actions_are_historical_replay_only() {
        let local_dir = unique_test_dir("postfiat-legacy-cleartext-shielded-disabled-test");
        init(InitOptions {
            data_dir: local_dir.clone(),
            chain_id: "postfiat-local".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 1,
        })
        .expect("init local historical-replay chain");
        let store = NodeStore::new(&local_dir);
        let direct_state_before = store.read_shielded().expect("read pre-direct-mint state");
        let direct_error = shield_mint(ShieldMintOptions {
            data_dir: local_dir.clone(),
            owner: "alice".to_string(),
            asset_id: DEFAULT_SHIELDED_ASSET_ID.to_string(),
            amount: 1,
            memo: "must not be admitted through direct state".to_string(),
        })
        .expect_err("direct legacy cleartext mint must fail closed");
        assert_eq!(direct_error.kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(
            store.read_shielded().expect("read post-direct-mint state"),
            direct_state_before,
            "rejected direct legacy mint must not mutate state"
        );
        let direct_spend_error = shield_spend(ShieldSpendOptions {
            data_dir: local_dir.clone(),
            note_id: "nonexistent-historical-note".to_string(),
            to: "bob".to_string(),
            amount: 1,
            memo: "must not be admitted through direct state".to_string(),
        })
        .expect_err("direct legacy cleartext spend must fail closed");
        assert_eq!(direct_spend_error.kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(
            store.read_shielded().expect("read post-direct-spend state"),
            direct_state_before,
            "rejected direct legacy spend must not mutate state"
        );
        let create_error = create_shielded_mint_batch(ShieldMintBatchOptions {
            data_dir: local_dir.clone(),
            owner: "alice".to_string(),
            asset_id: DEFAULT_SHIELDED_ASSET_ID.to_string(),
            amount: 1,
            memo: "must not be admitted live".to_string(),
            batch_file: local_dir.join("forbidden-debug-mint.batch.json"),
        })
        .expect_err("legacy cleartext mint creation must fail closed");
        assert_eq!(create_error.kind(), io::ErrorKind::PermissionDenied);

        let genesis = store.read_genesis().expect("read genesis");
        let batch = build_shielded_action_batch(
            &genesis,
            vec![ShieldedAction::Mint(ShieldMintAction {
                owner: "historical-alice".to_string(),
                asset_id: DEFAULT_SHIELDED_ASSET_ID.to_string(),
                amount: 1,
                memo: "historical fixture".to_string(),
            })],
        );
        let batch = batch.expect("build historical batch fixture");
        let admission_error = reject_live_legacy_cleartext_shielded_actions(&batch)
            .expect_err("legacy cleartext action must fail live admission");
        assert_eq!(admission_error.kind(), io::ErrorKind::PermissionDenied);

        let public_batch_file = local_dir.join("forbidden-public-legacy-mint.batch.json");
        write_shielded_action_batch_file(&public_batch_file, &batch)
            .expect("write public legacy-injection fixture");
        let public_state_before = store.read_shielded().expect("read pre-public-apply state");
        let public_receipts = apply_shielded_batch(ApplyBatchOptions {
            data_dir: local_dir.clone(),
            batch_file: public_batch_file,
            certificate_file: None,
        })
        .expect("public apply must commit a rejected receipt, not legacy state");
        assert_eq!(public_receipts.len(), 1);
        assert!(!public_receipts[0].accepted, "{public_receipts:?}");
        assert_eq!(
            public_receipts[0].code,
            "legacy_cleartext_shielded_action_disabled"
        );
        assert_eq!(
            store.read_shielded().expect("read post-public-apply state"),
            public_state_before,
            "public apply must not inject historical cleartext state"
        );

        let mut ledger = store.read_ledger().expect("read ledger");
        let mut shielded = store.read_shielded().expect("read shielded state");
        let before = shielded.clone();
        let live_receipts = execute_shielded_batch(
            &genesis,
            &mut ledger,
            &mut shielded,
            &batch,
            1,
            AssetExecutionCompatibility::strict(),
            false,
            false,
        );
        assert_eq!(live_receipts[0].code, "legacy_cleartext_shielded_action_disabled");
        assert_eq!(shielded, before);

        let replay_receipts = execute_shielded_batch(
            &genesis,
            &mut ledger,
            &mut shielded,
            &batch,
            1,
            AssetExecutionCompatibility::strict(),
            false,
            true,
        );
        assert!(replay_receipts[0].accepted, "{replay_receipts:?}");

        fs::remove_dir_all(local_dir).expect("cleanup local debug pool test");
    }

    #[test]
    fn orchard_deposit_batch_locks_transparent_value_and_mints_spendable_note() {
        let data_dir = unique_test_dir("postfiat-orchard-direct-deposit-test");
        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-local".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 1,
        })
        .expect("init node");

        let orchard_key_file = data_dir.join("orchard-deposit-wallet.json");
        orchard_wallet_keygen(OrchardWalletKeygenOptions {
            master_seed_hex: bytes_to_hex(&[11u8; 32]),
            account_index: 0,
            key_file: orchard_key_file.clone(),
            overwrite: false,
        })
        .expect("generate deposit recipient");
        let orchard_view_key_file = data_dir.join("orchard-deposit-view.json");
        orchard_view_key_export(OrchardViewKeyExportOptions {
            key_file: orchard_key_file,
            view_key_file: orchard_view_key_file.clone(),
            overwrite: false,
        })
        .expect("export deposit recipient view key");

        let store = NodeStore::new(&data_dir);
        let funding_from = store
            .read_ledger()
            .expect("read ledger before deposit")
            .accounts
            .first()
            .expect("faucet account")
            .address
            .clone();
        let funding_balance_before = store
            .read_ledger()
            .expect("read balance before deposit")
            .account(&funding_from)
            .expect("funding account exists")
            .balance;
        let sink_balance_before = store
            .read_ledger()
            .expect("read sink before deposit")
            .account(FEE_COLLECTOR_ADDRESS)
            .map(|account| account.balance)
            .unwrap_or_default();

        let deposit_file = data_dir.join("orchard-direct-deposit.json");
        let deposit_report = create_orchard_deposit_action(OrchardDepositActionOptions {
            data_dir: data_dir.clone(),
            key_file: None,
            recipient_address_raw_hex: None,
            recipient_key_file: None,
            recipient_view_key_file: Some(orchard_view_key_file.clone()),
            memo_hex: None,
            amount: 25,
            fee: 0,
            policy_id: None,
            disclosure_hash: None,
            deposit_file: deposit_file.clone(),
            overwrite: false,
        })
        .expect("create direct Orchard deposit");
        assert_eq!(deposit_report.schema, ORCHARD_DEPOSIT_ACTION_REPORT_SCHEMA);
        assert_eq!(deposit_report.from, funding_from);
        assert_eq!(deposit_report.amount, 25);
        assert!(deposit_report.fee >= ORCHARD_FEE_BURN_MIN_FEE);
        assert_eq!(deposit_report.fee, deposit_report.minimum_fee);
        assert_eq!(deposit_report.value_balance, -25);
        assert_eq!(deposit_report.external_binding_hash.len(), 96);

        let batch_file = data_dir.join("orchard-direct-deposit.batch.json");
        let batch = create_orchard_deposit_action_batch(OrchardDepositActionBatchOptions {
            data_dir: data_dir.clone(),
            deposit_file,
            batch_file: batch_file.clone(),
        })
        .expect("create direct Orchard deposit batch");
        assert!(matches!(
            batch.actions.first(),
            Some(ShieldedAction::OrchardDepositV1(_))
        ));

        let receipts = apply_shielded_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file,
            certificate_file: None,
        })
        .expect("apply direct Orchard deposit");
        assert_eq!(receipts.len(), 1);
        assert!(receipts[0].accepted, "{receipts:?}");
        assert_eq!(
            receipts[0].fee_charged,
            deposit_report.fee + deposit_report.funding_transfer_fee
        );
        assert_eq!(receipts[0].fee_burned, receipts[0].fee_charged);

        let ledger_after = store.read_ledger().expect("read ledger after deposit");
        let funding_balance_after = ledger_after
            .account(&funding_from)
            .expect("funding account after deposit")
            .balance;
        assert_eq!(
            funding_balance_after,
            funding_balance_before
                - deposit_report.amount
                - deposit_report.fee
                - deposit_report.funding_transfer_fee
        );
        let sink_balance_after = ledger_after
            .account(FEE_COLLECTOR_ADDRESS)
            .map(|account| account.balance)
            .unwrap_or_default();
        assert_eq!(sink_balance_after, sink_balance_before);

        let shielded_report = verify_shielded(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("verify direct Orchard deposit state");
        assert_eq!(shielded_report.orchard_turnstile_deposit_total, 25);
        assert_eq!(shielded_report.orchard_value_balance_total, -25);
        assert_eq!(shielded_report.orchard_deposit_total, 25);
        assert_eq!(shielded_report.turnstile_event_count, 1);

        let scan = orchard_wallet_scan(OrchardWalletScanOptions {
            data_dir,
            spending_key_hex: None,
            key_file: None,
            view_key_file: Some(orchard_view_key_file),
        })
        .expect("scan direct Orchard deposit output");
        assert_eq!(scan.decrypted_count, 1);
        assert!(scan
            .outputs
            .iter()
            .any(|output| output.value == 25 && !output.spent));
    }

    #[test]
    fn direct_rejection_id_commits_to_chain_domain() {
        let local_genesis = Genesis::new("postfiat-local");
        let other_genesis = Genesis::new("postfiat-other");
        let seed = ("missing-note", 10_u64, "missing_note");
        let local_id =
            direct_rejection_id(&local_genesis, "postfiat.test.direct_rejection.v1", &seed)
                .expect("local rejection id");
        let other_id =
            direct_rejection_id(&other_genesis, "postfiat.test.direct_rejection.v1", &seed)
                .expect("other rejection id");

        assert_eq!(local_id.len(), 96);
        assert_ne!(local_id, other_id);
    }

    #[test]
    fn direct_bridge_domain_receipt_id_commits_to_chain_domain_and_operation() {
        let local_genesis = Genesis::new("postfiat-local");
        let other_genesis = Genesis::new("postfiat-other");
        let domain = BridgeDomain::with_metadata(BridgeDomainSpec {
            domain_id: "xrpl-mainnet".to_string(),
            name: "XRPL Mainnet".to_string(),
            source_chain: "xrpl".to_string(),
            target_chain: "postfiat-local".to_string(),
            bridge_id: "bridge-xrpl".to_string(),
            door_account: "rDoor".to_string(),
            inbound_cap: 1_000,
            outbound_cap: 500,
        });

        let local_upsert = direct_bridge_domain_receipt_id(&local_genesis, "upsert", &domain)
            .expect("local direct bridge domain receipt id");
        let other_upsert = direct_bridge_domain_receipt_id(&other_genesis, "upsert", &domain)
            .expect("other direct bridge domain receipt id");
        let local_pause = direct_bridge_domain_receipt_id(&local_genesis, "pause", &domain)
            .expect("local direct bridge pause receipt id");

        assert_eq!(local_upsert.len(), 96);
        assert_ne!(local_upsert, other_upsert);
        assert_ne!(local_upsert, local_pause);
    }

    #[test]
    fn bridge_witness_chain_domain_mismatch_is_rejected() {
        let genesis = Genesis::new("postfiat-local");
        let mut action = BridgeTransferAction {
            domain_id: "xrpl-test".to_string(),
            direction: BRIDGE_DIRECTION_INBOUND.to_string(),
            from: "xrpl:rSource".to_string(),
            to: "pfrecipient".to_string(),
            asset_id: "XRP".to_string(),
            amount: 1,
            witness_id: "witness-1".to_string(),
            witness_epoch: 1,
            witness_attestation: Some(BridgeWitnessAttestation {
                attestation_id: "a".repeat(96),
                chain_id: genesis.chain_id.clone(),
                genesis_hash: genesis_hash(&genesis),
                protocol_version: genesis.protocol_version,
                signer: "validator-0".to_string(),
                algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
                public_key_hex: "b".repeat(96),
                signature_hex: "c".repeat(96),
            }),
        };
        assert!(bridge_witness_chain_domain_error(&action, &genesis).is_none());

        action
            .witness_attestation
            .as_mut()
            .expect("witness attestation")
            .chain_id = "postfiat-other".to_string();
        let (code, message) = bridge_witness_chain_domain_error(&action, &genesis)
            .expect("wrong chain domain must fail");
        assert_eq!(code, "bad_witness_chain_domain");
        assert!(message.contains("postfiat-other"));
        assert!(message.contains("postfiat-local"));
    }

    #[test]
    fn init_rejects_malformed_chain_id() {
        let data_dir = std::env::temp_dir().join(format!(
            "postfiat-node-bad-chain-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));

        let error = init(InitOptions {
            data_dir,
            chain_id: " postfiat-local".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 1,
        })
        .expect_err("malformed chain id must fail");

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    }

    #[test]
    fn topology_rejects_malformed_chain_id() {
        let output_file = std::env::temp_dir().join(format!(
            "postfiat-topology-bad-chain-test-{}.json",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));

        let error = write_local_topology(TopologyOptions {
            chain_id: "\tpostfiat-local".to_string(),
            validators: 4,
            base_port: 39000,
            rpc_base_port: None,
            hosts: None,
            output_file,
        })
        .expect_err("malformed topology chain id must fail");

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    }

    #[test]
    fn local_json_reader_rejects_oversized_artifacts_before_parse() {
        let path = std::env::temp_dir().join(format!(
            "postfiat-oversized-json-test-{}.json",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let file = fs::File::create(&path).expect("create oversized artifact");
        file.set_len(MAX_LOCAL_JSON_FILE_BYTES + 1)
            .expect("size oversized artifact");

        let error = read_batch_file(&path).expect_err("oversized batch artifact must fail");

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        fs::remove_file(path).expect("remove oversized artifact");
    }

    #[test]
    fn wallet_keygen_restore_round_trips_without_report_secret_leakage() {
        let data_dir = std::env::temp_dir().join(format!(
            "postfiat-wallet-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let key_file = data_dir.join("wallet.key.json");
        let restored_key_file = data_dir.join("wallet.restored.key.json");
        let backup_file = data_dir.join("wallet.backup.json");
        let master_seed_hex = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";

        let report = wallet_keygen(WalletKeygenOptions {
            chain_id: "postfiat-local".to_string(),
            master_seed_hex: master_seed_hex.to_string(),
            account_index: 7,
            key_file: key_file.clone(),
            backup_file: backup_file.clone(),
            overwrite: false,
        })
        .expect("wallet keygen");
        assert_eq!(report.schema, WALLET_KEY_REPORT_SCHEMA);
        assert_eq!(report.operation, "keygen");
        assert_eq!(report.account_index, 7);
        assert!(report.private_key_material_redacted);

        let key = read_key_file(&key_file).expect("wallet key file");
        let backup = read_wallet_backup_file(&backup_file).expect("wallet backup file");
        assert_eq!(backup.schema, WALLET_BACKUP_FILE_SCHEMA);
        assert_eq!(backup.master_seed_hex, master_seed_hex);
        assert_eq!(key.address, report.address);
        assert_eq!(key.public_key_hex, report.public_key_hex);

        let restore_report = wallet_restore(WalletRestoreOptions {
            backup_file: backup_file.clone(),
            key_file: restored_key_file.clone(),
            overwrite: false,
        })
        .expect("wallet restore");
        let restored_key = read_key_file(&restored_key_file).expect("restored key file");
        assert_eq!(restored_key, key);
        assert_eq!(restore_report.operation, "restore");
        assert_eq!(restore_report.address, report.address);
        assert!(restore_report.private_key_material_redacted);

        let report_json = serde_json::to_string(&report).expect("wallet report json");
        let restore_report_json =
            serde_json::to_string(&restore_report).expect("restore report json");
        for json in [report_json, restore_report_json] {
            assert!(!json.contains("private_key_hex"));
            assert!(!json.contains("master_seed_hex"));
            assert!(!json.contains(master_seed_hex));
        }

        let overwrite_error = wallet_restore(WalletRestoreOptions {
            backup_file,
            key_file,
            overwrite: false,
        })
        .expect_err("restore should not overwrite an existing key without opt-in");
        assert_eq!(overwrite_error.kind(), io::ErrorKind::AlreadyExists);

        fs::remove_dir_all(data_dir).expect("remove wallet test directory");
    }

    #[test]
    fn wallet_test_vector_is_deterministic_and_redacted() {
        let options = WalletTestVectorOptions {
            chain_id: "postfiat-vector-test".to_string(),
            validator_count: 5,
            master_seed_hex: "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f"
                .to_string(),
            account_index: 0,
            to: "pfvectortestrecipient000000000000000001".to_string(),
            amount: 17,
            sequence: 1,
            signature_seed_hex: "1f1e1d1c1b1a191817161514131211100f0e0d0c0b0a09080706050403020100"
                .to_string(),
        };
        let first = wallet_test_vector(options.clone()).expect("first wallet vector");
        let second = wallet_test_vector(options.clone()).expect("second wallet vector");
        assert_eq!(first, second);
        assert_eq!(first.schema, "postfiat-wallet-test-vector-v2");
        assert_eq!(first.algorithm_id, ML_DSA_65_ALGORITHM);
        assert_eq!(first.kdf, WALLET_DERIVATION_KDF);
        assert_eq!(first.derivation_domain, WALLET_DERIVATION_DOMAIN);
        assert_eq!(first.account_index, 0);
        assert_eq!(first.validator_count, 5);
        assert_eq!(
            first.genesis_hash,
            "aa450fa2d12a9d10e290266df634b9319d8805450a85bedcbd4d72ec9dbbc7c4d4cca350a3202bf1e58ee01b4dbca732"
        );
        assert_eq!(first.address, "pf857c81edb95af1d64262ed6c0fdcf3ef7aff56fe");
        assert_eq!(first.address, first.signed_transfer.unsigned.from);
        assert_eq!(first.public_key_hex, first.signed_transfer.public_key_hex);
        assert_eq!(
            first.minimum_fee,
            minimum_transfer_fee(&first.signed_transfer)
                .saturating_add(TRANSFER_ACCOUNT_CREATION_FEE)
        );
        let expected_transfer_signing_bytes = concat!(
            "postfiat.transfer.v1\n",
            "chain_id=postfiat-vector-test\n",
            "genesis_hash=aa450fa2d12a9d10e290266df634b9319d8805450a85bedcbd4d72ec9dbbc7c4d4cca350a3202bf1e58ee01b4dbca732\n",
            "protocol_version=1\n",
            "address_namespace=postfiat.address.v1\n",
            "transaction_kind=transparent_transfer\n",
            "signature_algorithm_id=ML-DSA-65\n",
            "from=pf857c81edb95af1d64262ed6c0fdcf3ef7aff56fe\n",
            "to=pfvectortestrecipient000000000000000001\n",
            "amount=17\n",
            "fee=32\n",
            "sequence=1\n"
        );
        assert_eq!(
            first.transfer_signing_bytes_hex,
            bytes_to_hex(expected_transfer_signing_bytes.as_bytes())
        );
        assert_eq!(
            String::from_utf8(
                hex_to_bytes(&first.transfer_signing_bytes_hex).expect("signing bytes hex")
            )
            .expect("utf8 signing bytes"),
            expected_transfer_signing_bytes
        );
        assert_eq!(
            first.transfer_signing_hash,
            "b7ded680cf9ee025253aa8dcedde3b1e090f5853ea681fb240cf8579b5e4de8cf665df97e231d0d565ff91c37dc84189"
        );
        assert_eq!(
            first.tx_id,
            "99991ac80058445af49c45ace67bdc837fcd0dff85f4a15aae3ec05640fc9db1a2ae2270ec4653ec5aa59b262e59c764"
        );
        assert_eq!(first.tx_id, transfer_tx_id(&first.signed_transfer));
        assert!(first.signature_verified);
        assert!(first.private_key_material_redacted);

        let json = serde_json::to_string(&first).expect("wallet vector json");
        assert!(!json.contains("private_key_hex"));
        assert!(!json.contains("master_seed_hex"));
        assert!(!json.contains("signature_seed_hex"));
        assert!(!json.contains(&options.master_seed_hex));
        assert!(!json.contains(&options.signature_seed_hex));

        let mut other_options = options;
        other_options.account_index = 1;
        let other = wallet_test_vector(other_options).expect("other wallet vector");
        assert_ne!(first.address, other.address);
        assert_ne!(first.public_key_hex, other.public_key_hex);
    }

    #[test]
    fn wallet_sign_transfer_emits_submit_ready_redacted_transfer() {
        let data_dir = std::env::temp_dir().join(format!(
            "postfiat-wallet-sign-transfer-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        fs::create_dir_all(&data_dir).expect("create wallet sign-transfer test directory");
        let key_file = data_dir.join("wallet.key.json");
        let backup_file = data_dir.join("wallet.backup.json");
        let master_seed_hex = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";
        let key_report = wallet_keygen(WalletKeygenOptions {
            chain_id: "postfiat-wallet-sign-transfer".to_string(),
            master_seed_hex: master_seed_hex.to_string(),
            account_index: 0,
            key_file: key_file.clone(),
            backup_file,
            overwrite: false,
        })
        .expect("wallet keygen");
        let genesis =
            Genesis::try_new_with_validator_count("postfiat-wallet-sign-transfer".to_string(), 1)
                .expect("genesis");
        let signed = wallet_sign_transfer(WalletSignTransferOptions {
            key_file,
            chain_id: genesis.chain_id.clone(),
            genesis_hash: genesis_hash(&genesis),
            protocol_version: genesis.protocol_version,
            to: "pfwalletsignrecipient00000000000000001".to_string(),
            amount: 17,
            fee: 64,
            sequence: 1,
        })
        .expect("wallet sign transfer");

        assert_eq!(signed.unsigned.chain_id, genesis.chain_id);
        assert_eq!(signed.unsigned.genesis_hash, genesis_hash(&genesis));
        assert_eq!(signed.unsigned.protocol_version, genesis.protocol_version);
        assert_eq!(signed.unsigned.from, key_report.address);
        assert_eq!(signed.public_key_hex, key_report.public_key_hex);
        let public_key = hex_to_bytes(&signed.public_key_hex).expect("public key");
        let signature = hex_to_bytes(&signed.signature_hex).expect("signature");
        assert!(ml_dsa_65_verify(
            &public_key,
            &signed.unsigned.signing_bytes(),
            &signature
        ));

        let json = serde_json::to_string(&signed).expect("signed transfer json");
        assert!(!json.contains("private_key_hex"));
        assert!(!json.contains(master_seed_hex));

        let mut ledger = LedgerState::new(vec![Account::new(key_report.address, 200, None)]);
        let receipt = execute_transfer(&genesis, &mut ledger, &signed);
        assert!(receipt.accepted, "{receipt:?}");
        assert_eq!(receipt.tx_id, transfer_tx_id(&signed));

        fs::remove_dir_all(data_dir).expect("remove wallet sign-transfer test directory");
    }

    #[test]
    fn read_query_limits_are_bounded() {
        assert_eq!(
            bounded_read_query_limit(None, "blocks").expect("default limit"),
            MAX_READ_QUERY_LIMIT
        );
        assert_eq!(
            bounded_read_query_limit(Some(7), "blocks").expect("explicit limit"),
            7
        );
        assert!(bounded_read_query_limit(Some(0), "blocks").is_err());
        assert!(bounded_read_query_limit(Some(MAX_READ_QUERY_LIMIT + 1), "blocks").is_err());
        assert_eq!(
            bounded_read_query_limit_with_max(
                Some(postfiat_types::MAX_OWNED_INPUTS_PER_TRANSFER),
                "owned_objects",
                postfiat_types::MAX_OWNED_INPUTS_PER_TRANSFER,
            )
            .expect("owned_objects protocol-sized limit"),
            postfiat_types::MAX_OWNED_INPUTS_PER_TRANSFER
        );
        assert!(
            bounded_read_query_limit_with_max(
                Some(postfiat_types::MAX_OWNED_INPUTS_PER_TRANSFER + 1),
                "owned_objects",
                postfiat_types::MAX_OWNED_INPUTS_PER_TRANSFER,
            )
            .is_err()
        );
    }

    #[test]
    fn block_query_from_height_returns_forward_range() {
        let data_dir = std::env::temp_dir().join(format!(
            "postfiat-block-range-query-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        fs::create_dir_all(&data_dir).expect("create data dir");
        let store = NodeStore::new(&data_dir);
        store
            .write_blocks(&BlockLog {
                blocks: (1..=5).map(dummy_block_record).collect(),
            })
            .expect("write blocks");

        let latest = blocks(BlockQueryOptions {
            data_dir: data_dir.clone(),
            from_height: None,
            limit: Some(2),
        })
        .expect("latest blocks");
        assert_eq!(
            latest
                .iter()
                .map(|block| block.header.height)
                .collect::<Vec<_>>(),
            vec![4, 5]
        );

        let forward_range = blocks(BlockQueryOptions {
            data_dir: data_dir.clone(),
            from_height: Some(2),
            limit: Some(3),
        })
        .expect("forward range blocks");
        assert_eq!(
            forward_range
                .iter()
                .map(|block| block.header.height)
                .collect::<Vec<_>>(),
            vec![2, 3, 4]
        );

        fs::remove_dir_all(data_dir).expect("cleanup block range query data");
    }

    #[test]
    fn signed_transfer_mempool_admission_rejects_bad_signature_without_persisting() {
        let data_dir = std::env::temp_dir().join(format!(
            "postfiat-signed-transfer-mempool-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-local".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 1,
        })
        .expect("init");
        let store = NodeStore::new(&data_dir);
        let genesis = store.read_genesis().expect("genesis");
        let ledger = store.read_ledger().expect("ledger");
        let signed = build_signed_transfer(
            &genesis,
            &ledger,
            &data_dir,
            None,
            "pfexternalsigned0000000000000000000".to_string(),
            ACCOUNT_RESERVE,
        )
        .expect("build signed transfer");
        let transfer_file = data_dir.join("external-transfer.json");
        let mut bad_signature = signed.clone();
        bad_signature.signature_hex = "00".repeat(signed.signature_hex.len() / 2);
        write_signed_transfer_file(&transfer_file, &bad_signature).expect("write bad transfer");

        let bad_error = submit_signed_transfer_to_mempool(SignedTransferSubmitOptions {
            data_dir: data_dir.clone(),
            transfer_file: transfer_file.clone(),
        })
        .expect_err("bad external signature should not be admitted");
        assert!(
            bad_error.to_string().contains("bad_signature"),
            "{bad_error}"
        );
        assert!(mempool_state(NodeOptions {
            data_dir: data_dir.clone()
        })
        .expect("mempool after rejected external transfer")
        .is_empty());

        write_signed_transfer_file(&transfer_file, &signed).expect("write valid transfer");
        let admitted = submit_signed_transfer_to_mempool(SignedTransferSubmitOptions {
            data_dir: data_dir.clone(),
            transfer_file: transfer_file.clone(),
        })
        .expect("valid external transfer should be admitted");
        assert_eq!(admitted.tx_id, transfer_tx_id(&signed));
        let duplicate_error = submit_signed_transfer_to_mempool(SignedTransferSubmitOptions {
            data_dir: data_dir.clone(),
            transfer_file,
        })
        .expect_err("duplicate external transfer should not be admitted twice");
        assert!(
            duplicate_error.to_string().contains("already pending"),
            "{duplicate_error}"
        );
        assert_eq!(
            mempool_state(NodeOptions {
                data_dir: data_dir.clone()
            })
            .expect("mempool after duplicate external transfer")
            .len(),
            1
        );
        fs::remove_dir_all(data_dir).expect("cleanup signed transfer mempool data");
    }

    #[test]
    fn payment_v2_memo_flows_through_mempool_batch_finality_and_account_tx() {
        let data_dir = std::env::temp_dir().join(format!(
            "postfiat-payment-v2-mempool-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-local".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 1,
        })
        .expect("init");
        let store = NodeStore::new(&data_dir);
        let genesis = store.read_genesis().expect("genesis");
        let key_file = read_transfer_key_file(&data_dir, None).expect("faucet key");
        let memo = PaymentMemo {
            memo_type: "74657874".to_string(),
            memo_format: "746578742f706c61696e".to_string(),
            memo_data: "68656c6c6f2d7061796d656e742d7632".to_string(),
        };
        let quote = transfer_fee_quote(TransferFeeQuoteOptions {
            data_dir: data_dir.clone(),
            from: key_file.address.clone(),
            to: "pfpaymentv2recipient0000000000000001".to_string(),
            amount: ACCOUNT_RESERVE,
            sequence: None,
            memo_type: Some(memo.memo_type.clone()),
            memo_format: Some(memo.memo_format.clone()),
            memo_data: Some(memo.memo_data.clone()),
        })
        .expect("payment v2 fee quote");
        assert_eq!(
            quote.transaction_kind.as_deref(),
            Some(PAYMENT_V2_TRANSACTION_KIND)
        );
        assert_eq!(quote.memo_count, Some(1));
        assert_eq!(
            quote.memo_bytes,
            Some(u64::try_from(memo.byte_len()).expect("memo byte len"))
        );

        let unsigned = UnsignedPaymentV2 {
            chain_id: genesis.chain_id.clone(),
            genesis_hash: genesis_hash(&genesis),
            protocol_version: genesis.protocol_version,
            address_namespace: postfiat_types::ADDRESS_NAMESPACE.to_string(),
            transaction_kind: PAYMENT_V2_TRANSACTION_KIND.to_string(),
            signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            from: key_file.address.clone(),
            to: quote.to.clone(),
            amount: quote.amount,
            fee: quote.minimum_fee,
            sequence: quote.sequence,
            memos: vec![memo.clone()],
        };
        unsigned.validate().expect("unsigned payment validates");
        let private_key = hex_to_bytes(&key_file.private_key_hex).expect("private key bytes");
        let signature = ml_dsa_65_sign(&private_key, &unsigned.signing_bytes()).expect("sign");
        let signed = SignedPaymentV2 {
            unsigned,
            algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            public_key_hex: key_file.public_key_hex.clone(),
            signature_hex: bytes_to_hex(&signature),
        };
        signed.validate().expect("signed payment validates");

        let signed_json = serde_json::to_string(&signed).expect("signed payment json");
        let admitted = submit_signed_payment_v2_json_to_mempool(
            SignedPaymentV2JsonSubmitOptions {
                data_dir: data_dir.clone(),
                signed_payment_v2_json: signed_json,
            },
        )
        .expect("admit payment v2");
        assert_eq!(admitted.tx_id, payment_v2_tx_id(&signed));
        let mempool = mempool_state(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("mempool state");
        assert_eq!(mempool.pending.len(), 0);
        assert_eq!(mempool.pending_payment_v2.len(), 1);

        let batch_file = data_dir.join("payment-v2-batch.json");
        let batch = create_mempool_batch(MempoolBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: batch_file.clone(),
            max_transactions: 10,
        })
        .expect("create payment v2 mempool batch");
        assert!(batch.transactions.is_empty());
        assert_eq!(batch.payments_v2.len(), 1);
        assert!(mempool_state(NodeOptions {
            data_dir: data_dir.clone()
        })
        .expect("mempool after batch")
        .is_empty());

        let receipts = apply_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file,
            certificate_file: None,
        })
        .expect("apply payment v2 batch");
        assert_eq!(receipts.len(), 1);
        assert!(receipts[0].accepted, "{receipts:?}");
        let finality = tx_finality(TxFinalityQueryOptions {
            data_dir: data_dir.clone(),
            tx_id: receipts[0].tx_id.clone(),
            audit_block_log: true,
        })
        .expect("payment v2 finality");
        assert!(finality.confirmed);
        assert_eq!(finality.receipt_count, 1);

        let account_history = account_tx(AccountTxQueryOptions {
            data_dir: data_dir.clone(),
            address: key_file.address.clone(),
            from_height: Some(1),
            to_height: Some(1),
            limit: Some(10),
        })
        .expect("payment v2 account tx");
        assert_eq!(account_history.row_count, 1);
        let row = &account_history.rows[0];
        assert_eq!(row.tx_id, receipts[0].tx_id);
        assert_eq!(row.transaction_index, 0);
        assert_eq!(row.transaction_kind, PAYMENT_V2_TRANSACTION_KIND);
        assert_eq!(row.memo_count, Some(1));
        assert_eq!(
            row.memo_bytes,
            Some(u64::try_from(memo.byte_len()).expect("memo byte len"))
        );
        assert!(row.memo_hash.as_deref().is_some_and(|hash| hash.len() == 96));
        assert_eq!(row.accepted, Some(true));

        fs::remove_dir_all(data_dir).expect("cleanup payment v2 mempool data");
    }
