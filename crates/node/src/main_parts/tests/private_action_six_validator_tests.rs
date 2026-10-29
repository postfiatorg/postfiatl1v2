#[cfg(test)]
mod private_action_six_validator_tests {
    //! S6: user-proven shielded actions through the validator-owned admission
    //! service on an owned six-validator network. The gateway's role is played
    //! by submitting the wallet's exact action text to every validator's
    //! `private_action_submit`; each validator's round service holds the record
    //! until it is the proposer, dispatches the reserved batch through the
    //! certified transport round, and every validator serves the receipt.

    use super::*;
    use postfiat_node::{
        create_asset_orchard_ingress, private_action_hash_for_json, private_action_intent_id,
        private_action_receipt, private_action_service_step, private_action_submit,
        private_pool_page, private_pool_snapshot, AssetOrchardIngressCreateOptions,
        AssetOrchardIngressFile, PrivateActionAdmissionStore, PrivateActionDispatch,
        PrivateActionRecord, PrivateActionState, PrivateActionSubmitDomain,
        PrivateActionSubmitRequest, PrivatePoolPageItems, PrivatePoolPageOptions,
        PrivatePoolSnapshotOptions, PRIVATE_ACTION_FINALITY_KIND, PRIVATE_ACTION_POOL_ID,
    };
    use postfiat_node::DevKeyFile;
    use postfiat_types::{
        AssetCreateOperation, AssetTransactionOperation, Genesis,
        IssuedPaymentOperation, LedgerState, ShieldedAction, SignedAssetTransaction,
        TrustSetOperation, UnsignedAssetTransaction,
    };
    use std::io;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// Per validator: commitments, nullifiers, the holder's line balances, tip.
    pub(crate) type PoolFingerprint = (usize, usize, Vec<(String, u64)>, String);

    pub(crate) const VALIDATORS: usize = 6;
    /// Consensus v2 stays inactive: every round is the legacy certified round
    /// with a BFT quorum certificate (one block vote per peer).
    const CONSENSUS_V2_ACTIVATION: u64 = 1_000;
    /// Atomic shielded batches activate at this height by governance.
    pub(crate) const ATOMIC_ACTIVATION: u64 = 6;
    const CHAIN_ID: &str = "postfiat-private-action-six-validator-test";

    pub(crate) struct Fleet {
        pub(crate) root: PathBuf,
        pub(crate) data_dirs: Vec<PathBuf>,
        topology_file: PathBuf,
        listeners: Vec<TcpListener>,
        validator_ids: Vec<String>,
        pub(crate) holder_key_file: PathBuf,
        pub(crate) asset_ids: [String; 2],
        /// A third issued asset that no route names, when requested.
        pub(crate) extra_asset_id: Option<String>,
    }

    fn reserve_ports(count: usize) -> (u16, Vec<TcpListener>) {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .subsec_nanos() as u16;
        for offset in 0..512u16 {
            let base = 30_000u16 + ((seed.wrapping_add(offset * 7)) % 20_000);
            let listeners = (0..count)
                .map(|index| TcpListener::bind(("127.0.0.1", base + index as u16 * 2)))
                .collect::<io::Result<Vec<_>>>();
            if let Ok(listeners) = listeners {
                return (base, listeners);
            }
        }
        panic!("could not reserve {count} transport ports");
    }

    pub(crate) fn write_private_json<T: serde::Serialize>(path: &Path, value: &T) {
        std::fs::write(path, serde_json::to_vec_pretty(value).expect("json")).expect("write json");
        #[cfg(unix)]
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .expect("protect json");
    }

    fn signed_asset_transaction(
        genesis: &Genesis,
        ledger: &LedgerState,
        key: &DevKeyFile,
        transaction_kind: &str,
        sequence: u64,
        operation: AssetTransactionOperation,
    ) -> SignedAssetTransaction {
        let private_key = postfiat_crypto_provider::hex_to_bytes(&key.private_key_hex).expect("private key bytes");
        let mut fee = 1;
        for _ in 0..8 {
            let unsigned = UnsignedAssetTransaction {
                chain_id: genesis.chain_id.clone(),
                genesis_hash: postfiat_execution::genesis_hash(genesis),
                protocol_version: genesis.protocol_version,
                address_namespace: postfiat_types::ADDRESS_NAMESPACE.to_string(),
                transaction_kind: transaction_kind.to_string(),
                signature_algorithm_id: postfiat_crypto_provider::ML_DSA_65_ALGORITHM.to_string(),
                source: key.address.clone(),
                fee,
                sequence,
                operation: operation.clone(),
            };
            let signature =
                postfiat_crypto_provider::ml_dsa_65_sign(&private_key, &unsigned.signing_bytes())
                    .expect("sign asset tx");
            let signed = SignedAssetTransaction {
                unsigned,
                algorithm_id: postfiat_crypto_provider::ML_DSA_65_ALGORITHM.to_string(),
                public_key_hex: key.public_key_hex.clone(),
                signature_hex: postfiat_crypto_provider::bytes_to_hex(&signature),
            };
            let minimum = postfiat_execution::minimum_asset_transaction_fee_for_ledger(ledger, &signed);
            if fee >= minimum {
                return signed;
            }
            fee = minimum;
        }
        panic!("asset transaction fee did not converge");
    }

    pub(crate) const NAV_PER_UNIT: u64 = 2 * postfiat_types::VAULT_BRIDGE_UNIT;

