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
const RPC_READ_CACHE_BUSY_WAIT: Duration = Duration::from_millis(250);

#[derive(Debug, Clone)]
struct RpcCachedReadEntry {
    height: u64,
    inserted_at: Instant,
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
    writer_busy_fallbacks: u64,
}

impl RpcServeReadCache {
    fn get_at_height(
        &mut self,
        height: u64,
        method: &str,
        canonical_params: &str,
        request_id: &str,
    ) -> Option<RpcResponse> {
        let key = (method.to_string(), canonical_params.to_string());
        if let Some(entry) = self.entries.get(&key) {
            if entry.height == height {
                self.hits = self.hits.saturating_add(1);
                let mut response = entry.response.clone();
                response.id = request_id.to_string();
                return Some(response);
            }
        }
        None
    }

    fn get_recent_fallback(
        &mut self,
        method: &str,
        canonical_params: &str,
        request_id: &str,
        max_age: Duration,
    ) -> Option<RpcResponse> {
        let key = (method.to_string(), canonical_params.to_string());
        if let Some(entry) = self.entries.get(&key) {
            if entry.inserted_at.elapsed() <= max_age {
                self.writer_busy_fallbacks = self.writer_busy_fallbacks.saturating_add(1);
                let mut response = entry.response.clone();
                response.id = request_id.to_string();
                return Some(response);
            }
        }
        None
    }

