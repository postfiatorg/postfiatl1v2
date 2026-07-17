use super::*;

pub fn account(options: NodeOptions, address: &str) -> io::Result<Account> {
    let store = NodeStore::new(options.data_dir);
    let ledger = store.read_ledger()?;
    Ok(ledger
        .account(address)
        .cloned()
        .unwrap_or_else(|| Account::new(address, 0, None)))
}

pub fn receipts(options: ReceiptQueryOptions) -> io::Result<Vec<Receipt>> {
    let store = NodeStore::new(options.data_dir);
    let mut receipts = store.read_receipts()?;
    if let Some(tx_id) = options.tx_id {
        receipts.retain(|receipt| receipt.tx_id == tx_id);
    }
    let limit = bounded_read_query_limit(options.limit, "receipts")?;
    if receipts.len() > limit {
        receipts = receipts[receipts.len() - limit..].to_vec();
    }
    Ok(receipts)
}

pub fn verify_state(options: NodeOptions) -> io::Result<StateVerificationReport> {
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let governance = verify_governance(options.clone())?;
    let block_log = verify_blocks(options.clone())?;
    let bridge = verify_bridge(options.clone())?;
    let shielded = verify_shielded(options.clone())?;
    let mempool = verify_mempool(options)?;

    Ok(StateVerificationReport {
        schema: "postfiat-state-verification-v1".to_string(),
        verified: block_log.verified
            && governance.verified
            && bridge.verified
            && shielded.verified
            && mempool.verified,
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        block_log,
        governance,
        bridge,
        shielded,
        mempool,
    })
}

pub fn export_market_ops_replay_bundle(
    options: MarketOpsReplayBundleExportOptions,
) -> io::Result<MarketOpsReplayBundle> {
    let store = NodeStore::new(options.data_dir);
    let ledger = store.read_ledger()?;
    let record = ledger
        .market_ops_envelope(&options.asset_id, options.epoch)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "missing finalized market ops envelope for asset `{}` epoch `{}`",
                    options.asset_id, options.epoch
                ),
            )
        })?;
    let policy_inputs = record.policy_inputs.clone().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "finalized market ops envelope does not carry replay policy_inputs",
        )
    })?;
    let reserve_packet = ledger
        .nav_reserve_packets
        .iter()
        .find(|packet| {
            packet.asset_id == options.asset_id
                && packet.epoch == options.epoch
                && market_ops_reserve_packet_hash(&packet.reserve_packet_hash)
                    .is_ok_and(|hash| hash == record.envelope.reserve_packet_hash)
        })
        .cloned()
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "missing source reserve packet for finalized market ops envelope",
            )
        })?;
    let bundle = MarketOpsReplayBundle {
        schema: MARKET_OPS_REPLAY_BUNDLE_SCHEMA.to_string(),
        asset_id: record.asset_id.clone(),
        epoch: record.epoch,
        reserve_packet,
        envelope: record.envelope.clone(),
        policy_inputs,
        reserve_packet_hash: bytes_to_hex(&record.envelope.reserve_packet_hash),
        supply_packet_hash: bytes_to_hex(&record.envelope.supply_packet_hash),
        evidence_root: bytes_to_hex(&record.envelope.evidence_root),
        program_id: bytes_to_hex(&record.envelope.program_id),
        policy_hash: bytes_to_hex(&record.envelope.policy_hash),
        parameter_hash: bytes_to_hex(&record.envelope.parameter_hash),
        previous_market_state_hash: bytes_to_hex(&record.envelope.previous_market_state_hash),
        expected_envelope_hash: record.envelope_hash.clone(),
    };
    replay_market_ops_bundle_data(&bundle)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    std::fs::create_dir_all(&options.bundle_dir)?;
    let bundle_file = market_ops_replay_bundle_file(&options.bundle_dir);
    if bundle_file.exists() && !options.overwrite {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "market ops replay bundle `{}` already exists; pass --overwrite to replace it",
                bundle_file.display()
            ),
        ));
    }
    let json = serde_json::to_string_pretty(&bundle).map_err(invalid_data)?;
    atomic_write(&bundle_file, format!("{json}\n"))?;
    Ok(bundle)
}

pub fn replay_market_ops_bundle(
    options: MarketOpsReplayBundleVerifyOptions,
) -> io::Result<MarketOpsReplayReport> {
    let bundle_file = market_ops_replay_bundle_file(&options.bundle_dir);
    let bundle: MarketOpsReplayBundle = read_json_file(&bundle_file, "market ops replay bundle")?;
    let computed_envelope_hash = replay_market_ops_bundle_data(&bundle)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    Ok(MarketOpsReplayReport {
        schema: MARKET_OPS_REPLAY_REPORT_SCHEMA.to_string(),
        bundle_file: bundle_file.display().to_string(),
        asset_id: bundle.asset_id,
        epoch: bundle.epoch,
        expected_envelope_hash: bundle.expected_envelope_hash,
        computed_envelope_hash,
        verified: true,
    })
}

