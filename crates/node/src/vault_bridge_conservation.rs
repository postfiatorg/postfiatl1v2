use super::*;

pub const VAULT_BRIDGE_CONSERVATION_REPORT_SCHEMA: &str = "postfiat-vault-bridge-conservation-v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultBridgeConservationOptions {
    pub data_dir: PathBuf,
    pub asset_id: String,
    pub source_rpc_url: String,
    pub cast_binary: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultBridgeConservationRouteRow {
    pub profile_hash: String,
    pub route_id: String,
    pub route_epoch: u32,
    pub source_chain_id: u64,
    pub vault_address: String,
    pub token_address: String,
    pub vault_runtime_code_hash: String,
    pub token_runtime_code_hash: String,
    pub vault_balance_atoms: u64,
    pub balance_counted_once: bool,
    pub activation_height: u64,
    pub expires_at_height: u64,
    pub current_for_new_ingress: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultBridgeConservationDepositRow {
    pub evidence_root: String,
    pub deposit_id: String,
    pub profile_hash: String,
    pub amount_atoms: u64,
    pub status: String,
    pub source_deposit_seen: bool,
    pub counted_atoms: u64,
    pub uncredited_atoms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultBridgeConservationRedemptionRow {
    pub redemption_id: String,
    pub profile_hash: String,
    pub amount_atoms: u64,
    pub settled_atoms: u64,
    pub burned_unsettled_atoms: u64,
    pub source_withdrawal_claimed: bool,
    pub released_unsettled_atoms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultBridgeConservationReport {
    pub schema: String,
    pub asset_id: String,
    pub current_height: u64,
    /// V: exact token atoms held across every governed source vault for this asset.
    pub source_vault_atoms: u64,
    /// S: live PFTL claims backed by the vault, excluding burned redemptions.
    pub live_claim_atoms: u64,
    pub issued_supply_atoms: u64,
    pub wrapped_supply_atoms: u64,
    pub nav_subscription_claim_atoms: u64,
    pub other_claim_atoms: u64,
    /// D: source deposits not yet represented by a live PFTL claim.
    pub uncredited_deposit_atoms: u64,
    pub recognized_but_unallocated_atoms: u64,
    pub observed_but_uncounted_atoms: u64,
    /// B: burned PFTL claims not yet settled on PFTL.
    pub burned_unsettled_atoms: u64,
    /// R: the subset of B already released by the source vault.
    pub released_unsettled_atoms: u64,
    pub expected_source_vault_atoms: u64,
    pub unexplained_delta_atoms: i128,
    pub conserved: bool,
    pub route_count: u64,
    pub deposit_count: u64,
    pub redemption_count: u64,
    pub routes: Vec<VaultBridgeConservationRouteRow>,
    pub deposits: Vec<VaultBridgeConservationDepositRow>,
    pub redemptions: Vec<VaultBridgeConservationRedemptionRow>,
    pub disclosure: String,
}

impl VaultBridgeConservationReport {
    pub fn verify(&self) -> io::Result<()> {
        if !self.conserved || self.unexplained_delta_atoms != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "vault bridge conservation mismatch: V={} S={} D={} B={} R={} expected={} unexplained_delta={}",
                    self.source_vault_atoms,
                    self.live_claim_atoms,
                    self.uncredited_deposit_atoms,
                    self.burned_unsettled_atoms,
                    self.released_unsettled_atoms,
                    self.expected_source_vault_atoms,
                    self.unexplained_delta_atoms,
                ),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SourceRouteFacts {
    source_deposit_ids: BTreeSet<String>,
    source_claimed_withdrawal_ids: BTreeSet<String>,
}

pub fn vault_bridge_conservation_audit(
    options: VaultBridgeConservationOptions,
) -> io::Result<VaultBridgeConservationReport> {
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let governance = store.read_governance()?;
    let ledger = store.read_ledger()?;
    let shielded = store.read_shielded()?;
    let tip = read_chain_tip_or_reconstruct_for_genesis(&store, &genesis)?;

    let current = governance
        .active_vault_bridge_route_profile(&options.asset_id, tip.height)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    validate_vault_bridge_route_profile_against_ledger(
        &ledger,
        &current.profile,
        &current.profile_hash,
    )
    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

    let mut records = governance
        .vault_bridge_route_profiles
        .iter()
        .filter(|record| record.profile.asset_id == options.asset_id)
        .cloned()
        .collect::<Vec<_>>();
    records.sort_by(|left, right| {
        left.profile
            .route_epoch
            .cmp(&right.profile.route_epoch)
            .then(left.profile_hash.cmp(&right.profile_hash))
    });
    if records.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "vault bridge conservation requires at least one governed route profile",
        ));
    }
    for record in &records {
        governance
            .authorized_vault_bridge_route_profile(&options.asset_id, &record.profile_hash)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    }

    let source_chain_id = cast_u64(
        &options.cast_binary,
        &["chain-id", "--rpc-url", &options.source_rpc_url],
        "source chain id",
    )?;
    if records
        .iter()
        .any(|record| record.profile.source_chain_id != source_chain_id)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("source RPC chain id {source_chain_id} does not match every governed route"),
        ));
    }

    let mut route_rows = Vec::with_capacity(records.len());
    let mut unique_vault_balances = BTreeMap::<(String, String), u64>::new();
    for record in &records {
        let vault_code = cast_hex_bytes(
            &options.cast_binary,
            &[
                "code",
                &record.profile.vault_address,
                "--rpc-url",
                &options.source_rpc_url,
            ],
            "vault runtime code",
        )?;
        let token_code = cast_hex_bytes(
            &options.cast_binary,
            &[
                "code",
                &record.profile.token_address,
                "--rpc-url",
                &options.source_rpc_url,
            ],
            "token runtime code",
        )?;
        if vault_code.is_empty() || token_code.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "governed route `{}` resolves to empty runtime code",
                    record.profile_hash
                ),
            ));
        }
        let observed_vault_hash =
            format!("0x{}", bytes_to_hex(&vault_bridge_keccak256(&vault_code)));
        let observed_token_hash =
            format!("0x{}", bytes_to_hex(&vault_bridge_keccak256(&token_code)));
        if observed_vault_hash != record.profile.vault_runtime_code_hash
            || observed_token_hash != record.profile.token_runtime_code_hash
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "governed route `{}` runtime code hash mismatch",
                    record.profile_hash
                ),
            ));
        }

        let key = (
            record.profile.vault_address.clone(),
            record.profile.token_address.clone(),
        );
        let balance_counted_once = !unique_vault_balances.contains_key(&key);
        let vault_balance_atoms = if let Some(balance) = unique_vault_balances.get(&key) {
            *balance
        } else {
            let balance = cast_u64(
                &options.cast_binary,
                &[
                    "call",
                    &record.profile.token_address,
                    "balanceOf(address)(uint256)",
                    &record.profile.vault_address,
                    "--rpc-url",
                    &options.source_rpc_url,
                ],
                "source vault token balance",
            )?;
            unique_vault_balances.insert(key, balance);
            balance
        };
        route_rows.push(VaultBridgeConservationRouteRow {
            profile_hash: record.profile_hash.clone(),
            route_id: record.profile.route_id.clone(),
            route_epoch: record.profile.route_epoch,
            source_chain_id: record.profile.source_chain_id,
            vault_address: record.profile.vault_address.clone(),
            token_address: record.profile.token_address.clone(),
            vault_runtime_code_hash: observed_vault_hash,
            token_runtime_code_hash: observed_token_hash,
            vault_balance_atoms,
            balance_counted_once,
            activation_height: record.profile.activation_height,
            expires_at_height: record.profile.expires_at_height,
            current_for_new_ingress: record.profile_hash == current.profile_hash,
        });
    }

    let mut source_facts = records
        .iter()
        .map(|record| {
            let key = (
                record.profile.vault_address.clone(),
                record.profile.token_address.clone(),
            );
            unique_vault_balances.get(&key).copied().ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "missing source vault balance")
            })?;
            Ok((
                record.profile_hash.clone(),
                SourceRouteFacts {
                    source_deposit_ids: BTreeSet::new(),
                    source_claimed_withdrawal_ids: BTreeSet::new(),
                },
            ))
        })
        .collect::<io::Result<BTreeMap<_, _>>>()?;

    for deposit in ledger
        .vault_bridge_deposits
        .iter()
        .filter(|deposit| deposit.asset_id == options.asset_id)
    {
        let record = route_record_for_policy(&records, &deposit.policy_hash)?;
        ensure_deposit_matches_route(deposit, record)?;
        let seen = cast_bool(
            &options.cast_binary,
            &[
                "call",
                &record.profile.vault_address,
                "deposit_seen(bytes32)(bool)",
                &format!("0x{}", deposit.evidence.deposit_id),
                "--rpc-url",
                &options.source_rpc_url,
            ],
            "source deposit_seen",
        )?;
        if !seen {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "PFTL deposit `{}` is absent from its governed source vault",
                    deposit.evidence.deposit_id
                ),
            ));
        }
        source_facts
            .get_mut(&record.profile_hash)
            .expect("route facts initialized from records")
            .source_deposit_ids
            .insert(deposit.evidence.deposit_id.clone());
    }

    for redemption in ledger
        .vault_bridge_redemptions
        .iter()
        .filter(|redemption| redemption.asset_id == options.asset_id)
    {
        let bucket = ledger
            .vault_bridge_bucket_states
            .iter()
            .find(|bucket| bucket.bucket_id == redemption.bucket_id)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "redemption `{}` references missing bucket",
                        redemption.redemption_id
                    ),
                )
            })?;
        let record = route_record_for_policy(&records, &bucket.policy_hash)?;
        ensure_redemption_matches_route(redemption, record)?;
        let withdrawal_id = vault_bridge_hex_bytes_exact(
            "vault bridge redemption id",
            &redemption.redemption_id,
            48,
        )
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        let commitment = format!(
            "0x{}",
            bytes_to_hex(&vault_bridge_keccak256(&withdrawal_id))
        );
        let claimed = cast_bool(
            &options.cast_binary,
            &[
                "call",
                &record.profile.vault_address,
                "claimed_withdrawal_id(bytes32)(bool)",
                &commitment,
                "--rpc-url",
                &options.source_rpc_url,
            ],
            "source claimed_withdrawal_id",
        )?;
        if claimed {
            source_facts
                .get_mut(&record.profile_hash)
                .expect("route facts initialized from records")
                .source_claimed_withdrawal_ids
                .insert(redemption.redemption_id.clone());
        } else if redemption.settled_atoms != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "PFTL redemption `{}` records {} settled atom(s), but the governed source vault has not claimed it",
                    redemption.redemption_id, redemption.settled_atoms
                ),
            ));
        }
    }

    let report = build_vault_bridge_conservation_report(
        &ledger,
        &shielded,
        &options.asset_id,
        tip.height,
        &source_facts,
        route_rows,
    )?;
    report.verify()?;
    Ok(report)
}

