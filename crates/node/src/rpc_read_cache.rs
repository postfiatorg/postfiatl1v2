// Height-stamped in-process read cache, shared-lease standard read execution,
// and non-voting read-replica profile enforcement for `postfiat-node rpc-serve`.
//
// Eliminates the 7-10s devnet read-path contention caused by spawning a child
// process per read (each taking an exclusive redb flock + rewriting allocator
// state + 10x fdatasync on close). Standard read methods now run in-process
// under a single process-shared `Arc<TransactionalStore>` lease and a bounded
// height-stamped response cache (Contract C2 §5.2 and §6.1).

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum RpcServeProfile {
    #[default]
    Standard,
    PrivateReplica,
    PublicReplica,
}

impl RpcServeProfile {
    fn parse(raw: &str) -> Result<Self, String> {
        match raw {
            "standard" => Ok(Self::Standard),
            "private-replica" => Ok(Self::PrivateReplica),
            "public-replica" => Ok(Self::PublicReplica),
            other => Err(format!(
                "invalid --rpc-profile `{other}`; expected `standard`, `private-replica`, or `public-replica`"
            )),
        }
    }

    fn is_replica(self) -> bool {
        matches!(self, Self::PrivateReplica | Self::PublicReplica)
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::PrivateReplica => "private-replica",
            Self::PublicReplica => "public-replica",
        }
    }
}

const DEFAULT_MAX_REPLICA_LAG_BLOCKS: u64 = 5;
const RPC_READ_CACHE_MAX_ENTRIES: usize = 256;
const RPC_READ_CACHE_MAX_BYTES: usize = 16 * 1024 * 1024;
/// How old the source-tip record of the catch-up job may be before the replica
/// no longer knows how far behind it is.
const RPC_REPLICA_SOURCE_TIP_MAX_AGE_SECS: u64 = 120;
const RPC_REPLICA_SOURCE_TIP_FILE: &str = "readiness/replica-source-tip.json";

/// Methods a replica profile serves: exactly the `read_only_public` posture of
/// docs/runbooks/rpc-method-inventory.json. A test keeps the two equal.
const RPC_REPLICA_READ_METHODS: &[&str] = postfiat_node::rpc_http_edge::READ_ONLY_PUBLIC_METHODS;

fn rpc_replica_profile_serves(
    profile: RpcServeProfile,
    method: &str,
    allow_private_action_admission: bool,
) -> bool {
    if !profile.is_replica() {
        return true;
    }
    if is_private_action_method(method) {
        return profile == RpcServeProfile::PublicReplica && allow_private_action_admission;
    }
    RPC_REPLICA_READ_METHODS.binary_search(&method).is_ok()
}

/// Written by `rpc-catch-up --source-tip-file` after every run.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct RpcReplicaSourceTip {
    schema: String,
    source_height: u64,
    local_height: u64,
    checked_at_unix: u64,
}

const RPC_REPLICA_SOURCE_TIP_SCHEMA: &str = "postfiat-rpc-replica-source-tip-v1";

fn write_rpc_replica_source_tip(
    path: &Path,
    source_height: u64,
    local_height: u64,
) -> Result<(), String> {
    let record = RpcReplicaSourceTip {
        schema: RPC_REPLICA_SOURCE_TIP_SCHEMA.to_string(),
        source_height,
        local_height,
        checked_at_unix: unix_now_secs(),
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("replica source tip directory failed: {error}"))?;
    }
    let tmp = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec(&record)
        .map_err(|error| format!("replica source tip serialization failed: {error}"))?;
    fs::write(&tmp, bytes).map_err(|error| format!("replica source tip write failed: {error}"))?;
    fs::rename(&tmp, path).map_err(|error| format!("replica source tip rename failed: {error}"))
}

fn unix_now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

/// How far the replica is behind its source, or why that is not known.
#[derive(Debug, Clone, PartialEq, Eq)]
enum RpcReplicaLag {
    Within { lag_blocks: u64 },
    Lagging { lag_blocks: u64, local_height: u64 },
    Unknown { reason: String },
}

fn rpc_replica_lag(
    data_dir: &Path,
    local_height: u64,
    max_replica_lag_blocks: u64,
    now_unix: u64,
) -> RpcReplicaLag {
    let path = data_dir.join(RPC_REPLICA_SOURCE_TIP_FILE);
    let record = match fs::read(&path) {
        Ok(bytes) => match serde_json::from_slice::<RpcReplicaSourceTip>(&bytes) {
            Ok(record) if record.schema == RPC_REPLICA_SOURCE_TIP_SCHEMA => record,
            _ => {
                return RpcReplicaLag::Unknown {
                    reason: "the source tip record is not readable".to_string(),
                }
            }
        },
        Err(_) => {
            return RpcReplicaLag::Unknown {
                reason: "no source tip record yet; the catch-up job has not run".to_string(),
            }
        }
    };
    if record.checked_at_unix > now_unix || local_height > record.source_height {
        return RpcReplicaLag::Unknown { reason: "source height or timestamp inconsistent with local replica".into() };
    }
    let age = now_unix - record.checked_at_unix;
    if age > RPC_REPLICA_SOURCE_TIP_MAX_AGE_SECS {
        return RpcReplicaLag::Unknown {
            reason: format!("the source tip record is {age}s old; the catch-up job is not running"),
        };
    }
    let lag_blocks = record.source_height.saturating_sub(local_height);
    if lag_blocks > max_replica_lag_blocks {
        RpcReplicaLag::Lagging {
            lag_blocks,
            local_height,
        }
    } else {
        RpcReplicaLag::Within { lag_blocks }
    }
}