    /// The NAV state the pricing claim binds to and the governed route for the
    /// pair (no governance kind registers a route).
    fn write_pricing_state(primary: &Path, genesis: &Genesis, issuer: &str) {
        let store = NodeStore::new(primary);
        let base = postfiat_types::AssetDefinition::new(&genesis.chain_id, issuer, "A666", 1, 6)
            .expect("A666 id");
        let quote = postfiat_types::AssetDefinition::new(&genesis.chain_id, issuer, "USDC", 1, 6)
            .expect("USDC id");
        let profile = postfiat_types::NavProofProfile::new(
            issuer,
            postfiat_types::NAV_PROFILE_VERIFIER_PLACEHOLDER,
            "s6-finalized-nav",
            100,
            1,
            10_000,
            0,
            0,
            0,
            0,
            "",
            "",
            "",
            0,
            0,
        )
        .expect("NAV profile");
        let mut nav = postfiat_types::NavTrackedAsset::new(
            base.asset_id.clone(),
            issuer,
            issuer,
            profile.profile_id.clone(),
            "USD",
            issuer,
        )
        .expect("NAV asset");
        nav.finalized_epoch = 7;
        nav.circulating_supply = 200;
        nav.nav_per_unit = NAV_PER_UNIT;
        nav.finalized_reserve_packet_hash = "ab".repeat(48);
        nav.finalized_at_height = 1;
        let mut ledger = store.read_ledger().expect("ledger");
        ledger.nav_proof_profiles.push(profile);
        ledger.nav_assets.push(nav);
        ledger.pftl_uniswap_routes.push(postfiat_types::PftlUniswapConsensusRouteState {
            route_id: "s6-a666-usdc-route".to_string(),
            route_family: postfiat_types::PFTL_UNISWAP_ROUTE_FAMILY_PRIMARY_MINT.to_string(),
            route_config_digest: "11".repeat(48),
            route_trust_class: "governed".to_string(),
            native_nav_asset_id: base.asset_id.clone(),
            settlement_asset_id: quote.asset_id.clone(),
            handoff_controller: "0x".to_string() + &"33".repeat(20),
            settlement_adapter: "0x".to_string() + &"44".repeat(20),
            wrapped_navcoin_token: "0x".to_string() + &"22".repeat(20),
            ethereum_chain_id: 1,
            route_supply_cap_atoms: 1_000_000,
            packet_notional_cap_atoms: 1_000,
            latest_finalized_nav_epoch: 7,
            return_finality_blocks: 10,
            live_value_enabled: false,
            ethereum_verification_policy: None,
            authorized_valid_supply_atoms: 0,
            pftl_spendable_supply_atoms: 0,
            native_spendable_balances_atoms: BTreeMap::new(),
            ethereum_spendable_supply_atoms: 0,
            other_registered_venue_supply_atoms: 0,
            outstanding_bridge_claims_atoms: 0,
            pending_return_import_claims_atoms: 0,
            settlement_reserve_atoms: 0,
            primary_subscription_nonces: BTreeMap::new(),
            export_packets: BTreeMap::new(),
            export_nonces: BTreeMap::new(),
            return_imports: BTreeMap::new(),
            paused: false,
            v2: None,
        });
        store.write_ledger(&ledger).expect("write pricing ledger");
    }

    /// Builds the common pre-state on validator-0 (governed activation set at
    /// genesis, a funded holder, two issued assets with trustlines) and
    /// restores it on every other validator.
    pub(crate) fn fleet() -> Fleet {
        fleet_with(false)
    }

    /// With `pricing`, the common state also carries a NAV-tracked base asset,
    /// a governed A666/USDC route and the signed pricing enforcement activation.
    pub(crate) fn fleet_with(pricing: bool) -> Fleet {
        fleet_with_options(FleetOptions {
            pricing,
            max_nav_age_blocks: None,
            extra_asset: false,
        })
    }

    pub(crate) struct FleetOptions {
        pub(crate) pricing: bool,
        /// Signed governance for `asset_orchard_pricing_max_nav_age_blocks`.
        pub(crate) max_nav_age_blocks: Option<u32>,
        /// Issue a third asset ("EURC") with a holder trustline.
        pub(crate) extra_asset: bool,
    }