fn build_vault_bridge_conservation_report(
    ledger: &LedgerState,
    shielded: &ShieldedState,
    asset_id: &str,
    current_height: u64,
    source_facts: &BTreeMap<String, SourceRouteFacts>,
    routes: Vec<VaultBridgeConservationRouteRow>,
) -> io::Result<VaultBridgeConservationReport> {
    let issued_supply_atoms = issued_asset_supply_for_status(ledger, shielded, asset_id)?;
    let source_vault_atoms = routes
        .iter()
        .filter(|route| route.balance_counted_once)
        .try_fold(0_u64, |total, route| {
            total.checked_add(route.vault_balance_atoms).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "source vault balance overflow")
            })
        })?;

    let buckets = ledger
        .vault_bridge_bucket_states
        .iter()
        .filter(|bucket| bucket.asset_id == asset_id)
        .collect::<Vec<_>>();
    let wrapped_supply_atoms =
        sum_bucket_field(&buckets, |bucket| bucket.outstanding_vault_bridge_atoms)?;
    let nav_subscription_claim_atoms =
        sum_bucket_field(&buckets, |bucket| bucket.nav_subscription_allocations_atoms)?;
    let other_claim_atoms = sum_bucket_field(&buckets, |bucket| bucket.other_allocations_atoms)?;
    let burned_unsettled_atoms =
        sum_bucket_field(&buckets, |bucket| bucket.redemption_queue_atoms)?;
    let recognized_but_unallocated_atoms = buckets.iter().try_fold(0_u64, |total, bucket| {
        bucket
            .validate()
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        let allocated = bucket
            .allocated_atoms()
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        let unallocated = bucket.counted_value_atoms.saturating_sub(allocated);
        total.checked_add(unallocated).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "recognized unallocated bridge backing overflow",
            )
        })
    })?;
    if issued_supply_atoms != wrapped_supply_atoms {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "vault bridge issued supply {} does not match bucket wrapped supply {}",
                issued_supply_atoms, wrapped_supply_atoms
            ),
        ));
    }
    let live_claim_atoms = wrapped_supply_atoms
        .checked_add(nav_subscription_claim_atoms)
        .and_then(|value| value.checked_add(other_claim_atoms))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "live claim overflow"))?;

    let mut receipt_counted_by_evidence = BTreeMap::<String, u64>::new();
    for receipt in ledger
        .vault_bridge_receipts
        .iter()
        .filter(|receipt| receipt.asset_id == asset_id)
    {
        if let Some(evidence) = &receipt.bridge_deposit_evidence {
            let root = vault_bridge_deposit_evidence_root(evidence)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
            if receipt_counted_by_evidence
                .insert(root, receipt.counted_value_atoms)
                .is_some()
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "multiple bridge receipts reference one deposit evidence root",
                ));
            }
        }
    }

    let mut observed_but_uncounted_atoms = 0_u64;
    let mut deposits = Vec::new();
    for deposit in ledger
        .vault_bridge_deposits
        .iter()
        .filter(|deposit| deposit.asset_id == asset_id)
    {
        let facts = source_facts.get(&deposit.policy_hash).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("missing source facts for profile `{}`", deposit.policy_hash),
            )
        })?;
        let source_deposit_seen = facts
            .source_deposit_ids
            .contains(&deposit.evidence.deposit_id);
        if !source_deposit_seen {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "source facts omit deposit `{}`",
                    deposit.evidence.deposit_id
                ),
            ));
        }
        let counted_atoms = receipt_counted_by_evidence
            .get(&deposit.evidence_root)
            .copied()
            .unwrap_or(0);
        if counted_atoms > deposit.evidence.amount_atoms {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "bridge receipt counted value exceeds source deposit amount",
            ));
        }
        let uncredited_atoms = deposit.evidence.amount_atoms - counted_atoms;
        observed_but_uncounted_atoms = observed_but_uncounted_atoms
            .checked_add(uncredited_atoms)
            .ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "uncredited deposit overflow")
        })?;
        deposits.push(VaultBridgeConservationDepositRow {
            evidence_root: deposit.evidence_root.clone(),
            deposit_id: deposit.evidence.deposit_id.clone(),
            profile_hash: deposit.policy_hash.clone(),
            amount_atoms: deposit.evidence.amount_atoms,
            status: deposit.status.clone(),
            source_deposit_seen,
            counted_atoms,
            uncredited_atoms,
        });
    }
    deposits.sort_by(|left, right| left.evidence_root.cmp(&right.evidence_root));
    let uncredited_deposit_atoms = recognized_but_unallocated_atoms
        .checked_add(observed_but_uncounted_atoms)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "deposit total overflow"))?;

    let mut redemption_remaining_atoms = 0_u64;
    let mut released_unsettled_atoms = 0_u64;
    let mut redemptions = Vec::new();
    for redemption in ledger
        .vault_bridge_redemptions
        .iter()
        .filter(|redemption| redemption.asset_id == asset_id)
    {
        let bucket = buckets
            .iter()
            .find(|bucket| bucket.bucket_id == redemption.bucket_id)
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "redemption bucket is missing")
            })?;
        let facts = source_facts.get(&bucket.policy_hash).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("missing source facts for profile `{}`", bucket.policy_hash),
            )
        })?;
        let remaining = redemption
            .amount_atoms
            .checked_sub(redemption.settled_atoms)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "redemption settled amount exceeds amount",
                )
            })?;
        redemption_remaining_atoms = redemption_remaining_atoms
            .checked_add(remaining)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "redemption overflow"))?;
        let source_withdrawal_claimed = facts
            .source_claimed_withdrawal_ids
            .contains(&redemption.redemption_id);
        let released = if source_withdrawal_claimed {
            remaining
        } else {
            0
        };
        released_unsettled_atoms = released_unsettled_atoms
            .checked_add(released)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "release overflow"))?;
        redemptions.push(VaultBridgeConservationRedemptionRow {
            redemption_id: redemption.redemption_id.clone(),
            profile_hash: bucket.policy_hash.clone(),
            amount_atoms: redemption.amount_atoms,
            settled_atoms: redemption.settled_atoms,
            burned_unsettled_atoms: remaining,
            source_withdrawal_claimed,
            released_unsettled_atoms: released,
        });
    }
    redemptions.sort_by(|left, right| left.redemption_id.cmp(&right.redemption_id));
    if redemption_remaining_atoms != burned_unsettled_atoms {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "redemption records total {} does not match bucket redemption queue {}",
                redemption_remaining_atoms, burned_unsettled_atoms
            ),
        ));
    }

    let expected_before_release = live_claim_atoms
        .checked_add(uncredited_deposit_atoms)
        .and_then(|value| value.checked_add(burned_unsettled_atoms))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "conservation total overflow"))?;
    let expected_source_vault_atoms = expected_before_release
        .checked_sub(released_unsettled_atoms)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "released-unsettled atoms exceed bridge claims and deposits",
            )
        })?;
    let unexplained_delta_atoms =
        i128::from(source_vault_atoms) - i128::from(expected_source_vault_atoms);

    Ok(VaultBridgeConservationReport {
        schema: VAULT_BRIDGE_CONSERVATION_REPORT_SCHEMA.to_string(),
        asset_id: asset_id.to_string(),
        current_height,
        source_vault_atoms,
        live_claim_atoms,
        issued_supply_atoms,
        wrapped_supply_atoms,
        nav_subscription_claim_atoms,
        other_claim_atoms,
        uncredited_deposit_atoms,
        recognized_but_unallocated_atoms,
        observed_but_uncounted_atoms,
        burned_unsettled_atoms,
        released_unsettled_atoms,
        expected_source_vault_atoms,
        unexplained_delta_atoms,
        conserved: unexplained_delta_atoms == 0,
        route_count: routes.len() as u64,
        deposit_count: deposits.len() as u64,
        redemption_count: redemptions.len() as u64,
        routes,
        deposits,
        redemptions,
        disclosure: "Exact source-vault conservation audit. V is fetched directly from the governed token contract; S is the complete live PFTL claim set; D separates recognized-unallocated and observed-uncounted deposits; B is the burned-unsettled redemption queue; R is independently read from the governed source vault claimed-withdrawal mapping. Any nonzero unexplained delta fails the audit."
            .to_string(),
    })
}

