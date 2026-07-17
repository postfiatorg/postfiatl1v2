fn nav_roundtrip_live_demo_nav_exit(
    options: NavRoundtripNavExitOptions,
) -> Result<NavRoundtripNavExitReport, String> {
    std::fs::create_dir_all(&options.artifact_dir).map_err(|error| {
        format!(
            "failed to create NAV roundtrip artifact dir `{}`: {error}",
            options.artifact_dir.display()
        )
    })?;
    let artifact_file = options.artifact_dir.join("nav-exit.json");
    if options.resume && artifact_file.is_file() {
        let raw = std::fs::read_to_string(&artifact_file).map_err(|error| {
            format!(
                "failed to read existing NAV exit artifact `{}`: {error}",
                artifact_file.display()
            )
        })?;
        return serde_json::from_str::<NavRoundtripNavExitReport>(&raw).map_err(|error| {
            format!(
                "existing NAV exit artifact `{}` is not a NAV roundtrip NAV exit report: {error}",
                artifact_file.display()
            )
        });
    }
    if artifact_file.exists() && !options.overwrite {
        return Err(format!(
            "NAV exit artifact `{}` already exists; use --resume, --overwrite, or a new artifact dir",
            artifact_file.display()
        ));
    }

    let primary_raw = std::fs::read_to_string(&options.primary_mint_report_file).map_err(|error| {
        format!(
            "failed to read primary mint report `{}`: {error}",
            options.primary_mint_report_file.display()
        )
    })?;
    let primary_report = serde_json::from_str::<NavRoundtripPrimaryMintReport>(&primary_raw)
        .map_err(|error| {
            format!(
                "primary mint report `{}` is invalid: {error}",
                options.primary_mint_report_file.display()
            )
        })?;
    if primary_report.nav_asset_id != options.nav_asset_id {
        return Err(format!(
            "primary mint report NAV asset `{}` does not match --nav-asset `{}`",
            primary_report.nav_asset_id, options.nav_asset_id
        ));
    }
    if primary_report.settlement_asset_id != options.settlement_asset_id {
        return Err(format!(
            "primary mint report settlement asset `{}` does not match --pfusdc `{}`",
            primary_report.settlement_asset_id, options.settlement_asset_id
        ));
    }

    let store = postfiat_storage::NodeStore::new(&options.data_dir);
    let genesis = store
        .read_genesis()
        .map_err(|error| format!("NAV exit read genesis failed: {error}"))?;
    let ledger_before = store
        .read_ledger()
        .map_err(|error| format!("NAV exit read ledger failed: {error}"))?;
    let nav_asset = ledger_before
        .nav_asset(&options.nav_asset_id)
        .ok_or_else(|| format!("missing NAV asset `{}`", options.nav_asset_id))?
        .clone();
    let nav_asset_definition = ledger_before
        .asset_definition(&options.nav_asset_id)
        .ok_or_else(|| format!("missing NAV asset definition `{}`", options.nav_asset_id))?
        .clone();
    let settlement_nav_asset = ledger_before
        .nav_asset(&options.settlement_asset_id)
        .ok_or_else(|| {
            format!(
                "missing settlement NAV asset `{}`",
                options.settlement_asset_id
            )
        })?
        .clone();
    let settlement_asset = ledger_before
        .asset_definition(&options.settlement_asset_id)
        .ok_or_else(|| {
            format!(
                "missing settlement asset definition `{}`",
                options.settlement_asset_id
            )
        })?
        .clone();
    let owner = options
        .owner
        .clone()
        .unwrap_or_else(|| primary_report.subscriber.clone());
    let redeem_amount = options.amount.unwrap_or(primary_report.mint_amount);
    if redeem_amount == 0 {
        return Err("NAV exit amount must be nonzero".to_string());
    }
    let nav_epoch = options.nav_epoch.unwrap_or(nav_asset.finalized_epoch);
    if nav_epoch == 0 {
        return Err(format!(
            "NAV asset `{}` has no finalized epoch; pass --nav-epoch only if this is intentional",
            options.nav_asset_id
        ));
    }
    let nav_reserve_packet_hash = options
        .nav_reserve_packet_hash
        .clone()
        .unwrap_or_else(|| nav_asset.finalized_reserve_packet_hash.clone());
    if nav_reserve_packet_hash.is_empty() {
        return Err(format!(
            "NAV asset `{}` has no finalized reserve packet hash; pass --nav-reserve-packet-hash",
            options.nav_asset_id
        ));
    }
    let settlement_amount_atoms = match options.settlement_amount_atoms {
        Some(value) => value,
        None => nav_roundtrip_required_vault_bridge_settlement_atoms(
            redeem_amount,
            nav_asset_definition.precision,
            nav_asset.nav_per_unit,
            &nav_asset.valuation_unit,
            &settlement_nav_asset.valuation_unit,
            settlement_asset.precision,
        )?,
    };

    let nav_balance_before =
        nav_roundtrip_trustline_balance(&ledger_before, &owner, &options.nav_asset_id);
    let settlement_balance_before =
        nav_roundtrip_trustline_balance(&ledger_before, &owner, &options.settlement_asset_id);
    if nav_balance_before.unwrap_or(0) < redeem_amount && options.redemption_id.is_none() {
        return Err(format!(
            "NAV exit owner `{owner}` has {:?} NAV atoms, needs {redeem_amount}",
            nav_balance_before
        ));
    }
    if settlement_balance_before.is_none() {
        return Err(format!(
            "NAV exit owner `{owner}` has no trustline for settlement asset `{}`",
            options.settlement_asset_id
        ));
    }

    let settlement_status_before = vault_bridge_status(postfiat_node::VaultBridgeStatusOptions {
        data_dir: options.data_dir.clone(),
        asset_id: options.settlement_asset_id.clone(),
    })
    .map_err(|error| format!("NAV exit settlement status failed: {error}"))?;

    let redeem_operation = postfiat_types::AssetTransactionOperation::NavRedeemAtNav(
        postfiat_types::NavRedeemAtNavOperation {
            owner: owner.clone(),
            issuer: nav_asset.issuer.clone(),
            asset_id: options.nav_asset_id.clone(),
            amount: redeem_amount,
            epoch: nav_epoch,
            reserve_packet_hash: nav_reserve_packet_hash.clone(),
        },
    );
    redeem_operation
        .validate()
        .map_err(|error| format!("NAV exit redeem operation invalid: {error}"))?;
    let redeem_operation_file = options.artifact_dir.join("nav-redeem-at-nav.operation.json");
    write_json_file(&redeem_operation_file, &redeem_operation)?;
    let redeem_operations_file = options.artifact_dir.join("nav-exit-redeem.certified-ops.json");
    let redeem_request = serde_json::json!({
        "schema": CERTIFIED_ASSET_OPS_REQUEST_SCHEMA,
        "operations": [
            {
                "label": "nav-redeem-at-nav",
                "source": owner.clone(),
                "key_file": options.owner_key_file.display().to_string(),
                "operation": redeem_operation.clone(),
                "dependencies": [],
            },
        ],
    });
    write_json_file(&redeem_operations_file, &redeem_request)?;

    if options.same_round_settlement && options.redemption_id.is_some() {
        return Err(
            "--same-round-nav-exit cannot be used with --redemption-id; same-round settlement must derive the redemption id from the redeem sequence"
                .to_string(),
        );
    }

    let mut redemption_id = options.redemption_id.clone();
    let mut settlement_receipt_hash = None;
    let mut settle_operations_file = None;
    let mut settle_operation_file = None;
    let mut settle_certified_ops_artifact_dir = None;
    let mut settle_certified_ops = None;
    let mut nav_balance_after = None;
    let mut settlement_balance_after = None;
    let mut settlement_status_after = None;
    let mut redeem_certified_ops_artifact_dir =
        options.artifact_dir.join("nav-exit-redeem-certified");

    let redeem_certified_ops = if options.same_round_settlement {
        let redeem_operation_json = serde_json::to_string(&redeem_operation).map_err(|error| {
            format!("NAV exit same-round redeem operation serialization failed: {error}")
        })?;
        let redeem_quote = asset_fee_quote(AssetFeeQuoteOptions {
            data_dir: options.data_dir.clone(),
            source: owner.clone(),
            operation_json: redeem_operation_json,
            sequence: None,
        })
        .map_err(|error| format!("NAV exit same-round redeem quote failed: {error}"))?;
        let same_round_redemption_id = postfiat_types::nav_redemption_id(
            &genesis.chain_id,
            &owner,
            &options.nav_asset_id,
            redeem_quote.sequence,
        )
        .map_err(|error| format!("NAV exit same-round redemption id derivation failed: {error}"))?;
        let receipt_hash = options.settlement_receipt_hash.clone().unwrap_or_else(|| {
            nav_roundtrip_nav_exit_settlement_receipt_hash(
                &genesis.chain_id,
                &options.nav_asset_id,
                &options.settlement_asset_id,
                &same_round_redemption_id,
                &primary_report.settlement_allocation_id,
                settlement_amount_atoms,
            )
        });
        let settle_operation = postfiat_types::AssetTransactionOperation::NavRedeemSettle(
            postfiat_types::NavRedeemSettleOperation {
                issuer: nav_asset.issuer.clone(),
                asset_id: options.nav_asset_id.clone(),
                redemption_id: same_round_redemption_id.clone(),
                settlement_receipt_hash: receipt_hash.clone(),
                settlement_asset_id: options.settlement_asset_id.clone(),
                settlement_bucket_id: primary_report.settlement_bucket_id.clone(),
                settlement_allocation_id: primary_report.settlement_allocation_id.clone(),
                settlement_amount_atoms,
            },
        );
        settle_operation
            .validate()
            .map_err(|error| format!("NAV exit same-round settle operation invalid: {error}"))?;
        let settle_operation_path = options.artifact_dir.join("nav-redeem-settle.operation.json");
        write_json_file(&settle_operation_path, &settle_operation)?;
        let combined_operations_path =
            options.artifact_dir.join("nav-exit-redeem-settle.certified-ops.json");
        let combined_request = serde_json::json!({
            "schema": CERTIFIED_ASSET_OPS_REQUEST_SCHEMA,
            "operations": [
                {
                    "label": "nav-redeem-at-nav",
                    "source": owner.clone(),
                    "key_file": options.owner_key_file.display().to_string(),
                    "operation": redeem_operation.clone(),
                    "dependencies": [],
                },
                {
                    "label": "nav-redeem-settle",
                    "source": nav_asset.issuer.clone(),
                    "key_file": options.issuer_key_file.display().to_string(),
                    "operation": settle_operation,
                    "dependencies": [{
                        "label": "nav-redeem-at-nav",
                        "mode": "same_round",
                        "reason": "settlement consumes a redemption id deterministically derived from the signed redeem sequence",
                    }],
                },
            ],
        });
        write_json_file(&combined_operations_path, &combined_request)?;
        let combined_artifact_dir = options.artifact_dir.join("nav-exit-redeem-settle-certified");
        redeem_certified_ops_artifact_dir = combined_artifact_dir.clone();
        let combined_report = certified_asset_ops_batch(CertifiedAssetOpsBatchOptions {
            data_dir: options.data_dir.clone(),
            topology_file: options.topology_file.clone(),
            key_file: options.validator_key_file.clone(),
            proposal_key_file: options.proposal_key_file.clone(),
            ops_file: combined_operations_path.clone(),
            artifact_dir: combined_artifact_dir.clone(),
            max_transactions: None,
            require_local_proposer: options.require_local_proposer,
            require_signed_proposal: options.require_signed_proposal,
            allow_peer_failures: options.allow_peer_failures,
            quorum_early_full_propagation: options.quorum_early_full_propagation,
            local_apply_before_certified_send: options.local_apply_before_certified_send,
            defer_certified_sends: options.defer_certified_sends,
            block_height: options.block_height,
            view: options.view,
            timeout_certificate_file: options.timeout_certificate_file.clone(),
            timeout_ms: options.timeout_ms,
            send_retries: options.send_retries,
            retry_backoff_ms: options.retry_backoff_ms,
            allow_existing_mempool: options.allow_existing_mempool,
            resume: options.resume,
            overwrite: options.overwrite,
            prepare_only: options.prepare_only,
            batch_only: options.batch_only,
        })?;
        if let Some(actual_sequence) = combined_report
            .operations
            .iter()
            .find(|operation| operation.label == "nav-redeem-at-nav")
            .and_then(|operation| operation.sequence)
        {
            if actual_sequence != redeem_quote.sequence {
                return Err(format!(
                    "NAV exit same-round redeem sequence changed from predicted {} to signed {}; refusing to settle against the wrong redemption id",
                    redeem_quote.sequence, actual_sequence
                ));
            }
        }
        if !options.prepare_only && !options.batch_only {
            let ledger_after_settle = store.read_ledger().map_err(|error| {
                format!("NAV exit same-round read post-settle ledger failed: {error}")
            })?;
            let redemption_after = ledger_after_settle
                .nav_redemption(&same_round_redemption_id)
                .ok_or_else(|| {
                    format!(
                        "NAV exit same-round batch did not create redemption `{same_round_redemption_id}`"
                    )
                })?;
            if redemption_after.state != postfiat_types::NAV_REDEMPTION_STATE_SETTLED {
                return Err(format!(
                    "NAV exit same-round redemption `{same_round_redemption_id}` ended in state `{}`",
                    redemption_after.state
                ));
            }
            nav_balance_after = nav_roundtrip_trustline_balance(
                &ledger_after_settle,
                &owner,
                &options.nav_asset_id,
            );
            settlement_balance_after = nav_roundtrip_trustline_balance(
                &ledger_after_settle,
                &owner,
                &options.settlement_asset_id,
            );
            settlement_status_after = Some(
                vault_bridge_status(postfiat_node::VaultBridgeStatusOptions {
                    data_dir: options.data_dir.clone(),
                    asset_id: options.settlement_asset_id.clone(),
                })
                .map_err(|error| format!("NAV exit settlement final status failed: {error}"))?,
            );
        }
        redemption_id = Some(same_round_redemption_id);
        settlement_receipt_hash = Some(receipt_hash);
        settle_operations_file = Some(combined_operations_path.display().to_string());
        settle_operation_file = Some(settle_operation_path.display().to_string());
        settle_certified_ops_artifact_dir = Some(combined_artifact_dir.display().to_string());
        settle_certified_ops = Some(combined_report.clone());
        combined_report
    } else if options.redemption_id.is_some() {
        certified_asset_ops_batch(CertifiedAssetOpsBatchOptions {
            data_dir: options.data_dir.clone(),
            topology_file: options.topology_file.clone(),
            key_file: options.validator_key_file.clone(),
            proposal_key_file: options.proposal_key_file.clone(),
            ops_file: redeem_operations_file.clone(),
            artifact_dir: redeem_certified_ops_artifact_dir.clone(),
            max_transactions: None,
            require_local_proposer: options.require_local_proposer,
            require_signed_proposal: options.require_signed_proposal,
            allow_peer_failures: options.allow_peer_failures,
            quorum_early_full_propagation: options.quorum_early_full_propagation,
            local_apply_before_certified_send: options.local_apply_before_certified_send,
            defer_certified_sends: options.defer_certified_sends,
            block_height: options.block_height,
            view: options.view,
            timeout_certificate_file: options.timeout_certificate_file.clone(),
            timeout_ms: options.timeout_ms,
            send_retries: options.send_retries,
            retry_backoff_ms: options.retry_backoff_ms,
            allow_existing_mempool: options.allow_existing_mempool,
            resume: true,
            overwrite: options.overwrite,
            prepare_only: true,
            batch_only: false,
        })?
    } else {
        certified_asset_ops_batch(CertifiedAssetOpsBatchOptions {
            data_dir: options.data_dir.clone(),
            topology_file: options.topology_file.clone(),
            key_file: options.validator_key_file.clone(),
            proposal_key_file: options.proposal_key_file.clone(),
            ops_file: redeem_operations_file.clone(),
            artifact_dir: redeem_certified_ops_artifact_dir.clone(),
            max_transactions: None,
            require_local_proposer: options.require_local_proposer,
            require_signed_proposal: options.require_signed_proposal,
            allow_peer_failures: options.allow_peer_failures,
            quorum_early_full_propagation: options.quorum_early_full_propagation,
            local_apply_before_certified_send: options.local_apply_before_certified_send,
            defer_certified_sends: options.defer_certified_sends,
            block_height: options.block_height,
            view: options.view,
            timeout_certificate_file: options.timeout_certificate_file.clone(),
            timeout_ms: options.timeout_ms,
            send_retries: options.send_retries,
            retry_backoff_ms: options.retry_backoff_ms,
            allow_existing_mempool: options.allow_existing_mempool,
            resume: options.resume,
            overwrite: options.overwrite,
            prepare_only: options.prepare_only,
            batch_only: options.batch_only,
        })?
    };

    if !options.same_round_settlement && !options.prepare_only && !options.batch_only {
        let ledger_after_redeem = store
            .read_ledger()
            .map_err(|error| format!("NAV exit read post-redeem ledger failed: {error}"))?;
        if redemption_id.is_none() {
            let sequence = redeem_certified_ops
                .operations
                .first()
                .and_then(|operation| operation.sequence)
                .ok_or_else(|| "NAV exit redeem report did not include a sequence".to_string())?;
            let expected = postfiat_types::nav_redemption_id(
                &genesis.chain_id,
                &owner,
                &options.nav_asset_id,
                sequence,
            )
            .map_err(|error| format!("NAV exit redemption id derivation failed: {error}"))?;
            redemption_id = Some(expected);
        }
        let redemption_id_value = redemption_id.clone().ok_or_else(|| {
            "NAV exit could not determine redemption id after redeem submission".to_string()
        })?;
        let redemption = ledger_after_redeem
            .nav_redemption(&redemption_id_value)
            .cloned()
            .or_else(|| {
                nav_roundtrip_find_matching_redemption(
                    &ledger_after_redeem,
                    &owner,
                    &options.nav_asset_id,
                    redeem_amount,
                    nav_epoch,
                    &nav_reserve_packet_hash,
                )
            })
            .ok_or_else(|| {
                format!(
                    "NAV exit redeem did not create a matching redemption `{redemption_id_value}`"
                )
            })?;
        redemption_id = Some(redemption.redemption_id.clone());
        let receipt_hash = options.settlement_receipt_hash.clone().unwrap_or_else(|| {
            nav_roundtrip_nav_exit_settlement_receipt_hash(
                &genesis.chain_id,
                &options.nav_asset_id,
                &options.settlement_asset_id,
                &redemption.redemption_id,
                &primary_report.settlement_allocation_id,
                settlement_amount_atoms,
            )
        });
        let settle_operation = postfiat_types::AssetTransactionOperation::NavRedeemSettle(
            postfiat_types::NavRedeemSettleOperation {
                issuer: nav_asset.issuer.clone(),
                asset_id: options.nav_asset_id.clone(),
                redemption_id: redemption.redemption_id.clone(),
                settlement_receipt_hash: receipt_hash.clone(),
                settlement_asset_id: options.settlement_asset_id.clone(),
                settlement_bucket_id: primary_report.settlement_bucket_id.clone(),
                settlement_allocation_id: primary_report.settlement_allocation_id.clone(),
                settlement_amount_atoms,
            },
        );
        settle_operation
            .validate()
            .map_err(|error| format!("NAV exit settle operation invalid: {error}"))?;
        let settle_operation_path = options.artifact_dir.join("nav-redeem-settle.operation.json");
        write_json_file(&settle_operation_path, &settle_operation)?;
        let settle_operations_path = options.artifact_dir.join("nav-exit-settle.certified-ops.json");
        let settle_request = serde_json::json!({
            "schema": CERTIFIED_ASSET_OPS_REQUEST_SCHEMA,
            "operations": [
                {
                    "label": "nav-redeem-settle",
                    "source": nav_asset.issuer.clone(),
                    "key_file": options.issuer_key_file.display().to_string(),
                    "operation": settle_operation,
                    "dependencies": [{
                        "label": "nav-redeem-at-nav",
                        "mode": "prior_round",
                        "reason": "settlement consumes the redemption created by the prior certified redeem round",
                    }],
                },
            ],
        });
        write_json_file(&settle_operations_path, &settle_request)?;
        let settle_artifact_dir = options.artifact_dir.join("nav-exit-settle-certified");
        let settle_report = certified_asset_ops_batch(CertifiedAssetOpsBatchOptions {
            data_dir: options.data_dir.clone(),
            topology_file: options.topology_file.clone(),
            key_file: options.validator_key_file.clone(),
            proposal_key_file: options.proposal_key_file.clone(),
            ops_file: settle_operations_path.clone(),
            artifact_dir: settle_artifact_dir.clone(),
            max_transactions: None,
            require_local_proposer: options.require_local_proposer,
            require_signed_proposal: options.require_signed_proposal,
            allow_peer_failures: options.allow_peer_failures,
            quorum_early_full_propagation: options.quorum_early_full_propagation,
            local_apply_before_certified_send: options.local_apply_before_certified_send,
            defer_certified_sends: options.defer_certified_sends,
            block_height: options.block_height,
            view: options.view,
            timeout_certificate_file: options.timeout_certificate_file.clone(),
            timeout_ms: options.timeout_ms,
            send_retries: options.send_retries,
            retry_backoff_ms: options.retry_backoff_ms,
            allow_existing_mempool: options.allow_existing_mempool,
            resume: options.resume,
            overwrite: options.overwrite,
            prepare_only: false,
            batch_only: false,
        })?;
        let ledger_after_settle = store
            .read_ledger()
            .map_err(|error| format!("NAV exit read post-settle ledger failed: {error}"))?;
        nav_balance_after =
            nav_roundtrip_trustline_balance(&ledger_after_settle, &owner, &options.nav_asset_id);
        settlement_balance_after = nav_roundtrip_trustline_balance(
            &ledger_after_settle,
            &owner,
            &options.settlement_asset_id,
        );
        settlement_status_after = Some(
            vault_bridge_status(postfiat_node::VaultBridgeStatusOptions {
                data_dir: options.data_dir.clone(),
                asset_id: options.settlement_asset_id.clone(),
            })
            .map_err(|error| format!("NAV exit settlement final status failed: {error}"))?,
        );
        settlement_receipt_hash = Some(receipt_hash);
        settle_operations_file = Some(settle_operations_path.display().to_string());
        settle_operation_file = Some(settle_operation_path.display().to_string());
        settle_certified_ops_artifact_dir = Some(settle_artifact_dir.display().to_string());
        settle_certified_ops = Some(settle_report);
    }

    let report = NavRoundtripNavExitReport {
        schema: NAV_ROUNDTRIP_NAV_EXIT_REPORT_SCHEMA.to_string(),
        artifact_file: artifact_file.display().to_string(),
        primary_mint_report_file: options.primary_mint_report_file.display().to_string(),
        nav_asset_id: options.nav_asset_id,
        settlement_asset_id: options.settlement_asset_id,
        owner,
        issuer: nav_asset.issuer,
        nav_epoch,
        nav_reserve_packet_hash,
        redeem_amount,
        settlement_amount_atoms,
        settlement_bucket_id: primary_report.settlement_bucket_id,
        settlement_allocation_id: primary_report.settlement_allocation_id,
        settlement_receipt_hash,
        redemption_id,
        same_round_settlement: options.same_round_settlement,
        nav_balance_before,
        nav_balance_after,
        settlement_balance_before,
        settlement_balance_after,
        settlement_status_before,
        settlement_status_after,
        redeem_operations_file: redeem_operations_file.display().to_string(),
        redeem_operation_file: redeem_operation_file.display().to_string(),
        redeem_certified_ops_artifact_dir: redeem_certified_ops_artifact_dir.display().to_string(),
        redeem_certified_ops,
        settle_operations_file,
        settle_operation_file,
        settle_certified_ops_artifact_dir,
        settle_certified_ops,
    };
    write_json_file(&artifact_file, &report)?;
    Ok(report)
}