    pub(crate) fn fleet_with_options(options: FleetOptions) -> Fleet {
        let pricing = options.pricing;
        let root = std::env::temp_dir().join(format!(
            "postfiat-s6-private-actions-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let data_dirs = (0..VALIDATORS)
            .map(|index| root.join(format!("validator-{index}")))
            .collect::<Vec<_>>();
        for (index, data_dir) in data_dirs.iter().enumerate() {
            init_consensus_v2(InitConsensusV2Options {
                data_dir: data_dir.clone(),
                chain_id: CHAIN_ID.to_string(),
                node_id: format!("validator-{index}"),
                validator_count: VALIDATORS as u32,
                activation_height: CONSENSUS_V2_ACTIVATION,
                storage_activation_height: None,
            })
            .expect("init validator");
        }
        let primary = &data_dirs[0];
        let store = NodeStore::new(primary);
        let genesis = store.read_genesis().expect("genesis");
        let validator_ids = (0..VALIDATORS)
            .map(|index| format!("validator-{index}"))
            .collect::<Vec<_>>();
        let faucet: DevKeyFile = serde_json::from_slice(
            &std::fs::read(primary.join("faucet_key.json")).expect("faucet key"),
        )
        .expect("parse faucet key");
        // Signed governance amendments authorized by all six validators: the
        // atomic shielded batch activation, and with `pricing` the pricing
        // enforcement activation.
        let all_keys: ValidatorKeyFile = serde_json::from_slice(
            &std::fs::read(primary.join(VALIDATOR_KEYS_FILE)).expect("validator keys"),
        )
        .expect("parse validator keys");
        std::fs::create_dir_all(&root).expect("root");
        let mut amendments = vec![(
            GOVERNANCE_KIND_SHIELDED_ATOMIC_BATCH_ACTIVATION_HEIGHT,
            ATOMIC_ACTIVATION as u32,
        )];
        if pricing {
            amendments.push((
                GOVERNANCE_KIND_ASSET_ORCHARD_PRICING_ENFORCEMENT_ACTIVATION_HEIGHT,
                3,
            ));
        }
        if let Some(blocks) = options.max_nav_age_blocks {
            amendments.push((
                postfiat_types::GOVERNANCE_KIND_ASSET_ORCHARD_PRICING_MAX_NAV_AGE_BLOCKS,
                blocks,
            ));
        }
        for (slot, (kind, value)) in amendments.into_iter().enumerate() {
            let slot = slot as u64 + 1;
            let unsigned = root.join(format!("governance-{slot}.unsigned.json"));
            ratify_governance(RatifyGovernanceOptions {
                data_dir: primary.clone(),
                validators: validator_ids.clone(),
                support: validator_ids.clone(),
                kind: kind.to_string(),
                value,
                activation_height: 0,
                veto_until_height: 0,
                paused: false,
                amendment_file: unsigned.clone(),
            })
            .expect("ratify amendment");
            let authorization_files = all_keys
                .validators
                .iter()
                .map(|record| {
                    let key_path = root.join(format!("{}.governance-key.json", record.node_id));
                    write_private_json(
                        &key_path,
                        &ValidatorKeyFile {
                            validators: vec![record.clone()],
                        },
                    );
                    let authorization =
                        root.join(format!("{}.authorization-{slot}.json", record.node_id));
                    sign_governance_amendment_authorization(GovernanceAuthorizationSignOptions {
                        data_dir: primary.clone(),
                        amendment_file: unsigned.clone(),
                        validator: record.node_id.clone(),
                        validator_key_file: key_path,
                        proposal_slot: slot,
                        expires_at_height: slot + 8,
                        authorization_file: authorization.clone(),
                    })
                    .expect("sign amendment");
                    authorization
                })
                .collect::<Vec<_>>();
            let signed = root.join(format!("governance-{slot}.signed.json"));
            assemble_signed_governance_amendment(GovernanceAmendmentAssembleOptions {
                data_dir: primary.clone(),
                amendment_file: unsigned,
                authorization_files,
                proposal_slot: slot,
                output_file: signed.clone(),
            })
            .expect("assemble amendment");
            let governance_batch = root.join(format!("governance-{slot}.batch.json"));
            create_governance_batch(GovernanceBatchOptions {
                data_dir: primary.clone(),
                amendment_file: Some(signed),
                registry_update_file: None,
                batch_file: governance_batch.clone(),
            })
            .expect("governance batch");
            let receipts = apply_governance_batch(ApplyBatchOptions {
                data_dir: primary.clone(),
                batch_file: governance_batch,
                certificate_file: None,
            })
            .expect("apply governance");
            assert!(receipts.iter().all(|receipt| receipt.accepted), "{receipts:?}");
        }
        assert_eq!(
            store.read_governance().unwrap().shielded_atomic_batch_activation_height(),
            Some(ATOMIC_ACTIVATION)
        );

        let holder_pair = postfiat_crypto_provider::ml_dsa_65_keygen_from_seed(&[17u8; 32]);
        let holder = DevKeyFile {
            algorithm_id: postfiat_crypto_provider::ML_DSA_65_ALGORITHM.to_string(),
            address: postfiat_crypto_provider::address_from_public_key(&holder_pair.public_key),
            public_key_hex: postfiat_crypto_provider::bytes_to_hex(&holder_pair.public_key),
            private_key_hex: postfiat_crypto_provider::bytes_to_hex(&holder_pair.private_key),
        };
        let holder_key_file = root.join("holder.key.json");
        write_private_json(&holder_key_file, &holder);

        let funding_file = root.join("setup-funding.batch.json");
        create_transfer_batch(BatchTransferOptions {
            data_dir: primary.clone(),
            key_file: Some(primary.join("faucet_key.json")),
            to: holder.address.clone(),
            amount: 100_000,
            batch_file: funding_file.clone(),
        })
        .expect("funding batch");
        assert!(apply_batch(ApplyBatchOptions {
            data_dir: primary.clone(),
            batch_file: funding_file,
            certificate_file: None,
        })
        .expect("apply funding")
        .iter()
        .all(|receipt| receipt.accepted));

        let ledger = store.read_ledger().expect("funded ledger");
        let mut dry_run = ledger.clone();
        let mut transactions = Vec::new();
        let mut asset_ids = Vec::new();
        let codes: &[&str] = if options.extra_asset {
            &["A666", "USDC", "EURC"]
        } else {
            &["A666", "USDC"]
        };
        let asset_count = codes.len() as u64;
        for (offset, code) in codes.iter().copied().enumerate() {
            let create = signed_asset_transaction(
                &genesis,
                &dry_run,
                &faucet,
                postfiat_types::ASSET_CREATE_TRANSACTION_KIND,
                2 + offset as u64,
                AssetTransactionOperation::AssetCreate(AssetCreateOperation {
                    issuer: faucet.address.clone(),
                    code: code.to_string(),
                    version: 1,
                    precision: 6,
                    display_name: format!("S6 {code}"),
                    max_supply: Some(1_000),
                    requires_authorization: false,
                    freeze_enabled: true,
                    clawback_enabled: false,
                }),
            );
            assert!(postfiat_execution::execute_asset_transaction(&genesis, &mut dry_run, &create, 1).accepted);
            asset_ids.push(dry_run.asset_definitions.last().unwrap().asset_id.clone());
            transactions.push(create);
        }
        for (offset, asset_id) in asset_ids.iter().enumerate() {
            let trust = signed_asset_transaction(
                &genesis,
                &dry_run,
                &holder,
                postfiat_types::TRUST_SET_TRANSACTION_KIND,
                1 + offset as u64,
                AssetTransactionOperation::TrustSet(TrustSetOperation {
                    account: holder.address.clone(),
                    issuer: faucet.address.clone(),
                    asset_id: asset_id.clone(),
                    limit: 500,
                    authorized: false,
                    frozen: false,
                    reserve_paid: postfiat_execution::TRUSTLINE_STATE_EXPANSION_FEE,
                }),
            );
            assert!(postfiat_execution::execute_asset_transaction(&genesis, &mut dry_run, &trust, 1).accepted);
            transactions.push(trust);
        }
        for (offset, asset_id) in asset_ids.iter().enumerate() {
            let issue = signed_asset_transaction(
                &genesis,
                &dry_run,
                &faucet,
                postfiat_types::ISSUED_PAYMENT_TRANSACTION_KIND,
                2 + asset_count + offset as u64,
                AssetTransactionOperation::IssuedPayment(IssuedPaymentOperation {
                    from: faucet.address.clone(),
                    to: holder.address.clone(),
                    issuer: faucet.address.clone(),
                    asset_id: asset_id.clone(),
                    amount: 200,
                }),
            );
            assert!(postfiat_execution::execute_asset_transaction(&genesis, &mut dry_run, &issue, 1).accepted);
            transactions.push(issue);
        }
        let domain = postfiat_mempool_dag::MempoolBatchDomain {
            chain_id: genesis.chain_id.clone(),
            genesis_hash: postfiat_execution::genesis_hash(&genesis),
            protocol_version: genesis.protocol_version,
        };
        let setup = postfiat_mempool_dag::build_mixed_transaction_batch_with_assets(
            &domain,
            Vec::new(),
            Vec::new(),
            transactions,
        )
        .expect("asset setup batch")
        .batch;
        let setup_file = root.join("setup-assets.batch.json");
        std::fs::write(&setup_file, serde_json::to_vec_pretty(&setup).unwrap()).unwrap();
        let receipts = apply_batch(ApplyBatchOptions {
            data_dir: primary.clone(),
            batch_file: setup_file,
            certificate_file: None,
        })
        .expect("apply asset setup");
        assert!(receipts.iter().all(|receipt| receipt.accepted), "{receipts:?}");
        // Common state for every validator, with each validator's own key.
        let shared_keys = std::fs::read(primary.join(VALIDATOR_KEYS_FILE)).expect("keys");
        let key_file: ValidatorKeyFile = serde_json::from_slice(&shared_keys).expect("keys json");
        let shared_registry = std::fs::read(primary.join(VALIDATOR_REGISTRY_FILE)).expect("registry");
        let snapshot = root.join("bootstrap-snapshot");
        export_snapshot(SnapshotExportOptions {
            data_dir: primary.clone(),
            snapshot_dir: snapshot.clone(),
        })
        .expect("export common state");
        for (index, data_dir) in data_dirs.iter().enumerate().skip(1) {
            std::fs::remove_dir_all(data_dir).expect("clear replica");
            import_snapshot(SnapshotImportOptions {
                data_dir: data_dir.clone(),
                snapshot_dir: snapshot.clone(),
                node_id: Some(format!("validator-{index}")),
            })
            .expect("import common state");
            std::fs::write(data_dir.join(VALIDATOR_REGISTRY_FILE), &shared_registry)
                .expect("registry");
        }
        for (index, data_dir) in data_dirs.iter().enumerate() {
            let record = key_file
                .validators
                .iter()
                .find(|record| record.node_id == format!("validator-{index}"))
                .expect("validator key")
                .clone();
            write_private_json(
                &data_dir.join(VALIDATOR_KEYS_FILE),
                &ValidatorKeyFile {
                    validators: vec![record],
                },
            );
        }
        if pricing {
            // The NAV state the claim binds to and the governed route reference
            // the issued assets. Snapshot export replays every block, so they
            // are written identically on all six validators after the import;
            // the next certified block's state root commits them.
            for data_dir in &data_dirs {
                write_pricing_state(data_dir, &genesis, &faucet.address);
            }
        }
        let topology_file = root.join("topology.json");
        let (base_port, listeners) = reserve_ports(VALIDATORS);
        write_consensus_v2_topology(TopologyConsensusV2Options {
            chain_id: CHAIN_ID.to_string(),
            validators: VALIDATORS as u32,
            base_port,
            rpc_base_port: Some(base_port.saturating_add(100)),
            hosts: Some(vec!["127.0.0.1".to_string(); VALIDATORS]),
            output_file: topology_file.clone(),
            activation_height: CONSENSUS_V2_ACTIVATION,
            storage_activation_height: None,
        })
        .expect("topology");
        Fleet {
            root,
            data_dirs,
            topology_file,
            listeners,
            validator_ids,
            holder_key_file,
            asset_ids: [asset_ids[0].clone(), asset_ids[1].clone()],
            extra_asset_id: asset_ids.get(2).cloned(),
        }
    }

    impl Fleet {
        pub(crate) fn height(&self) -> u64 {
            status(NodeOptions {
                data_dir: self.data_dirs[0].clone(),
            })
            .expect("status")
            .block_height
        }

        pub(crate) fn proposer_index(&self, height: u64) -> usize {
            let proposer = postfiat_ordering_fast::leader_for_view(&self.validator_ids, height, 0)
                .expect("proposer");
            self.validator_ids
                .iter()
                .position(|id| id == &proposer)
                .expect("proposer index")
        }

        /// One certified round on all six validators with the real transport.
        pub(crate) fn round(&self, kind: &str, batch_file: PathBuf) {
            let height = self.height() + 1;
            let proposer = self.proposer_index(height);
            let label = format!("h{height}");
            let mut handles = Vec::new();
            for (index, data_dir) in self.data_dirs.iter().enumerate() {
                if index == proposer {
                    continue;
                }
                let data_dir = data_dir.clone();
                let topology_file = self.topology_file.clone();
                let vote_dir = data_dir.join(format!("{label}-votes"));
                let key_file = data_dir.join(VALIDATOR_KEYS_FILE);
                let listener = self.listeners[index].try_clone().expect("listener");
                handles.push(std::thread::spawn(move || {
                    let shutdown = std::sync::atomic::AtomicBool::new(false);
                    crate::transport_runtime::transport_validator_serve_inner(
                        data_dir,
                        topology_file,
                        key_file,
                        vote_dir,
                        Some("127.0.0.1".to_string()),
                        2,
                        30_000,
                        None,
                        true,
                        Some(TransportShieldedVerifierPrewarmReport {
                            schema: "postfiat-transport-shielded-verifier-prewarm-v1".to_string(),
                            requested: true,
                            total_ms: 0.0,
                            asset_orchard_swap_verifier_warm: true,
                            asset_orchard_swap_verifier_ms: Some(0.0),
                            asset_orchard_private_egress_verifier_warm: true,
                            asset_orchard_private_egress_verifier_ms: Some(0.0),
                            asset_orchard_private_egress_verifier_breakdown: None,
                        }),
                        Some(listener),
                        &shutdown,
                    )
                }));
            }
            let data_dir = self.data_dirs[proposer].clone();
            transport_peer_certified_batch_round(TransportPeerCertifiedBatchRoundOptions {
                data_dir: data_dir.clone(),
                topology_file: self.topology_file.clone(),
                batch_kind: Some(kind.to_string()),
                batch_file,
                key_file: data_dir.join(VALIDATOR_KEYS_FILE),
                proposal_key_file: Some(data_dir.join(VALIDATOR_KEYS_FILE)),
                require_local_proposer: true,
                require_signed_proposal: true,
                allow_peer_failures: false,
                quorum_early_full_propagation: true,
                artifact_dir: self.root.join(format!("{label}-artifacts")),
                block_height: Some(height),
                view: Some(0),
                timeout_certificate_file: None,
                timeout_ms: 30_000,
                send_retries: 2,
                retry_backoff_ms: 50,
                local_apply_before_certified_send: true,
                defer_certified_sends: false,
                commit_processed_dir: None,
                required_parent: None,
            })
            .expect("certified round");
            crate::transport_runtime::clear_transport_vote_stream_pool_for_test()
                .expect("close vote sessions");
            for handle in handles {
                let report = handle.join().expect("validator thread").expect("validator service");
                assert!(report.rejected.is_empty(), "{:?}", report.rejected);
            }
            self.assert_converged(height);
        }

        pub(crate) fn assert_converged(&self, height: u64) {
            let statuses = self
                .data_dirs
                .iter()
                .map(|data_dir| {
                    status(NodeOptions {
                        data_dir: data_dir.clone(),
                    })
                    .expect("status")
                })
                .collect::<Vec<_>>();
            assert!(statuses.iter().all(|status| status.block_height == height));
            assert!(statuses.windows(2).all(|pair| pair[0].block_tip_hash == pair[1].block_tip_hash
                && pair[0].state_root == pair[1].state_root));
        }

        pub(crate) fn transparent_round(&self) {
            let height = self.height() + 1;
            let proposer = self.proposer_index(height);
            let batch = self.root.join(format!("h{height}-transfer.batch.json"));
            create_transfer_batch(BatchTransferOptions {
                data_dir: self.data_dirs[proposer].clone(),
                key_file: Some(self.data_dirs[0].join("faucet_key.json")),
                to: format!("pf{}", "a".repeat(40)),
                amount: 1,
                batch_file: batch.clone(),
            })
            .expect("transfer batch");
            self.round("transparent", batch);
        }

        pub(crate) fn ingress_action(&self, asset: usize, amount: u64, seed: u8) -> String {
            self.ingress_action_for(&self.asset_ids[asset].clone(), amount, seed)
        }

        pub(crate) fn ingress_action_for(&self, asset_id: &str, amount: u64, seed: u8) -> String {
            let ingress_file = self.root.join(format!("ingress-{seed}.json"));
            create_asset_orchard_ingress(AssetOrchardIngressCreateOptions {
                data_dir: self.data_dirs[0].clone(),
                key_file: self.holder_key_file.clone(),
                asset_id: asset_id.to_string(),
                amount,
                fee: 0,
                note_seed_hex: postfiat_crypto_provider::bytes_to_hex(&[seed; 32]),
                encrypted_output_hex: None,
                ingress_file: ingress_file.clone(),
                note_file: self.root.join(format!("note-{seed}.json")),
                overwrite: false,
            })
            .expect("ingress");
            let file: AssetOrchardIngressFile =
                serde_json::from_slice(&std::fs::read(&ingress_file).unwrap()).unwrap();
            serde_json::to_string(&ShieldedAction::AssetOrchardIngressV2(file.payload)).unwrap()
        }

        pub(crate) fn request(&self, action_json: &str) -> PrivateActionSubmitRequest {
            let genesis = NodeStore::new(&self.data_dirs[0]).read_genesis().unwrap();
            let domain = PrivateActionSubmitDomain {
                chain_id: genesis.chain_id.clone(),
                genesis_hash: postfiat_execution::genesis_hash(&genesis),
                protocol_version: genesis.protocol_version,
                pool_id: PRIVATE_ACTION_POOL_ID.to_string(),
                session_scope: "native:s6-install".to_string(),
            };
            PrivateActionSubmitRequest {
                intent_id: private_action_intent_id(&domain.intent_domain(), action_json).unwrap(),
                action_hash: private_action_hash_for_json(action_json).unwrap(),
                action_json: action_json.to_string(),
                domain,
            }
        }

        /// The gateway relays the same bytes to every validator.
        pub(crate) fn submit_everywhere(&self, request: &PrivateActionSubmitRequest) -> Vec<io::Result<PrivateActionRecord>> {
            self.data_dirs
                .iter()
                .map(|data_dir| private_action_submit(data_dir, request).map(|report| report.record))
                .collect()
        }

        /// One round-service pass on every validator. The next height's
        /// proposer dispatches through the certified round.
        pub(crate) fn service_pass(&self) -> usize {
            let mut rounds = 0;
            for (index, data_dir) in self.data_dirs.iter().enumerate() {
                private_action_service_step(
                    data_dir,
                    |height| Ok(self.proposer_index(height) == index),
                    |record, _batch| {
                        rounds += 1;
                        self.dispatch(record)
                    },
                )
                .expect("service step");
            }
            rounds
        }

        pub(crate) fn dispatch(&self, record: &PrivateActionRecord) -> PrivateActionDispatch {
            let batch_file = self.root.join(format!("reserved-{}.json", &record.batch_id[..16]));
            std::fs::write(&batch_file, &record.batch_json).expect("write reserved batch");
            self.round("shielded", batch_file);
            PrivateActionDispatch::Finished
        }

        pub(crate) fn state(&self, index: usize, intent_id: &str) -> Option<PrivateActionState> {
            PrivateActionAdmissionStore::open(&self.data_dirs[index])
                .unwrap()
                .get(intent_id)
                .map(|record| record.state)
        }

        pub(crate) fn pool_fingerprint(&self) -> Vec<PoolFingerprint> {
            self.data_dirs
                .iter()
                .map(|data_dir| {
                    let store = NodeStore::new(data_dir);
                    let shielded = store.read_shielded().unwrap();
                    let (commitments, nullifiers) = shielded
                        .orchard
                        .as_ref()
                        .map(|pool| (pool.output_commitments.len(), pool.nullifiers.len()))
                        .unwrap_or((0, 0));
                    let ledger = store.read_ledger().unwrap();
                    let holder: DevKeyFile =
                        serde_json::from_slice(&std::fs::read(&self.holder_key_file).unwrap()).unwrap();
                    let balances = self
                        .asset_ids
                        .iter()
                        .map(|asset_id| {
                            let line = ledger
                                .trustline_for_account_asset(&holder.address, asset_id)
                                .map(|line| line.balance)
                                .unwrap_or(0);
                            (asset_id.clone(), line)
                        })
                        .collect();
                    let tip = status(NodeOptions {
                        data_dir: data_dir.clone(),
                    })
                    .unwrap()
                    .block_tip_hash;
                    (commitments, nullifiers, balances, tip)
                })
                .collect()
        }
    }

    pub(crate) fn refused_code(result: &io::Result<PrivateActionRecord>) -> String {
        let error = result.as_ref().expect_err("refused").to_string();
        error.split(": ").next().unwrap().to_string()
    }

    /// The wallet's check of a receipt from one validator against another
    /// validator's finalized view.
    pub(crate) fn receipt_matches_independent_view(
        receipt: &postfiat_types::PrivateReceiptResponse,
        data_dir: &Path,
    ) -> bool {
        let blocks = NodeStore::new(data_dir).read_blocks().unwrap();
        let Some(height) = receipt.inclusion_height else {
            return false;
        };
        let Some(block) = blocks.blocks.iter().find(|block| block.header.height == height) else {
            return false;
        };
        let finality = match receipt.finality.as_ref() {
            Some(finality) => finality,
            None => return false,
        };
        receipt.accepted == Some(true)
            && block.header.batch_id == receipt.batch_id
            && Some(&block.header.block_hash) == receipt.block_hash.as_ref()
            && Some(&block.header.state_root) == receipt.state_root.as_ref()
            && receipt
                .transaction_id
                .as_ref()
                .is_some_and(|id| block.receipt_ids.contains(id))
            && finality.kind == PRIVATE_ACTION_FINALITY_KIND
            && finality.block_hash == block.header.block_hash
            && finality.certificate_json
                == serde_json::to_string(&block.header.certificate).unwrap()
            && finality.quorum as usize == block.header.certificate.quorum
            && block
                .header
                .certificate
                .votes
                .iter()
                .filter(|vote| vote.accept)
                .count()
                >= block.header.certificate.quorum
    }

    #[test]
    fn six_validators_admit_round_and_receipt_user_proven_ingress() {
        let started = Instant::now();
        let fleet = fleet();
        assert_eq!(fleet.height(), 3);

        // Inactive governance: refused at admission, nothing reserved, no state change.
        let early = fleet.ingress_action(0, 5, 31);
        let before = fleet.pool_fingerprint();
        let refused = fleet.submit_everywhere(&fleet.request(&early));
        for result in &refused {
            assert_eq!(refused_code(result), "shielded_atomic_batch_not_active");
        }
        assert!(fleet.state(0, &fleet.request(&early).intent_id).is_none());
        assert_eq!(fleet.pool_fingerprint(), before);

        fleet.transparent_round();
        fleet.transparent_round();
        assert_eq!(fleet.height(), ATOMIC_ACTIVATION - 1);

        // Wrong domain: refused at admission on every validator.
        let action_a = fleet.ingress_action(0, 7, 41);
        let pre_wrong = fleet.pool_fingerprint();
        let mut wrong_domain = fleet.request(&action_a);
        wrong_domain.domain.chain_id = "postfiat-other-chain".to_string();
        wrong_domain.intent_id =
            private_action_intent_id(&wrong_domain.domain.intent_domain(), &action_a).unwrap();
        for result in fleet.submit_everywhere(&wrong_domain) {
            assert_eq!(refused_code(&result), "private_action_wrong_domain");
        }

        // Wrong issuer binding: the burn names the other issued asset.
        let mut wrong_asset: serde_json::Value = serde_json::from_str(&action_a).unwrap();
        wrong_asset["asset_id"] = serde_json::json!(fleet.asset_ids[1]);
        let wrong_asset = wrong_asset.to_string();
        let wrong_results = fleet.submit_everywhere(&fleet.request(&wrong_asset));
        let wrong_code = refused_code(&wrong_results[0]);
        assert!(wrong_results.iter().all(|result| refused_code(result) == wrong_code));
        assert!(!wrong_code.starts_with("private_action_request"), "{wrong_code}");
        assert_eq!(fleet.pool_fingerprint(), pre_wrong);
        eprintln!("s6 wrong issuer/asset binding refused with `{wrong_code}`");

        // Asset A ingress accepted through the admission service on six validators.
        let request_a = fleet.request(&action_a);
        for result in fleet.submit_everywhere(&request_a) {
            assert_eq!(result.expect("reserve").state, PrivateActionState::Queued);
        }
        let pre_a = fleet.pool_fingerprint();
        assert_eq!(fleet.service_pass(), 1);
        fleet.service_pass();
        for index in 0..VALIDATORS {
            assert_eq!(fleet.state(index, &request_a.intent_id), Some(PrivateActionState::Accepted));
        }
        let post_a = fleet.pool_fingerprint();
        for (pre, post) in pre_a.iter().zip(&post_a) {
            assert_eq!(post.0, pre.0 + 1, "one new commitment per validator");
            assert_eq!(post.1, pre.1, "ingress spends no nullifier");
        }
        let receipts = fleet
            .data_dirs
            .iter()
            .map(|data_dir| private_action_receipt(data_dir, &request_a.intent_id).unwrap())
            .collect::<Vec<_>>();
        for (index, receipt) in receipts.iter().enumerate() {
            assert_eq!(receipt.state, "accepted");
            let contract = receipt.contract_receipt.as_ref().unwrap();
            // Checked against a different validator's finalized view.
            assert!(receipt_matches_independent_view(
                contract,
                &fleet.data_dirs[(index + 1) % VALIDATORS]
            ));
        }
        // Tampered receipts are refused by the same check.
        let honest = receipts[0].contract_receipt.clone().unwrap();
        let mut tampered = Vec::new();
        let mut copy = honest.clone();
        copy.block_hash = Some("00".repeat(48));
        tampered.push(copy);
        let mut copy = honest.clone();
        copy.state_root = Some("11".repeat(48));
        tampered.push(copy);
        let mut copy = honest.clone();
        copy.batch_id = "22".repeat(48);
        tampered.push(copy);
        let mut copy = honest.clone();
        if let Some(finality) = copy.finality.as_mut() {
            finality.certificate_json = finality.certificate_json.replacen("\"accept\":true", "\"accept\":false", 1);
        }
        tampered.push(copy);
        let mut copy = honest.clone();
        copy.inclusion_height = copy.inclusion_height.map(|height| height - 1);
        tampered.push(copy);
        for receipt in &tampered {
            assert!(!receipt_matches_independent_view(receipt, &fleet.data_dirs[3]));
        }

        // Replay: the same action re-encoded under a new intent, and the exact
        // same request again; neither starts a round.
        let height_after_a = fleet.height();
        let pre_replay = fleet.pool_fingerprint();
        let reencoded = serde_json::to_string_pretty(
            &serde_json::from_str::<serde_json::Value>(&action_a).unwrap(),
        )
        .unwrap();
        for result in fleet.submit_everywhere(&fleet.request(&reencoded)) {
            assert_eq!(refused_code(&result), "private_action_duplicate");
        }
        for result in fleet.submit_everywhere(&request_a) {
            assert_eq!(result.unwrap().state, PrivateActionState::Accepted);
        }
        assert_eq!(fleet.service_pass(), 0);
        assert_eq!(fleet.height(), height_after_a);
        assert_eq!(fleet.pool_fingerprint(), pre_replay);

        // Asset B ingress accepted: both ingress assets.
        let action_b = fleet.ingress_action(1, 9, 51);
        let request_b = fleet.request(&action_b);
        fleet.submit_everywhere(&request_b).into_iter().for_each(|result| {
            result.expect("reserve B");
        });
        assert_eq!(fleet.service_pass(), 1);
        fleet.service_pass();
        for index in 0..VALIDATORS {
            assert_eq!(fleet.state(index, &request_b.intent_id), Some(PrivateActionState::Accepted));
        }

        // Final rejection after reservation: two actions at one account
        // sequence both verify alone; the second is refused before any round.
        let first = fleet.request(&fleet.ingress_action(0, 3, 61));
        let second = fleet.request(&fleet.ingress_action(1, 4, 62));
        fleet.submit_everywhere(&first).into_iter().for_each(|r| { r.expect("first"); });
        fleet.submit_everywhere(&second).into_iter().for_each(|r| { r.expect("second"); });
        assert_eq!(fleet.service_pass(), 1);
        let pre_reject = fleet.pool_fingerprint();
        assert_eq!(fleet.service_pass(), 0);
        let (accepted_one, rejected_one) = if fleet.state(0, &first.intent_id) == Some(PrivateActionState::Accepted) {
            (&first, &second)
        } else {
            (&second, &first)
        };
        for index in 0..VALIDATORS {
            assert_eq!(fleet.state(index, &accepted_one.intent_id), Some(PrivateActionState::Accepted));
            assert_eq!(fleet.state(index, &rejected_one.intent_id), Some(PrivateActionState::Rejected));
            let receipt = private_action_receipt(&fleet.data_dirs[index], &rejected_one.intent_id).unwrap();
            let outcome = receipt.record.unwrap().outcome.unwrap();
            assert!(!outcome.accepted);
            assert_eq!(outcome.fee_charged, 0);
            assert_eq!(outcome.state_effects, "none");
            let contract = receipt.contract_receipt.unwrap();
            assert_eq!(contract.state, postfiat_types::PrivateAdmissionState::Rejected);
            assert_eq!(contract.code.as_deref(), Some(outcome.code.as_str()));
            assert_eq!(contract.fee, Some(0));
        }
        assert_eq!(fleet.pool_fingerprint(), pre_reject);

        // Crash between the durable `dispatched` write and the certified send.
        let crash_a = fleet.request(&fleet.ingress_action(0, 2, 71));
        fleet.submit_everywhere(&crash_a).into_iter().for_each(|r| { r.expect("crash a"); });
        let next = fleet.height() + 1;
        let crash_proposer = fleet.proposer_index(next);
        let pre_crash = fleet.pool_fingerprint();
        let killed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            private_action_service_step(
                &fleet.data_dirs[crash_proposer],
                |_| Ok(true),
                |_, _| panic!("round service killed before the certified send"),
            )
        }));
        assert!(killed.is_err());
        assert_eq!(fleet.state(crash_proposer, &crash_a.intent_id), Some(PrivateActionState::Dispatched));
        // Restart: reconciled from the chain, never re-run.
        private_action_service_step(&fleet.data_dirs[crash_proposer], |_| Ok(true), |_, _| {
            panic!("an interrupted dispatch is never re-run")
        })
        .unwrap();
        assert_eq!(fleet.state(crash_proposer, &crash_a.intent_id), Some(PrivateActionState::Unknown));
        assert_eq!(fleet.pool_fingerprint(), pre_crash);
        // A new session recovers the same persisted action: no replacement.
        let recovered = private_action_submit(&fleet.data_dirs[crash_proposer], &crash_a).unwrap();
        assert!(recovered.existing);
        assert_eq!(recovered.record.action_json, crash_a.action_json);
        // Once the chain moves, the next proposer holds the same reserved batch
        // and lands it; the crashed validator settles from the chain.
        fleet.transparent_round();
        assert_eq!(fleet.service_pass(), 1);
        fleet.service_pass();
        for index in 0..VALIDATORS {
            assert_eq!(fleet.state(index, &crash_a.intent_id), Some(PrivateActionState::Accepted));
        }

