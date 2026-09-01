use super::*;

pub const VAULT_BRIDGE_CONSERVATION_REPORT_SCHEMA: &str = "postfiat-vault-bridge-conservation-v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultBridgeConservationOptions {
    pub data_dir: PathBuf,
    pub asset_id: String,
    /// Either one RPC URL for a single-chain route set, or a comma-separated
    /// `chain_id=url` map when the governed history spans multiple chains.
    pub source_rpc_url: String,
    pub cast_binary: PathBuf,
    pub vault_interface_lineage_manifest: PathBuf,
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
    pub vault_interface_abi_class: String,
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

fn source_rpc_urls_for_routes(
    cast_binary: &Path,
    source_rpc_url: &str,
    records: &[postfiat_types::VaultBridgeRouteProfileRecordV1],
) -> io::Result<BTreeMap<u64, String>> {
    let required_chain_ids = records
        .iter()
        .map(|record| record.profile.source_chain_id)
        .collect::<BTreeSet<_>>();
    let mut rpc_urls = BTreeMap::new();
    if source_rpc_url.contains('=') {
        for entry in source_rpc_url.split(',') {
            let (chain_id, rpc_url) = entry.split_once('=').ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "multi-chain source RPC entries must use chain_id=url",
                )
            })?;
            let chain_id = chain_id.parse::<u64>().map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("invalid source RPC chain id `{chain_id}`"),
                )
            })?;
            if rpc_url.is_empty() || rpc_urls.insert(chain_id, rpc_url.to_string()).is_some() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("invalid or duplicate source RPC entry for chain {chain_id}"),
                ));
            }
        }
    } else {
        let observed_chain_id = cast_u64(
            cast_binary,
            &["chain-id", "--rpc-url", source_rpc_url],
            "source chain id",
        )?;
        if required_chain_ids
            .iter()
            .any(|chain_id| *chain_id != observed_chain_id)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "source RPC chain id {observed_chain_id} does not match every governed route"
                ),
            ));
        }
        rpc_urls.insert(observed_chain_id, source_rpc_url.to_string());
    }
    for chain_id in &required_chain_ids {
        let rpc_url = rpc_urls.get(chain_id).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("missing source RPC URL for governed chain {chain_id}"),
            )
        })?;
        let observed_chain_id = cast_u64(
            cast_binary,
            &["chain-id", "--rpc-url", rpc_url],
            "source chain id",
        )?;
        if observed_chain_id != *chain_id {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "source RPC declared for chain {chain_id} returned chain id {observed_chain_id}"
                ),
            ));
        }
    }
    if rpc_urls
        .keys()
        .any(|chain_id| !required_chain_ids.contains(chain_id))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "source RPC map contains a chain with no governed route",
        ));
    }
    Ok(rpc_urls)
}

const VAULT_INTERFACE_LINEAGE_SCHEMA: &str = "postfiat.pfusdc.vault_interface_lineage.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum VaultInterfaceAbiClass {
    SnakeCaseV1,
    CamelCaseV2,
}

impl VaultInterfaceAbiClass {
    fn lineage_name(self) -> &'static str {
        match self {
            Self::SnakeCaseV1 => "snake_case_v1",
            Self::CamelCaseV2 => "camel_case_v2",
        }
    }

    fn deposit_seen_selector(self) -> &'static str {
        match self {
            Self::SnakeCaseV1 => "deposit_seen(bytes32)(bool)",
            Self::CamelCaseV2 => "depositSeen(bytes32)(bool)",
        }
    }

    fn withdrawal_claimed_selector(self) -> &'static str {
        match self {
            Self::SnakeCaseV1 => "claimed_withdrawal_id(bytes32)(bool)",
            Self::CamelCaseV2 => "consumedWithdrawalIdCommitment(bytes32)(bool)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum VaultInterfaceVerificationStatus {
    LiveVerified,
    ExpectedPendingLiveReadback,
}

#[derive(Debug, Clone, Deserialize)]
struct VaultInterfaceLineageEntry {
    runtime_code_hash: String,
    abi_class: VaultInterfaceAbiClass,
    source_manifest_path: String,
    source_manifest_sha256: String,
    deployment_revision_label: String,
    verification_status: VaultInterfaceVerificationStatus,
}

