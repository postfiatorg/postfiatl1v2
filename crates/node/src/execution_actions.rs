use super::*;

pub(super) fn execute_fastlane_primary_for_chain(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    transaction: &postfiat_types::FastLanePrimaryTransactionV1,
    block_height: u64,
) -> Receipt {
    if let postfiat_types::FastLanePrimaryOperationV1::OwnedDeposit { signed } =
        &transaction.operation
    {
        let tx_id = || {
            transaction
                .tx_id()
                .map(|id| bytes_to_hex(&id.0))
                .unwrap_or_else(|_| "owned-deposit-invalid-id".to_string())
        };
        let genesis_hash_bytes: [u8; 48] = match hex_to_bytes(&genesis_hash(genesis))
            .ok()
            .and_then(|bytes| bytes.try_into().ok())
        {
            Some(bytes) => bytes,
            None => {
                return Receipt::rejected(
                    tx_id(),
                    "owned_deposit_invalid_local_genesis",
                    "local genesis hash is not a canonical 48-byte value",
                )
            }
        };
        let expected = postfiat_types::FastSwapChainDomainV1 {
            chain_id: genesis.chain_id.clone(),
            genesis_hash: postfiat_types::FastSwapOpaqueHashV1(genesis_hash_bytes),
            protocol_version: genesis.protocol_version,
        };
        if signed.deposit.domain != expected {
            return Receipt::rejected(
                tx_id(),
                "owned_deposit_wrong_domain",
                "signed account-to-FastPay deposit does not match chain genesis",
            );
        }
    }
    execute_fastlane_primary_transaction(ledger, transaction, block_height)
}

pub(super) fn execute_transparent_batch(
    genesis: &Genesis,
    governance: &GovernanceState,
    ledger: &mut LedgerState,
    batch: &TransactionBatch,
    block_height: u64,
    asset_execution_compatibility: AssetExecutionCompatibility,
) -> Vec<Receipt> {
    let mut receipts = Vec::with_capacity(batch.transaction_count());
    for transfer in &batch.transactions {
        receipts.push(execute_transfer(genesis, ledger, transfer));
    }
    for payment in &batch.payments_v2 {
        receipts.push(execute_payment_v2(genesis, ledger, payment));
    }
    for transaction in &batch.asset_transactions {
        receipts.push(
            governed_vault_bridge_route_rejection(transaction, governance, ledger, block_height)
                .unwrap_or_else(|| {
                    execute_asset_transaction_with_compatibility(
                        genesis,
                        ledger,
                        transaction,
                        block_height,
                        asset_execution_compatibility,
                    )
                }),
        );
    }
    for transaction in &batch.atomic_swap_transactions {
        receipts.push(execute_atomic_swap_transaction_with_compatibility(
            genesis,
            ledger,
            transaction,
            block_height,
            asset_execution_compatibility,
        ));
    }
    for transaction in &batch.fastlane_primary_transactions {
        receipts.push(execute_fastlane_primary_for_chain(
            genesis,
            ledger,
            transaction,
            block_height,
        ));
    }
    for transaction in &batch.escrow_transactions {
        receipts.push(execute_escrow_transaction(
            genesis,
            ledger,
            transaction,
            block_height,
        ));
    }
    for transaction in &batch.nft_transactions {
        receipts.push(execute_nft_transaction(genesis, ledger, transaction));
    }
    for transaction in &batch.offer_transactions {
        receipts.push(execute_offer_transaction(
            genesis,
            ledger,
            transaction,
            block_height,
        ));
    }
    receipts
}

fn governed_vault_bridge_route_rejection(
    transaction: &SignedAssetTransaction,
    governance: &GovernanceState,
    ledger: &LedgerState,
    block_height: u64,
) -> Option<Receipt> {
    let authority_height = governance.vault_bridge_route_authority_activation_height()?;
    if block_height < authority_height {
        return None;
    }
    let result = governed_vault_bridge_route_target(&transaction.unsigned.operation, ledger)
        .and_then(|target| {
            let Some(target) = target else {
                return Ok(());
            };
            let record = if target.require_active {
                governance.active_vault_bridge_route_profile(&target.asset_id, block_height)?
            } else {
                governance
                    .authorized_vault_bridge_route_profile(&target.asset_id, &target.policy_hash)?
            };
            if record.profile_hash != target.policy_hash {
                return Err(if target.require_active {
                    "vault bridge transaction policy_hash does not match the active governed route"
                        .to_string()
                } else {
                    "vault bridge lifecycle policy_hash is not an authorized pinned route"
                        .to_string()
                });
            }
            if target.source_domain != record.profile.source_domain() {
                return Err(
                    "vault bridge lifecycle source does not match its governed route".to_string(),
                );
            }
            if let Some(evidence) = target.evidence.as_ref() {
                if evidence.source_chain_id != record.profile.source_chain_id
                    || evidence.vault_address != record.profile.vault_address
                    || evidence.token_address != record.profile.token_address
                {
                    return Err(
                        "vault bridge deposit source does not match its governed route".to_string(),
                    );
                }
                let expected_binding =
                    vault_bridge_route_binding(&record.profile_hash, record.profile.route_epoch)?;
                if evidence.route_binding != expected_binding {
                    return Err(
                        "vault bridge deposit route binding does not match its governed route"
                            .to_string(),
                    );
                }
            }
            if let Some(packet) = target.withdrawal_packet.as_ref() {
                if packet.source_chain_id != record.profile.source_chain_id
                    || packet.vault_address != record.profile.vault_address
                    || packet.token_address != record.profile.token_address
                {
                    return Err(
                        "vault bridge withdrawal packet does not match its governed pinned route"
                            .to_string(),
                    );
                }
            }
            Ok(())
        });
    result.err().map(|message| {
        Receipt::rejected(
            asset_transaction_tx_id(transaction),
            "vault_bridge_route_authority_mismatch",
            message,
        )
    })
}

#[derive(Debug, Clone)]
struct GovernedVaultBridgeRouteTarget {
    asset_id: String,
    policy_hash: String,
    source_domain: String,
    evidence: Option<VaultBridgeDepositEvidence>,
    withdrawal_packet: Option<VaultBridgeWithdrawalPacket>,
    require_active: bool,
}

fn governed_vault_bridge_route_target(
    operation: &AssetTransactionOperation,
    ledger: &LedgerState,
) -> Result<Option<GovernedVaultBridgeRouteTarget>, String> {
    let deposit_target = |asset_id: &str, evidence_root: &str| {
        let deposit = ledger
            .vault_bridge_deposit(asset_id, evidence_root)
            .ok_or_else(|| "vault bridge lifecycle deposit is missing".to_string())?;
        Ok::<_, String>(GovernedVaultBridgeRouteTarget {
            asset_id: asset_id.to_string(),
            policy_hash: deposit.policy_hash.clone(),
            source_domain: deposit.evidence.source_domain(),
            evidence: Some(deposit.evidence.clone()),
            withdrawal_packet: None,
            require_active: false,
        })
    };
    let bucket_target = |asset_id: &str, bucket_id: &str, require_active: bool| {
        let bucket = ledger
            .vault_bridge_bucket(bucket_id)
            .ok_or_else(|| "vault bridge lifecycle bucket is missing".to_string())?;
        if bucket.asset_id != asset_id {
            return Err("vault bridge lifecycle bucket asset mismatch".to_string());
        }
        Ok::<_, String>(GovernedVaultBridgeRouteTarget {
            asset_id: asset_id.to_string(),
            policy_hash: bucket.policy_hash.clone(),
            source_domain: bucket.source_domain.clone(),
            evidence: None,
            withdrawal_packet: None,
            require_active,
        })
    };

    let target = match operation {
        AssetTransactionOperation::VaultBridgeDepositPropose(operation) => {
            GovernedVaultBridgeRouteTarget {
                asset_id: operation.asset_id.clone(),
                policy_hash: operation.policy_hash.clone(),
                source_domain: operation.evidence.source_domain(),
                evidence: Some(operation.evidence.clone()),
                withdrawal_packet: None,
                require_active: true,
            }
        }
        AssetTransactionOperation::VaultBridgeReceiptSubmit(operation) => {
            let evidence = operation.bridge_deposit_evidence.clone().ok_or_else(|| {
                "governed vault bridge receipt submission requires deposit evidence".to_string()
            })?;
            GovernedVaultBridgeRouteTarget {
                asset_id: operation.asset_id.clone(),
                policy_hash: operation.policy_hash.clone(),
                source_domain: operation.source_domain.clone(),
                evidence: Some(evidence),
                withdrawal_packet: None,
                require_active: true,
            }
        }
        AssetTransactionOperation::VaultBridgeDepositChallenge(operation) => {
            deposit_target(&operation.asset_id, &operation.evidence_root)?
        }
        AssetTransactionOperation::VaultBridgeDepositAttest(operation) => {
            deposit_target(&operation.asset_id, &operation.evidence_root)?
        }
        AssetTransactionOperation::VaultBridgeDepositFinalize(operation) => {
            deposit_target(&operation.asset_id, &operation.evidence_root)?
        }
        AssetTransactionOperation::VaultBridgeDepositClaim(operation) => {
            let target = deposit_target(&operation.asset_id, &operation.evidence_root)?;
            if target.policy_hash != operation.policy_hash {
                return Err("vault bridge claim policy does not match pinned deposit".to_string());
            }
            target
        }
        AssetTransactionOperation::VaultBridgeReceiptCount(operation) => {
            let receipt = ledger
                .vault_bridge_receipt(&operation.receipt_id)
                .ok_or_else(|| "vault bridge lifecycle receipt is missing".to_string())?;
            if receipt.asset_id != operation.asset_id
                || receipt.policy_hash != operation.policy_hash
            {
                return Err(
                    "vault bridge count operation does not match pinned receipt".to_string()
                );
            }
            GovernedVaultBridgeRouteTarget {
                asset_id: operation.asset_id.clone(),
                policy_hash: receipt.policy_hash.clone(),
                source_domain: receipt.source_domain.clone(),
                evidence: receipt.bridge_deposit_evidence.clone(),
                withdrawal_packet: None,
                require_active: false,
            }
        }
        AssetTransactionOperation::VaultBridgeMintFromReceipts(operation) => {
            bucket_target(&operation.asset_id, &operation.bucket_id, false)?
        }
        AssetTransactionOperation::VaultBridgeBurnToRedeem(operation) => {
            bucket_target(&operation.asset_id, &operation.bucket_id, false)?
        }
        AssetTransactionOperation::VaultBridgeRedeemSettle(operation) => {
            let redemption = ledger
                .vault_bridge_redemptions
                .iter()
                .find(|redemption| redemption.redemption_id == operation.redemption_id)
                .ok_or_else(|| "vault bridge lifecycle redemption is missing".to_string())?;
            if redemption.asset_id != operation.asset_id {
                return Err("vault bridge settlement redemption asset mismatch".to_string());
            }
            let mut target = bucket_target(&operation.asset_id, &redemption.bucket_id, false)?;
            target.withdrawal_packet = Some(redemption.withdrawal_packet.clone());
            target
        }
        AssetTransactionOperation::VaultBridgeBucketImpair(operation) => {
            let target = bucket_target(&operation.asset_id, &operation.bucket_id, false)?;
            if target.policy_hash != operation.policy_hash {
                return Err(
                    "vault bridge impairment policy does not match pinned bucket".to_string(),
                );
            }
            target
        }
        AssetTransactionOperation::VaultBridgeNavSubscriptionAllocate(operation) => bucket_target(
            &operation.settlement_asset_id,
            &operation.settlement_bucket_id,
            true,
        )?,
        _ => return Ok(None),
    };
    Ok(Some(target))
}

pub(super) fn bridge_verification_activation_height_for_chain(
    genesis: &Genesis,
    governance: &GovernanceState,
) -> Option<u64> {
    governance
        .bridge_verification_activation_height()
        .or(genesis.bridge_verification_activation_height)
}

pub(super) fn atomic_swap_activation_height_for_chain(
    genesis: &Genesis,
    governance: &GovernanceState,
) -> Option<u64> {
    governance
        .atomic_swap_activation_height()
        .or(genesis.atomic_swap_activation_height)
}

pub(super) fn bridge_exit_root_activation_height_for_chain(
    governance: &GovernanceState,
) -> Option<u64> {
    governance.bridge_exit_root_activation_height()
}

pub(super) fn asset_execution_compatibility_for_genesis_and_governance(
    genesis: &Genesis,
    governance: &GovernanceState,
) -> AssetExecutionCompatibility {
    AssetExecutionCompatibility::strict()
        .with_bridge_verification_activation_height(
            bridge_verification_activation_height_for_chain(genesis, governance),
        )
        .with_atomic_swap_activation_height(atomic_swap_activation_height_for_chain(
            genesis, governance,
        ))
        .with_atomic_swap_paused(governance.atomic_swap_paused)
}