/// `Some(error response)` when a replica must not answer a read because it is
/// behind its source or cannot tell.
fn rpc_replica_lag_refusal(
    request_id: &str,
    context: &RpcServeConnectionContext,
) -> Option<RpcResponse> {
    if !context.rpc_profile.is_replica() {
        return None;
    }
    let local_height = match postfiat_storage::NodeStore::new(&context.data_dir).read_chain_tip() {
        Ok(tip) => tip.height,
        Err(_) => return Some(rpc_serve_error_response(request_id, "replica_lagging", "cannot read local replica tip")),
    };
    match rpc_replica_lag(
        &context.data_dir,
        local_height,
        context.max_replica_lag_blocks,
        unix_now_secs(),
    ) {
        RpcReplicaLag::Within { .. } => None,
        RpcReplicaLag::Lagging {
            lag_blocks,
            local_height,
        } => Some(rpc_serve_error_response(
            request_id,
            "replica_lagging",
            &format!(
                "replica is {lag_blocks} blocks behind its source at height {local_height}; limit {}",
                context.max_replica_lag_blocks
            ),
        )),
        RpcReplicaLag::Unknown { reason } => Some(rpc_serve_error_response(
            request_id,
            "replica_lagging",
            &format!("replica lag is unknown: {reason}"),
        )),
    }
}

#[derive(Debug, Clone)]
struct RpcCachedReadEntry {
    inserted_at: std::time::Instant,
    height: u64,
    approx_bytes: usize,
    response: RpcResponse,
}

#[derive(Debug, Default)]
struct RpcServeReadCache {
    entries: BTreeMap<(String, String), RpcCachedReadEntry>,
    order: std::collections::VecDeque<(String, String)>,
    total_bytes: usize,
    hits: u64,
    misses: u64,
}

impl RpcServeReadCache {
    /// A stored answer is served only for the committed height it was read at.
    fn get_at_height(
        &mut self,
        height: u64,
        method: &str,
        canonical_params: &str,
        request_id: &str,
    ) -> Option<RpcResponse> {
        let key = (method.to_string(), canonical_params.to_string());
        if let Some(entry) = self.entries.get(&key) {
            if entry.height == height && entry.inserted_at.elapsed() < Duration::from_secs(2) {
                self.hits = self.hits.saturating_add(1);
                let mut response = entry.response.clone();
                response.id = request_id.to_string();
                // Every cached answer names the committed height it was read at.
                response.events.push(RpcEvent::new(
                    "read_cache",
                    height.to_string(),
                    "served from the read cache at this committed height",
                ));
                return Some(response);
            }
        }
        None
    }

    fn insert(&mut self, height: u64, method: &str, canonical_params: &str, response: RpcResponse) {
        let approx_bytes = serde_json::to_vec(&response).map_or(0, |v| v.len());
        if approx_bytes == 0 || approx_bytes > RPC_READ_CACHE_MAX_BYTES / 2 {
            return;
        }
        let key = (method.to_string(), canonical_params.to_string());
        if let Some(prev) = self.entries.remove(&key) {
            self.total_bytes = self.total_bytes.saturating_sub(prev.approx_bytes);
        }
        while (self.entries.len() >= RPC_READ_CACHE_MAX_ENTRIES
            || self.total_bytes.saturating_add(approx_bytes) > RPC_READ_CACHE_MAX_BYTES)
            && !self.order.is_empty()
        {
            if let Some(oldest) = self.order.pop_front() {
                if let Some(evicted) = self.entries.remove(&oldest) {
                    self.total_bytes = self.total_bytes.saturating_sub(evicted.approx_bytes);
                }
            }
        }
        self.total_bytes = self.total_bytes.saturating_add(approx_bytes);
        self.order.push_back(key.clone());
        self.entries.insert(
            key,
            RpcCachedReadEntry {
                inserted_at: std::time::Instant::now(),
                height,
                approx_bytes,
                response,
            },
        );
    }

    fn record_miss(&mut self) {
        self.misses = self.misses.saturating_add(1);
    }
}