fn sum_bucket_field(
    buckets: &[&VaultBridgeBucketState],
    field: impl Fn(&VaultBridgeBucketState) -> u64,
) -> io::Result<u64> {
    buckets.iter().try_fold(0_u64, |total, bucket| {
        bucket
            .validate()
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        total.checked_add(field(bucket)).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "vault bridge bucket total overflow",
            )
        })
    })
}

fn route_record_for_policy<'a>(
    records: &'a [postfiat_types::VaultBridgeRouteProfileRecordV1],
    profile_hash: &str,
) -> io::Result<&'a postfiat_types::VaultBridgeRouteProfileRecordV1> {
    records
        .iter()
        .find(|record| record.profile_hash == profile_hash)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("bridge state references unknown governed profile `{profile_hash}`"),
            )
        })
}

fn ensure_deposit_matches_route(
    deposit: &VaultBridgeDepositRecord,
    route: &postfiat_types::VaultBridgeRouteProfileRecordV1,
) -> io::Result<()> {
    let expected_binding =
        vault_bridge_route_binding(&route.profile_hash, route.profile.route_epoch)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    if deposit.evidence.source_chain_id != route.profile.source_chain_id
        || deposit.evidence.vault_address != route.profile.vault_address
        || deposit.evidence.token_address != route.profile.token_address
        || deposit.evidence.route_binding != expected_binding
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "deposit `{}` does not match governed route `{}`",
                deposit.evidence.deposit_id, route.profile_hash
            ),
        ));
    }
    Ok(())
}