pub(super) fn asset_execution_compatibility_with_chain_activation(
    compatibility: AssetExecutionCompatibility,
    genesis: &Genesis,
    governance: &GovernanceState,
) -> AssetExecutionCompatibility {
    compatibility
        .with_bridge_verification_activation_height(
            bridge_verification_activation_height_for_chain(genesis, governance),
        )
        .with_atomic_swap_activation_height(atomic_swap_activation_height_for_chain(
            genesis, governance,
        ))
        .with_atomic_swap_paused(governance.atomic_swap_paused)
}

pub(super) fn ensure_atomic_swap_batch_allowed(
    batch: &TransactionBatch,
    block_height: u64,
    compatibility: AssetExecutionCompatibility,
) -> io::Result<()> {
    if batch.atomic_swap_transactions.is_empty() {
        return Ok(());
    }
    if compatibility.atomic_swap_paused {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "atomic_swap_paused: atomic-swap-bearing batches are disabled by governance",
        ));
    }
    if !compatibility.atomic_swap_active(block_height) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "atomic_swap_not_active: atomic-swap-bearing batch is invalid at height {block_height}"
            ),
        ));
    }
    Ok(())
}

pub(super) fn execute_transparent_batch_for_archive_replay(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    batch: &TransactionBatch,
    block: &BlockRecord,
    governance: &GovernanceState,
) -> io::Result<Vec<Receipt>> {
    if block.receipt_ids.len() != batch.transaction_count() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "block {} receipt id count {} does not match transparent transaction count {}",
                block.header.height,
                block.receipt_ids.len(),
                batch.transaction_count()
            ),
        ));
    }

    let compatibility =
        asset_execution_compatibility_for_genesis_and_governance(genesis, governance);
    ensure_atomic_swap_batch_allowed(batch, block.header.height, compatibility)?;

    let mut receipts = Vec::with_capacity(batch.transaction_count());
    for transfer in &batch.transactions {
        receipts.push(execute_transfer(genesis, ledger, transfer));
    }
    for payment in &batch.payments_v2 {
        receipts.push(execute_payment_v2(genesis, ledger, payment));
    }
    for transaction in &batch.asset_transactions {
        let receipt_index = receipts.len();
        receipts.push(execute_asset_transaction_for_archive_replay(
            genesis,
            ledger,
            transaction,
            block,
            receipt_index,
            governance,
        )?);
    }
    for transaction in &batch.atomic_swap_transactions {
        receipts.push(execute_atomic_swap_transaction_with_compatibility(
            genesis,
            ledger,
            transaction,
            block.header.height,
            compatibility,
        ));
    }
    for transaction in &batch.fastlane_primary_transactions {
        receipts.push(execute_fastlane_primary_for_chain(
            genesis,
            ledger,
            transaction,
            block.header.height,
        ));
    }
    for transaction in &batch.escrow_transactions {
        receipts.push(execute_escrow_transaction(
            genesis,
            ledger,
            transaction,
            block.header.height,
        ));
    }
    for transaction in &batch.nft_transactions {
        receipts.push(execute_nft_transaction(genesis, ledger, transaction));
    }
    for transaction in &batch.offer_transactions {
        receipts.push(execute_offer_transaction(
            genesis,
            ledger,
            transaction,
            block.header.height,
        ));
    }
    Ok(receipts)
}

pub(super) fn execute_asset_transaction_for_archive_replay(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    transaction: &SignedAssetTransaction,
    block: &BlockRecord,
    receipt_index: usize,
    governance: &GovernanceState,
) -> io::Result<Receipt> {
    if archived_wan_devnet_legacy_nav_profile_id_allowed(genesis, block) {
        let compatibility = if archived_wan_devnet_legacy_domainless_withdrawal_packet_emit_allowed(
            genesis,
            block,
            transaction,
        ) {
            AssetExecutionCompatibility::wan_devnet_legacy_replay()
        } else {
            AssetExecutionCompatibility::wan_devnet_legacy_nav_replay()
        };
        let compatibility =
            asset_execution_compatibility_with_chain_activation(compatibility, genesis, governance);
        if let Some(signing_bytes) =
            verified_legacy_wan_asset_transaction_signing_bytes(transaction)
        {
            let Some(receipt_id) = block.receipt_ids.get(receipt_index) else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "block {} missing receipt id for legacy asset transaction at index {receipt_index}",
                        block.header.height
                    ),
                ));
            };
            return Ok(
                execute_asset_transaction_with_replay_preimage_and_compatibility(
                    genesis,
                    ledger,
                    transaction,
                    block.header.height,
                    receipt_id.clone(),
                    &signing_bytes,
                    compatibility,
                ),
            );
        }
        return Ok(execute_asset_transaction_with_replay_compatibility(
            genesis,
            ledger,
            transaction,
            block.header.height,
            compatibility,
        ));
    }
    if archived_wan_devnet_legacy_cash_omitted_sp1_nav_allowed(genesis, block, transaction) {
        let compatibility = asset_execution_compatibility_with_chain_activation(
            AssetExecutionCompatibility::wan_devnet_legacy_cash_omitted_sp1_replay(),
            genesis,
            governance,
        );
        return Ok(execute_asset_transaction_with_replay_compatibility(
            genesis,
            ledger,
            transaction,
            block.header.height,
            compatibility,
        ));
    }

    if let Some(receipt) =
        governed_vault_bridge_route_rejection(transaction, governance, ledger, block.header.height)
    {
        return Ok(receipt);
    }

    Ok(execute_asset_transaction_with_compatibility(
        genesis,
        ledger,
        transaction,
        block.header.height,
        asset_execution_compatibility_for_genesis_and_governance(genesis, governance),
    ))
}

pub(super) fn execute_governance_batch(
    governance: &mut GovernanceState,
    mut ledger: Option<&mut LedgerState>,
    batch: &GovernanceActionBatch,
    block_height: u64,
) -> Vec<Receipt> {
    let mut receipts = Vec::with_capacity(
        batch.amendments.len()
            + batch.validator_registry_updates.len()
            + batch.governance_agent_dry_runs.len()
            + batch.fastswap_bootstraps.len()
            + batch.fastpay_recovery_bootstraps.len()
            + batch.vault_bridge_route_profile_activations.len(),
    );
    for amendment in &batch.amendments {
        if let Some((code, message)) =
            governance_amendment_lifecycle_rejection(amendment, block_height)
        {
            receipts.push(Receipt::rejected(
                amendment.amendment_id.clone(),
                code,
                message,
            ));
        } else if governance
            .amendments
            .iter()
            .any(|existing| existing.amendment_id == amendment.amendment_id)
        {
            receipts.push(Receipt::rejected(
                amendment.amendment_id.clone(),
                "duplicate_amendment",
                "governance amendment already applied",
            ));
        } else {
            apply_governance_amendment_with_lifecycle_records(
                governance,
                amendment.clone(),
                &batch.batch_id,
                block_height,
            );
            receipts.push(Receipt::accepted(
                amendment.amendment_id.clone(),
                "governance amendment applied",
            ));
        }
    }
    for update in &batch.validator_registry_updates {
        if governance
            .validator_registry_updates
            .iter()
            .any(|existing| existing.update_id == update.update_id)
        {
            receipts.push(Receipt::rejected(
                update.update_id.clone(),
                "duplicate_validator_registry_update",
                "validator registry update already applied",
            ));
        } else {
            governance.validator_registry_updates.push(update.clone());
            receipts.push(Receipt::accepted(
                update.update_id.clone(),
                "validator registry update recorded",
            ));
        }
    }
    for dry_run in &batch.governance_agent_dry_runs {
        if let Some((code, message)) = governance_agent_dry_run_rejection(governance, dry_run) {
            receipts.push(Receipt::rejected(dry_run.dry_run_id.clone(), code, message));
        } else {
            let record = governance_agent_dry_run_record(
                dry_run,
                &batch.batch_id,
                block_height,
                governance
                    .governance_agent_dry_run_records
                    .last()
                    .map(|record| record.dry_run_id.clone())
                    .unwrap_or_default(),
            );
            governance.governance_agent_dry_run_records.push(record);
            receipts.push(Receipt::accepted(
                dry_run.dry_run_id.clone(),
                "governance agent dry run recorded",
            ));
        }
    }
    for bootstrap in &batch.fastswap_bootstraps {
        let amendment = &bootstrap.amendment;
        if let Some((code, message)) =
            governance_amendment_lifecycle_rejection(amendment, block_height)
        {
            receipts.push(Receipt::rejected(
                amendment.amendment_id.clone(),
                code,
                message,
            ));
        } else if governance
            .amendments
            .iter()
            .any(|existing| existing.amendment_id == amendment.amendment_id)
        {
            receipts.push(Receipt::rejected(
                amendment.amendment_id.clone(),
                "duplicate_fastswap_bootstrap",
                "FastSwap governance bootstrap already applied",
            ));
        } else if let Some(ledger) = ledger.as_deref_mut() {
            match postfiat_execution::fastswap_control::execute_fastswap_governance_bootstrap(
                ledger,
                bootstrap,
                block_height,
            ) {
                Ok(()) => {
                    apply_governance_amendment_with_lifecycle_records(
                        governance,
                        amendment.clone(),
                        &batch.batch_id,
                        block_height,
                    );
                    receipts.push(Receipt::accepted(
                        amendment.amendment_id.clone(),
                        "FastSwap governance bootstrap applied",
                    ));
                }
                Err(error) => receipts.push(Receipt::rejected(
                    amendment.amendment_id.clone(),
                    "fastswap_bootstrap_rejected",
                    format!("FastSwap governance bootstrap rejected: {error:?}"),
                )),
            }
        } else {
            receipts.push(Receipt::rejected(
                amendment.amendment_id.clone(),
                "fastswap_bootstrap_ledger_unavailable",
                "FastSwap governance bootstrap requires canonical ledger context",
            ));
        }
    }
    for bootstrap in &batch.fastpay_recovery_bootstraps {
        let amendment = &bootstrap.amendment;
        if let Some((code, message)) =
            governance_amendment_lifecycle_rejection(amendment, block_height)
        {
            receipts.push(Receipt::rejected(
                amendment.amendment_id.clone(),
                code,
                message,
            ));
        } else if governance
            .amendments
            .iter()
            .any(|existing| existing.amendment_id == amendment.amendment_id)
        {
            receipts.push(Receipt::rejected(
                amendment.amendment_id.clone(),
                "duplicate_fastpay_recovery_bootstrap",
                "FastPay recovery governance bootstrap already applied",
            ));
        } else if let Some(ledger) = ledger.as_deref_mut() {
            match postfiat_execution::execute_fastpay_recovery_governance_update_v1(
                ledger,
                bootstrap,
                block_height,
            ) {
                Ok(outcome) => {
                    apply_governance_amendment_with_lifecycle_records(
                        governance,
                        amendment.clone(),
                        &batch.batch_id,
                        block_height,
                    );
                    let (code, message) = match outcome {
                        postfiat_execution::FastPayRecoveryGovernanceOutcomeV1::Bootstrapped => (
                            "fastpay_recovery_bootstrap_applied",
                            "FastPay recovery governance bootstrap applied",
                        ),
                        postfiat_execution::FastPayRecoveryGovernanceOutcomeV1::CommitteeRotated => (
                            "fastpay_recovery_committee_rotated",
                            "FastPay recovery committee rotation applied",
                        ),
                    };
                    receipts.push(
                        Receipt::accepted(amendment.amendment_id.clone(), message).with_code(code),
                    );
                }
                Err(error) => receipts.push(Receipt::rejected(
                    amendment.amendment_id.clone(),
                    "fastpay_recovery_bootstrap_rejected",
                    format!("FastPay recovery bootstrap rejected: {error}"),
                )),
            }
        } else {
            receipts.push(Receipt::rejected(
                amendment.amendment_id.clone(),
                "fastpay_recovery_bootstrap_ledger_unavailable",
                "FastPay recovery bootstrap requires canonical ledger context",
            ));
        }
    }
    for activation in &batch.vault_bridge_route_profile_activations {
        let amendment = &activation.amendment;
        let validated = (|| -> Result<postfiat_types::VaultBridgeRouteProfileRecordV1, String> {
            activation.validate()?;
            if governance
                .vault_bridge_route_authority_activation_height()
                .is_none_or(|height| block_height < height)
            {
                return Err("vault bridge route authority is not active".to_string());
            }
            if let Some((code, message)) =
                governance_amendment_lifecycle_rejection(amendment, block_height)
            {
                return Err(format!("{code}: {message}"));
            }
            if activation.profile.activation_height != block_height {
                return Err(
                    "vault bridge route profile must be committed at its activation height"
                        .to_string(),
                );
            }
            if governance
                .amendments
                .iter()
                .any(|existing| existing.amendment_id == amendment.amendment_id)
            {
                return Err("vault bridge route amendment already applied".to_string());
            }
            let profile_hash = activation.profile.profile_hash()?;
            if governance
                .vault_bridge_route_profiles
                .iter()
                .any(|existing| existing.profile_hash == profile_hash)
            {
                return Err("vault bridge route profile already recorded".to_string());
            }
            if governance
                .vault_bridge_route_profiles
                .iter()
                .filter(|existing| existing.profile.asset_id == activation.profile.asset_id)
                .any(|existing| {
                    existing.profile.route_epoch >= activation.profile.route_epoch
                        || existing.profile.activation_height
                            >= activation.profile.activation_height
                })
            {
                return Err(
                    "vault bridge route profile epoch and activation must advance monotonically"
                        .to_string(),
                );
            }
            let ledger = ledger.as_deref().ok_or_else(|| {
                "vault bridge route activation requires canonical ledger context".to_string()
            })?;
            validate_vault_bridge_route_profile_against_ledger(
                ledger,
                &activation.profile,
                &profile_hash,
            )?;
            postfiat_types::VaultBridgeRouteProfileRecordV1::new(activation, block_height)
        })();
        match validated {
            Ok(record) => {
                apply_governance_amendment_with_lifecycle_records(
                    governance,
                    amendment.clone(),
                    &batch.batch_id,
                    block_height,
                );
                governance.vault_bridge_route_profiles.push(record);
                receipts.push(Receipt::accepted(
                    amendment.amendment_id.clone(),
                    "vault bridge route profile activated",
                ));
            }
            Err(error) => receipts.push(Receipt::rejected(
                amendment.amendment_id.clone(),
                "vault_bridge_route_profile_rejected",
                error,
            )),
        }
    }
    receipts
}

