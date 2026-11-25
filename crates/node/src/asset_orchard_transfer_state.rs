// Consensus transition for `asset_orchard_transfer_v1`: shielding into the asset pool, a
// private payment inside it, and a public amount leaving it. Included into the crate root next
// to the other pool transitions.
//
// The proof establishes that the spent notes exist and are owned, that all four notes carry one
// asset, and that inputs plus the public entry equal outputs plus the public exit. This
// transition adds what only the ledger knows: the action is active on this chain, the anchor
// is one the pool has had, the nullifiers are unspent, an entry is funded by a burn of exactly
// its asset and amount by its named account, and an exit moves exactly its amount from the
// pool's count of that asset to the destination account.

fn asset_orchard_transfer_receipt_id(
    genesis: &Genesis,
    batch_id: &str,
    index: usize,
    action: &AssetOrchardTransferAction,
    code: &str,
) -> io::Result<String> {
    // The batch and position are part of the id, so the same action refused at two places
    // has two receipts.
    direct_receipt_id(
        genesis,
        "postfiat.privacy.asset_orchard_transfer_receipt.v1",
        &(
            batch_id,
            index as u64,
            action.pool_id.as_str(),
            action.anchor.as_hex(),
            action
                .nullifiers
                .iter()
                .map(|nullifier| nullifier.as_hex())
                .collect::<Vec<_>>(),
            action
                .output_commitments
                .iter()
                .map(|commitment| commitment.as_hex())
                .collect::<Vec<_>>(),
            action.action_binding_hash.as_hex(),
            code,
        ),
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn execute_asset_orchard_transfer_action(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    shielded: &mut ShieldedState,
    batch_id: &str,
    block_height: u64,
    index: usize,
    payload: &AssetOrchardTransferActionPayload,
    asset_execution_compatibility: AssetExecutionCompatibility,
    egress_policies: &postfiat_types::AssetOrchardEgressPolicyRegistry,
) -> Receipt {
    let reject = |code: &'static str, message: String| {
        Receipt::rejected(
            shielded_action_rejection_id(batch_id, index, code),
            code,
            message,
        )
    };
    if !asset_execution_compatibility
        .asset_orchard_transfer_activation_height
        .is_some_and(|activation| block_height >= activation)
    {
        return reject(
            "asset_orchard_transfer_not_active",
            format!(
                "asset_orchard_transfer_v1 is not active on `{}` at height {block_height}",
                genesis.chain_id
            ),
        );
    }
    if payload.transfer_json.len() as u64 > MAX_LOCAL_JSON_FILE_BYTES {
        return reject(
            "asset_orchard_transfer_too_large",
            format!("transfer JSON exceeds {MAX_LOCAL_JSON_FILE_BYTES} bytes"),
        );
    }
    let action = match serde_json::from_str::<AssetOrchardTransferAction>(&payload.transfer_json) {
        Ok(action) => action,
        Err(error) => return reject("asset_orchard_transfer_bad_payload", error.to_string()),
    };
    match apply_asset_orchard_transfer_action_to_state(
        genesis,
        ledger,
        shielded,
        batch_id,
        block_height,
        index,
        &action,
        payload.burn_transaction.as_ref(),
        asset_execution_compatibility,
        egress_policies,
    ) {
        Ok(receipt) => receipt,
        Err(error) => reject("asset_orchard_transfer_apply_error", error.to_string()),
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_asset_orchard_transfer_action_to_state(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    shielded: &mut ShieldedState,
    batch_id: &str,
    block_height: u64,
    index: usize,
    action: &AssetOrchardTransferAction,
    burn_transaction: Option<&SignedAssetTransaction>,
    asset_execution_compatibility: AssetExecutionCompatibility,
    egress_policies: &postfiat_types::AssetOrchardEgressPolicyRegistry,
) -> io::Result<Receipt> {
    let reject = |code: &str, message: String| -> io::Result<Receipt> {
        Ok(Receipt::rejected(
            asset_orchard_transfer_receipt_id(genesis, batch_id, index, action, code)?,
            code,
            message,
        ))
    };

    // Cheap checks first: shape, the ledger's view of anchor and nullifiers, and the funding
    // burn's terms. The proof is verified only for an action that could otherwise apply.
    if let Err(error) = action.validate() {
        return reject(error.code(), error.to_string());
    }
    if action.pool_id != ASSET_ORCHARD_POOL_ID_V1 {
        return reject(
            "pool_id_mismatch",
            format!("the transfer names pool `{}`", action.pool_id),
        );
    }
    let anchor = action.anchor.as_hex().to_string();
    match shielded.orchard.as_ref() {
        Some(pool) => {
            if pool.pool_id != action.pool_id {
                return reject(
                    "pool_id_mismatch",
                    format!(
                        "Orchard pool state is for `{}`, the transfer is for `{}`",
                        pool.pool_id, action.pool_id
                    ),
                );
            }
            if !orchard_anchor_is_retained_for_apply(pool, &anchor)? {
                return reject(
                    "unretained_orchard_anchor",
                    format!("transfer anchor `{anchor}` is not retained by pool state"),
                );
            }
            if let Some(nullifier) = action
                .nullifiers
                .iter()
                .map(|nullifier| nullifier.as_hex())
                .find(|nullifier| pool.is_nullified(nullifier))
            {
                return reject(
                    "duplicate_nullifier",
                    format!("transfer nullifier `{nullifier}` already exists"),
                );
            }
            if let Some(commitment) = action
                .output_commitments
                .iter()
                .map(|commitment| commitment.as_hex())
                .find(|commitment| {
                    pool.output_commitments
                        .iter()
                        .any(|existing| existing == commitment)
                })
            {
                return reject(
                    "duplicate_output_commitment",
                    format!("transfer output commitment `{commitment}` already exists"),
                );
            }
        }
        // The first action of a pool can only be a shield, against the empty tree.
        None if action.entry.is_some() && anchor == orchard_empty_root_hex() => {}
        None => {
            return reject(
                "unretained_orchard_anchor",
                "the pool is empty; its first action must shield against the empty root"
                    .to_string(),
            );
        }
    }
    match (&action.entry, burn_transaction) {
        (Some(entry), Some(burn)) => {
            let terms_match = matches!(
                &burn.unsigned.operation,
                AssetTransactionOperation::AssetBurn(operation)
                    if operation.owner == entry.from
                        && operation.asset_id == entry.asset_id
                        && operation.amount == entry.amount
            );
            if !terms_match
                || burn.unsigned.source != entry.from
                || asset_transaction_tx_id(burn) != entry.burn_transaction_id
            {
                return reject(
                    "asset_orchard_transfer_entry_burn_mismatch",
                    "the attached burn is not the one the transfer's entry names".to_string(),
                );
            }
        }
        (None, None) => {}
        _ => {
            return reject(
                "asset_orchard_transfer_entry_burn_mismatch",
                "a transfer carries a burn exactly when it has an entry".to_string(),
            );
        }
    }
    if let Some(exit) = &action.exit {
        // A transfer exit cannot bind a governed policy or disclosure schema, so it is
        // refused wherever governance requires one for private exits.
        if egress_policies.record_at(&exit.asset_id, block_height).is_some()
            || egress_policies.activated_at(block_height)
        {
            return reject(
                "asset_orchard_transfer_exit_policy_governed",
                format!(
                    "private exits of asset {} are governed at height {block_height}; use the governed private egress",
                    exit.asset_id
                ),
            );
        }
    }

    let domain = orchard_authorizing_domain(genesis, &action.pool_id)?;
    let domain_genesis_hash =
        asset_orchard_domain_genesis_hash(&domain.genesis_hash).map_err(invalid_data)?;
    if let Err(error) = verify_asset_orchard_transfer_action(
        &genesis.chain_id,
        domain_genesis_hash,
        genesis.protocol_version,
        action,
    ) {
        return reject(error.code(), error.to_string());
    }

    // Everything below changes state. It is done on copies and committed together, so a
    // rejected burn or exit leaves nothing behind.
    let mut trial_ledger = ledger.clone();
    let mut trial_shielded = shielded.clone();
    let mut fees = None;
    if let Some(burn) = burn_transaction {
        let burn_receipt = execute_asset_transaction_with_replay_compatibility(
            genesis,
            &mut trial_ledger,
            burn,
            block_height,
            asset_execution_compatibility,
        );
        if !burn_receipt.accepted {
            return Ok(Receipt::rejected(
                asset_orchard_transfer_receipt_id(
                    genesis,
                    batch_id,
                    index,
                    action,
                    "burn_rejected",
                )?,
                "asset_orchard_transfer_burn_rejected",
                format!(
                    "the entry's burn was rejected with {}: {}",
                    burn_receipt.code, burn_receipt.message
                ),
            )
            .with_fee_policy(
                0,
                0,
                burn_receipt.minimum_fee,
                burn_receipt.state_expansion_fee,
            ));
        }
        fees = Some(burn_receipt);
    }
    if let Some(exit) = &action.exit {
        if let Err((code, message)) = credit_issued_asset_from_shielded_pool(
            &mut trial_ledger,
            &exit.to,
            &exit.asset_id,
            exit.amount,
        ) {
            return reject(code, message);
        }
    }
    if action.entry.is_some() || action.exit.is_some() {
        trial_ledger
            .validate_asset_state(&genesis.chain_id)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        trial_ledger
            .validate_nav_state(&genesis.chain_id)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    }
    {
        let pool = trial_shielded
            .orchard
            .get_or_insert_with(|| OrchardPoolState::empty(action.pool_id.clone()));
        ensure_orchard_root_history_for_apply(pool)?;
        if let Some(entry) = &action.entry {
            if let Err((code, message)) =
                credit_asset_orchard_balance(pool, &entry.asset_id, entry.amount)
            {
                return reject(code, message);
            }
        }
        if let Some(exit) = &action.exit {
            if let Err((code, message)) =
                debit_asset_orchard_balance(pool, &exit.asset_id, exit.amount)
            {
                return reject(code, message);
            }
        }
        for nullifier in &action.nullifiers {
            pool.nullifiers.push(nullifier.as_hex().to_string());
        }
        for (commitment, encrypted_output) in action
            .output_commitments
            .iter()
            .zip(action.encrypted_outputs.iter())
        {
            pool.output_commitments
                .push(commitment.as_hex().to_string());
            pool.asset_orchard_outputs
                .push(AssetOrchardEncryptedOutputRecord {
                    output_commitment: commitment.as_hex().to_string(),
                    encrypted_output: encrypted_output.as_hex().to_string(),
                });
        }
        if !pool
            .accepted_anchors
            .iter()
            .any(|existing| existing == &anchor)
        {
            pool.accepted_anchors.push(anchor);
        }
        append_orchard_current_root(pool)?;
    }
    verify_shielded_state(&trial_shielded)?;
    *ledger = trial_ledger;
    *shielded = trial_shielded;
    let receipt = Receipt::accepted(
        asset_orchard_transfer_receipt_id(genesis, batch_id, index, action, "accepted")?,
        match (&action.entry, &action.exit) {
            (Some(_), _) => "asset-orchard transfer verified; public burn shielded into the pool",
            (_, Some(_)) => "asset-orchard transfer verified; public exit credited from the pool",
            _ => "asset-orchard transfer verified and pool state updated",
        },
    );
    Ok(match fees {
        Some(burn) => receipt.with_fee_policy(
            burn.fee_charged,
            burn.fee_burned,
            burn.minimum_fee,
            burn.state_expansion_fee,
        ),
        None => receipt,
    })
}