fn nav_roundtrip_live_demo_burn_to_redeem(
    options: NavRoundtripBurnToRedeemOptions,
) -> Result<NavRoundtripBurnToRedeemReport, String> {
    std::fs::create_dir_all(&options.artifact_dir).map_err(|error| {
        format!(
            "failed to create NAV roundtrip artifact dir `{}`: {error}",
            options.artifact_dir.display()
        )
    })?;
    let artifact_file = options.artifact_dir.join("burn-to-redeem.json");
    if options.resume && artifact_file.is_file() {
        let raw = std::fs::read_to_string(&artifact_file).map_err(|error| {
            format!(
                "failed to read existing burn-to-redeem artifact `{}`: {error}",
                artifact_file.display()
            )
        })?;
        return serde_json::from_str::<NavRoundtripBurnToRedeemReport>(&raw).map_err(|error| {
            format!(
                "existing burn-to-redeem artifact `{}` is not a NAV roundtrip burn-to-redeem report: {error}",
                artifact_file.display()
            )
        });
    }
    if artifact_file.exists() && !options.overwrite {
        return Err(format!(
            "burn-to-redeem artifact `{}` already exists; use --resume, --overwrite, or a new artifact dir",
            artifact_file.display()
        ));
    }

    let nav_exit_raw = std::fs::read_to_string(&options.nav_exit_report_file).map_err(|error| {
        format!(
            "failed to read NAV exit report `{}`: {error}",
            options.nav_exit_report_file.display()
        )
    })?;
    let nav_exit_report = serde_json::from_str::<NavRoundtripNavExitReport>(&nav_exit_raw)
        .map_err(|error| {
            format!(
                "NAV exit report `{}` is invalid: {error}",
                options.nav_exit_report_file.display()
            )
        })?;
    if nav_exit_report.settlement_asset_id != options.settlement_asset_id {
        return Err(format!(
            "NAV exit report settlement asset `{}` does not match --pfusdc `{}`",
            nav_exit_report.settlement_asset_id, options.settlement_asset_id
        ));
    }

    let store = postfiat_storage::NodeStore::new(&options.data_dir);
    let genesis = store
        .read_genesis()
        .map_err(|error| format!("burn-to-redeem read genesis failed: {error}"))?;
    let ledger_before = store
        .read_ledger()
        .map_err(|error| format!("burn-to-redeem read ledger failed: {error}"))?;
    let owner = options.owner.clone().unwrap_or(nav_exit_report.owner);
    let amount_atoms = options
        .amount_atoms
        .unwrap_or(nav_exit_report.settlement_amount_atoms);
    if amount_atoms == 0 {
        return Err("burn-to-redeem amount must be nonzero".to_string());
    }
    let owner_balance_before =
        nav_roundtrip_trustline_balance(&ledger_before, &owner, &options.settlement_asset_id);
    if owner_balance_before.unwrap_or(0) < amount_atoms {
        return Err(format!(
            "burn-to-redeem owner `{owner}` has {:?} settlement atoms, needs {amount_atoms}",
            owner_balance_before
        ));
    }
    let settlement_status_before = vault_bridge_status(postfiat_node::VaultBridgeStatusOptions {
        data_dir: options.data_dir.clone(),
        asset_id: options.settlement_asset_id.clone(),
    })
    .map_err(|error| format!("burn-to-redeem settlement status failed: {error}"))?;

    let bundle_dir = options.artifact_dir.join("burn-to-redeem-bundle");
    let bundle = vault_bridge_burn_to_redeem_bundle(postfiat_node::VaultBridgeBurnToRedeemBundleOptions {
        data_dir: options.data_dir.clone(),
        owner: owner.clone(),
        issuer: options.issuer.clone(),
        asset_id: options.settlement_asset_id.clone(),
        bucket_id: options.bucket_id.clone(),
        amount_atoms,
        epoch: options.epoch,
        reserve_packet_hash: options.reserve_packet_hash.clone(),
        destination_ref: options.destination_ref.clone(),
        bundle_dir: bundle_dir.clone(),
        overwrite: options.overwrite,
    })
    .map_err(|error| format!("burn-to-redeem bundle build failed: {error}"))?;

    let certified_ops_file = options.artifact_dir.join("burn-to-redeem.certified-ops.json");
    let adapter_report = certified_asset_ops_from_bundle(CertifiedAssetOpsFromBundleOptions {
        bundle_dir: bundle_dir.clone(),
        output_file: certified_ops_file.clone(),
        proposer_key_file: None,
        attestor_key_file: None,
        finalizer_key_file: None,
        claimer_key_file: None,
        owner_key_file: Some(options.owner_key_file.clone()),
        include_deposit_claim: true,
        overwrite: options.overwrite,
    })?;
    if adapter_report.operation_count != 1 {
        return Err(format!(
            "burn-to-redeem bundle adapter produced {} operations, expected 1",
            adapter_report.operation_count
        ));
    }

    let certified_ops_artifact_dir = options.artifact_dir.join("burn-to-redeem-certified");
    let certified_ops = certified_asset_ops_batch(CertifiedAssetOpsBatchOptions {
        data_dir: options.data_dir.clone(),
        topology_file: options.topology_file.clone(),
        key_file: options.validator_key_file.clone(),
        proposal_key_file: options.proposal_key_file.clone(),
        ops_file: certified_ops_file.clone(),
        artifact_dir: certified_ops_artifact_dir.clone(),
        max_transactions: None,
        require_local_proposer: options.require_local_proposer,
        require_signed_proposal: options.require_signed_proposal,
        allow_peer_failures: options.allow_peer_failures,
        quorum_early_full_propagation: options.quorum_early_full_propagation,
        local_apply_before_certified_send: options.local_apply_before_certified_send,
        defer_certified_sends: options.defer_certified_sends,
        block_height: options.block_height,
        view: options.view,
        timeout_certificate_file: options.timeout_certificate_file.clone(),
        timeout_ms: options.timeout_ms,
        send_retries: options.send_retries,
        retry_backoff_ms: options.retry_backoff_ms,
        allow_existing_mempool: options.allow_existing_mempool,
        resume: options.resume,
        overwrite: options.overwrite,
        prepare_only: options.prepare_only,
        batch_only: options.batch_only,
    })?;

    let mut redemption_id = None;
    let mut owner_balance_after = None;
    let mut settlement_status_after = None;
    if !options.prepare_only && !options.batch_only {
        let sequence = certified_ops
            .operations
            .first()
            .and_then(|operation| operation.sequence)
            .ok_or_else(|| "burn-to-redeem report did not include a sequence".to_string())?;
        let expected_redemption_id = postfiat_types::vault_bridge_redemption_id(
            &genesis.chain_id,
            &owner,
            &options.settlement_asset_id,
            sequence,
        )
        .map_err(|error| format!("burn-to-redeem redemption id derivation failed: {error}"))?;
        let ledger_after = store
            .read_ledger()
            .map_err(|error| format!("burn-to-redeem read final ledger failed: {error}"))?;
        let redemption = ledger_after
            .vault_bridge_redemption(&expected_redemption_id)
            .ok_or_else(|| {
                format!("burn-to-redeem did not create redemption `{expected_redemption_id}`")
            })?;
        if redemption.amount_atoms != amount_atoms || redemption.destination_ref != options.destination_ref {
            return Err("burn-to-redeem redemption does not match requested amount/destination".to_string());
        }
        redemption_id = Some(expected_redemption_id);
        owner_balance_after =
            nav_roundtrip_trustline_balance(&ledger_after, &owner, &options.settlement_asset_id);
        settlement_status_after = Some(
            vault_bridge_status(postfiat_node::VaultBridgeStatusOptions {
                data_dir: options.data_dir.clone(),
                asset_id: options.settlement_asset_id.clone(),
            })
            .map_err(|error| format!("burn-to-redeem settlement final status failed: {error}"))?,
        );
    }

    let report = NavRoundtripBurnToRedeemReport {
        schema: NAV_ROUNDTRIP_BURN_TO_REDEEM_REPORT_SCHEMA.to_string(),
        artifact_file: artifact_file.display().to_string(),
        nav_exit_report_file: options.nav_exit_report_file.display().to_string(),
        settlement_asset_id: options.settlement_asset_id,
        owner,
        amount_atoms,
        destination_ref: options.destination_ref,
        owner_balance_before,
        owner_balance_after,
        redemption_id,
        settlement_status_before,
        settlement_status_after,
        bundle_dir: bundle_dir.display().to_string(),
        bundle,
        certified_ops_file: certified_ops_file.display().to_string(),
        certified_ops_artifact_dir: certified_ops_artifact_dir.display().to_string(),
        certified_ops,
    };
    write_json_file(&artifact_file, &report)?;
    Ok(report)
}