pub(super) fn validate_vault_bridge_route_profile_against_ledger(
    ledger: &LedgerState,
    route: &postfiat_types::VaultBridgeRouteProfileV1,
    route_hash: &str,
) -> Result<(), String> {
    route.validate()?;
    if route.profile_hash()? != route_hash {
        return Err("vault bridge route profile hash mismatch".to_string());
    }
    let nav_asset = ledger
        .nav_asset(&route.asset_id)
        .ok_or_else(|| "vault bridge route asset is missing from the NAV registry".to_string())?;
    let profile = ledger
        .nav_proof_profile(&nav_asset.proof_profile)
        .ok_or_else(|| "vault bridge route NAV proof profile is missing".to_string())?;
    let expected_source_class = format!("vault_bridge:{}", route.source_domain());
    let effective_route_policy_hash = if profile.vault_bridge_route_policy_hash.is_empty() {
        &profile.valuation_policy_hash
    } else {
        &profile.vault_bridge_route_policy_hash
    };
    let verifier_contract_matches =
        if route.verifier_kind == postfiat_types::NAV_PROFILE_VERIFIER_SP1_GROTH16 {
            profile.valuation_policy_hash == route.verifier_policy_hash
                && profile.sp1_program_vkey == route.verifier_program_vkey
                && profile.sp1_proof_encoding == route.verifier_proof_encoding
                && profile.max_proof_bytes == route.max_proof_bytes
                && profile.max_public_values_bytes == route.max_public_values_bytes
        } else {
            profile.valuation_policy_hash == route_hash
                && profile.sp1_program_vkey.is_empty()
                && profile.sp1_proof_encoding.is_empty()
                && profile.max_proof_bytes == 0
                && profile.max_public_values_bytes == 0
        };
    if profile.source_class != expected_source_class
        || effective_route_policy_hash != route_hash
        || profile.verifier_kind != route.verifier_kind
        || profile.max_snapshot_age_blocks != route.max_snapshot_age_blocks
        || profile.challenge_window_blocks != route.challenge_window_blocks
        || profile.max_epoch_gap_blocks != route.max_epoch_gap_blocks
        || profile.settle_deadline_blocks != route.settle_deadline_blocks
        || profile.min_challenge_bond != route.min_challenge_bond
        || profile.min_attestations != route.min_attestations
        || profile.bridge_observer_min_confirmations != route.minimum_confirmations
        || !verifier_contract_matches
    {
        return Err(
            "vault bridge route does not exactly match the active NAV proof profile".to_string(),
        );
    }
    Ok(())
}

pub(super) fn governance_amendment_lifecycle_rejection(
    amendment: &GovernanceAmendment,
    block_height: u64,
) -> Option<(&'static str, String)> {
    if amendment.kind == GOVERNANCE_KIND_ORCHARD_POOL_PAUSE && amendment.value > 1 {
        return Some((
            "invalid_orchard_pool_pause_value",
            "orchard pool pause amendment value must be 0 or 1".to_string(),
        ));
    }
    if amendment.kind == GOVERNANCE_KIND_ATOMIC_SWAP_PAUSE && amendment.value > 1 {
        return Some((
            "invalid_atomic_swap_pause_value",
            "atomic swap pause amendment value must be 0 or 1".to_string(),
        ));
    }
    if amendment.kind == GOVERNANCE_KIND_REPLICATED_STATE_V2_ACTIVATION_HEIGHT
        && u64::from(amendment.value) <= block_height
    {
        return Some((
            "invalid_replicated_state_v2_activation_height",
            "replicated-state-v2 activation must be scheduled strictly after the amendment block"
                .to_string(),
        ));
    }
    if amendment.kind == GOVERNANCE_KIND_BRIDGE_EXIT_ROOT_ACTIVATION_HEIGHT
        && u64::from(amendment.value) <= block_height
    {
        return Some((
            "invalid_bridge_exit_root_activation_height",
            "bridge-exit-root activation must be scheduled strictly after the amendment block"
                .to_string(),
        ));
    }
    if amendment.paused {
        return Some((
            "governance_amendment_paused",
            "governance amendment is paused".to_string(),
        ));
    }
    if amendment.veto_until_height > 0 && block_height <= amendment.veto_until_height {
        return Some((
            "governance_amendment_veto_window",
            format!(
                "governance amendment veto window is active until height {}",
                amendment.veto_until_height
            ),
        ));
    }
    if amendment.activation_height > 0 && block_height < amendment.activation_height {
        return Some((
            "governance_amendment_activation_pending",
            format!(
                "governance amendment activation height {} is not reached at height {block_height}",
                amendment.activation_height
            ),
        ));
    }
    None
}

pub(super) fn ensure_governance_batch_lifecycle_ready(
    batch: &GovernanceActionBatch,
    block_height: u64,
) -> io::Result<()> {
    for amendment in batch
        .amendments
        .iter()
        .chain(
            batch
                .fastswap_bootstraps
                .iter()
                .map(|bootstrap| &bootstrap.amendment),
        )
        .chain(
            batch
                .fastpay_recovery_bootstraps
                .iter()
                .map(|bootstrap| &bootstrap.amendment),
        )
        .chain(
            batch
                .vault_bridge_route_profile_activations
                .iter()
                .map(|activation| &activation.amendment),
        )
    {
        if let Some((code, message)) =
            governance_amendment_lifecycle_rejection(amendment, block_height)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{code}: {message}"),
            ));
        }
    }
    Ok(())
}

pub(super) fn apply_governance_amendment_with_lifecycle_records(
    governance: &mut GovernanceState,
    amendment: GovernanceAmendment,
    batch_id: &str,
    activated_height: u64,
) {
    let previous_value = governance_amendment_current_value(governance, &amendment.kind);
    let supersession_record = governance_amendment_supersession_record(
        governance,
        &amendment,
        batch_id,
        activated_height,
        previous_value,
    );
    let rollback_record = governance_amendment_rollback_record(
        governance,
        &amendment,
        batch_id,
        activated_height,
        previous_value,
    );
    let record = governance_amendment_activation_record(
        governance,
        &amendment,
        batch_id,
        activated_height,
        previous_value,
    );
    governance.apply(amendment);
    governance.amendment_activation_records.push(record);
    if let Some(record) = supersession_record {
        governance.amendment_supersession_records.push(record);
    }
    if let Some(record) = rollback_record {
        governance.amendment_rollback_records.push(record);
    }
}

pub(super) fn governance_amendment_current_value(governance: &GovernanceState, kind: &str) -> u32 {
    match kind {
        GOVERNANCE_KIND_VALIDATOR_SET => governance.active_validator_count,
        GOVERNANCE_KIND_CRYPTO_POLICY => governance.crypto_policy_version,
        GOVERNANCE_KIND_BRIDGE_WITNESS_EPOCH => governance.bridge_witness_epoch,
        GOVERNANCE_KIND_AUTHORITY_MODE => governance.authority_mode,
        GOVERNANCE_KIND_ORCHARD_POOL_PAUSE => u32::from(governance.orchard_pool_paused),
        GOVERNANCE_KIND_ATOMIC_SWAP_PAUSE => u32::from(governance.atomic_swap_paused),
        GOVERNANCE_KIND_BRIDGE_VERIFICATION_ACTIVATION_HEIGHT => governance
            .bridge_verification_activation_height()
            .and_then(|height| u32::try_from(height).ok())
            .unwrap_or(0),
        GOVERNANCE_KIND_ATOMIC_SWAP_ACTIVATION_HEIGHT => governance
            .atomic_swap_activation_height()
            .and_then(|height| u32::try_from(height).ok())
            .unwrap_or(0),
        GOVERNANCE_KIND_REPLICATED_STATE_V2_ACTIVATION_HEIGHT => governance
            .replicated_state_v2_activation_height()
            .and_then(|height| u32::try_from(height).ok())
            .unwrap_or(0),
        GOVERNANCE_KIND_BRIDGE_EXIT_ROOT_ACTIVATION_HEIGHT => governance
            .bridge_exit_root_activation_height()
            .and_then(|height| u32::try_from(height).ok())
            .unwrap_or(0),
        _ => 0,
    }
}

pub(super) fn governance_agent_dry_run_rejection(
    governance: &GovernanceState,
    dry_run: &GovernanceAgentDryRunAmendment,
) -> Option<(&'static str, String)> {
    if let Err(error) = validate_governance_agent_dry_run_amendment(dry_run) {
        return Some(("invalid_governance_agent_dry_run", error.to_string()));
    }
    if governance
        .governance_agent_dry_run_records
        .iter()
        .any(|record| record.dry_run_id == dry_run.dry_run_id)
    {
        return Some((
            "duplicate_governance_agent_dry_run",
            "governance agent dry run already recorded".to_string(),
        ));
    }
    let latest = governance
        .governance_agent_dry_run_records
        .last()
        .map(|record| record.dry_run_id.as_str())
        .unwrap_or_default();
    if dry_run.expected_previous_dry_run_id != latest {
        return Some((
            "stale_governance_agent_dry_run",
            "governance agent dry run does not extend latest recorded dry run".to_string(),
        ));
    }
    None
}

pub(super) fn governance_agent_dry_run_record(
    dry_run: &GovernanceAgentDryRunAmendment,
    batch_id: &str,
    recorded_height: u64,
    previous_dry_run_id: String,
) -> GovernanceAgentDryRunRecord {
    let mut record = GovernanceAgentDryRunRecord {
        schema: GOVERNANCE_AGENT_DRY_RUN_RECORD_SCHEMA.to_string(),
        record_id: String::new(),
        dry_run_id: dry_run.dry_run_id.clone(),
        chain_id: dry_run.chain_id.clone(),
        genesis_hash: dry_run.genesis_hash.clone(),
        protocol_version: dry_run.protocol_version,
        batch_id: batch_id.to_string(),
        recorded_height,
        action_mode: dry_run.action_mode.clone(),
        previous_dry_run_id,
        bundle_hash: dry_run.bundle_hash.clone(),
        architecture_statement_hash: dry_run.architecture_statement_hash.clone(),
        objective_statement_hash: dry_run.objective_statement_hash.clone(),
        ruleset_hash: dry_run.ruleset_hash.clone(),
        compiled_policy_hash: dry_run.compiled_policy_hash.clone(),
        replay_bundle_root: dry_run.replay_bundle_root.clone(),
        replay_bundle_uri: dry_run.replay_bundle_uri.clone(),
        report_root: dry_run.report_root.clone(),
        report_uri: dry_run.report_uri.clone(),
        validator_registry_root_before: dry_run.validator_registry_root_before.clone(),
        validator_registry_root_after: dry_run.validator_registry_root_after.clone(),
        registry_mutation_count: dry_run.registry_mutation_count,
    };
    record.record_id = governance_agent_dry_run_record_id(&record);
    record
}

