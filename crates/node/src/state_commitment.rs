use super::*;

pub(super) fn replicated_state_root(
    genesis: &Genesis,
    governance: &GovernanceState,
    ledger: &LedgerState,
    ordered_batches: &[String],
    shielded: &ShieldedState,
    bridge: &BridgeState,
) -> io::Result<String> {
    replicated_state_root_with_nav_completeness(
        genesis,
        governance,
        ledger,
        ordered_batches,
        shielded,
        bridge,
        true,
        true,
        false,
        false,
    )
}

pub(super) fn legacy_nav_incomplete_replicated_state_root(
    genesis: &Genesis,
    governance: &GovernanceState,
    ledger: &LedgerState,
    ordered_batches: &[String],
    shielded: &ShieldedState,
    bridge: &BridgeState,
) -> io::Result<String> {
    replicated_state_root_with_nav_completeness(
        genesis,
        governance,
        ledger,
        ordered_batches,
        shielded,
        bridge,
        false,
        true,
        false,
        false,
    )
}

pub(super) fn legacy_nav_profile_sp1_uncommitted_replicated_state_root(
    genesis: &Genesis,
    governance: &GovernanceState,
    ledger: &LedgerState,
    ordered_batches: &[String],
    shielded: &ShieldedState,
    bridge: &BridgeState,
) -> io::Result<String> {
    replicated_state_root_with_nav_completeness(
        genesis,
        governance,
        ledger,
        ordered_batches,
        shielded,
        bridge,
        true,
        false,
        false,
        false,
    )
}

pub(super) fn legacy_vault_bridge_domainless_withdrawal_replicated_state_root(
    genesis: &Genesis,
    governance: &GovernanceState,
    ledger: &LedgerState,
    ordered_batches: &[String],
    shielded: &ShieldedState,
    bridge: &BridgeState,
) -> io::Result<String> {
    replicated_state_root_with_nav_completeness(
        genesis,
        governance,
        ledger,
        ordered_batches,
        shielded,
        bridge,
        true,
        true,
        true,
        false,
    )
}

pub(super) fn legacy_vault_bridge_deposit_attestation_replicated_state_root(
    genesis: &Genesis,
    governance: &GovernanceState,
    ledger: &LedgerState,
    ordered_batches: &[String],
    shielded: &ShieldedState,
    bridge: &BridgeState,
) -> io::Result<String> {
    replicated_state_root_with_nav_completeness(
        genesis,
        governance,
        ledger,
        ordered_batches,
        shielded,
        bridge,
        true,
        true,
        false,
        true,
    )
}

pub(super) fn bridge_verification_legacy_replay_allowed(
    governance: &GovernanceState,
    block_height: u64,
) -> bool {
    governance
        .bridge_verification_activation_height()
        .is_none_or(|activation_height| block_height < activation_height)
}

pub(super) fn legacy_nav_asset_uncommitted_replicated_state_root(
    genesis: &Genesis,
    governance: &GovernanceState,
    ledger: &LedgerState,
    ordered_batches: &[String],
    shielded: &ShieldedState,
    bridge: &BridgeState,
) -> io::Result<String> {
    let mut legacy_ledger = ledger.clone();
    legacy_ledger.nav_assets.clear();
    replicated_state_root_with_nav_completeness(
        genesis,
        governance,
        &legacy_ledger,
        ordered_batches,
        shielded,
        bridge,
        true,
        true,
        false,
        false,
    )
}

pub(super) fn archived_wan_devnet_legacy_nav_asset_commitment_allowed(
    genesis: &Genesis,
    block: &BlockRecord,
) -> bool {
    genesis.chain_id == "postfiat-wan-devnet" && block.header.height <= 9
}

pub(super) fn replicated_state_root_with_nav_completeness(
    genesis: &Genesis,
    governance: &GovernanceState,
    ledger: &LedgerState,
    ordered_batches: &[String],
    shielded: &ShieldedState,
    bridge: &BridgeState,
    commit_complete_nav_state: bool,
    commit_nav_profile_sp1_fields: bool,
    legacy_vault_bridge_domainless_withdrawal_packets: bool,
    legacy_vault_bridge_deposit_attestation_fields: bool,
) -> io::Result<String> {
    assert_genesis_commitment_inventory_complete(genesis);
    verify_global_issued_asset_supply_caps(ledger, shielded)?;
    let state_height = u64::try_from(ordered_batches.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "state height overflow"))?;
    let commit_fastlane_state = genesis
        .replicated_state_v2_activation_height
        .or_else(|| governance.replicated_state_v2_activation_height())
        .is_some_and(|activation_height| state_height >= activation_height);
    let genesis_hash_hex = genesis_hash(genesis);
    let mut state_bytes = Vec::new();
    append_canonical_str(&mut state_bytes, "chain_id", &genesis.chain_id);
    append_canonical_str(&mut state_bytes, "genesis_hash", &genesis_hash_hex);
    append_canonical_u32(
        &mut state_bytes,
        "protocol_version",
        genesis.protocol_version,
    );
    append_governance_state(&mut state_bytes, governance);
    append_ledger_state(
        &mut state_bytes,
        ledger,
        commit_complete_nav_state,
        commit_nav_profile_sp1_fields,
        legacy_vault_bridge_domainless_withdrawal_packets,
        legacy_vault_bridge_deposit_attestation_fields,
        commit_fastlane_state,
    )?;
    append_string_list(&mut state_bytes, "ordered_batch", ordered_batches);
    append_shielded_state(&mut state_bytes, shielded);
    append_bridge_state(&mut state_bytes, bridge);
    Ok(hash_hex("postfiat.replicated_state.v1", &state_bytes))
}

pub(super) fn verify_global_issued_asset_supply_caps(
    ledger: &LedgerState,
    shielded: &ShieldedState,
) -> io::Result<()> {
    validate_issued_supply_custody_inventory(ledger, shielded)?;
    let orchard_balances = shielded
        .orchard
        .as_ref()
        .map(|pool| pool.asset_orchard_balances.as_slice())
        .unwrap_or_default();
    let mut orchard_by_asset = BTreeMap::<&str, u64>::new();
    for balance in orchard_balances {
        if orchard_by_asset
            .insert(balance.asset_id.as_str(), balance.live_total)
            .is_some()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "duplicate AssetOrchard balance for issued asset `{}`",
                    balance.asset_id
                ),
            ));
        }
        if ledger.asset_definition(&balance.asset_id).is_none() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "AssetOrchard balance references unknown issued asset `{}`",
                    balance.asset_id
                ),
            ));
        }
    }
    for reserve in &ledger.fast_lane_reserves {
        if reserve.asset_id == postfiat_types::FastAssetIdV1::native_pft() {
            continue;
        }
        let asset_id = bytes_to_hex(&reserve.asset_id.0);
        if ledger.asset_definition(&asset_id).is_none() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("FastLane reserve references unknown issued asset `{asset_id}`"),
            ));
        }
    }

    for definition in &ledger.asset_definitions {
        let global_supply = issued_asset_global_supply_after_inventory(
            ledger,
            &orchard_by_asset,
            &definition.asset_id,
        )?;
        if definition
            .max_supply
            .is_some_and(|max_supply| global_supply > max_supply)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "issued asset supply cap exceeded for `{}`: global supply {} exceeds max_supply {}",
                    definition.asset_id,
                    global_supply,
                    definition.max_supply.expect("checked Some")
                ),
            ));
        }
        if let Some(nav_asset) = ledger.nav_asset(&definition.asset_id) {
            if nav_asset.finalized_epoch > 0 && global_supply > nav_asset.circulating_supply {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "issued asset supply exceeds finalized NAV circulating supply for `{}`: global supply {} exceeds finalized supply {}",
                        definition.asset_id, global_supply, nav_asset.circulating_supply
                    ),
                ));
            }
        }
    }
    Ok(())
}

/// Returns the complete live supply of one issued asset across every supported
/// custody lane, rejecting duplicate, unknown, or unsupported inventory.
///
/// This is public so invariant and recovery harnesses exercise the same
/// production boundary used by state commitment and supply-cap validation.
pub fn global_issued_asset_supply(
    ledger: &LedgerState,
    shielded: &ShieldedState,
    asset_id: &str,
) -> io::Result<u64> {
    validate_issued_supply_custody_inventory(ledger, shielded)?;
    if ledger.asset_definition(asset_id).is_none() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("missing issued asset `{asset_id}`"),
        ));
    }
    let orchard_by_asset = shielded
        .orchard
        .as_ref()
        .map(|pool| {
            pool.asset_orchard_balances
                .iter()
                .map(|balance| (balance.asset_id.as_str(), balance.live_total))
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    issued_asset_global_supply_after_inventory(ledger, &orchard_by_asset, asset_id)
}

fn issued_asset_global_supply_after_inventory(
    ledger: &LedgerState,
    orchard_by_asset: &BTreeMap<&str, u64>,
    asset_id: &str,
) -> io::Result<u64> {
    let public_fastlane_external =
        issued_asset_supply(ledger, asset_id).map_err(|(code, message)| {
            io::Error::new(io::ErrorKind::InvalidData, format!("{code}: {message}"))
        })?;
    let orchard_live = orchard_by_asset.get(asset_id).copied().unwrap_or(0);
    public_fastlane_external
        .checked_add(orchard_live)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "issued asset supply overflow for `{asset_id}` across transparent, FastLane, external bridge, and AssetOrchard custody"
                ),
            )
        })
}

fn validate_issued_supply_custody_inventory(
    ledger: &LedgerState,
    shielded: &ShieldedState,
) -> io::Result<()> {
    // Deliberately exhaustive. Any new replicated field must be classified as
    // live issued custody or explicit non-custody before this code compiles.
    let LedgerState {
        accounts: _,
        asset_definitions,
        trustlines,
        escrows,
        nfts: _,
        offers,
        nav_assets: _,
        nav_reserve_packets: _,
        nav_redemptions: _,
        nav_proof_profiles: _,
        nav_attestors: _,
        market_ops_policies: _,
        market_ops_envelopes: _,
        vault_bridge_receipts: _,
        vault_bridge_bucket_states: _,
        vault_bridge_allocations: _,
        vault_bridge_redemptions: _,
        vault_bridge_deposits: _,
        pftl_uniswap_routes,
        pftl_uniswap_receipts: _,
        owned_objects,
        fastpay_recovery_policy: _,
        fastpay_recovery_committees: _,
        fastpay_recovery_reveals: _,
        fastpay_version_fences: _,
        fast_lane_reserves,
        fast_lane_deposit_receipts: _,
        redeemed_fast_lane_exit_claims: _,
        fast_lane_asset_rules: _,
        fast_lane_holder_permits: _,
        fastswap_policy_snapshots: _,
        fastswap_committees: _,
        fast_lane_prepare_fences: _,
        fast_lane_checkpoint_anchors: _,
        fastswap_activation_height: _,
        ethereum_arbitrum_finality_states: _,
    } = ledger;
    let ShieldedState {
        next_note_position: _,
        notes: _,
        nullifiers: _,
        turnstile_events: _,
        orchard,
    } = shielded;

    let mut definition_ids = BTreeSet::new();
    for definition in asset_definitions {
        if !definition_ids.insert(definition.asset_id.as_str()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "duplicate issued asset definition `{}`",
                    definition.asset_id
                ),
            ));
        }
    }
    let known = |asset_id: &str| definition_ids.contains(asset_id);

    let mut trustline_keys = BTreeSet::new();
    for line in trustlines {
        if !known(&line.asset_id) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "trustline references unknown issued asset `{}`",
                    line.asset_id
                ),
            ));
        }
        if !trustline_keys.insert((line.account.as_str(), line.asset_id.as_str())) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "duplicate issued trustline for account `{}` and asset `{}`",
                    line.account, line.asset_id
                ),
            ));
        }
    }

    let mut escrow_ids = BTreeSet::new();
    for escrow in escrows {
        if !escrow_ids.insert(escrow.escrow_id.as_str()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "duplicate escrow `{}` in issued custody inventory",
                    escrow.escrow_id
                ),
            ));
        }
        if escrow.asset_id != postfiat_execution::NATIVE_PFT_ESCROW_ASSET_ID
            && !known(&escrow.asset_id)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "escrow references unknown issued asset `{}`",
                    escrow.asset_id
                ),
            ));
        }
    }

    let mut offer_ids = BTreeSet::new();
    for offer in offers {
        if !offer_ids.insert(offer.offer_id.as_str()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "duplicate offer `{}` in issued custody inventory",
                    offer.offer_id
                ),
            ));
        }
        for asset_id in [&offer.taker_gets_asset_id, &offer.taker_pays_asset_id] {
            if asset_id != postfiat_execution::NATIVE_PFT_ESCROW_ASSET_ID && !known(asset_id) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("offer references unknown issued asset `{asset_id}`"),
                ));
            }
        }
    }

    let mut object_keys = BTreeSet::new();
    for object in owned_objects {
        if !object_keys.insert((object.id.as_str(), object.version)) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "duplicate owned object `{}` in issued custody inventory",
                    object.id
                ),
            ));
        }
        if object.asset != postfiat_execution::OWNED_NATIVE_ASSET {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "unsupported issued owned-object custody for object `{}` asset `{}`",
                    object.id, object.asset
                ),
            ));
        }
    }

    let mut reserve_assets = BTreeSet::new();
    for reserve in fast_lane_reserves {
        if !reserve_assets.insert(reserve.asset_id) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "duplicate FastLane reserve for asset `{}`",
                    bytes_to_hex(&reserve.asset_id.0)
                ),
            ));
        }
        if reserve.asset_id != postfiat_types::FastAssetIdV1::native_pft() {
            let asset_id = bytes_to_hex(&reserve.asset_id.0);
            if !known(&asset_id) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("FastLane reserve references unknown issued asset `{asset_id}`"),
                ));
            }
        }
    }

    let mut route_ids = BTreeSet::new();
    for route in pftl_uniswap_routes {
        if !route_ids.insert(route.route_id.as_str()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("duplicate PFTL-Uniswap route `{}`", route.route_id),
            ));
        }
        if !known(&route.native_nav_asset_id) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "PFTL-Uniswap route references unknown issued asset `{}`",
                    route.native_nav_asset_id
                ),
            ));
        }
    }

    if let Some(pool) = orchard {
        let mut orchard_assets = BTreeSet::new();
        for balance in &pool.asset_orchard_balances {
            if !known(&balance.asset_id) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "AssetOrchard balance references unknown issued asset `{}`",
                        balance.asset_id
                    ),
                ));
            }
            if !orchard_assets.insert(balance.asset_id.as_str()) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "duplicate AssetOrchard balance for issued asset `{}`",
                        balance.asset_id
                    ),
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn legacy_json_replicated_state_root(
    genesis: &Genesis,
    governance: &GovernanceState,
    ledger: &LedgerState,
    ordered_batches: &[String],
    shielded: &ShieldedState,
    bridge: &BridgeState,
) -> io::Result<String> {
    let genesis_hash_hex = genesis_hash(genesis);
    let legacy_governance = LegacyJsonGovernanceState {
        active_validator_count: governance.active_validator_count,
        active_validators: governance.active_validators.as_slice(),
        crypto_policy_version: governance.crypto_policy_version,
        bridge_witness_epoch: governance.bridge_witness_epoch,
        validator_registry_updates: governance.validator_registry_updates.as_slice(),
        amendment_activation_records: governance.amendment_activation_records.as_slice(),
        amendment_supersession_records: governance.amendment_supersession_records.as_slice(),
        amendment_rollback_records: governance.amendment_rollback_records.as_slice(),
        amendments: governance.amendments.as_slice(),
    };
    let legacy_ledger = LegacyJsonLedgerState {
        accounts: ledger.accounts.as_slice(),
    };
    let legacy_shielded = LegacyJsonShieldedState {
        notes: shielded.notes.as_slice(),
        nullifiers: shielded.nullifiers.as_slice(),
        turnstile_events: shielded.turnstile_events.as_slice(),
    };
    let state_bytes = serde_json::to_vec(&(
        genesis.chain_id.as_str(),
        genesis_hash_hex.as_str(),
        genesis.protocol_version,
        legacy_governance,
        legacy_ledger,
        ordered_batches,
        legacy_shielded,
        bridge,
    ))
    .map_err(invalid_data)?;
    Ok(hash_hex("postfiat.replicated_state.v1", &state_bytes))
}