fn nav_roundtrip_live_demo_evm_withdrawal(
    options: NavRoundtripEvmWithdrawalOptions,
) -> Result<NavRoundtripEvmWithdrawalReport, String> {
    if options.signatures_file.is_some() && options.withdrawal_signer_key_file.is_some() {
        return Err("use only one of --signatures-file or --withdrawal-signer-key-file".to_string());
    }
    std::fs::create_dir_all(&options.artifact_dir).map_err(|error| {
        format!(
            "failed to create NAV roundtrip artifact dir `{}`: {error}",
            options.artifact_dir.display()
        )
    })?;
    let artifact_file = options.artifact_dir.join("evm-withdrawal.json");
    if options.resume && artifact_file.is_file() {
        let raw = std::fs::read_to_string(&artifact_file).map_err(|error| {
            format!(
                "failed to read existing EVM withdrawal artifact `{}`: {error}",
                artifact_file.display()
            )
        })?;
        return serde_json::from_str::<NavRoundtripEvmWithdrawalReport>(&raw).map_err(|error| {
            format!(
                "existing EVM withdrawal artifact `{}` is not a NAV roundtrip withdrawal report: {error}",
                artifact_file.display()
            )
        });
    }
    if artifact_file.exists() && !options.overwrite {
        return Err(format!(
            "EVM withdrawal artifact `{}` already exists; use --resume, --overwrite, or a new artifact dir",
            artifact_file.display()
        ));
    }

    let burn_raw = std::fs::read_to_string(&options.burn_to_redeem_report_file).map_err(|error| {
        format!(
            "failed to read burn-to-redeem report `{}`: {error}",
            options.burn_to_redeem_report_file.display()
        )
    })?;
    let burn_report = serde_json::from_str::<NavRoundtripBurnToRedeemReport>(&burn_raw)
        .map_err(|error| {
            format!(
                "burn-to-redeem report `{}` is invalid: {error}",
                options.burn_to_redeem_report_file.display()
            )
        })?;
    if burn_report.settlement_asset_id != options.settlement_asset_id {
        return Err(format!(
            "burn-to-redeem report settlement asset `{}` does not match --pfusdc `{}`",
            burn_report.settlement_asset_id, options.settlement_asset_id
        ));
    }
    let redemption_id = options
        .redemption_id
        .clone()
        .or(burn_report.redemption_id.clone())
        .ok_or_else(|| {
            format!(
                "burn-to-redeem report `{}` has no redemption_id; rerun burn-to-redeem live or pass --redemption-id",
                options.burn_to_redeem_report_file.display()
            )
        })?;
    let signatures_file = match options.signatures_file.clone() {
        Some(path) => path,
        None => {
            let bundle_dir = options.artifact_dir.join("withdrawal-signature-request");
            let signature_bundle = vault_bridge_withdrawal_signature_bundle(
                VaultBridgeWithdrawalSignatureBundleOptions {
                    plan_options: VaultBridgeWithdrawalPlanOptions {
                        data_dir: options.data_dir.clone(),
                        asset_id: options.settlement_asset_id.clone(),
                        redemption_id: redemption_id.clone(),
                        pftl_finalized_height: options.pftl_finalized_height,
                        evm_chain_id: Some(options.source_chain_id),
                        verifier_address: Some(options.verifier_address.clone()),
                        signatures_file: None,
                    },
                    bundle_dir: bundle_dir.clone(),
                    relay_bundle_dir: None,
                    overwrite: options.overwrite,
                },
            )
            .map_err(|error| {
                format!(
                    "failed to create withdrawal signature request bundle `{}`: {error}",
                    bundle_dir.display()
                )
            })?;
            let Some(signer_key_file) = options.withdrawal_signer_key_file.as_ref() else {
                return Err(format!(
                    "EVM withdrawal needs verifier signatures; signature request written to `{}` and empty signatures file to `{}`. Rerun with --signatures-file {}, or pass --withdrawal-signer-key-file PATH",
                    signature_bundle.signature_request_file,
                    signature_bundle.signatures_file,
                    signature_bundle.signatures_file
                ));
            };
            nav_roundtrip_align_withdrawal_signature_request_with_live_abi(
                &options.cast_binary,
                &options.source_rpc_url,
                &options.vault_address,
                &options.verifier_address,
                &options.usdc_address,
                &options.stakehub_wallet,
                std::path::Path::new(&signature_bundle.plan_file),
                std::path::Path::new(&signature_bundle.signature_request_file),
            )?;
            let auto_signature = nav_roundtrip_auto_sign_withdrawal_bundle(
                std::path::Path::new(&signature_bundle.signature_request_file),
                std::path::Path::new(&signature_bundle.signatures_file),
                signer_key_file,
            )?;
            nav_roundtrip_require_verifier_signer(
                &options.cast_binary,
                &options.source_rpc_url,
                &options.verifier_address,
                &auto_signature.signer_address,
            )?;
            let auto_report_file = bundle_dir.join("auto-signature.json");
            write_json_file(&auto_report_file, &auto_signature)?;
            std::path::PathBuf::from(signature_bundle.signatures_file)
        }
    };
    let signatures = nav_roundtrip_read_evm_signatures(&signatures_file)?;
    if signatures.is_empty() {
        return Err(format!(
            "withdrawal signatures file `{}` is empty",
            signatures_file.display()
        ));
    }

    let plan = vault_bridge_withdrawal_plan(VaultBridgeWithdrawalPlanOptions {
        data_dir: options.data_dir.clone(),
        asset_id: options.settlement_asset_id.clone(),
        redemption_id: redemption_id.clone(),
        pftl_finalized_height: options.pftl_finalized_height,
        evm_chain_id: Some(options.source_chain_id),
        verifier_address: Some(options.verifier_address.clone()),
        signatures_file: Some(signatures_file.clone()),
    })
    .map_err(|error| format!("EVM withdrawal plan failed: {error}"))?;
    if plan.withdrawal_packet.recipient.to_ascii_lowercase()
        != options.stakehub_wallet.to_ascii_lowercase()
    {
        return Err(format!(
            "withdrawal recipient `{}` does not match --stakehub-wallet `{}`",
            plan.withdrawal_packet.recipient, options.stakehub_wallet
        ));
    }
    if plan.withdrawal_packet.amount_atoms != burn_report.amount_atoms {
        return Err(format!(
            "withdrawal plan amount {} does not match burn-to-redeem amount {}",
            plan.withdrawal_packet.amount_atoms, burn_report.amount_atoms
        ));
    }

    let bridge_abi = classify_nav_roundtrip_vault_abi(
        &options.cast_binary,
        &options.source_rpc_url,
        &options.vault_address,
        &options.usdc_address,
        &options.stakehub_wallet,
    )?;
    if bridge_abi.bridge_class == NAV_ROUNDTRIP_BRIDGE_CLASS_UNKNOWN {
        return Err("vault withdrawal ABI is unknown; cannot relay EVM withdrawal".to_string());
    }

    let call_plan = nav_roundtrip_evm_withdrawal_call_plan(
        &options.cast_binary,
        &options.source_rpc_url,
        &options.vault_address,
        &options.verifier_address,
        &plan,
        &bridge_abi,
        &signatures,
    )?;

    let submit_proof_data = cast_calldata(
        &options.cast_binary,
        &call_plan.verifier_submit_proof_signature,
        &[
            call_plan.withdrawal_packet_digest.as_str(),
            call_plan.pftl_withdrawal_hash_commitment.as_str(),
            &call_plan.pftl_finalized_height.to_string(),
            call_plan.signatures_arg.as_str(),
        ],
    )?;
    let finalize_proof_data = cast_calldata(
        &options.cast_binary,
        "finalizeProof(bytes32)",
        &[call_plan.verifier_pending_proof_id.as_str()],
    )?;
    let submit_withdrawal_data = cast_calldata(
        &options.cast_binary,
        &call_plan.vault_submit_withdrawal_signature,
        &[
            call_plan.withdrawal_packet_tuple_arg.as_str(),
            call_plan.pftl_withdrawal_hash.as_str(),
        ],
    )?;
    let finalize_withdrawal_data = cast_calldata(
        &options.cast_binary,
        "finalizeWithdrawal(bytes32)",
        &[call_plan.vault_pending_withdrawal_id.as_str()],
    )?;
    let claim_withdrawal_data = cast_calldata(
        &options.cast_binary,
        "claimWithdrawal(bytes32)",
        &[call_plan.vault_pending_withdrawal_id.as_str()],
    )?;

    let submit_proof_calldata_file = options.artifact_dir.join("submit-proof.calldata.txt");
    let finalize_proof_calldata_file = options.artifact_dir.join("finalize-proof.calldata.txt");
    let submit_withdrawal_calldata_file = options.artifact_dir.join("submit-withdrawal.calldata.txt");
    let finalize_withdrawal_calldata_file = options.artifact_dir.join("finalize-withdrawal.calldata.txt");
    let claim_withdrawal_calldata_file = options.artifact_dir.join("claim-withdrawal.calldata.txt");
    write_text_file(&submit_proof_calldata_file, &submit_proof_data)?;
    write_text_file(&finalize_proof_calldata_file, &finalize_proof_data)?;
    write_text_file(&submit_withdrawal_calldata_file, &submit_withdrawal_data)?;
    write_text_file(&finalize_withdrawal_calldata_file, &finalize_withdrawal_data)?;
    write_text_file(&claim_withdrawal_calldata_file, &claim_withdrawal_data)?;

    let status_response = stakehub_agent_call(
        &options.stakehub_home,
        &serde_json::json!({ "op": "status" }),
        options.agent_timeout_secs,
    )?;
    require_agent_ok(&status_response, "status")?;
    if status_response
        .get("unlocked")
        .and_then(serde_json::Value::as_bool)
        != Some(true)
    {
        return Err("StakeHub agent is locked; run `stakehub agent unlock` first".to_string());
    }

    let wallet_usdc_before = cast_u128_call(
        &options.cast_binary,
        &options.source_rpc_url,
        &options.usdc_address,
        "balanceOf(address)(uint256)",
        &[options.stakehub_wallet.as_str()],
    )?;
    let vault_usdc_before = cast_u128_call(
        &options.cast_binary,
        &options.source_rpc_url,
        &options.usdc_address,
        "balanceOf(address)(uint256)",
        &[options.vault_address.as_str()],
    )?;
    if vault_usdc_before < u128::from(plan.withdrawal_packet.amount_atoms) {
        return Err(format!(
            "vault has {} USDC atoms, needs {}",
            vault_usdc_before, plan.withdrawal_packet.amount_atoms
        ));
    }

    let verifier_delay = cast_optional_u64_call(
        &options.cast_binary,
        &options.source_rpc_url,
        &options.verifier_address,
        "challenge_delay()(uint64)",
        &[],
    )?
    .ok_or_else(|| "verifier challenge_delay() is unavailable".to_string())?;
    let vault_delay = cast_optional_u64_call(
        &options.cast_binary,
        &options.source_rpc_url,
        &options.vault_address,
        "challenge_delay()(uint64)",
        &[],
    )?
    .ok_or_else(|| "vault challenge_delay() is unavailable".to_string())?;
    let verifier_challenge_wait_secs = options
        .challenge_wait_secs
        .unwrap_or_else(|| verifier_delay.saturating_add(1));
    let vault_challenge_wait_secs = options
        .challenge_wait_secs
        .unwrap_or_else(|| vault_delay.saturating_add(1));

    let agent_open_session_file = options.artifact_dir.join("agent-open-session.json");
    let open_response = if options.launch_session_managed_externally {
        serde_json::json!({
            "ok": true,
            "skipped": true,
            "reason": "launch_session_managed_by_full_runner",
            "session_id": options.session_id,
        })
    } else {
        let close_existing = stakehub_agent_call(
            &options.stakehub_home,
            &serde_json::json!({
                "op": "close_launch_session",
                "session_id": options.session_id,
            }),
            options.agent_timeout_secs,
        )?;
        require_agent_ok(&close_existing, "close existing launch session")?;

        let open_request = serde_json::json!({
            "op": "open_launch_session",
            "session_id": options.session_id,
            "chain_id": options.source_chain_id,
            "allowlist": [
                options.stakehub_wallet,
                options.verifier_address,
                options.vault_address,
                options.usdc_address,
            ],
            "expected_deploys": [{
                "label": "nav_roundtrip_noop_deploy",
                "bytecode_hash": "0x0000000000000000000000000000000000000000000000000000000000000000",
                "bytecode_len": 1,
            }],
            "usdc_address": options.usdc_address,
            "usdc_budget": 0,
            "close_after_action": "claim-withdrawal",
            "ttl_seconds": 1800,
        });
        let response = stakehub_agent_call(
            &options.stakehub_home,
            &open_request,
            options.agent_timeout_secs,
        )?;
        require_agent_ok(&response, "open launch session")?;
        response
    };
    write_json_file(&agent_open_session_file, &open_response)?;

    let source_rpc_provider_class =
        nav_roundtrip_source_rpc_provider_class(&options.source_rpc_url);
    let mut receipt_watches = Vec::new();
    let submit_proof_start = std::time::Instant::now();
    let submit_proof_response = nav_roundtrip_agent_evm_tx(
        &options,
        &options.verifier_address,
        &submit_proof_data,
        "NAV roundtrip submit withdrawal proof",
        "submit-proof",
    )?;
    receipt_watches.push(nav_roundtrip_evm_receipt_watch(
        "submit-proof",
        &submit_proof_response,
        &source_rpc_provider_class,
        monotonic_elapsed_ms(submit_proof_start),
    )?);
    let agent_submit_proof_file = options.artifact_dir.join("agent-submit-proof.json");
    write_json_file(&agent_submit_proof_file, &submit_proof_response)?;
    if verifier_challenge_wait_secs > 0 {
        std::thread::sleep(std::time::Duration::from_secs(verifier_challenge_wait_secs));
    }

    let finalize_proof_start = std::time::Instant::now();
    let finalize_proof_response = nav_roundtrip_agent_evm_tx(
        &options,
        &options.verifier_address,
        &finalize_proof_data,
        "NAV roundtrip finalize withdrawal proof",
        "finalize-proof",
    )?;
    receipt_watches.push(nav_roundtrip_evm_receipt_watch(
        "finalize-proof",
        &finalize_proof_response,
        &source_rpc_provider_class,
        monotonic_elapsed_ms(finalize_proof_start),
    )?);
    let agent_finalize_proof_file = options.artifact_dir.join("agent-finalize-proof.json");
    write_json_file(&agent_finalize_proof_file, &finalize_proof_response)?;

    let submit_withdrawal_start = std::time::Instant::now();
    let submit_withdrawal_response = nav_roundtrip_agent_evm_tx(
        &options,
        &options.vault_address,
        &submit_withdrawal_data,
        "NAV roundtrip submit withdrawal",
        "submit-withdrawal",
    )?;
    receipt_watches.push(nav_roundtrip_evm_receipt_watch(
        "submit-withdrawal",
        &submit_withdrawal_response,
        &source_rpc_provider_class,
        monotonic_elapsed_ms(submit_withdrawal_start),
    )?);
    let agent_submit_withdrawal_file = options.artifact_dir.join("agent-submit-withdrawal.json");
    write_json_file(&agent_submit_withdrawal_file, &submit_withdrawal_response)?;
    if vault_challenge_wait_secs > 0 {
        std::thread::sleep(std::time::Duration::from_secs(vault_challenge_wait_secs));
    }

    let finalize_withdrawal_start = std::time::Instant::now();
    let finalize_withdrawal_response = nav_roundtrip_agent_evm_tx(
        &options,
        &options.vault_address,
        &finalize_withdrawal_data,
        "NAV roundtrip finalize withdrawal",
        "finalize-withdrawal",
    )?;
    receipt_watches.push(nav_roundtrip_evm_receipt_watch(
        "finalize-withdrawal",
        &finalize_withdrawal_response,
        &source_rpc_provider_class,
        monotonic_elapsed_ms(finalize_withdrawal_start),
    )?);
    let agent_finalize_withdrawal_file = options.artifact_dir.join("agent-finalize-withdrawal.json");
    write_json_file(&agent_finalize_withdrawal_file, &finalize_withdrawal_response)?;

    let claim_withdrawal_start = std::time::Instant::now();
    let claim_withdrawal_response = nav_roundtrip_agent_evm_tx(
        &options,
        &options.vault_address,
        &claim_withdrawal_data,
        "NAV roundtrip claim withdrawal",
        "claim-withdrawal",
    )?;
    receipt_watches.push(nav_roundtrip_evm_receipt_watch(
        "claim-withdrawal",
        &claim_withdrawal_response,
        &source_rpc_provider_class,
        monotonic_elapsed_ms(claim_withdrawal_start),
    )?);
    let agent_claim_withdrawal_file = options.artifact_dir.join("agent-claim-withdrawal.json");
    write_json_file(&agent_claim_withdrawal_file, &claim_withdrawal_response)?;

    let agent_close_session_file = options.artifact_dir.join("agent-close-session.json");
    let close_response = if options.launch_session_managed_externally {
        serde_json::json!({
            "ok": true,
            "skipped": true,
            "reason": "launch_session_managed_by_full_runner",
            "session_id": options.session_id,
        })
    } else {
        let response = stakehub_agent_call(
            &options.stakehub_home,
            &serde_json::json!({
                "op": "close_launch_session",
                "session_id": options.session_id,
            }),
            options.agent_timeout_secs,
        )?;
        require_agent_ok(&response, "close launch session")?;
        response
    };
    write_json_file(&agent_close_session_file, &close_response)?;

    let wallet_usdc_after = cast_u128_call(
        &options.cast_binary,
        &options.source_rpc_url,
        &options.usdc_address,
        "balanceOf(address)(uint256)",
        &[options.stakehub_wallet.as_str()],
    )?;
    let vault_usdc_after = cast_u128_call(
        &options.cast_binary,
        &options.source_rpc_url,
        &options.usdc_address,
        "balanceOf(address)(uint256)",
        &[options.vault_address.as_str()],
    )?;

    let amount = u128::from(plan.withdrawal_packet.amount_atoms);
    let mut failure_reasons = Vec::new();
    if wallet_usdc_after.saturating_sub(wallet_usdc_before) != amount {
        failure_reasons.push(format!(
            "wallet USDC delta was {}, expected {}",
            wallet_usdc_after.saturating_sub(wallet_usdc_before),
            amount
        ));
    }
    if vault_usdc_before.saturating_sub(vault_usdc_after) != amount {
        failure_reasons.push(format!(
            "vault USDC delta was {}, expected {}",
            vault_usdc_before.saturating_sub(vault_usdc_after),
            amount
        ));
    }

    let report = NavRoundtripEvmWithdrawalReport {
        schema: NAV_ROUNDTRIP_EVM_WITHDRAWAL_REPORT_SCHEMA.to_string(),
        artifact_file: artifact_file.display().to_string(),
        burn_to_redeem_report_file: options.burn_to_redeem_report_file.display().to_string(),
        source_rpc_url: options.source_rpc_url,
        source_rpc_provider_class,
        source_chain_id: options.source_chain_id,
        bridge_class: bridge_abi.bridge_class,
        vault_address: options.vault_address,
        verifier_address: options.verifier_address,
        usdc_address: options.usdc_address,
        stakehub_wallet: options.stakehub_wallet,
        settlement_asset_id: options.settlement_asset_id,
        redemption_id,
        amount_atoms: plan.withdrawal_packet.amount_atoms,
        pftl_finalized_height: call_plan.pftl_finalized_height,
        pftl_withdrawal_hash: call_plan.pftl_withdrawal_hash,
        pftl_withdrawal_hash_commitment: call_plan.pftl_withdrawal_hash_commitment,
        withdrawal_packet_digest: call_plan.withdrawal_packet_digest,
        verifier_pending_proof_id: call_plan.verifier_pending_proof_id,
        verifier_proof_digest_to_sign: call_plan.verifier_proof_digest_to_sign,
        vault_pending_withdrawal_id: call_plan.vault_pending_withdrawal_id,
        verifier_challenge_wait_secs,
        vault_challenge_wait_secs,
        session_id: options.session_id,
        wallet_usdc_before_atoms: wallet_usdc_before.to_string(),
        wallet_usdc_after_atoms: wallet_usdc_after.to_string(),
        vault_usdc_before_atoms: vault_usdc_before.to_string(),
        vault_usdc_after_atoms: vault_usdc_after.to_string(),
        launch_session_managed_externally: options.launch_session_managed_externally,
        submit_proof_tx: agent_tx_hash(&submit_proof_response, "submit proof")?,
        submit_proof_gas_used: agent_gas_used(&submit_proof_response, "submit proof")?,
        finalize_proof_tx: agent_tx_hash(&finalize_proof_response, "finalize proof")?,
        finalize_proof_gas_used: agent_gas_used(&finalize_proof_response, "finalize proof")?,
        submit_withdrawal_tx: agent_tx_hash(&submit_withdrawal_response, "submit withdrawal")?,
        submit_withdrawal_gas_used: agent_gas_used(&submit_withdrawal_response, "submit withdrawal")?,
        finalize_withdrawal_tx: agent_tx_hash(&finalize_withdrawal_response, "finalize withdrawal")?,
        finalize_withdrawal_gas_used: agent_gas_used(&finalize_withdrawal_response, "finalize withdrawal")?,
        claim_withdrawal_tx: agent_tx_hash(&claim_withdrawal_response, "claim withdrawal")?,
        claim_withdrawal_gas_used: agent_gas_used(&claim_withdrawal_response, "claim withdrawal")?,
        submit_proof_calldata_file: submit_proof_calldata_file.display().to_string(),
        finalize_proof_calldata_file: finalize_proof_calldata_file.display().to_string(),
        submit_withdrawal_calldata_file: submit_withdrawal_calldata_file.display().to_string(),
        finalize_withdrawal_calldata_file: finalize_withdrawal_calldata_file.display().to_string(),
        claim_withdrawal_calldata_file: claim_withdrawal_calldata_file.display().to_string(),
        agent_open_session_file: agent_open_session_file.display().to_string(),
        agent_submit_proof_file: agent_submit_proof_file.display().to_string(),
        agent_finalize_proof_file: agent_finalize_proof_file.display().to_string(),
        agent_submit_withdrawal_file: agent_submit_withdrawal_file.display().to_string(),
        agent_finalize_withdrawal_file: agent_finalize_withdrawal_file.display().to_string(),
        agent_claim_withdrawal_file: agent_claim_withdrawal_file.display().to_string(),
        agent_close_session_file: agent_close_session_file.display().to_string(),
        receipt_watches,
        delta_ok: failure_reasons.is_empty(),
        failure_reasons,
    };
    write_json_file(&artifact_file, &report)?;
    Ok(report)
}