pub(super) fn validate_governance_agent_dry_run_amendment(
    dry_run: &GovernanceAgentDryRunAmendment,
) -> io::Result<()> {
    if dry_run.schema != GOVERNANCE_AGENT_DRY_RUN_AMENDMENT_SCHEMA {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance agent dry-run amendment schema mismatch",
        ));
    }
    validate_chain_bound_id("governance agent dry_run_id", &dry_run.dry_run_id)?;
    validate_chain_id_text("governance agent chain_id", &dry_run.chain_id)?;
    validate_lower_hex_96("governance agent genesis_hash", &dry_run.genesis_hash)?;
    if dry_run.protocol_version == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance agent dry-run protocol_version must be nonzero",
        ));
    }
    if dry_run.action_mode != GOVERNANCE_AGENT_ACTION_MODE_DRY_RUN_VALIDATE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance agent dry-run action_mode must be DryRunValidate",
        ));
    }
    if !dry_run.expected_previous_dry_run_id.is_empty() {
        validate_chain_bound_id(
            "governance agent expected_previous_dry_run_id",
            &dry_run.expected_previous_dry_run_id,
        )?;
    }
    for (label, value) in [
        ("bundle_hash", dry_run.bundle_hash.as_str()),
        (
            "architecture_statement_hash",
            dry_run.architecture_statement_hash.as_str(),
        ),
        (
            "objective_statement_hash",
            dry_run.objective_statement_hash.as_str(),
        ),
        (
            "ruleset_source_bundle_hash",
            dry_run.ruleset_source_bundle_hash.as_str(),
        ),
        ("ruleset_hash", dry_run.ruleset_hash.as_str()),
        (
            "compiled_policy_ruleset_hash",
            dry_run.compiled_policy_ruleset_hash.as_str(),
        ),
        (
            "compiled_policy_hash",
            dry_run.compiled_policy_hash.as_str(),
        ),
        ("replay_bundle_root", dry_run.replay_bundle_root.as_str()),
        ("report_root", dry_run.report_root.as_str()),
        (
            "validator_registry_root_before",
            dry_run.validator_registry_root_before.as_str(),
        ),
        (
            "validator_registry_root_after",
            dry_run.validator_registry_root_after.as_str(),
        ),
    ] {
        validate_lower_hex_96(&format!("governance agent {label}"), value)?;
    }
    validate_chain_id_text(
        "governance agent replay_bundle_uri",
        &dry_run.replay_bundle_uri,
    )?;
    validate_chain_id_text("governance agent report_uri", &dry_run.report_uri)?;
    if dry_run.bundle_hash != dry_run.ruleset_source_bundle_hash {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance agent dry-run ruleset source bundle hash mismatch",
        ));
    }
    if dry_run.ruleset_hash != dry_run.compiled_policy_ruleset_hash {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance agent dry-run compiled policy ruleset hash mismatch",
        ));
    }
    if dry_run.validator_registry_root_before != dry_run.validator_registry_root_after {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance agent dry-run cannot change validator registry root",
        ));
    }
    if dry_run.registry_mutation_count != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance agent dry-run registry_mutation_count must be zero",
        ));
    }
    if dry_run.dry_run_id != governance_agent_dry_run_amendment_id(dry_run) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance agent dry-run id mismatch",
        ));
    }
    Ok(())
}

pub(super) fn validate_governance_agent_dry_run_record(
    expected_previous_dry_run_id: &str,
    record: &GovernanceAgentDryRunRecord,
) -> io::Result<()> {
    if record.schema != GOVERNANCE_AGENT_DRY_RUN_RECORD_SCHEMA {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance agent dry-run record schema mismatch",
        ));
    }
    validate_chain_bound_id("governance agent dry-run record id", &record.record_id)?;
    validate_chain_bound_id("governance agent dry_run_id", &record.dry_run_id)?;
    if !record.previous_dry_run_id.is_empty() {
        validate_chain_bound_id(
            "governance agent previous_dry_run_id",
            &record.previous_dry_run_id,
        )?;
    }
    if record.previous_dry_run_id != expected_previous_dry_run_id {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance agent dry-run record lineage mismatch",
        ));
    }
    if record.action_mode != GOVERNANCE_AGENT_ACTION_MODE_DRY_RUN_VALIDATE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance agent dry-run record action_mode must be DryRunValidate",
        ));
    }
    validate_chain_id_text("governance agent dry-run record chain_id", &record.chain_id)?;
    validate_lower_hex_96(
        "governance agent dry-run record genesis_hash",
        &record.genesis_hash,
    )?;
    if record.protocol_version == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance agent dry-run record protocol_version must be nonzero",
        ));
    }
    validate_chain_id_text("governance agent dry-run record batch_id", &record.batch_id)?;
    for (label, value) in [
        ("bundle_hash", record.bundle_hash.as_str()),
        (
            "architecture_statement_hash",
            record.architecture_statement_hash.as_str(),
        ),
        (
            "objective_statement_hash",
            record.objective_statement_hash.as_str(),
        ),
        ("ruleset_hash", record.ruleset_hash.as_str()),
        ("compiled_policy_hash", record.compiled_policy_hash.as_str()),
        ("replay_bundle_root", record.replay_bundle_root.as_str()),
        ("report_root", record.report_root.as_str()),
        (
            "validator_registry_root_before",
            record.validator_registry_root_before.as_str(),
        ),
        (
            "validator_registry_root_after",
            record.validator_registry_root_after.as_str(),
        ),
    ] {
        validate_lower_hex_96(&format!("governance agent dry-run record {label}"), value)?;
    }
    validate_chain_id_text(
        "governance agent dry-run record replay_bundle_uri",
        &record.replay_bundle_uri,
    )?;
    validate_chain_id_text(
        "governance agent dry-run record report_uri",
        &record.report_uri,
    )?;
    if record.validator_registry_root_before != record.validator_registry_root_after {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance agent dry-run record changed validator registry root",
        ));
    }
    if record.registry_mutation_count != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance agent dry-run record mutation count must be zero",
        ));
    }
    if record.record_id != governance_agent_dry_run_record_id(record) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance agent dry-run record id mismatch",
        ));
    }
    Ok(())
}

pub(super) fn governance_agent_dry_run_amendment_id(
    dry_run: &GovernanceAgentDryRunAmendment,
) -> String {
    let payload = format!(
        "schema={}\nchain_id={}\ngenesis_hash={}\nprotocol_version={}\naction_mode={}\nexpected_previous_dry_run_id={}\nbundle_hash={}\narchitecture_statement_hash={}\nobjective_statement_hash={}\nruleset_source_bundle_hash={}\nruleset_hash={}\ncompiled_policy_ruleset_hash={}\ncompiled_policy_hash={}\nreplay_bundle_root={}\nreplay_bundle_uri={}\nreport_root={}\nreport_uri={}\nvalidator_registry_root_before={}\nvalidator_registry_root_after={}\nregistry_mutation_count={}\n",
        dry_run.schema,
        dry_run.chain_id,
        dry_run.genesis_hash,
        dry_run.protocol_version,
        dry_run.action_mode,
        dry_run.expected_previous_dry_run_id,
        dry_run.bundle_hash,
        dry_run.architecture_statement_hash,
        dry_run.objective_statement_hash,
        dry_run.ruleset_source_bundle_hash,
        dry_run.ruleset_hash,
        dry_run.compiled_policy_ruleset_hash,
        dry_run.compiled_policy_hash,
        dry_run.replay_bundle_root,
        dry_run.replay_bundle_uri,
        dry_run.report_root,
        dry_run.report_uri,
        dry_run.validator_registry_root_before,
        dry_run.validator_registry_root_after,
        dry_run.registry_mutation_count,
    );
    hash_hex(
        "postfiat.governance_agent_dry_run_amendment.v1",
        payload.as_bytes(),
    )
}

pub(super) fn governance_agent_dry_run_record_id(record: &GovernanceAgentDryRunRecord) -> String {
    let payload = format!(
        "schema={}\ndry_run_id={}\nchain_id={}\ngenesis_hash={}\nprotocol_version={}\nbatch_id={}\nrecorded_height={}\naction_mode={}\nprevious_dry_run_id={}\nbundle_hash={}\narchitecture_statement_hash={}\nobjective_statement_hash={}\nruleset_hash={}\ncompiled_policy_hash={}\nreplay_bundle_root={}\nreplay_bundle_uri={}\nreport_root={}\nreport_uri={}\nvalidator_registry_root_before={}\nvalidator_registry_root_after={}\nregistry_mutation_count={}\n",
        record.schema,
        record.dry_run_id,
        record.chain_id,
        record.genesis_hash,
        record.protocol_version,
        record.batch_id,
        record.recorded_height,
        record.action_mode,
        record.previous_dry_run_id,
        record.bundle_hash,
        record.architecture_statement_hash,
        record.objective_statement_hash,
        record.ruleset_hash,
        record.compiled_policy_hash,
        record.replay_bundle_root,
        record.replay_bundle_uri,
        record.report_root,
        record.report_uri,
        record.validator_registry_root_before,
        record.validator_registry_root_after,
        record.registry_mutation_count,
    );
    hash_hex(
        "postfiat.governance_agent_dry_run_record.v1",
        payload.as_bytes(),
    )
}

pub(super) fn governance_amendment_activation_record(
    governance: &GovernanceState,
    amendment: &GovernanceAmendment,
    batch_id: &str,
    activated_height: u64,
    previous_value: u32,
) -> GovernanceAmendmentActivationRecord {
    let mut record = GovernanceAmendmentActivationRecord {
        schema: GOVERNANCE_AMENDMENT_ACTIVATION_SCHEMA.to_string(),
        activation_record_id: String::new(),
        amendment_id: amendment.amendment_id.clone(),
        chain_id: amendment.chain_id.clone(),
        genesis_hash: amendment.genesis_hash.clone(),
        protocol_version: amendment.protocol_version,
        batch_id: batch_id.to_string(),
        kind: amendment.kind.clone(),
        value: amendment.value,
        previous_value,
        new_value: amendment.value,
        activation_height: amendment.activation_height,
        veto_until_height: amendment.veto_until_height,
        activated_height,
    };
    record.activation_record_id = governance_amendment_activation_record_id(&record);

    let expected_previous = governance_amendment_current_value(governance, &record.kind);
    debug_assert_eq!(record.previous_value, expected_previous);
    record
}

pub(super) fn governance_amendment_activation_record_id(
    record: &GovernanceAmendmentActivationRecord,
) -> String {
    let payload = format!(
        "schema={}\namendment_id={}\nchain_id={}\ngenesis_hash={}\nprotocol_version={}\nbatch_id={}\nkind={}\nvalue={}\nprevious_value={}\nnew_value={}\nactivation_height={}\nveto_until_height={}\nactivated_height={}\n",
        record.schema,
        record.amendment_id,
        record.chain_id,
        record.genesis_hash,
        record.protocol_version,
        record.batch_id,
        record.kind,
        record.value,
        record.previous_value,
        record.new_value,
        record.activation_height,
        record.veto_until_height,
        record.activated_height,
    );
    hash_hex(
        "postfiat.governance_amendment_activation_record.v1",
        payload.as_bytes(),
    )
}

pub(super) fn governance_amendment_supersession_record(
    governance: &GovernanceState,
    amendment: &GovernanceAmendment,
    batch_id: &str,
    supersession_height: u64,
    previous_value: u32,
) -> Option<GovernanceAmendmentSupersessionRecord> {
    let superseded = governance
        .amendments
        .iter()
        .rev()
        .find(|existing| existing.kind == amendment.kind)?;
    let mut record = GovernanceAmendmentSupersessionRecord {
        schema: GOVERNANCE_AMENDMENT_SUPERSESSION_SCHEMA.to_string(),
        supersession_record_id: String::new(),
        superseded_amendment_id: superseded.amendment_id.clone(),
        superseding_amendment_id: amendment.amendment_id.clone(),
        chain_id: amendment.chain_id.clone(),
        genesis_hash: amendment.genesis_hash.clone(),
        protocol_version: amendment.protocol_version,
        batch_id: batch_id.to_string(),
        kind: amendment.kind.clone(),
        previous_value,
        new_value: amendment.value,
        supersession_height,
    };
    record.supersession_record_id = governance_amendment_supersession_record_id(&record);
    Some(record)
}