pub fn market_ops_operation_bundle(
    options: MarketOpsOperationBundleOptions,
) -> io::Result<MarketOpsOperationBundle> {
    if options.encoding_version == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "--encoding-version must be nonzero",
        ));
    }
    if options.evm_chain_id == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "--evm-chain-id must be nonzero",
        ));
    }
    if options.data_window_start >= options.data_window_end {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "--data-window-start must be before --data-window-end",
        ));
    }
    if options.valid_after > options.expires_at {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "--valid-after must be <= --expires-at",
        ));
    }
    if options.discount_trigger_bps > 10_000 || options.premium_trigger_bps > 10_000 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "market ops trigger bps values must be <= 10000",
        ));
    }

    let policy: MarketOpsPolicyRegistration =
        read_json_file(&options.policy_file, "market ops policy registration")?;
    policy
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    let policy_inputs: MarketOpsPolicyInputs =
        read_json_file(&options.policy_inputs_file, "market ops policy inputs")?;
    policy_inputs
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;

    let store = NodeStore::new(options.data_dir.clone());
    let ledger = store.read_ledger()?;
    let nav_asset = ledger.nav_asset(&options.asset_id).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("missing NAV asset `{}`", options.asset_id),
        )
    })?;
    let issuer = options
        .issuer
        .clone()
        .unwrap_or_else(|| nav_asset.issuer.clone());
    if issuer != nav_asset.issuer {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "market ops issuer must match the NAV asset issuer",
        ));
    }
    let epoch = options.epoch.unwrap_or(nav_asset.finalized_epoch);
    if epoch == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "NAV asset has no finalized epoch; cannot build market ops operations",
        ));
    }
    let reserve_packet = ledger
        .nav_reserve_packets
        .iter()
        .find(|packet| {
            packet.asset_id == options.asset_id
                && packet.epoch == epoch
                && packet.state == NAV_RESERVE_STATE_FINALIZED
                && (nav_asset.finalized_epoch != epoch
                    || packet.reserve_packet_hash == nav_asset.finalized_reserve_packet_hash)
        })
        .cloned()
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "missing finalized NAV reserve packet for asset `{}` epoch `{epoch}`",
                    options.asset_id
                ),
            )
        })?;

    let previous_market_state_hash = options
        .previous_market_state_hash
        .as_deref()
        .map(|value| market_ops_parse_hex_array::<32>("--previous-market-state-hash", value))
        .transpose()?
        .unwrap_or([0u8; 32]);
    let nonce = options
        .nonce
        .as_deref()
        .map(|value| market_ops_parse_hex_array::<32>("--nonce", value))
        .transpose()?
        .unwrap_or_else(|| {
            derive_market_ops_operation_nonce(
                &options.asset_id,
                epoch,
                &reserve_packet.reserve_packet_hash,
                &policy,
                options.data_window_start,
                options.data_window_end,
                options.expires_at,
            )
        });

    let template = MarketOpsEnvelope {
        encoding_version: options.encoding_version,
        chain_id: options.evm_chain_id,
        adapter_address: market_ops_parse_hex_array::<20>(
            "--adapter-address",
            &options.adapter_address,
        )?,
        vault_address: market_ops_parse_hex_array::<20>("--vault-address", &options.vault_address)?,
        mint_controller_address: market_ops_parse_hex_array::<20>(
            "--mint-controller-address",
            &options.mint_controller_address,
        )?,
        asset_id: market_ops_asset_id(&options.asset_id)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?,
        epoch,
        program_id: policy.program_id,
        policy_hash: policy.policy_hash,
        parameter_hash: policy.parameter_hash,
        reserve_packet_hash: market_ops_reserve_packet_hash(&reserve_packet.reserve_packet_hash)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?,
        supply_packet_hash: market_ops_supply_packet_hash(
            &options.asset_id,
            epoch,
            u128::from(reserve_packet.circulating_supply),
        )
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?,
        evidence_root: market_ops_evidence_root(
            &policy_inputs.discount_observations,
            &policy_inputs.premium_observations,
        )
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?,
        previous_market_state_hash,
        venue_id: policy.venue_id,
        pool_config_hash: policy.pool_config_hash,
        hook_code_hash: policy.hook_code_hash,
        nav_floor_usd_e8: 1,
        valid_global_supply_atoms: 1,
        verified_net_assets_usd_e8: 1,
        funded_alignment_reserve_usd_e8: options.funded_alignment_reserve_usd_e8,
        required_alignment_reserve_usd_e8: 1,
        max_reserve_deploy_usd_e8: 1,
        max_mint_atoms: 1,
        discount_trigger_bps: options.discount_trigger_bps,
        premium_trigger_bps: options.premium_trigger_bps,
        data_window_start: options.data_window_start,
        data_window_end: options.data_window_end,
        valid_after: options.valid_after,
        expires_at: options.expires_at,
        cooldown_seconds: options.cooldown_seconds,
        nonce,
    };
    let envelope = recompute_market_ops_replay_envelope(
        &options.asset_id,
        &reserve_packet,
        &template,
        &policy_inputs,
    )
    .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    envelope
        .validate_basic()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    if !policy.accepts(&envelope) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "market ops policy does not accept the generated envelope",
        ));
    }
    let expected_envelope_hash = bytes_to_hex(&envelope.envelope_hash());
    let replay_bundle = MarketOpsReplayBundle {
        schema: MARKET_OPS_REPLAY_BUNDLE_SCHEMA.to_string(),
        asset_id: options.asset_id.clone(),
        epoch,
        reserve_packet: reserve_packet.clone(),
        envelope: envelope.clone(),
        policy_inputs: policy_inputs.clone(),
        reserve_packet_hash: bytes_to_hex(&envelope.reserve_packet_hash),
        supply_packet_hash: bytes_to_hex(&envelope.supply_packet_hash),
        evidence_root: bytes_to_hex(&envelope.evidence_root),
        program_id: bytes_to_hex(&envelope.program_id),
        policy_hash: bytes_to_hex(&envelope.policy_hash),
        parameter_hash: bytes_to_hex(&envelope.parameter_hash),
        previous_market_state_hash: bytes_to_hex(&envelope.previous_market_state_hash),
        expected_envelope_hash: expected_envelope_hash.clone(),
    };
    replay_market_ops_bundle_data(&replay_bundle)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;

    let policy_register_operation =
        AssetTransactionOperation::MarketOpsPolicyRegister(MarketOpsPolicyRegisterOperation {
            issuer: issuer.clone(),
            asset_id: options.asset_id.clone(),
            policy: policy.clone(),
        });
    policy_register_operation
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    let market_ops_finalize_operation =
        AssetTransactionOperation::MarketOpsFinalize(MarketOpsFinalizeOperation {
            issuer: issuer.clone(),
            asset_id: options.asset_id.clone(),
            envelope_hash: expected_envelope_hash.clone(),
            envelope: envelope.clone(),
            policy_inputs: policy_inputs.clone(),
        });
    market_ops_finalize_operation
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;

    let bundle_dir = options.bundle_dir;
    let policy_register_operation_file = bundle_dir.join("policy-register.operation.json");
    let market_ops_finalize_operation_file = bundle_dir.join("market-ops-finalize.operation.json");
    let operation_bundle_file = bundle_dir.join("operation-bundle.json");
    let replay_dir = bundle_dir.join("replay");
    let replay_bundle_file = market_ops_replay_bundle_file(&replay_dir);
    let commands_file = bundle_dir.join("commands.sh");
    let files = [
        policy_register_operation_file.clone(),
        market_ops_finalize_operation_file.clone(),
        operation_bundle_file.clone(),
        replay_bundle_file.clone(),
        commands_file.clone(),
    ];
    if !options.overwrite {
        if let Some(existing) = files.iter().find(|path| path.exists()) {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!(
                    "market ops operation bundle file `{}` already exists; pass --overwrite to replace it",
                    existing.display()
                ),
            ));
        }
    }
    std::fs::create_dir_all(&bundle_dir)?;
    std::fs::create_dir_all(&replay_dir)?;
    vault_bridge_write_json_file(&policy_register_operation_file, &policy_register_operation)?;
    vault_bridge_write_json_file(
        &market_ops_finalize_operation_file,
        &market_ops_finalize_operation,
    )?;
    vault_bridge_write_json_file(&replay_bundle_file, &replay_bundle)?;
    let relay_commands = market_ops_operation_relay_commands();
    let commands_script = market_ops_operation_relay_commands_script(&relay_commands);
    atomic_write(&commands_file, commands_script)?;

    let bundle = MarketOpsOperationBundle {
        schema: MARKET_OPS_OPERATION_BUNDLE_SCHEMA.to_string(),
        asset_id: options.asset_id,
        issuer,
        epoch,
        reserve_packet_hash: reserve_packet.reserve_packet_hash,
        supply_packet_hash: bytes_to_hex(&envelope.supply_packet_hash),
        evidence_root: bytes_to_hex(&envelope.evidence_root),
        expected_envelope_hash,
        policy,
        policy_inputs,
        envelope,
        policy_register_operation,
        market_ops_finalize_operation,
        policy_register_operation_file: policy_register_operation_file.display().to_string(),
        market_ops_finalize_operation_file: market_ops_finalize_operation_file
            .display()
            .to_string(),
        replay_bundle_file: replay_bundle_file.display().to_string(),
        commands_file: commands_file.display().to_string(),
        relay_commands,
    };
    vault_bridge_write_json_file(&operation_bundle_file, &bundle)?;
    Ok(bundle)
}

pub fn market_ops_status(options: MarketOpsStatusOptions) -> io::Result<MarketOpsPublicStatus> {
    let store = NodeStore::new(options.data_dir.clone());
    let chain_status = status(NodeOptions {
        data_dir: options.data_dir,
    })?;
    let ledger = store.read_ledger()?;
    let record = match options.epoch {
        Some(epoch) => ledger
            .market_ops_envelope(&options.asset_id, epoch)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    format!(
                        "missing finalized market ops envelope for asset `{}` epoch `{epoch}`",
                        options.asset_id
                    ),
                )
            })?,
        None => ledger
            .market_ops_envelopes
            .iter()
            .filter(|record| record.asset_id == options.asset_id)
            .max_by_key(|record| record.epoch)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    format!(
                        "missing finalized market ops envelope for asset `{}`",
                        options.asset_id
                    ),
                )
            })?,
    };

    build_market_ops_public_status(&ledger, record, chain_status.block_height, unix_now())
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