fn nav_roundtrip_live_demo_pftl_settle(
    options: NavRoundtripPftlSettleOptions,
) -> Result<NavRoundtripPftlSettleReport, String> {
    std::fs::create_dir_all(&options.artifact_dir).map_err(|error| {
        format!(
            "failed to create NAV roundtrip artifact dir `{}`: {error}",
            options.artifact_dir.display()
        )
    })?;
    let artifact_file = options.artifact_dir.join("pftl-settle.json");
    if options.resume && artifact_file.is_file() {
        let raw = std::fs::read_to_string(&artifact_file).map_err(|error| {
            format!(
                "failed to read existing PFTL settle artifact `{}`: {error}",
                artifact_file.display()
            )
        })?;
        return serde_json::from_str::<NavRoundtripPftlSettleReport>(&raw).map_err(|error| {
            format!(
                "existing PFTL settle artifact `{}` is not a NAV roundtrip settle report: {error}",
                artifact_file.display()
            )
        });
    }
    if artifact_file.exists() && !options.overwrite {
        return Err(format!(
            "PFTL settle artifact `{}` already exists; use --resume, --overwrite, or a new artifact dir",
            artifact_file.display()
        ));
    }

    let evm_raw = std::fs::read_to_string(&options.evm_withdrawal_report_file).map_err(|error| {
        format!(
            "failed to read EVM withdrawal report `{}`: {error}",
            options.evm_withdrawal_report_file.display()
        )
    })?;
    let evm_report = serde_json::from_str::<NavRoundtripEvmWithdrawalReport>(&evm_raw)
        .map_err(|error| {
            format!(
                "EVM withdrawal report `{}` is invalid: {error}",
                options.evm_withdrawal_report_file.display()
            )
        })?;
    if evm_report.settlement_asset_id != options.settlement_asset_id {
        return Err(format!(
            "EVM withdrawal report settlement asset `{}` does not match --pfusdc `{}`",
            evm_report.settlement_asset_id, options.settlement_asset_id
        ));
    }
    if !evm_report.delta_ok {
        return Err(format!(
            "EVM withdrawal report `{}` did not verify deltas: {:?}",
            options.evm_withdrawal_report_file.display(),
            evm_report.failure_reasons
        ));
    }

    let store = postfiat_storage::NodeStore::new(&options.data_dir);
    let ledger_before = store
        .read_ledger()
        .map_err(|error| format!("PFTL settle read ledger failed: {error}"))?;
    let nav_asset = ledger_before
        .nav_asset(&options.settlement_asset_id)
        .ok_or_else(|| {
            format!(
                "missing settlement NAV asset `{}`",
                options.settlement_asset_id
            )
        })?
        .clone();
    let issuer_or_redemption_account = options
        .issuer_or_redemption_account
        .clone()
        .unwrap_or_else(|| {
            if nav_asset.redemption_account.trim().is_empty() {
                nav_asset.issuer.clone()
            } else {
                nav_asset.redemption_account.clone()
            }
        });
    let redemption_before = ledger_before
        .vault_bridge_redemptions
        .iter()
        .find(|redemption| {
            redemption.asset_id == options.settlement_asset_id
                && redemption.redemption_id == evm_report.redemption_id
        })
        .cloned()
        .ok_or_else(|| {
            format!(
                "missing vault bridge redemption `{}` for asset `{}`",
                evm_report.redemption_id, options.settlement_asset_id
            )
        })?;
    let bucket_before = ledger_before
        .vault_bridge_bucket_states
        .iter()
        .find(|bucket| bucket.bucket_id == redemption_before.bucket_id)
        .cloned();
    let settlement_receipt_hash = options
        .settlement_receipt_hash
        .clone()
        .unwrap_or_else(|| nav_roundtrip_vault_bridge_settlement_receipt_hash(&evm_report));

    let operation = postfiat_types::AssetTransactionOperation::VaultBridgeRedeemSettle(
        postfiat_types::VaultBridgeRedeemSettleOperation {
            issuer_or_redemption_account: issuer_or_redemption_account.clone(),
            asset_id: options.settlement_asset_id.clone(),
            redemption_id: evm_report.redemption_id.clone(),
            settlement_receipt_hash: settlement_receipt_hash.clone(),
            settled_atoms: evm_report.amount_atoms,
            withdrawal_observations: Vec::new(),
        },
    );
    operation
        .validate()
        .map_err(|error| format!("PFTL settle operation invalid: {error}"))?;
    let operation_file = options.artifact_dir.join("vault-bridge-redeem-settle.operation.json");
    write_json_file(&operation_file, &operation)?;

    let operations_file = options.artifact_dir.join("pftl-settle.certified-ops.json");
    let request = serde_json::json!({
        "schema": CERTIFIED_ASSET_OPS_REQUEST_SCHEMA,
        "operations": [
            {
                "label": "vault-bridge-redeem-settle",
                "source": issuer_or_redemption_account.clone(),
                "key_file": options.settlement_key_file.display().to_string(),
                "operation": operation,
                "dependencies": [{
                    "label": "vault-bridge-burn-to-redeem",
                    "mode": "prior_round",
                    "reason": "settlement closes a redemption created by the prior burn-to-redeem round and claimed on the source chain",
                }],
            },
        ],
    });
    write_json_file(&operations_file, &request)?;

    let certified_ops_artifact_dir = options.artifact_dir.join("pftl-settle-certified");
    let certified_ops = certified_asset_ops_batch(CertifiedAssetOpsBatchOptions {
        data_dir: options.data_dir.clone(),
        topology_file: options.topology_file.clone(),
        key_file: options.validator_key_file.clone(),
        proposal_key_file: options.proposal_key_file.clone(),
        ops_file: operations_file.clone(),
        artifact_dir: certified_ops_artifact_dir.clone(),
        max_transactions: None,
        require_local_proposer: options.require_local_proposer,
        require_signed_proposal: options.require_signed_proposal,
        allow_peer_failures: options.allow_peer_failures,
        quorum_early_full_propagation: options.quorum_early_full_propagation,
        local_apply_before_certified_send: options.local_apply_before_certified_send,
        defer_certified_sends: options.defer_certified_sends,
        block_height: options.block_height,
        view: options.view,
        timeout_certificate_file: options.timeout_certificate_file.clone(),
        timeout_ms: options.timeout_ms,
        send_retries: options.send_retries,
        retry_backoff_ms: options.retry_backoff_ms,
        allow_existing_mempool: options.allow_existing_mempool,
        resume: options.resume,
        overwrite: options.overwrite,
        prepare_only: options.prepare_only,
        batch_only: options.batch_only,
    })?;

    let mut redemption_state_after = None;
    let mut redemption_queue_after_atoms = None;
    let mut counted_value_after_atoms = None;
    let mut accounting_ok = None;
    let mut failure_reasons = Vec::new();
    if !options.prepare_only && !options.batch_only {
        let ledger_after = store
            .read_ledger()
            .map_err(|error| format!("PFTL settle read final ledger failed: {error}"))?;
        let redemption_after = ledger_after
            .vault_bridge_redemptions
            .iter()
            .find(|redemption| {
                redemption.asset_id == options.settlement_asset_id
                    && redemption.redemption_id == evm_report.redemption_id
            })
            .ok_or_else(|| {
                format!(
                    "PFTL settle could not find redemption `{}` after settlement",
                    evm_report.redemption_id
                )
            })?;
        redemption_state_after = Some(redemption_after.state.clone());
        if redemption_after.settled_atoms != evm_report.amount_atoms {
            failure_reasons.push(format!(
                "redemption settled_atoms was {}, expected {}",
                redemption_after.settled_atoms, evm_report.amount_atoms
            ));
        }
        if redemption_after.settlement_receipt_hash != settlement_receipt_hash {
            failure_reasons.push("redemption settlement_receipt_hash did not match report".to_string());
        }
        if redemption_after.state != postfiat_types::VAULT_BRIDGE_REDEMPTION_STATE_SETTLED {
            failure_reasons.push(format!(
                "redemption state was `{}`, expected `{}`",
                redemption_after.state,
                postfiat_types::VAULT_BRIDGE_REDEMPTION_STATE_SETTLED
            ));
        }
        if let Some(bucket_before) = &bucket_before {
            if let Some(bucket_after) = ledger_after
                .vault_bridge_bucket_states
                .iter()
                .find(|bucket| bucket.bucket_id == bucket_before.bucket_id)
            {
                redemption_queue_after_atoms = Some(bucket_after.redemption_queue_atoms);
                counted_value_after_atoms = Some(bucket_after.counted_value_atoms);
                let expected_queue = bucket_before
                    .redemption_queue_atoms
                    .saturating_sub(evm_report.amount_atoms);
                let expected_counted = bucket_before
                    .counted_value_atoms
                    .saturating_sub(evm_report.amount_atoms);
                if bucket_after.redemption_queue_atoms != expected_queue {
                    failure_reasons.push(format!(
                        "bucket redemption_queue_atoms was {}, expected {}",
                        bucket_after.redemption_queue_atoms, expected_queue
                    ));
                }
                if bucket_after.counted_value_atoms != expected_counted {
                    failure_reasons.push(format!(
                        "bucket counted_value_atoms was {}, expected {}",
                        bucket_after.counted_value_atoms, expected_counted
                    ));
                }
            } else {
                failure_reasons.push(format!(
                    "missing bucket `{}` after settlement",
                    bucket_before.bucket_id
                ));
            }
        }
        accounting_ok = Some(failure_reasons.is_empty());
    }

    let report = NavRoundtripPftlSettleReport {
        schema: NAV_ROUNDTRIP_PFTL_SETTLE_REPORT_SCHEMA.to_string(),
        artifact_file: artifact_file.display().to_string(),
        evm_withdrawal_report_file: options.evm_withdrawal_report_file.display().to_string(),
        settlement_asset_id: options.settlement_asset_id,
        issuer_or_redemption_account,
        redemption_id: evm_report.redemption_id,
        settlement_receipt_hash,
        settled_atoms: evm_report.amount_atoms,
        redemption_state_before: Some(redemption_before.state),
        redemption_state_after,
        redemption_queue_before_atoms: bucket_before.as_ref().map(|bucket| bucket.redemption_queue_atoms),
        redemption_queue_after_atoms,
        counted_value_before_atoms: bucket_before.as_ref().map(|bucket| bucket.counted_value_atoms),
        counted_value_after_atoms,
        operation_file: operation_file.display().to_string(),
        operations_file: operations_file.display().to_string(),
        certified_ops_artifact_dir: certified_ops_artifact_dir.display().to_string(),
        certified_ops,
        accounting_ok,
        failure_reasons,
    };
    write_json_file(&artifact_file, &report)?;
    Ok(report)
}