#[derive(Serialize)]
pub(super) struct LegacyJsonGovernanceState<'a> {
    pub(super) active_validator_count: u32,
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    pub(super) active_validators: &'a [String],
    pub(super) crypto_policy_version: u32,
    pub(super) bridge_witness_epoch: u32,
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    pub(super) validator_registry_updates: &'a [ValidatorRegistryUpdateRecord],
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    pub(super) amendment_activation_records: &'a [GovernanceAmendmentActivationRecord],
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    pub(super) amendment_supersession_records: &'a [GovernanceAmendmentSupersessionRecord],
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    pub(super) amendment_rollback_records: &'a [GovernanceAmendmentRollbackRecord],
    pub(super) amendments: &'a [GovernanceAmendment],
}

#[derive(Serialize)]
pub(super) struct LegacyJsonLedgerState<'a> {
    pub(super) accounts: &'a [Account],
}

#[derive(Serialize)]
pub(super) struct LegacyJsonShieldedState<'a> {
    pub(super) notes: &'a [ShieldedNote],
    pub(super) nullifiers: &'a [String],
    pub(super) turnstile_events: &'a [TurnstileEvent],
}

pub(super) fn append_governance_state(bytes: &mut Vec<u8>, governance: &GovernanceState) {
    assert_governance_state_commitment_inventory_complete(governance);
    append_canonical_u32(
        bytes,
        "governance.active_validator_count",
        governance.active_validator_count,
    );
    append_string_list(
        bytes,
        "governance.active_validator",
        &governance.active_validators,
    );
    append_canonical_u32(
        bytes,
        "governance.crypto_policy_version",
        governance.crypto_policy_version,
    );
    append_canonical_u32(
        bytes,
        "governance.bridge_witness_epoch",
        governance.bridge_witness_epoch,
    );
    if governance.authority_mode != 0 {
        append_canonical_u32(
            bytes,
            "governance.authority_mode",
            governance.authority_mode,
        );
    }
    if governance.orchard_pool_paused {
        append_canonical_u32(bytes, "governance.orchard_pool_paused", 1);
    }
    if governance.atomic_swap_paused {
        append_canonical_u32(bytes, "governance.atomic_swap_paused", 1);
    }
    append_canonical_usize(
        bytes,
        "governance.validator_registry_update_count",
        governance.validator_registry_updates.len(),
    );
    for record in &governance.validator_registry_updates {
        append_validator_registry_update_record(
            bytes,
            "governance.validator_registry_update",
            record,
        );
    }
    append_canonical_usize(
        bytes,
        "governance.activation_record_count",
        governance.amendment_activation_records.len(),
    );
    for record in &governance.amendment_activation_records {
        append_governance_activation_record(bytes, "governance.activation_record", record);
    }
    append_canonical_usize(
        bytes,
        "governance.supersession_record_count",
        governance.amendment_supersession_records.len(),
    );
    for record in &governance.amendment_supersession_records {
        append_governance_supersession_record(bytes, "governance.supersession_record", record);
    }
    append_canonical_usize(
        bytes,
        "governance.rollback_record_count",
        governance.amendment_rollback_records.len(),
    );
    for record in &governance.amendment_rollback_records {
        append_governance_rollback_record(bytes, "governance.rollback_record", record);
    }
    if !governance.governance_agent_dry_run_records.is_empty() {
        append_canonical_usize(
            bytes,
            "governance.agent_dry_run_record_count",
            governance.governance_agent_dry_run_records.len(),
        );
        for record in &governance.governance_agent_dry_run_records {
            append_governance_agent_dry_run_record(
                bytes,
                "governance.agent_dry_run_record",
                record,
            );
        }
    }
    if !governance.vault_bridge_route_profiles.is_empty() {
        let mut profiles = governance
            .vault_bridge_route_profiles
            .iter()
            .collect::<Vec<_>>();
        profiles.sort_by(|left, right| {
            left.profile
                .asset_id
                .cmp(&right.profile.asset_id)
                .then(left.profile.route_epoch.cmp(&right.profile.route_epoch))
                .then(left.profile_hash.cmp(&right.profile_hash))
        });
        append_canonical_usize(
            bytes,
            "governance.vault_bridge_route_profile_count",
            profiles.len(),
        );
        for record in profiles {
            append_vault_bridge_route_profile_record(
                bytes,
                "governance.vault_bridge_route_profile",
                record,
            );
        }
    }
    append_canonical_usize(
        bytes,
        "governance.amendment_count",
        governance.amendments.len(),
    );
    for amendment in &governance.amendments {
        append_governance_amendment(bytes, "governance.amendment", amendment);
    }
}

pub(super) fn append_ledger_state(
    bytes: &mut Vec<u8>,
    ledger: &LedgerState,
    commit_complete_nav_state: bool,
    commit_nav_profile_sp1_fields: bool,
    legacy_vault_bridge_domainless_withdrawal_packets: bool,
    legacy_vault_bridge_deposit_attestation_fields: bool,
    commit_fastlane_state: bool,
) -> io::Result<()> {
    assert_ledger_state_commitment_inventory_complete(ledger);
    let mut accounts = ledger.accounts.iter().collect::<Vec<_>>();
    accounts.sort_by(|left, right| left.address.cmp(&right.address));
    append_canonical_usize(bytes, "ledger.account_count", accounts.len());
    for account in accounts {
        append_account(bytes, "ledger.account", account);
    }

    let mut assets = ledger.asset_definitions.iter().collect::<Vec<_>>();
    assets.sort_by(|left, right| left.asset_id.cmp(&right.asset_id));
    append_canonical_usize(bytes, "ledger.asset_count", assets.len());
    for asset in assets {
        append_asset_definition(bytes, "ledger.asset", asset);
    }

    let mut trustlines = ledger.trustlines.iter().collect::<Vec<_>>();
    trustlines.sort_by(|left, right| left.trustline_id.cmp(&right.trustline_id));
    append_canonical_usize(bytes, "ledger.trustline_count", trustlines.len());
    for trustline in trustlines {
        append_trustline(bytes, "ledger.trustline", trustline);
    }

    let mut escrows = ledger.escrows.iter().collect::<Vec<_>>();
    escrows.sort_by(|left, right| left.escrow_id.cmp(&right.escrow_id));
    append_canonical_usize(bytes, "ledger.escrow_count", escrows.len());
    for escrow in escrows {
        append_escrow(bytes, "ledger.escrow", escrow);
    }

    let mut nfts = ledger.nfts.iter().collect::<Vec<_>>();
    nfts.sort_by(|left, right| left.nft_id.cmp(&right.nft_id));
    append_canonical_usize(bytes, "ledger.nft_count", nfts.len());
    for nft in nfts {
        append_nft(bytes, "ledger.nft", nft);
    }

    let mut offers = ledger.offers.iter().collect::<Vec<_>>();
    offers.sort_by(|left, right| left.offer_id.cmp(&right.offer_id));
    append_canonical_usize(bytes, "ledger.offer_count", offers.len());
    for offer in offers {
        append_offer(bytes, "ledger.offer", offer);
    }

    let mut nav_assets = ledger.nav_assets.iter().collect::<Vec<_>>();
    nav_assets.sort_by(|left, right| left.asset_id.cmp(&right.asset_id));
    append_canonical_usize(bytes, "ledger.nav_asset_count", nav_assets.len());
    for nav_asset in nav_assets {
        append_nav_tracked_asset(bytes, "ledger.nav_asset", nav_asset);
    }

    let mut nav_reserve_packets = ledger.nav_reserve_packets.iter().collect::<Vec<_>>();
    nav_reserve_packets.sort_by(|left, right| left.packet_id.cmp(&right.packet_id));
    append_canonical_usize(
        bytes,
        "ledger.nav_reserve_packet_count",
        nav_reserve_packets.len(),
    );
    for packet in nav_reserve_packets {
        append_nav_reserve_packet(
            bytes,
            "ledger.nav_reserve_packet",
            packet,
            commit_complete_nav_state,
        );
    }

    let mut nav_redemptions = ledger.nav_redemptions.iter().collect::<Vec<_>>();
    nav_redemptions.sort_by(|left, right| left.redemption_id.cmp(&right.redemption_id));
    append_canonical_usize(bytes, "ledger.nav_redemption_count", nav_redemptions.len());
    for redemption in nav_redemptions {
        append_nav_redemption(bytes, "ledger.nav_redemption", redemption);
    }

    if commit_complete_nav_state && !ledger.nav_proof_profiles.is_empty() {
        let mut nav_proof_profiles = ledger.nav_proof_profiles.iter().collect::<Vec<_>>();
        nav_proof_profiles.sort_by(|left, right| left.profile_id.cmp(&right.profile_id));
        append_canonical_usize(
            bytes,
            "ledger.nav_proof_profile_count",
            nav_proof_profiles.len(),
        );
        for profile in nav_proof_profiles {
            append_nav_proof_profile(
                bytes,
                "ledger.nav_proof_profile",
                profile,
                commit_nav_profile_sp1_fields,
            );
        }
    }

    if commit_complete_nav_state && !ledger.nav_attestors.is_empty() {
        let mut nav_attestors = ledger.nav_attestors.iter().collect::<Vec<_>>();
        nav_attestors.sort_by(|left, right| left.address.cmp(&right.address));
        append_canonical_usize(bytes, "ledger.nav_attestor_count", nav_attestors.len());
        for attestor in nav_attestors {
            append_nav_attestor(bytes, "ledger.nav_attestor", attestor);
        }
    }

    if commit_complete_nav_state && !ledger.market_ops_policies.is_empty() {
        let mut market_ops_policies = ledger.market_ops_policies.iter().collect::<Vec<_>>();
        market_ops_policies.sort_by(|left, right| {
            left.program_id
                .cmp(&right.program_id)
                .then(left.policy_hash.cmp(&right.policy_hash))
                .then(left.parameter_hash.cmp(&right.parameter_hash))
                .then(left.venue_id.cmp(&right.venue_id))
                .then(left.pool_config_hash.cmp(&right.pool_config_hash))
                .then(left.hook_code_hash.cmp(&right.hook_code_hash))
                .then(left.activation_epoch.cmp(&right.activation_epoch))
        });
        append_canonical_usize(
            bytes,
            "ledger.market_ops_policy_count",
            market_ops_policies.len(),
        );
        for policy in market_ops_policies {
            append_market_ops_policy(bytes, "ledger.market_ops_policy", policy);
        }
    }

    if commit_complete_nav_state && !ledger.market_ops_envelopes.is_empty() {
        let mut market_ops_envelopes = ledger.market_ops_envelopes.iter().collect::<Vec<_>>();
        market_ops_envelopes.sort_by(|left, right| {
            left.asset_id
                .cmp(&right.asset_id)
                .then(left.epoch.cmp(&right.epoch))
                .then(left.envelope_hash.cmp(&right.envelope_hash))
        });
        append_canonical_usize(
            bytes,
            "ledger.market_ops_envelope_count",
            market_ops_envelopes.len(),
        );
        for record in market_ops_envelopes {
            append_finalized_market_ops_envelope(bytes, "ledger.market_ops_envelope", record);
        }
    }

    if commit_complete_nav_state && !ledger.vault_bridge_receipts.is_empty() {
        let mut vault_bridge_receipts = ledger.vault_bridge_receipts.iter().collect::<Vec<_>>();
        vault_bridge_receipts.sort_by(|left, right| left.receipt_id.cmp(&right.receipt_id));
        append_canonical_usize(
            bytes,
            "ledger.vault_bridge_receipt_count",
            vault_bridge_receipts.len(),
        );
        for receipt in vault_bridge_receipts {
            append_vault_bridge_receipt(bytes, "ledger.vault_bridge_receipt", receipt);
        }
    }

    if commit_complete_nav_state && !ledger.vault_bridge_deposits.is_empty() {
        let mut vault_bridge_deposits = ledger.vault_bridge_deposits.iter().collect::<Vec<_>>();
        vault_bridge_deposits.sort_by(|left, right| {
            left.asset_id
                .cmp(&right.asset_id)
                .then(left.evidence_root.cmp(&right.evidence_root))
        });
        append_canonical_usize(
            bytes,
            "ledger.vault_bridge_deposit_count",
            vault_bridge_deposits.len(),
        );
        for record in vault_bridge_deposits {
            append_vault_bridge_deposit_record(
                bytes,
                "ledger.vault_bridge_deposit",
                record,
                legacy_vault_bridge_deposit_attestation_fields,
            );
        }
    }

    if commit_complete_nav_state && !ledger.vault_bridge_bucket_states.is_empty() {
        let mut vault_bridge_buckets = ledger.vault_bridge_bucket_states.iter().collect::<Vec<_>>();
        vault_bridge_buckets.sort_by(|left, right| {
            left.asset_id
                .cmp(&right.asset_id)
                .then(left.bucket_id.cmp(&right.bucket_id))
        });
        append_canonical_usize(
            bytes,
            "ledger.vault_bridge_bucket_count",
            vault_bridge_buckets.len(),
        );
        for bucket in vault_bridge_buckets {
            append_vault_bridge_bucket(bytes, "ledger.vault_bridge_bucket", bucket);
        }
    }

    if commit_complete_nav_state && !ledger.vault_bridge_allocations.is_empty() {
        let mut vault_bridge_allocations =
            ledger.vault_bridge_allocations.iter().collect::<Vec<_>>();
        vault_bridge_allocations
            .sort_by(|left, right| left.allocation_id.cmp(&right.allocation_id));
        append_canonical_usize(
            bytes,
            "ledger.vault_bridge_allocation_count",
            vault_bridge_allocations.len(),
        );
        for allocation in vault_bridge_allocations {
            append_vault_bridge_allocation(bytes, "ledger.vault_bridge_allocation", allocation);
        }
    }

    if commit_complete_nav_state && !ledger.vault_bridge_redemptions.is_empty() {
        let mut vault_bridge_redemptions =
            ledger.vault_bridge_redemptions.iter().collect::<Vec<_>>();
        vault_bridge_redemptions
            .sort_by(|left, right| left.redemption_id.cmp(&right.redemption_id));
        append_canonical_usize(
            bytes,
            "ledger.vault_bridge_redemption_count",
            vault_bridge_redemptions.len(),
        );
        for redemption in vault_bridge_redemptions {
            append_vault_bridge_redemption(
                bytes,
                "ledger.vault_bridge_redemption",
                redemption,
                legacy_vault_bridge_domainless_withdrawal_packets,
            );
        }
    }

    if commit_complete_nav_state && !ledger.pftl_uniswap_routes.is_empty() {
        let mut pftl_uniswap_routes = ledger.pftl_uniswap_routes.iter().collect::<Vec<_>>();
        pftl_uniswap_routes.sort_by(|left, right| left.route_id.cmp(&right.route_id));
        append_canonical_usize(
            bytes,
            "ledger.pftl_uniswap_route_count",
            pftl_uniswap_routes.len(),
        );
        for route in pftl_uniswap_routes {
            append_pftl_uniswap_route(bytes, "ledger.pftl_uniswap_route", route);
        }
    }

    if commit_complete_nav_state && !ledger.pftl_uniswap_receipts.is_empty() {
        let mut pftl_uniswap_receipts = ledger.pftl_uniswap_receipts.iter().collect::<Vec<_>>();
        pftl_uniswap_receipts.sort_by(|left, right| left.receipt_hash.cmp(&right.receipt_hash));
        append_canonical_usize(
            bytes,
            "ledger.pftl_uniswap_receipt_count",
            pftl_uniswap_receipts.len(),
        );
        for receipt in pftl_uniswap_receipts {
            append_pftl_uniswap_receipt(bytes, "ledger.pftl_uniswap_receipt", receipt);
        }
    }

    if commit_complete_nav_state && !ledger.owned_objects.is_empty() {
        let mut owned_objects = ledger.owned_objects.iter().collect::<Vec<_>>();
        owned_objects.sort_by(|left, right| {
            left.id
                .cmp(&right.id)
                .then(left.version.cmp(&right.version))
                .then(left.asset.cmp(&right.asset))
        });
        append_canonical_usize(bytes, "ledger.owned_object_count", owned_objects.len());
        for object in owned_objects {
            append_owned_object(bytes, "ledger.owned_object", object);
        }
    }

    if commit_complete_nav_state && !ledger.ethereum_arbitrum_finality_states.is_empty() {
        append_sorted_canonical_commitments(
            bytes,
            "ledger.ethereum_arbitrum_finality_state",
            ledger
                .ethereum_arbitrum_finality_states
                .iter()
                .map(|value| value.state_commitment_bytes()),
        )?;
    }

    let fastlane_state_present = commit_fastlane_state
        && (!ledger.fast_lane_reserves.is_empty()
            || !ledger.fast_lane_deposit_receipts.is_empty()
            || !ledger.redeemed_fast_lane_exit_claims.is_empty()
            || !ledger.fast_lane_asset_rules.is_empty()
            || !ledger.fast_lane_holder_permits.is_empty()
            || !ledger.fastswap_policy_snapshots.is_empty()
            || !ledger.fastswap_committees.is_empty()
            || !ledger.fast_lane_prepare_fences.is_empty()
            || !ledger.fast_lane_checkpoint_anchors.is_empty()
            || ledger.fastpay_recovery_policy.is_some()
            || !ledger.fastpay_recovery_committees.is_empty()
            || !ledger.fastpay_recovery_reveals.is_empty()
            || !ledger.fastpay_version_fences.is_empty()
            || ledger.fastswap_activation_height.is_some());
    if fastlane_state_present {
        append_canonical_bool(bytes, "ledger.fastlane_state.present", true);
        append_sorted_canonical_commitments(
            bytes,
            "ledger.fastlane_reserve",
            ledger
                .fast_lane_reserves
                .iter()
                .map(|value| value.state_commitment_bytes()),
        )?;
        append_sorted_canonical_commitments(
            bytes,
            "ledger.fastlane_deposit_receipt",
            ledger
                .fast_lane_deposit_receipts
                .iter()
                .map(|value| value.state_commitment_bytes()),
        )?;
        append_sorted_raw_commitments(
            bytes,
            "ledger.fastlane_redeemed_exit_claim",
            ledger
                .redeemed_fast_lane_exit_claims
                .iter()
                .map(|value| value.0.to_vec()),
        )?;
        append_sorted_canonical_commitments(
            bytes,
            "ledger.fastlane_asset_rule",
            ledger
                .fast_lane_asset_rules
                .iter()
                .map(|value| value.canonical_bytes()),
        )?;
        append_sorted_canonical_commitments(
            bytes,
            "ledger.fastlane_holder_permit",
            ledger
                .fast_lane_holder_permits
                .iter()
                .map(|value| value.state_commitment_bytes()),
        )?;
        append_sorted_canonical_commitments(
            bytes,
            "ledger.fastswap_policy_snapshot",
            ledger
                .fastswap_policy_snapshots
                .iter()
                .map(|value| value.state_commitment_bytes()),
        )?;
        append_sorted_canonical_commitments(
            bytes,
            "ledger.fastswap_committee",
            ledger
                .fastswap_committees
                .iter()
                .map(|value| value.state_commitment_bytes()),
        )?;
        append_sorted_canonical_commitments(
            bytes,
            "ledger.fastlane_prepare_fence",
            ledger
                .fast_lane_prepare_fences
                .iter()
                .map(|value| value.state_commitment_bytes()),
        )?;
        append_sorted_canonical_commitments(
            bytes,
            "ledger.fastlane_checkpoint_anchor",
            ledger
                .fast_lane_checkpoint_anchors
                .iter()
                .map(|value| value.state_commitment_bytes()),
        )?;
        let fastpay_recovery_state_present = ledger.fastpay_recovery_policy.is_some()
            || !ledger.fastpay_recovery_committees.is_empty()
            || !ledger.fastpay_recovery_reveals.is_empty()
            || !ledger.fastpay_version_fences.is_empty();
        if fastpay_recovery_state_present {
            append_canonical_bool(bytes, "ledger.fastpay_recovery_state.present", true);
            append_sorted_canonical_commitments(
                bytes,
                "ledger.fastpay_recovery_policy",
                ledger
                    .fastpay_recovery_policy
                    .iter()
                    .map(|value| value.state_commitment_bytes()),
            )?;
            append_sorted_canonical_commitments(
                bytes,
                "ledger.fastpay_recovery_committee",
                ledger
                    .fastpay_recovery_committees
                    .iter()
                    .map(|value| value.state_commitment_bytes()),
            )?;
            append_sorted_canonical_commitments(
                bytes,
                "ledger.fastpay_recovery_reveal",
                ledger
                    .fastpay_recovery_reveals
                    .iter()
                    .map(|value| value.state_commitment_bytes()),
            )?;
            append_sorted_canonical_commitments(
                bytes,
                "ledger.fastpay_version_fence",
                ledger
                    .fastpay_version_fences
                    .iter()
                    .map(|value| value.state_commitment_bytes()),
            )?;
        }
        append_option_u64(
            bytes,
            "ledger.fastswap_activation_height",
            ledger.fastswap_activation_height,
        );
    }

    Ok(())
}