pub fn vault_bridge_status(
    options: VaultBridgeStatusOptions,
) -> io::Result<VaultBridgeStatusReport> {
    let store = NodeStore::new(options.data_dir);
    let ledger = store.read_ledger()?;
    let shielded = store.read_shielded()?;
    let nav_asset = ledger.nav_asset(&options.asset_id).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("missing NAV asset `{}`", options.asset_id),
        )
    })?;
    let issued_supply_atoms =
        issued_asset_supply_for_status(&ledger, &shielded, &options.asset_id)?;
    let source_root =
        vault_bridge_source_root_for_asset(&ledger.vault_bridge_bucket_states, &options.asset_id)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

    let mut buckets = ledger
        .vault_bridge_bucket_states
        .iter()
        .filter(|bucket| bucket.asset_id == options.asset_id)
        .collect::<Vec<_>>();
    buckets.sort_by(|left, right| left.bucket_id.cmp(&right.bucket_id));

    let mut receipts = ledger
        .vault_bridge_receipts
        .iter()
        .filter(|receipt| receipt.asset_id == options.asset_id)
        .collect::<Vec<_>>();
    receipts.sort_by(|left, right| left.receipt_id.cmp(&right.receipt_id));

    let mut bridge_deposits = ledger
        .vault_bridge_deposits
        .iter()
        .filter(|record| record.asset_id == options.asset_id)
        .collect::<Vec<_>>();
    bridge_deposits.sort_by(|left, right| left.evidence_root.cmp(&right.evidence_root));

    let mut allocations = ledger
        .vault_bridge_allocations
        .iter()
        .filter(|allocation| allocation.asset_id == options.asset_id)
        .collect::<Vec<_>>();
    allocations.sort_by(|left, right| left.allocation_id.cmp(&right.allocation_id));

    let mut redemptions = ledger
        .vault_bridge_redemptions
        .iter()
        .filter(|redemption| redemption.asset_id == options.asset_id)
        .collect::<Vec<_>>();
    redemptions.sort_by(|left, right| left.redemption_id.cmp(&right.redemption_id));

    let counted_value_atoms =
        vault_bridge_counted_value_for_asset(&ledger.vault_bridge_bucket_states, &options.asset_id)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let unallocated_counted_capacity_atoms = receipts
        .iter()
        .filter(|receipt| {
            buckets.iter().any(|bucket| {
                bucket.bucket_id == receipt.bucket_id
                    && bucket.status == VAULT_BRIDGE_BUCKET_STATUS_ACTIVE
            })
        })
        .try_fold(0_u64, |total, receipt| {
            let available = receipt
                .available_counted_value()
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
            total.checked_add(available).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "vault bridge asset unallocated receipt capacity overflowed",
                )
            })
        })?;

    let bucket_rows = buckets
        .iter()
        .map(|bucket| {
            let allocated = bucket
                .allocated_atoms()
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
            let unallocated = if bucket.status == VAULT_BRIDGE_BUCKET_STATUS_ACTIVE {
                bucket
                    .counted_value_atoms
                    .checked_sub(allocated)
                    .ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidData,
                            "vault bridge asset active bucket allocated atoms exceed counted value",
                        )
                    })?
            } else {
                0
            };
            Ok(VaultBridgeBucketStatusRow {
                bucket_id: bucket.bucket_id.clone(),
                source_domain: bucket.source_domain.clone(),
                policy_hash: bucket.policy_hash.clone(),
                gross_receipt_atoms: bucket.gross_receipt_atoms,
                counted_value_atoms: bucket.counted_value_atoms,
                outstanding_vault_bridge_atoms: bucket.outstanding_vault_bridge_atoms,
                nav_subscription_allocations_atoms: bucket.nav_subscription_allocations_atoms,
                redemption_queue_atoms: bucket.redemption_queue_atoms,
                other_allocations_atoms: bucket.other_allocations_atoms,
                unallocated_counted_capacity_atoms: unallocated,
                impairment_factor_bps: bucket.impairment_factor_bps,
                status: bucket.status.clone(),
                last_packet_epoch: bucket.last_packet_epoch,
                last_updated_height: bucket.last_updated_height,
            })
        })
        .collect::<io::Result<Vec<_>>>()?;

    let receipt_rows = receipts
        .iter()
        .map(|receipt| {
            let bridge_deposit_evidence_root = receipt
                .bridge_deposit_evidence
                .as_ref()
                .map(vault_bridge_deposit_evidence_root)
                .transpose()
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
            Ok(VaultBridgeReceiptStatusRow {
                receipt_id: receipt.receipt_id.clone(),
                bucket_id: receipt.bucket_id.clone(),
                source_domain: receipt.source_domain.clone(),
                source_asset: receipt.source_asset.clone(),
                claim_type: receipt.claim_type.clone(),
                amount_atoms: receipt.amount_atoms,
                haircut_bps: receipt.haircut_bps,
                counted_value_atoms: receipt.counted_value_atoms,
                allocated_value_atoms: receipt.allocated_value_atoms,
                unallocated_value_atoms: if buckets.iter().any(|bucket| {
                    bucket.bucket_id == receipt.bucket_id
                        && bucket.status == VAULT_BRIDGE_BUCKET_STATUS_ACTIVE
                }) {
                    receipt
                        .available_counted_value()
                        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?
                } else {
                    0
                },
                status: receipt.status.clone(),
                created_at_height: receipt.created_at_height,
                counted_at_height: receipt.counted_at_height,
                expires_at_height: receipt.expires_at_height,
                source_tx_or_attestation: receipt.source_tx_or_attestation.clone(),
                finality_ref: receipt.finality_ref.clone(),
                vault_id: receipt.vault_id.clone(),
                bridge_deposit_evidence_root,
            })
        })
        .collect::<io::Result<Vec<_>>>()?;

    let bridge_deposit_rows = bridge_deposits
        .iter()
        .map(|record| {
            let mut attestations = record.attestations.iter().collect::<Vec<_>>();
            attestations.sort_by(|left, right| left.attestor.cmp(&right.attestor));
            let pass_attestation_count = attestations
                .iter()
                .filter(|attestation| attestation.pass)
                .count() as u64;
            let fail_attestation_count = attestations
                .iter()
                .filter(|attestation| !attestation.pass)
                .count() as u64;
            VaultBridgeDepositStatusRow {
                evidence_root: record.evidence_root.clone(),
                policy_hash: record.policy_hash.clone(),
                source_proof_kind: record.source_proof_kind.clone(),
                source_proof_hash: record.source_proof_hash.clone(),
                source_public_values_hash: record.source_public_values_hash.clone(),
                source_chain_id: record.evidence.source_chain_id,
                vault_address: record.evidence.vault_address.clone(),
                token_address: record.evidence.token_address.clone(),
                depositor: record.evidence.depositor.clone(),
                pftl_recipient: record.evidence.pftl_recipient.clone(),
                amount_atoms: record.evidence.amount_atoms,
                deposit_id: record.evidence.deposit_id.clone(),
                block_hash: record.evidence.block_hash.clone(),
                tx_hash: record.evidence.tx_hash.clone(),
                log_index: record.evidence.log_index,
                proposer: record.proposer.clone(),
                status: record.status.clone(),
                submitted_at_height: record.submitted_at_height,
                finalized_at_height: record.finalized_at_height,
                expires_at_height: record.expires_at_height,
                challenger: record.challenger.clone(),
                challenge_hash: record.challenge_hash.clone(),
                challenge_bond: record.challenge_bond,
                pass_attestation_count,
                fail_attestation_count,
                attestations: attestations
                    .into_iter()
                    .map(|attestation| VaultBridgeDepositAttestationStatusRow {
                        attestor: attestation.attestor.clone(),
                        pass: attestation.pass,
                        observation_root: attestation.observation_root.clone(),
                        attested_at_height: attestation.attested_at_height,
                    })
                    .collect(),
            }
        })
        .collect::<Vec<_>>();

    let allocation_rows = allocations
        .iter()
        .map(|allocation| {
            let remaining_atoms = allocation
                .amount_atoms
                .checked_sub(allocation.released_atoms)
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "vault bridge allocation `{}` released atoms exceed amount",
                            allocation.allocation_id
                        ),
                    )
                })?;
            Ok(VaultBridgeAllocationStatusRow {
                allocation_id: allocation.allocation_id.clone(),
                receipt_id: allocation.receipt_id.clone(),
                bucket_id: allocation.bucket_id.clone(),
                amount_atoms: allocation.amount_atoms,
                released_atoms: allocation.released_atoms,
                remaining_atoms,
                purpose: allocation.purpose.clone(),
                consumer_id: allocation.consumer_id.clone(),
                created_at_height: allocation.created_at_height,
                retired_at_height: allocation.retired_at_height,
            })
        })
        .collect::<io::Result<Vec<_>>>()?;

    let redemption_rows = redemptions
        .iter()
        .map(|redemption| VaultBridgeRedemptionStatusRow {
            redemption_id: redemption.redemption_id.clone(),
            owner: redemption.owner.clone(),
            owner_sequence: redemption.owner_sequence,
            issuer: redemption.issuer.clone(),
            bucket_id: redemption.bucket_id.clone(),
            amount_atoms: redemption.amount_atoms,
            epoch: redemption.epoch,
            reserve_packet_hash: redemption.reserve_packet_hash.clone(),
            destination_ref: redemption.destination_ref.clone(),
            settled_atoms: redemption.settled_atoms,
            state: redemption.state.clone(),
            created_at_height: redemption.created_at_height,
            settlement_receipt_hash: redemption.settlement_receipt_hash.clone(),
            burn_tx_id: redemption.withdrawal_packet.burn_tx_id.clone(),
            withdrawal_recipient: redemption.withdrawal_packet.recipient.clone(),
            withdrawal_evidence_root: redemption.withdrawal_packet.evidence_root.clone(),
            withdrawal_packet_hash: redemption.withdrawal_packet_hash.clone(),
            withdrawal_packet_evm_digest: redemption.withdrawal_packet_evm_digest.clone(),
        })
        .collect::<Vec<_>>();

    Ok(VaultBridgeStatusReport {
        schema: VAULT_BRIDGE_STATUS_REPORT_SCHEMA.to_string(),
        asset_id: options.asset_id,
        issuer: nav_asset.issuer.clone(),
        proof_profile: nav_asset.proof_profile.clone(),
        valuation_unit: nav_asset.valuation_unit.clone(),
        finalized_epoch: nav_asset.finalized_epoch,
        nav_per_unit: nav_asset.nav_per_unit,
        circulating_supply: nav_asset.circulating_supply,
        finalized_reserve_packet_hash: nav_asset.finalized_reserve_packet_hash.clone(),
        issued_supply_atoms,
        counted_value_atoms,
        unallocated_counted_capacity_atoms,
        source_root,
        bucket_count: bucket_rows.len() as u64,
        receipt_count: receipt_rows.len() as u64,
        bridge_deposit_count: bridge_deposit_rows.len() as u64,
        allocation_count: allocation_rows.len() as u64,
        redemption_count: redemption_rows.len() as u64,
        buckets: bucket_rows,
        receipts: receipt_rows,
        bridge_deposits: bridge_deposit_rows,
        allocations: allocation_rows,
        redemptions: redemption_rows,
        disclosure: VAULT_BRIDGE_STATUS_DISCLOSURE.to_string(),
    })
}