/// Enforce Contract C2 §5.2 non-voting read-replica invariants at startup.
fn validate_rpc_replica_profile_flags(
    profile: RpcServeProfile,
    flags: &[String],
    bind_host: &str,
    allow_mempool_submit: bool,
    allow_mempool_submit_finality: bool,
    allow_orchard_batch_create: bool,
    allow_private_action_admission: bool,
    data_dir: &Path,
) -> Result<(), String> {
    if !profile.is_replica() {
        return Ok(());
    }
    // Non-voting is a fact about the process, not about its flags: without a
    // flag the server would load the default key file of the data directory.
    if data_dir.join(VALIDATOR_KEYS_FILE).exists() {
        return Err(format!(
            "rpc_replica_key_material_present: read-replica profiles refuse a data directory that holds `{VALIDATOR_KEYS_FILE}`"
        ));
    }
    if flag_present(flags, "--finality-key-file")
        || flag_present(flags, "--finality-proposal-key-file")
        || flag_present(flags, "--finality-signer-socket")
    {
        return Err(
            "rpc_replica_signer_forbidden: read-replica profiles are strictly non-voting and forbid --finality-key-file, --finality-proposal-key-file, and --finality-signer-socket"
                .to_string(),
        );
    }
    if allow_mempool_submit_finality || allow_orchard_batch_create {
        return Err(
            "rpc_replica_write_forbidden: read-replica profiles forbid finality and batch-creation write flags"
                .to_string(),
        );
    }
    if allow_mempool_submit {
        return Err(
            "rpc_replica_write_forbidden: read-replica profiles order nothing and forbid --allow-mempool-submit"
                .to_string(),
        );
    }
    if profile == RpcServeProfile::PrivateReplica && allow_private_action_admission {
        return Err(
            "rpc_replica_write_forbidden: private-replica profile is strictly read-only"
                .to_string(),
        );
    }
    if !postfiat_node::validator_top::is_loopback_host(bind_host) {
        return Err(format!(
            "rpc_replica_bind_forbidden: read-replica profile requires loopback bind host (got `{bind_host}`)"
        ));
    }
    Ok(())
}

fn merge_rpc_replica_status_fields(
    value: &mut serde_json::Value,
    profile: RpcServeProfile,
    max_replica_lag_blocks: u64,
    data_dir: &Path,
) {
    if !profile.is_replica() {
        return;
    }
    let local_height = value
        .get("block_height")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let lag = rpc_replica_lag(data_dir, local_height, max_replica_lag_blocks, unix_now_secs());
    if let Some(obj) = value.as_object_mut() {
        let (ready, lag_blocks, reason) = match lag {
            RpcReplicaLag::Within { lag_blocks } => (true, Some(lag_blocks), None),
            RpcReplicaLag::Lagging { lag_blocks, .. } => {
                (false, Some(lag_blocks), Some("replica_lagging".to_string()))
            }
            RpcReplicaLag::Unknown { reason } => (false, None, Some(reason)),
        };
        obj.insert("replica_ready".to_string(), serde_json::json!(ready));
        obj.insert("replica_lag_blocks".to_string(), serde_json::json!(lag_blocks));
        obj.insert("replica_unready_reason".to_string(), serde_json::json!(reason));
        obj.insert("node_role".to_string(), serde_json::json!("rpc_follower"));
        obj.insert("voting_enabled".to_string(), serde_json::json!(false));
        obj.insert(
            "rpc_profile".to_string(),
            serde_json::json!(profile.as_str()),
        );
        obj.insert(
            "max_replica_lag_blocks".to_string(),
            serde_json::json!(max_replica_lag_blocks),
        );
    }
}

/// A validator (the standard profile) keeps every read on the worker path, so
/// shipping the in-process path changes nothing for it.
fn rpc_serve_reads_in_process(profile: RpcServeProfile, request: &RpcRequest) -> bool {
    profile.is_replica() && is_in_process_standard_read_method(request)
}

fn is_in_process_standard_read_method(request: &RpcRequest) -> bool {
    match request.method.as_str() {
        "metrics"
        | "account"
        | "account_lines"
        | "account_assets"
        | "account_tx"
        | "account_tx_index_status"
        | "block_vote_records"
        | "validators"
        | "manifests"
        | "fee"
        | "blocks"
        | "receipts"
        | "vault_bridge_route"
        | "vault_bridge_status"
        | "bridge_status"
        | "ledger" => true,
        // `tx` with `audit_block_log: true` is an explicit full-history audit;
        // keep it isolated on the bounded child path.
        "tx" => !request
            .params
            .get("audit_block_log")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        _ => false,
    }
}

/// Health methods answer even when the replica is behind, so the edge and the
/// operator can see why it is unready. Every other read is refused.
fn rpc_replica_lag_refusal_for(
    method: &str,
    request_id: &str,
    context: &RpcServeConnectionContext,
) -> Option<RpcResponse> {
    if matches!(method, "status" | "server_info" | "private_readiness") {
        return None;
    }
    rpc_replica_lag_refusal(request_id, context)
}

/// Cache only queries derived solely from committed ledger state.
fn is_height_pure_read(request: &RpcRequest) -> bool {
    matches!(request.method.as_str(), "validators" | "manifests" | "vault_bridge_route" | "blocks")
        && is_in_process_standard_read_method(request)
}