// Deliberately exhaustive: adding a consensus LedgerState field must fail the
// build until its state-root commitment and regression are added here.
fn assert_ledger_state_commitment_inventory_complete(ledger: &LedgerState) {
    let LedgerState {
        accounts: _,
        asset_definitions: _,
        trustlines: _,
        escrows: _,
        nfts: _,
        offers: _,
        nav_assets: _,
        nav_reserve_packets: _,
        nav_redemptions: _,
        nav_proof_profiles: _,
        nav_attestors: _,
        market_ops_policies: _,
        market_ops_envelopes: _,
        vault_bridge_receipts: _,
        vault_bridge_bucket_states: _,
        vault_bridge_allocations: _,
        vault_bridge_redemptions: _,
        vault_bridge_deposits: _,
        pftl_uniswap_routes: _,
        pftl_uniswap_receipts: _,
        owned_objects: _,
        fastpay_recovery_policy: _,
        fastpay_recovery_committees: _,
        fastpay_recovery_reveals: _,
        fastpay_version_fences: _,
        fast_lane_reserves: _,
        fast_lane_deposit_receipts: _,
        redeemed_fast_lane_exit_claims: _,
        fast_lane_asset_rules: _,
        fast_lane_holder_permits: _,
        fastswap_policy_snapshots: _,
        fastswap_committees: _,
        fast_lane_prepare_fences: _,
        fast_lane_checkpoint_anchors: _,
        fastswap_activation_height: _,
        ethereum_arbitrum_finality_states: _,
    } = ledger;
}

// Deliberately exhaustive: every replicated top-level domain fails compilation
// when a field is added until its commitment and compatibility disposition are
// reviewed. Nested canonical encoders retain their own field-level tests.
fn assert_genesis_commitment_inventory_complete(genesis: &Genesis) {
    let Genesis {
        chain_id: _,
        protocol_version: _,
        validator_count: _,
        native_supply_atoms: _,
        replicated_state_v2_activation_height: _,
        bridge_verification_activation_height: _,
        atomic_swap_activation_height: _,
        consensus_v2_activation_height: _,
    } = genesis;
}

fn assert_governance_state_commitment_inventory_complete(governance: &GovernanceState) {
    let GovernanceState {
        active_validator_count: _,
        active_validators: _,
        crypto_policy_version: _,
        bridge_witness_epoch: _,
        authority_mode: _,
        orchard_pool_paused: _,
        atomic_swap_paused: _,
        validator_registry_updates: _,
        amendment_activation_records: _,
        amendment_supersession_records: _,
        amendment_rollback_records: _,
        governance_agent_dry_run_records: _,
        vault_bridge_route_profiles: _,
        amendments: _,
    } = governance;
}

fn assert_shielded_state_commitment_inventory_complete(shielded: &ShieldedState) {
    let ShieldedState {
        next_note_position: _,
        notes: _,
        nullifiers: _,
        turnstile_events: _,
        orchard: _,
    } = shielded;
}

fn assert_bridge_state_commitment_inventory_complete(bridge: &BridgeState) {
    let BridgeState {
        domains: _,
        transfers: _,
        replay_cache: _,
    } = bridge;
}

fn append_sorted_canonical_commitments<E: std::fmt::Debug>(
    bytes: &mut Vec<u8>,
    label: &str,
    values: impl IntoIterator<Item = Result<Vec<u8>, E>>,
) -> io::Result<()> {
    let encoded = values
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, format!("{error:?}")))?;
    append_sorted_raw_commitments(bytes, label, encoded)
}

fn append_sorted_raw_commitments(
    bytes: &mut Vec<u8>,
    label: &str,
    values: impl IntoIterator<Item = Vec<u8>>,
) -> io::Result<()> {
    let mut encoded = values.into_iter().collect::<Vec<_>>();
    encoded.sort();
    append_canonical_usize(bytes, &format!("{label}_count"), encoded.len());
    for value in encoded {
        append_canonical_bytes_commitment(bytes, label, &value);
    }
    Ok(())
}

pub(super) fn append_shielded_state(bytes: &mut Vec<u8>, shielded: &ShieldedState) {
    assert_shielded_state_commitment_inventory_complete(shielded);
    append_canonical_u64(
        bytes,
        "shielded.next_note_position",
        shielded.next_note_position,
    );
    let mut notes = shielded.notes.iter().collect::<Vec<_>>();
    notes.sort_by(|left, right| {
        left.position
            .cmp(&right.position)
            .then(left.note_id.cmp(&right.note_id))
    });
    append_canonical_usize(bytes, "shielded.note_count", notes.len());
    for note in notes {
        append_shielded_note(bytes, "shielded.note", note);
    }
    let mut nullifiers = shielded.nullifiers.clone();
    nullifiers.sort();
    append_string_list(bytes, "shielded.nullifier", &nullifiers);
    let mut events = shielded.turnstile_events.iter().collect::<Vec<_>>();
    events.sort_by(|left, right| left.event_id.cmp(&right.event_id));
    append_canonical_usize(bytes, "shielded.turnstile_event_count", events.len());
    for event in events {
        append_turnstile_event(bytes, "shielded.turnstile_event", event);
    }
    append_canonical_bool(
        bytes,
        "shielded.orchard.present",
        shielded.orchard.is_some(),
    );
    if let Some(orchard) = &shielded.orchard {
        append_orchard_pool_state(bytes, "shielded.orchard", orchard);
    }
}

pub(super) fn append_bridge_state(bytes: &mut Vec<u8>, bridge: &BridgeState) {
    assert_bridge_state_commitment_inventory_complete(bridge);
    let mut domains = bridge.domains.iter().collect::<Vec<_>>();
    domains.sort_by(|left, right| left.domain_id.cmp(&right.domain_id));
    append_canonical_usize(bytes, "bridge.domain_count", domains.len());
    for domain in domains {
        append_bridge_domain(bytes, "bridge.domain", domain);
    }
    append_canonical_usize(bytes, "bridge.transfer_count", bridge.transfers.len());
    for transfer in &bridge.transfers {
        append_bridge_transfer(bytes, "bridge.transfer", transfer);
    }
    let mut replay_cache = bridge.replay_cache.clone();
    replay_cache.sort();
    append_string_list(bytes, "bridge.replay_cache", &replay_cache);
}

pub(super) fn append_account(bytes: &mut Vec<u8>, prefix: &str, account: &Account) {
    append_canonical_str(bytes, &format!("{prefix}.address"), &account.address);
    append_canonical_u64(bytes, &format!("{prefix}.balance"), account.balance);
    append_canonical_u64(bytes, &format!("{prefix}.sequence"), account.sequence);
    append_option_str(
        bytes,
        &format!("{prefix}.public_key_hex"),
        &account.public_key_hex,
    );
}

pub(super) fn append_asset_definition(bytes: &mut Vec<u8>, prefix: &str, asset: &AssetDefinition) {
    append_canonical_str(bytes, &format!("{prefix}.asset_id"), &asset.asset_id);
    append_canonical_str(bytes, &format!("{prefix}.issuer"), &asset.issuer);
    append_canonical_str(bytes, &format!("{prefix}.code"), &asset.code);
    append_canonical_u32(bytes, &format!("{prefix}.version"), asset.version);
    append_canonical_u8(bytes, &format!("{prefix}.precision"), asset.precision);
    append_canonical_str(
        bytes,
        &format!("{prefix}.display_name"),
        &asset.display_name,
    );
    append_option_u64(bytes, &format!("{prefix}.max_supply"), asset.max_supply);
    append_canonical_bool(
        bytes,
        &format!("{prefix}.requires_authorization"),
        asset.requires_authorization,
    );
    append_canonical_bool(
        bytes,
        &format!("{prefix}.freeze_enabled"),
        asset.freeze_enabled,
    );
    append_canonical_bool(
        bytes,
        &format!("{prefix}.clawback_enabled"),
        asset.clawback_enabled,
    );
}

pub(super) fn append_trustline(bytes: &mut Vec<u8>, prefix: &str, trustline: &TrustLine) {
    append_canonical_str(
        bytes,
        &format!("{prefix}.trustline_id"),
        &trustline.trustline_id,
    );
    append_canonical_str(bytes, &format!("{prefix}.account"), &trustline.account);
    append_canonical_str(bytes, &format!("{prefix}.issuer"), &trustline.issuer);
    append_canonical_str(bytes, &format!("{prefix}.asset_id"), &trustline.asset_id);
    append_canonical_u64(bytes, &format!("{prefix}.limit"), trustline.limit);
    append_canonical_u64(bytes, &format!("{prefix}.balance"), trustline.balance);
    append_canonical_bool(bytes, &format!("{prefix}.authorized"), trustline.authorized);
    append_canonical_bool(bytes, &format!("{prefix}.frozen"), trustline.frozen);
    append_canonical_u64(
        bytes,
        &format!("{prefix}.reserve_paid"),
        trustline.reserve_paid,
    );
}

pub(super) fn append_escrow(bytes: &mut Vec<u8>, prefix: &str, escrow: &Escrow) {
    append_canonical_str(bytes, &format!("{prefix}.escrow_id"), &escrow.escrow_id);
    append_canonical_str(bytes, &format!("{prefix}.owner"), &escrow.owner);
    append_canonical_u64(
        bytes,
        &format!("{prefix}.owner_sequence"),
        escrow.owner_sequence,
    );
    append_canonical_str(bytes, &format!("{prefix}.recipient"), &escrow.recipient);
    append_canonical_str(bytes, &format!("{prefix}.asset_id"), &escrow.asset_id);
    append_canonical_u64(bytes, &format!("{prefix}.amount"), escrow.amount);
    append_canonical_u64(bytes, &format!("{prefix}.fee"), escrow.fee);
    append_canonical_str(bytes, &format!("{prefix}.condition"), &escrow.condition);
    append_canonical_u64(
        bytes,
        &format!("{prefix}.finish_after"),
        escrow.finish_after,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.cancel_after"),
        escrow.cancel_after,
    );
    append_canonical_str(bytes, &format!("{prefix}.state"), &escrow.state);
    append_canonical_u64(
        bytes,
        &format!("{prefix}.created_height"),
        escrow.created_height,
    );
}