pub fn vault_bridge_route(options: VaultBridgeRouteOptions) -> io::Result<VaultBridgeRouteReport> {
    let store = NodeStore::new(options.data_dir);
    let genesis = store.read_genesis()?;
    let governance = store.read_governance()?;
    let ledger = store.read_ledger()?;
    let tip = read_chain_tip_or_reconstruct_for_genesis(&store, &genesis)?;
    validate_lower_hex_len(
        "vault bridge route asset id",
        &options.asset_id,
        ISSUED_ASSET_ID_HEX_LEN,
    )
    .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    let record = governance
        .active_vault_bridge_route_profile(&options.asset_id, tip.height)
        .map_err(|error| io::Error::new(io::ErrorKind::PermissionDenied, error))?;
    record
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let route = &record.profile;
    let amendment = governance
        .active_vault_bridge_route_amendment(route, tip.height)
        .map_err(|error| io::Error::new(io::ErrorKind::PermissionDenied, error))?;
    let nav_asset = ledger.nav_asset(&options.asset_id).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "missing NAV asset `{}` for governed vault bridge route",
                options.asset_id
            ),
        )
    })?;
    let profile = ledger
        .nav_proof_profile(&nav_asset.proof_profile)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "missing NAV proof profile `{}` for governed vault bridge route",
                    nav_asset.proof_profile
                ),
            )
        })?;
    validate_vault_bridge_route_profile_against_ledger(&ledger, route, &record.profile_hash)
        .map_err(|error| io::Error::new(io::ErrorKind::PermissionDenied, error))?;
    Ok(VaultBridgeRouteReport {
        schema: "postfiat.vault_bridge.route_report.v1".to_string(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        current_height: tip.height,
        profile: route.clone(),
        profile_hash: record.profile_hash.clone(),
        route_binding: vault_bridge_route_binding(&record.profile_hash, record.profile.route_epoch)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?,
        governance_amendment_id: amendment.amendment_id.clone(),
        governance_activation_height: amendment.activation_height,
        governance_route_epoch: amendment.value,
        nav_profile_id: profile.profile_id.clone(),
        nav_profile_source_class: profile.source_class.clone(),
        nav_profile_verifier_kind: profile.verifier_kind.clone(),
        nav_profile_policy_hash: if profile.vault_bridge_route_policy_hash.is_empty() {
            profile.valuation_policy_hash.clone()
        } else {
            profile.vault_bridge_route_policy_hash.clone()
        },
        active: true,
    })
}

pub fn navcoin_bridge_routes(
    options: NavcoinBridgeRoutesOptions,
) -> io::Result<PftlUniswapRoutesStatusReport> {
    let store = NodeStore::new(&options.data_dir);
    match store.read_ledger() {
        Ok(ledger) => pftl_uniswap_consensus_routes_status(&ledger),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let ledgers = read_pftl_uniswap_bridge_ledgers(&options.data_dir)?;
            pftl_uniswap_bridge_routes_status(&ledgers)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
        }
        Err(error) => Err(error),
    }
}

pub fn navcoin_bridge_packet(
    options: NavcoinBridgePacketOptions,
) -> io::Result<PftlUniswapPacketStatusReport> {
    validate_navcoin_bridge_route_id(&options.route_id)?;
    let store = NodeStore::new(&options.data_dir);
    match store.read_ledger() {
        Ok(ledger) => {
            let route = ledger
                .pftl_uniswap_route(&options.route_id)
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::NotFound,
                        format!(
                            "missing consensus PFTL-Uniswap route `{}`",
                            options.route_id
                        ),
                    )
                })?;
            pftl_uniswap_consensus_packet_status(route, &options.packet_hash)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let ledgers = read_pftl_uniswap_bridge_ledgers(&options.data_dir)?;
            let ledger = pftl_uniswap_bridge_ledger_for_route(&ledgers, &options.route_id)?;
            pftl_uniswap_bridge_packet_status(ledger, &options.packet_hash)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
        }
        Err(error) => Err(error),
    }
}

pub fn navcoin_bridge_claims(
    options: NavcoinBridgeClaimsOptions,
) -> io::Result<PftlUniswapClaimsStatusReport> {
    validate_navcoin_bridge_route_id(&options.route_id)?;
    let ledgers = read_pftl_uniswap_bridge_ledgers(&options.data_dir)?;
    let ledger = pftl_uniswap_bridge_ledger_for_route(&ledgers, &options.route_id)?;
    pftl_uniswap_bridge_claims_status(
        ledger,
        options.limit.unwrap_or(PFTL_UNISWAP_STATUS_MAX_ROWS),
        options.include_terminal,
    )
    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

pub fn navcoin_bridge_supply_status(
    options: NavcoinBridgeSupplyStatusOptions,
) -> io::Result<PftlUniswapSupplyStatusReport> {
    validate_navcoin_bridge_route_id(&options.route_id)?;
    let store = NodeStore::new(&options.data_dir);
    match store.read_ledger() {
        Ok(ledger) => {
            let route = ledger
                .pftl_uniswap_route(&options.route_id)
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::NotFound,
                        format!(
                            "missing consensus PFTL-Uniswap route `{}`",
                            options.route_id
                        ),
                    )
                })?;
            pftl_uniswap_consensus_supply_status(route)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let ledgers = read_pftl_uniswap_bridge_ledgers(&options.data_dir)?;
            let ledger = pftl_uniswap_bridge_ledger_for_route(&ledgers, &options.route_id)?;
            pftl_uniswap_bridge_supply_status(ledger)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
        }
        Err(error) => Err(error),
    }
}

fn pftl_uniswap_consensus_routes_status(
    ledger: &LedgerState,
) -> io::Result<PftlUniswapRoutesStatusReport> {
    if ledger.pftl_uniswap_routes.len() > PFTL_UNISWAP_STATUS_MAX_ROWS {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "PFTL-Uniswap consensus route status request exceeds the status row limit",
        ));
    }
    let mut routes = ledger
        .pftl_uniswap_routes
        .iter()
        .map(pftl_uniswap_consensus_route_status_row)
        .collect::<io::Result<Vec<_>>>()?;
    routes.sort_by(|left, right| left.route_id.cmp(&right.route_id));
    Ok(PftlUniswapRoutesStatusReport {
        schema: "postfiat-pftl-uniswap-routes-status-v1".to_string(),
        route_count: routes.len() as u64,
        routes,
    })
}

fn pftl_uniswap_consensus_route_status_row(
    route: &PftlUniswapConsensusRouteState,
) -> io::Result<PftlUniswapRouteStatusRow> {
    route
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let supply_cap_remaining_atoms = route
        .route_supply_cap_atoms
        .checked_sub(route.authorized_valid_supply_atoms)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "PFTL-Uniswap consensus route supply exceeds route cap",
            )
        })?;
    let outstanding_export_packet_count = route
        .export_packets
        .values()
        .filter(|packet| packet.status == PFTL_UNISWAP_EXPORT_STATUS_SOURCE_DEBITED)
        .count() as u64;
    let consumed_export_packet_count = route
        .export_packets
        .values()
        .filter(|packet| packet.status == PFTL_UNISWAP_EXPORT_STATUS_DESTINATION_CONSUMED)
        .count() as u64;
    let refunded_export_packet_count = route
        .export_packets
        .values()
        .filter(|packet| packet.status == PFTL_UNISWAP_EXPORT_STATUS_SOURCE_REFUNDED)
        .count() as u64;
    let imported_return_burn_count = route
        .return_imports
        .values()
        .filter(|import| import.status == PFTL_UNISWAP_RETURN_STATUS_IMPORTED)
        .count() as u64;
    Ok(PftlUniswapRouteStatusRow {
        route_id: route.route_id.clone(),
        route_family: route.route_family.clone(),
        route_config_digest: route.route_config_digest.clone(),
        route_trust_class: route.route_trust_class.clone(),
        route_live: !route.paused && route.route_trust_class != ROUTE_TRUST_CLASS_DISABLED,
        paused: route.paused,
        native_nav_asset_id: route.native_nav_asset_id.clone(),
        settlement_asset_id: route.settlement_asset_id.clone(),
        wrapped_navcoin_token: route.wrapped_navcoin_token.clone(),
        handoff_controller: route.handoff_controller.clone(),
        settlement_adapter: route.settlement_adapter.clone(),
        ethereum_chain_id: route.ethereum_chain_id,
        latest_finalized_nav_epoch: route.latest_finalized_nav_epoch,
        route_supply_cap_atoms: route.route_supply_cap_atoms,
        packet_notional_cap_atoms: route.packet_notional_cap_atoms,
        authorized_valid_supply_atoms: route.authorized_valid_supply_atoms,
        supply_cap_remaining_atoms,
        outstanding_bridge_claims_atoms: route.outstanding_bridge_claims_atoms,
        pending_return_import_claims_atoms: route.pending_return_import_claims_atoms,
        primary_subscription_count: route.primary_subscription_nonces.len() as u64,
        export_packet_count: route.export_packets.len() as u64,
        outstanding_export_packet_count,
        consumed_export_packet_count,
        refunded_export_packet_count,
        return_burn_count: route.return_imports.len() as u64,
        pending_return_burn_count: 0,
        imported_return_burn_count,
        ledger_hash: pftl_uniswap_route_state_hash(route),
    })
}

fn pftl_uniswap_consensus_packet_status(
    route: &PftlUniswapConsensusRouteState,
    packet_hash: &str,
) -> io::Result<PftlUniswapPacketStatusReport> {
    route
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    validate_lower_hex_len("navcoin_bridge_packet.packet_hash", packet_hash, 96)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    let packet = route.export_packets.get(packet_hash).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "PFTL-to-Uniswap bridge packet hash is unknown",
        )
    })?;
    Ok(PftlUniswapPacketStatusReport {
        schema: "postfiat-pftl-uniswap-packet-status-v1".to_string(),
        route_id: route.route_id.clone(),
        route_config_digest: route.route_config_digest.clone(),
        packet_hash: packet_hash.to_string(),
        packet: pftl_uniswap_consensus_export_packet_status_row(packet_hash, packet)?,
        ledger_hash: pftl_uniswap_route_state_hash(route),
    })
}