fn nav_roundtrip_live_demo_nav_checkpoint(
    options: NavRoundtripNavCheckpointOptions,
) -> Result<NavRoundtripNavCheckpointReport, String> {
    if options.prepare_only && options.resume {
        let artifact_file = options.artifact_dir.join("nav-checkpoint.json");
        if artifact_file.is_file() {
            let raw = std::fs::read_to_string(&artifact_file).map_err(|error| {
                format!(
                    "failed to read existing NAV checkpoint artifact `{}`: {error}",
                    artifact_file.display()
                )
            })?;
            return serde_json::from_str::<NavRoundtripNavCheckpointReport>(&raw).map_err(
                |error| {
                    format!(
                        "existing NAV checkpoint artifact `{}` is not a NAV roundtrip checkpoint report: {error}",
                        artifact_file.display()
                    )
                },
            );
        }
    }
    std::fs::create_dir_all(&options.artifact_dir).map_err(|error| {
        format!(
            "failed to create NAV checkpoint artifact dir `{}`: {error}",
            options.artifact_dir.display()
        )
    })?;
    let artifact_file = options.artifact_dir.join("nav-checkpoint.json");
    if options.resume && artifact_file.is_file() {
        let raw = std::fs::read_to_string(&artifact_file).map_err(|error| {
            format!(
                "failed to read existing NAV checkpoint artifact `{}`: {error}",
                artifact_file.display()
            )
        })?;
        return serde_json::from_str::<NavRoundtripNavCheckpointReport>(&raw).map_err(|error| {
            format!(
                "existing NAV checkpoint artifact `{}` is not a NAV roundtrip checkpoint report: {error}",
                artifact_file.display()
            )
        });
    }
    if artifact_file.exists() && !options.overwrite {
        return Err(format!(
            "NAV checkpoint artifact `{}` already exists; use --resume, --overwrite, or a new artifact dir",
            artifact_file.display()
        ));
    }

    let store = postfiat_storage::NodeStore::new(&options.data_dir);
    let ledger_before = store
        .read_ledger()
        .map_err(|error| format!("NAV checkpoint read ledger failed: {error}"))?;
    let nav_asset = ledger_before
        .nav_asset(&options.nav_asset_id)
        .ok_or_else(|| format!("missing NAV asset `{}`", options.nav_asset_id))?
        .clone();
    let base_packet = if nav_asset.finalized_epoch == 0
        || nav_asset.finalized_reserve_packet_hash.is_empty()
    {
        None
    } else {
        ledger_before
            .nav_reserve_packet(
                &options.nav_asset_id,
                nav_asset.finalized_epoch,
                &nav_asset.finalized_reserve_packet_hash,
            )
            .cloned()
    };
    let verified_net_assets_before = base_packet.as_ref().map(|packet| packet.verified_net_assets);
    let checkpoint_epoch = options
        .epoch
        .unwrap_or_else(|| nav_asset.finalized_epoch.saturating_add(1));
    if checkpoint_epoch <= nav_asset.finalized_epoch {
        return Err(format!(
            "NAV checkpoint epoch {checkpoint_epoch} must be greater than finalized epoch {}",
            nav_asset.finalized_epoch
        ));
    }
    let profile = if nav_asset.proof_profile.len() == postfiat_types::NAV_PROFILE_ID_HEX_LEN {
        ledger_before.nav_proof_profile(&nav_asset.proof_profile).cloned()
    } else {
        None
    };
    let submitter = if nav_asset.reserve_operator.is_empty() {
        nav_asset.issuer.clone()
    } else {
        nav_asset.reserve_operator.clone()
    };
    let submitter_key_file = options
        .submitter_key_file
        .clone()
        .unwrap_or_else(|| options.issuer_key_file.clone());
    let checkpoint = nav_roundtrip_build_nav_checkpoint_fields(
        &ledger_before,
        &nav_asset,
        base_packet.as_ref(),
        profile.as_ref(),
        checkpoint_epoch,
        options.reserve_packet_hash.as_deref(),
        options.attestor_root.as_deref(),
    )?;

    let submit_operation = postfiat_types::AssetTransactionOperation::NavReserveSubmit(
        postfiat_types::NavReserveSubmitOperation {
            issuer: nav_asset.issuer.clone(),
            submitter: submitter.clone(),
            asset_id: options.nav_asset_id.clone(),
            epoch: checkpoint_epoch,
            nav_per_unit: checkpoint.nav_per_unit,
            circulating_supply: checkpoint.circulating_supply,
            verified_net_assets: checkpoint.verified_net_assets,
            proof_profile: nav_asset.proof_profile.clone(),
            source_root: checkpoint.source_root.clone(),
            attestor_root: checkpoint.attestor_root.clone(),
            reserve_packet_hash: checkpoint.reserve_packet_hash.clone(),
            reserve_accounts: checkpoint.reserve_accounts.clone(),
            sp1_proof_bytes: checkpoint.sp1_proof_bytes.clone(),
            sp1_public_values: checkpoint.sp1_public_values.clone(),
        },
    );
    let finalize_operation = postfiat_types::AssetTransactionOperation::NavEpochFinalize(
        postfiat_types::NavEpochFinalizeOperation {
            issuer: nav_asset.issuer.clone(),
            asset_id: options.nav_asset_id.clone(),
            epoch: checkpoint_epoch,
            reserve_packet_hash: checkpoint.reserve_packet_hash.clone(),
        },
    );
    submit_operation
        .validate()
        .map_err(|error| format!("NAV checkpoint submit operation invalid: {error}"))?;
    finalize_operation
        .validate()
        .map_err(|error| format!("NAV checkpoint finalize operation invalid: {error}"))?;

    let submit_operation_file = options.artifact_dir.join("nav-reserve-submit.operation.json");
    let finalize_operation_file = options.artifact_dir.join("nav-epoch-finalize.operation.json");
    write_json_file(&submit_operation_file, &submit_operation)?;
    write_json_file(&finalize_operation_file, &finalize_operation)?;

    let submit_operations_file = options.artifact_dir.join("nav-checkpoint-submit.certified-ops.json");
    let submit_request = serde_json::json!({
        "schema": CERTIFIED_ASSET_OPS_REQUEST_SCHEMA,
        "operations": [
            {
                "label": "nav-reserve-submit",
                "source": submitter.clone(),
                "key_file": submitter_key_file.display().to_string(),
                "operation": submit_operation,
                "dependencies": [],
            },
        ],
    });
    write_json_file(&submit_operations_file, &submit_request)?;
    let finalize_operations_file = options.artifact_dir.join("nav-checkpoint-finalize.certified-ops.json");
    let finalize_request = serde_json::json!({
        "schema": CERTIFIED_ASSET_OPS_REQUEST_SCHEMA,
        "operations": [
            {
                "label": "nav-epoch-finalize",
                "source": nav_asset.issuer.clone(),
                "key_file": options.issuer_key_file.display().to_string(),
                "operation": finalize_operation,
                "dependencies": [{
                    "label": "nav-reserve-submit",
                    "mode": "prior_round",
                    "reason": "epoch finalize must respect the reserve packet/profile freshness gate",
                }],
            },
        ],
    });
    write_json_file(&finalize_operations_file, &finalize_request)?;

    let submit_certified_ops_artifact_dir = options.artifact_dir.join("nav-checkpoint-submit-certified");
    let submit_certified_ops = certified_asset_ops_batch(CertifiedAssetOpsBatchOptions {
        data_dir: options.data_dir.clone(),
        topology_file: options.topology_file.clone(),
        key_file: options.validator_key_file.clone(),
        proposal_key_file: options.proposal_key_file.clone(),
        ops_file: submit_operations_file.clone(),
        artifact_dir: submit_certified_ops_artifact_dir.clone(),
        max_transactions: Some(1),
        require_local_proposer: options.require_local_proposer,
        require_signed_proposal: options.require_signed_proposal,
        allow_peer_failures: options.allow_peer_failures,
        quorum_early_full_propagation: options.quorum_early_full_propagation,
        local_apply_before_certified_send: options.local_apply_before_certified_send,
        defer_certified_sends: options.defer_certified_sends,
        block_height: options.block_height,
        view: options.view,
        timeout_certificate_file: options.timeout_certificate_file.clone(),
        timeout_ms: options.timeout_ms,
        send_retries: options.send_retries,
        retry_backoff_ms: options.retry_backoff_ms,
        allow_existing_mempool: options.allow_existing_mempool,
        resume: options.resume,
        overwrite: options.overwrite,
        prepare_only: options.prepare_only,
        batch_only: false,
    })?;

    let finalize_certified_ops_artifact_dir = options.artifact_dir.join("nav-checkpoint-finalize-certified");
    let finalize_prepare_only = options.prepare_only;
    let finalize_certified_ops = certified_asset_ops_batch(CertifiedAssetOpsBatchOptions {
        data_dir: options.data_dir.clone(),
        topology_file: options.topology_file.clone(),
        key_file: options.validator_key_file.clone(),
        proposal_key_file: options.proposal_key_file.clone(),
        ops_file: finalize_operations_file.clone(),
        artifact_dir: finalize_certified_ops_artifact_dir.clone(),
        max_transactions: Some(1),
        require_local_proposer: options.require_local_proposer,
        require_signed_proposal: options.require_signed_proposal,
        allow_peer_failures: options.allow_peer_failures,
        quorum_early_full_propagation: options.quorum_early_full_propagation,
        local_apply_before_certified_send: options.local_apply_before_certified_send,
        defer_certified_sends: options.defer_certified_sends,
        block_height: None,
        view: None,
        timeout_certificate_file: options.timeout_certificate_file.clone(),
        timeout_ms: options.timeout_ms,
        send_retries: options.send_retries,
        retry_backoff_ms: options.retry_backoff_ms,
        allow_existing_mempool: options.allow_existing_mempool,
        resume: options.resume,
        overwrite: options.overwrite,
        prepare_only: finalize_prepare_only,
        batch_only: false,
    })?;

    let mut epoch_after = None;
    let mut reserve_packet_hash_after = None;
    let mut nav_per_unit_after = None;
    let mut circulating_supply_after = None;
    let mut verified_net_assets_after = None;
    let mut verified_net_assets_delta = None;
    let mut delta_ok = None;
    let mut failure_reasons = Vec::new();
    if !options.prepare_only {
        let ledger_after = store
            .read_ledger()
            .map_err(|error| format!("NAV checkpoint read final ledger failed: {error}"))?;
        let final_asset = ledger_after
            .nav_asset(&options.nav_asset_id)
            .ok_or_else(|| format!("missing NAV asset `{}` after checkpoint", options.nav_asset_id))?;
        epoch_after = Some(final_asset.finalized_epoch);
        reserve_packet_hash_after = Some(final_asset.finalized_reserve_packet_hash.clone());
        nav_per_unit_after = Some(final_asset.nav_per_unit);
        circulating_supply_after = Some(final_asset.circulating_supply);
        let final_packet = ledger_after
            .nav_reserve_packet(
                &options.nav_asset_id,
                checkpoint_epoch,
                &checkpoint.reserve_packet_hash,
            )
            .ok_or_else(|| {
                format!(
                    "missing finalized NAV checkpoint packet for asset `{}` epoch {checkpoint_epoch}",
                    options.nav_asset_id
                )
            })?;
        verified_net_assets_after = Some(final_packet.verified_net_assets);
        if final_asset.finalized_epoch != checkpoint_epoch {
            failure_reasons.push(format!(
                "finalized epoch was {}, expected {checkpoint_epoch}",
                final_asset.finalized_epoch
            ));
        }
        if final_asset.finalized_reserve_packet_hash != checkpoint.reserve_packet_hash {
            failure_reasons.push("finalized reserve packet hash did not match checkpoint".to_string());
        }
        if final_packet.verified_net_assets != checkpoint.verified_net_assets {
            failure_reasons.push(format!(
                "finalized verified_net_assets was {}, expected {}",
                final_packet.verified_net_assets, checkpoint.verified_net_assets
            ));
        }
        if let Some(before) = verified_net_assets_before {
            let delta = i128::from(final_packet.verified_net_assets) - i128::from(before);
            verified_net_assets_delta = Some(delta);
            if let Some(expected) = options.expected_vna_delta {
                if delta != expected {
                    failure_reasons.push(format!(
                        "verified_net_assets delta was {delta}, expected {expected}"
                    ));
                }
            }
        }
        delta_ok = Some(failure_reasons.is_empty());
    }

    let report = NavRoundtripNavCheckpointReport {
        schema: NAV_ROUNDTRIP_NAV_CHECKPOINT_REPORT_SCHEMA.to_string(),
        artifact_file: artifact_file.display().to_string(),
        nav_asset_id: options.nav_asset_id,
        issuer: nav_asset.issuer,
        submitter,
        verifier_kind: profile.as_ref().map(|profile| profile.verifier_kind.clone()),
        source_class: profile.as_ref().map(|profile| profile.source_class.clone()),
        epoch_before: nav_asset.finalized_epoch,
        epoch_after,
        checkpoint_epoch,
        reserve_packet_hash_before: nav_asset.finalized_reserve_packet_hash,
        reserve_packet_hash_after,
        reserve_packet_hash: checkpoint.reserve_packet_hash,
        nav_per_unit_before: nav_asset.nav_per_unit,
        nav_per_unit_after,
        nav_per_unit: checkpoint.nav_per_unit,
        circulating_supply_before: nav_asset.circulating_supply,
        circulating_supply_after,
        circulating_supply: checkpoint.circulating_supply,
        verified_net_assets_before,
        verified_net_assets_after,
        verified_net_assets: checkpoint.verified_net_assets,
        verified_net_assets_delta,
        expected_verified_net_assets_delta: options.expected_vna_delta,
        delta_ok,
        source_root: checkpoint.source_root,
        attestor_root: checkpoint.attestor_root,
        overlay_value_nav_units: checkpoint.overlay_value_nav_units,
        overlay_source_root: checkpoint.overlay_source_root,
        sp1_base_verified_net_assets: checkpoint.sp1_base_verified_net_assets,
        submit_operation_file: submit_operation_file.display().to_string(),
        finalize_operation_file: finalize_operation_file.display().to_string(),
        submit_operations_file: submit_operations_file.display().to_string(),
        finalize_operations_file: finalize_operations_file.display().to_string(),
        submit_certified_ops_artifact_dir: submit_certified_ops_artifact_dir.display().to_string(),
        finalize_certified_ops_artifact_dir: finalize_certified_ops_artifact_dir.display().to_string(),
        submit_certified_ops,
        finalize_certified_ops,
        failure_reasons,
    };
    write_json_file(&artifact_file, &report)?;
    Ok(report)
}