fn ensure_redemption_matches_route(
    redemption: &VaultBridgeRedemption,
    route: &postfiat_types::VaultBridgeRouteProfileRecordV1,
) -> io::Result<()> {
    let packet = &redemption.withdrawal_packet;
    if packet.source_chain_id != route.profile.source_chain_id
        || packet.vault_address != route.profile.vault_address
        || packet.token_address != route.profile.token_address
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "redemption `{}` does not match governed route `{}`",
                redemption.redemption_id, route.profile_hash
            ),
        ));
    }
    Ok(())
}

fn cast_output(cast_binary: &Path, args: &[&str], description: &str) -> io::Result<String> {
    let output = Command::new(cast_binary)
        .args(args)
        .output()
        .map_err(|error| {
            io::Error::new(
                error.kind(),
                format!("failed to run cast for {description}: {error}"),
            )
        })?;
    if !output.status.success() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!(
                "cast {description} failed with status {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        ));
    }
    if output.stdout.len() > 1024 * 1024 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("cast {description} output exceeds 1 MiB"),
        ));
    }
    String::from_utf8(output.stdout)
        .map(|value| value.trim().to_string())
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn cast_u64(cast_binary: &Path, args: &[&str], description: &str) -> io::Result<u64> {
    let output = cast_output(cast_binary, args, description)?;
    let value = output.split_whitespace().next().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("cast {description} returned no value"),
        )
    })?;
    if let Some(hex) = value.strip_prefix("0x") {
        u64::from_str_radix(hex, 16)
    } else {
        value.parse::<u64>()
    }
    .map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("cast {description} did not return a u64: `{value}`"),
        )
    })
}