fn pftl_uniswap_consensus_export_packet_status_row(
    packet_hash: &str,
    packet: &PftlUniswapConsensusExportPacket,
) -> io::Result<PftlUniswapExportPacketStatusRow> {
    let (status, claim_class) = match packet.status.as_str() {
        PFTL_UNISWAP_EXPORT_STATUS_SOURCE_DEBITED => (
            PftlUniswapExportPacketStatus::SourceDebited,
            "outstanding_bridge_claim",
        ),
        PFTL_UNISWAP_EXPORT_STATUS_DESTINATION_CONSUMED => (
            PftlUniswapExportPacketStatus::DestinationConsumed,
            "destination_consumed",
        ),
        PFTL_UNISWAP_EXPORT_STATUS_SOURCE_REFUNDED => (
            PftlUniswapExportPacketStatus::SourceRefunded,
            "source_refunded",
        ),
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unsupported pftl_uniswap export packet status",
            ));
        }
    };
    Ok(PftlUniswapExportPacketStatusRow {
        packet_hash: packet_hash.to_string(),
        nonce: packet.nonce.clone(),
        source_wallet: packet.source_wallet.clone(),
        ethereum_recipient: packet.ethereum_recipient.clone(),
        amount_atoms: packet.amount_atoms,
        source_height: packet.source_height,
        destination_deadline_seconds: packet.destination_deadline_seconds,
        refund_not_before_height: packet.refund_not_before_height,
        status,
        claim_class: claim_class.to_string(),
    })
}

fn pftl_uniswap_consensus_supply_status(
    route: &PftlUniswapConsensusRouteState,
) -> io::Result<PftlUniswapSupplyStatusReport> {
    route
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let live_supply_sum_atoms = route
        .pftl_spendable_supply_atoms
        .checked_add(route.ethereum_spendable_supply_atoms)
        .and_then(|value| value.checked_add(route.other_registered_venue_supply_atoms))
        .and_then(|value| value.checked_add(route.outstanding_bridge_claims_atoms))
        .and_then(|value| value.checked_add(route.pending_return_import_claims_atoms))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "PFTL-Uniswap consensus route live supply sum overflow",
            )
        })?;
    let supply_cap_remaining_atoms = route
        .route_supply_cap_atoms
        .checked_sub(route.authorized_valid_supply_atoms)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "PFTL-Uniswap consensus route supply exceeds route cap",
            )
        })?;
    let native_spendable_balance_sum_atoms = route
        .native_spendable_balances_atoms
        .values()
        .try_fold(0u64, |sum, amount| {
            sum.checked_add(*amount).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "PFTL-Uniswap native spendable balance sum overflow",
                )
            })
        })?;
    let native_spendable_balance_count = route.native_spendable_balances_atoms.len();
    let mut native_spendable_balances = route
        .native_spendable_balances_atoms
        .iter()
        .take(PFTL_UNISWAP_STATUS_MAX_ROWS)
        .map(|(wallet, amount_atoms)| PftlUniswapNativeBalanceRow {
            wallet: wallet.clone(),
            amount_atoms: *amount_atoms,
        })
        .collect::<Vec<_>>();
    native_spendable_balances.sort_by(|left, right| left.wallet.cmp(&right.wallet));
    Ok(PftlUniswapSupplyStatusReport {
        schema: "postfiat-pftl-uniswap-supply-status-v1".to_string(),
        route_id: route.route_id.clone(),
        route_config_digest: route.route_config_digest.clone(),
        native_nav_asset_id: route.native_nav_asset_id.clone(),
        settlement_asset_id: route.settlement_asset_id.clone(),
        wrapped_navcoin_token: route.wrapped_navcoin_token.clone(),
        native_spendable_balances,
        native_spendable_balance_count: native_spendable_balance_count as u64,
        native_spendable_balance_limit: PFTL_UNISWAP_STATUS_MAX_ROWS as u64,
        native_spendable_balances_truncated: native_spendable_balance_count
            > PFTL_UNISWAP_STATUS_MAX_ROWS,
        native_spendable_balance_sum_atoms,
        authorized_valid_supply_atoms: route.authorized_valid_supply_atoms,
        pftl_spendable_supply_atoms: route.pftl_spendable_supply_atoms,
        ethereum_spendable_supply_atoms: route.ethereum_spendable_supply_atoms,
        other_registered_venue_supply_atoms: route.other_registered_venue_supply_atoms,
        outstanding_bridge_claims_atoms: route.outstanding_bridge_claims_atoms,
        pending_return_import_claims_atoms: route.pending_return_import_claims_atoms,
        live_supply_sum_atoms,
        route_supply_cap_atoms: route.route_supply_cap_atoms,
        supply_cap_remaining_atoms,
        packet_notional_cap_atoms: route.packet_notional_cap_atoms,
        settlement_reserve_atoms: route.settlement_reserve_atoms,
        invariant_holds: live_supply_sum_atoms == route.authorized_valid_supply_atoms,
        ledger_hash: pftl_uniswap_route_state_hash(route),
    })
}

pub fn navcoin_bridge_receipt_replay(
    options: NavcoinBridgeReceiptReplayOptions,
) -> io::Result<NavcoinBridgeReceiptReplayReport> {
    validate_navcoin_bridge_route_id(&options.route_id)?;
    let ledgers = read_pftl_uniswap_bridge_ledgers(&options.data_dir)?;
    let final_ledger = pftl_uniswap_bridge_ledger_for_route(&ledgers, &options.route_id)?;
    let initial_ledger = pftl_uniswap_initial_ledger_for_receipt_replay(final_ledger);
    let initial_ledger_hash = pftl_uniswap_bridge_ledger_hash(&initial_ledger)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let final_ledger_hash = pftl_uniswap_bridge_ledger_hash(final_ledger)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let receipts = read_pftl_uniswap_bridge_receipts(&options.data_dir)?;
    let route_receipts = receipts
        .into_iter()
        .filter(|receipt| receipt.route_id == options.route_id)
        .collect::<Vec<_>>();
    let (receipt_root, status, replay) = if route_receipts.is_empty() {
        if initial_ledger != *final_ledger {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "PFTL-to-Uniswap route has no receipts but current ledger is not the deterministic initial ledger",
            ));
        }
        (None, "empty_clean".to_string(), None)
    } else {
        let replay = pftl_uniswap_verify_transition_receipt_replay(
            &initial_ledger,
            &route_receipts,
            final_ledger,
        )
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        (
            Some(replay.receipt_root.clone()),
            "verified".to_string(),
            Some(replay),
        )
    };
    Ok(NavcoinBridgeReceiptReplayReport {
        schema: "postfiat-navcoin-bridge-receipt-replay-v1".to_string(),
        route_id: options.route_id,
        route_config_digest: final_ledger.route_config_digest.clone(),
        initial_ledger_hash,
        final_ledger_hash,
        receipt_root,
        receipt_count: route_receipts.len() as u64,
        ledger_file: options
            .data_dir
            .join(PFTL_UNISWAP_BRIDGE_LEDGER_FILE)
            .display()
            .to_string(),
        receipt_file: options
            .data_dir
            .join(PFTL_UNISWAP_BRIDGE_RECEIPTS_FILE)
            .display()
            .to_string(),
        status,
        replay,
    })
}

pub fn navcoin_bridge_route_init(
    options: NavcoinBridgeRouteInitOptions,
) -> io::Result<NavcoinBridgeRouteInitReport> {
    if options.ethereum_chain_id == 0
        || options.latest_finalized_nav_epoch == 0
        || options.return_finality_blocks == 0
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "navcoin bridge route init requires nonzero chain id, NAV epoch, and return finality blocks",
        ));
    }
    let config: PftlUniswapRouteConfig =
        read_json_file(&options.config_file, "PFTL-to-Uniswap route config")?;
    let ledger = pftl_uniswap_bridge_ledger_from_config(
        &config,
        options.ethereum_chain_id,
        options.latest_finalized_nav_epoch,
        options.return_finality_blocks,
    )
    .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    let mut ledgers = read_pftl_uniswap_bridge_ledgers(&options.data_dir)?;
    match ledgers
        .iter()
        .position(|existing| existing.route_id == ledger.route_id)
    {
        Some(index) if options.replace => ledgers[index] = ledger.clone(),
        Some(_) => {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!(
                    "PFTL-to-Uniswap bridge route `{}` already exists; pass --replace to overwrite",
                    ledger.route_id
                ),
            ))
        }
        None => ledgers.push(ledger.clone()),
    }
    write_pftl_uniswap_bridge_ledgers(&options.data_dir, &mut ledgers)?;
    let ledger_hash = pftl_uniswap_bridge_ledger_hash(&ledger)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    Ok(NavcoinBridgeRouteInitReport {
        schema: "postfiat-navcoin-bridge-route-init-v1".to_string(),
        route_id: ledger.route_id,
        route_config_digest: ledger.route_config_digest,
        ledger_hash,
        ledger_file: options
            .data_dir
            .join(PFTL_UNISWAP_BRIDGE_LEDGER_FILE)
            .display()
            .to_string(),
        route_count: ledgers.len() as u64,
    })
}

