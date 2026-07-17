pub fn nft_transfer_issuer_fee_terms(
    ledger: &LedgerState,
    operation: &NftTransferOperation,
) -> Result<(String, u64), (&'static str, String)> {
    let index = nft_index(ledger, &operation.nft_id).ok_or_else(|| {
        (
            "missing_nft",
            format!("nft `{}` does not exist", operation.nft_id),
        )
    })?;
    let nft = &ledger.nfts[index];
    if nft.burned {
        return Err(("nft_burned", "nft is burned".to_string()));
    }
    if nft.owner != operation.from {
        return Err((
            "nft_owner_mismatch",
            "nft_transfer from does not match current owner".to_string(),
        ));
    }
    if nft.collection_flags & NFT_COLLECTION_FLAG_TRANSFER_LOCKED != 0 {
        return Err((
            "nft_collection_transfer_locked",
            "nft collection policy does not allow owner transfer".to_string(),
        ));
    }
    let issuer_transfer_fee = if operation.from == nft.issuer {
        0
    } else {
        nft.issuer_transfer_fee
    };
    Ok((nft.issuer.clone(), issuer_transfer_fee))
}

fn apply_nft_operation(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    transaction: &SignedNftTransaction,
) -> Result<(), (&'static str, String)> {
    match &transaction.unsigned.operation {
        NftTransactionOperation::NftMint(operation) => {
            if transaction.unsigned.transaction_kind != NFT_MINT_TRANSACTION_KIND {
                return Err((
                    "wrong_transaction_kind",
                    "nft_mint transaction kind mismatch".to_string(),
                ));
            }
            if ledger.account(&operation.owner).is_none() {
                return Err((
                    "missing_owner",
                    format!("nft owner `{}` does not exist", operation.owner),
                ));
            }
            let mut nft = NftDefinition::new(
                &genesis.chain_id,
                operation.issuer.clone(),
                operation.collection_id.clone(),
                operation.serial,
                operation.owner.clone(),
                operation.metadata_hash.clone(),
            )
            .map_err(|error| ("bad_nft_definition", error))?;
            nft.metadata_uri = operation.metadata_uri.clone();
            nft.flags = operation.flags;
            nft.collection_flags = operation.collection_flags;
            nft.issuer_transfer_fee = operation.issuer_transfer_fee;
            nft.validate_for_chain(&genesis.chain_id)
                .map_err(|error| ("bad_nft_definition", error))?;
            if ledger.nft(&nft.nft_id).is_some() {
                return Err((
                    "duplicate_nft",
                    format!("nft `{}` already exists", nft.nft_id),
                ));
            }
            if let Some(existing) = ledger.nfts.iter().find(|existing| {
                existing.issuer == nft.issuer && existing.collection_id == nft.collection_id
            }) {
                if existing.collection_flags != nft.collection_flags {
                    return Err((
                        "nft_collection_policy_mismatch",
                        format!(
                            "nft collection `{}` for issuer `{}` already uses collection_flags {}",
                            nft.collection_id, nft.issuer, existing.collection_flags
                        ),
                    ));
                }
            }
            ledger.nfts.push(nft);
            Ok(())
        }
        NftTransactionOperation::NftTransfer(operation) => {
            if transaction.unsigned.transaction_kind != NFT_TRANSFER_TRANSACTION_KIND {
                return Err((
                    "wrong_transaction_kind",
                    "nft_transfer transaction kind mismatch".to_string(),
                ));
            }
            if ledger.account(&operation.to).is_none() {
                return Err((
                    "missing_recipient",
                    format!("nft recipient `{}` does not exist", operation.to),
                ));
            }
            let index = nft_index(ledger, &operation.nft_id).ok_or_else(|| {
                (
                    "missing_nft",
                    format!("nft `{}` does not exist", operation.nft_id),
                )
            })?;
            if ledger.nfts[index].burned {
                return Err(("nft_burned", "nft is burned".to_string()));
            }
            if ledger.nfts[index].owner != operation.from {
                return Err((
                    "nft_owner_mismatch",
                    "nft_transfer from does not match current owner".to_string(),
                ));
            }
            if ledger.nfts[index].flags & NFT_FLAG_TRANSFERABLE == 0 {
                return Err((
                    "nft_not_transferable",
                    "nft does not allow owner transfer".to_string(),
                ));
            }
            if ledger.nfts[index].collection_flags & NFT_COLLECTION_FLAG_TRANSFER_LOCKED != 0 {
                return Err((
                    "nft_collection_transfer_locked",
                    "nft collection policy does not allow owner transfer".to_string(),
                ));
            }
            let (issuer, issuer_transfer_fee) = nft_transfer_issuer_fee_terms(ledger, operation)?;
            if operation.issuer_transfer_fee != issuer_transfer_fee {
                return Err((
                    "nft_issuer_transfer_fee_mismatch",
                    format!("nft_transfer issuer_transfer_fee must be {issuer_transfer_fee}"),
                ));
            }
            if !operation.issuer.is_empty() && operation.issuer != issuer {
                return Err((
                    "nft_issuer_mismatch",
                    "nft_transfer issuer does not match nft issuer".to_string(),
                ));
            }
            if issuer_transfer_fee != 0 && operation.issuer.is_empty() {
                return Err((
                    "nft_issuer_missing",
                    "nft_transfer with issuer_transfer_fee must include issuer".to_string(),
                ));
            }
            if issuer_transfer_fee != 0 {
                let Some(source) = ledger.account(&operation.from) else {
                    return Err((
                        "missing_sender",
                        format!("nft transfer source `{}` does not exist", operation.from),
                    ));
                };
                if source.balance < issuer_transfer_fee {
                    return Err((
                        "insufficient_funds",
                        "source balance is too low for nft issuer transfer fee".to_string(),
                    ));
                }
                let source_after_issuer_fee = source.balance - issuer_transfer_fee;
                if let Some(message) =
                    account_reserve_violation(&operation.from, source_after_issuer_fee)
                {
                    return Err(("below_account_reserve", message));
                }
                if ledger.account(&issuer).is_none() {
                    return Err((
                        "missing_issuer",
                        format!("nft issuer `{issuer}` does not exist"),
                    ));
                }
                let Some(source) = ledger.account_mut(&operation.from) else {
                    return Err((
                        "missing_sender",
                        format!("nft transfer source `{}` does not exist", operation.from),
                    ));
                };
                source.balance = source_after_issuer_fee;
                let Some(issuer_account) = ledger.account_mut(&issuer) else {
                    return Err((
                        "missing_issuer",
                        format!("nft issuer `{issuer}` does not exist"),
                    ));
                };
                issuer_account.balance = issuer_account
                    .balance
                    .checked_add(issuer_transfer_fee)
                    .ok_or_else(|| {
                        (
                            "nft_issuer_transfer_fee_overflow",
                            "issuer balance overflow while applying nft issuer transfer fee"
                                .to_string(),
                        )
                    })?;
            }
            ledger.nfts[index].owner = operation.to.clone();
            Ok(())
        }
        NftTransactionOperation::NftBurn(operation) => {
            if transaction.unsigned.transaction_kind != NFT_BURN_TRANSACTION_KIND {
                return Err((
                    "wrong_transaction_kind",
                    "nft_burn transaction kind mismatch".to_string(),
                ));
            }
            let index = nft_index(ledger, &operation.nft_id).ok_or_else(|| {
                (
                    "missing_nft",
                    format!("nft `{}` does not exist", operation.nft_id),
                )
            })?;
            if ledger.nfts[index].burned {
                return Err(("nft_burned", "nft is already burned".to_string()));
            }
            if ledger.nfts[index].owner != operation.owner {
                return Err((
                    "nft_owner_mismatch",
                    "nft_burn owner does not match current owner".to_string(),
                ));
            }
            if ledger.nfts[index].collection_flags & NFT_COLLECTION_FLAG_BURN_LOCKED != 0 {
                return Err((
                    "nft_collection_burn_locked",
                    "nft collection policy does not allow burn".to_string(),
                ));
            }
            ledger.nfts[index].burned = true;
            Ok(())
        }
    }
}