fn cast_bool(cast_binary: &Path, args: &[&str], description: &str) -> io::Result<bool> {
    match cast_output(cast_binary, args, description)?.as_str() {
        "true" | "1" | "0x1" => Ok(true),
        "false" | "0" | "0x0" => Ok(false),
        value => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("cast {description} did not return a bool: `{value}`"),
        )),
    }
}

fn cast_hex_bytes(cast_binary: &Path, args: &[&str], description: &str) -> io::Result<Vec<u8>> {
    let output = cast_output(cast_binary, args, description)?;
    let value = output
        .strip_prefix("0x")
        .or_else(|| output.strip_prefix("0X"))
        .unwrap_or(&output);
    hex_to_bytes(value).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("cast {description} returned invalid hex: {error}"),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_amendment(kind: &str, value: u32, activation_height: u64) -> GovernanceAmendment {
        GovernanceAmendment {
            amendment_id: format!("conservation:{kind}:{value}:{activation_height}"),
            chain_id: "postfiat-local".to_string(),
            genesis_hash: "11".repeat(48),
            protocol_version: 1,
            instance_id: "conservation-instance".to_string(),
            proposal_id: "conservation-proposal".to_string(),
            certificate_id: "conservation-certificate".to_string(),
            proposer: "validator-0".to_string(),
            validators: vec!["validator-0".to_string()],
            quorum: 1,
            kind: kind.to_string(),
            value,
            activation_height,
            veto_until_height: 0,
            paused: false,
            support: vec!["validator-0".to_string()],
            votes: Vec::new(),
            signed_authorizations: Vec::new(),
        }
    }

    #[test]
    fn conservation_report_fails_closed_on_any_unexplained_atom() {
        let mut report = VaultBridgeConservationReport {
            schema: VAULT_BRIDGE_CONSERVATION_REPORT_SCHEMA.to_string(),
            asset_id: "11".repeat(48),
            current_height: 1,
            source_vault_atoms: 100,
            live_claim_atoms: 80,
            issued_supply_atoms: 80,
            wrapped_supply_atoms: 80,
            nav_subscription_claim_atoms: 0,
            other_claim_atoms: 0,
            uncredited_deposit_atoms: 10,
            recognized_but_unallocated_atoms: 10,
            observed_but_uncounted_atoms: 0,
            burned_unsettled_atoms: 20,
            released_unsettled_atoms: 10,
            expected_source_vault_atoms: 100,
            unexplained_delta_atoms: 0,
            conserved: true,
            route_count: 0,
            deposit_count: 0,
            redemption_count: 0,
            routes: Vec::new(),
            deposits: Vec::new(),
            redemptions: Vec::new(),
            disclosure: String::new(),
        };
        report.verify().expect("exact identity");
        report.source_vault_atoms = 101;
        report.unexplained_delta_atoms = 1;
        report.conserved = false;
        let error = report.verify().expect_err("one unexplained atom must fail");
        assert!(error.to_string().contains("unexplained_delta=1"));
    }

    #[test]
    fn cast_scalar_parsers_are_exact_and_reject_ambiguous_values() {
        let root = std::env::temp_dir().join(format!(
            "postfiat-vault-bridge-conservation-cast-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create fixture directory");
        let script = root.join("cast");
        std::fs::write(
            &script,
            "#!/bin/sh\ncase \"$1\" in\n  decimal) echo '42 [4.2e1]' ;;\n  hex) echo '0x2a' ;;\n  yes) echo true ;;\n  no) echo false ;;\n  ambiguous) echo maybe ;;\nesac\n",
        )
        .expect("write fixture cast");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = std::fs::metadata(&script).expect("metadata").permissions();
            permissions.set_mode(0o700);
            std::fs::set_permissions(&script, permissions).expect("chmod fixture cast");
        }
        assert_eq!(cast_u64(&script, &["decimal"], "decimal").unwrap(), 42);
        assert_eq!(cast_u64(&script, &["hex"], "hex").unwrap(), 42);
        assert!(cast_bool(&script, &["yes"], "yes").unwrap());
        assert!(!cast_bool(&script, &["no"], "no").unwrap());
        assert!(cast_bool(&script, &["ambiguous"], "ambiguous").is_err());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn source_rpc_audit_tracks_deposit_burn_release_and_fails_on_balance_drift() {
        use postfiat_types::{
            vault_bridge_deposit_id, AssetDefinition, NavProofProfile, NavTrackedAsset, TrustLine,
            VaultBridgeBucketState, VaultBridgeDepositEvidence, VaultBridgeDepositRecord,
            VaultBridgeRedemption, VaultBridgeRouteProfileActivationV1,
            VaultBridgeRouteProfileRecordV1, VaultBridgeRouteProfileV1,
            GOVERNANCE_KIND_VAULT_BRIDGE_ROUTE_AUTHORITY_ACTIVATION_HEIGHT,
            NAV_PROFILE_VERIFIER_MULTI_FETCH, VAULT_BRIDGE_EVIDENCE_TIER_INDEPENDENTLY_OBSERVED,
            VAULT_BRIDGE_ROUTE_PROFILE_ACTIVATION_SCHEMA_V1, VAULT_BRIDGE_ROUTE_PROFILE_SCHEMA_V1,
        };

        let root = std::env::temp_dir().join(format!(
            "postfiat-vault-bridge-conservation-boundary-{}",
            std::process::id()
        ));
        let data_dir = root.join("node");
        let _ = std::fs::remove_dir_all(&root);
        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-local".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 1,
        })
        .expect("initialize audit fixture");
        let store = NodeStore::new(&data_dir);
        let genesis = store.read_genesis().expect("fixture genesis");
        store
            .write_chain_tip(&ChainTipState {
                schema: CHAIN_TIP_SCHEMA.to_string(),
                chain_id: genesis.chain_id.clone(),
                genesis_hash: genesis_hash(&genesis),
                protocol_version: genesis.protocol_version,
                height: 10,
                block_hash: "aa".repeat(48),
                state_root: "bb".repeat(48),
                ordered_batch_count: 0,
                receipt_count: 0,
                history_base_height: 0,
            })
            .expect("write fixture tip");

        let vault_code = hex_to_bytes("6001").expect("vault code");
        let token_code = hex_to_bytes("6002").expect("token code");
        let asset = AssetDefinition::new("postfiat-local", "bridge-issuer", "pfUSDC", 1, 6)
            .expect("asset definition");
        let route = VaultBridgeRouteProfileV1 {
            schema: VAULT_BRIDGE_ROUTE_PROFILE_SCHEMA_V1.to_string(),
            route_id: "arbitrum-pfusdc".to_string(),
            asset_id: asset.asset_id.clone(),
            source_chain_id: 42_161,
            vault_address: "0x1111111111111111111111111111111111111111".to_string(),
            vault_runtime_code_hash: format!(
                "0x{}",
                bytes_to_hex(&vault_bridge_keccak256(&vault_code))
            ),
            token_address: "0x3333333333333333333333333333333333333333".to_string(),
            token_runtime_code_hash: format!(
                "0x{}",
                bytes_to_hex(&vault_bridge_keccak256(&token_code))
            ),
            route_epoch: 1,
            verifier_kind: NAV_PROFILE_VERIFIER_MULTI_FETCH.to_string(),
            evidence_tier: VAULT_BRIDGE_EVIDENCE_TIER_INDEPENDENTLY_OBSERVED.to_string(),
            verifier_policy_hash: String::new(),
            verifier_program_vkey: String::new(),
            verifier_proof_encoding: String::new(),
            max_proof_bytes: 0,
            max_public_values_bytes: 0,
            max_snapshot_age_blocks: 100,
            challenge_window_blocks: 6,
            max_epoch_gap_blocks: 100,
            settle_deadline_blocks: 100,
            min_challenge_bond: 1,
            min_attestations: 1,
            minimum_confirmations: 1,
            activation_height: 1,
            expires_at_height: 1_000,
        };
        let route_hash = route.profile_hash().expect("route hash");
        let mut current_route = route.clone();
        current_route.route_id = "arbitrum-pfusdc-v2".to_string();
        current_route.vault_address = "0x2222222222222222222222222222222222222222".to_string();
        current_route.route_epoch = 2;
        current_route.activation_height = 5;
        let current_route_hash = current_route.profile_hash().expect("current route hash");
        let route_amendment = test_amendment(
            &postfiat_types::vault_bridge_route_amendment_kind(&route)
                .expect("route amendment kind"),
            1,
            1,
        );
        let route_activation = VaultBridgeRouteProfileActivationV1 {
            schema: VAULT_BRIDGE_ROUTE_PROFILE_ACTIVATION_SCHEMA_V1.to_string(),
            profile: route.clone(),
            amendment: route_amendment.clone(),
            tier4_finality_bootstrap: None,
        };
        let current_route_amendment = test_amendment(
            &postfiat_types::vault_bridge_route_amendment_kind(&current_route)
                .expect("current route amendment kind"),
            2,
            5,
        );
        let current_route_activation = VaultBridgeRouteProfileActivationV1 {
            schema: VAULT_BRIDGE_ROUTE_PROFILE_ACTIVATION_SCHEMA_V1.to_string(),
            profile: current_route.clone(),
            amendment: current_route_amendment.clone(),
            tier4_finality_bootstrap: None,
        };
        let mut governance = store.read_governance().expect("fixture governance");
        governance.amendments.push(test_amendment(
            GOVERNANCE_KIND_VAULT_BRIDGE_ROUTE_AUTHORITY_ACTIVATION_HEIGHT,
            1,
            1,
        ));
        governance.amendments.push(route_amendment);
        governance.amendments.push(current_route_amendment);
        governance.vault_bridge_route_profiles.push(
            VaultBridgeRouteProfileRecordV1::new(&route_activation, 1).expect("route record"),
        );
        governance.vault_bridge_route_profiles.push(
            VaultBridgeRouteProfileRecordV1::new(&current_route_activation, 5)
                .expect("current route record"),
        );
        store
            .write_governance(&governance)
            .expect("write governed route");

        let nav_profile = NavProofProfile::new_with_bridge_observer_min_confirmations(
            "bridge-issuer",
            route.verifier_kind.clone(),
            format!("vault_bridge:{}", route.source_domain()),
            route.max_snapshot_age_blocks,
            route.challenge_window_blocks,
            route.max_epoch_gap_blocks,
            route.settle_deadline_blocks,
            route.min_challenge_bond,
            route.min_attestations,
            0,
            route.minimum_confirmations,
            route_hash.clone(),
            "",
            "",
            0,
            0,
        )
        .expect("NAV profile")
        .with_vault_bridge_route_policy_hash(route_hash.clone())
        .expect("route-bound NAV profile");
        let current_nav_profile = NavProofProfile::new_with_bridge_observer_min_confirmations(
            "bridge-issuer",
            current_route.verifier_kind.clone(),
            format!("vault_bridge:{}", current_route.source_domain()),
            current_route.max_snapshot_age_blocks,
            current_route.challenge_window_blocks,
            current_route.max_epoch_gap_blocks,
            current_route.settle_deadline_blocks,
            current_route.min_challenge_bond,
            current_route.min_attestations,
            0,
            current_route.minimum_confirmations,
            current_route_hash.clone(),
            "",
            "",
            0,
            0,
        )
        .expect("current NAV profile")
        .with_vault_bridge_route_policy_hash(current_route_hash)
        .expect("current route-bound NAV profile");
        let nav_asset = NavTrackedAsset::new(
            asset.asset_id.clone(),
            "bridge-issuer",
            "bridge-issuer",
            current_nav_profile.profile_id.clone(),
            "USDC",
            "redemption-account",
        )
        .expect("NAV asset");
        let mut holder_line =
            TrustLine::new("holder", "bridge-issuer", asset.asset_id.clone(), 1_000, 1)
                .expect("holder trustline");
        holder_line.balance = 80;
        let mut ledger = LedgerState::new(vec![
            Account::new("bridge-issuer", 0, None),
            Account::new("holder", 0, None),
        ]);
        ledger.asset_definitions.push(asset.clone());
        ledger.nav_proof_profiles.push(nav_profile);
        ledger.nav_proof_profiles.push(current_nav_profile);
        ledger.nav_assets.push(nav_asset);
        ledger.trustlines.push(holder_line);

        let mut bucket = VaultBridgeBucketState::new(
            asset.asset_id.clone(),
            route.source_domain(),
            route_hash.clone(),
            2,
        )
        .expect("bucket");
        bucket.gross_receipt_atoms = 110;
        bucket.counted_value_atoms = 110;
        bucket.outstanding_vault_bridge_atoms = 80;
        bucket.redemption_queue_atoms = 20;
        bucket.validate().expect("balanced bucket");
        let bucket_id = bucket.bucket_id.clone();
        ledger.vault_bridge_bucket_states.push(bucket);

        let mut evidence = VaultBridgeDepositEvidence {
            source_chain_id: route.source_chain_id,
            vault_address: route.vault_address.clone(),
            token_address: route.token_address.clone(),
            depositor: "0x5555555555555555555555555555555555555555".to_string(),
            pftl_recipient: "holder".to_string(),
            pftl_recipient_hash: vault_bridge_pftl_recipient_hash("holder")
                .expect("recipient hash"),
            amount_atoms: 5,
            nonce: "77".repeat(32),
            route_binding: vault_bridge_route_binding(&route_hash, route.route_epoch)
                .expect("route binding"),
            deposit_id: String::new(),
            block_hash: "88".repeat(32),
            tx_hash: "99".repeat(32),
            log_index: 0,
        };
        evidence.deposit_id = vault_bridge_deposit_id(&evidence).expect("deposit id");
        let evidence_root = vault_bridge_deposit_evidence_root(&evidence).expect("evidence root");
        ledger.vault_bridge_deposits.push(
            VaultBridgeDepositRecord::new(
                asset.asset_id.clone(),
                evidence_root,
                evidence,
                route_hash.clone(),
                String::new(),
                String::new(),
                String::new(),
                "holder",
                2,
                100,
            )
            .expect("uncredited deposit"),
        );
        ledger.vault_bridge_redemptions.push(
            VaultBridgeRedemption::new(
                "postfiat-local",
                "holder",
                "bridge-issuer",
                asset.asset_id.clone(),
                bucket_id,
                route.source_domain(),
                1,
                20,
                1,
                "aa".repeat(48),
                "evm-erc20:42161:0x2222222222222222222222222222222222222222",
                "cc".repeat(48),
                3,
            )
            .expect("pending redemption"),
        );
        store.write_ledger(&ledger).expect("write fixture ledger");

        let cast = root.join("cast");
        let write_cast = |chain_id: u64,
                          observed_vault_code: &str,
                          old_vault_balance: u64,
                          current_vault_balance: u64,
                          deposit_seen: bool,
                          withdrawal_claimed: bool| {
            std::fs::write(
                &cast,
                format!(
                    "#!/bin/sh\nif [ \"$1\" = chain-id ]; then echo {chain_id}; exit 0; fi\nif [ \"$1\" = code ] && [ \"$2\" = '{}' ]; then echo 0x{observed_vault_code}; exit 0; fi\nif [ \"$1\" = code ] && [ \"$2\" = '{}' ]; then echo 0x6001; exit 0; fi\nif [ \"$1\" = code ] && [ \"$2\" = '{}' ]; then echo 0x6002; exit 0; fi\nif [ \"$1\" = call ] && [ \"$3\" = 'balanceOf(address)(uint256)' ] && [ \"$4\" = '{}' ]; then echo {old_vault_balance}; exit 0; fi\nif [ \"$1\" = call ] && [ \"$3\" = 'balanceOf(address)(uint256)' ] && [ \"$4\" = '{}' ]; then echo {current_vault_balance}; exit 0; fi\nif [ \"$1\" = call ] && [ \"$3\" = 'deposit_seen(bytes32)(bool)' ]; then echo {deposit_seen}; exit 0; fi\nif [ \"$1\" = call ] && [ \"$3\" = 'claimed_withdrawal_id(bytes32)(bool)' ]; then echo {withdrawal_claimed}; exit 0; fi\necho unexpected >&2; exit 1\n",
                    route.vault_address,
                    current_route.vault_address,
                    route.token_address,
                    route.vault_address,
                    current_route.vault_address,
                ),
            )
            .expect("write fake cast");
        };
        write_cast(42_161, "6001", 80, 15, true, true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = std::fs::metadata(&cast).expect("metadata").permissions();
            permissions.set_mode(0o700);
            std::fs::set_permissions(&cast, permissions).expect("chmod fake cast");
        }

        let options = VaultBridgeConservationOptions {
            data_dir: data_dir.clone(),
            asset_id: asset.asset_id,
            source_rpc_url: "http://127.0.0.1:8545".to_string(),
            cast_binary: cast.clone(),
        };
        let report = vault_bridge_conservation_audit(options.clone()).expect("conserved audit");
        assert_eq!(95, report.source_vault_atoms);
        assert_eq!(80, report.live_claim_atoms);
        assert_eq!(15, report.uncredited_deposit_atoms);
        assert_eq!(10, report.recognized_but_unallocated_atoms);
        assert_eq!(5, report.observed_but_uncounted_atoms);
        assert_eq!(20, report.burned_unsettled_atoms);
        assert_eq!(20, report.released_unsettled_atoms);
        assert_eq!(95, report.expected_source_vault_atoms);
        assert_eq!(2, report.route_count);
        assert_eq!(
            1,
            report
                .routes
                .iter()
                .filter(|route| route.current_for_new_ingress)
                .count()
        );
        assert!(report.conserved);

        write_cast(42_162, "6001", 80, 15, true, true);
        let error = vault_bridge_conservation_audit(options.clone())
            .expect_err("wrong source network must fail closed");
        assert!(
            error.to_string().contains("chain id 42162"),
            "unexpected error: {error}"
        );

        write_cast(42_161, "6003", 80, 15, true, true);
        let error = vault_bridge_conservation_audit(options.clone())
            .expect_err("runtime code drift must fail closed");
        assert!(
            error.to_string().contains("runtime code hash mismatch"),
            "unexpected error: {error}"
        );

        write_cast(42_161, "6001", 80, 15, false, true);
        let error = vault_bridge_conservation_audit(options.clone())
            .expect_err("PFTL-only deposit must fail closed");
        assert!(
            error
                .to_string()
                .contains("absent from its governed source vault"),
            "unexpected error: {error}"
        );

        let mut impossible_ledger = ledger.clone();
        impossible_ledger.vault_bridge_redemptions[0].settled_atoms = 1;
        store
            .write_ledger(&impossible_ledger)
            .expect("write impossible settlement fixture");
        write_cast(42_161, "6001", 80, 15, true, false);
        let error = vault_bridge_conservation_audit(options.clone())
            .expect_err("PFTL settlement without source claim must fail closed");
        assert!(
            error
                .to_string()
                .contains("governed source vault has not claimed it"),
            "unexpected error: {error}"
        );
        store.write_ledger(&ledger).expect("restore fixture ledger");

        write_cast(42_161, "6001", 81, 15, true, true);
        let error = vault_bridge_conservation_audit(options)
            .expect_err("one unexplained source atom must fail closed");
        assert!(
            error.to_string().contains("unexplained_delta=1"),
            "unexpected error: {error}"
        );
        let _ = std::fs::remove_dir_all(root);
    }
}