pub fn navcoin_bridge_launch_config_template(
    options: NavcoinBridgeLaunchConfigTemplateOptions,
) -> io::Result<NavcoinBridgeLaunchConfigTemplateReport> {
    let route_config: PftlUniswapRouteConfig =
        read_json_file(&options.route_config_file, "PFTL-to-Uniswap route config")?;
    let official_uniswap: PftlUniswapOfficialUniswapV4Deployments = read_json_file(
        &options.official_uniswap_file,
        "official Uniswap v4 deployments",
    )?;
    let route_config_digest = pftl_uniswap_route_config_digest(&route_config)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    let launch_config = PftlUniswapLaunchConfig {
        schema: "postfiat-pftl-uniswap-launch-config-v1".to_string(),
        route_id: route_config.route_id.clone(),
        route_config_digest: route_config_digest.clone(),
        route_trust_class: route_config.route_trust_class.clone(),
        native_nav_asset_id: route_config.native_nav_asset_id.clone(),
        settlement_asset_id: route_config.settlement_asset_id.clone(),
        wrapped_navcoin_token: route_config.wrapped_navcoin_token.clone(),
        usdc_token: options.usdc_token,
        handoff_controller: route_config.handoff_controller.clone(),
        receipt_verifier: options.receipt_verifier,
        settlement_adapter: route_config.settlement_adapter.clone(),
        official_uniswap,
        uniswap_pool_key_hash: options.uniswap_pool_key_hash,
        uniswap_pool_id: route_config.uniswap_pool_id_or_path.clone(),
        seed: PftlUniswapPoolSeedConfig {
            pricing_nav_epoch: route_config.seed_nav_epoch,
            pricing_reserve_packet_hash: options.pricing_reserve_packet_hash,
            seed_usdc_atoms: route_config.seed_usdc_atoms,
            seed_wrapped_navcoin_atoms: route_config.seed_wrapped_navcoin_atoms,
            nav_price_settlement_atoms_per_nav_atom: options
                .nav_price_settlement_atoms_per_nav_atom,
            tick_lower: options.tick_lower,
            tick_upper: options.tick_upper,
            fee_pips: options.fee_pips,
            lp_recipient: route_config.lp_recipient.clone(),
            position_recipient: options.position_recipient,
            lp_custody_policy: route_config.lp_custody_policy.clone(),
        },
        fork_rehearsal_required: true,
    };
    let launch_config_digest = pftl_uniswap_launch_config_digest(&launch_config)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    if options.output_file.exists() && !options.overwrite {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "PFTL-to-Uniswap launch config template `{}` already exists; pass --overwrite to replace it",
                options.output_file.display()
            ),
        ));
    }
    vault_bridge_write_json_file(&options.output_file, &launch_config)?;
    Ok(NavcoinBridgeLaunchConfigTemplateReport {
        schema: "postfiat-navcoin-bridge-launch-config-template-v1".to_string(),
        route_id: launch_config.route_id.clone(),
        route_config_digest,
        launch_config_digest,
        output_file: options.output_file.display().to_string(),
        launch_config,
    })
}

pub fn navcoin_bridge_launch_config_init(
    options: NavcoinBridgeLaunchConfigInitOptions,
) -> io::Result<NavcoinBridgeLaunchConfigInitReport> {
    let launch_config: PftlUniswapLaunchConfig =
        read_json_file(&options.launch_config_file, "PFTL-to-Uniswap launch config")?;
    let ledgers = read_pftl_uniswap_bridge_ledgers(&options.data_dir)?;
    let ledger = pftl_uniswap_bridge_ledger_for_route(&ledgers, &launch_config.route_id)?;
    validate_pftl_uniswap_launch_config_against_ledger(&launch_config, ledger)?;
    let launch_config_digest = pftl_uniswap_launch_config_digest(&launch_config)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let mut launch_configs = read_pftl_uniswap_launch_configs(&options.data_dir)?;
    match launch_configs
        .iter()
        .position(|existing| existing.route_id == launch_config.route_id)
    {
        Some(index) if options.replace => launch_configs[index] = launch_config.clone(),
        Some(_) => {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!(
                    "PFTL-to-Uniswap launch config for route `{}` already exists; pass --replace to overwrite",
                    launch_config.route_id
                ),
            ))
        }
        None => launch_configs.push(launch_config.clone()),
    }
    write_pftl_uniswap_launch_configs(&options.data_dir, &mut launch_configs)?;
    Ok(NavcoinBridgeLaunchConfigInitReport {
        schema: "postfiat-navcoin-bridge-launch-config-init-v1".to_string(),
        route_id: launch_config.route_id,
        route_config_digest: launch_config.route_config_digest,
        launch_config_digest,
        launch_config_file: options
            .data_dir
            .join(PFTL_UNISWAP_LAUNCH_CONFIG_FILE)
            .display()
            .to_string(),
        launch_config_count: launch_configs.len() as u64,
    })
}

pub fn navcoin_bridge_record_fork_rehearsal(
    options: NavcoinBridgeRecordForkRehearsalOptions,
) -> io::Result<NavcoinBridgeForkRehearsalRecordReport> {
    validate_navcoin_bridge_route_id(&options.route_id)?;
    let evidence: PftlUniswapForkRehearsalEvidence = read_json_file(
        &options.evidence_file,
        "PFTL-to-Uniswap fork rehearsal evidence",
    )?;
    let ledgers = read_pftl_uniswap_bridge_ledgers(&options.data_dir)?;
    let ledger = pftl_uniswap_bridge_ledger_for_route(&ledgers, &options.route_id)?;
    let launch_configs = read_pftl_uniswap_launch_configs(&options.data_dir)?;
    let launch_config = pftl_uniswap_launch_config_for_route(&launch_configs, &options.route_id)?;
    validate_pftl_uniswap_launch_config_against_ledger(launch_config, ledger)?;
    validate_pftl_uniswap_fork_rehearsal_evidence(&evidence, launch_config)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let evidence_digest = pftl_uniswap_fork_rehearsal_evidence_digest(&evidence, launch_config)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let launch_config_digest = pftl_uniswap_launch_config_digest(launch_config)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let mut evidences = read_pftl_uniswap_fork_rehearsals(&options.data_dir)?;
    for existing in &evidences {
        if existing.rehearsal_id == evidence.rehearsal_id {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!(
                    "PFTL-to-Uniswap fork rehearsal `{}` already exists",
                    evidence.rehearsal_id
                ),
            ));
        }
        if existing.route_config_digest == launch_config.route_config_digest {
            let existing_digest =
                pftl_uniswap_fork_rehearsal_evidence_digest(existing, launch_config)
                    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
            if existing_digest == evidence_digest {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!(
                        "PFTL-to-Uniswap fork rehearsal digest `{evidence_digest}` already exists"
                    ),
                ));
            }
        }
    }
    evidences.push(evidence.clone());
    write_pftl_uniswap_fork_rehearsals(&options.data_dir, &mut evidences)?;
    Ok(NavcoinBridgeForkRehearsalRecordReport {
        schema: "postfiat-navcoin-bridge-fork-rehearsal-record-v1".to_string(),
        route_id: options.route_id,
        route_config_digest: launch_config.route_config_digest.clone(),
        launch_config_digest,
        rehearsal_id: evidence.rehearsal_id,
        rehearsal_evidence_digest: evidence_digest,
        evidence_file: options
            .data_dir
            .join(PFTL_UNISWAP_FORK_REHEARSAL_FILE)
            .display()
            .to_string(),
        evidence_count: evidences.len() as u64,
    })
}

pub fn navcoin_bridge_packet_preflight(
    options: NavcoinBridgePacketPreflightOptions,
) -> io::Result<NavcoinBridgePacketPreflightReport> {
    validate_navcoin_bridge_route_id(&options.route_id)?;
    let packet: PftlUniswapMintAndSwapPacket =
        read_json_file(&options.packet_file, "PFTL-to-Uniswap mint-and-swap packet")?;
    let ledgers = read_pftl_uniswap_bridge_ledgers(&options.data_dir)?;
    let ledger = pftl_uniswap_bridge_ledger_for_route(&ledgers, &options.route_id)?;
    validate_pftl_uniswap_bridge_ledger(ledger)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let launch_configs = read_pftl_uniswap_launch_configs(&options.data_dir)?;
    let launch_config = pftl_uniswap_launch_config_for_route(&launch_configs, &options.route_id)?;
    validate_pftl_uniswap_launch_config_against_ledger(launch_config, ledger)?;
    validate_pftl_uniswap_packet_against_launch_config(&packet, launch_config)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let packet_digest = pftl_uniswap_packet_id(&packet)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let launch_config_digest = pftl_uniswap_launch_config_digest(launch_config)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let ledger_hash = pftl_uniswap_bridge_ledger_hash(ledger)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    Ok(NavcoinBridgePacketPreflightReport {
        schema: "postfiat-navcoin-bridge-packet-preflight-v1".to_string(),
        route_id: options.route_id,
        route_config_digest: launch_config.route_config_digest.clone(),
        launch_config_digest,
        packet_digest,
        ledger_hash,
        packet_file: options.packet_file.display().to_string(),
        status: "ready".to_string(),
    })
}