pub(super) fn append_nft(bytes: &mut Vec<u8>, prefix: &str, nft: &NftDefinition) {
    append_canonical_str(bytes, &format!("{prefix}.nft_id"), &nft.nft_id);
    append_canonical_str(bytes, &format!("{prefix}.issuer"), &nft.issuer);
    append_canonical_str(
        bytes,
        &format!("{prefix}.collection_id"),
        &nft.collection_id,
    );
    append_canonical_u64(bytes, &format!("{prefix}.serial"), nft.serial);
    append_canonical_str(bytes, &format!("{prefix}.owner"), &nft.owner);
    append_canonical_str(
        bytes,
        &format!("{prefix}.metadata_hash"),
        &nft.metadata_hash,
    );
    append_canonical_str(bytes, &format!("{prefix}.metadata_uri"), &nft.metadata_uri);
    append_canonical_u32(bytes, &format!("{prefix}.flags"), nft.flags);
    append_canonical_u32(
        bytes,
        &format!("{prefix}.collection_flags"),
        nft.collection_flags,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.issuer_transfer_fee"),
        nft.issuer_transfer_fee,
    );
    append_canonical_bool(bytes, &format!("{prefix}.burned"), nft.burned);
}

pub(super) fn append_offer(bytes: &mut Vec<u8>, prefix: &str, offer: &Offer) {
    append_canonical_str(bytes, &format!("{prefix}.offer_id"), &offer.offer_id);
    append_canonical_str(bytes, &format!("{prefix}.owner"), &offer.owner);
    append_canonical_u64(
        bytes,
        &format!("{prefix}.owner_sequence"),
        offer.owner_sequence,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.taker_gets_asset_id"),
        &offer.taker_gets_asset_id,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.taker_gets_amount_remaining"),
        offer.taker_gets_amount_remaining,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.taker_pays_asset_id"),
        &offer.taker_pays_asset_id,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.taker_pays_amount_remaining"),
        offer.taker_pays_amount_remaining,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.original_taker_gets_amount"),
        offer.original_taker_gets_amount,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.original_taker_pays_amount"),
        offer.original_taker_pays_amount,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.created_height"),
        offer.created_height,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.expiration_height"),
        offer.expiration_height,
    );
    append_canonical_u64(bytes, &format!("{prefix}.reserve_paid"), offer.reserve_paid);
    append_canonical_str(bytes, &format!("{prefix}.state"), &offer.state);
}

pub(super) fn append_nav_tracked_asset(
    bytes: &mut Vec<u8>,
    prefix: &str,
    nav_asset: &NavTrackedAsset,
) {
    append_canonical_str(bytes, &format!("{prefix}.asset_id"), &nav_asset.asset_id);
    append_canonical_str(bytes, &format!("{prefix}.issuer"), &nav_asset.issuer);
    append_canonical_str(
        bytes,
        &format!("{prefix}.reserve_operator"),
        &nav_asset.reserve_operator,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.proof_profile"),
        &nav_asset.proof_profile,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.valuation_unit"),
        &nav_asset.valuation_unit,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.redemption_account"),
        &nav_asset.redemption_account,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.finalized_epoch"),
        nav_asset.finalized_epoch,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.nav_per_unit"),
        nav_asset.nav_per_unit,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.circulating_supply"),
        nav_asset.circulating_supply,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.finalized_reserve_packet_hash"),
        &nav_asset.finalized_reserve_packet_hash,
    );
    append_canonical_bool(bytes, &format!("{prefix}.halted"), nav_asset.halted);
    append_canonical_str(
        bytes,
        &format!("{prefix}.halt_reason"),
        &nav_asset.halt_reason,
    );
}

pub(super) fn append_nav_reserve_packet(
    bytes: &mut Vec<u8>,
    prefix: &str,
    packet: &NavReservePacket,
    commit_complete_nav_state: bool,
) {
    append_canonical_str(bytes, &format!("{prefix}.packet_id"), &packet.packet_id);
    append_canonical_str(bytes, &format!("{prefix}.asset_id"), &packet.asset_id);
    append_canonical_str(bytes, &format!("{prefix}.issuer"), &packet.issuer);
    append_canonical_str(bytes, &format!("{prefix}.submitter"), &packet.submitter);
    append_canonical_u64(bytes, &format!("{prefix}.epoch"), packet.epoch);
    append_canonical_u64(
        bytes,
        &format!("{prefix}.nav_per_unit"),
        packet.nav_per_unit,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.circulating_supply"),
        packet.circulating_supply,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.verified_net_assets"),
        packet.verified_net_assets,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.proof_profile"),
        &packet.proof_profile,
    );
    append_canonical_str(bytes, &format!("{prefix}.source_root"), &packet.source_root);
    append_canonical_str(
        bytes,
        &format!("{prefix}.attestor_root"),
        &packet.attestor_root,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.reserve_packet_hash"),
        &packet.reserve_packet_hash,
    );
    append_canonical_str(bytes, &format!("{prefix}.state"), &packet.state);
    append_canonical_str(
        bytes,
        &format!("{prefix}.challenge_hash"),
        &packet.challenge_hash,
    );
    if !commit_complete_nav_state {
        return;
    }
    if packet.submitted_at_height != 0 {
        append_canonical_u64(
            bytes,
            &format!("{prefix}.submitted_at_height"),
            packet.submitted_at_height,
        );
    }
    if !packet.reserve_accounts.is_empty() {
        let mut reserve_accounts = packet.reserve_accounts.clone();
        reserve_accounts.sort();
        append_string_list(
            bytes,
            &format!("{prefix}.reserve_account"),
            &reserve_accounts,
        );
    }
    if !packet.challenger.is_empty() {
        append_canonical_str(bytes, &format!("{prefix}.challenger"), &packet.challenger);
    }
    if packet.challenge_bond != 0 {
        append_canonical_u64(
            bytes,
            &format!("{prefix}.challenge_bond"),
            packet.challenge_bond,
        );
    }
    if !packet.attestations.is_empty() {
        let mut attestations = packet.attestations.iter().collect::<Vec<_>>();
        attestations.sort_by(|left, right| {
            left.attestor
                .cmp(&right.attestor)
                .then(left.observation_root.cmp(&right.observation_root))
                .then(left.attested_at_height.cmp(&right.attested_at_height))
        });
        append_canonical_usize(
            bytes,
            &format!("{prefix}.attestation_count"),
            attestations.len(),
        );
        for attestation in attestations {
            append_nav_reserve_attestation(bytes, &format!("{prefix}.attestation"), attestation);
        }
    }
    if !packet.sp1_proof_bytes.is_empty() {
        append_canonical_bytes_commitment(
            bytes,
            &format!("{prefix}.sp1_proof_bytes"),
            &packet.sp1_proof_bytes,
        );
    }
    if !packet.sp1_public_values.is_empty() {
        append_canonical_bytes_commitment(
            bytes,
            &format!("{prefix}.sp1_public_values"),
            &packet.sp1_public_values,
        );
    }
}

pub(super) fn append_nav_reserve_attestation(
    bytes: &mut Vec<u8>,
    prefix: &str,
    attestation: &NavReserveAttestation,
) {
    append_canonical_str(bytes, &format!("{prefix}.attestor"), &attestation.attestor);
    append_canonical_bool(bytes, &format!("{prefix}.pass"), attestation.pass);
    append_canonical_str(
        bytes,
        &format!("{prefix}.observation_root"),
        &attestation.observation_root,
    );
    if attestation.attested_at_height != 0 {
        append_canonical_u64(
            bytes,
            &format!("{prefix}.attested_at_height"),
            attestation.attested_at_height,
        );
    }
}

pub(super) fn append_nav_proof_profile(
    bytes: &mut Vec<u8>,
    prefix: &str,
    profile: &NavProofProfile,
    commit_sp1_fields: bool,
) {
    append_canonical_str(bytes, &format!("{prefix}.profile_id"), &profile.profile_id);
    append_canonical_str(
        bytes,
        &format!("{prefix}.registered_by"),
        &profile.registered_by,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.verifier_kind"),
        &profile.verifier_kind,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.source_class"),
        &profile.source_class,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.max_snapshot_age_blocks"),
        profile.max_snapshot_age_blocks,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.challenge_window_blocks"),
        profile.challenge_window_blocks,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.max_epoch_gap_blocks"),
        profile.max_epoch_gap_blocks,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.settle_deadline_blocks"),
        profile.settle_deadline_blocks,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.min_challenge_bond"),
        profile.min_challenge_bond,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.min_attestations"),
        profile.min_attestations,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.tolerance_bp"),
        profile.tolerance_bp,
    );
    if profile.bridge_observer_min_confirmations != 0 {
        append_canonical_u64(
            bytes,
            &format!("{prefix}.bridge_observer_min_confirmations"),
            profile.bridge_observer_min_confirmations,
        );
    }
    append_canonical_str(
        bytes,
        &format!("{prefix}.valuation_policy_hash"),
        &profile.valuation_policy_hash,
    );
    if !commit_sp1_fields {
        return;
    }
    if !profile.vault_bridge_route_policy_hash.is_empty() {
        append_canonical_str(
            bytes,
            &format!("{prefix}.vault_bridge_route_policy_hash"),
            &profile.vault_bridge_route_policy_hash,
        );
    }
    append_canonical_str(
        bytes,
        &format!("{prefix}.sp1_program_vkey"),
        &profile.sp1_program_vkey,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.sp1_proof_encoding"),
        &profile.sp1_proof_encoding,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.max_proof_bytes"),
        profile.max_proof_bytes,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.max_public_values_bytes"),
        profile.max_public_values_bytes,
    );
}

pub(super) fn append_nav_attestor(bytes: &mut Vec<u8>, prefix: &str, attestor: &NavAttestor) {
    append_canonical_str(bytes, &format!("{prefix}.address"), &attestor.address);
    append_canonical_str(bytes, &format!("{prefix}.domain"), &attestor.domain);
    append_canonical_u64(bytes, &format!("{prefix}.bond"), attestor.bond);
    append_canonical_u64(
        bytes,
        &format!("{prefix}.registered_at_height"),
        attestor.registered_at_height,
    );
}

pub(super) fn append_market_ops_policy(
    bytes: &mut Vec<u8>,
    prefix: &str,
    policy: &MarketOpsPolicyRegistration,
) {
    append_canonical_str(
        bytes,
        &format!("{prefix}.program_id"),
        &bytes_to_hex(&policy.program_id),
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.policy_hash"),
        &bytes_to_hex(&policy.policy_hash),
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.parameter_hash"),
        &bytes_to_hex(&policy.parameter_hash),
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.venue_id"),
        &bytes_to_hex(&policy.venue_id),
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.pool_config_hash"),
        &bytes_to_hex(&policy.pool_config_hash),
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.hook_code_hash"),
        &bytes_to_hex(&policy.hook_code_hash),
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.activation_epoch"),
        policy.activation_epoch,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.deactivation_epoch"),
        policy.deactivation_epoch,
    );
}

pub(super) fn append_finalized_market_ops_envelope(
    bytes: &mut Vec<u8>,
    prefix: &str,
    record: &FinalizedMarketOpsEnvelope,
) {
    append_canonical_str(bytes, &format!("{prefix}.asset_id"), &record.asset_id);
    append_canonical_u64(bytes, &format!("{prefix}.epoch"), record.epoch);
    append_canonical_str(
        bytes,
        &format!("{prefix}.envelope_hash"),
        &record.envelope_hash,
    );
    append_market_ops_envelope(bytes, &format!("{prefix}.envelope"), &record.envelope);
    append_canonical_bool(
        bytes,
        &format!("{prefix}.policy_inputs.present"),
        record.policy_inputs.is_some(),
    );
    if let Some(policy_inputs) = &record.policy_inputs {
        append_market_ops_policy_inputs(bytes, &format!("{prefix}.policy_inputs"), policy_inputs);
    }
    append_canonical_u64(
        bytes,
        &format!("{prefix}.finalized_at_height"),
        record.finalized_at_height,
    );
}

pub(super) fn append_vault_bridge_receipt(
    bytes: &mut Vec<u8>,
    prefix: &str,
    receipt: &VaultBridgeReceipt,
) {
    append_canonical_str(bytes, &format!("{prefix}.receipt_id"), &receipt.receipt_id);
    append_canonical_str(bytes, &format!("{prefix}.asset_id"), &receipt.asset_id);
    append_canonical_str(
        bytes,
        &format!("{prefix}.source_domain"),
        &receipt.source_domain,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.source_asset"),
        &receipt.source_asset,
    );
    append_canonical_str(bytes, &format!("{prefix}.claim_type"), &receipt.claim_type);
    append_canonical_u64(
        bytes,
        &format!("{prefix}.amount_atoms"),
        receipt.amount_atoms,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.source_tx_or_attestation"),
        &receipt.source_tx_or_attestation,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.finality_ref"),
        &receipt.finality_ref,
    );
    append_canonical_str(bytes, &format!("{prefix}.vault_id"), &receipt.vault_id);
    append_canonical_str(
        bytes,
        &format!("{prefix}.policy_hash"),
        &receipt.policy_hash,
    );
    append_canonical_u64(bytes, &format!("{prefix}.haircut_bps"), receipt.haircut_bps);
    append_canonical_u64(
        bytes,
        &format!("{prefix}.counted_value_atoms"),
        receipt.counted_value_atoms,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.allocated_value_atoms"),
        receipt.allocated_value_atoms,
    );
    append_canonical_str(bytes, &format!("{prefix}.bucket_id"), &receipt.bucket_id);
    append_canonical_str(bytes, &format!("{prefix}.status"), &receipt.status);
    append_canonical_u64(
        bytes,
        &format!("{prefix}.created_at_height"),
        receipt.created_at_height,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.finalized_at_height"),
        receipt.finalized_at_height,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.counted_at_height"),
        receipt.counted_at_height,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.expires_at_height"),
        receipt.expires_at_height,
    );
    append_canonical_bool(
        bytes,
        &format!("{prefix}.bridge_deposit_evidence.present"),
        receipt.bridge_deposit_evidence.is_some(),
    );
    if let Some(evidence) = &receipt.bridge_deposit_evidence {
        append_vault_bridge_deposit_evidence(
            bytes,
            &format!("{prefix}.bridge_deposit_evidence"),
            evidence,
        );
    }
}

pub(super) fn append_vault_bridge_deposit_evidence(
    bytes: &mut Vec<u8>,
    prefix: &str,
    evidence: &VaultBridgeDepositEvidence,
) {
    append_canonical_u64(
        bytes,
        &format!("{prefix}.source_chain_id"),
        evidence.source_chain_id,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.vault_address"),
        &evidence.vault_address,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.token_address"),
        &evidence.token_address,
    );
    append_canonical_str(bytes, &format!("{prefix}.depositor"), &evidence.depositor);
    append_canonical_str(
        bytes,
        &format!("{prefix}.pftl_recipient"),
        &evidence.pftl_recipient,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.pftl_recipient_hash"),
        &evidence.pftl_recipient_hash,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.amount_atoms"),
        evidence.amount_atoms,
    );
    append_canonical_str(bytes, &format!("{prefix}.nonce"), &evidence.nonce);
    append_canonical_str(bytes, &format!("{prefix}.deposit_id"), &evidence.deposit_id);
    append_canonical_str(bytes, &format!("{prefix}.block_hash"), &evidence.block_hash);
    append_canonical_str(bytes, &format!("{prefix}.tx_hash"), &evidence.tx_hash);
    append_canonical_u64(bytes, &format!("{prefix}.log_index"), evidence.log_index);
}