pub(super) fn governance_amendment_supersession_record_id(
    record: &GovernanceAmendmentSupersessionRecord,
) -> String {
    let payload = format!(
        "schema={}\nsuperseded_amendment_id={}\nsuperseding_amendment_id={}\nchain_id={}\ngenesis_hash={}\nprotocol_version={}\nbatch_id={}\nkind={}\nprevious_value={}\nnew_value={}\nsupersession_height={}\n",
        record.schema,
        record.superseded_amendment_id,
        record.superseding_amendment_id,
        record.chain_id,
        record.genesis_hash,
        record.protocol_version,
        record.batch_id,
        record.kind,
        record.previous_value,
        record.new_value,
        record.supersession_height,
    );
    hash_hex(
        "postfiat.governance_amendment_supersession_record.v1",
        payload.as_bytes(),
    )
}

pub(super) fn governance_amendment_rollback_record(
    governance: &GovernanceState,
    amendment: &GovernanceAmendment,
    batch_id: &str,
    rollback_height: u64,
    previous_value: u32,
) -> Option<GovernanceAmendmentRollbackRecord> {
    let rolled_back = governance
        .amendments
        .iter()
        .rev()
        .find(|existing| existing.kind == amendment.kind)?;
    let restored = governance
        .amendments
        .iter()
        .rev()
        .skip_while(|existing| existing.amendment_id != rolled_back.amendment_id)
        .skip(1)
        .find(|existing| existing.kind == amendment.kind && existing.value == amendment.value)?;
    let mut record = GovernanceAmendmentRollbackRecord {
        schema: GOVERNANCE_AMENDMENT_ROLLBACK_SCHEMA.to_string(),
        rollback_record_id: String::new(),
        rolled_back_amendment_id: rolled_back.amendment_id.clone(),
        restored_amendment_id: restored.amendment_id.clone(),
        rollback_amendment_id: amendment.amendment_id.clone(),
        chain_id: amendment.chain_id.clone(),
        genesis_hash: amendment.genesis_hash.clone(),
        protocol_version: amendment.protocol_version,
        batch_id: batch_id.to_string(),
        kind: amendment.kind.clone(),
        previous_value,
        restored_value: amendment.value,
        rollback_height,
    };
    record.rollback_record_id = governance_amendment_rollback_record_id(&record);
    Some(record)
}

pub(super) fn governance_amendment_rollback_record_id(
    record: &GovernanceAmendmentRollbackRecord,
) -> String {
    let payload = format!(
        "schema={}\nrolled_back_amendment_id={}\nrestored_amendment_id={}\nrollback_amendment_id={}\nchain_id={}\ngenesis_hash={}\nprotocol_version={}\nbatch_id={}\nkind={}\nprevious_value={}\nrestored_value={}\nrollback_height={}\n",
        record.schema,
        record.rolled_back_amendment_id,
        record.restored_amendment_id,
        record.rollback_amendment_id,
        record.chain_id,
        record.genesis_hash,
        record.protocol_version,
        record.batch_id,
        record.kind,
        record.previous_value,
        record.restored_value,
        record.rollback_height,
    );
    hash_hex(
        "postfiat.governance_amendment_rollback_record.v1",
        payload.as_bytes(),
    )
}

pub(super) fn verify_governance_amendment_activation_records(
    genesis: &Genesis,
    governance: &GovernanceState,
) -> io::Result<()> {
    if governance.amendments.len() != governance.amendment_activation_records.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance amendment activation record count mismatch",
        ));
    }

    let mut seen_record_ids = HashSet::new();
    let mut seen_amendment_ids = HashSet::new();
    let mut replay = GovernanceState::new(genesis.validator_count);
    for (index, amendment) in governance.amendments.iter().enumerate() {
        let record = &governance.amendment_activation_records[index];
        if record.amendment_id != amendment.amendment_id {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "governance amendment activation record order mismatch for {}",
                    amendment.amendment_id
                ),
            ));
        }
        verify_governance_amendment_activation_record(
            &replay,
            amendment,
            record,
            governance.validator_registry_updates.is_empty(),
        )?;
        if !seen_record_ids.insert(record.activation_record_id.as_str()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "duplicate governance amendment activation record id",
            ));
        }
        if !seen_amendment_ids.insert(record.amendment_id.as_str()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "duplicate governance amendment activation record amendment id",
            ));
        }
        replay.apply(amendment.clone());
    }

    let active_validator_count_mismatch = governance.validator_registry_updates.is_empty()
        && replay.active_validator_count != governance.active_validator_count;
    if active_validator_count_mismatch
        || replay.crypto_policy_version != governance.crypto_policy_version
        || replay.bridge_witness_epoch != governance.bridge_witness_epoch
        || replay.authority_mode != governance.authority_mode
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance amendment activation replay does not match governance state",
        ));
    }
    Ok(())
}

pub(super) fn verify_governance_amendment_activation_record(
    replay: &GovernanceState,
    amendment: &GovernanceAmendment,
    record: &GovernanceAmendmentActivationRecord,
    can_replay_validator_set_previous_value: bool,
) -> io::Result<()> {
    if record.schema != GOVERNANCE_AMENDMENT_ACTIVATION_SCHEMA {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance amendment activation record schema mismatch",
        ));
    }
    if record.activation_record_id != governance_amendment_activation_record_id(record) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance amendment activation record id mismatch",
        ));
    }
    if record.amendment_id != amendment.amendment_id
        || record.chain_id != amendment.chain_id
        || record.genesis_hash != amendment.genesis_hash
        || record.protocol_version != amendment.protocol_version
        || record.kind != amendment.kind
        || record.value != amendment.value
        || record.new_value != amendment.value
        || record.activation_height != amendment.activation_height
        || record.veto_until_height != amendment.veto_until_height
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance amendment activation record does not match amendment",
        ));
    }
    if record.batch_id.trim().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance amendment activation record batch id is empty",
        ));
    }
    let can_replay_previous_value =
        amendment.kind != GOVERNANCE_KIND_VALIDATOR_SET || can_replay_validator_set_previous_value;
    if can_replay_previous_value
        && record.previous_value != governance_amendment_current_value(replay, &amendment.kind)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance amendment activation record previous value mismatch",
        ));
    }
    if amendment.paused {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "paused governance amendment has an activation record",
        ));
    }
    if amendment.activation_height > 0 && record.activated_height < amendment.activation_height {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance amendment activation record is before activation height",
        ));
    }
    if amendment.veto_until_height > 0 && record.activated_height <= amendment.veto_until_height {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance amendment activation record is inside veto window",
        ));
    }
    Ok(())
}

pub(super) fn verify_governance_amendment_supersession_records(
    genesis: &Genesis,
    governance: &GovernanceState,
) -> io::Result<()> {
    let expected = expected_governance_amendment_supersessions(&governance.amendments);
    if expected.len() != governance.amendment_supersession_records.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance amendment supersession record count mismatch",
        ));
    }

    let mut seen_record_ids = HashSet::new();
    let mut seen_superseding_ids = HashSet::new();
    for (index, (superseded, superseding)) in expected.iter().enumerate() {
        let record = &governance.amendment_supersession_records[index];
        verify_governance_amendment_supersession_record(
            genesis,
            superseded,
            superseding,
            record,
            governance.validator_registry_updates.is_empty(),
        )?;
        if !seen_record_ids.insert(record.supersession_record_id.as_str()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "duplicate governance amendment supersession record id",
            ));
        }
        if !seen_superseding_ids.insert(record.superseding_amendment_id.as_str()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "duplicate governance amendment supersession record superseding amendment id",
            ));
        }
    }
    Ok(())
}

pub(super) fn expected_governance_amendment_supersessions(
    amendments: &[GovernanceAmendment],
) -> Vec<(&GovernanceAmendment, &GovernanceAmendment)> {
    let mut expected = Vec::new();
    for (index, amendment) in amendments.iter().enumerate() {
        if let Some(previous_index) = amendments[..index]
            .iter()
            .rposition(|previous| previous.kind == amendment.kind)
        {
            expected.push((&amendments[previous_index], amendment));
        }
    }
    expected
}

pub(super) fn verify_governance_amendment_supersession_record(
    genesis: &Genesis,
    superseded: &GovernanceAmendment,
    superseding: &GovernanceAmendment,
    record: &GovernanceAmendmentSupersessionRecord,
    can_replay_validator_set_previous_value: bool,
) -> io::Result<()> {
    let domain = cobalt_domain(genesis);
    verify_governance_amendment_supersession_record_for_domain(
        &domain,
        superseded,
        superseding,
        record,
        can_replay_validator_set_previous_value,
    )
}

pub(super) fn verify_governance_amendment_supersession_record_for_domain(
    domain: &CobaltDomain,
    superseded: &GovernanceAmendment,
    superseding: &GovernanceAmendment,
    record: &GovernanceAmendmentSupersessionRecord,
    can_replay_validator_set_previous_value: bool,
) -> io::Result<()> {
    if record.schema != GOVERNANCE_AMENDMENT_SUPERSESSION_SCHEMA {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance amendment supersession record schema mismatch",
        ));
    }
    if record.supersession_record_id != governance_amendment_supersession_record_id(record) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance amendment supersession record id mismatch",
        ));
    }
    if record.superseded_amendment_id != superseded.amendment_id
        || record.superseding_amendment_id != superseding.amendment_id
        || record.chain_id != superseding.chain_id
        || record.genesis_hash != superseding.genesis_hash
        || record.protocol_version != superseding.protocol_version
        || record.kind != superseding.kind
        || record.kind != superseded.kind
        || record.new_value != superseding.value
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance amendment supersession record does not match amendments",
        ));
    }
    let can_replay_previous_value =
        record.kind != GOVERNANCE_KIND_VALIDATOR_SET || can_replay_validator_set_previous_value;
    if can_replay_previous_value && record.previous_value != superseded.value {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance amendment supersession record previous value mismatch",
        ));
    }
    if record.chain_id != domain.chain_id
        || record.genesis_hash != domain.genesis_hash
        || record.protocol_version != domain.protocol_version
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance amendment supersession record domain mismatch",
        ));
    }
    if record.batch_id.trim().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance amendment supersession record batch id is empty",
        ));
    }
    if superseding.paused {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "paused governance amendment has a supersession record",
        ));
    }
    if superseding.activation_height > 0
        && record.supersession_height < superseding.activation_height
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance amendment supersession record is before activation height",
        ));
    }
    if superseding.veto_until_height > 0
        && record.supersession_height <= superseding.veto_until_height
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance amendment supersession record is inside veto window",
        ));
    }
    Ok(())
}

pub(super) fn verify_governance_amendment_rollback_records(
    genesis: &Genesis,
    governance: &GovernanceState,
) -> io::Result<()> {
    let expected = expected_governance_amendment_rollbacks(&governance.amendments);
    if expected.len() != governance.amendment_rollback_records.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance amendment rollback record count mismatch",
        ));
    }

    let mut seen_record_ids = HashSet::new();
    let mut seen_rollback_ids = HashSet::new();
    for (index, (rolled_back, restored, rollback)) in expected.iter().enumerate() {
        let record = &governance.amendment_rollback_records[index];
        verify_governance_amendment_rollback_record(
            genesis,
            rolled_back,
            restored,
            rollback,
            record,
            governance.validator_registry_updates.is_empty(),
        )?;
        if !seen_record_ids.insert(record.rollback_record_id.as_str()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "duplicate governance amendment rollback record id",
            ));
        }
        if !seen_rollback_ids.insert(record.rollback_amendment_id.as_str()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "duplicate governance amendment rollback record rollback amendment id",
            ));
        }
    }
    Ok(())
}

pub(super) fn expected_governance_amendment_rollbacks(
    amendments: &[GovernanceAmendment],
) -> Vec<(
    &GovernanceAmendment,
    &GovernanceAmendment,
    &GovernanceAmendment,
)> {
    let mut expected = Vec::new();
    for (index, rollback) in amendments.iter().enumerate() {
        let previous = &amendments[..index];
        let Some(rolled_back_index) = previous
            .iter()
            .rposition(|existing| existing.kind == rollback.kind)
        else {
            continue;
        };
        let Some(restored_index) = previous[..rolled_back_index].iter().rposition(|existing| {
            existing.kind == rollback.kind && existing.value == rollback.value
        }) else {
            continue;
        };
        expected.push((
            &amendments[rolled_back_index],
            &amendments[restored_index],
            rollback,
        ));
    }
    expected
}

pub(super) fn verify_governance_amendment_rollback_record(
    genesis: &Genesis,
    rolled_back: &GovernanceAmendment,
    restored: &GovernanceAmendment,
    rollback: &GovernanceAmendment,
    record: &GovernanceAmendmentRollbackRecord,
    can_replay_validator_set_previous_value: bool,
) -> io::Result<()> {
    let domain = cobalt_domain(genesis);
    verify_governance_amendment_rollback_record_for_domain(
        &domain,
        rolled_back,
        restored,
        rollback,
        record,
        can_replay_validator_set_previous_value,
    )
}