fn rpc_param_str<'a>(
    params: &'a serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Result<&'a str, String> {
    params
        .get(key)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("missing --{}", key.replace('_', "-")))
}

fn rpc_param_opt_str<'a>(
    params: &'a serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Result<Option<&'a str>, String> {
    match params.get(key) {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::String(s)) => Ok(Some(s.as_str())),
        Some(_) => Err(format!("--{} must be a string", key.replace('_', "-"))),
    }
}

fn rpc_param_opt_usize(
    params: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Result<Option<usize>, String> {
    match params.get(key) {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::Number(n)) => n
            .as_u64()
            .and_then(|v| usize::try_from(v).ok())
            .map(Some)
            .ok_or_else(|| format!("--{} must be a usize", key.replace('_', "-"))),
        Some(serde_json::Value::String(s)) => s
            .parse::<usize>()
            .map(Some)
            .map_err(|_| format!("--{} must be a usize", key.replace('_', "-"))),
        Some(_) => Err(format!("--{} must be a usize", key.replace('_', "-"))),
    }
}

fn rpc_param_opt_u64(
    params: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Result<Option<u64>, String> {
    match params.get(key) {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::Number(n)) => n
            .as_u64()
            .map(Some)
            .ok_or_else(|| format!("--{} must be a u64", key.replace('_', "-"))),
        Some(serde_json::Value::String(s)) => s
            .parse::<u64>()
            .map(Some)
            .map_err(|_| format!("--{} must be a u64", key.replace('_', "-"))),
        Some(_) => Err(format!("--{} must be a u64", key.replace('_', "-"))),
    }
}