pub fn navcoin_bridge_primary_subscribe(
    options: NavcoinBridgePrimarySubscribeOptions,
) -> io::Result<NavcoinBridgeTransitionApplyReport> {
    let request: PftlUniswapPrimarySubscriptionRequest = read_json_file(
        &options.request_file,
        "PFTL-to-Uniswap primary subscription request",
    )?;
    let route_id = request.route_id.clone();
    pftl_uniswap_apply_transition(&options.data_dir, &route_id, |ledger| {
        pftl_uniswap_apply_primary_subscription_with_receipt(ledger, request)
    })
}

pub fn navcoin_bridge_export_debit(
    options: NavcoinBridgeExportDebitOptions,
) -> io::Result<NavcoinBridgeTransitionApplyReport> {
    let request: PftlUniswapExportDebitRequest = read_json_file(
        &options.request_file,
        "PFTL-to-Uniswap export debit request",
    )?;
    let route_id = request.route_id.clone();
    pftl_uniswap_apply_transition(&options.data_dir, &route_id, |ledger| {
        pftl_uniswap_export_debit_with_receipt(ledger, request)
    })
}

pub fn navcoin_bridge_destination_consume(
    options: NavcoinBridgeDestinationConsumeOptions,
) -> io::Result<NavcoinBridgeTransitionApplyReport> {
    validate_navcoin_bridge_route_id(&options.route_id)?;
    let packet_hash = options.packet_hash.clone();
    pftl_uniswap_apply_transition(&options.data_dir, &options.route_id, |ledger| {
        pftl_uniswap_mark_destination_consumed_with_receipt(ledger, &packet_hash)
    })
}

pub fn navcoin_bridge_refund_source(
    options: NavcoinBridgeRefundSourceOptions,
) -> io::Result<NavcoinBridgeTransitionApplyReport> {
    validate_navcoin_bridge_route_id(&options.route_id)?;
    let request: PftlUniswapRefundRequest = read_json_file(
        &options.request_file,
        "PFTL-to-Uniswap refund source request",
    )?;
    pftl_uniswap_apply_transition(&options.data_dir, &options.route_id, |ledger| {
        pftl_uniswap_refund_source_with_receipt(ledger, request)
    })
}

pub fn navcoin_bridge_record_return_burn(
    options: NavcoinBridgeRecordReturnBurnOptions,
) -> io::Result<NavcoinBridgeTransitionApplyReport> {
    validate_navcoin_bridge_route_id(&options.route_id)?;
    let request: PftlUniswapReturnBurnRequest =
        read_json_file(&options.request_file, "PFTL-to-Uniswap return burn request")?;
    pftl_uniswap_apply_transition(&options.data_dir, &options.route_id, |ledger| {
        pftl_uniswap_record_return_burn_with_receipt(ledger, request)
    })
}

pub fn navcoin_bridge_return_burn_request(
    options: NavcoinBridgeReturnBurnRequestOptions,
) -> io::Result<NavcoinBridgeReturnBurnRequestReport> {
    validate_navcoin_bridge_route_id(&options.route_id)?;
    if options.amount_atoms == 0 || options.burn_height == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "navcoin bridge return burn request requires nonzero amount and burn height",
        ));
    }
    let ledgers = read_pftl_uniswap_bridge_ledgers(&options.data_dir)?;
    let ledger = pftl_uniswap_bridge_ledger_for_route(&ledgers, &options.route_id)?;
    validate_pftl_uniswap_bridge_ledger(ledger)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let finalized_height = options
        .burn_height
        .checked_add(ledger.return_finality_blocks)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "return burn finalized height overflow",
            )
        })?;
    let mut request = PftlUniswapReturnBurnRequest {
        burn_event_hash: "0".repeat(64),
        ethereum_chain_id: ledger.ethereum_chain_id,
        bridge_controller: ledger.handoff_controller.clone(),
        wrapped_navcoin_token: ledger.wrapped_navcoin_token.clone(),
        native_nav_asset_id: ledger.native_nav_asset_id.clone(),
        ethereum_sender: options.ethereum_sender,
        pftl_recipient: options.pftl_recipient,
        amount_atoms: options.amount_atoms,
        return_nonce: options.return_nonce,
        burn_height: options.burn_height,
        finalized_height,
    };
    request.burn_event_hash = pftl_uniswap_return_burn_id(&request)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    if options.output_file.exists() && !options.overwrite {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "PFTL-to-Uniswap return burn request `{}` already exists; pass --overwrite to replace it",
                options.output_file.display()
            ),
        ));
    }
    vault_bridge_write_json_file(&options.output_file, &request)?;
    Ok(NavcoinBridgeReturnBurnRequestReport {
        schema: "postfiat-navcoin-bridge-return-burn-request-v1".to_string(),
        route_id: options.route_id,
        burn_event_hash: request.burn_event_hash.clone(),
        output_file: options.output_file.display().to_string(),
        request,
    })
}

pub fn navcoin_bridge_import_return(
    options: NavcoinBridgeImportReturnOptions,
) -> io::Result<NavcoinBridgeTransitionApplyReport> {
    validate_navcoin_bridge_route_id(&options.route_id)?;
    let burn_event_hash = options.burn_event_hash.clone();
    let pftl_recipient = options.pftl_recipient.clone();
    pftl_uniswap_apply_transition(&options.data_dir, &options.route_id, |ledger| {
        pftl_uniswap_import_return_with_receipt(ledger, &burn_event_hash, &pftl_recipient)
    })
}

fn pftl_uniswap_initial_ledger_for_receipt_replay(
    ledger: &PftlUniswapBridgeLedger,
) -> PftlUniswapBridgeLedger {
    PftlUniswapBridgeLedger {
        schema: ledger.schema.clone(),
        route_id: ledger.route_id.clone(),
        route_family: ledger.route_family.clone(),
        route_config_digest: ledger.route_config_digest.clone(),
        route_trust_class: ledger.route_trust_class.clone(),
        native_nav_asset_id: ledger.native_nav_asset_id.clone(),
        settlement_asset_id: ledger.settlement_asset_id.clone(),
        handoff_controller: ledger.handoff_controller.clone(),
        settlement_adapter: ledger.settlement_adapter.clone(),
        wrapped_navcoin_token: ledger.wrapped_navcoin_token.clone(),
        ethereum_chain_id: ledger.ethereum_chain_id,
        route_supply_cap_atoms: ledger.route_supply_cap_atoms,
        packet_notional_cap_atoms: ledger.packet_notional_cap_atoms,
        latest_finalized_nav_epoch: ledger.latest_finalized_nav_epoch,
        return_finality_blocks: ledger.return_finality_blocks,
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
        return_burns: BTreeMap::new(),
        paused: false,
    }
}

fn read_pftl_uniswap_bridge_ledgers(data_dir: &Path) -> io::Result<Vec<PftlUniswapBridgeLedger>> {
    let path = data_dir.join(PFTL_UNISWAP_BRIDGE_LEDGER_FILE);
    let ledgers: Vec<PftlUniswapBridgeLedger> =
        match read_json_file(&path, "PFTL-to-Uniswap bridge ledgers") {
            Ok(ledgers) => ledgers,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error),
        };
    validate_pftl_uniswap_bridge_ledgers(&ledgers)?;
    Ok(ledgers)
}

fn write_pftl_uniswap_bridge_ledgers(
    data_dir: &Path,
    ledgers: &mut Vec<PftlUniswapBridgeLedger>,
) -> io::Result<()> {
    validate_pftl_uniswap_bridge_ledgers(ledgers)?;
    ledgers.sort_by(|left, right| left.route_id.cmp(&right.route_id));
    let path = data_dir.join(PFTL_UNISWAP_BRIDGE_LEDGER_FILE);
    vault_bridge_write_json_file(&path, ledgers)
}

fn read_pftl_uniswap_launch_configs(data_dir: &Path) -> io::Result<Vec<PftlUniswapLaunchConfig>> {
    let path = data_dir.join(PFTL_UNISWAP_LAUNCH_CONFIG_FILE);
    let launch_configs: Vec<PftlUniswapLaunchConfig> =
        match read_json_file(&path, "PFTL-to-Uniswap launch configs") {
            Ok(launch_configs) => launch_configs,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error),
        };
    validate_pftl_uniswap_launch_configs(&launch_configs)?;
    Ok(launch_configs)
}

fn write_pftl_uniswap_launch_configs(
    data_dir: &Path,
    launch_configs: &mut Vec<PftlUniswapLaunchConfig>,
) -> io::Result<()> {
    validate_pftl_uniswap_launch_configs(launch_configs)?;
    launch_configs.sort_by(|left, right| left.route_id.cmp(&right.route_id));
    let path = data_dir.join(PFTL_UNISWAP_LAUNCH_CONFIG_FILE);
    vault_bridge_write_json_file(&path, launch_configs)
}

fn read_pftl_uniswap_fork_rehearsals(
    data_dir: &Path,
) -> io::Result<Vec<PftlUniswapForkRehearsalEvidence>> {
    let path = data_dir.join(PFTL_UNISWAP_FORK_REHEARSAL_FILE);
    let evidences: Vec<PftlUniswapForkRehearsalEvidence> =
        match read_json_file(&path, "PFTL-to-Uniswap fork rehearsals") {
            Ok(evidences) => evidences,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error),
        };
    validate_pftl_uniswap_fork_rehearsals(&evidences)?;
    Ok(evidences)
}