pub(super) fn verify_governance_amendment_rollback_record_for_domain(
    domain: &CobaltDomain,
    rolled_back: &GovernanceAmendment,
    restored: &GovernanceAmendment,
    rollback: &GovernanceAmendment,
    record: &GovernanceAmendmentRollbackRecord,
    can_replay_validator_set_previous_value: bool,
) -> io::Result<()> {
    if record.schema != GOVERNANCE_AMENDMENT_ROLLBACK_SCHEMA {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance amendment rollback record schema mismatch",
        ));
    }
    if record.rollback_record_id != governance_amendment_rollback_record_id(record) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance amendment rollback record id mismatch",
        ));
    }
    if record.rolled_back_amendment_id != rolled_back.amendment_id
        || record.restored_amendment_id != restored.amendment_id
        || record.rollback_amendment_id != rollback.amendment_id
        || record.chain_id != rollback.chain_id
        || record.genesis_hash != rollback.genesis_hash
        || record.protocol_version != rollback.protocol_version
        || record.kind != rolled_back.kind
        || record.kind != restored.kind
        || record.kind != rollback.kind
        || record.restored_value != restored.value
        || record.restored_value != rollback.value
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance amendment rollback record does not match amendments",
        ));
    }
    let can_replay_previous_value =
        record.kind != GOVERNANCE_KIND_VALIDATOR_SET || can_replay_validator_set_previous_value;
    if can_replay_previous_value && record.previous_value != rolled_back.value {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance amendment rollback record previous value mismatch",
        ));
    }
    if record.chain_id != domain.chain_id
        || record.genesis_hash != domain.genesis_hash
        || record.protocol_version != domain.protocol_version
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance amendment rollback record domain mismatch",
        ));
    }
    if record.batch_id.trim().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance amendment rollback record batch id is empty",
        ));
    }
    if rollback.paused {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "paused governance amendment has a rollback record",
        ));
    }
    if rollback.activation_height > 0 && record.rollback_height < rollback.activation_height {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance amendment rollback record is before activation height",
        ));
    }
    if rollback.veto_until_height > 0 && record.rollback_height <= rollback.veto_until_height {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance amendment rollback record is inside veto window",
        ));
    }
    Ok(())
}

pub(super) fn execute_shielded_batch(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    shielded: &mut ShieldedState,
    batch: &ShieldedActionBatch,
    block_height: u64,
    asset_execution_compatibility: AssetExecutionCompatibility,
    orchard_pool_paused: bool,
    archive_replay: bool,
) -> Vec<Receipt> {
    if orchard_pool_paused {
        return batch
            .actions
            .iter()
            .enumerate()
            .map(|(index, _)| {
                Receipt::rejected(
                    shielded_action_rejection_id(&batch.batch_id, index, "orchard_pool_paused"),
                    "orchard_pool_paused",
                    "shielded actions are disabled by the active governance pause",
                )
            })
            .collect();
    }
    let mut receipts = Vec::with_capacity(batch.actions.len());
    let genesis_hash_hex = genesis_hash(genesis);
    let debug_pool_enabled =
        debug_shielded_pool_enabled_for_chain(&genesis.chain_id, &genesis_hash_hex);
    for (index, action) in batch.actions.iter().enumerate() {
        let receipt = match action {
            ShieldedAction::Mint(action) => {
                if !archive_replay {
                    legacy_cleartext_shielded_action_disabled_receipt(
                        &batch.batch_id,
                        index,
                        "mint",
                    )
                } else if !debug_pool_enabled {
                    debug_shielded_pool_disabled_receipt(
                        &batch.batch_id,
                        index,
                        "mint",
                        &genesis.chain_id,
                    )
                } else {
                    match mint_debug_note_with_creator_for_chain(
                        shielded,
                        postfiat_privacy::ShieldedChainContext {
                            chain_id: &genesis.chain_id,
                            genesis_hash: &genesis_hash_hex,
                        },
                        action.owner.clone(),
                        action.asset_id.clone(),
                        action.amount,
                        action.memo.clone(),
                        ordered_shielded_mint_creator(&batch.batch_id, index),
                    ) {
                        Ok(note) => Receipt::accepted(note.note_id, "shielded mint action applied"),
                        Err(error) => Receipt::rejected(
                            shielded_action_rejection_id(&batch.batch_id, index, error.code()),
                            error.code(),
                            error.to_string(),
                        ),
                    }
                }
            }
            ShieldedAction::Spend(action) => {
                if !archive_replay {
                    legacy_cleartext_shielded_action_disabled_receipt(
                        &batch.batch_id,
                        index,
                        "spend",
                    )
                } else if !debug_pool_enabled {
                    debug_shielded_pool_disabled_receipt(
                        &batch.batch_id,
                        index,
                        "spend",
                        &genesis.chain_id,
                    )
                } else {
                    match spend_debug_note_for_chain(
                        shielded,
                        &genesis.chain_id,
                        &genesis_hash_hex,
                        &action.note_id,
                        action.to.clone(),
                        action.amount,
                        action.memo.clone(),
                    ) {
                        Ok(result) => {
                            Receipt::accepted(result.spend_id, "shielded spend action applied")
                        }
                        Err(error) => Receipt::rejected(
                            shielded_action_rejection_id(&batch.batch_id, index, error.code()),
                            error.code(),
                            error.to_string(),
                        ),
                    }
                }
            }
            ShieldedAction::Migrate(action) => {
                if !debug_pool_enabled {
                    debug_shielded_pool_disabled_receipt(
                        &batch.batch_id,
                        index,
                        "migrate",
                        &genesis.chain_id,
                    )
                } else {
                    match migrate_debug_note(
                        shielded,
                        &action.note_id,
                        action.target_pool.clone(),
                        action.memo.clone(),
                    ) {
                        Ok(event) => Receipt::accepted(
                            event.event_id,
                            "shielded turnstile migration recorded",
                        ),
                        Err(error) => Receipt::rejected(
                            shielded_action_rejection_id(&batch.batch_id, index, error.code()),
                            error.code(),
                            error.to_string(),
                        ),
                    }
                }
            }
            ShieldedAction::OrchardV1(action) => {
                execute_orchard_shielded_action(genesis, shielded, &batch.batch_id, index, action)
            }
            ShieldedAction::OrchardWithdrawV1(action) => execute_orchard_withdraw_shielded_action(
                genesis,
                ledger,
                shielded,
                &batch.batch_id,
                index,
                action,
            ),
            ShieldedAction::OrchardDepositV1(action) => execute_orchard_deposit_shielded_action(
                genesis,
                ledger,
                shielded,
                &batch.batch_id,
                index,
                action,
            ),
            ShieldedAction::ShieldedSwapV1(action) => execute_shielded_swap_action(
                genesis,
                ledger,
                shielded,
                &batch.batch_id,
                block_height,
                index,
                action,
                archive_replay,
            ),
            ShieldedAction::AssetOrchardIngressV1(action) => {
                if archive_replay {
                    execute_asset_orchard_ingress_action(
                        genesis,
                        ledger,
                        shielded,
                        &batch.batch_id,
                        index,
                        action,
                        block_height,
                        asset_execution_compatibility,
                    )
                } else {
                    Receipt::rejected(
                        shielded_action_rejection_id(
                            &batch.batch_id,
                            index,
                            "asset_orchard_ingress_v1_privacy_disabled",
                        ),
                        "asset_orchard_ingress_v1_privacy_disabled",
                        "AssetOrchard ingress v1 exposes the note opening and is historical-replay-only",
                    )
                }
            }
            ShieldedAction::AssetOrchardIngressV2(action) => {
                execute_asset_orchard_ingress_v2_action(
                    genesis,
                    ledger,
                    shielded,
                    &batch.batch_id,
                    index,
                    action,
                    block_height,
                    asset_execution_compatibility,
                )
            }
            ShieldedAction::AssetOrchardEgressV1(action) => execute_asset_orchard_egress_action(
                genesis,
                ledger,
                shielded,
                &batch.batch_id,
                index,
                action,
            ),
            ShieldedAction::AssetOrchardPrivateEgressV1(action) => {
                execute_asset_orchard_private_egress_action(
                    genesis,
                    ledger,
                    shielded,
                    &batch.batch_id,
                    block_height,
                    index,
                    action,
                    archive_replay,
                )
            }
        };
        receipts.push(receipt);
    }
    receipts
}

pub(super) fn debug_shielded_pool_disabled_receipt(
    batch_id: &str,
    index: usize,
    action_kind: &str,
    chain_id: &str,
) -> Receipt {
    Receipt::rejected(
        shielded_action_rejection_id(batch_id, index, "debug_shielded_pool_disabled"),
        "debug_shielded_pool_disabled",
        format!("debug shielded {action_kind} action is disabled for chain `{chain_id}`"),
    )
}

fn legacy_cleartext_shielded_action_disabled_receipt(
    batch_id: &str,
    index: usize,
    action_kind: &str,
) -> Receipt {
    Receipt::rejected(
        shielded_action_rejection_id(batch_id, index, "legacy_cleartext_shielded_action_disabled"),
        "legacy_cleartext_shielded_action_disabled",
        format!(
            "legacy cleartext shielded {action_kind} is historical-replay-only; use Asset-Orchard"
        ),
    )
}

pub(super) fn execute_orchard_shielded_action(
    genesis: &Genesis,
    shielded: &mut ShieldedState,
    batch_id: &str,
    index: usize,
    payload: &OrchardActionPayload,
) -> Receipt {
    if payload.action_json.trim().is_empty() {
        return Receipt::rejected(
            shielded_action_rejection_id(batch_id, index, "empty_orchard_action"),
            "empty_orchard_action",
            "Orchard action JSON is empty",
        );
    }
    if payload.action_json.len() as u64 > MAX_LOCAL_JSON_FILE_BYTES {
        return Receipt::rejected(
            shielded_action_rejection_id(batch_id, index, "orchard_action_too_large"),
            "orchard_action_too_large",
            format!("Orchard action JSON exceeds {MAX_LOCAL_JSON_FILE_BYTES} bytes"),
        );
    }

    let action = match serde_json::from_str::<OrchardShieldedAction>(&payload.action_json) {
        Ok(action) => action,
        Err(error) => {
            return Receipt::rejected(
                shielded_action_rejection_id(batch_id, index, "invalid_orchard_action_json"),
                "invalid_orchard_action_json",
                format!("Orchard action JSON parse failed: {error}"),
            );
        }
    };
    let domain = match orchard_authorizing_domain(genesis, &action.pool_id) {
        Ok(domain) => domain,
        Err(error) => {
            return Receipt::rejected(
                shielded_action_rejection_id(batch_id, index, "invalid_orchard_domain"),
                "invalid_orchard_domain",
                error.to_string(),
            );
        }
    };
    let verified = match verify_serialized_orchard_action_with_built_key(&action, &domain) {
        Ok(verified) => verified,
        Err(error) => {
            return Receipt::rejected(
                shielded_action_rejection_id(batch_id, index, error.code()),
                error.code(),
                error.to_string(),
            );
        }
    };
    match apply_verified_orchard_action_to_shielded_state(genesis, shielded, &action, &verified) {
        Ok(receipt) => receipt,
        Err(error) => Receipt::rejected(
            shielded_action_rejection_id(batch_id, index, "orchard_apply_error"),
            "orchard_apply_error",
            error.to_string(),
        ),
    }
}

pub(super) fn execute_orchard_withdraw_shielded_action(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    shielded: &mut ShieldedState,
    batch_id: &str,
    index: usize,
    payload: &OrchardWithdrawActionPayload,
) -> Receipt {
    if let Err(error) = validate_orchard_withdraw_payload(payload) {
        return Receipt::rejected(
            shielded_action_rejection_id(batch_id, index, "orchard_withdraw_bad_payload"),
            "orchard_withdraw_bad_payload",
            error.to_string(),
        );
    }
    if payload.action_json.trim().is_empty() {
        return Receipt::rejected(
            shielded_action_rejection_id(batch_id, index, "empty_orchard_action"),
            "empty_orchard_action",
            "Orchard withdraw action JSON is empty",
        );
    }
    if payload.action_json.len() as u64 > MAX_LOCAL_JSON_FILE_BYTES {
        return Receipt::rejected(
            shielded_action_rejection_id(batch_id, index, "orchard_action_too_large"),
            "orchard_action_too_large",
            format!("Orchard withdraw action JSON exceeds {MAX_LOCAL_JSON_FILE_BYTES} bytes"),
        );
    }

    let action = match serde_json::from_str::<OrchardShieldedAction>(&payload.action_json) {
        Ok(action) => action,
        Err(error) => {
            return Receipt::rejected(
                shielded_action_rejection_id(batch_id, index, "invalid_orchard_action_json"),
                "invalid_orchard_action_json",
                format!("Orchard withdraw action JSON parse failed: {error}"),
            );
        }
    };
    let domain = match orchard_authorizing_domain(genesis, &action.pool_id) {
        Ok(domain) => domain,
        Err(error) => {
            return Receipt::rejected(
                shielded_action_rejection_id(batch_id, index, "invalid_orchard_domain"),
                "invalid_orchard_domain",
                error.to_string(),
            );
        }
    };
    let verified = match verify_serialized_orchard_action_with_built_key(&action, &domain) {
        Ok(verified) => verified,
        Err(error) => {
            return Receipt::rejected(
                shielded_action_rejection_id(batch_id, index, error.code()),
                error.code(),
                error.to_string(),
            );
        }
    };
    match apply_verified_orchard_withdraw_action_to_state(
        genesis, ledger, shielded, &action, &verified, payload,
    ) {
        Ok(receipt) => receipt,
        Err(error) => Receipt::rejected(
            shielded_action_rejection_id(batch_id, index, "orchard_apply_error"),
            "orchard_apply_error",
            error.to_string(),
        ),
    }
}