pub(super) fn append_vault_bridge_deposit_record(
    bytes: &mut Vec<u8>,
    prefix: &str,
    record: &VaultBridgeDepositRecord,
    legacy_vault_bridge_deposit_attestation_fields: bool,
) {
    append_canonical_str(bytes, &format!("{prefix}.asset_id"), &record.asset_id);
    append_canonical_str(
        bytes,
        &format!("{prefix}.evidence_root"),
        &record.evidence_root,
    );
    append_vault_bridge_deposit_evidence(bytes, &format!("{prefix}.evidence"), &record.evidence);
    append_canonical_str(bytes, &format!("{prefix}.policy_hash"), &record.policy_hash);
    append_canonical_str(
        bytes,
        &format!("{prefix}.source_proof_kind"),
        &record.source_proof_kind,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.source_proof_hash"),
        &record.source_proof_hash,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.source_public_values_hash"),
        &record.source_public_values_hash,
    );
    append_canonical_str(bytes, &format!("{prefix}.proposer"), &record.proposer);
    append_canonical_str(bytes, &format!("{prefix}.status"), &record.status);
    append_canonical_u64(
        bytes,
        &format!("{prefix}.submitted_at_height"),
        record.submitted_at_height,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.finalized_at_height"),
        record.finalized_at_height,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.expires_at_height"),
        record.expires_at_height,
    );
    append_canonical_str(bytes, &format!("{prefix}.challenger"), &record.challenger);
    append_canonical_str(
        bytes,
        &format!("{prefix}.challenge_hash"),
        &record.challenge_hash,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.challenge_bond"),
        record.challenge_bond,
    );
    if !record.attestations.is_empty() {
        let mut attestations = record.attestations.iter().collect::<Vec<_>>();
        attestations.sort_by(|left, right| {
            left.attestor
                .cmp(&right.attestor)
                .then(left.observation_root.cmp(&right.observation_root))
        });
        append_canonical_usize(
            bytes,
            &format!("{prefix}.attestation_count"),
            attestations.len(),
        );
        for attestation in attestations {
            append_vault_bridge_deposit_attestation(
                bytes,
                &format!("{prefix}.attestation"),
                attestation,
                legacy_vault_bridge_deposit_attestation_fields,
            );
        }
    }
}

pub(super) fn append_vault_bridge_deposit_attestation(
    bytes: &mut Vec<u8>,
    prefix: &str,
    attestation: &VaultBridgeDepositAttestation,
    legacy_vault_bridge_deposit_attestation_fields: bool,
) {
    append_canonical_str(bytes, &format!("{prefix}.attestor"), &attestation.attestor);
    append_canonical_bool(bytes, &format!("{prefix}.pass"), attestation.pass);
    append_canonical_str(
        bytes,
        &format!("{prefix}.observation_root"),
        &attestation.observation_root,
    );
    if !legacy_vault_bridge_deposit_attestation_fields {
        append_canonical_bool(
            bytes,
            &format!("{prefix}.observation.present"),
            attestation.observation.is_some(),
        );
        if let Some(observation) = &attestation.observation {
            append_vault_bridge_deposit_observation(
                bytes,
                &format!("{prefix}.observation"),
                observation,
            );
        }
    }
    append_canonical_u64(
        bytes,
        &format!("{prefix}.attested_at_height"),
        attestation.attested_at_height,
    );
}

pub(super) fn append_vault_bridge_deposit_observation(
    bytes: &mut Vec<u8>,
    prefix: &str,
    observation: &VaultBridgeDepositObservation,
) {
    append_canonical_bool(bytes, &format!("{prefix}.tx_exists"), observation.tx_exists);
    append_canonical_u64(
        bytes,
        &format!("{prefix}.receipt_status"),
        observation.receipt_status,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.source_chain_id"),
        observation.source_chain_id,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.vault_address"),
        &observation.vault_address,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.token_address"),
        &observation.token_address,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.depositor"),
        &observation.depositor,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.amount_atoms"),
        observation.amount_atoms,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.deposit_id"),
        &observation.deposit_id,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.block_hash"),
        &observation.block_hash,
    );
    append_canonical_str(bytes, &format!("{prefix}.tx_hash"), &observation.tx_hash);
    append_canonical_u64(bytes, &format!("{prefix}.log_index"), observation.log_index);
    append_canonical_u64(
        bytes,
        &format!("{prefix}.confirmation_depth"),
        observation.confirmation_depth,
    );
}

pub(super) fn append_vault_bridge_bucket(
    bytes: &mut Vec<u8>,
    prefix: &str,
    bucket: &VaultBridgeBucketState,
) {
    append_canonical_str(bytes, &format!("{prefix}.asset_id"), &bucket.asset_id);
    append_canonical_str(bytes, &format!("{prefix}.bucket_id"), &bucket.bucket_id);
    append_canonical_str(
        bytes,
        &format!("{prefix}.source_domain"),
        &bucket.source_domain,
    );
    append_canonical_str(bytes, &format!("{prefix}.policy_hash"), &bucket.policy_hash);
    append_canonical_u64(
        bytes,
        &format!("{prefix}.gross_receipt_atoms"),
        bucket.gross_receipt_atoms,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.counted_value_atoms"),
        bucket.counted_value_atoms,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.outstanding_vault_bridge_atoms"),
        bucket.outstanding_vault_bridge_atoms,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.nav_subscription_allocations_atoms"),
        bucket.nav_subscription_allocations_atoms,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.redemption_queue_atoms"),
        bucket.redemption_queue_atoms,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.other_allocations_atoms"),
        bucket.other_allocations_atoms,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.impairment_factor_bps"),
        bucket.impairment_factor_bps,
    );
    append_canonical_str(bytes, &format!("{prefix}.status"), &bucket.status);
    append_canonical_u64(
        bytes,
        &format!("{prefix}.last_packet_epoch"),
        bucket.last_packet_epoch,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.last_updated_height"),
        bucket.last_updated_height,
    );
}

pub(super) fn append_vault_bridge_allocation(
    bytes: &mut Vec<u8>,
    prefix: &str,
    allocation: &VaultBridgeAllocation,
) {
    append_canonical_str(
        bytes,
        &format!("{prefix}.allocation_id"),
        &allocation.allocation_id,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.receipt_id"),
        &allocation.receipt_id,
    );
    append_canonical_str(bytes, &format!("{prefix}.asset_id"), &allocation.asset_id);
    append_canonical_str(bytes, &format!("{prefix}.bucket_id"), &allocation.bucket_id);
    append_canonical_u64(
        bytes,
        &format!("{prefix}.amount_atoms"),
        allocation.amount_atoms,
    );
    append_canonical_str(bytes, &format!("{prefix}.purpose"), &allocation.purpose);
    append_canonical_str(
        bytes,
        &format!("{prefix}.consumer_id"),
        &allocation.consumer_id,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.created_at_height"),
        allocation.created_at_height,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.retired_at_height"),
        allocation.retired_at_height,
    );
}

pub(super) fn append_vault_bridge_redemption(
    bytes: &mut Vec<u8>,
    prefix: &str,
    redemption: &VaultBridgeRedemption,
    legacy_domainless_withdrawal_packet: bool,
) {
    append_canonical_str(
        bytes,
        &format!("{prefix}.redemption_id"),
        &redemption.redemption_id,
    );
    append_canonical_str(bytes, &format!("{prefix}.owner"), &redemption.owner);
    append_canonical_u64(
        bytes,
        &format!("{prefix}.owner_sequence"),
        redemption.owner_sequence,
    );
    append_canonical_str(bytes, &format!("{prefix}.issuer"), &redemption.issuer);
    append_canonical_str(bytes, &format!("{prefix}.asset_id"), &redemption.asset_id);
    append_canonical_str(bytes, &format!("{prefix}.bucket_id"), &redemption.bucket_id);
    append_canonical_u64(
        bytes,
        &format!("{prefix}.amount_atoms"),
        redemption.amount_atoms,
    );
    append_canonical_u64(bytes, &format!("{prefix}.epoch"), redemption.epoch);
    append_canonical_str(
        bytes,
        &format!("{prefix}.reserve_packet_hash"),
        &redemption.reserve_packet_hash,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.destination_ref"),
        &redemption.destination_ref,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.settled_atoms"),
        redemption.settled_atoms,
    );
    append_canonical_str(bytes, &format!("{prefix}.state"), &redemption.state);
    append_canonical_u64(
        bytes,
        &format!("{prefix}.created_at_height"),
        redemption.created_at_height,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.settlement_receipt_hash"),
        &redemption.settlement_receipt_hash,
    );
    append_vault_bridge_withdrawal_packet(
        bytes,
        &format!("{prefix}.withdrawal_packet"),
        &redemption.withdrawal_packet,
        legacy_domainless_withdrawal_packet,
    );
    let legacy_packet_hash = if legacy_domainless_withdrawal_packet {
        Some(legacy_domainless_vault_bridge_withdrawal_packet_hash(
            &redemption.withdrawal_packet,
        ))
    } else {
        None
    };
    let legacy_evm_digest = if legacy_domainless_withdrawal_packet {
        legacy_domainless_vault_bridge_withdrawal_packet_evm_digest(&redemption.withdrawal_packet)
    } else {
        None
    };
    append_canonical_str(
        bytes,
        &format!("{prefix}.withdrawal_packet_hash"),
        legacy_packet_hash
            .as_deref()
            .unwrap_or(&redemption.withdrawal_packet_hash),
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.withdrawal_packet_evm_digest"),
        legacy_evm_digest
            .as_deref()
            .unwrap_or(&redemption.withdrawal_packet_evm_digest),
    );
    if !redemption.withdrawal_observations.is_empty() {
        let mut observations = redemption
            .withdrawal_observations
            .iter()
            .collect::<Vec<_>>();
        observations.sort_by(|left, right| {
            left.attestor
                .cmp(&right.attestor)
                .then(left.observation_root.cmp(&right.observation_root))
        });
        append_canonical_usize(
            bytes,
            &format!("{prefix}.withdrawal_observation_count"),
            observations.len(),
        );
        for observation in observations {
            append_vault_bridge_withdrawal_execution_attestation(
                bytes,
                &format!("{prefix}.withdrawal_observation"),
                observation,
            );
        }
    }
}

pub(super) fn append_vault_bridge_withdrawal_execution_attestation(
    bytes: &mut Vec<u8>,
    prefix: &str,
    attestation: &VaultBridgeWithdrawalExecutionAttestation,
) {
    append_canonical_str(bytes, &format!("{prefix}.attestor"), &attestation.attestor);
    append_canonical_str(
        bytes,
        &format!("{prefix}.observation_root"),
        &attestation.observation_root,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.signature_hex"),
        &attestation.signature_hex,
    );
    append_vault_bridge_withdrawal_execution_observation(
        bytes,
        &format!("{prefix}.observation"),
        &attestation.observation,
    );
}

pub(super) fn append_vault_bridge_withdrawal_execution_observation(
    bytes: &mut Vec<u8>,
    prefix: &str,
    observation: &VaultBridgeWithdrawalExecutionObservation,
) {
    append_canonical_bool(bytes, &format!("{prefix}.tx_exists"), observation.tx_exists);
    append_canonical_u64(
        bytes,
        &format!("{prefix}.receipt_status"),
        observation.receipt_status,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.source_chain_id"),
        observation.source_chain_id,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.vault_address"),
        &observation.vault_address,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.token_address"),
        &observation.token_address,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.recipient"),
        &observation.recipient,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.amount_atoms"),
        observation.amount_atoms,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.withdrawal_id"),
        &observation.withdrawal_id,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.withdrawal_packet_hash"),
        &observation.withdrawal_packet_hash,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.block_hash"),
        &observation.block_hash,
    );
    append_canonical_str(bytes, &format!("{prefix}.tx_hash"), &observation.tx_hash);
    append_canonical_u64(bytes, &format!("{prefix}.log_index"), observation.log_index);
    append_canonical_u64(
        bytes,
        &format!("{prefix}.confirmation_depth"),
        observation.confirmation_depth,
    );
}

pub(super) fn append_vault_bridge_withdrawal_packet(
    bytes: &mut Vec<u8>,
    prefix: &str,
    packet: &VaultBridgeWithdrawalPacket,
    legacy_domainless: bool,
) {
    append_canonical_u64(
        bytes,
        &format!("{prefix}.pftl_chain_id"),
        packet.pftl_chain_id,
    );
    if !legacy_domainless {
        append_canonical_u64(
            bytes,
            &format!("{prefix}.source_chain_id"),
            packet.source_chain_id,
        );
        append_canonical_str(
            bytes,
            &format!("{prefix}.vault_address"),
            &packet.vault_address,
        );
        append_canonical_str(
            bytes,
            &format!("{prefix}.token_address"),
            &packet.token_address,
        );
    }
    append_canonical_str(
        bytes,
        &format!("{prefix}.vault_bridge_asset_id"),
        &packet.vault_bridge_asset_id,
    );
    append_canonical_str(bytes, &format!("{prefix}.burn_tx_id"), &packet.burn_tx_id);
    append_canonical_str(
        bytes,
        &format!("{prefix}.withdrawal_id"),
        &packet.withdrawal_id,
    );
    append_canonical_str(bytes, &format!("{prefix}.recipient"), &packet.recipient);
    append_canonical_u64(
        bytes,
        &format!("{prefix}.amount_atoms"),
        packet.amount_atoms,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.source_bucket_id"),
        &packet.source_bucket_id,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.destination_hash"),
        &packet.destination_hash,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.finalized_height"),
        packet.finalized_height,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.evidence_root"),
        &packet.evidence_root,
    );
}

pub(super) fn append_pftl_uniswap_route(
    bytes: &mut Vec<u8>,
    prefix: &str,
    route: &PftlUniswapConsensusRouteState,
) {
    append_canonical_str(bytes, &format!("{prefix}.route_id"), &route.route_id);
    append_canonical_str(
        bytes,
        &format!("{prefix}.route_family"),
        &route.route_family,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.route_config_digest"),
        &route.route_config_digest,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.route_trust_class"),
        &route.route_trust_class,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.native_nav_asset_id"),
        &route.native_nav_asset_id,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.settlement_asset_id"),
        &route.settlement_asset_id,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.handoff_controller"),
        &route.handoff_controller,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.settlement_adapter"),
        &route.settlement_adapter,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.wrapped_navcoin_token"),
        &route.wrapped_navcoin_token,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.ethereum_chain_id"),
        route.ethereum_chain_id,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.route_supply_cap_atoms"),
        route.route_supply_cap_atoms,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.packet_notional_cap_atoms"),
        route.packet_notional_cap_atoms,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.latest_finalized_nav_epoch"),
        route.latest_finalized_nav_epoch,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.return_finality_blocks"),
        route.return_finality_blocks,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.authorized_valid_supply_atoms"),
        route.authorized_valid_supply_atoms,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.pftl_spendable_supply_atoms"),
        route.pftl_spendable_supply_atoms,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.ethereum_spendable_supply_atoms"),
        route.ethereum_spendable_supply_atoms,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.other_registered_venue_supply_atoms"),
        route.other_registered_venue_supply_atoms,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.outstanding_bridge_claims_atoms"),
        route.outstanding_bridge_claims_atoms,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.pending_return_import_claims_atoms"),
        route.pending_return_import_claims_atoms,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.settlement_reserve_atoms"),
        route.settlement_reserve_atoms,
    );
    append_canonical_bool(bytes, &format!("{prefix}.paused"), route.paused);

    append_canonical_usize(
        bytes,
        &format!("{prefix}.native_balance_count"),
        route.native_spendable_balances_atoms.len(),
    );
    for (wallet, amount) in &route.native_spendable_balances_atoms {
        append_canonical_str(bytes, &format!("{prefix}.native_balance.wallet"), wallet);
        append_canonical_u64(
            bytes,
            &format!("{prefix}.native_balance.amount_atoms"),
            *amount,
        );
    }
    append_canonical_usize(
        bytes,
        &format!("{prefix}.primary_subscription_nonce_count"),
        route.primary_subscription_nonces.len(),
    );
    for (nonce, wallet) in &route.primary_subscription_nonces {
        append_canonical_str(
            bytes,
            &format!("{prefix}.primary_subscription_nonce.nonce"),
            nonce,
        );
        append_canonical_str(
            bytes,
            &format!("{prefix}.primary_subscription_nonce.wallet"),
            wallet,
        );
    }
    append_canonical_usize(
        bytes,
        &format!("{prefix}.export_packet_count"),
        route.export_packets.len(),
    );
    for packet in route.export_packets.values() {
        append_pftl_uniswap_export_packet(bytes, &format!("{prefix}.export_packet"), packet);
    }
    append_canonical_usize(
        bytes,
        &format!("{prefix}.export_nonce_count"),
        route.export_nonces.len(),
    );
    for (nonce, packet_hash) in &route.export_nonces {
        append_canonical_str(bytes, &format!("{prefix}.export_nonce.nonce"), nonce);
        append_canonical_str(
            bytes,
            &format!("{prefix}.export_nonce.packet_hash"),
            packet_hash,
        );
    }
    append_canonical_usize(
        bytes,
        &format!("{prefix}.return_import_count"),
        route.return_imports.len(),
    );
    for import in route.return_imports.values() {
        append_pftl_uniswap_return_import(bytes, &format!("{prefix}.return_import"), import);
    }
}