#[derive(Debug, Deserialize)]
struct VaultInterfaceLineageManifest {
    schema: String,
    version: u32,
    entries: Vec<VaultInterfaceLineageEntry>,
}

fn invalid_vault_interface_lineage(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

fn validate_runtime_code_hash(runtime_code_hash: &str) -> io::Result<()> {
    if runtime_code_hash.len() != 66
        || !runtime_code_hash.starts_with("0x")
        || runtime_code_hash != runtime_code_hash.to_ascii_lowercase()
        || !runtime_code_hash[2..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(invalid_vault_interface_lineage(format!(
            "vault interface lineage runtime code hash must be exact lowercase 0x-prefixed bytes32: `{runtime_code_hash}`"
        )));
    }
    Ok(())
}

fn source_manifest_declares_runtime_hash(
    value: &serde_json::Value,
    runtime_code_hash: &str,
) -> bool {
    match value {
        serde_json::Value::String(text) => text == runtime_code_hash,
        serde_json::Value::Array(items) => items
            .iter()
            .any(|item| source_manifest_declares_runtime_hash(item, runtime_code_hash)),
        serde_json::Value::Object(items) => items
            .values()
            .any(|item| source_manifest_declares_runtime_hash(item, runtime_code_hash)),
        _ => false,
    }
}

fn resolve_lineage_source_manifest(
    lineage_manifest_path: &Path,
    source_manifest_path: &Path,
) -> io::Result<PathBuf> {
    lineage_manifest_path
        .parent()
        .into_iter()
        .flat_map(Path::ancestors)
        .map(|root| root.join(source_manifest_path))
        .find(|candidate| candidate.is_file())
        .ok_or_else(|| {
            invalid_vault_interface_lineage(format!(
                "vault interface lineage source manifest `{}` is unreadable from lineage ancestry",
                source_manifest_path.display()
            ))
        })
}

fn load_vault_interface_lineage(
    lineage_manifest_path: &Path,
) -> io::Result<BTreeMap<String, VaultInterfaceLineageEntry>> {
    let bytes = std::fs::read(lineage_manifest_path).map_err(|error| {
        invalid_vault_interface_lineage(format!(
            "vault interface lineage manifest `{}` is unreadable: {error}",
            lineage_manifest_path.display()
        ))
    })?;
    let lineage: VaultInterfaceLineageManifest =
        serde_json::from_slice(&bytes).map_err(|error| {
            invalid_vault_interface_lineage(format!(
                "vault interface lineage manifest `{}` is invalid JSON: {error}",
                lineage_manifest_path.display()
            ))
        })?;
    if lineage.schema != VAULT_INTERFACE_LINEAGE_SCHEMA || lineage.version != 1 {
        return Err(invalid_vault_interface_lineage(format!(
            "vault interface lineage manifest `{}` has unsupported schema/version",
            lineage_manifest_path.display()
        )));
    }
    if lineage.entries.is_empty() {
        return Err(invalid_vault_interface_lineage(
            "vault interface lineage manifest has no entries",
        ));
    }
    let mut entries = BTreeMap::new();
    for entry in lineage.entries {
        validate_runtime_code_hash(&entry.runtime_code_hash)?;
        if entry.deployment_revision_label.trim().is_empty() {
            return Err(invalid_vault_interface_lineage(format!(
                "vault interface lineage `{}` has an empty deployment revision label",
                entry.runtime_code_hash
            )));
        }
        if entry.source_manifest_sha256.len() != 64
            || entry.source_manifest_sha256 != entry.source_manifest_sha256.to_ascii_lowercase()
            || !entry
                .source_manifest_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(invalid_vault_interface_lineage(format!(
                "vault interface lineage `{}` has an invalid source manifest SHA-256",
                entry.runtime_code_hash
            )));
        }
        let source_path = Path::new(&entry.source_manifest_path);
        if source_path.as_os_str().is_empty()
            || source_path.is_absolute()
            || source_path
                .components()
                .any(|component| matches!(component, std::path::Component::ParentDir))
        {
            return Err(invalid_vault_interface_lineage(format!(
                "vault interface lineage `{}` has a non-repository-relative source manifest path",
                entry.runtime_code_hash
            )));
        }
        let resolved_source_path =
            resolve_lineage_source_manifest(lineage_manifest_path, source_path)?;
        let source_bytes = std::fs::read(&resolved_source_path).map_err(|error| {
            invalid_vault_interface_lineage(format!(
                "vault interface lineage source manifest `{}` is unreadable: {error}",
                resolved_source_path.display()
            ))
        })?;
        let mut hasher = Sha256::new();
        Sha2Digest::update(&mut hasher, &source_bytes);
        let observed_digest = bytes_to_hex(&hasher.finalize());
        if observed_digest != entry.source_manifest_sha256 {
            return Err(invalid_vault_interface_lineage(format!(
                "vault interface lineage source manifest digest mismatch for `{}`",
                entry.runtime_code_hash
            )));
        }
        let source_manifest: serde_json::Value =
            serde_json::from_slice(&source_bytes).map_err(|error| {
                invalid_vault_interface_lineage(format!(
                    "vault interface lineage source manifest `{}` is invalid JSON: {error}",
                    entry.source_manifest_path
                ))
            })?;
        if !source_manifest_declares_runtime_hash(&source_manifest, &entry.runtime_code_hash) {
            return Err(invalid_vault_interface_lineage(format!(
                "vault interface lineage source manifest `{}` does not declare runtime hash `{}`",
                entry.source_manifest_path, entry.runtime_code_hash
            )));
        }
        if entries
            .insert(entry.runtime_code_hash.clone(), entry)
            .is_some()
        {
            return Err(invalid_vault_interface_lineage(
                "vault interface lineage has duplicate runtime code hash mapping",
            ));
        }
    }
    Ok(entries)
}