        // Crash between the certified send and the receipt write.
        let crash_b = fleet.request(&fleet.ingress_action(1, 6, 81));
        fleet.submit_everywhere(&crash_b).into_iter().for_each(|r| { r.expect("crash b"); });
        let next = fleet.height() + 1;
        let crash_proposer = fleet.proposer_index(next);
        let pre_send = fleet.pool_fingerprint();
        let killed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            private_action_service_step(
                &fleet.data_dirs[crash_proposer],
                |_| Ok(true),
                |record, _| {
                    fleet.dispatch(record);
                    panic!("round service killed before the receipt write")
                },
            )
        }));
        assert!(killed.is_err());
        assert_eq!(fleet.state(crash_proposer, &crash_b.intent_id), Some(PrivateActionState::Dispatched));
        assert_eq!(fleet.service_pass(), 0, "restart reconciles from the chain");
        let post_send = fleet.pool_fingerprint();
        for index in 0..VALIDATORS {
            assert_eq!(fleet.state(index, &crash_b.intent_id), Some(PrivateActionState::Accepted));
            assert_eq!(post_send[index].0, pre_send[index].0 + 1, "landed exactly once");
        }

        // Fresh-key rescan across page boundaries of the S4 reads.
        let snapshot = private_pool_snapshot(PrivatePoolSnapshotOptions {
            data_dir: fleet.data_dirs[4].clone(),
            pool_id: PRIVATE_ACTION_POOL_ID.to_string(),
            address: None,
            base_asset_id: None,
            quote_asset_id: None,
            inclusion_height: None,
        })
        .expect("snapshot");
        assert!(snapshot.completeness.complete, "{:?}", snapshot.completeness);
        let mut outputs = Vec::new();
        let mut cursor = None;
        let mut pages = 0;
        loop {
            let page = private_pool_page(PrivatePoolPageOptions {
                data_dir: fleet.data_dirs[4].clone(),
                snapshot_token: snapshot.snapshot_token.clone(),
                section: "outputs".to_string(),
                cursor: cursor.clone(),
                limit: Some(2),
            })
            .expect("outputs page");
            assert_eq!(page.status, "ok", "{:?}", page.refusal);
            pages += 1;
            if let Some(PrivatePoolPageItems::Outputs(rows)) = page.items {
                outputs.extend(rows);
            }
            cursor = page.next_cursor;
            if cursor.is_none() {
                break;
            }
        }
        assert_eq!(outputs.len() as u64, snapshot.counts.outputs);
        assert!(pages > 1, "the page limit is below the total");
        let mut assets = Vec::new();
        let mut cursor = None;
        loop {
            let page = private_pool_page(PrivatePoolPageOptions {
                data_dir: fleet.data_dirs[4].clone(),
                snapshot_token: snapshot.snapshot_token.clone(),
                section: "assets".to_string(),
                cursor: cursor.clone(),
                limit: Some(1),
            })
            .expect("assets page");
            if let Some(PrivatePoolPageItems::Assets(rows)) = page.items {
                assets.extend(rows);
            }
            cursor = page.next_cursor;
            if cursor.is_none() {
                break;
            }
        }
        // A fresh wallet holds only its note seeds and recovers every note.
        let genesis = NodeStore::new(&fleet.data_dirs[4]).read_genesis().unwrap();
        let genesis_hash_bytes: [u8; 32] = postfiat_privacy_orchard::asset_orchard_domain_genesis_hash(
            &postfiat_execution::genesis_hash(&genesis),
        )
        .expect("domain genesis hash");
        let mut recovered_totals = BTreeMap::<String, u64>::new();
        let mut recovered = 0;
        for row in &outputs {
            let ciphertext = postfiat_crypto_provider::hex_to_bytes(row.ciphertext.as_ref().expect("retained ciphertext")).unwrap();
            for seed in [41u8, 51, 61, 62, 71, 81] {
                if let Ok(Some(note)) = postfiat_privacy_orchard::decrypt_asset_orchard_wallet_note(
                    &genesis.chain_id,
                    genesis_hash_bytes,
                    genesis.protocol_version,
                    &postfiat_crypto_provider::bytes_to_hex(&[seed; 32]),
                    &row.commitment,
                    &ciphertext,
                ) {
                    *recovered_totals.entry(note.asset_id.clone()).or_default() += note.value;
                    recovered += 1;
                }
            }
        }
        assert_eq!(recovered, outputs.len(), "every retained output belongs to the wallet");
        for row in &assets {
            assert_eq!(recovered_totals.get(&row.asset_id).copied().unwrap_or(0), row.live_total);
        }
        assert_eq!(assets.len(), 2, "both ingress assets are in the pool");

        eprintln!("s6 six-validator module finished in {:.1}s", started.elapsed().as_secs_f64());
        let _ = std::fs::remove_dir_all(&fleet.root);
    }
}