fn write_pftl_uniswap_fork_rehearsals(
    data_dir: &Path,
    evidences: &mut Vec<PftlUniswapForkRehearsalEvidence>,
) -> io::Result<()> {
    validate_pftl_uniswap_fork_rehearsals(evidences)?;
    evidences.sort_by(|left, right| left.rehearsal_id.cmp(&right.rehearsal_id));
    let path = data_dir.join(PFTL_UNISWAP_FORK_REHEARSAL_FILE);
    vault_bridge_write_json_file(&path, evidences)
}

fn read_pftl_uniswap_bridge_receipts(
    data_dir: &Path,
) -> io::Result<Vec<PftlUniswapTransitionReceipt>> {
    let path = data_dir.join(PFTL_UNISWAP_BRIDGE_RECEIPTS_FILE);
    let receipts: Vec<PftlUniswapTransitionReceipt> =
        match read_json_file(&path, "PFTL-to-Uniswap bridge receipts") {
            Ok(receipts) => receipts,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error),
        };
    validate_pftl_uniswap_bridge_receipts(&receipts)?;
    Ok(receipts)
}

fn append_pftl_uniswap_bridge_receipt(
    data_dir: &Path,
    receipt: PftlUniswapTransitionReceipt,
) -> io::Result<String> {
    let receipt_hash = pftl_uniswap_transition_receipt_hash(&receipt)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let mut receipts = read_pftl_uniswap_bridge_receipts(data_dir)?;
    for existing in &receipts {
        let existing_hash = pftl_uniswap_transition_receipt_hash(existing)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        if existing_hash == receipt_hash {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("PFTL-to-Uniswap receipt `{receipt_hash}` already exists"),
            ));
        }
    }
    receipts.push(receipt);
    let path = data_dir.join(PFTL_UNISWAP_BRIDGE_RECEIPTS_FILE);
    vault_bridge_write_json_file(&path, &receipts)?;
    Ok(receipt_hash)
}

fn validate_pftl_uniswap_bridge_receipts(
    receipts: &[PftlUniswapTransitionReceipt],
) -> io::Result<()> {
    if receipts.len() > PFTL_UNISWAP_STATUS_MAX_ROWS.saturating_mul(16) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "PFTL-to-Uniswap bridge receipt file exceeds bounded local history limit",
        ));
    }
    let mut receipt_hashes = BTreeSet::new();
    for receipt in receipts {
        let receipt_hash = pftl_uniswap_transition_receipt_hash(receipt)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        if !receipt_hashes.insert(receipt_hash.clone()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("duplicate PFTL-to-Uniswap receipt `{receipt_hash}`"),
            ));
        }
    }
    Ok(())
}

fn pftl_uniswap_apply_transition<T, F>(
    data_dir: &Path,
    route_id: &str,
    mutate: F,
) -> io::Result<NavcoinBridgeTransitionApplyReport>
where
    T: Serialize,
    F: FnOnce(
        &mut PftlUniswapBridgeLedger,
    ) -> Result<(T, PftlUniswapTransitionReceipt), BridgeError>,
{
    validate_navcoin_bridge_route_id(route_id)?;
    let mut ledgers = read_pftl_uniswap_bridge_ledgers(data_dir)?;
    let index = ledgers
        .iter()
        .position(|ledger| ledger.route_id == route_id)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("missing PFTL-to-Uniswap bridge route `{route_id}`"),
            )
        })?;
    let (result, receipt, ledger_hash) = {
        let ledger = &mut ledgers[index];
        let (result, receipt) =
            mutate(ledger).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        let ledger_hash = pftl_uniswap_bridge_ledger_hash(ledger)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        (result, receipt, ledger_hash)
    };
    write_pftl_uniswap_bridge_ledgers(data_dir, &mut ledgers)?;
    let receipt_hash = append_pftl_uniswap_bridge_receipt(data_dir, receipt.clone())?;
    let result = serde_json::to_value(result).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("PFTL-to-Uniswap transition result serialization failed: {error}"),
        )
    })?;
    Ok(NavcoinBridgeTransitionApplyReport {
        schema: "postfiat-navcoin-bridge-transition-apply-v1".to_string(),
        route_id: route_id.to_string(),
        transition: receipt.transition.clone(),
        ledger_hash,
        receipt_hash,
        ledger_file: data_dir
            .join(PFTL_UNISWAP_BRIDGE_LEDGER_FILE)
            .display()
            .to_string(),
        receipt_file: data_dir
            .join(PFTL_UNISWAP_BRIDGE_RECEIPTS_FILE)
            .display()
            .to_string(),
        receipt,
        result,
    })
}

fn validate_pftl_uniswap_bridge_ledgers(ledgers: &[PftlUniswapBridgeLedger]) -> io::Result<()> {
    if ledgers.len() > PFTL_UNISWAP_STATUS_MAX_ROWS {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "PFTL-to-Uniswap bridge ledger file exceeds status row limit",
        ));
    }
    let mut route_ids = BTreeSet::new();
    for ledger in ledgers {
        validate_pftl_uniswap_bridge_ledger(ledger)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        if !route_ids.insert(ledger.route_id.clone()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "duplicate PFTL-to-Uniswap bridge route id `{}`",
                    ledger.route_id
                ),
            ));
        }
    }
    Ok(())
}

fn validate_pftl_uniswap_launch_configs(
    launch_configs: &[PftlUniswapLaunchConfig],
) -> io::Result<()> {
    if launch_configs.len() > PFTL_UNISWAP_STATUS_MAX_ROWS {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "PFTL-to-Uniswap launch config file exceeds status row limit",
        ));
    }
    let mut route_ids = BTreeSet::new();
    let mut digests = BTreeSet::new();
    for launch_config in launch_configs {
        validate_pftl_uniswap_launch_config(launch_config)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        if !route_ids.insert(launch_config.route_id.clone()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "duplicate PFTL-to-Uniswap launch config route id `{}`",
                    launch_config.route_id
                ),
            ));
        }
        let digest = pftl_uniswap_launch_config_digest(launch_config)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        if !digests.insert(digest.clone()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("duplicate PFTL-to-Uniswap launch config digest `{digest}`"),
            ));
        }
    }
    Ok(())
}

fn validate_pftl_uniswap_fork_rehearsals(
    evidences: &[PftlUniswapForkRehearsalEvidence],
) -> io::Result<()> {
    if evidences.len() > PFTL_UNISWAP_STATUS_MAX_ROWS {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "PFTL-to-Uniswap fork rehearsal file exceeds status row limit",
        ));
    }
    let mut rehearsal_ids = BTreeSet::new();
    for evidence in evidences {
        if evidence.rehearsal_id.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "PFTL-to-Uniswap fork rehearsal id must be nonempty",
            ));
        }
        if !rehearsal_ids.insert(evidence.rehearsal_id.clone()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "duplicate PFTL-to-Uniswap fork rehearsal id `{}`",
                    evidence.rehearsal_id
                ),
            ));
        }
    }
    Ok(())
}

fn pftl_uniswap_bridge_ledger_for_route<'a>(
    ledgers: &'a [PftlUniswapBridgeLedger],
    route_id: &str,
) -> io::Result<&'a PftlUniswapBridgeLedger> {
    ledgers
        .iter()
        .find(|ledger| ledger.route_id == route_id)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("missing PFTL-to-Uniswap bridge route `{route_id}`"),
            )
        })
}

fn pftl_uniswap_launch_config_for_route<'a>(
    launch_configs: &'a [PftlUniswapLaunchConfig],
    route_id: &str,
) -> io::Result<&'a PftlUniswapLaunchConfig> {
    launch_configs
        .iter()
        .find(|launch_config| launch_config.route_id == route_id)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("missing PFTL-to-Uniswap launch config for route `{route_id}`"),
            )
        })
}

fn validate_pftl_uniswap_launch_config_against_ledger(
    launch_config: &PftlUniswapLaunchConfig,
    ledger: &PftlUniswapBridgeLedger,
) -> io::Result<()> {
    validate_pftl_uniswap_launch_config(launch_config)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    if launch_config.route_id != ledger.route_id
        || launch_config.route_config_digest != ledger.route_config_digest
        || launch_config.route_trust_class != ledger.route_trust_class
        || launch_config.native_nav_asset_id != ledger.native_nav_asset_id
        || launch_config.settlement_asset_id != ledger.settlement_asset_id
        || launch_config.wrapped_navcoin_token != ledger.wrapped_navcoin_token
        || launch_config.handoff_controller != ledger.handoff_controller
        || launch_config.settlement_adapter != ledger.settlement_adapter
        || launch_config.official_uniswap.chain_id != ledger.ethereum_chain_id
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "PFTL-to-Uniswap launch config does not match persisted bridge ledger route fields",
        ));
    }
    Ok(())
}

fn validate_navcoin_bridge_route_id(route_id: &str) -> io::Result<()> {
    if route_id.is_empty() || route_id.len() > MAX_TEXT_FIELD_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "navcoin bridge route_id must be nonempty bounded text",
        ));
    }
    Ok(())
}