fn select_vault_interface(
    entries: &BTreeMap<String, VaultInterfaceLineageEntry>,
    runtime_code_hash: &str,
) -> io::Result<VaultInterfaceAbiClass> {
    let entry = entries.get(runtime_code_hash).ok_or_else(|| {
        invalid_vault_interface_lineage(format!(
            "vault interface lineage has no entry for governed runtime hash `{runtime_code_hash}`"
        ))
    })?;
    if entry.verification_status != VaultInterfaceVerificationStatus::LiveVerified {
        return Err(invalid_vault_interface_lineage(format!(
            "vault interface lineage runtime hash `{runtime_code_hash}` is not live_verified"
        )));
    }
    Ok(entry.abi_class)
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
    let interface_lineage =
        load_vault_interface_lineage(&options.vault_interface_lineage_manifest)?;

    let source_rpc_urls =
        source_rpc_urls_for_routes(&options.cast_binary, &options.source_rpc_url, &records)?;

    let mut route_rows = Vec::with_capacity(records.len());
    let mut route_interfaces = BTreeMap::new();
    let mut unique_vault_balances = BTreeMap::<(u64, String, String), u64>::new();
    for record in &records {
        let source_rpc_url = source_rpc_urls
            .get(&record.profile.source_chain_id)
            .expect("required source RPC URLs validated");
        let vault_code = cast_hex_bytes(
            &options.cast_binary,
            &[
                "code",
                &record.profile.vault_address,
                "--rpc-url",
                source_rpc_url,
            ],
            "vault runtime code",
        )?;
        let token_code = cast_hex_bytes(
            &options.cast_binary,
            &[
                "code",
                &record.profile.token_address,
                "--rpc-url",
                source_rpc_url,
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
        let interface = select_vault_interface(&interface_lineage, &observed_vault_hash)?;
        route_interfaces.insert(record.profile_hash.clone(), interface);

        let key = (
            record.profile.source_chain_id,
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
                    source_rpc_url,
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
            vault_interface_abi_class: interface.lineage_name().to_string(),
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
                record.profile.source_chain_id,
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
        let interface = route_interfaces.get(&record.profile_hash).ok_or_else(|| {
            invalid_vault_interface_lineage("missing selected interface for governed deposit route")
        })?;
        let source_rpc_url = source_rpc_urls
            .get(&record.profile.source_chain_id)
            .expect("required source RPC URLs validated");
        let seen = cast_bool(
            &options.cast_binary,
            &[
                "call",
                &record.profile.vault_address,
                interface.deposit_seen_selector(),
                &format!("0x{}", deposit.evidence.deposit_id),
                "--rpc-url",
                source_rpc_url,
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
        let interface = route_interfaces.get(&record.profile_hash).ok_or_else(|| {
            invalid_vault_interface_lineage(
                "missing selected interface for governed redemption route",
            )
        })?;
        let source_rpc_url = source_rpc_urls
            .get(&record.profile.source_chain_id)
            .expect("required source RPC URLs validated");
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
                interface.withdrawal_claimed_selector(),
                &commitment,
                "--rpc-url",
                source_rpc_url,
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
    const MAX_ATTEMPTS: u32 = 4;
    let mut last_failure = None;
    let mut successful_output = None;
    for attempt in 1..=MAX_ATTEMPTS {
        let output = Command::new(cast_binary)
            .args(args)
            .output()
            .map_err(|error| {
                io::Error::new(
                    error.kind(),
                    format!("failed to run cast for {description}: {error}"),
                )
            })?;
        if output.status.success() {
            successful_output = Some(output);
            break;
        }
        last_failure = Some(format!(
            "status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
        if attempt < MAX_ATTEMPTS {
            std::thread::sleep(std::time::Duration::from_millis(u64::from(attempt) * 250));
        }
    }
    let output = successful_output.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::Other,
            format!(
                "cast {description} failed after {MAX_ATTEMPTS} attempt(s): {}",
                last_failure.unwrap_or_else(|| "unknown failure".to_string())
            ),
        )
    })?;
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

    fn write_test_interface_lineage(root: &Path, entries: &[(&str, &str, &str)]) -> PathBuf {
        std::fs::create_dir_all(root).expect("create interface lineage fixture directory");
        let mut serialized_entries = Vec::new();
        for (index, (runtime_code_hash, abi_class, verification_status)) in
            entries.iter().enumerate()
        {
            let source_name = format!("source-{index}.json");
            let source_path = root.join(&source_name);
            std::fs::write(
                &source_path,
                format!("{{\"vault_runtime_code_hash\":\"{runtime_code_hash}\"}}"),
            )
            .expect("write interface lineage source manifest");
            let mut hasher = Sha256::new();
            Sha2Digest::update(
                &mut hasher,
                std::fs::read(&source_path).expect("read interface lineage source manifest"),
            );
            serialized_entries.push(format!(
                "{{\"runtime_code_hash\":\"{runtime_code_hash}\",\"abi_class\":\"{abi_class}\",\"source_manifest_path\":\"{source_name}\",\"source_manifest_sha256\":\"{}\",\"deployment_revision_label\":\"fixture-{index}\",\"verification_status\":\"{verification_status}\"}}",
                bytes_to_hex(&hasher.finalize())
            ));
        }
        let lineage_path = root.join("vault-interface-lineage.json");
        std::fs::write(
            &lineage_path,
            format!(
                "{{\"schema\":\"{VAULT_INTERFACE_LINEAGE_SCHEMA}\",\"version\":1,\"entries\":[{}]}}",
                serialized_entries.join(",")
            ),
        )
        .expect("write interface lineage manifest");
        lineage_path
    }

    #[test]
    fn vault_bridge_conservation_interface_lineage_selects_closed_abis() {
        let root = std::env::temp_dir().join(format!(
            "postfiat-vault-bridge-conservation-interfaces-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let snake = format!("0x{}", "11".repeat(32));
        let tier4_v2 = format!("0x{}", "22".repeat(32));
        let ethereum_l1 = format!("0x{}", "33".repeat(32));
        let lineage_path = write_test_interface_lineage(
            &root,
            &[
                (&snake, "snake_case_v1", "live_verified"),
                (&tier4_v2, "camel_case_v2", "live_verified"),
                (&ethereum_l1, "camel_case_v2", "live_verified"),
            ],
        );
        let entries = load_vault_interface_lineage(&lineage_path).expect("load lineage");
        let snake = select_vault_interface(&entries, &snake).expect("select snake interface");
        let tier4_v2 =
            select_vault_interface(&entries, &tier4_v2).expect("select tier4 V2 interface");
        let ethereum_l1 =
            select_vault_interface(&entries, &ethereum_l1).expect("select Ethereum L1 interface");
        assert_eq!("deposit_seen(bytes32)(bool)", snake.deposit_seen_selector());
        assert_eq!(
            "claimed_withdrawal_id(bytes32)(bool)",
            snake.withdrawal_claimed_selector()
        );
        assert_eq!(
            "depositSeen(bytes32)(bool)",
            tier4_v2.deposit_seen_selector()
        );
        assert_eq!(
            "consumedWithdrawalIdCommitment(bytes32)(bool)",
            tier4_v2.withdrawal_claimed_selector()
        );
        assert_eq!(tier4_v2, ethereum_l1);
        let unknown = format!("0x{}", "44".repeat(32));
        let error =
            select_vault_interface(&entries, &unknown).expect_err("unknown hash must fail closed");
        assert!(error.to_string().contains("no entry"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn vault_bridge_conservation_interface_lineage_rejects_pending_digest_and_duplicates() {
        let root = std::env::temp_dir().join(format!(
            "postfiat-vault-bridge-conservation-interface-rejections-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let pending = format!("0x{}", "55".repeat(32));
        let pending_path = write_test_interface_lineage(
            &root.join("pending"),
            &[(&pending, "camel_case_v2", "expected_pending_live_readback")],
        );
        let entries = load_vault_interface_lineage(&pending_path).expect("load pending lineage");
        let error =
            select_vault_interface(&entries, &pending).expect_err("pending hash must fail closed");
        assert!(error.to_string().contains("not live_verified"));

        let digest_hash = format!("0x{}", "66".repeat(32));
        let digest_root = root.join("digest");
        let digest_path = write_test_interface_lineage(
            &digest_root,
            &[(&digest_hash, "camel_case_v2", "live_verified")],
        );
        std::fs::write(digest_root.join("source-0.json"), "{}")
            .expect("tamper test source manifest");
        let error = load_vault_interface_lineage(&digest_path)
            .expect_err("digest mismatch must fail closed");
        assert!(error.to_string().contains("digest mismatch"));

        let duplicate = format!("0x{}", "77".repeat(32));
        let duplicate_path = write_test_interface_lineage(
            &root.join("duplicate"),
            &[
                (&duplicate, "snake_case_v1", "live_verified"),
                (&duplicate, "camel_case_v2", "live_verified"),
            ],
        );
        let error = load_vault_interface_lineage(&duplicate_path)
            .expect_err("duplicate mapping must fail closed");
        assert!(error.to_string().contains("duplicate runtime code hash"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn vault_bridge_conservation_selected_getter_revert_has_no_fallback() {
        let root = std::env::temp_dir().join(format!(
            "postfiat-vault-bridge-conservation-selector-revert-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create selector fixture directory");
        let script = root.join("cast");
        let fallback_marker = root.join("unexpected-snake-fallback");
        std::fs::write(
            &script,
            format!(
                "#!/bin/sh\nif [ \"$3\" = 'depositSeen(bytes32)(bool)' ]; then exit 1; fi\nif [ \"$3\" = 'deposit_seen(bytes32)(bool)' ]; then touch '{}'; echo true; exit 0; fi\nexit 1\n",
                fallback_marker.display()
            ),
        )
        .expect("write selector fixture cast");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = std::fs::metadata(&script).expect("metadata").permissions();
            permissions.set_mode(0o700);
            std::fs::set_permissions(&script, permissions).expect("chmod selector fixture cast");
        }
        let error = cast_bool(
            &script,
            &[
                "call",
                "0x1111111111111111111111111111111111111111",
                VaultInterfaceAbiClass::CamelCaseV2.deposit_seen_selector(),
            ],
            "selected camel-case getter",
        )
        .expect_err("selected getter revert must remain fatal");
        assert!(error.to_string().contains("selected camel-case getter"));
        assert!(!fallback_marker.exists());
        let _ = std::fs::remove_dir_all(root);
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
        let interface_lineage_manifest = write_test_interface_lineage(
            &root,
            &[(
                &route.vault_runtime_code_hash,
                "snake_case_v1",
                "live_verified",
            )],
        );
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
            vault_interface_lineage_manifest: interface_lineage_manifest,
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
        let report = vault_bridge_conservation_audit(options)
            .expect("audit must return the non-conserved report");
        let error = report
            .verify()
            .expect_err("one unexplained source atom must fail closed");
        assert!(
            error.to_string().contains("unexplained_delta=1"),
            "unexpected error: {error}"
        );
        let _ = std::fs::remove_dir_all(root);
    }
}