fn execute_in_process_standard_read(
    request: &RpcRequest,
    data_dir: &Path,
) -> Result<RpcResponse, String> {
    let params = request
        .params
        .as_object()
        .ok_or_else(|| "rpc request params must be an object".to_string())?;
    let id = &request.id;
    let dir = data_dir.to_path_buf();
    let wrap = |value: serde_json::Value, events: Vec<RpcEvent>| {
        success_response(id, &value, events)
            .map_err(|error| format!("rpc {} serialization failed: {error}", request.method))
    };

    match request.method.as_str() {
        "metrics" => {
            let report = metrics(NodeOptions { data_dir: dir })
                .map_err(|e| format!("rpc metrics failed: {e}"))?;
            let target = report.node_id.clone();
            let value = serde_json::to_value(&report)
                .map_err(|e| format!("rpc metrics serialization failed: {e}"))?;
            wrap(
                value,
                vec![RpcEvent::new("metrics", target, "metrics queried")],
            )
        }
        "account" => {
            let address = rpc_param_str(params, "address")?;
            let acct = account(NodeOptions { data_dir: dir }, address)
                .map_err(|e| format!("rpc account failed: {e}"))?;
            let target = acct.address.clone();
            let value = rpc_dispatch::account_rpc_result(&acct)?;
            wrap(
                value,
                vec![RpcEvent::new("account", target, "account queried")],
            )
        }
        "account_lines" => {
            let account_addr = rpc_param_str(params, "account")?.to_string();
            let report = account_lines(AccountLinesOptions {
                data_dir: dir,
                account: account_addr,
                issuer: rpc_param_opt_str(params, "issuer")?.map(str::to_string),
                asset_id: rpc_param_opt_str(params, "asset_id")?.map(str::to_string),
                limit: rpc_param_opt_usize(params, "limit")?,
            })
            .map_err(|e| format!("rpc account_lines failed: {e}"))?;
            let target = report.account.clone();
            let value = serde_json::to_value(&report)
                .map_err(|e| format!("rpc account_lines serialization failed: {e}"))?;
            wrap(
                value,
                vec![RpcEvent::new(
                    "account_lines",
                    target,
                    "account trustlines queried",
                )],
            )
        }
        "account_assets" => {
            let account_addr = rpc_param_str(params, "account")?.to_string();
            let report = account_assets(AccountAssetsOptions {
                data_dir: dir,
                account: account_addr,
                asset_id: rpc_param_opt_str(params, "asset_id")?.map(str::to_string),
                limit: rpc_param_opt_usize(params, "limit")?,
            })
            .map_err(|e| format!("rpc account_assets failed: {e}"))?;
            let target = report.account.clone();
            let value = serde_json::to_value(&report)
                .map_err(|e| format!("rpc account_assets serialization failed: {e}"))?;
            wrap(
                value,
                vec![RpcEvent::new(
                    "account_assets",
                    target,
                    "account issued assets queried",
                )],
            )
        }
        "account_tx" => {
            let address = rpc_param_str(params, "address")?.to_string();
            let report = account_tx(AccountTxQueryOptions {
                data_dir: dir,
                address,
                from_height: rpc_param_opt_u64(params, "from_height")?,
                to_height: rpc_param_opt_u64(params, "to_height")?,
                limit: rpc_param_opt_usize(params, "limit")?,
            })
            .map_err(|e| format!("rpc account_tx failed: {e}"))?;
            let target = report.address.clone();
            let value = serde_json::to_value(&report)
                .map_err(|e| format!("rpc account_tx serialization failed: {e}"))?;
            wrap(
                value,
                vec![RpcEvent::new(
                    "account_tx",
                    target,
                    "account transaction history queried",
                )],
            )
        }
        "account_tx_index_status" => {
            let report = account_tx_index_status(AccountTxIndexOptions { data_dir: dir })
                .map_err(|e| format!("rpc account_tx_index_status failed: {e}"))?;
            let target = if report.index_usable {
                "usable"
            } else {
                "unusable"
            };
            let value = serde_json::to_value(&report)
                .map_err(|e| format!("rpc account_tx_index_status serialization failed: {e}"))?;
            wrap(
                value,
                vec![RpcEvent::new(
                    "account_tx_index_status",
                    target,
                    "account transaction index status queried",
                )],
            )
        }
        "block_vote_records" => {
            let from_height = rpc_param_opt_u64(params, "from_height")?
                .ok_or_else(|| "missing --from-height".to_string())?;
            let report = rpc_dispatch::rpc_block_vote_records(
                &dir,
                from_height,
                rpc_param_opt_usize(params, "limit")?,
            )?;
            wrap(
                report,
                vec![RpcEvent::new(
                    "block_vote_records",
                    from_height.to_string(),
                    "block vote records queried",
                )],
            )
        }
        "fee" => {
            let report = rpc_dispatch::rpc_fee_alias(dir)?;
            wrap(
                report,
                vec![RpcEvent::new("fee", "policy", "fee policy queried")],
            )
        }
        "ledger" => {
            let limit = rpc_param_opt_usize(params, "limit")?;
            let report = rpc_dispatch::rpc_ledger_alias(dir, limit)?;
            let target = report
                .get("ledger_index")
                .and_then(serde_json::Value::as_u64)
                .map(|h| h.to_string())
                .unwrap_or_else(|| "unknown".to_string());
            wrap(
                report,
                vec![RpcEvent::new("ledger", target, "ledger queried")],
            )
        }
        "validators" => {
            let report = rpc_dispatch::rpc_validators_alias(dir)?;
            let target = report
                .get("validator_count")
                .and_then(serde_json::Value::as_u64)
                .map(|c| c.to_string())
                .unwrap_or_else(|| "unknown".to_string());
            wrap(
                report,
                vec![RpcEvent::new("validators", target, "validators queried")],
            )
        }
        "manifests" => {
            let report = rpc_dispatch::rpc_manifests_alias(dir)?;
            let target = report
                .get("manifest_count")
                .and_then(serde_json::Value::as_u64)
                .map(|c| c.to_string())
                .unwrap_or_else(|| "unknown".to_string());
            wrap(
                report,
                vec![RpcEvent::new(
                    "manifests",
                    target,
                    "operator manifests queried",
                )],
            )
        }
        "blocks" => {
            let block_log = blocks(BlockQueryOptions {
                data_dir: dir,
                from_height: rpc_param_opt_u64(params, "from_height")?,
                limit: rpc_param_opt_usize(params, "limit")?,
            })
            .map_err(|e| format!("rpc blocks failed: {e}"))?;
            let target = block_log
                .last()
                .map(|b| b.header.height.to_string())
                .unwrap_or_else(|| "empty".to_string());
            let value = serde_json::to_value(&block_log)
                .map_err(|e| format!("rpc blocks serialization failed: {e}"))?;
            wrap(
                value,
                vec![RpcEvent::new("blocks", target, "blocks queried")],
            )
        }
        "receipts" => {
            let tx_id = rpc_param_opt_str(params, "tx_id")?.map(str::to_string);
            let limit = rpc_param_opt_usize(params, "limit")?;
            let target = tx_id.clone().unwrap_or_else(|| "latest".to_string());
            let options = ReceiptQueryOptions {
                data_dir: dir,
                tx_id,
                limit,
            };
            #[cfg(target_os = "linux")]
            let read = postfiat_node::validator_receipt_reader::read(options);
            #[cfg(not(target_os = "linux"))]
            let read = receipts(options).map(|rows| (rows, None));
            let (receipt_log, snapshot) = read.map_err(|e| format!("rpc receipts failed: {e}"))?;
            let value = serde_json::to_value(&receipt_log)
                .map_err(|e| format!("rpc receipts serialization failed: {e}"))?;
            wrap(
                value,
                vec![RpcEvent::new(
                    "receipts",
                    target,
                    snapshot.unwrap_or_else(|| "receipts queried".into()),
                )],
            )
        }
        "tx" => {
            let tx_id = rpc_param_str(params, "tx_id")?.to_string();
            let report = tx_finality(TxFinalityQueryOptions {
                data_dir: dir,
                tx_id,
                audit_block_log: false,
            })
            .map_err(rpc_dispatch::rpc_tx_finality_error)?;
            let target = report.tx_id.clone();
            let value = serde_json::to_value(&report)
                .map_err(|e| format!("rpc tx serialization failed: {e}"))?;
            wrap(
                value,
                vec![RpcEvent::new(
                    "tx",
                    target,
                    "transaction finality proof queried",
                )],
            )
        }
        "vault_bridge_route" => {
            let asset_id = rpc_param_str(params, "asset_id")?.to_string();
            let report = vault_bridge_route(VaultBridgeRouteOptions {
                data_dir: dir,
                asset_id,
            })
            .map_err(|e| format!("rpc vault_bridge_route failed: {e}"))?;
            let target = report.profile.route_id.clone();
            let value = serde_json::to_value(&report)
                .map_err(|e| format!("rpc vault_bridge_route serialization failed: {e}"))?;
            wrap(
                value,
                vec![RpcEvent::new(
                    "vault_bridge_route",
                    target,
                    "governed vault bridge route verified",
                )],
            )
        }
        "vault_bridge_status" => {
            let asset_id = rpc_param_str(params, "asset_id")?.to_string();
            let report = vault_bridge_status(VaultBridgeStatusOptions {
                data_dir: dir,
                asset_id,
            })
            .map_err(|e| format!("rpc vault_bridge_status failed: {e}"))?;
            let target = report.asset_id.clone();
            let value = serde_json::to_value(&report)
                .map_err(|e| format!("rpc vault_bridge_status serialization failed: {e}"))?;
            wrap(
                value,
                vec![RpcEvent::new(
                    "vault_bridge_status",
                    target,
                    "vault bridge status queried",
                )],
            )
        }
        "bridge_status" => {
            let bridge = bridge_state(NodeOptions { data_dir: dir })
                .map_err(|e| format!("rpc bridge_status failed: {e}"))?;
            let value = serde_json::to_value(&bridge)
                .map_err(|e| format!("rpc bridge_status serialization failed: {e}"))?;
            wrap(
                value,
                vec![RpcEvent::new(
                    "bridge_status",
                    "bridge",
                    "bridge state queried",
                )],
            )
        }
        other => Err(format!("unsupported in-process read method `{other}`")),
    }
}