pub(super) fn execute_orchard_deposit_shielded_action(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    shielded: &mut ShieldedState,
    batch_id: &str,
    index: usize,
    payload: &OrchardDepositActionPayload,
) -> Receipt {
    if let Err(error) = validate_orchard_deposit_payload(payload) {
        return Receipt::rejected(
            shielded_action_rejection_id(batch_id, index, "orchard_deposit_bad_payload"),
            "orchard_deposit_bad_payload",
            error.to_string(),
        );
    }
    if payload.action_json.trim().is_empty() {
        return Receipt::rejected(
            shielded_action_rejection_id(batch_id, index, "empty_orchard_action"),
            "empty_orchard_action",
            "Orchard deposit action JSON is empty",
        );
    }
    if payload.action_json.len() as u64 > MAX_LOCAL_JSON_FILE_BYTES {
        return Receipt::rejected(
            shielded_action_rejection_id(batch_id, index, "orchard_action_too_large"),
            "orchard_action_too_large",
            format!("Orchard deposit action JSON exceeds {MAX_LOCAL_JSON_FILE_BYTES} bytes"),
        );
    }

    let action = match serde_json::from_str::<OrchardShieldedAction>(&payload.action_json) {
        Ok(action) => action,
        Err(error) => {
            return Receipt::rejected(
                shielded_action_rejection_id(batch_id, index, "invalid_orchard_action_json"),
                "invalid_orchard_action_json",
                format!("Orchard deposit action JSON parse failed: {error}"),
            );
        }
    };
    let domain = match orchard_authorizing_domain(genesis, &action.pool_id) {
        Ok(domain) => domain,
        Err(error) => {
            return Receipt::rejected(
                shielded_action_rejection_id(batch_id, index, "invalid_orchard_domain"),
                "invalid_orchard_domain",
                error.to_string(),
            );
        }
    };
    let verified = match verify_serialized_orchard_action_with_built_key(&action, &domain) {
        Ok(verified) => verified,
        Err(error) => {
            return Receipt::rejected(
                shielded_action_rejection_id(batch_id, index, error.code()),
                error.code(),
                error.to_string(),
            );
        }
    };
    match apply_verified_orchard_deposit_action_to_state(
        genesis, ledger, shielded, &action, &verified, payload,
    ) {
        Ok(receipt) => receipt,
        Err(error) => Receipt::rejected(
            shielded_action_rejection_id(batch_id, index, "orchard_apply_error"),
            "orchard_apply_error",
            error.to_string(),
        ),
    }
}

pub(super) fn execute_asset_orchard_ingress_action(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    shielded: &mut ShieldedState,
    batch_id: &str,
    index: usize,
    payload: &AssetOrchardIngressActionPayload,
    block_height: u64,
    asset_execution_compatibility: AssetExecutionCompatibility,
) -> Receipt {
    if let Err(error) = validate_asset_orchard_ingress_payload_for_genesis(genesis, payload) {
        return Receipt::rejected(
            shielded_action_rejection_id(batch_id, index, "asset_orchard_ingress_bad_payload"),
            "asset_orchard_ingress_bad_payload",
            error.to_string(),
        );
    }
    match apply_asset_orchard_ingress_action_to_state(
        genesis,
        ledger,
        shielded,
        &asset_orchard_ingress_v1_state_payload(payload),
        block_height,
        asset_execution_compatibility,
    ) {
        Ok(receipt) => receipt,
        Err(error) => Receipt::rejected(
            shielded_action_rejection_id(batch_id, index, "asset_orchard_ingress_apply_error"),
            "asset_orchard_ingress_apply_error",
            error.to_string(),
        ),
    }
}

pub(super) fn execute_asset_orchard_ingress_v2_action(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    shielded: &mut ShieldedState,
    batch_id: &str,
    index: usize,
    payload: &AssetOrchardIngressV2ActionPayload,
    block_height: u64,
    asset_execution_compatibility: AssetExecutionCompatibility,
) -> Receipt {
    if let Err(error) = validate_asset_orchard_ingress_v2_payload(payload) {
        return Receipt::rejected(
            shielded_action_rejection_id(batch_id, index, "asset_orchard_ingress_v2_bad_payload"),
            "asset_orchard_ingress_v2_bad_payload",
            error.to_string(),
        );
    }
    match apply_asset_orchard_ingress_action_to_state(
        genesis,
        ledger,
        shielded,
        &asset_orchard_ingress_v2_state_payload(payload),
        block_height,
        asset_execution_compatibility,
    ) {
        Ok(receipt) => receipt,
        Err(error) => Receipt::rejected(
            shielded_action_rejection_id(batch_id, index, "asset_orchard_ingress_v2_apply_error"),
            "asset_orchard_ingress_v2_apply_error",
            error.to_string(),
        ),
    }
}

pub(super) fn execute_asset_orchard_egress_action(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    shielded: &mut ShieldedState,
    batch_id: &str,
    index: usize,
    payload: &AssetOrchardEgressActionPayload,
) -> Receipt {
    if let Err(error) = validate_asset_orchard_egress_payload_for_genesis(genesis, payload) {
        return Receipt::rejected(
            shielded_action_rejection_id(batch_id, index, "asset_orchard_egress_bad_payload"),
            "asset_orchard_egress_bad_payload",
            error.to_string(),
        );
    }
    match apply_asset_orchard_egress_action_to_state(genesis, ledger, shielded, payload) {
        Ok(receipt) => receipt,
        Err(error) => Receipt::rejected(
            shielded_action_rejection_id(batch_id, index, "asset_orchard_egress_apply_error"),
            "asset_orchard_egress_apply_error",
            error.to_string(),
        ),
    }
}

pub(super) fn execute_asset_orchard_private_egress_action(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    shielded: &mut ShieldedState,
    batch_id: &str,
    block_height: u64,
    index: usize,
    payload: &AssetOrchardPrivateEgressActionPayload,
    archive_replay: bool,
) -> Receipt {
    if let Err(error) = validate_asset_orchard_private_egress_payload(payload) {
        return Receipt::rejected(
            shielded_action_rejection_id(
                batch_id,
                index,
                "asset_orchard_private_egress_bad_payload",
            ),
            "asset_orchard_private_egress_bad_payload",
            error.to_string(),
        );
    }
    let archived_pre_repin = archived_pre_repin_private_egress_execution_allowed(
        archive_replay,
        genesis,
        block_height,
        batch_id,
    );
    match apply_asset_orchard_private_egress_action_to_state(
        genesis,
        ledger,
        shielded,
        payload,
        archived_pre_repin,
        archive_replay,
    ) {
        Ok(receipt) => receipt,
        Err(error) => Receipt::rejected(
            shielded_action_rejection_id(
                batch_id,
                index,
                "asset_orchard_private_egress_apply_error",
            ),
            "asset_orchard_private_egress_apply_error",
            error.to_string(),
        ),
    }
}

pub(super) fn archived_pre_repin_private_egress_execution_allowed(
    archive_replay: bool,
    genesis: &Genesis,
    block_height: u64,
    batch_id: &str,
) -> bool {
    archive_replay
        && archived_wan_devnet2_pre_repin_private_egress_allowed(genesis, block_height, batch_id)
}

pub(super) fn archived_pre_pricing_swap_execution_allowed(
    archive_replay: bool,
    genesis: &Genesis,
    block_height: u64,
    batch_id: &str,
) -> bool {
    archive_replay && archived_wan_devnet2_pre_pricing_swap_allowed(genesis, block_height, batch_id)
}

#[derive(Debug, Deserialize)]
pub(super) struct ArchivedAssetOrchardSwapReplayAction {
    pub(super) pool_id: String,
    pub(super) anchor: AssetOrchardFieldElement,
    pub(super) nullifiers: Vec<AssetOrchardFieldElement>,
    pub(super) output_commitments: Vec<AssetOrchardFieldElement>,
    pub(super) encrypted_outputs: Vec<AssetOrchardBoundedBytes>,
    pub(super) fee: u64,
}

pub(super) fn apply_archived_wan_devnet2_pre_pricing_swap(
    shielded: &mut ShieldedState,
    batch_id: &str,
    index: usize,
    action: ArchivedAssetOrchardSwapReplayAction,
) -> io::Result<Receipt> {
    if action.pool_id != ASSET_ORCHARD_POOL_ID_V1
        || action.fee != 0
        || action.nullifiers.len() != ASSET_ORCHARD_LEG_COUNT
        || action.output_commitments.len() != ASSET_ORCHARD_LEG_COUNT
        || action.encrypted_outputs.len() != ASSET_ORCHARD_LEG_COUNT
        || has_duplicate_strings(action.nullifiers.iter().map(|value| value.as_hex()))
        || has_duplicate_strings(action.output_commitments.iter().map(|value| value.as_hex()))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "allowlisted archived pre-pricing swap has invalid public shape",
        ));
    }

    let action_anchor = action.anchor.as_hex().to_string();
    if let Some(pool) = shielded.orchard.as_ref() {
        if pool.pool_id != action.pool_id {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "allowlisted archived pre-pricing swap pool id mismatch",
            ));
        }
        if action
            .nullifiers
            .iter()
            .any(|value| pool.is_nullified(value.as_hex()))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "allowlisted archived pre-pricing swap repeats a nullifier",
            ));
        }
        if action.output_commitments.iter().any(|value| {
            pool.output_commitments
                .iter()
                .any(|existing| existing == value.as_hex())
        }) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "allowlisted archived pre-pricing swap repeats an output commitment",
            ));
        }
        if !orchard_anchor_is_retained_for_apply(pool, &action_anchor)? {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "allowlisted archived pre-pricing swap anchor is not retained",
            ));
        }
    } else if action_anchor != orchard_empty_root_hex() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "allowlisted archived pre-pricing swap does not use the empty initial root",
        ));
    }

    let shielded_before_apply = shielded.clone();
    let apply_result = (|| -> io::Result<()> {
        let pool = shielded
            .orchard
            .get_or_insert_with(|| OrchardPoolState::empty(action.pool_id));
        ensure_orchard_root_history_for_apply(pool)?;
        pool.nullifiers.extend(
            action
                .nullifiers
                .iter()
                .map(|value| value.as_hex().to_string()),
        );
        pool.output_commitments.extend(
            action
                .output_commitments
                .iter()
                .map(|value| value.as_hex().to_string()),
        );
        for (output_commitment, encrypted_output) in action
            .output_commitments
            .iter()
            .zip(action.encrypted_outputs.iter())
        {
            pool.asset_orchard_outputs
                .push(AssetOrchardEncryptedOutputRecord {
                    output_commitment: output_commitment.as_hex().to_string(),
                    encrypted_output: encrypted_output.as_hex().to_string(),
                });
        }
        if !pool
            .accepted_anchors
            .iter()
            .any(|root| root == &action_anchor)
        {
            pool.accepted_anchors.push(action_anchor);
        }
        append_orchard_current_root(pool)
    })();
    if let Err(error) = apply_result {
        *shielded = shielded_before_apply;
        return Err(error);
    }
    if let Err(error) = verify_shielded_state(shielded) {
        *shielded = shielded_before_apply;
        return Err(error);
    }
    Ok(Receipt::accepted(
        shielded_action_rejection_id(batch_id, index, "archived_pre_pricing_swap_accepted"),
        "asset-orchard swap verified and public pool state updated",
    ))
}