pub(super) fn append_pftl_uniswap_export_packet(
    bytes: &mut Vec<u8>,
    prefix: &str,
    packet: &PftlUniswapConsensusExportPacket,
) {
    append_canonical_str(bytes, &format!("{prefix}.packet_hash"), &packet.packet_hash);
    append_canonical_str(bytes, &format!("{prefix}.nonce"), &packet.nonce);
    append_canonical_str(
        bytes,
        &format!("{prefix}.source_wallet"),
        &packet.source_wallet,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.ethereum_recipient"),
        &packet.ethereum_recipient,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.amount_atoms"),
        packet.amount_atoms,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.source_height"),
        packet.source_height,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.destination_deadline_seconds"),
        packet.destination_deadline_seconds,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.refund_not_before_height"),
        packet.refund_not_before_height,
    );
    append_canonical_str(bytes, &format!("{prefix}.status"), &packet.status);
}

pub(super) fn append_pftl_uniswap_return_import(
    bytes: &mut Vec<u8>,
    prefix: &str,
    import: &PftlUniswapConsensusReturnImport,
) {
    append_canonical_str(
        bytes,
        &format!("{prefix}.burn_event_hash"),
        &import.burn_event_hash,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.ethereum_chain_id"),
        import.ethereum_chain_id,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.bridge_controller"),
        &import.bridge_controller,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.wrapped_navcoin_token"),
        &import.wrapped_navcoin_token,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.native_nav_asset_id"),
        &import.native_nav_asset_id,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.ethereum_sender"),
        &import.ethereum_sender,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.pftl_recipient"),
        &import.pftl_recipient,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.amount_atoms"),
        import.amount_atoms,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.return_nonce"),
        &import.return_nonce,
    );
    append_canonical_u64(bytes, &format!("{prefix}.burn_height"), import.burn_height);
    append_canonical_u64(
        bytes,
        &format!("{prefix}.finalized_height"),
        import.finalized_height,
    );
    append_canonical_str(bytes, &format!("{prefix}.status"), &import.status);
}

pub(super) fn append_pftl_uniswap_receipt(
    bytes: &mut Vec<u8>,
    prefix: &str,
    receipt: &PftlUniswapConsensusReceipt,
) {
    append_canonical_str(
        bytes,
        &format!("{prefix}.receipt_hash"),
        &receipt.receipt_hash,
    );
    append_canonical_str(bytes, &format!("{prefix}.transition"), &receipt.transition);
    append_canonical_str(bytes, &format!("{prefix}.route_id"), &receipt.route_id);
    append_canonical_str(
        bytes,
        &format!("{prefix}.state_before_hash"),
        &receipt.state_before_hash,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.state_after_hash"),
        &receipt.state_after_hash,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.packet_hash"),
        receipt.packet_hash.as_deref().unwrap_or(""),
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.burn_event_hash"),
        receipt.burn_event_hash.as_deref().unwrap_or(""),
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.wallet"),
        receipt.wallet.as_deref().unwrap_or(""),
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.amount_atoms"),
        receipt.amount_atoms.unwrap_or(0),
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.block_height"),
        receipt.block_height,
    );
}

pub(super) fn legacy_domainless_vault_bridge_withdrawal_packet_hash(
    packet: &VaultBridgeWithdrawalPacket,
) -> String {
    let preimage = format!(
        "pftl_chain_id={}\nvault_bridge_asset_id={}\nburn_tx_id={}\nwithdrawal_id={}\nrecipient={}\namount_atoms={}\nsource_bucket_id={}\ndestination_hash={}\nfinalized_height={}\nevidence_root={}\n",
        packet.pftl_chain_id,
        packet.vault_bridge_asset_id,
        packet.burn_tx_id,
        packet.withdrawal_id,
        packet.recipient,
        packet.amount_atoms,
        packet.source_bucket_id,
        packet.destination_hash,
        packet.finalized_height,
        packet.evidence_root,
    );
    hash_hex(
        "postfiat.vault_bridge.withdrawal_packet_hash.v1",
        preimage.as_bytes(),
    )
}

pub(super) fn legacy_domainless_vault_bridge_withdrawal_packet_evm_digest(
    packet: &VaultBridgeWithdrawalPacket,
) -> Option<String> {
    let asset_id = vault_bridge_hex_bytes_exact(
        "vault_bridge_withdrawal_packet.vault_bridge_asset_id",
        &packet.vault_bridge_asset_id,
        48,
    )
    .ok()?;
    let burn_tx_id = vault_bridge_hex_bytes_exact(
        "vault_bridge_withdrawal_packet.burn_tx_id",
        &packet.burn_tx_id,
        48,
    )
    .ok()?;
    let withdrawal_id = vault_bridge_hex_bytes_exact(
        "vault_bridge_withdrawal_packet.withdrawal_id",
        &packet.withdrawal_id,
        48,
    )
    .ok()?;
    let recipient = vault_bridge_evm_address_bytes(
        "vault_bridge_withdrawal_packet.recipient",
        &packet.recipient,
    )
    .ok()?;
    let source_bucket_id = vault_bridge_hex_bytes_exact(
        "vault_bridge_withdrawal_packet.source_bucket_id",
        &packet.source_bucket_id,
        48,
    )
    .ok()?;
    let destination_hash = vault_bridge_hex_bytes_exact(
        "vault_bridge_withdrawal_packet.destination_hash",
        &packet.destination_hash,
        48,
    )
    .ok()?;
    let evidence_root = vault_bridge_hex_bytes_exact(
        "vault_bridge_withdrawal_packet.evidence_root",
        &packet.evidence_root,
        48,
    )
    .ok()?;
    let head_words = 11usize;
    let head_len = head_words.checked_mul(EVM_ABI_WORD_BYTES)?;
    let mut head = Vec::with_capacity(head_len);
    let mut tail = Vec::new();
    vault_bridge_append_abi_dynamic_bytes(
        &mut head,
        &mut tail,
        head_len,
        "postfiat.erc20_bridge.withdrawal_packet.v1".as_bytes(),
    )
    .ok()?;
    vault_bridge_append_abi_u256_u64(&mut head, packet.pftl_chain_id);
    vault_bridge_append_abi_dynamic_bytes(&mut head, &mut tail, head_len, &asset_id).ok()?;
    vault_bridge_append_abi_dynamic_bytes(&mut head, &mut tail, head_len, &burn_tx_id).ok()?;
    vault_bridge_append_abi_dynamic_bytes(&mut head, &mut tail, head_len, &withdrawal_id).ok()?;
    vault_bridge_append_abi_address(&mut head, &recipient);
    vault_bridge_append_abi_u256_u64(&mut head, packet.amount_atoms);
    vault_bridge_append_abi_dynamic_bytes(&mut head, &mut tail, head_len, &source_bucket_id)
        .ok()?;
    vault_bridge_append_abi_dynamic_bytes(&mut head, &mut tail, head_len, &destination_hash)
        .ok()?;
    vault_bridge_append_abi_u256_u64(&mut head, packet.finalized_height);
    vault_bridge_append_abi_dynamic_bytes(&mut head, &mut tail, head_len, &evidence_root).ok()?;
    let mut abi = head;
    abi.extend_from_slice(&tail);
    Some(bytes_to_hex(&vault_bridge_keccak256(&abi)))
}

pub(super) fn append_market_ops_envelope(
    bytes: &mut Vec<u8>,
    prefix: &str,
    envelope: &MarketOpsEnvelope,
) {
    append_canonical_u32(
        bytes,
        &format!("{prefix}.encoding_version"),
        envelope.encoding_version,
    );
    append_canonical_u64(bytes, &format!("{prefix}.chain_id"), envelope.chain_id);
    append_canonical_str(
        bytes,
        &format!("{prefix}.adapter_address"),
        &bytes_to_hex(&envelope.adapter_address),
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.vault_address"),
        &bytes_to_hex(&envelope.vault_address),
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.mint_controller_address"),
        &bytes_to_hex(&envelope.mint_controller_address),
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.asset_id"),
        &bytes_to_hex(&envelope.asset_id),
    );
    append_canonical_u64(bytes, &format!("{prefix}.epoch"), envelope.epoch);
    append_canonical_str(
        bytes,
        &format!("{prefix}.program_id"),
        &bytes_to_hex(&envelope.program_id),
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.policy_hash"),
        &bytes_to_hex(&envelope.policy_hash),
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.parameter_hash"),
        &bytes_to_hex(&envelope.parameter_hash),
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.reserve_packet_hash"),
        &bytes_to_hex(&envelope.reserve_packet_hash),
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.supply_packet_hash"),
        &bytes_to_hex(&envelope.supply_packet_hash),
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.evidence_root"),
        &bytes_to_hex(&envelope.evidence_root),
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.previous_market_state_hash"),
        &bytes_to_hex(&envelope.previous_market_state_hash),
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.venue_id"),
        &bytes_to_hex(&envelope.venue_id),
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.pool_config_hash"),
        &bytes_to_hex(&envelope.pool_config_hash),
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.hook_code_hash"),
        &bytes_to_hex(&envelope.hook_code_hash),
    );
    append_canonical_u128(
        bytes,
        &format!("{prefix}.nav_floor_usd_e8"),
        envelope.nav_floor_usd_e8,
    );
    append_canonical_u128(
        bytes,
        &format!("{prefix}.valid_global_supply_atoms"),
        envelope.valid_global_supply_atoms,
    );
    append_canonical_u128(
        bytes,
        &format!("{prefix}.verified_net_assets_usd_e8"),
        envelope.verified_net_assets_usd_e8,
    );
    append_canonical_u128(
        bytes,
        &format!("{prefix}.funded_alignment_reserve_usd_e8"),
        envelope.funded_alignment_reserve_usd_e8,
    );
    append_canonical_u128(
        bytes,
        &format!("{prefix}.required_alignment_reserve_usd_e8"),
        envelope.required_alignment_reserve_usd_e8,
    );
    append_canonical_u128(
        bytes,
        &format!("{prefix}.max_reserve_deploy_usd_e8"),
        envelope.max_reserve_deploy_usd_e8,
    );
    append_canonical_u128(
        bytes,
        &format!("{prefix}.max_mint_atoms"),
        envelope.max_mint_atoms,
    );
    append_canonical_u32(
        bytes,
        &format!("{prefix}.discount_trigger_bps"),
        envelope.discount_trigger_bps,
    );
    append_canonical_u32(
        bytes,
        &format!("{prefix}.premium_trigger_bps"),
        envelope.premium_trigger_bps,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.data_window_start"),
        envelope.data_window_start,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.data_window_end"),
        envelope.data_window_end,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.valid_after"),
        envelope.valid_after,
    );
    append_canonical_u64(bytes, &format!("{prefix}.expires_at"), envelope.expires_at);
    append_canonical_u64(
        bytes,
        &format!("{prefix}.cooldown_seconds"),
        envelope.cooldown_seconds,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.nonce"),
        &bytes_to_hex(&envelope.nonce),
    );
}

pub(super) fn append_market_ops_policy_inputs(
    bytes: &mut Vec<u8>,
    prefix: &str,
    inputs: &MarketOpsPolicyInputs,
) {
    append_canonical_u128(bytes, &format!("{prefix}.unit_scale"), inputs.unit_scale);
    append_canonical_u32(
        bytes,
        &format!("{prefix}.floor_factor_bps"),
        inputs.floor_factor_bps,
    );
    append_canonical_u128(
        bytes,
        &format!("{prefix}.alignment.policy_min_usd_e8"),
        inputs.alignment_params.policy_min_usd_e8,
    );
    append_canonical_u32(
        bytes,
        &format!("{prefix}.alignment.min_alignment_bps"),
        inputs.alignment_params.min_alignment_bps,
    );
    append_canonical_u128(
        bytes,
        &format!("{prefix}.alignment.stress_repeat_factor_14d"),
        inputs.alignment_params.stress_repeat_factor_14d,
    );
    append_canonical_u128(
        bytes,
        &format!("{prefix}.alignment.stress_repeat_factor_90d"),
        inputs.alignment_params.stress_repeat_factor_90d,
    );
    append_canonical_u128(
        bytes,
        &format!("{prefix}.alignment.stale_epochs_allowed"),
        inputs.alignment_params.stale_epochs_allowed,
    );
    append_canonical_u32(
        bytes,
        &format!("{prefix}.alignment.max_decay_per_epoch_bps"),
        inputs.alignment_params.max_decay_per_epoch_bps,
    );
    append_canonical_u128(
        bytes,
        &format!("{prefix}.previous_required_alignment_reserve_usd_e8"),
        inputs.previous_required_alignment_reserve_usd_e8,
    );
    append_u128_list(
        bytes,
        &format!("{prefix}.cost_to_restore_14d_usd_e8"),
        &inputs.cost_to_restore_14d_usd_e8,
    );
    append_u128_list(
        bytes,
        &format!("{prefix}.cost_to_restore_90d_usd_e8"),
        &inputs.cost_to_restore_90d_usd_e8,
    );
    append_canonical_u128(
        bytes,
        &format!("{prefix}.reserve_limits.available_alignment_reserve_usd_e8"),
        inputs.reserve_limits.available_alignment_reserve_usd_e8,
    );
    append_canonical_u128(
        bytes,
        &format!("{prefix}.reserve_limits.venue_policy_cap_usd_e8"),
        inputs.reserve_limits.venue_policy_cap_usd_e8,
    );
    append_canonical_u128(
        bytes,
        &format!("{prefix}.reserve_limits.depth_limited_cap_usd_e8"),
        inputs.reserve_limits.depth_limited_cap_usd_e8,
    );
    append_canonical_u128(
        bytes,
        &format!("{prefix}.reserve_limits.cooldown_limited_cap_usd_e8"),
        inputs.reserve_limits.cooldown_limited_cap_usd_e8,
    );
    append_canonical_u128(
        bytes,
        &format!("{prefix}.mint_limits.policy_max_mint_atoms"),
        inputs.mint_limits.policy_max_mint_atoms,
    );
    append_canonical_u128(
        bytes,
        &format!("{prefix}.mint_limits.venue_bid_depth_atoms"),
        inputs.mint_limits.venue_bid_depth_atoms,
    );
    append_canonical_u128(
        bytes,
        &format!("{prefix}.mint_limits.cooldown_mint_atoms"),
        inputs.mint_limits.cooldown_mint_atoms,
    );
    append_market_ops_observations(
        bytes,
        &format!("{prefix}.discount_observation"),
        &inputs.discount_observations,
    );
    append_market_ops_observations(
        bytes,
        &format!("{prefix}.premium_observation"),
        &inputs.premium_observations,
    );
}

pub(super) fn append_u128_list(bytes: &mut Vec<u8>, prefix: &str, values: &[u128]) {
    append_canonical_usize(bytes, &format!("{prefix}.count"), values.len());
    for (index, value) in values.iter().enumerate() {
        append_canonical_u128(bytes, &format!("{prefix}.{index}"), *value);
    }
}