    fn has_recent_fallback(&self, method: &str, canonical_params: &str, max_age: Duration) -> bool {
        let key = (method.to_string(), canonical_params.to_string());
        self.entries
            .get(&key)
            .is_some_and(|entry| entry.inserted_at.elapsed() <= max_age)
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
                height,
                inserted_at: Instant::now(),
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
) -> Result<(), String> {
    if !profile.is_replica() {
        return Ok(());
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
    if profile == RpcServeProfile::PrivateReplica
        && (allow_mempool_submit || allow_private_action_admission)
    {
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
) {
    if !profile.is_replica() {
        return;
    }
    if let Some(obj) = value.as_object_mut() {
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

fn is_in_process_standard_read_method(request: &RpcRequest) -> bool {
    match request.method.as_str() {
        "metrics"
        | "account"
        | "account_lines"
        | "account_assets"
        | "account_tx"
        | "account_tx_index_status"
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

/// Serve a standard read-only RPC method in-process using the bounded
/// height-stamped response cache and a single shared `Arc<TransactionalStore>`
/// lease, returning `(response, cache_hit)`.
fn run_rpc_serve_in_process_read(
    request: &RpcRequest,
    context: &RpcServeConnectionContext,
) -> (RpcResponse, bool) {
    let canonical_params = postfiat_node::validator_uptime::canonical_json_bytes(&request.params)
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .unwrap_or_else(|| request.params.to_string());

    // Step 1: Read current committed height cheaply via the health cache.
    // For `receipts`, never block waiting on the transactional store lease when
    // the health cache is cold: `validator_receipt_reader::read` performs its own
    // 100ms direct-read probe and falls back to the validator's held-snapshot
    // abstract socket when a sibling validator holds the store lease.
    let is_receipts = request.method == "receipts";
    let height_before = if is_receipts {
        context.health_cache.lock().ok().and_then(|cache| {
            if cache
                .status_checked_at
                .is_some_and(|checked_at| checked_at.elapsed() <= RPC_SERVE_HEALTH_STAMP_MAX_AGE)
            {
                cache.status.as_ref().map(|report| report.block_height)
            } else {
                None
            }
        })
    } else {
        rpc_serve_cached_status(&context.data_dir, &context.health_cache)
            .ok()
            .map(|(report, _)| report.block_height)
    };

    // Step 2: Check height-stamped response cache (zero redb open / zero flock).
    if let Some(h) = height_before {
        if let Ok(mut cache) = context.read_cache.lock() {
            if let Some(hit) =
                cache.get_at_height(h, &request.method, &canonical_params, &request.id)
            {
                return (hit, true);
            }
            cache.record_miss();
        }
    }

    // Step 3: Cache miss -> execute in-process under ONE shared transactional store lease.
    // For `receipts`, delegate lease acquisition to `validator_receipt_reader::read`
    // so the held-snapshot protocol runs before waiting on a writer lease.
    let has_fallback = context.read_cache.lock().ok().is_some_and(|cache| {
        cache.has_recent_fallback(
            &request.method,
            &canonical_params,
            RPC_SERVE_STATUS_COMMIT_WINDOW_MAX_AGE,
        )
    });

    let run_read = || -> Result<(RpcResponse, Option<u64>), String> {
        if is_receipts {
            let response = execute_in_process_standard_read(request, &context.data_dir)?;
            let height_after = if height_before.is_some() {
                let store = postfiat_storage::NodeStore::new(&context.data_dir);
                postfiat_storage::transactional::with_writer_lease_deadline(
                    Duration::from_millis(50),
                    || store.read_chain_tip(),
                )
                .ok()
                .map(|tip| tip.height)
            } else {
                None
            };
            return Ok((response, height_after));
        }
        let store = postfiat_storage::NodeStore::new(&context.data_dir);
        let _lease = store
            .transactional_store_lease()
            .map_err(|error| format!("rpc {} store lease failed: {error}", request.method))?;
        let response = execute_in_process_standard_read(request, &context.data_dir)?;
        let height_after = store
            .read_chain_tip()
            .ok()
            .map(|tip| tip.height)
            .or(height_before);
        Ok((response, height_after))
    };

    let outcome = if has_fallback {
        postfiat_storage::transactional::with_writer_lease_deadline(
            RPC_READ_CACHE_BUSY_WAIT,
            run_read,
        )
    } else {
        run_read()
    };

    match outcome {
        Ok((response, height_after)) => {
            if response.ok {
                if let (Some(h_before), Some(h_after)) = (height_before, height_after) {
                    if h_before == h_after {
                        if let Ok(mut cache) = context.read_cache.lock() {
                            cache.insert(
                                h_after,
                                &request.method,
                                &canonical_params,
                                response.clone(),
                            );
                        }
                    }
                }
            }
            (response, false)
        }
        Err(error) => {
            if error.contains("storage_writer_busy") {
                if let Ok(mut cache) = context.read_cache.lock() {
                    if let Some(fallback) = cache.get_recent_fallback(
                        &request.method,
                        &canonical_params,
                        &request.id,
                        RPC_SERVE_STATUS_COMMIT_WINDOW_MAX_AGE,
                    ) {
                        return (fallback, true);
                    }
                }
            }
            let (code, message) = rpc_dispatch::rpc_dispatch_error_response_parts(&error);
            (rpc_serve_error_response(&request.id, code, message), false)
        }
    }
}

#[cfg(test)]
mod rpc_read_cache_tests {
    use super::*;

    fn replica_serve_options(
        root: &Path,
        port: u16,
        max_requests: usize,
        profile: RpcServeProfile,
    ) -> RpcServeOptions {
        RpcServeOptions {
            data_dir: root.to_path_buf(),
            spool_dir: root.join("runtime/rpc-spool"),
            ready_file: root.join(format!("readiness/rpc-replica-{port}.json")),
            bind_host: "127.0.0.1".to_string(),
            port,
            max_requests,
            timeout_ms: 5_000,
            child_timeout_ms: 5_000,
            event_log: None,
            allow_mempool_submit: false,
            allow_mempool_submit_finality: false,
            allow_orchard_batch_create: false,
            owned_lane_enabled: false,
            allow_private_action_admission: false,
            finality_topology_file: root.join("topology.json"),
            finality_key_file: root.join(VALIDATOR_KEYS_FILE),
            finality_proposal_key_file: None,
            finality_signer_socket: None,
            finality_artifact_root: root.join("finality-artifacts"),
            finality_timeout_ms: 2_000,
            finality_send_retries: 0,
            finality_retry_backoff_ms: 0,
            finality_vote_phase_deadline_ms: PEER_VOTE_PHASE_DEADLINE_MS,
            finality_quorum_early_full_propagation: false,
            max_mempool_submit_per_peer: 8,
            max_mempool_submit_total: 32,
            max_orchard_batch_create_per_peer: 2,
            max_orchard_batch_create_total: 8,
            max_orchard_batch_create_concurrent: 1,
            max_child_dispatch_concurrent: 8,
            max_child_dispatch_per_peer: 4,
            keep_alive: true,
            rpc_profile: profile,
            max_replica_lag_blocks: DEFAULT_MAX_REPLICA_LAG_BLOCKS,
        }
    }

    #[test]
    fn replica_profile_forbids_signer_and_public_bind() {
        let flags = vec![
            "--finality-key-file".to_string(),
            "/tmp/key.json".to_string(),
        ];
        let err = validate_rpc_replica_profile_flags(
            RpcServeProfile::PublicReplica,
            &flags,
            "127.0.0.1",
            false,
            false,
            false,
            false,
        )
        .expect_err("signer key file must be refused in replica profile");
        assert!(err.contains("rpc_replica_signer_forbidden"), "{err}");

        let socket_flags = vec![
            "--finality-signer-socket".to_string(),
            "/run/signer.sock".to_string(),
        ];
        let err = validate_rpc_replica_profile_flags(
            RpcServeProfile::PrivateReplica,
            &socket_flags,
            "127.0.0.1",
            false,
            false,
            false,
            false,
        )
        .expect_err("signer socket must be refused in replica profile");
        assert!(err.contains("rpc_replica_signer_forbidden"), "{err}");

        let err = validate_rpc_replica_profile_flags(
            RpcServeProfile::PublicReplica,
            &[],
            "0.0.0.0",
            false,
            false,
            false,
            false,
        )
        .expect_err("wildcard bind must be refused in replica profile");
        assert!(err.contains("rpc_replica_bind_forbidden"), "{err}");

        validate_rpc_replica_profile_flags(
            RpcServeProfile::PublicReplica,
            &[],
            "127.0.0.1",
            false,
            false,
            false,
            false,
        )
        .expect("valid loopback public replica");
    }

    #[test]
    fn height_stamped_read_cache_serves_concurrent_reads_under_250ms_and_matches_direct() {
        let root = env::temp_dir().join(format!(
            "postfiat-replica-read-cache-{}-{}",
            process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        init_consensus_v2(InitConsensusV2Options {
            data_dir: root.clone(),
            chain_id: "replica-cache-test".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 4,
            activation_height: 1,
            storage_activation_height: None,
        })
        .expect("init node");
        let faucet = faucet_key(NodeOptions {
            data_dir: root.clone(),
        })
        .expect("faucet key");

        // Baseline direct reads for differential equivalence.
        let direct_fee = rpc_dispatch::rpc_fee_alias(root.clone()).expect("direct fee");
        let direct_validators =
            rpc_dispatch::rpc_validators_alias(root.clone()).expect("direct validators");
        let direct_acct = rpc_dispatch::account_rpc_result(
            &account(
                NodeOptions {
                    data_dir: root.clone(),
                },
                &faucet.address,
            )
            .expect("direct account"),
        )
        .expect("direct account json");
        let direct_lines = serde_json::to_value(
            account_lines(AccountLinesOptions {
                data_dir: root.clone(),
                account: faucet.address.clone(),
                issuer: None,
                asset_id: None,
                limit: None,
            })
            .expect("direct account_lines"),
        )
        .expect("direct account_lines json");

        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind port");
        let port = listener.local_addr().expect("addr").port();
        let total_conns = 12_usize;
        let options =
            replica_serve_options(&root, port, total_conns, RpcServeProfile::PublicReplica);
        let ready_file = options.ready_file.clone();
        let server = std::thread::spawn(move || rpc_serve_inner(options, Some(listener)));
        for _ in 0..200 {
            if ready_file.is_file() {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        assert!(ready_file.is_file(), "replica server ready");

        // Fire 12 concurrent keep-alive clients, each issuing 5 mixed standard reads (60 reads total).
        let addr = faucet.address.clone();
        let handles: Vec<_> = (0..total_conns)
            .map(|worker_idx| {
                let addr = addr.clone();
                let direct_fee = direct_fee.clone();
                let direct_validators = direct_validators.clone();
                let direct_acct = direct_acct.clone();
                let direct_lines = direct_lines.clone();
                thread::spawn(move || {
                    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
                    stream
                        .set_read_timeout(Some(Duration::from_secs(5)))
                        .expect("timeout");
                    let mut reader = BufReader::new(stream.try_clone().expect("clone"));
                    let requests = vec![
                        RpcRequest::empty(format!("w{worker_idx}-status"), "status"),
                        RpcRequest::empty(format!("w{worker_idx}-fee"), "fee"),
                        RpcRequest::empty(format!("w{worker_idx}-validators"), "validators"),
                        RpcRequest::new(
                            format!("w{worker_idx}-account"),
                            "account",
                            serde_json::json!({ "address": addr }),
                        ),
                        RpcRequest::new(
                            format!("w{worker_idx}-lines"),
                            "account_lines",
                            serde_json::json!({ "account": addr }),
                        ),
                    ];
                    let mut latencies_ms = Vec::new();
                    for req in requests {
                        let started = Instant::now();
                        let mut line = serde_json::to_vec(&req).expect("serialize");
                        line.push(b'\n');
                        stream.write_all(&line).expect("write");
                        let mut reply_line = String::new();
                        reader.read_line(&mut reply_line).expect("read");
                        latencies_ms.push(started.elapsed().as_secs_f64() * 1_000.0);
                        let resp: RpcResponse = serde_json::from_str(&reply_line).expect("parse");
                        assert!(resp.ok, "{}: {:?}", req.method, resp.error);
                        let result = resp.result.expect("result");
                        match req.method.as_str() {
                            "status" => {
                                assert_eq!(result["node_role"], "rpc_follower");
                                assert_eq!(result["voting_enabled"], false);
                                assert_eq!(result["rpc_profile"], "public-replica");
                            }
                            "fee" => assert_eq!(result, direct_fee),
                            "validators" => assert_eq!(result, direct_validators),
                            "account" => assert_eq!(result, direct_acct),
                            "account_lines" => assert_eq!(result, direct_lines),
                            _ => {}
                        }
                    }
                    let _ = stream.shutdown(std::net::Shutdown::Write);
                    latencies_ms
                })
            })
            .collect();

        let mut all_latencies = Vec::new();
        for h in handles {
            all_latencies.extend(h.join().expect("worker"));
        }
        all_latencies.sort_by(|a, b| a.total_cmp(b));
        let p95 = all_latencies[(all_latencies.len() * 95).saturating_sub(1) / 100];
        assert!(
            p95 < 250.0,
            "concurrent read p95 latency {p95:.2}ms exceeded 250ms SLA"
        );

        let report = server.join().expect("server thread").expect("rpc serve");
        assert!(report.read_only);
        assert_eq!(report.ok_count, (total_conns * 5) as u64);
        assert!(
            report
                .requests
                .iter()
                .any(|ev| ev.handler_mode.as_deref() == Some("in_process_read_cache")),
            "expected height-stamped read cache hits in event log"
        );
        let _ = fs::remove_dir_all(root);
    }
}