pub(super) fn execute_shielded_swap_action(
    genesis: &Genesis,
    ledger: &LedgerState,
    shielded: &mut ShieldedState,
    batch_id: &str,
    block_height: u64,
    index: usize,
    payload: &ShieldedSwapActionPayload,
    archive_replay: bool,
) -> Receipt {
    if payload.swap_json.trim().is_empty() {
        return Receipt::rejected(
            shielded_action_rejection_id(batch_id, index, "empty_shielded_swap"),
            "empty_shielded_swap",
            "shielded swap JSON is empty",
        );
    }
    if payload.swap_json.len() as u64 > MAX_LOCAL_JSON_FILE_BYTES {
        return Receipt::rejected(
            shielded_action_rejection_id(batch_id, index, "shielded_swap_too_large"),
            "shielded_swap_too_large",
            format!("shielded swap JSON exceeds {MAX_LOCAL_JSON_FILE_BYTES} bytes"),
        );
    }
    if let Ok(action) = serde_json::from_str::<AssetOrchardSwapAction>(&payload.swap_json) {
        let domain = match orchard_authorizing_domain(genesis, &action.pool_id) {
            Ok(domain) => domain,
            Err(error) => {
                return Receipt::rejected(
                    shielded_action_rejection_id(batch_id, index, "invalid_shielded_swap_domain"),
                    "invalid_shielded_swap_domain",
                    error.to_string(),
                );
            }
        };
        let verified_result = if archive_replay {
            verify_serialized_asset_orchard_swap_action_for_archive_replay(&action, &domain)
        } else {
            verify_serialized_asset_orchard_swap_action(&action, &domain)
        };
        let verified = match verified_result {
            Ok(verified) => verified,
            Err(error) => {
                return Receipt::rejected(
                    shielded_action_rejection_id(batch_id, index, error.code()),
                    error.code(),
                    error.to_string(),
                );
            }
        };
        if let Err(error) =
            validate_asset_orchard_swap_pricing_against_ledger(ledger, &verified, block_height)
        {
            return Receipt::rejected(
                shielded_action_rejection_id(batch_id, index, error.code()),
                error.code(),
                error.to_string(),
            );
        }
        return match apply_verified_asset_orchard_swap_action_to_state(
            genesis, shielded, &action, &verified,
        ) {
            Ok(receipt) => receipt,
            Err(error) => Receipt::rejected(
                shielded_action_rejection_id(batch_id, index, "asset_orchard_swap_apply_error"),
                "asset_orchard_swap_apply_error",
                error.to_string(),
            ),
        };
    }

    if archived_pre_pricing_swap_execution_allowed(archive_replay, genesis, block_height, batch_id)
    {
        return match serde_json::from_str::<ArchivedAssetOrchardSwapReplayAction>(
            &payload.swap_json,
        ) {
            Ok(action) => {
                match apply_archived_wan_devnet2_pre_pricing_swap(shielded, batch_id, index, action)
                {
                    Ok(receipt) => receipt,
                    Err(error) => Receipt::rejected(
                        shielded_action_rejection_id(
                            batch_id,
                            index,
                            "archived_pre_pricing_swap_apply_error",
                        ),
                        "archived_pre_pricing_swap_apply_error",
                        error.to_string(),
                    ),
                }
            }
            Err(error) => Receipt::rejected(
                shielded_action_rejection_id(
                    batch_id,
                    index,
                    "invalid_archived_pre_pricing_swap_json",
                ),
                "invalid_archived_pre_pricing_swap_json",
                format!("archived pre-pricing swap JSON parse failed: {error}"),
            ),
        };
    }

    let action = match serde_json::from_str::<ShieldedSwapAction>(&payload.swap_json) {
        Ok(action) => action,
        Err(error) => {
            return Receipt::rejected(
                shielded_action_rejection_id(batch_id, index, "invalid_shielded_swap_json"),
                "invalid_shielded_swap_json",
                format!("shielded swap JSON parse failed: {error}"),
            );
        }
    };
    let domain = match orchard_authorizing_domain(genesis, &action.pool_id) {
        Ok(domain) => domain,
        Err(error) => {
            return Receipt::rejected(
                shielded_action_rejection_id(batch_id, index, "invalid_shielded_swap_domain"),
                "invalid_shielded_swap_domain",
                error.to_string(),
            );
        }
    };
    let verified = match verify_serialized_shielded_swap_action(&action, &domain) {
        Ok(verified) => verified,
        Err(error) => {
            return Receipt::rejected(
                shielded_action_rejection_id(batch_id, index, error.code()),
                error.code(),
                error.to_string(),
            );
        }
    };
    match apply_verified_shielded_swap_action_to_state(genesis, shielded, &action, &verified) {
        Ok(receipt) => receipt,
        Err(error) => Receipt::rejected(
            shielded_action_rejection_id(batch_id, index, "shielded_swap_apply_error"),
            "shielded_swap_apply_error",
            error.to_string(),
        ),
    }
}

pub(super) const ASSET_ORCHARD_NAV_USD_E8_ACTIVATION_HEIGHT: u64 = 608;

pub(super) fn asset_orchard_nav_ratio_denominator(block_height: u64) -> u64 {
    if block_height >= ASSET_ORCHARD_NAV_USD_E8_ACTIVATION_HEIGHT {
        postfiat_types::NAV_USD_E8_UNIT
    } else {
        postfiat_types::VAULT_BRIDGE_UNIT
    }
}

pub(super) fn validate_asset_orchard_swap_pricing_against_ledger(
    ledger: &LedgerState,
    verified: &postfiat_privacy_orchard::VerifiedAssetOrchardSwap,
    block_height: u64,
) -> Result<(), postfiat_privacy_orchard::OrchardVerificationError> {
    use postfiat_privacy_orchard::{
        validate_asset_orchard_pricing_policy, AssetOrchardPricingPolicy, AssetTag,
        OrchardVerificationError,
    };

    let claim = &verified.pricing.claim;
    let base_tag = AssetTag {
        lo: claim.base_asset_tag_lo,
        hi: claim.base_asset_tag_hi,
    };
    let quote_tag = AssetTag {
        lo: claim.quote_asset_tag_lo,
        hi: claim.quote_asset_tag_hi,
    };
    let matching_nav_assets = ledger
        .nav_assets
        .iter()
        .filter(|asset| AssetTag::derive(&asset.asset_id).ok().as_ref() == Some(&base_tag))
        .collect::<Vec<_>>();
    if matching_nav_assets.len() != 1 {
        return Err(OrchardVerificationError::new(
            "asset_orchard_pricing_nav_asset_mismatch",
            "pricing base asset tag must resolve to exactly one ledger NAV asset",
        ));
    }
    let nav_asset = matching_nav_assets[0];
    let quote_matches = ledger
        .asset_definitions
        .iter()
        .filter(|asset| AssetTag::derive(&asset.asset_id).ok().as_ref() == Some(&quote_tag))
        .count();
    if quote_matches != 1 {
        return Err(OrchardVerificationError::new(
            "asset_orchard_pricing_quote_asset_mismatch",
            "pricing quote asset tag must resolve to exactly one ledger asset",
        ));
    }
    let profile = ledger
        .nav_proof_profile(&nav_asset.proof_profile)
        .ok_or_else(|| {
            OrchardVerificationError::new(
                "asset_orchard_pricing_profile_missing",
                "pricing NAV asset references a missing active proof profile",
            )
        })?;
    if nav_asset.finalized_epoch == 0
        || nav_asset.nav_per_unit == 0
        || nav_asset.finalized_reserve_packet_hash.is_empty()
    {
        return Err(OrchardVerificationError::new(
            "asset_orchard_pricing_not_finalized",
            "pricing NAV asset does not have a finalized epoch tuple",
        ));
    }
    let band_bps = u16::try_from(profile.tolerance_bp).map_err(|_| {
        OrchardVerificationError::new(
            "asset_orchard_pricing_band_invalid",
            "active NAV profile tolerance does not fit validator pricing band",
        )
    })?;
    let policy = AssetOrchardPricingPolicy {
        nav_epoch: nav_asset.finalized_epoch,
        reserve_packet_hash: nav_asset.finalized_reserve_packet_hash.clone(),
        nav_ratio_numerator: nav_asset.nav_per_unit,
        nav_ratio_denominator: asset_orchard_nav_ratio_denominator(block_height),
        band_bps,
        base_asset_tag: base_tag,
        quote_asset_tag: quote_tag,
        halted: nav_asset.halted,
    };
    validate_asset_orchard_pricing_policy(&verified.pricing, &policy)
}

pub(super) fn execute_bridge_batch(
    genesis: &Genesis,
    bridge: &mut BridgeState,
    batch: &BridgeActionBatch,
    governed_witness_epoch: u32,
    validator_registry: &ValidatorRegistry,
) -> Vec<Receipt> {
    let mut receipts = Vec::with_capacity(batch.actions.len());
    for (index, action) in batch.actions.iter().enumerate() {
        let receipt = match action {
            BridgeAction::Domain(action) => match upsert_domain_with_metadata(
                bridge,
                BridgeDomainSpec {
                    domain_id: action.domain_id.clone(),
                    name: action.name.clone(),
                    source_chain: action.source_chain.clone(),
                    target_chain: action.target_chain.clone(),
                    bridge_id: action.bridge_id.clone(),
                    door_account: action.door_account.clone(),
                    inbound_cap: action.inbound_cap,
                    outbound_cap: action.outbound_cap,
                },
            ) {
                Ok(domain) => Receipt::accepted(domain.domain_id, "bridge domain action applied"),
                Err(error) => Receipt::rejected(
                    bridge_action_rejection_id(&batch.batch_id, index, error.code()),
                    error.code(),
                    error.to_string(),
                ),
            },
            BridgeAction::Transfer(action) => {
                if action.witness_epoch != governed_witness_epoch {
                    Receipt::rejected(
                        bridge_action_rejection_id(&batch.batch_id, index, "bad_witness_epoch"),
                        "bad_witness_epoch",
                        format!(
                            "bridge witness epoch {} does not match governed epoch {}",
                            action.witness_epoch, governed_witness_epoch
                        ),
                    )
                } else if let Some((code, message)) =
                    bridge_witness_registry_error(action, validator_registry)
                {
                    Receipt::rejected(
                        bridge_action_rejection_id(&batch.batch_id, index, code),
                        code,
                        message,
                    )
                } else if let Some((code, message)) =
                    bridge_witness_chain_domain_error(action, genesis)
                {
                    Receipt::rejected(
                        bridge_action_rejection_id(&batch.batch_id, index, code),
                        code,
                        message,
                    )
                } else {
                    let request = BridgeTransferRequest {
                        domain_id: action.domain_id.clone(),
                        direction: action.direction.clone(),
                        from: action.from.clone(),
                        to: action.to.clone(),
                        asset_id: action.asset_id.clone(),
                        amount: action.amount,
                        witness_id: action.witness_id.clone(),
                        witness_epoch: action.witness_epoch,
                        witness_attestation: action.witness_attestation.clone(),
                    };
                    match apply_simulated_transfer(bridge, request) {
                        Ok(transfer) => Receipt::accepted(
                            transfer.transfer_id,
                            "bridge transfer action applied",
                        ),
                        Err(error) => Receipt::rejected(
                            bridge_action_rejection_id(&batch.batch_id, index, error.code()),
                            error.code(),
                            error.to_string(),
                        ),
                    }
                }
            }
            BridgeAction::Pause(action) => {
                match set_domain_paused(bridge, &action.domain_id, action.paused) {
                    Ok(domain) => Receipt::accepted(
                        domain.domain_id,
                        if action.paused {
                            "bridge pause action applied"
                        } else {
                            "bridge resume action applied"
                        },
                    ),
                    Err(error) => Receipt::rejected(
                        bridge_action_rejection_id(&batch.batch_id, index, error.code()),
                        error.code(),
                        error.to_string(),
                    ),
                }
            }
        };
        receipts.push(receipt);
    }
    receipts
}

pub(super) fn bridge_witness_registry_error(
    action: &BridgeTransferAction,
    validator_registry: &ValidatorRegistry,
) -> Option<(&'static str, String)> {
    let attestation = action.witness_attestation.as_ref()?;
    let registry_record = match validator_registry_record(validator_registry, &attestation.signer) {
        Ok(record) => record,
        Err(_) => {
            return Some((
                "unknown_witness_signer",
                format!(
                    "bridge witness signer `{}` is not in validator registry",
                    attestation.signer
                ),
            ))
        }
    };
    if attestation.algorithm_id != registry_record.algorithm_id {
        return Some((
            "bad_witness_registry",
            format!(
                "bridge witness signer `{}` algorithm does not match validator registry",
                attestation.signer
            ),
        ));
    }
    if attestation.public_key_hex != registry_record.public_key_hex {
        return Some((
            "bad_witness_registry",
            format!(
                "bridge witness signer `{}` public key does not match validator registry",
                attestation.signer
            ),
        ));
    }
    None
}