/// Serve a read-only RPC method inside a replica's `rpc-serve`, returning
/// `(response, cache_hit)`. Only replica profiles come here; a validator keeps
/// the worker path. The same limits as the worker path apply: the global and
/// per-caller concurrency caps, and the request time limit on the store lease.
fn run_rpc_serve_in_process_read(
    request: &RpcRequest,
    context: &RpcServeConnectionContext,
) -> (RpcResponse, bool) {
    let _dispatch_guard = match try_acquire_rpc_serve_child_dispatch(context) {
        Ok(guard) => guard,
        Err(error) => {
            return (
                rpc_serve_error_response(&request.id, "rpc_rate_limited", &error),
                false,
            )
        }
    };
    let cacheable = is_height_pure_read(request);
    let canonical_params = postfiat_node::validator_uptime::canonical_json_bytes(&request.params)
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .unwrap_or_else(|| request.params.to_string());

    let height_before = if cacheable {
        postfiat_storage::NodeStore::new(&context.data_dir).read_chain_tip().ok().map(|tip| tip.height)
    } else {
        None
    };
    if let Some(height) = height_before {
        if let Ok(mut cache) = context.read_cache.lock() {
            if let Some(hit) =
                cache.get_at_height(height, &request.method, &canonical_params, &request.id)
            {
                return (hit, true);
            }
            cache.record_miss();
        }
    }

    let is_receipts = request.method == "receipts";
    let run_read = || -> Result<(RpcResponse, Option<u64>), String> {
        if is_receipts {
            // `validator_receipt_reader::read` takes its own lease.
            let response = execute_in_process_standard_read(request, &context.data_dir)?;
            return Ok((response, None));
        }
        let store = postfiat_storage::NodeStore::new(&context.data_dir);
        let _lease = store
            .transactional_store_lease()
            .map_err(|error| format!("rpc {} store lease failed: {error}", request.method))?;
        let response = execute_in_process_standard_read(request, &context.data_dir)?;
        let height_after = store.read_chain_tip().ok().map(|tip| tip.height);
        Ok((response, height_after))
    };
    let outcome = postfiat_storage::transactional::with_writer_lease_deadline(
        Duration::from_millis(context.child_timeout_ms),
        run_read,
    );

    match outcome {
        Ok((response, height_after)) => {
            if cacheable && response.ok && height_before.is_some() && height_before == height_after {
                if let (Some(height), Ok(mut cache)) = (height_after, context.read_cache.lock()) {
                    cache.insert(height, &request.method, &canonical_params, response.clone());
                }
            }
            (response, false)
        }
        Err(error) => {
            let (code, message) = rpc_dispatch::rpc_dispatch_error_response_parts(&error);
            (rpc_serve_error_response(&request.id, code, message), false)
        }
    }
}