fn apply_escrow_operation(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    transaction: &SignedEscrowTransaction,
    block_height: u64,
) -> Result<(), (&'static str, String)> {
    match &transaction.unsigned.operation {
        EscrowTransactionOperation::EscrowCreate(operation) => {
            if transaction.unsigned.transaction_kind != ESCROW_CREATE_TRANSACTION_KIND {
                return Err((
                    "wrong_transaction_kind",
                    "escrow_create transaction kind mismatch".to_string(),
                ));
            }
            let escrow_id = escrow_id(
                &genesis.chain_id,
                &operation.owner,
                transaction.unsigned.sequence,
            )
            .map_err(|error| ("bad_escrow_id", error))?;
            if ledger.escrow(&escrow_id).is_some() {
                return Err((
                    "duplicate_escrow",
                    format!("escrow `{escrow_id}` already exists"),
                ));
            }
            if operation.asset_id == NATIVE_PFT_ESCROW_ASSET_ID {
                let owner = ledger.account_mut(&operation.owner).ok_or_else(|| {
                    (
                        "missing_owner",
                        format!("escrow owner `{}` does not exist", operation.owner),
                    )
                })?;
                if owner.balance < operation.amount {
                    return Err((
                        "insufficient_funds",
                        "escrow owner balance is too low for locked amount".to_string(),
                    ));
                }
                let owner_after_lock = owner.balance - operation.amount;
                if let Some(message) = account_reserve_violation(&operation.owner, owner_after_lock)
                {
                    return Err(("below_account_reserve", message));
                }
                owner.balance = owner_after_lock;
            } else {
                lock_issued_asset_for_escrow(ledger, operation)?;
            }
            let escrow = Escrow::new(
                &genesis.chain_id,
                operation.owner.clone(),
                transaction.unsigned.sequence,
                operation.recipient.clone(),
                operation.asset_id.clone(),
                operation.amount,
                transaction.unsigned.fee,
                operation.condition.clone(),
                operation.finish_after,
                operation.cancel_after,
                block_height,
            )
            .map_err(|error| ("bad_escrow", error))?;
            ledger.escrows.push(escrow);
            Ok(())
        }
        EscrowTransactionOperation::EscrowFinish(operation) => {
            if transaction.unsigned.transaction_kind != ESCROW_FINISH_TRANSACTION_KIND {
                return Err((
                    "wrong_transaction_kind",
                    "escrow_finish transaction kind mismatch".to_string(),
                ));
            }
            let escrow_index = escrow_index(ledger, &operation.escrow_id).ok_or_else(|| {
                (
                    "missing_escrow",
                    format!("escrow `{}` does not exist", operation.escrow_id),
                )
            })?;
            let escrow = ledger.escrows[escrow_index].clone();
            if escrow.owner != operation.owner || escrow.recipient != operation.recipient {
                return Err((
                    "escrow_party_mismatch",
                    "escrow_finish owner or recipient does not match escrow".to_string(),
                ));
            }
            if escrow.state != ESCROW_STATE_OPEN {
                return Err((
                    "escrow_not_open",
                    "escrow is not open for finish".to_string(),
                ));
            }
            if escrow.finish_after != 0 && block_height < escrow.finish_after {
                return Err((
                    "escrow_finish_too_early",
                    format!("escrow cannot finish before height {}", escrow.finish_after),
                ));
            }
            if !escrow.condition.is_empty() && operation.fulfillment != escrow.condition {
                return Err((
                    "escrow_condition_unsatisfied",
                    "escrow fulfillment does not satisfy condition".to_string(),
                ));
            }
            if escrow.asset_id == NATIVE_PFT_ESCROW_ASSET_ID {
                let recipient_base = ledger
                    .account(&escrow.recipient)
                    .map(|account| account.balance)
                    .unwrap_or_default();
                let recipient_after_credit =
                    recipient_base.checked_add(escrow.amount).ok_or_else(|| {
                        (
                            "balance_overflow",
                            "escrow recipient balance would overflow".to_string(),
                        )
                    })?;
                if let Some(message) =
                    account_reserve_violation(&escrow.recipient, recipient_after_credit)
                {
                    return Err(("below_account_reserve", message));
                }
                let recipient = ledger.ensure_account(&escrow.recipient);
                recipient.balance = recipient_after_credit;
            } else {
                release_issued_asset_escrow_to_recipient(ledger, &escrow)?;
            }
            ledger.escrows[escrow_index].state = ESCROW_STATE_FINISHED.to_string();
            Ok(())
        }
        EscrowTransactionOperation::EscrowCancel(operation) => {
            if transaction.unsigned.transaction_kind != ESCROW_CANCEL_TRANSACTION_KIND {
                return Err((
                    "wrong_transaction_kind",
                    "escrow_cancel transaction kind mismatch".to_string(),
                ));
            }
            let escrow_index = escrow_index(ledger, &operation.escrow_id).ok_or_else(|| {
                (
                    "missing_escrow",
                    format!("escrow `{}` does not exist", operation.escrow_id),
                )
            })?;
            let escrow = ledger.escrows[escrow_index].clone();
            if escrow.owner != operation.owner {
                return Err((
                    "escrow_owner_mismatch",
                    "escrow_cancel owner does not match escrow".to_string(),
                ));
            }
            if escrow.state != ESCROW_STATE_OPEN {
                return Err((
                    "escrow_not_open",
                    "escrow is not open for cancel".to_string(),
                ));
            }
            if escrow.cancel_after == 0 {
                return Err((
                    "escrow_cancel_unavailable",
                    "escrow has no cancel_after height".to_string(),
                ));
            }
            if block_height < escrow.cancel_after {
                return Err((
                    "escrow_cancel_too_early",
                    format!("escrow cannot cancel before height {}", escrow.cancel_after),
                ));
            }
            if escrow.asset_id == NATIVE_PFT_ESCROW_ASSET_ID {
                let owner_base = ledger
                    .account(&escrow.owner)
                    .map(|account| account.balance)
                    .unwrap_or_default();
                let owner_after_credit =
                    owner_base.checked_add(escrow.amount).ok_or_else(|| {
                        (
                            "balance_overflow",
                            "escrow owner balance would overflow".to_string(),
                        )
                    })?;
                let owner = ledger.ensure_account(&escrow.owner);
                owner.balance = owner_after_credit;
            } else {
                refund_issued_asset_escrow_to_owner(ledger, &escrow)?;
            }
            ledger.escrows[escrow_index].state = ESCROW_STATE_CANCELED.to_string();
            Ok(())
        }
    }
}

fn lock_issued_asset_for_escrow(
    ledger: &mut LedgerState,
    operation: &EscrowCreateOperation,
) -> Result<(), (&'static str, String)> {
    let asset = ledger
        .asset_definition(&operation.asset_id)
        .cloned()
        .ok_or_else(|| {
            (
                "missing_asset",
                format!("asset `{}` does not exist", operation.asset_id),
            )
        })?;
    if operation.owner == asset.issuer || operation.recipient == asset.issuer {
        return Err((
            "unsupported_issued_escrow_party",
            "issued-asset escrow requires owner and recipient holder trustlines".to_string(),
        ));
    }

    let owner_index =
        trustline_index(ledger, &operation.owner, &operation.asset_id).ok_or_else(|| {
            (
                "missing_trustline",
                "issued-asset escrow owner has no trustline for asset".to_string(),
            )
        })?;
    let recipient_index = trustline_index(ledger, &operation.recipient, &operation.asset_id)
        .ok_or_else(|| {
            (
                "missing_trustline",
                "issued-asset escrow recipient has no trustline for asset".to_string(),
            )
        })?;

    ensure_line_can_move(&asset, &ledger.trustlines[owner_index])?;
    ensure_line_can_move(&asset, &ledger.trustlines[recipient_index])?;

    if ledger.trustlines[owner_index].balance < operation.amount {
        return Err((
            "insufficient_issued_balance",
            "issued-asset escrow amount exceeds owner trustline balance".to_string(),
        ));
    }

    let recipient_reserved = issued_asset_reserved_total_for_account(
        ledger,
        &operation.recipient,
        &operation.asset_id,
        None,
        None,
    )?;
    let recipient_required = ledger.trustlines[recipient_index]
        .balance
        .checked_add(recipient_reserved)
        .and_then(|balance| balance.checked_add(operation.amount))
        .ok_or_else(|| {
            (
                "issued_balance_overflow",
                "recipient issued-asset escrow balance would overflow".to_string(),
            )
        })?;
    if recipient_required > ledger.trustlines[recipient_index].limit {
        return Err((
            "trustline_limit_exceeded",
            "issued-asset escrow exceeds recipient trustline limit".to_string(),
        ));
    }

    ledger.trustlines[owner_index].balance -= operation.amount;
    Ok(())
}

fn release_issued_asset_escrow_to_recipient(
    ledger: &mut LedgerState,
    escrow: &Escrow,
) -> Result<(), (&'static str, String)> {
    let asset = ledger
        .asset_definition(&escrow.asset_id)
        .cloned()
        .ok_or_else(|| {
            (
                "missing_asset",
                format!("asset `{}` does not exist", escrow.asset_id),
            )
        })?;
    let recipient_index =
        trustline_index(ledger, &escrow.recipient, &escrow.asset_id).ok_or_else(|| {
            (
                "missing_trustline",
                "issued-asset escrow recipient has no trustline for asset".to_string(),
            )
        })?;
    ensure_line_can_move(&asset, &ledger.trustlines[recipient_index])?;

    let recipient_after = ledger.trustlines[recipient_index]
        .balance
        .checked_add(escrow.amount)
        .ok_or_else(|| {
            (
                "issued_balance_overflow",
                "recipient issued-asset escrow balance would overflow".to_string(),
            )
        })?;
    let reserved_after_finish = issued_asset_reserved_total_for_account(
        ledger,
        &escrow.recipient,
        &escrow.asset_id,
        Some(&escrow.escrow_id),
        None,
    )?;
    let required_limit = recipient_after
        .checked_add(reserved_after_finish)
        .ok_or_else(|| {
            (
                "issued_balance_overflow",
                "recipient issued-asset escrow reservations would overflow".to_string(),
            )
        })?;
    if required_limit > ledger.trustlines[recipient_index].limit {
        return Err((
            "trustline_limit_exceeded",
            "issued-asset escrow finish exceeds recipient trustline limit".to_string(),
        ));
    }

    ledger.trustlines[recipient_index].balance = recipient_after;
    Ok(())
}

