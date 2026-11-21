// Consensus transition for `asset_orchard_transfer_v1`: a private payment inside the asset
// pool, optionally with a public amount leaving it. Included into the crate root next to the
// other pool transitions.
//
// The proof establishes that the two spent notes exist and are owned, that all four notes
// carry one asset, and that inputs equal outputs plus the public exit amount. This transition
// adds what only the ledger knows: the anchor is one the pool has had, the nullifiers are
// unspent, and an exit moves exactly its public amount from the pool's count of that asset to
// the destination account.

fn asset_orchard_transfer_receipt_id(
    genesis: &Genesis,
    action: &AssetOrchardTransferAction,
    code: &str,
) -> io::Result<String> {
    direct_receipt_id(
        genesis,
        "postfiat.privacy.asset_orchard_transfer_receipt.v1",
        &(
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

pub(crate) fn execute_asset_orchard_transfer_action(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    shielded: &mut ShieldedState,
    batch_id: &str,
    index: usize,
    payload: &AssetOrchardTransferActionPayload,
) -> Receipt {
    let reject = |code: &'static str, message: String| {
        Receipt::rejected(
            shielded_action_rejection_id(batch_id, index, code),
            code,
            message,
        )
    };
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
    match apply_asset_orchard_transfer_action_to_state(genesis, ledger, shielded, &action) {
        Ok(receipt) => receipt,
        Err(error) => reject("asset_orchard_transfer_apply_error", error.to_string()),
    }
}

fn apply_asset_orchard_transfer_action_to_state(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    shielded: &mut ShieldedState,
    action: &AssetOrchardTransferAction,
) -> io::Result<Receipt> {
    let reject = |code: &str, message: String| -> io::Result<Receipt> {
        Ok(Receipt::rejected(
            asset_orchard_transfer_receipt_id(genesis, action, code)?,
            code,
            message,
        ))
    };
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

    let Some(pool) = shielded.orchard.as_ref() else {
        return reject(
            "asset_orchard_transfer_empty_pool",
            "a transfer spends notes, so it needs an existing pool".to_string(),
        );
    };
    if pool.pool_id != action.pool_id {
        return reject(
            "pool_id_mismatch",
            format!(
                "Orchard pool state is for `{}`, the transfer is for `{}`",
                pool.pool_id, action.pool_id
            ),
        );
    }
    let anchor = action.anchor.as_hex().to_string();
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

    // Everything below changes state. It is done on copies and committed together, so a
    // rejected exit leaves no spent nullifier behind.
    let mut trial_ledger = ledger.clone();
    let mut trial_shielded = shielded.clone();
    if let Some(exit) = &action.exit {
        if let Err((code, message)) = credit_issued_asset_from_shielded_pool(
            &mut trial_ledger,
            &exit.to,
            &exit.asset_id,
            exit.amount,
        ) {
            return reject(code, message);
        }
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
            .as_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing Orchard pool"))?;
        if let Some(exit) = &action.exit {
            if let Err((code, message)) =
                debit_asset_orchard_balance(pool, &exit.asset_id, exit.amount)
            {
                return reject(code, message);
            }
        }
        ensure_orchard_root_history_for_apply(pool)?;
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
    Ok(Receipt::accepted(
        asset_orchard_transfer_receipt_id(genesis, action, "accepted")?,
        if action.exit.is_some() {
            "asset-orchard transfer verified; public exit credited and pool state updated"
        } else {
            "asset-orchard transfer verified and pool state updated"
        },
    ))
}