pub(super) fn append_market_ops_observations(
    bytes: &mut Vec<u8>,
    prefix: &str,
    observations: &[MarketOpsVenueObservation],
) {
    append_canonical_usize(bytes, &format!("{prefix}.count"), observations.len());
    for (index, observation) in observations.iter().enumerate() {
        append_canonical_u64(
            bytes,
            &format!("{prefix}.{index}.dt_seconds"),
            observation.dt_seconds,
        );
        append_canonical_u128(
            bytes,
            &format!("{prefix}.{index}.price_usd_e8"),
            observation.price_usd_e8,
        );
        append_canonical_u128(
            bytes,
            &format!("{prefix}.{index}.volume_usd_e8"),
            observation.volume_usd_e8,
        );
    }
}

pub(super) fn append_owned_object(bytes: &mut Vec<u8>, prefix: &str, object: &OwnedObject) {
    append_canonical_str(bytes, &format!("{prefix}.id"), &object.id);
    append_canonical_u64(bytes, &format!("{prefix}.version"), object.version);
    append_canonical_str(
        bytes,
        &format!("{prefix}.owner_pubkey_hex"),
        &object.owner_pubkey_hex,
    );
    append_canonical_u64(bytes, &format!("{prefix}.value"), object.value);
    append_canonical_str(bytes, &format!("{prefix}.asset"), &object.asset);
}

pub(super) fn append_nav_redemption(bytes: &mut Vec<u8>, prefix: &str, redemption: &NavRedemption) {
    append_canonical_str(
        bytes,
        &format!("{prefix}.redemption_id"),
        &redemption.redemption_id,
    );
    append_canonical_str(bytes, &format!("{prefix}.owner"), &redemption.owner);
    append_canonical_u64(
        bytes,
        &format!("{prefix}.owner_sequence"),
        redemption.owner_sequence,
    );
    append_canonical_str(bytes, &format!("{prefix}.issuer"), &redemption.issuer);
    append_canonical_str(bytes, &format!("{prefix}.asset_id"), &redemption.asset_id);
    append_canonical_u64(bytes, &format!("{prefix}.amount"), redemption.amount);
    append_canonical_u64(bytes, &format!("{prefix}.epoch"), redemption.epoch);
    append_canonical_u64(
        bytes,
        &format!("{prefix}.nav_per_unit"),
        redemption.nav_per_unit,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.reserve_packet_hash"),
        &redemption.reserve_packet_hash,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.redemption_claim"),
        redemption.redemption_claim,
    );
    append_canonical_str(bytes, &format!("{prefix}.state"), &redemption.state);
}

pub(super) fn append_governance_amendment(
    bytes: &mut Vec<u8>,
    prefix: &str,
    amendment: &GovernanceAmendment,
) {
    append_canonical_str(
        bytes,
        &format!("{prefix}.amendment_id"),
        &amendment.amendment_id,
    );
    append_canonical_str(bytes, &format!("{prefix}.chain_id"), &amendment.chain_id);
    append_canonical_str(
        bytes,
        &format!("{prefix}.genesis_hash"),
        &amendment.genesis_hash,
    );
    append_canonical_u32(
        bytes,
        &format!("{prefix}.protocol_version"),
        amendment.protocol_version,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.instance_id"),
        &amendment.instance_id,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.proposal_id"),
        &amendment.proposal_id,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.certificate_id"),
        &amendment.certificate_id,
    );
    append_canonical_str(bytes, &format!("{prefix}.proposer"), &amendment.proposer);
    append_string_list(bytes, &format!("{prefix}.validator"), &amendment.validators);
    append_canonical_usize(bytes, &format!("{prefix}.quorum"), amendment.quorum);
    append_canonical_str(bytes, &format!("{prefix}.kind"), &amendment.kind);
    append_canonical_u32(bytes, &format!("{prefix}.value"), amendment.value);
    append_canonical_u64(
        bytes,
        &format!("{prefix}.activation_height"),
        amendment.activation_height,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.veto_until_height"),
        amendment.veto_until_height,
    );
    append_canonical_bool(bytes, &format!("{prefix}.paused"), amendment.paused);
    append_string_list(bytes, &format!("{prefix}.support"), &amendment.support);
    append_canonical_usize(
        bytes,
        &format!("{prefix}.vote_count"),
        amendment.votes.len(),
    );
    for vote in &amendment.votes {
        append_governance_vote(bytes, &format!("{prefix}.vote"), vote);
    }
}

pub(super) fn append_governance_vote(bytes: &mut Vec<u8>, prefix: &str, vote: &GovernanceVote) {
    append_canonical_str(bytes, &format!("{prefix}.vote_id"), &vote.vote_id);
    append_canonical_str(bytes, &format!("{prefix}.validator"), &vote.validator);
    append_canonical_bool(bytes, &format!("{prefix}.accept"), vote.accept);
}

pub(super) fn append_governance_activation_record(
    bytes: &mut Vec<u8>,
    prefix: &str,
    record: &GovernanceAmendmentActivationRecord,
) {
    append_canonical_str(bytes, &format!("{prefix}.schema"), &record.schema);
    append_canonical_str(
        bytes,
        &format!("{prefix}.activation_record_id"),
        &record.activation_record_id,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.amendment_id"),
        &record.amendment_id,
    );
    append_canonical_str(bytes, &format!("{prefix}.chain_id"), &record.chain_id);
    append_canonical_str(
        bytes,
        &format!("{prefix}.genesis_hash"),
        &record.genesis_hash,
    );
    append_canonical_u32(
        bytes,
        &format!("{prefix}.protocol_version"),
        record.protocol_version,
    );
    append_canonical_str(bytes, &format!("{prefix}.batch_id"), &record.batch_id);
    append_canonical_str(bytes, &format!("{prefix}.kind"), &record.kind);
    append_canonical_u32(bytes, &format!("{prefix}.value"), record.value);
    append_canonical_u32(
        bytes,
        &format!("{prefix}.previous_value"),
        record.previous_value,
    );
    append_canonical_u32(bytes, &format!("{prefix}.new_value"), record.new_value);
    append_canonical_u64(
        bytes,
        &format!("{prefix}.activation_height"),
        record.activation_height,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.veto_until_height"),
        record.veto_until_height,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.activated_height"),
        record.activated_height,
    );
}

pub(super) fn append_governance_supersession_record(
    bytes: &mut Vec<u8>,
    prefix: &str,
    record: &GovernanceAmendmentSupersessionRecord,
) {
    append_canonical_str(bytes, &format!("{prefix}.schema"), &record.schema);
    append_canonical_str(
        bytes,
        &format!("{prefix}.supersession_record_id"),
        &record.supersession_record_id,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.superseded_amendment_id"),
        &record.superseded_amendment_id,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.superseding_amendment_id"),
        &record.superseding_amendment_id,
    );
    append_canonical_str(bytes, &format!("{prefix}.chain_id"), &record.chain_id);
    append_canonical_str(
        bytes,
        &format!("{prefix}.genesis_hash"),
        &record.genesis_hash,
    );
    append_canonical_u32(
        bytes,
        &format!("{prefix}.protocol_version"),
        record.protocol_version,
    );
    append_canonical_str(bytes, &format!("{prefix}.batch_id"), &record.batch_id);
    append_canonical_str(bytes, &format!("{prefix}.kind"), &record.kind);
    append_canonical_u32(
        bytes,
        &format!("{prefix}.previous_value"),
        record.previous_value,
    );
    append_canonical_u32(bytes, &format!("{prefix}.new_value"), record.new_value);
    append_canonical_u64(
        bytes,
        &format!("{prefix}.supersession_height"),
        record.supersession_height,
    );
}

pub(super) fn append_governance_rollback_record(
    bytes: &mut Vec<u8>,
    prefix: &str,
    record: &GovernanceAmendmentRollbackRecord,
) {
    append_canonical_str(bytes, &format!("{prefix}.schema"), &record.schema);
    append_canonical_str(
        bytes,
        &format!("{prefix}.rollback_record_id"),
        &record.rollback_record_id,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.rolled_back_amendment_id"),
        &record.rolled_back_amendment_id,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.restored_amendment_id"),
        &record.restored_amendment_id,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.rollback_amendment_id"),
        &record.rollback_amendment_id,
    );
    append_canonical_str(bytes, &format!("{prefix}.chain_id"), &record.chain_id);
    append_canonical_str(
        bytes,
        &format!("{prefix}.genesis_hash"),
        &record.genesis_hash,
    );
    append_canonical_u32(
        bytes,
        &format!("{prefix}.protocol_version"),
        record.protocol_version,
    );
    append_canonical_str(bytes, &format!("{prefix}.batch_id"), &record.batch_id);
    append_canonical_str(bytes, &format!("{prefix}.kind"), &record.kind);
    append_canonical_u32(
        bytes,
        &format!("{prefix}.previous_value"),
        record.previous_value,
    );
    append_canonical_u32(
        bytes,
        &format!("{prefix}.restored_value"),
        record.restored_value,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.rollback_height"),
        record.rollback_height,
    );
}

pub(super) fn append_governance_agent_dry_run_record(
    bytes: &mut Vec<u8>,
    prefix: &str,
    record: &GovernanceAgentDryRunRecord,
) {
    append_canonical_str(bytes, &format!("{prefix}.schema"), &record.schema);
    append_canonical_str(bytes, &format!("{prefix}.record_id"), &record.record_id);
    append_canonical_str(bytes, &format!("{prefix}.dry_run_id"), &record.dry_run_id);
    append_canonical_str(bytes, &format!("{prefix}.chain_id"), &record.chain_id);
    append_canonical_str(
        bytes,
        &format!("{prefix}.genesis_hash"),
        &record.genesis_hash,
    );
    append_canonical_u32(
        bytes,
        &format!("{prefix}.protocol_version"),
        record.protocol_version,
    );
    append_canonical_str(bytes, &format!("{prefix}.batch_id"), &record.batch_id);
    append_canonical_u64(
        bytes,
        &format!("{prefix}.recorded_height"),
        record.recorded_height,
    );
    append_canonical_str(bytes, &format!("{prefix}.action_mode"), &record.action_mode);
    append_canonical_str(
        bytes,
        &format!("{prefix}.previous_dry_run_id"),
        &record.previous_dry_run_id,
    );
    append_canonical_str(bytes, &format!("{prefix}.bundle_hash"), &record.bundle_hash);
    append_canonical_str(
        bytes,
        &format!("{prefix}.architecture_statement_hash"),
        &record.architecture_statement_hash,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.objective_statement_hash"),
        &record.objective_statement_hash,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.ruleset_hash"),
        &record.ruleset_hash,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.compiled_policy_hash"),
        &record.compiled_policy_hash,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.replay_bundle_root"),
        &record.replay_bundle_root,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.replay_bundle_uri"),
        &record.replay_bundle_uri,
    );
    append_canonical_str(bytes, &format!("{prefix}.report_root"), &record.report_root);
    append_canonical_str(bytes, &format!("{prefix}.report_uri"), &record.report_uri);
    append_canonical_str(
        bytes,
        &format!("{prefix}.validator_registry_root_before"),
        &record.validator_registry_root_before,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.validator_registry_root_after"),
        &record.validator_registry_root_after,
    );
    append_canonical_u32(
        bytes,
        &format!("{prefix}.registry_mutation_count"),
        record.registry_mutation_count,
    );
}

pub(super) fn append_vault_bridge_route_profile_record(
    bytes: &mut Vec<u8>,
    prefix: &str,
    record: &postfiat_types::VaultBridgeRouteProfileRecordV1,
) {
    append_canonical_str(bytes, &format!("{prefix}.schema"), &record.schema);
    append_canonical_str(
        bytes,
        &format!("{prefix}.profile_hash"),
        &record.profile_hash,
    );
    let profile = &record.profile;
    append_canonical_str(bytes, &format!("{prefix}.profile.schema"), &profile.schema);
    append_canonical_str(
        bytes,
        &format!("{prefix}.profile.route_id"),
        &profile.route_id,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.profile.asset_id"),
        &profile.asset_id,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.profile.source_chain_id"),
        profile.source_chain_id,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.profile.vault_address"),
        &profile.vault_address,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.profile.vault_runtime_code_hash"),
        &profile.vault_runtime_code_hash,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.profile.token_address"),
        &profile.token_address,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.profile.token_runtime_code_hash"),
        &profile.token_runtime_code_hash,
    );
    append_canonical_u32(
        bytes,
        &format!("{prefix}.profile.route_epoch"),
        profile.route_epoch,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.profile.verifier_kind"),
        &profile.verifier_kind,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.profile.evidence_tier"),
        &profile.evidence_tier,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.profile.verifier_policy_hash"),
        &profile.verifier_policy_hash,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.profile.verifier_program_vkey"),
        &profile.verifier_program_vkey,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.profile.verifier_proof_encoding"),
        &profile.verifier_proof_encoding,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.profile.max_proof_bytes"),
        profile.max_proof_bytes,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.profile.max_public_values_bytes"),
        profile.max_public_values_bytes,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.profile.max_snapshot_age_blocks"),
        profile.max_snapshot_age_blocks,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.profile.challenge_window_blocks"),
        profile.challenge_window_blocks,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.profile.max_epoch_gap_blocks"),
        profile.max_epoch_gap_blocks,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.profile.settle_deadline_blocks"),
        profile.settle_deadline_blocks,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.profile.min_challenge_bond"),
        profile.min_challenge_bond,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.profile.min_attestations"),
        profile.min_attestations,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.profile.minimum_confirmations"),
        profile.minimum_confirmations,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.profile.activation_height"),
        profile.activation_height,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.profile.expires_at_height"),
        profile.expires_at_height,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.governance_amendment_id"),
        &record.governance_amendment_id,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.authorized_height"),
        record.authorized_height,
    );
}

pub(super) fn append_validator_registry_update_record(
    bytes: &mut Vec<u8>,
    prefix: &str,
    record: &ValidatorRegistryUpdateRecord,
) {
    append_canonical_str(bytes, &format!("{prefix}.schema"), &record.schema);
    append_canonical_str(bytes, &format!("{prefix}.update_id"), &record.update_id);
    append_canonical_str(bytes, &format!("{prefix}.chain_id"), &record.chain_id);
    append_canonical_str(
        bytes,
        &format!("{prefix}.genesis_hash"),
        &record.genesis_hash,
    );
    append_canonical_u32(
        bytes,
        &format!("{prefix}.protocol_version"),
        record.protocol_version,
    );
    append_canonical_str(bytes, &format!("{prefix}.instance_id"), &record.instance_id);
    append_canonical_str(bytes, &format!("{prefix}.proposal_id"), &record.proposal_id);
    append_canonical_str(
        bytes,
        &format!("{prefix}.certificate_id"),
        &record.certificate_id,
    );
    append_canonical_str(bytes, &format!("{prefix}.proposer"), &record.proposer);
    append_string_list(bytes, &format!("{prefix}.validator"), &record.validators);
    append_canonical_usize(bytes, &format!("{prefix}.quorum"), record.quorum);
    append_string_list(bytes, &format!("{prefix}.support"), &record.support);
    append_canonical_usize(bytes, &format!("{prefix}.vote_count"), record.votes.len());
    for vote in &record.votes {
        append_governance_vote(bytes, &format!("{prefix}.vote"), vote);
    }
    append_canonical_u64(
        bytes,
        &format!("{prefix}.activation_height"),
        record.activation_height,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.previous_registry_root"),
        &record.previous_registry_root,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.new_registry_root"),
        &record.new_registry_root,
    );
    append_option_str(
        bytes,
        &format!("{prefix}.previous_trust_graph_root"),
        &record.previous_trust_graph_root,
    );
    append_option_str(
        bytes,
        &format!("{prefix}.new_trust_graph_root"),
        &record.new_trust_graph_root,
    );
    append_option_str(
        bytes,
        &format!("{prefix}.trust_graph_transition_id"),
        &record.trust_graph_transition_id,
    );
    append_string_list(
        bytes,
        &format!("{prefix}.previous_validator"),
        &record.previous_validators,
    );
    append_string_list(
        bytes,
        &format!("{prefix}.new_validator"),
        &record.new_validators,
    );
    append_canonical_str(bytes, &format!("{prefix}.operation"), &record.operation);
    append_canonical_str(
        bytes,
        &format!("{prefix}.subject_node_id"),
        &record.subject_node_id,
    );
    append_option_validator_registry_entry(
        bytes,
        &format!("{prefix}.previous_record"),
        record.previous_record.as_ref(),
    );
    append_option_validator_registry_entry(
        bytes,
        &format!("{prefix}.new_record"),
        record.new_record.as_ref(),
    );
}