#[derive(Debug, Clone)]
struct NavRoundtripCheckpointFields {
    nav_per_unit: u64,
    circulating_supply: u64,
    verified_net_assets: u64,
    source_root: String,
    attestor_root: String,
    reserve_packet_hash: String,
    reserve_accounts: Vec<String>,
    sp1_proof_bytes: Vec<u8>,
    sp1_public_values: Vec<u8>,
    overlay_value_nav_units: Option<u64>,
    overlay_source_root: Option<String>,
    sp1_base_verified_net_assets: Option<u64>,
}

#[derive(Debug, Clone)]
struct NavRoundtripSp1Decoded {
    policy_hash_hex: String,
    verified_net_assets: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NavRoundtripSubscriptionOverlay {
    value_nav_units: u64,
    source_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NavRoundtripSubscriptionOverlayRow {
    allocation_id: String,
    settlement_asset_id: String,
    bucket_id: String,
    receipt_id: String,
    amount_atoms: u64,
    released_atoms: u64,
    remaining_atoms: u64,
    value_nav_units: u64,
    retired_at_height: u64,
    bucket_source_domain: String,
    bucket_policy_hash: String,
    bucket_gross_receipt_atoms: u64,
    bucket_counted_value_atoms: u64,
    bucket_nav_subscription_allocations_atoms: u64,
    bucket_redemption_queue_atoms: u64,
    bucket_outstanding_vault_bridge_atoms: u64,
    bucket_status: String,
}

fn nav_roundtrip_asset_unit_scale(
    ledger: &postfiat_types::LedgerState,
    asset_id: &str,
) -> Result<u128, String> {
    let asset = ledger
        .asset_definition(asset_id)
        .ok_or_else(|| format!("missing issued asset definition `{asset_id}`"))?;
    10_u128
        .checked_pow(asset.precision.into())
        .ok_or_else(|| format!("asset `{asset_id}` precision scale would overflow"))
}

fn nav_roundtrip_circulating_supply_atoms(
    ledger: &postfiat_types::LedgerState,
    nav_asset: &postfiat_types::NavTrackedAsset,
) -> Result<u64, String> {
    let asset = ledger
        .asset_definition(&nav_asset.asset_id)
        .ok_or_else(|| format!("missing issued asset definition `{}`", nav_asset.asset_id))?;
    let scale = 10_u64
        .checked_pow(asset.precision.into())
        .ok_or_else(|| format!("asset `{}` precision scale would overflow", nav_asset.asset_id))?;
    if scale <= 1 || nav_asset.circulating_supply == 0 {
        return Ok(nav_asset.circulating_supply);
    }
    if let Some(max_supply) = asset.max_supply {
        let whole_unit_cap = max_supply / scale;
        if max_supply % scale == 0 && nav_asset.circulating_supply == whole_unit_cap {
            return nav_asset.circulating_supply.checked_mul(scale).ok_or_else(|| {
                format!(
                    "NAV asset `{}` circulating supply scaling would overflow",
                    nav_asset.asset_id
                )
            });
        }
    }
    Ok(nav_asset.circulating_supply)
}

fn nav_roundtrip_build_nav_checkpoint_fields(
    ledger: &postfiat_types::LedgerState,
    nav_asset: &postfiat_types::NavTrackedAsset,
    base_packet: Option<&postfiat_types::NavReservePacket>,
    profile: Option<&postfiat_types::NavProofProfile>,
    epoch: u64,
    reserve_packet_hash_override: Option<&str>,
    attestor_root_override: Option<&str>,
) -> Result<NavRoundtripCheckpointFields, String> {
    let profile = profile.ok_or_else(|| {
        format!(
            "NAV asset `{}` proof profile `{}` is not a registered deterministic profile; automatic checkpoint generation is unsupported",
            nav_asset.asset_id, nav_asset.proof_profile
        )
    })?;
    match profile.verifier_kind.as_str() {
        postfiat_types::NAV_PROFILE_VERIFIER_SP1_GROTH16 => {
            let base_packet = base_packet.ok_or_else(|| {
                format!(
                    "NAV asset `{}` needs a finalized SP1 packet before automatic checkpoint generation",
                    nav_asset.asset_id
                )
            })?;
            if base_packet.sp1_proof_bytes.is_empty() || base_packet.sp1_public_values.is_empty() {
                return Err(format!(
                    "finalized NAV packet for `{}` has no SP1 proof/public values",
                    nav_asset.asset_id
                ));
            }
            let decoded = nav_roundtrip_decode_sp1_public_values(&base_packet.sp1_public_values)?;
            postfiat_execution::verify_sp1_groth16(
                profile,
                decoded.verified_net_assets,
                &base_packet.sp1_proof_bytes,
                &base_packet.sp1_public_values,
            )
            .map_err(|error| {
                format!(
                    "finalized SP1 proof/public values failed verification for checkpoint: {}: {}",
                    error.code(),
                    error.message()
                )
            })?;
            let overlay = nav_roundtrip_subscription_overlay(ledger, nav_asset)?;
            let circulating_supply = nav_roundtrip_circulating_supply_atoms(ledger, nav_asset)?;
            if circulating_supply == 0 {
                return Err(format!(
                    "NAV asset `{}` has zero finalized circulating_supply",
                    nav_asset.asset_id
                ));
            }
            let verified_net_assets = decoded
                .verified_net_assets
                .checked_add(overlay.as_ref().map_or(0, |overlay| overlay.value_nav_units))
                .ok_or_else(|| {
                    "NAV checkpoint SP1 overlay verified_net_assets overflowed".to_string()
                })?;
            let unit_scale = nav_roundtrip_asset_unit_scale(ledger, &nav_asset.asset_id)?;
            let nav_per_unit = postfiat_types::nav_per_unit_floor_with_unit_scale(
                verified_net_assets,
                circulating_supply,
                unit_scale,
            )
            .map_err(|error| format!("NAV checkpoint nav_per_unit failed: {error}"))?;
            let source_root = if let Some(overlay) = &overlay {
                nav_roundtrip_sp1_subscription_source_root(
                    nav_asset,
                    profile,
                    &decoded,
                    &base_packet.sp1_public_values,
                    overlay,
                )?
            } else {
                nav_roundtrip_find_sp1_base_source_root(
                    ledger,
                    nav_asset,
                    &base_packet.sp1_proof_bytes,
                    &base_packet.sp1_public_values,
                    decoded.verified_net_assets,
                )?
            };
            let attestor_root = attestor_root_override
                .map(str::to_string)
                .unwrap_or_else(|| {
                    nav_roundtrip_checkpoint_attestor_root(
                        &nav_asset.asset_id,
                        epoch,
                        &source_root,
                        verified_net_assets,
                    )
                });
            let reserve_packet_hash = reserve_packet_hash_override
                .map(str::to_string)
                .unwrap_or_else(|| {
                    nav_roundtrip_checkpoint_reserve_packet_hash(
                        nav_asset,
                        epoch,
                        nav_per_unit,
                        circulating_supply,
                        verified_net_assets,
                        &source_root,
                        &attestor_root,
                    )
                });
            Ok(NavRoundtripCheckpointFields {
                nav_per_unit,
                circulating_supply,
                verified_net_assets,
                source_root,
                attestor_root,
                reserve_packet_hash,
                reserve_accounts: Vec::new(),
                sp1_proof_bytes: base_packet.sp1_proof_bytes.clone(),
                sp1_public_values: base_packet.sp1_public_values.clone(),
                overlay_value_nav_units: overlay.as_ref().map(|overlay| overlay.value_nav_units),
                overlay_source_root: overlay.as_ref().map(|overlay| overlay.source_root.clone()),
                sp1_base_verified_net_assets: Some(decoded.verified_net_assets),
            })
        }
        postfiat_types::NAV_PROFILE_VERIFIER_LEDGER_TRANSPARENT => {
            let base_packet = base_packet.ok_or_else(|| {
                format!(
                    "NAV asset `{}` needs a finalized ledger-transparent packet to inherit reserve_accounts",
                    nav_asset.asset_id
                )
            })?;
            if base_packet.reserve_accounts.is_empty() {
                return Err(
                    "ledger-transparent checkpoint needs nonempty reserve_accounts".to_string(),
                );
            }
            let circulating_supply = nav_roundtrip_circulating_supply_atoms(ledger, nav_asset)?;
            if circulating_supply == 0 {
                return Err(format!(
                    "NAV asset `{}` has zero finalized circulating_supply",
                    nav_asset.asset_id
                ));
            }
            let verified_net_assets =
                base_packet
                    .reserve_accounts
                    .iter()
                    .try_fold(0_u64, |total, account| {
                        let balance = ledger
                            .account(account)
                            .ok_or_else(|| {
                                format!(
                                    "ledger-transparent reserve account `{account}` is missing"
                                )
                            })?
                            .balance;
                        total.checked_add(balance).ok_or_else(|| {
                            "ledger-transparent reserve sum overflowed".to_string()
                        })
                    })?;
            let unit_scale = nav_roundtrip_asset_unit_scale(ledger, &nav_asset.asset_id)?;
            let nav_per_unit = postfiat_types::nav_per_unit_floor_with_unit_scale(
                verified_net_assets,
                circulating_supply,
                unit_scale,
            )
            .map_err(|error| format!("NAV checkpoint nav_per_unit failed: {error}"))?;
            let source_root = base_packet.source_root.clone();
            let attestor_root = attestor_root_override
                .map(str::to_string)
                .unwrap_or_else(|| {
                    nav_roundtrip_checkpoint_attestor_root(
                        &nav_asset.asset_id,
                        epoch,
                        &source_root,
                        verified_net_assets,
                    )
                });
            let reserve_packet_hash = reserve_packet_hash_override
                .map(str::to_string)
                .unwrap_or_else(|| {
                    nav_roundtrip_checkpoint_reserve_packet_hash(
                        nav_asset,
                        epoch,
                        nav_per_unit,
                        circulating_supply,
                        verified_net_assets,
                        &source_root,
                        &attestor_root,
                    )
                });
            Ok(NavRoundtripCheckpointFields {
                nav_per_unit,
                circulating_supply,
                verified_net_assets,
                source_root,
                attestor_root,
                reserve_packet_hash,
                reserve_accounts: base_packet.reserve_accounts.clone(),
                sp1_proof_bytes: Vec::new(),
                sp1_public_values: Vec::new(),
                overlay_value_nav_units: None,
                overlay_source_root: None,
                sp1_base_verified_net_assets: None,
            })
        }
        _ if profile
            .source_class
            .starts_with(postfiat_types::VAULT_BRIDGE_PROFILE_SOURCE_CLASS_PREFIX) =>
        {
            let reserve_accounts = nav_roundtrip_vault_bridge_reserve_accounts(profile)?;
            let circulating_supply = nav_roundtrip_issued_asset_supply(ledger, &nav_asset.asset_id)?;
            let verified_net_assets = postfiat_types::vault_bridge_counted_value_for_asset(
                &ledger.vault_bridge_bucket_states,
                &nav_asset.asset_id,
            )
            .map_err(|error| format!("vault bridge counted value failed: {error}"))?;
            let source_root = postfiat_types::vault_bridge_source_root_for_asset(
                &ledger.vault_bridge_bucket_states,
                &nav_asset.asset_id,
            )
            .map_err(|error| format!("vault bridge source root failed: {error}"))?;
            let nav_per_unit = postfiat_types::VAULT_BRIDGE_UNIT;
            let attestor_root = attestor_root_override
                .map(str::to_string)
                .unwrap_or_else(|| {
                    nav_roundtrip_checkpoint_attestor_root(
                        &nav_asset.asset_id,
                        epoch,
                        &source_root,
                        verified_net_assets,
                    )
                });
            let reserve_packet_hash = reserve_packet_hash_override
                .map(str::to_string)
                .unwrap_or_else(|| {
                    nav_roundtrip_checkpoint_reserve_packet_hash(
                        nav_asset,
                        epoch,
                        nav_per_unit,
                        circulating_supply,
                        verified_net_assets,
                        &source_root,
                        &attestor_root,
                    )
                });
            Ok(NavRoundtripCheckpointFields {
                nav_per_unit,
                circulating_supply,
                verified_net_assets,
                source_root,
                attestor_root,
                reserve_packet_hash,
                reserve_accounts,
                sp1_proof_bytes: Vec::new(),
                sp1_public_values: Vec::new(),
                overlay_value_nav_units: None,
                overlay_source_root: None,
                sp1_base_verified_net_assets: None,
            })
        }
        _ => Err(format!(
            "automatic checkpoint generation does not support verifier_kind `{}` source_class `{}`",
            profile.verifier_kind, profile.source_class
        )),
    }
}