#[cfg(test)]
mod rpc_read_cache_tests {
    use super::*;

    fn check(
        profile: RpcServeProfile,
        flags: &[&str],
        bind_host: &str,
        allow_mempool_submit: bool,
        allow_private_action_admission: bool,
        data_dir: &Path,
    ) -> Result<(), String> {
        let flags: Vec<String> = flags.iter().map(|flag| (*flag).to_string()).collect();
        validate_rpc_replica_profile_flags(
            profile,
            &flags,
            bind_host,
            allow_mempool_submit,
            false,
            false,
            allow_private_action_admission,
            data_dir,
        )
    }

    fn temp_dir(label: &str) -> PathBuf {
        let dir = env::temp_dir().join(format!(
            "postfiat-replica-{label}-{}-{}",
            process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    #[test]
    fn replica_profile_is_non_voting_by_fact_not_by_flag() {
        let empty = temp_dir("empty");
        let public = RpcServeProfile::PublicReplica;
        let private = RpcServeProfile::PrivateReplica;

        for flag in [
            "--finality-key-file",
            "--finality-proposal-key-file",
            "--finality-signer-socket",
        ] {
            let err = check(public, &[flag, "/tmp/x"], "127.0.0.1", false, false, &empty)
                .expect_err("signer flag must be refused");
            assert!(err.contains("rpc_replica_signer_forbidden"), "{err}");
        }
        let err = check(public, &[], "0.0.0.0", false, false, &empty).expect_err("wildcard bind");
        assert!(err.contains("rpc_replica_bind_forbidden"), "{err}");
        // A replica orders nothing: no profile accepts transaction submission.
        for profile in [public, private] {
            let err = check(profile, &[], "127.0.0.1", true, false, &empty)
                .expect_err("mempool submit must be refused");
            assert!(err.contains("rpc_replica_write_forbidden"), "{err}");
        }
        let err = check(private, &[], "127.0.0.1", false, true, &empty)
            .expect_err("private replica admits nothing");
        assert!(err.contains("rpc_replica_write_forbidden"), "{err}");
        check(public, &[], "127.0.0.1", false, true, &empty).expect("public replica with admission");
        check(private, &[], "127.0.0.1", false, false, &empty).expect("private replica");

        // No flag is given, but the default key file is there: still refused.
        let with_key = temp_dir("with-key");
        fs::write(with_key.join(VALIDATOR_KEYS_FILE), b"{}").expect("key file");
        let err = check(public, &[], "127.0.0.1", false, false, &with_key)
            .expect_err("key material in the data directory must be refused");
        assert!(err.contains("rpc_replica_key_material_present"), "{err}");
        // The standard profile is untouched by all of this.
        check(RpcServeProfile::Standard, &["--finality-key-file", "/tmp/x"], "0.0.0.0", true, true, &with_key)
            .expect("standard profile is not a replica");
        let _ = fs::remove_dir_all(empty);
        let _ = fs::remove_dir_all(with_key);
    }

    #[test]
    fn replica_method_list_is_the_inventory_read_only_posture() {
        let inventory_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/runbooks/rpc-method-inventory.json");
        let inventory: serde_json::Value =
            serde_json::from_slice(&fs::read(&inventory_path).expect("inventory file"))
                .expect("inventory json");
        let mut expected: Vec<String> = inventory["methods"]
            .as_array()
            .expect("methods")
            .iter()
            .filter(|method| method["posture"] == "read_only_public")
            .map(|method| method["method"].as_str().expect("method name").to_string())
            .collect();
        expected.sort();
        let listed: Vec<String> = RPC_REPLICA_READ_METHODS.iter().map(|m| (*m).to_string()).collect();
        assert_eq!(listed, expected, "RPC_REPLICA_READ_METHODS must equal the inventory, sorted");

        let public = RpcServeProfile::PublicReplica;
        let private = RpcServeProfile::PrivateReplica;
        for method in inventory["methods"].as_array().expect("methods") {
            let name = method["method"].as_str().expect("method name");
            let posture = method["posture"].as_str().expect("posture");
            let read = posture == "read_only_public";
            assert_eq!(rpc_replica_profile_serves(private, name, false), read, "{name}");
            assert_eq!(rpc_replica_profile_serves(public, name, false), read, "{name}");
            if posture != "read_only_public" && posture != "private_action_gated" {
                assert!(!rpc_replica_profile_serves(public, name, true), "{name} ({posture})");
            }
            assert!(rpc_replica_profile_serves(RpcServeProfile::Standard, name, false));
        }
        assert!(rpc_replica_profile_serves(public, "private_action_submit", true));
        assert!(!rpc_replica_profile_serves(private, "private_action_submit", true));
        assert!(!rpc_replica_profile_serves(public, "a_method_nobody_listed", true));
    }

    #[test]
    fn replica_lag_is_enforced_and_unknown_is_not_ready() {
        let dir = temp_dir("lag");
        let now = 1_800_000_000;
        let tip = dir.join(RPC_REPLICA_SOURCE_TIP_FILE);
        let write = |source_height: u64, checked_at_unix: u64| {
            fs::create_dir_all(tip.parent().expect("parent")).expect("dir");
            fs::write(
                &tip,
                serde_json::to_vec(&RpcReplicaSourceTip {
                    schema: RPC_REPLICA_SOURCE_TIP_SCHEMA.to_string(),
                    source_height,
                    local_height: 100,
                    checked_at_unix,
                })
                .expect("json"),
            )
            .expect("write");
        };
        assert!(matches!(rpc_replica_lag(&dir, 100, 5, now), RpcReplicaLag::Unknown { .. }));
        write(105, now);
        assert_eq!(rpc_replica_lag(&dir, 100, 5, now), RpcReplicaLag::Within { lag_blocks: 5 });
        write(106, now);
        assert_eq!(
            rpc_replica_lag(&dir, 100, 5, now),
            RpcReplicaLag::Lagging { lag_blocks: 6, local_height: 100 }
        );
        // A replica ahead of the last record is not behind.
        assert!(matches!(rpc_replica_lag(&dir, 200, 5, now), RpcReplicaLag::Unknown { .. }));
        // The catch-up job stopped: the replica no longer knows.
        write(100, now - RPC_REPLICA_SOURCE_TIP_MAX_AGE_SECS - 1);
        assert!(matches!(rpc_replica_lag(&dir, 100, 5, now), RpcReplicaLag::Unknown { .. }));
        fs::write(&tip, b"not json").expect("write");
        assert!(matches!(rpc_replica_lag(&dir, 100, 5, now), RpcReplicaLag::Unknown { .. }));
        // The record `rpc-catch-up --source-tip-file` writes is the one read here.
        write_rpc_replica_source_tip(&tip, 103, 100).expect("write record");
        assert_eq!(
            rpc_replica_lag(&dir, 100, 5, unix_now_secs()),
            RpcReplicaLag::Within { lag_blocks: 3 }
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn cache_serves_a_read_only_at_the_height_it_was_read_at() {
        let request = |method: &str, params: serde_json::Value| RpcRequest::new("id", method, params);
        for method in [
            "validators", "manifests", "vault_bridge_route", "blocks",
        ] {
            assert!(is_height_pure_read(&request(method, serde_json::json!({}))), "{method}");
        }
        for method in ["block_vote_records", "account_tx_index_status", "account_tx", "bridge_status"] {
            assert!(!is_height_pure_read(&request(method, serde_json::json!({}))), "{method}");
        }
        // The clock and the server's own counters; a read with its own lease; a full-history audit;
        // and anything that is not a read of this path.
        assert!(!is_height_pure_read(&request("metrics", serde_json::json!({}))));
        assert!(!is_height_pure_read(&request("receipts", serde_json::json!({}))));
        assert!(!is_height_pure_read(&request("tx", serde_json::json!({"audit_block_log": true}))));
        assert!(!is_height_pure_read(&request("mempool_submit_signed_payment_v2", serde_json::json!({}))));

        let mut cache = RpcServeReadCache::default();
        let response = success_response("first", &serde_json::json!({"v": 1}), Vec::new()).expect("response");
        cache.insert(7, "validators", "{}", response);
        let hit = cache.get_at_height(7, "validators", "{}", "second").expect("hit at the same height");
        assert_eq!(hit.id, "second");
        // The cached answer names the height it was read at.
        let event = hit.events.last().expect("cache event");
        assert_eq!((event.event_type.as_str(), event.subject.as_str()), ("read_cache", "7"));
        assert!(cache.get_at_height(8, "validators", "{}", "x").is_none(), "the height moved");
        assert!(cache.get_at_height(6, "validators", "{}", "x").is_none(), "an earlier height");
        assert!(cache.get_at_height(7, "validators", "{\"a\":1}", "x").is_none(), "other params");
        cache.entries.get_mut(&("validators".into(), "{}".into())).unwrap().inserted_at -= Duration::from_secs(3);
        assert!(cache.get_at_height(7, "validators", "{}", "expired").is_none());
    }

    #[test]
    fn only_a_replica_profile_reads_in_process() {
        let request = RpcRequest::new("id", "account", serde_json::json!({"address": "x"}));
        assert!(is_in_process_standard_read_method(&request));
        assert!(!rpc_serve_reads_in_process(RpcServeProfile::Standard, &request));
        assert!(rpc_serve_reads_in_process(RpcServeProfile::PublicReplica, &request));
        assert!(rpc_serve_reads_in_process(RpcServeProfile::PrivateReplica, &request));
        let submit = RpcRequest::new("id", "mempool_submit_signed_payment_v2", serde_json::json!({}));
        assert!(!rpc_serve_reads_in_process(RpcServeProfile::PublicReplica, &submit));
    }
}