pub(super) fn append_option_validator_registry_entry(
    bytes: &mut Vec<u8>,
    prefix: &str,
    entry: Option<&ValidatorRegistryEntry>,
) {
    append_canonical_bool(bytes, &format!("{prefix}.present"), entry.is_some());
    if let Some(entry) = entry {
        append_validator_registry_entry(bytes, prefix, entry);
    }
}

pub(super) fn append_validator_registry_entry(
    bytes: &mut Vec<u8>,
    prefix: &str,
    entry: &ValidatorRegistryEntry,
) {
    append_canonical_str(bytes, &format!("{prefix}.node_id"), &entry.node_id);
    append_canonical_str(
        bytes,
        &format!("{prefix}.algorithm_id"),
        &entry.algorithm_id,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.public_key_hex"),
        &entry.public_key_hex,
    );
    append_canonical_bool(bytes, &format!("{prefix}.active"), entry.active);
}

pub(super) fn append_shielded_note(bytes: &mut Vec<u8>, prefix: &str, note: &ShieldedNote) {
    append_canonical_str(bytes, &format!("{prefix}.note_id"), &note.note_id);
    append_canonical_str(bytes, &format!("{prefix}.commitment"), &note.commitment);
    append_canonical_u64(bytes, &format!("{prefix}.position"), note.position);
    append_canonical_str(bytes, &format!("{prefix}.owner"), &note.owner);
    append_canonical_str(bytes, &format!("{prefix}.asset_id"), &note.asset_id);
    append_canonical_u64(bytes, &format!("{prefix}.value"), note.value);
    append_canonical_str(bytes, &format!("{prefix}.rho"), &note.rho);
    append_canonical_str(bytes, &format!("{prefix}.memo"), &note.memo);
    append_canonical_str(bytes, &format!("{prefix}.created_by"), &note.created_by);
}

pub(super) fn append_turnstile_event(bytes: &mut Vec<u8>, prefix: &str, event: &TurnstileEvent) {
    append_canonical_str(bytes, &format!("{prefix}.event_id"), &event.event_id);
    append_canonical_str(bytes, &format!("{prefix}.kind"), &event.kind);
    append_canonical_str(bytes, &format!("{prefix}.owner"), &event.owner);
    append_canonical_str(bytes, &format!("{prefix}.asset_id"), &event.asset_id);
    append_canonical_u64(bytes, &format!("{prefix}.amount"), event.amount);
    append_canonical_str(bytes, &format!("{prefix}.note_id"), &event.note_id);
    append_canonical_str(bytes, &format!("{prefix}.source_pool"), &event.source_pool);
    append_canonical_str(bytes, &format!("{prefix}.target_pool"), &event.target_pool);
}

pub(super) fn append_orchard_pool_state(
    bytes: &mut Vec<u8>,
    prefix: &str,
    orchard: &OrchardPoolState,
) {
    append_canonical_str(bytes, &format!("{prefix}.pool_id"), &orchard.pool_id);
    let mut nullifiers = orchard.nullifiers.clone();
    nullifiers.sort();
    append_string_list(bytes, &format!("{prefix}.nullifier"), &nullifiers);
    append_string_list(
        bytes,
        &format!("{prefix}.output_commitment"),
        &orchard.output_commitments,
    );
    append_canonical_usize(
        bytes,
        &format!("{prefix}.encrypted_output_count"),
        orchard.encrypted_outputs.len(),
    );
    for output in &orchard.encrypted_outputs {
        append_orchard_encrypted_output(bytes, &format!("{prefix}.encrypted_output"), output);
    }
    let mut asset_records = orchard.asset_commitment_records.iter().collect::<Vec<_>>();
    asset_records.sort_by(|left, right| left.output_commitment.cmp(&right.output_commitment));
    append_canonical_usize(
        bytes,
        &format!("{prefix}.asset_commitment_record_count"),
        asset_records.len(),
    );
    for record in asset_records {
        append_orchard_asset_commitment_record(
            bytes,
            &format!("{prefix}.asset_commitment_record"),
            record,
        );
    }
    let mut asset_orchard_outputs = orchard.asset_orchard_outputs.iter().collect::<Vec<_>>();
    asset_orchard_outputs
        .sort_by(|left, right| left.output_commitment.cmp(&right.output_commitment));
    append_canonical_usize(
        bytes,
        &format!("{prefix}.asset_orchard_output_count"),
        asset_orchard_outputs.len(),
    );
    for record in asset_orchard_outputs {
        append_asset_orchard_encrypted_output_record(
            bytes,
            &format!("{prefix}.asset_orchard_output"),
            record,
        );
    }
    let mut asset_orchard_balances = orchard.asset_orchard_balances.iter().collect::<Vec<_>>();
    asset_orchard_balances.sort_by(|left, right| left.asset_id.cmp(&right.asset_id));
    append_canonical_usize(
        bytes,
        &format!("{prefix}.asset_orchard_balance_count"),
        asset_orchard_balances.len(),
    );
    for balance in asset_orchard_balances {
        append_asset_orchard_asset_balance(
            bytes,
            &format!("{prefix}.asset_orchard_balance"),
            balance,
        );
    }
    append_canonical_usize(
        bytes,
        &format!("{prefix}.root_history_count"),
        orchard.root_history.len(),
    );
    for record in &orchard.root_history {
        append_orchard_root_record(bytes, &format!("{prefix}.root_history"), record);
    }
    append_string_list(
        bytes,
        &format!("{prefix}.accepted_anchor"),
        &orchard.accepted_anchors,
    );
    append_canonical_i64(
        bytes,
        &format!("{prefix}.value_balance_total"),
        orchard.value_balance_total,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.turnstile_deposit_total"),
        orchard.turnstile_deposit_total,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.fee_burn_total"),
        orchard.fee_burn_total,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.withdraw_total"),
        orchard.withdraw_total,
    );
}

pub(super) fn append_orchard_root_record(
    bytes: &mut Vec<u8>,
    prefix: &str,
    record: &OrchardRootRecord,
) {
    append_canonical_str(bytes, &format!("{prefix}.root"), &record.root);
    append_canonical_u64(
        bytes,
        &format!("{prefix}.output_count"),
        record.output_count,
    );
}

pub(super) fn append_orchard_encrypted_output(
    bytes: &mut Vec<u8>,
    prefix: &str,
    output: &OrchardEncryptedOutputRecord,
) {
    append_canonical_str(bytes, &format!("{prefix}.cmx"), &output.cmx);
    append_canonical_str(bytes, &format!("{prefix}.epk"), &output.epk);
    append_canonical_str(
        bytes,
        &format!("{prefix}.enc_ciphertext"),
        &output.enc_ciphertext,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.out_ciphertext"),
        &output.out_ciphertext,
    );
    append_option_str(
        bytes,
        &format!("{prefix}.compact_ciphertext"),
        &output.compact_ciphertext,
    );
}

pub(super) fn append_orchard_asset_commitment_record(
    bytes: &mut Vec<u8>,
    prefix: &str,
    record: &OrchardAssetCommitmentRecord,
) {
    append_canonical_str(
        bytes,
        &format!("{prefix}.output_commitment"),
        &record.output_commitment,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.asset_commitment"),
        &record.asset_commitment,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.value_commitment"),
        &record.value_commitment,
    );
}

pub(super) fn append_asset_orchard_encrypted_output_record(
    bytes: &mut Vec<u8>,
    prefix: &str,
    record: &AssetOrchardEncryptedOutputRecord,
) {
    append_canonical_str(
        bytes,
        &format!("{prefix}.output_commitment"),
        &record.output_commitment,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.encrypted_output"),
        &record.encrypted_output,
    );
}

pub(super) fn append_asset_orchard_asset_balance(
    bytes: &mut Vec<u8>,
    prefix: &str,
    balance: &AssetOrchardAssetBalance,
) {
    append_canonical_str(bytes, &format!("{prefix}.asset_id"), &balance.asset_id);
    append_canonical_u64(
        bytes,
        &format!("{prefix}.ingress_total"),
        balance.ingress_total,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.egress_total"),
        balance.egress_total,
    );
    append_canonical_u64(bytes, &format!("{prefix}.live_total"), balance.live_total);
}

pub(super) fn append_bridge_domain(bytes: &mut Vec<u8>, prefix: &str, domain: &BridgeDomain) {
    append_canonical_str(bytes, &format!("{prefix}.domain_id"), &domain.domain_id);
    append_canonical_str(bytes, &format!("{prefix}.name"), &domain.name);
    append_canonical_str(
        bytes,
        &format!("{prefix}.source_chain"),
        &domain.source_chain,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.target_chain"),
        &domain.target_chain,
    );
    append_canonical_str(bytes, &format!("{prefix}.bridge_id"), &domain.bridge_id);
    append_canonical_str(
        bytes,
        &format!("{prefix}.door_account"),
        &domain.door_account,
    );
    append_canonical_u64(bytes, &format!("{prefix}.inbound_cap"), domain.inbound_cap);
    append_canonical_u64(
        bytes,
        &format!("{prefix}.outbound_cap"),
        domain.outbound_cap,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.inbound_used"),
        domain.inbound_used,
    );
    append_canonical_u64(
        bytes,
        &format!("{prefix}.outbound_used"),
        domain.outbound_used,
    );
    append_canonical_bool(bytes, &format!("{prefix}.paused"), domain.paused);
}

pub(super) fn append_bridge_transfer(bytes: &mut Vec<u8>, prefix: &str, transfer: &BridgeTransfer) {
    append_canonical_str(
        bytes,
        &format!("{prefix}.transfer_id"),
        &transfer.transfer_id,
    );
    append_canonical_str(bytes, &format!("{prefix}.domain_id"), &transfer.domain_id);
    append_canonical_str(
        bytes,
        &format!("{prefix}.source_chain"),
        &transfer.source_chain,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.target_chain"),
        &transfer.target_chain,
    );
    append_canonical_str(bytes, &format!("{prefix}.bridge_id"), &transfer.bridge_id);
    append_canonical_str(
        bytes,
        &format!("{prefix}.door_account"),
        &transfer.door_account,
    );
    append_canonical_str(bytes, &format!("{prefix}.direction"), &transfer.direction);
    append_canonical_str(bytes, &format!("{prefix}.from"), &transfer.from);
    append_canonical_str(bytes, &format!("{prefix}.to"), &transfer.to);
    append_canonical_str(bytes, &format!("{prefix}.asset_id"), &transfer.asset_id);
    append_canonical_u64(bytes, &format!("{prefix}.amount"), transfer.amount);
    append_canonical_str(bytes, &format!("{prefix}.witness_id"), &transfer.witness_id);
    append_canonical_u32(
        bytes,
        &format!("{prefix}.witness_epoch"),
        transfer.witness_epoch,
    );
    append_canonical_bool(
        bytes,
        &format!("{prefix}.witness_attestation.present"),
        transfer.witness_attestation.is_some(),
    );
    if let Some(attestation) = &transfer.witness_attestation {
        append_bridge_witness_attestation(
            bytes,
            &format!("{prefix}.witness_attestation"),
            attestation,
        );
    }
    append_canonical_u64(bytes, &format!("{prefix}.sequence"), transfer.sequence);
}

pub(super) fn append_bridge_witness_attestation(
    bytes: &mut Vec<u8>,
    prefix: &str,
    attestation: &BridgeWitnessAttestation,
) {
    append_canonical_str(
        bytes,
        &format!("{prefix}.attestation_id"),
        &attestation.attestation_id,
    );
    append_canonical_str(bytes, &format!("{prefix}.chain_id"), &attestation.chain_id);
    append_canonical_str(
        bytes,
        &format!("{prefix}.genesis_hash"),
        &attestation.genesis_hash,
    );
    append_canonical_u32(
        bytes,
        &format!("{prefix}.protocol_version"),
        attestation.protocol_version,
    );
    append_canonical_str(bytes, &format!("{prefix}.signer"), &attestation.signer);
    append_canonical_str(
        bytes,
        &format!("{prefix}.algorithm_id"),
        &attestation.algorithm_id,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.public_key_hex"),
        &attestation.public_key_hex,
    );
    append_canonical_str(
        bytes,
        &format!("{prefix}.signature_hex"),
        &attestation.signature_hex,
    );
}

pub(super) fn append_string_list(bytes: &mut Vec<u8>, label: &str, values: &[String]) {
    append_canonical_usize(bytes, &format!("{label}_count"), values.len());
    for value in values {
        append_canonical_str(bytes, label, value);
    }
}

pub(super) fn append_canonical_bytes_commitment(bytes: &mut Vec<u8>, label: &str, value: &[u8]) {
    append_canonical_usize(bytes, &format!("{label}.len"), value.len());
    append_canonical_str(
        bytes,
        &format!("{label}.sha3_384"),
        &hash_hex("postfiat.state-root.bytes.v1", value),
    );
}

pub(super) fn append_option_str(bytes: &mut Vec<u8>, label: &str, value: &Option<String>) {
    append_canonical_bool(bytes, &format!("{label}.present"), value.is_some());
    if let Some(value) = value {
        append_canonical_str(bytes, label, value);
    }
}

pub(super) fn append_option_u64(bytes: &mut Vec<u8>, label: &str, value: Option<u64>) {
    append_canonical_bool(bytes, &format!("{label}.present"), value.is_some());
    if let Some(value) = value {
        append_canonical_u64(bytes, label, value);
    }
}

pub(super) fn append_canonical_str(bytes: &mut Vec<u8>, label: &str, value: &str) {
    bytes.extend_from_slice(label.as_bytes());
    bytes.push(b'=');
    bytes.extend_from_slice(value.len().to_string().as_bytes());
    bytes.push(b':');
    bytes.extend_from_slice(value.as_bytes());
    bytes.push(b'\n');
}

pub(super) fn append_canonical_bool(bytes: &mut Vec<u8>, label: &str, value: bool) {
    bytes.extend_from_slice(label.as_bytes());
    bytes.push(b'=');
    bytes.extend_from_slice(if value { b"true" } else { b"false" });
    bytes.push(b'\n');
}

pub(super) fn append_canonical_u64(bytes: &mut Vec<u8>, label: &str, value: u64) {
    bytes.extend_from_slice(label.as_bytes());
    bytes.push(b'=');
    bytes.extend_from_slice(value.to_string().as_bytes());
    bytes.push(b'\n');
}

pub(super) fn append_canonical_u128(bytes: &mut Vec<u8>, label: &str, value: u128) {
    bytes.extend_from_slice(label.as_bytes());
    bytes.push(b'=');
    bytes.extend_from_slice(value.to_string().as_bytes());
    bytes.push(b'\n');
}

pub(super) fn append_canonical_u32(bytes: &mut Vec<u8>, label: &str, value: u32) {
    append_canonical_u64(bytes, label, value as u64);
}

pub(super) fn append_canonical_u8(bytes: &mut Vec<u8>, label: &str, value: u8) {
    append_canonical_u64(bytes, label, value as u64);
}

pub(super) fn append_canonical_usize(bytes: &mut Vec<u8>, label: &str, value: usize) {
    append_canonical_u64(bytes, label, value as u64);
}

pub(super) fn append_canonical_i64(bytes: &mut Vec<u8>, label: &str, value: i64) {
    bytes.extend_from_slice(label.as_bytes());
    bytes.push(b'=');
    bytes.extend_from_slice(value.to_string().as_bytes());
    bytes.push(b'\n');
}