fn refund_issued_asset_escrow_to_owner(
    ledger: &mut LedgerState,
    escrow: &Escrow,
) -> Result<(), (&'static str, String)> {
    let asset = ledger
        .asset_definition(&escrow.asset_id)
        .cloned()
        .ok_or_else(|| {
            (
                "missing_asset",
                format!("asset `{}` does not exist", escrow.asset_id),
            )
        })?;
    let owner_index =
        trustline_index(ledger, &escrow.owner, &escrow.asset_id).ok_or_else(|| {
            (
                "missing_trustline",
                "issued-asset escrow owner has no trustline for asset".to_string(),
            )
        })?;
    ensure_line_can_move(&asset, &ledger.trustlines[owner_index])?;

    let owner_after = ledger.trustlines[owner_index]
        .balance
        .checked_add(escrow.amount)
        .ok_or_else(|| {
            (
                "issued_balance_overflow",
                "owner issued-asset escrow refund would overflow".to_string(),
            )
        })?;
    let reserved_after_cancel = issued_asset_reserved_total_for_account(
        ledger,
        &escrow.owner,
        &escrow.asset_id,
        Some(&escrow.escrow_id),
        None,
    )?;
    let required_limit = owner_after
        .checked_add(reserved_after_cancel)
        .ok_or_else(|| {
            (
                "issued_balance_overflow",
                "owner issued-asset escrow reservations would overflow".to_string(),
            )
        })?;
    if required_limit > ledger.trustlines[owner_index].limit {
        return Err((
            "trustline_limit_exceeded",
            "issued-asset escrow cancel exceeds owner trustline limit".to_string(),
        ));
    }

    ledger.trustlines[owner_index].balance = owner_after;
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssetExecutionCompatibility {
    pub allow_legacy_nav_subscription_source_root: bool,
    pub allow_legacy_cash_omitted_sp1_verified_net_assets: bool,
    pub emit_legacy_domainless_withdrawal_packet: bool,
    pub reject_legacy_domainless_withdrawal_packet_state: bool,
    pub allow_unverified_pftl_uniswap_bridge_replay: bool,
    pub bridge_verification_activation_height: Option<u64>,
    pub atomic_swap_activation_height: Option<u64>,
    pub atomic_swap_paused: bool,
}

impl AssetExecutionCompatibility {
    pub const fn strict() -> Self {
        Self {
            allow_legacy_nav_subscription_source_root: false,
            allow_legacy_cash_omitted_sp1_verified_net_assets: false,
            emit_legacy_domainless_withdrawal_packet: false,
            reject_legacy_domainless_withdrawal_packet_state: false,
            allow_unverified_pftl_uniswap_bridge_replay: false,
            bridge_verification_activation_height: Some(0),
            atomic_swap_activation_height: Some(0),
            atomic_swap_paused: false,
        }
    }

    pub const fn wan_devnet_legacy_replay() -> Self {
        Self {
            allow_legacy_nav_subscription_source_root: true,
            allow_legacy_cash_omitted_sp1_verified_net_assets: true,
            emit_legacy_domainless_withdrawal_packet: true,
            reject_legacy_domainless_withdrawal_packet_state: false,
            allow_unverified_pftl_uniswap_bridge_replay: true,
            bridge_verification_activation_height: Some(0),
            atomic_swap_activation_height: None,
            atomic_swap_paused: false,
        }
    }

    pub const fn wan_devnet_legacy_nav_replay() -> Self {
        Self {
            allow_legacy_nav_subscription_source_root: true,
            allow_legacy_cash_omitted_sp1_verified_net_assets: true,
            emit_legacy_domainless_withdrawal_packet: false,
            reject_legacy_domainless_withdrawal_packet_state: false,
            allow_unverified_pftl_uniswap_bridge_replay: true,
            bridge_verification_activation_height: Some(0),
            atomic_swap_activation_height: None,
            atomic_swap_paused: false,
        }
    }

    pub const fn wan_devnet_legacy_cash_omitted_sp1_replay() -> Self {
        Self {
            allow_legacy_nav_subscription_source_root: true,
            allow_legacy_cash_omitted_sp1_verified_net_assets: true,
            emit_legacy_domainless_withdrawal_packet: false,
            reject_legacy_domainless_withdrawal_packet_state: false,
            allow_unverified_pftl_uniswap_bridge_replay: true,
            bridge_verification_activation_height: Some(0),
            atomic_swap_activation_height: None,
            atomic_swap_paused: false,
        }
    }

    pub const fn wan_devnet_legacy_strict_domain_validation() -> Self {
        Self {
            allow_legacy_nav_subscription_source_root: false,
            allow_legacy_cash_omitted_sp1_verified_net_assets: false,
            emit_legacy_domainless_withdrawal_packet: false,
            reject_legacy_domainless_withdrawal_packet_state: true,
            allow_unverified_pftl_uniswap_bridge_replay: true,
            bridge_verification_activation_height: Some(0),
            atomic_swap_activation_height: None,
            atomic_swap_paused: false,
        }
    }

    pub const fn with_bridge_verification_activation_height(
        mut self,
        bridge_verification_activation_height: Option<u64>,
    ) -> Self {
        self.bridge_verification_activation_height = bridge_verification_activation_height;
        self
    }

    #[cfg(test)]
    pub const fn with_unverified_pftl_uniswap_bridge_fixture(mut self) -> Self {
        self.allow_unverified_pftl_uniswap_bridge_replay = true;
        self
    }

    pub fn bridge_verification_rules_active(&self, block_height: u64) -> bool {
        self.bridge_verification_activation_height
            .is_some_and(|activation_height| block_height >= activation_height)
    }

    pub const fn with_atomic_swap_activation_height(
        mut self,
        atomic_swap_activation_height: Option<u64>,
    ) -> Self {
        self.atomic_swap_activation_height = atomic_swap_activation_height;
        self
    }

    pub fn atomic_swap_active(&self, block_height: u64) -> bool {
        !self.atomic_swap_paused
            && self
                .atomic_swap_activation_height
            .is_some_and(|activation_height| block_height >= activation_height)
    }

    pub const fn with_atomic_swap_paused(mut self, paused: bool) -> Self {
        self.atomic_swap_paused = paused;
        self
    }
}

fn apply_asset_operation(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    transaction: &SignedAssetTransaction,
    block_height: u64,
    compatibility: AssetExecutionCompatibility,
) -> Result<(), (&'static str, String)> {
    match &transaction.unsigned.operation {
        AssetTransactionOperation::AssetCreate(operation) => {
            if transaction.unsigned.transaction_kind != ASSET_CREATE_TRANSACTION_KIND {
                return Err((
                    "wrong_transaction_kind",
                    "asset_create transaction kind mismatch".to_string(),
                ));
            }
            let mut asset = AssetDefinition::new(
                &genesis.chain_id,
                operation.issuer.clone(),
                operation.code.clone(),
                operation.version,
                operation.precision,
            )
            .map_err(|error| ("bad_asset_definition", error))?;
            asset.display_name = operation.display_name.clone();
            asset.max_supply = operation.max_supply;
            asset.requires_authorization = operation.requires_authorization;
            asset.freeze_enabled = operation.freeze_enabled;
            asset.clawback_enabled = operation.clawback_enabled;
            asset
                .validate_for_chain(&genesis.chain_id)
                .map_err(|error| ("bad_asset_definition", error))?;
            if ledger.asset_definition(&asset.asset_id).is_some() {
                return Err((
                    "duplicate_asset",
                    format!("asset `{}` already exists", asset.asset_id),
                ));
            }
            ledger.asset_definitions.push(asset);
            Ok(())
        }
        AssetTransactionOperation::TrustSet(operation) => {
            if transaction.unsigned.transaction_kind != TRUST_SET_TRANSACTION_KIND {
                return Err((
                    "wrong_transaction_kind",
                    "trust_set transaction kind mismatch".to_string(),
                ));
            }
            let (asset_issuer, requires_authorization, freeze_enabled) = {
                let asset = ledger
                    .asset_definition(&operation.asset_id)
                    .ok_or_else(|| {
                        (
                            "missing_asset",
                            format!("asset `{}` does not exist", operation.asset_id),
                        )
                    })?;
                (
                    asset.issuer.clone(),
                    asset.requires_authorization,
                    asset.freeze_enabled,
                )
            };
            if asset_issuer != operation.issuer {
                return Err((
                    "asset_issuer_mismatch",
                    "trust_set issuer does not match asset issuer".to_string(),
                ));
            }
            if ledger.account(&operation.account).is_none() {
                return Err((
                    "missing_trustline_account",
                    format!("trustline account `{}` does not exist", operation.account),
                ));
            }
            let source = transaction.unsigned.source.as_str();
            let line_index = trustline_index(ledger, &operation.account, &operation.asset_id);
            if source == operation.account {
                if operation.authorized || operation.frozen {
                    return Err((
                        "issuer_control_required",
                        "account-signed trust_set cannot set authorization or freeze flags"
                            .to_string(),
                    ));
                }
                if let Some(index) = line_index {
                    let line = &ledger.trustlines[index];
                    if operation.reserve_paid != line.reserve_paid {
                        return Err((
                            "reserve_mismatch",
                            "trust_set reserve_paid must match existing trustline reserve"
                                .to_string(),
                        ));
                    }
                    let reserved_escrows = issued_asset_reserved_total_for_account(
                        ledger,
                        &operation.account,
                        &operation.asset_id,
                        None,
                        None,
                    )?;
                    let required_limit =
                        line.balance.checked_add(reserved_escrows).ok_or_else(|| {
                            (
                                "issued_balance_overflow",
                                "trustline balance and escrow reservations would overflow"
                                    .to_string(),
                            )
                        })?;
                    if required_limit > operation.limit {
                        return Err((
                            "trustline_limit_too_low",
                            "trustline limit cannot be below current balance plus open escrow reservations".to_string(),
                        ));
                    }
                    let line = &mut ledger.trustlines[index];
                    line.limit = operation.limit;
                } else {
                    if operation.reserve_paid < TRUSTLINE_STATE_EXPANSION_FEE {
                        return Err((
                            "reserve_too_low",
                            format!(
                                "trustline reserve_paid must be at least {TRUSTLINE_STATE_EXPANSION_FEE}"
                            ),
                        ));
                    }
                    let mut line = TrustLine::new(
                        operation.account.clone(),
                        operation.issuer.clone(),
                        operation.asset_id.clone(),
                        operation.limit,
                        operation.reserve_paid,
                    )
                    .map_err(|error| ("bad_trustline", error))?;
                    line.authorized = !requires_authorization;
                    ledger.trustlines.push(line);
                }
                Ok(())
            } else if source == operation.issuer {
                let Some(index) = line_index else {
                    return Err((
                        "missing_trustline",
                        "issuer-signed trust_set requires an existing trustline".to_string(),
                    ));
                };
                if !freeze_enabled && operation.frozen {
                    return Err((
                        "freeze_not_enabled",
                        "asset policy does not allow freezing trustlines".to_string(),
                    ));
                }
                let line = &mut ledger.trustlines[index];
                if operation.limit != line.limit || operation.reserve_paid != line.reserve_paid {
                    return Err((
                        "issuer_cannot_change_holder_terms",
                        "issuer-signed trust_set cannot change holder limit or reserve".to_string(),
                    ));
                }
                line.authorized = operation.authorized;
                line.frozen = operation.frozen;
                Ok(())
            } else {
                Err((
                    "unauthorized_source",
                    "trust_set source must be the trustline account or issuer".to_string(),
                ))
            }
        }
        AssetTransactionOperation::IssuedPayment(operation) => {
            if transaction.unsigned.transaction_kind != ISSUED_PAYMENT_TRANSACTION_KIND {
                return Err((
                    "wrong_transaction_kind",
                    "issued_payment transaction kind mismatch".to_string(),
                ));
            }
            apply_issued_payment(ledger, operation, block_height, compatibility)
        }
        AssetTransactionOperation::AssetBurn(operation) => {
            if transaction.unsigned.transaction_kind != ASSET_BURN_TRANSACTION_KIND {
                return Err((
                    "wrong_transaction_kind",
                    "asset_burn transaction kind mismatch".to_string(),
                ));
            }
            let asset = ledger
                .asset_definition(&operation.asset_id)
                .cloned()
                .ok_or_else(|| {
                    (
                        "missing_asset",
                        format!("asset `{}` does not exist", operation.asset_id),
                    )
                })?;
            if asset.issuer != operation.issuer {
                return Err((
                    "asset_issuer_mismatch",
                    "asset_burn issuer does not match asset issuer".to_string(),
                ));
            }
            let index = trustline_index(ledger, &operation.owner, &operation.asset_id).ok_or_else(
                || {
                    (
                        "missing_trustline",
                        "asset_burn owner has no trustline for asset".to_string(),
                    )
                },
            )?;
            ensure_line_can_move(&asset, &ledger.trustlines[index])?;
            if ledger.trustlines[index].balance < operation.amount {
                return Err((
                    "insufficient_issued_balance",
                    "asset_burn amount exceeds trustline balance".to_string(),
                ));
            }
            ledger.trustlines[index].balance -= operation.amount;
            Ok(())
        }
        AssetTransactionOperation::AssetClawback(operation) => {
            if transaction.unsigned.transaction_kind != ASSET_CLAWBACK_TRANSACTION_KIND {
                return Err((
                    "wrong_transaction_kind",
                    "asset_clawback transaction kind mismatch".to_string(),
                ));
            }
            let asset = ledger
                .asset_definition(&operation.asset_id)
                .cloned()
                .ok_or_else(|| {
                    (
                        "missing_asset",
                        format!("asset `{}` does not exist", operation.asset_id),
                    )
                })?;
            if asset.issuer != operation.issuer {
                return Err((
                    "asset_issuer_mismatch",
                    "asset_clawback issuer does not match asset issuer".to_string(),
                ));
            }
            if !asset.clawback_enabled {
                return Err((
                    "clawback_not_enabled",
                    "asset policy does not allow issuer clawback".to_string(),
                ));
            }
            let index = trustline_index(ledger, &operation.owner, &operation.asset_id).ok_or_else(
                || {
                    (
                        "missing_trustline",
                        "asset_clawback owner has no trustline for asset".to_string(),
                    )
                },
            )?;
            if ledger.trustlines[index].balance < operation.amount {
                return Err((
                    "insufficient_issued_balance",
                    "asset_clawback amount exceeds trustline balance".to_string(),
                ));
            }
            ledger.trustlines[index].balance -= operation.amount;
            Ok(())
        }
        AssetTransactionOperation::NavAssetRegister(operation) => {
            if transaction.unsigned.transaction_kind != NAV_ASSET_REGISTER_TRANSACTION_KIND {
                return Err((
                    "wrong_transaction_kind",
                    "nav_asset_register transaction kind mismatch".to_string(),
                ));
            }
            let asset = ledger
                .asset_definition(&operation.asset_id)
                .cloned()
                .ok_or_else(|| {
                    (
                        "missing_asset",
                        format!("asset `{}` does not exist", operation.asset_id),
                    )
                })?;
            if asset.issuer != operation.issuer {
                return Err((
                    "asset_issuer_mismatch",
                    "nav_asset_register issuer does not match issued asset issuer".to_string(),
                ));
            }
            if compatibility.bridge_verification_rules_active(block_height)
                && is_nav_profile_id_shaped(&operation.proof_profile)
            {
                let profile = ledger
                    .nav_proof_profile(&operation.proof_profile)
                    .ok_or_else(|| {
                        (
                            "unknown_nav_profile",
                            "nav_asset_register proof_profile is profile-id shaped but no such profile is registered"
                                .to_string(),
                        )
                    })?;
                if profile.verifier_kind == NAV_PROFILE_VERIFIER_MULTI_FETCH
                    && profile
                        .source_class
                        .starts_with(VAULT_BRIDGE_PROFILE_SOURCE_CLASS_PREFIX)
                    && profile.bridge_observer_min_confirmations == 0
                {
                    return Err((
                        "vault_bridge_observer_policy_not_configured",
                        "active bridge verification rules forbid binding a vault-bridge asset to a multi-fetch profile without an explicit observer confirmation minimum"
                            .to_string(),
                    ));
                }
            }
            if ledger.nav_asset(&operation.asset_id).is_some() {
                if is_nav_profile_id_shaped(&operation.proof_profile)
                    && ledger.nav_proof_profile(&operation.proof_profile).is_none()
                {
                    return Err((
                        "unknown_nav_profile",
                        "nav_asset_register proof_profile is profile-id shaped but no such profile is registered".to_string(),
                    ));
                }
                let existing = ledger
                    .nav_asset_mut(&operation.asset_id)
                    .expect("nav asset exists");
                if existing.issuer != operation.issuer {
                    return Err((
                        "duplicate_nav_asset",
                        format!("nav asset `{}` already exists", operation.asset_id),
                    ));
                }
                existing.reserve_operator = operation.reserve_operator.clone();
                existing.proof_profile = operation.proof_profile.clone();
                existing.valuation_unit = operation.valuation_unit.clone();
                existing.redemption_account = operation.redemption_account.clone();
                return Ok(());
            }
            if is_nav_profile_id_shaped(&operation.proof_profile)
                && ledger.nav_proof_profile(&operation.proof_profile).is_none()
            {
                return Err((
                    "unknown_nav_profile",
                    "nav_asset_register proof_profile is profile-id shaped but no such profile is registered".to_string(),
                ));
            }
            let nav_asset = NavTrackedAsset::new(
                operation.asset_id.clone(),
                operation.issuer.clone(),
                operation.reserve_operator.clone(),
                operation.proof_profile.clone(),
                operation.valuation_unit.clone(),
                operation.redemption_account.clone(),
            )
            .map_err(|error| ("bad_nav_asset", error))?;
            ledger.nav_assets.push(nav_asset);
            Ok(())
        }
        AssetTransactionOperation::NavReserveSubmit(operation) => {
            if transaction.unsigned.transaction_kind != NAV_RESERVE_SUBMIT_TRANSACTION_KIND {
                return Err((
                    "wrong_transaction_kind",
                    "nav_reserve_submit transaction kind mismatch".to_string(),
                ));
            }
            let nav_asset = ledger.nav_asset(&operation.asset_id).cloned().ok_or_else(|| {
                (
                    "missing_nav_asset",
                    format!("nav asset `{}` does not exist", operation.asset_id),
                )
            })?;
            let nav_asset_definition = ledger.asset_definition(&operation.asset_id).ok_or_else(|| {
                (
                    "missing_asset",
                    format!("asset `{}` does not exist", operation.asset_id),
                )
            })?;
            let nav_unit_scale = 10_u128
                .checked_pow(nav_asset_definition.precision.into())
                .ok_or_else(|| {
                    (
                        "bad_nav_asset_precision",
                        "nav asset precision scale would overflow".to_string(),
                    )
                })?;
            validate_nav_reserve_collateralization_with_unit_scale(
                operation.verified_net_assets,
                operation.circulating_supply,
                operation.nav_per_unit,
                nav_unit_scale,
            )
            .map_err(|error| ("nav_reserve_undercollateralized", error))?;
            if nav_asset.issuer != operation.issuer {
                return Err((
                    "nav_issuer_mismatch",
                    "nav_reserve_submit issuer does not match nav asset issuer".to_string(),
                ));
            }
            if operation.submitter != nav_asset.reserve_operator && operation.submitter != nav_asset.issuer {
                return Err((
                    "unauthorized_nav_submitter",
                    "nav reserve submitter must be issuer or reserve operator".to_string(),
                ));
            }
            if operation.proof_profile != nav_asset.proof_profile {
                return Err((
                    "proof_profile_mismatch",
                    "nav reserve proof profile does not match nav asset".to_string(),
                ));
            }
            if operation.epoch <= nav_asset.finalized_epoch {
                return Err((
                    "stale_nav_epoch",
                    "nav reserve epoch must be greater than finalized epoch".to_string(),
                ));
            }
            if ledger
                .nav_reserve_packet(
                    &operation.asset_id,
                    operation.epoch,
                    &operation.reserve_packet_hash,
                )
                .is_some()
            {
                return Err((
                    "duplicate_nav_reserve_packet",
                    "nav reserve packet already exists".to_string(),
                ));
            }
            let profile = nav_profile_for_asset(ledger, &nav_asset).cloned();
            if let Some(profile) = &profile {
                if profile.verifier_kind == NAV_PROFILE_VERIFIER_MULTI_FETCH
                    && operation.reserve_accounts.is_empty()
                {
                    return Err((
                        "missing_reserve_accounts",
                        "multi-fetch nav profile requires declared external reserve accounts".to_string(),
                    ));
                }
                if profile.verifier_kind == NAV_PROFILE_VERIFIER_LEDGER_TRANSPARENT {
                    if operation.reserve_accounts.is_empty() {
                        return Err((
                            "missing_reserve_accounts",
                            "ledger-transparent nav profile requires reserve_accounts".to_string(),
                        ));
                    }
                    let mut reserve_sum: u64 = 0;
                    for account_address in &operation.reserve_accounts {
                        let account = ledger.account(account_address).ok_or_else(|| {
                            (
                                "missing_reserve_account",
                                format!(
                                    "nav reserve account `{account_address}` does not exist on ledger"
                                ),
                            )
                        })?;
                        reserve_sum =
                            reserve_sum.checked_add(account.balance).ok_or_else(|| {
                                (
                                    "reserve_sum_overflow",
                                    "nav reserve account balances overflow".to_string(),
                                )
                            })?;
                    }
                    if reserve_sum != operation.verified_net_assets {
                        return Err((
                            "reserve_sum_mismatch",
                            format!(
                                "ledger-transparent reserve sum {reserve_sum} does not equal verified_net_assets {}",
                                operation.verified_net_assets
                            ),
                        ));
                    }
                }
                if profile.verifier_kind == NAV_PROFILE_VERIFIER_SP1_GROTH16 {
                    let overlay = nav_subscription_reserve_overlay(ledger, &nav_asset)?;
                    let verified_net_assets_for_sp1 = if let Some(overlay) = overlay.as_ref() {
                        operation
                            .verified_net_assets
                            .checked_sub(overlay.value_nav_units)
                            .ok_or_else(|| {
                                (
                                    "nav_subscription_overlay_exceeds_assets",
                                    "nav subscription overlay exceeds submitted verified_net_assets"
                                        .to_string(),
                                )
                            })?
                    } else {
                        operation.verified_net_assets
                    };
                    let decoded = match verify_sp1_groth16_with_options(
                        profile,
                        verified_net_assets_for_sp1,
                        &operation.sp1_proof_bytes,
                        &operation.sp1_public_values,
                        NavSp1VerifyOptions {
                            allow_legacy_cash_omitted_verified_net_assets:
                                compatibility.allow_legacy_cash_omitted_sp1_verified_net_assets,
                        },
                    ) {
                        Ok(decoded) => decoded,
                        Err(error) => return Err((error.code(), error.message())),
                    };
                    if let Some(overlay) = overlay {
                        let expected_source_root = nav_sp1_subscription_source_root(
                            &nav_asset,
                            profile,
                            &decoded,
                            &operation.sp1_public_values,
                            &overlay,
                        )?;
                        if operation.source_root != expected_source_root
                            && !compatibility.allow_legacy_nav_subscription_source_root
                        {
                            return Err((
                                "nav_subscription_source_root_mismatch",
                                "sp1-groth16 nav reserve packet source_root must match the SP1 base proof plus retired vault bridge subscription allocations"
                                    .to_string(),
                            ));
                        }
                    }
                }
                if profile.source_class.starts_with(VAULT_BRIDGE_PROFILE_SOURCE_CLASS_PREFIX) {
                    validate_vault_bridge_reserve_packet_fields(ledger, &nav_asset, profile, operation)?;
                }
            }
            let mut packet = NavReservePacket::new(
                operation.asset_id.clone(),
                operation.issuer.clone(),
                operation.submitter.clone(),
                operation.epoch,
                operation.nav_per_unit,
                operation.circulating_supply,
                operation.verified_net_assets,
                operation.proof_profile.clone(),
                operation.source_root.clone(),
                operation.attestor_root.clone(),
                operation.reserve_packet_hash.clone(),
            )
            .map_err(|error| ("bad_nav_reserve_packet", error))?;
            if profile.is_some() {
                packet.submitted_at_height = block_height;
            }
            packet.reserve_accounts = operation.reserve_accounts.clone();
            packet.sp1_proof_bytes = operation.sp1_proof_bytes.clone();
            packet.sp1_public_values = operation.sp1_public_values.clone();
            ledger.nav_reserve_packets.push(packet);
            Ok(())
        }
        AssetTransactionOperation::NavReserveChallenge(operation) => {
            if transaction.unsigned.transaction_kind != NAV_RESERVE_CHALLENGE_TRANSACTION_KIND {
                return Err((
                    "wrong_transaction_kind",
                    "nav_reserve_challenge transaction kind mismatch".to_string(),
                ));
            }
            let packet = ledger
                .nav_reserve_packet_mut(
                    &operation.asset_id,
                    operation.epoch,
                    &operation.reserve_packet_hash,
                )
                .ok_or_else(|| {
                    (
                        "missing_nav_reserve_packet",
                        "nav reserve challenge references missing reserve packet".to_string(),
                    )
                })?;
            if packet.state == NAV_RESERVE_STATE_FINALIZED {
                return Err((
                    "finalized_nav_packet",
                    "finalized nav reserve packet is not challengeable in this path".to_string(),
                ));
            }
            if packet.state == NAV_RESERVE_STATE_CHALLENGED {
                return Err((
                    "nav_packet_already_challenged",
                    "nav reserve packet is already challenged".to_string(),
                ));
            }
            let nav_asset = ledger.nav_asset(&operation.asset_id).cloned().ok_or_else(|| {
                (
                    "missing_nav_asset",
                    "nav reserve challenge references missing nav asset".to_string(),
                )
            })?;
            let profile = nav_profile_for_asset(ledger, &nav_asset).cloned();
            if let Some(profile) = &profile {
                if profile.verifier_kind == NAV_PROFILE_VERIFIER_LEDGER_TRANSPARENT {
                    return Err((
                        "nav_packet_consensus_verified",
                        "ledger-transparent packets are verified by consensus at submit and are not challengeable".to_string(),
                    ));
                }
                if profile.verifier_kind == NAV_PROFILE_VERIFIER_SP1_GROTH16 {
                    return Err((
                        "nav_packet_consensus_verified",
                        "sp1-groth16 packets are cryptographically verified at submit and are not challengeable".to_string(),
                    ));
                }
                if operation.bond < profile.min_challenge_bond {
                    return Err((
                        "challenge_bond_too_low",
                        format!(
                            "challenge bond {} is below profile minimum {}",
                            operation.bond, profile.min_challenge_bond
                        ),
                    ));
                }
            }
            if operation.bond > 0 {
                let challenger_account = ledger
                    .account_mut(&operation.challenger)
                    .ok_or_else(|| {
                        (
                            "missing_challenger_account",
                            "nav reserve challenger account does not exist".to_string(),
                        )
                    })?;
                let balance_after =
                    challenger_account.balance.checked_sub(operation.bond).ok_or_else(|| {
                        (
                            "insufficient_funds",
                            "challenger balance is too low for challenge bond".to_string(),
                        )
                    })?;
                if let Some(message) =
                    account_reserve_violation(&operation.challenger, balance_after)
                {
                    return Err(("below_account_reserve", message));
                }
                challenger_account.balance = balance_after;
            }
            let packet = ledger
                .nav_reserve_packet_mut(
                    &operation.asset_id,
                    operation.epoch,
                    &operation.reserve_packet_hash,
                )
                .ok_or_else(|| {
                    (
                        "missing_nav_reserve_packet",
                        "nav reserve packet disappeared during challenge".to_string(),
                    )
                })?;
            packet.state = NAV_RESERVE_STATE_CHALLENGED.to_string();
            packet.challenge_hash = operation.challenge_hash.clone();
            packet.challenger = operation.challenger.clone();
            packet.challenge_bond = operation.bond;
            let nav_asset = ledger
                .nav_asset_mut(&operation.asset_id)
                .ok_or_else(|| {
                    (
                        "missing_nav_asset",
                        "nav reserve challenge references missing nav asset".to_string(),
                    )
                })?;
            nav_asset.halted = true;
            nav_asset.halt_reason = "reserve_packet_challenged".to_string();
            Ok(())
        }
        AssetTransactionOperation::NavEpochFinalize(operation) => {
            if transaction.unsigned.transaction_kind != NAV_EPOCH_FINALIZE_TRANSACTION_KIND {
                return Err((
                    "wrong_transaction_kind",
                    "nav_epoch_finalize transaction kind mismatch".to_string(),
                ));
            }
            let nav_asset = ledger.nav_asset(&operation.asset_id).cloned().ok_or_else(|| {
                (
                    "missing_nav_asset",
                    format!("nav asset `{}` does not exist", operation.asset_id),
                )
            })?;
            if nav_asset.issuer != operation.issuer {
                return Err((
                    "nav_issuer_mismatch",
                    "nav_epoch_finalize issuer does not match nav asset issuer".to_string(),
                ));
            }
            let packet = ledger
                .nav_reserve_packet_mut(
                    &operation.asset_id,
                    operation.epoch,
                    &operation.reserve_packet_hash,
                )
                .ok_or_else(|| {
                    (
                        "missing_nav_reserve_packet",
                        "nav epoch finalize references missing reserve packet".to_string(),
                    )
                })?;
            if packet.state != NAV_RESERVE_STATE_SUBMITTED {
                return Err((
                    "nav_packet_not_submitted",
                    "nav epoch finalize requires a submitted reserve packet".to_string(),
                ));
            }
            if packet.issuer != operation.issuer {
                return Err((
                    "nav_packet_issuer_mismatch",
                    "nav reserve packet issuer does not match finalize issuer".to_string(),
                ));
            }
            let packet_nav_per_unit = packet.nav_per_unit;
            let packet_circulating_supply = packet.circulating_supply;
            let packet_submitted_at_height = packet.submitted_at_height;
            let profile = nav_profile_for_asset(ledger, &nav_asset).cloned();
            if let Some(profile) = &profile {
                if packet_submitted_at_height > 0 {
                    let challenge_deadline = packet_submitted_at_height
                        .saturating_add(profile.challenge_window_blocks);
                    if profile.challenge_window_blocks > 0
                        && block_height < challenge_deadline
                    {
                        return Err((
                            "nav_challenge_window_open",
                            format!(
                                "nav epoch finalize before challenge window closes at height {}",
                                challenge_deadline
                            ),
                        ));
                    }
                    if profile.max_snapshot_age_blocks > 0
                        && block_height
                            > packet_submitted_at_height
                                .saturating_add(profile.max_snapshot_age_blocks)
                    {
                        return Err((
                            "stale_nav_reserve_packet",
                            "nav reserve packet is older than the profile's max snapshot age".to_string(),
                        ));
                    }
                }
                if profile.verifier_kind == NAV_PROFILE_VERIFIER_MULTI_FETCH {
                    let packet_ref = ledger
                        .nav_reserve_packet(
                            &operation.asset_id,
                            operation.epoch,
                            &operation.reserve_packet_hash,
                        )
                        .ok_or_else(|| {
                            (
                                "missing_nav_reserve_packet",
                                "nav reserve packet disappeared during finalize".to_string(),
                            )
                        })?;
                    let fail_count = packet_ref
                        .attestations
                        .iter()
                        .filter(|attestation| !attestation.pass)
                        .count() as u64;
                    if fail_count > 0 {
                        return Err((
                            "nav_failed_attestations_present",
                            format!(
                                "nav reserve packet has {fail_count} failing attestation(s); supersede the packet instead of finalizing"
                            ),
                        ));
                    }
                    let pass_count = packet_ref
                        .attestations
                        .iter()
                        .filter(|attestation| attestation.pass)
                        .count() as u64;
                    if pass_count < profile.min_attestations {
                        return Err((
                            "nav_attestation_quorum_not_met",
                            format!(
                                "nav reserve packet has {pass_count} pass attestation(s); profile requires {}",
                                profile.min_attestations
                            ),
                        ));
                    }
                }
            }
            let packet = ledger
                .nav_reserve_packet_mut(
                    &operation.asset_id,
                    operation.epoch,
                    &operation.reserve_packet_hash,
                )
                .ok_or_else(|| {
                    (
                        "missing_nav_reserve_packet",
                        "nav reserve packet disappeared during finalize".to_string(),
                    )
                })?;
            packet.state = NAV_RESERVE_STATE_FINALIZED.to_string();
            resolve_nav_challenge_bonds(
                ledger,
                &operation.asset_id,
                operation.epoch,
                &operation.reserve_packet_hash,
            )?;
            let is_vault_bridge_profile = profile
                .as_ref()
                .is_some_and(|profile| {
                    profile
                        .source_class
                        .starts_with(VAULT_BRIDGE_PROFILE_SOURCE_CLASS_PREFIX)
                });
            {
                let nav_asset = ledger.nav_asset_mut(&operation.asset_id).ok_or_else(|| {
                    (
                        "missing_nav_asset",
                        "nav asset disappeared during finalize".to_string(),
                    )
                })?;
                if operation.epoch <= nav_asset.finalized_epoch {
                    return Err((
                        "stale_nav_epoch",
                        "nav epoch must be greater than finalized epoch".to_string(),
                    ));
                }
                nav_asset.finalized_epoch = operation.epoch;
                nav_asset.nav_per_unit = packet_nav_per_unit;
                nav_asset.circulating_supply = packet_circulating_supply;
                nav_asset.finalized_reserve_packet_hash = operation.reserve_packet_hash.clone();
                nav_asset.halted = false;
                nav_asset.halt_reason.clear();
                nav_asset.finalized_at_height = block_height;
            }
            if is_vault_bridge_profile {
                for bucket in ledger
                    .vault_bridge_bucket_states
                    .iter_mut()
                    .filter(|bucket| bucket.asset_id == operation.asset_id)
                {
                    bucket.last_packet_epoch = operation.epoch;
                    bucket.last_updated_height = block_height;
                    bucket
                        .validate()
                        .map_err(|error| ("bad_vault_bridge_bucket", error))?;
                }
            }
            Ok(())
        }
        AssetTransactionOperation::MarketOpsPolicyRegister(operation) => {
            if transaction.unsigned.transaction_kind != MARKET_OPS_POLICY_REGISTER_TRANSACTION_KIND
            {
                return Err((
                    "wrong_transaction_kind",
                    "market_ops_policy_register transaction kind mismatch".to_string(),
                ));
            }
            operation
                .policy
                .validate()
                .map_err(|error| ("bad_market_ops_policy", error))?;
            let nav_asset = ledger.nav_asset(&operation.asset_id).ok_or_else(|| {
                (
                    "missing_nav_asset",
                    format!("nav asset `{}` does not exist", operation.asset_id),
                )
            })?;
            if nav_asset.issuer != operation.issuer {
                return Err((
                    "nav_issuer_mismatch",
                    "market_ops_policy_register issuer does not match nav asset issuer"
                        .to_string(),
                ));
            }
            if ledger
                .market_ops_policies
                .iter()
                .any(|policy| policy == &operation.policy)
            {
                return Err((
                    "duplicate_market_ops_policy",
                    "market ops policy is already registered".to_string(),
                ));
            }
            ledger.market_ops_policies.push(operation.policy.clone());
            Ok(())
        }
        AssetTransactionOperation::MarketOpsFinalize(operation) => {
            if transaction.unsigned.transaction_kind != MARKET_OPS_FINALIZE_TRANSACTION_KIND {
                return Err((
                    "wrong_transaction_kind",
                    "market_ops_finalize transaction kind mismatch".to_string(),
                ));
            }
            finalize_market_ops_envelope(ledger, operation, block_height)
        }
        AssetTransactionOperation::NavMintAtNav(operation) => {
            if transaction.unsigned.transaction_kind != NAV_MINT_AT_NAV_TRANSACTION_KIND {
                return Err((
                    "wrong_transaction_kind",
                    "nav_mint_at_nav transaction kind mismatch".to_string(),
                ));
            }
            let nav_asset = ledger.nav_asset(&operation.asset_id).cloned().ok_or_else(|| {
                (
                    "missing_nav_asset",
                    format!("nav asset `{}` does not exist", operation.asset_id),
                )
            })?;
            ensure_nav_asset_live_for_epoch(
                ledger,
                &nav_asset,
                operation.epoch,
                &operation.reserve_packet_hash,
                block_height,
            )?;
            if nav_asset.issuer != operation.issuer {
                return Err((
                    "nav_issuer_mismatch",
                    "nav_mint_at_nav issuer does not match nav asset issuer".to_string(),
                ));
            }
            if let Some(profile) = nav_profile_for_asset(ledger, &nav_asset) {
                if profile.settle_deadline_blocks > 0 {
                    let overdue = ledger.nav_redemptions.iter().any(|redemption| {
                        redemption.asset_id == operation.asset_id
                            && redemption.state == NAV_REDEMPTION_STATE_PENDING
                            && redemption.created_at_height > 0
                            && block_height
                                > redemption
                                    .created_at_height
                                    .saturating_add(profile.settle_deadline_blocks)
                    });
                    if overdue {
                        return Err((
                            "nav_redemptions_overdue",
                            "nav mint blocked: pending redemptions exceed the profile settlement deadline".to_string(),
                        ));
                    }
                }
            }
            let asset = ledger
                .asset_definition(&operation.asset_id)
                .cloned()
                .ok_or_else(|| {
                    (
                        "missing_asset",
                        format!("asset `{}` does not exist", operation.asset_id),
                    )
                })?;
            if compatibility.bridge_verification_rules_active(block_height) {
                ensure_not_vault_bridge_out_of_lane_mint(
                    ledger,
                    &operation.asset_id,
                    "nav_mint_at_nav",
                )?;
            }
            let current_supply = issued_asset_supply(ledger, &operation.asset_id)?;
            let supply_after_mint = current_supply.checked_add(operation.amount).ok_or_else(|| {
                (
                    "issued_supply_overflow",
                    "nav mint would overflow issued supply".to_string(),
                )
            })?;
            if supply_after_mint > nav_asset.circulating_supply {
                return Err((
                    "nav_supply_cap_exceeded",
                    "nav mint would exceed finalized reserve packet supply".to_string(),
                ));
            }
            if let Some(max_supply) = asset.max_supply {
                if supply_after_mint > max_supply {
                    return Err((
                        "issued_supply_cap_exceeded",
                        "nav mint exceeds issued asset max_supply".to_string(),
                    ));
                }
            }
            let to_index = issued_asset_credit_recipient_line_index(
                ledger,
                &asset,
                &operation.to,
                operation.amount,
                "nav mint",
            )?;
            let (recipient_after, required_limit) = prepare_issued_asset_credit(
                ledger,
                &asset,
                &operation.to,
                to_index,
                operation.amount,
                "nav mint",
            )?;
            if operation.has_vault_bridge_settlement() {
                apply_nav_mint_vault_bridge_settlement(ledger, &nav_asset, operation, block_height)?;
            }
            apply_prepared_issued_asset_credit(ledger, to_index, recipient_after, required_limit);
            Ok(())
        }
        AssetTransactionOperation::NavRedeemAtNav(operation) => {
            if transaction.unsigned.transaction_kind != NAV_REDEEM_AT_NAV_TRANSACTION_KIND {
                return Err((
                    "wrong_transaction_kind",
                    "nav_redeem_at_nav transaction kind mismatch".to_string(),
                ));
            }
            let nav_asset = ledger.nav_asset(&operation.asset_id).cloned().ok_or_else(|| {
                (
                    "missing_nav_asset",
                    format!("nav asset `{}` does not exist", operation.asset_id),
                )
            })?;
            ensure_nav_asset_live_for_epoch(
                ledger,
                &nav_asset,
                operation.epoch,
                &operation.reserve_packet_hash,
                block_height,
            )?;
            if nav_asset.issuer != operation.issuer {
                return Err((
                    "nav_issuer_mismatch",
                    "nav_redeem_at_nav issuer does not match nav asset issuer".to_string(),
                ));
            }
            let asset = ledger
                .asset_definition(&operation.asset_id)
                .cloned()
                .ok_or_else(|| {
                    (
                        "missing_asset",
                        format!("asset `{}` does not exist", operation.asset_id),
                    )
                })?;
            let nav_unit_scale = 10_u128
                .checked_pow(asset.precision.into())
                .ok_or_else(|| {
                    (
                        "bad_nav_asset_precision",
                        "nav asset precision scale would overflow".to_string(),
                    )
                })?;
            let owner_index =
                trustline_index(ledger, &operation.owner, &operation.asset_id).ok_or_else(
                    || {
                        (
                            "missing_trustline",
                            "nav redeem owner has no trustline for asset".to_string(),
                        )
                    },
                )?;
            ensure_line_can_move(&asset, &ledger.trustlines[owner_index])?;
            if ledger.trustlines[owner_index].balance < operation.amount {
                return Err((
                    "insufficient_issued_balance",
                    "nav redeem amount exceeds owner balance".to_string(),
                ));
            }
            let redemption_id = nav_redemption_id(
                &genesis.chain_id,
                &operation.owner,
                &operation.asset_id,
                transaction.unsigned.sequence,
            )
            .map_err(|error| ("bad_nav_redemption", error))?;
            if ledger
                .nav_redemptions
                .iter()
                .any(|redemption| redemption.redemption_id == redemption_id)
            {
                return Err((
                    "duplicate_nav_redemption",
                    "nav redemption already exists".to_string(),
                ));
            }
            ledger.trustlines[owner_index].balance -= operation.amount;
            let mut redemption = NavRedemption::new_with_unit_scale(
                &genesis.chain_id,
                operation.owner.clone(),
                operation.issuer.clone(),
                operation.asset_id.clone(),
                transaction.unsigned.sequence,
                operation.amount,
                operation.epoch,
                nav_asset.nav_per_unit,
                nav_unit_scale,
                operation.reserve_packet_hash.clone(),
            )
            .map_err(|error| ("bad_nav_redemption", error))?;
            if nav_profile_for_asset(ledger, &nav_asset).is_some() {
                redemption.created_at_height = block_height;
            }
            ledger.nav_redemptions.push(redemption);
            Ok(())
        }
        AssetTransactionOperation::NavHalt(operation) => {
            if transaction.unsigned.transaction_kind != NAV_HALT_TRANSACTION_KIND {
                return Err((
                    "wrong_transaction_kind",
                    "nav_halt transaction kind mismatch".to_string(),
                ));
            }
            let nav_asset = ledger
                .nav_asset_mut(&operation.asset_id)
                .ok_or_else(|| {
                    (
                        "missing_nav_asset",
                        format!("nav asset `{}` does not exist", operation.asset_id),
                    )
                })?;
            if nav_asset.issuer != operation.issuer {
                return Err((
                    "nav_issuer_mismatch",
                    "nav_halt issuer does not match nav asset issuer".to_string(),
                ));
            }
            nav_asset.halted = operation.halted;
            nav_asset.halt_reason = if operation.halted {
                operation.reason.clone()
            } else {
                String::new()
            };
            Ok(())
        }
        AssetTransactionOperation::NavProfileRegister(operation) => {
            if transaction.unsigned.transaction_kind != NAV_PROFILE_REGISTER_TRANSACTION_KIND {
                return Err((
                    "wrong_transaction_kind",
                    "nav_profile_register transaction kind mismatch".to_string(),
                ));
            }
            let mut profile = NavProofProfile::new_with_bridge_observer_min_confirmations(
                operation.registrant.clone(),
                operation.verifier_kind.clone(),
                operation.effective_source_class(),
                operation.max_snapshot_age_blocks,
                operation.challenge_window_blocks,
                operation.max_epoch_gap_blocks,
                operation.settle_deadline_blocks,
                operation.min_challenge_bond,
                operation.min_attestations,
                operation.tolerance_bp,
                operation.bridge_observer_min_confirmations,
                operation.valuation_policy_hash.clone(),
                operation.sp1_program_vkey.clone(),
                operation.sp1_proof_encoding.clone(),
                operation.max_proof_bytes,
                operation.max_public_values_bytes,
            )
            .map_err(|error| ("bad_nav_profile", error))?;
            if !operation.vault_bridge_route_policy_hash.is_empty() {
                profile = profile
                    .with_vault_bridge_route_policy_hash(
                        operation.vault_bridge_route_policy_hash.clone(),
                    )
                    .map_err(|error| ("bad_nav_profile", error))?;
            }
            if ledger.nav_proof_profile(&profile.profile_id).is_some() {
                return Err((
                    "duplicate_nav_profile",
                    "nav proof profile with identical parameters is already registered".to_string(),
                ));
            }
            ledger.nav_proof_profiles.push(profile);
            Ok(())
        }
        AssetTransactionOperation::NavRedeemSettle(operation) => {
            if transaction.unsigned.transaction_kind != NAV_REDEEM_SETTLE_TRANSACTION_KIND {
                return Err((
                    "wrong_transaction_kind",
                    "nav_redeem_settle transaction kind mismatch".to_string(),
                ));
            }
            let nav_asset = ledger.nav_asset(&operation.asset_id).cloned().ok_or_else(|| {
                (
                    "missing_nav_asset",
                    format!("nav asset `{}` does not exist", operation.asset_id),
                )
            })?;
            if nav_asset.issuer != operation.issuer
                && nav_asset.redemption_account != operation.issuer
            {
                return Err((
                    "nav_issuer_mismatch",
                    "nav_redeem_settle issuer must be the nav asset issuer or redemption account".to_string(),
                ));
            }
            let redemption = ledger
                .nav_redemption_mut(&operation.redemption_id)
                .ok_or_else(|| {
                    (
                        "missing_nav_redemption",
                        "nav_redeem_settle references missing redemption".to_string(),
                    )
                })?;
            if redemption.asset_id != operation.asset_id {
                return Err((
                    "nav_redemption_asset_mismatch",
                    "nav_redeem_settle redemption does not belong to asset".to_string(),
                ));
            }
            if redemption.state != NAV_REDEMPTION_STATE_PENDING {
                return Err((
                    "nav_redemption_not_pending",
                    "nav_redeem_settle requires a pending redemption".to_string(),
                ));
            }
            let redemption_snapshot = redemption.clone();
            if operation.has_vault_bridge_settlement() {
                apply_nav_redeem_vault_bridge_settlement(
                    genesis,
                    ledger,
                    &nav_asset,
                    operation,
                    &redemption_snapshot,
                    block_height,
                )?;
            }
            let redemption = ledger
                .nav_redemption_mut(&operation.redemption_id)
                .ok_or_else(|| {
                    (
                        "missing_nav_redemption",
                        "nav_redeem_settle references missing redemption".to_string(),
                    )
                })?;
            redemption.state = NAV_REDEMPTION_STATE_SETTLED.to_string();
            redemption.settlement_receipt_hash = operation.settlement_receipt_hash.clone();
            Ok(())
        }
        AssetTransactionOperation::NavAttestorRegister(operation) => {
            if transaction.unsigned.transaction_kind != NAV_ATTESTOR_REGISTER_TRANSACTION_KIND {
                return Err((
                    "wrong_transaction_kind",
                    "nav_attestor_register transaction kind mismatch".to_string(),
                ));
            }
            if ledger.nav_attestor(&operation.attestor).is_some() {
                return Err((
                    "duplicate_nav_attestor",
                    "attestor address is already registered".to_string(),
                ));
            }
            if operation.bond > 0 {
                let account = ledger.account_mut(&operation.attestor).ok_or_else(|| {
                    (
                        "missing_attestor_account",
                        "nav attestor account does not exist".to_string(),
                    )
                })?;
                let balance_after = account.balance.checked_sub(operation.bond).ok_or_else(|| {
                    (
                        "insufficient_funds",
                        "attestor balance is too low for registration bond".to_string(),
                    )
                })?;
                if let Some(message) =
                    account_reserve_violation(&operation.attestor, balance_after)
                {
                    return Err(("below_account_reserve", message));
                }
                account.balance = balance_after;
            }
            ledger.nav_attestors.push(NavAttestor {
                address: operation.attestor.clone(),
                domain: operation.domain.clone(),
                bond: operation.bond,
                registered_at_height: block_height,
            });
            Ok(())
        }
        AssetTransactionOperation::NavReserveAttest(operation) => {
            if transaction.unsigned.transaction_kind != NAV_RESERVE_ATTEST_TRANSACTION_KIND {
                return Err((
                    "wrong_transaction_kind",
                    "nav_reserve_attest transaction kind mismatch".to_string(),
                ));
            }
            let nav_asset = ledger.nav_asset(&operation.asset_id).cloned().ok_or_else(|| {
                (
                    "missing_nav_asset",
                    format!("nav asset `{}` does not exist", operation.asset_id),
                )
            })?;
            let profile = nav_profile_for_asset(ledger, &nav_asset).cloned().ok_or_else(|| {
                (
                    "nav_profile_not_attestable",
                    "nav_reserve_attest requires a registered proof profile".to_string(),
                )
            })?;
            if profile.verifier_kind != NAV_PROFILE_VERIFIER_MULTI_FETCH {
                return Err((
                    "nav_profile_not_attestable",
                    "nav_reserve_attest only applies to multi-fetch-quorum profiles".to_string(),
                ));
            }
            if ledger.nav_attestor(&operation.attestor).is_none() {
                return Err((
                    "unregistered_nav_attestor",
                    "nav_reserve_attest requires a registered attestor (nav_attestor_register)".to_string(),
                ));
            }
            let packet = ledger
                .nav_reserve_packet_mut(
                    &operation.asset_id,
                    operation.epoch,
                    &operation.reserve_packet_hash,
                )
                .ok_or_else(|| {
                    (
                        "missing_nav_reserve_packet",
                        "nav_reserve_attest references missing reserve packet".to_string(),
                    )
                })?;
            if packet.state != NAV_RESERVE_STATE_SUBMITTED {
                return Err((
                    "nav_packet_not_submitted",
                    "nav_reserve_attest requires a submitted reserve packet".to_string(),
                ));
            }
            if packet
                .attestations
                .iter()
                .any(|attestation| attestation.attestor == operation.attestor)
            {
                return Err((
                    "duplicate_nav_attestation",
                    "attestor has already attested this reserve packet".to_string(),
                ));
            }
            if packet.attestations.len() >= MAX_NAV_ATTESTATIONS_PER_PACKET {
                return Err((
                    "nav_attestations_full",
                    "reserve packet attestation list is full".to_string(),
                ));
            }
            packet.attestations.push(NavReserveAttestation {
                attestor: operation.attestor.clone(),
                pass: operation.pass,
                observation_root: operation.observation_root.clone(),
                attested_at_height: block_height,
            });
            Ok(())
        }
        AssetTransactionOperation::VaultBridgeDepositPropose(operation) => {
            if transaction.unsigned.transaction_kind != VAULT_BRIDGE_DEPOSIT_PROPOSE_TRANSACTION_KIND {
                return Err((
                    "wrong_transaction_kind",
                    "vault_bridge_deposit_propose transaction kind mismatch".to_string(),
                ));
            }
            apply_vault_bridge_deposit_propose(ledger, operation, block_height)
        }
        AssetTransactionOperation::VaultBridgeDepositChallenge(operation) => {
            if transaction.unsigned.transaction_kind
                != VAULT_BRIDGE_DEPOSIT_CHALLENGE_TRANSACTION_KIND
            {
                return Err((
                    "wrong_transaction_kind",
                    "vault_bridge_deposit_challenge transaction kind mismatch".to_string(),
                ));
            }
            apply_vault_bridge_deposit_challenge(ledger, operation, block_height)
        }
        AssetTransactionOperation::VaultBridgeDepositAttest(operation) => {
            if transaction.unsigned.transaction_kind != VAULT_BRIDGE_DEPOSIT_ATTEST_TRANSACTION_KIND
            {
                return Err((
                    "wrong_transaction_kind",
                    "vault_bridge_deposit_attest transaction kind mismatch".to_string(),
                ));
            }
            apply_vault_bridge_deposit_attest_with_compatibility(
                ledger,
                operation,
                block_height,
                compatibility,
            )
        }
        AssetTransactionOperation::VaultBridgeDepositFinalize(operation) => {
            if transaction.unsigned.transaction_kind
                != VAULT_BRIDGE_DEPOSIT_FINALIZE_TRANSACTION_KIND
            {
                return Err((
                    "wrong_transaction_kind",
                    "vault_bridge_deposit_finalize transaction kind mismatch".to_string(),
                ));
            }
            apply_vault_bridge_deposit_finalize_with_compatibility(
                ledger,
                operation,
                block_height,
                compatibility,
            )
        }
        AssetTransactionOperation::VaultBridgeDepositClaim(operation) => {
            if transaction.unsigned.transaction_kind != VAULT_BRIDGE_DEPOSIT_CLAIM_TRANSACTION_KIND
            {
                return Err((
                    "wrong_transaction_kind",
                    "vault_bridge_deposit_claim transaction kind mismatch".to_string(),
                ));
            }
            apply_vault_bridge_deposit_claim(genesis, ledger, operation, block_height)
        }
        AssetTransactionOperation::VaultBridgeReceiptSubmit(operation) => {
            if transaction.unsigned.transaction_kind != VAULT_BRIDGE_RECEIPT_SUBMIT_TRANSACTION_KIND {
                return Err((
                    "wrong_transaction_kind",
                    "vault_bridge_receipt_submit transaction kind mismatch".to_string(),
                ));
            }
            apply_vault_bridge_receipt_submit(genesis, ledger, operation, block_height)
        }
        AssetTransactionOperation::VaultBridgeReceiptCount(operation) => {
            if transaction.unsigned.transaction_kind != VAULT_BRIDGE_RECEIPT_COUNT_TRANSACTION_KIND {
                return Err((
                    "wrong_transaction_kind",
                    "vault_bridge_receipt_count transaction kind mismatch".to_string(),
                ));
            }
            apply_vault_bridge_receipt_count(ledger, operation, block_height)
        }
        AssetTransactionOperation::VaultBridgeMintFromReceipts(operation) => {
            if transaction.unsigned.transaction_kind != VAULT_BRIDGE_MINT_FROM_RECEIPTS_TRANSACTION_KIND {
                return Err((
                    "wrong_transaction_kind",
                    "vault_bridge_mint_from_receipts transaction kind mismatch".to_string(),
                ));
            }
            apply_vault_bridge_mint_from_receipts(genesis, ledger, transaction, operation, block_height)
        }
        AssetTransactionOperation::VaultBridgeBurnToRedeem(operation) => {
            if transaction.unsigned.transaction_kind != VAULT_BRIDGE_BURN_TO_REDEEM_TRANSACTION_KIND {
                return Err((
                    "wrong_transaction_kind",
                    "vault_bridge_burn_to_redeem transaction kind mismatch".to_string(),
                ));
            }
            apply_vault_bridge_burn_to_redeem(
                genesis,
                ledger,
                transaction,
                operation,
                block_height,
                compatibility,
            )
        }
        AssetTransactionOperation::VaultBridgeRedeemSettle(operation) => {
            if transaction.unsigned.transaction_kind != VAULT_BRIDGE_REDEEM_SETTLE_TRANSACTION_KIND {
                return Err((
                    "wrong_transaction_kind",
                    "vault_bridge_redeem_settle transaction kind mismatch".to_string(),
                ));
            }
            apply_vault_bridge_redeem_settle_with_compatibility(
                ledger,
                operation,
                block_height,
                compatibility,
            )
        }
        AssetTransactionOperation::VaultBridgeBucketImpair(operation) => {
            if transaction.unsigned.transaction_kind != VAULT_BRIDGE_BUCKET_IMPAIR_TRANSACTION_KIND {
                return Err((
                    "wrong_transaction_kind",
                    "vault_bridge_bucket_impair transaction kind mismatch".to_string(),
                ));
            }
            apply_vault_bridge_bucket_impair(ledger, operation, block_height)
        }
        AssetTransactionOperation::VaultBridgeNavSubscriptionAllocate(operation) => {
            if transaction.unsigned.transaction_kind
                != VAULT_BRIDGE_NAV_SUBSCRIPTION_ALLOCATE_TRANSACTION_KIND
            {
                return Err((
                    "wrong_transaction_kind",
                    "vault_bridge_nav_subscription_allocate transaction kind mismatch".to_string(),
                ));
            }
            apply_vault_bridge_nav_subscription_allocate_with_compatibility(
                genesis,
                ledger,
                &transaction.unsigned.source,
                operation,
                block_height,
                compatibility,
            )
        }
        AssetTransactionOperation::PftlUniswapRouteInit(operation) => {
            if transaction.unsigned.transaction_kind != PFTL_UNISWAP_ROUTE_INIT_TRANSACTION_KIND {
                return Err((
                    "wrong_transaction_kind",
                    "pftl_uniswap_route_init transaction kind mismatch".to_string(),
                ));
            }
            if !compatibility.allow_unverified_pftl_uniswap_bridge_replay {
                crate::pftl_uniswap_ethereum_verification::verify_live_route_initialization(
                    genesis, ledger, operation,
                )?;
            }
            apply_pftl_uniswap_route_init(genesis, ledger, operation, block_height)
        }
        AssetTransactionOperation::PftlUniswapPrimarySubscribe(operation) => {
            if transaction.unsigned.transaction_kind != PFTL_UNISWAP_PRIMARY_SUBSCRIBE_TRANSACTION_KIND {
                return Err((
                    "wrong_transaction_kind",
                    "pftl_uniswap_primary_subscribe transaction kind mismatch".to_string(),
                ));
            }
            if !compatibility.allow_unverified_pftl_uniswap_bridge_replay {
                let route = ledger.pftl_uniswap_route(&operation.route_id).ok_or_else(|| {
                    (
                        "missing_pftl_uniswap_route",
                        format!("PFTL-Uniswap route `{}` is missing", operation.route_id),
                    )
                })?;
                crate::pftl_uniswap_ethereum_verification::verify_live_route_reference(
                    genesis, ledger, route,
                )?;
            }
            apply_pftl_uniswap_primary_subscribe(genesis, ledger, operation, block_height)
        }
        AssetTransactionOperation::PftlUniswapExportDebit(operation) => {
            if transaction.unsigned.transaction_kind != PFTL_UNISWAP_EXPORT_DEBIT_TRANSACTION_KIND {
                return Err((
                    "wrong_transaction_kind",
                    "pftl_uniswap_export_debit transaction kind mismatch".to_string(),
                ));
            }
            if !compatibility.allow_unverified_pftl_uniswap_bridge_replay {
                let route = ledger.pftl_uniswap_route(&operation.route_id).ok_or_else(|| {
                    (
                        "missing_pftl_uniswap_route",
                        format!("PFTL-Uniswap route `{}` is missing", operation.route_id),
                    )
                })?;
                crate::pftl_uniswap_ethereum_verification::verify_live_export(
                    genesis, ledger, route, operation,
                )?;
            }
            apply_pftl_uniswap_export_debit(genesis, ledger, operation, block_height)
        }
        AssetTransactionOperation::PftlUniswapDestinationConsume(operation) => {
            if transaction.unsigned.transaction_kind
                != PFTL_UNISWAP_DESTINATION_CONSUME_TRANSACTION_KIND
            {
                return Err((
                    "wrong_transaction_kind",
                    "pftl_uniswap_destination_consume transaction kind mismatch".to_string(),
                ));
            }
            if !compatibility.allow_unverified_pftl_uniswap_bridge_replay {
                let route = ledger.pftl_uniswap_route(&operation.route_id).ok_or_else(|| {
                    (
                        "missing_pftl_uniswap_route",
                        format!("PFTL-Uniswap route `{}` is missing", operation.route_id),
                    )
                })?;
                let packet = route.export_packets.get(&operation.packet_hash).ok_or_else(|| {
                    (
                        "unknown_pftl_uniswap_export_packet",
                        "destination consume references unknown export packet".to_string(),
                    )
                })?;
                crate::pftl_uniswap_ethereum_verification::verify_destination_consume(
                    genesis, ledger, route, packet, operation,
                )?;
            }
            apply_pftl_uniswap_destination_consume(genesis, ledger, operation, block_height)
        }
        AssetTransactionOperation::PftlUniswapRefundSource(operation) => {
            if transaction.unsigned.transaction_kind != PFTL_UNISWAP_REFUND_SOURCE_TRANSACTION_KIND {
                return Err((
                    "wrong_transaction_kind",
                    "pftl_uniswap_refund_source transaction kind mismatch".to_string(),
                ));
            }
            if !compatibility.allow_unverified_pftl_uniswap_bridge_replay {
                let route = ledger.pftl_uniswap_route(&operation.route_id).ok_or_else(|| {
                    (
                        "missing_pftl_uniswap_route",
                        format!("PFTL-Uniswap route `{}` is missing", operation.route_id),
                    )
                })?;
                let packet = route.export_packets.get(&operation.packet_hash).ok_or_else(|| {
                    (
                        "unknown_pftl_uniswap_export_packet",
                        "refund references unknown export packet".to_string(),
                    )
                })?;
                crate::pftl_uniswap_ethereum_verification::verify_source_refund(
                    genesis, ledger, route, packet, operation,
                )?;
            }
            apply_pftl_uniswap_refund_source(genesis, ledger, operation, block_height)
        }
        AssetTransactionOperation::PftlUniswapReturnImport(operation) => {
            if transaction.unsigned.transaction_kind != PFTL_UNISWAP_RETURN_IMPORT_TRANSACTION_KIND {
                return Err((
                    "wrong_transaction_kind",
                    "pftl_uniswap_return_import transaction kind mismatch".to_string(),
                ));
            }
            if !compatibility.allow_unverified_pftl_uniswap_bridge_replay {
                let route = ledger.pftl_uniswap_route(&operation.route_id).ok_or_else(|| {
                    (
                        "missing_pftl_uniswap_route",
                        format!("PFTL-Uniswap route `{}` is missing", operation.route_id),
                    )
                })?;
                crate::pftl_uniswap_ethereum_verification::verify_return_import(
                    genesis, ledger, route, operation,
                )?;
            }
            apply_pftl_uniswap_return_import(genesis, ledger, operation, block_height)
        }
    }
}
