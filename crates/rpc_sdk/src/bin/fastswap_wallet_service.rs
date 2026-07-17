use postfiat_crypto_provider::address_from_public_key;
use postfiat_rpc_sdk::{
    derive_wallet_key_pair, drive_fastswap_three_wave, fastswap_effects_request,
    fastswap_objects_request, fastswap_policy_by_hash_request, fastswap_status_request,
    preview_fastswap, reconcile_fastswap_replication, status_request,
    wallet_dual_sign_fastswap_intent, FastSwapWalletSessionV1, RpcRequest, SwapSettlementModeV1,
    TcpFastSwapTransportV1, WalletBackupFile,
};
use postfiat_types::{
    FastAssetObjectV1, FastSwapCommitteeV1, FastSwapEffectsResponseV1, FastSwapIntentV1,
    FastSwapLocalStatusV1, FastSwapObjectsResponseV1, FastSwapPolicyResponseV1,
    FastSwapStatusResponseV1,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha3::{Digest, Sha3_384};
use std::collections::BTreeMap;
use std::env;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const MAX_HTTP_REQUEST_BYTES: usize = 64 * 1024;
const MAX_RAW_RPC_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const DEVNET_FAUCET_AMOUNT_ATOMS: u64 = 5_000;
const MAX_DEVNET_FAUCET_CLAIMS: usize = 100;

fn flag(args: &[String], name: &str) -> Result<String, String> {
    let index = args
        .iter()
        .position(|value| value == name)
        .ok_or_else(|| format!("missing {name}"))?;
    args.get(index + 1)
        .cloned()
        .ok_or_else(|| format!("missing value for {name}"))
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    let bytes = fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|error| format!("{}: {error}", path.display()))
}

fn persist_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let parent = path.parent().ok_or("output path has no parent")?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let temporary = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    let mut file = File::create(&temporary).map_err(|error| error.to_string())?;
    file.write_all(&bytes).map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())?;
    fs::rename(&temporary, path).map_err(|error| error.to_string())?;
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| error.to_string())
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        value.push(DIGITS[usize::from(byte >> 4)] as char);
        value.push(DIGITS[usize::from(byte & 0x0f)] as char);
    }
    value
}

fn object_for_asset<'a>(
    objects: &'a [FastAssetObjectV1],
    asset: &postfiat_types::FastAssetIdV1,
) -> Result<&'a FastAssetObjectV1, String> {
    let matching = objects
        .iter()
        .filter(|object| &object.asset_id == asset)
        .collect::<Vec<_>>();
    if matching.len() != 1 {
        return Err(format!(
            "expected one current object for asset {}, found {}",
            hex(&asset.0),
            matching.len()
        ));
    }
    Ok(matching[0])
}

fn next_intent(
    template: &FastSwapIntentV1,
    current_objects: &[FastAssetObjectV1],
    nonce: [u8; 32],
) -> Result<FastSwapIntentV1, String> {
    let mut intent = template.clone();
    intent.nonce = nonce;
    let party_0_input = object_for_asset(current_objects, &intent.party_0.offered_asset_id)?;
    let party_1_input = object_for_asset(current_objects, &intent.party_1.offered_asset_id)?;
    if party_0_input.amount_atoms != intent.party_0.offered_amount
        || party_1_input.amount_atoms != intent.party_1.offered_amount
        || party_0_input.asset_rule_hash != intent.party_0.offered_asset_rule_hash
        || party_1_input.asset_rule_hash != intent.party_1.offered_asset_rule_hash
    {
        return Err("current FastSwap objects do not match certified quote economics".to_owned());
    }
    intent.party_0.owner_pubkey = party_0_input.owner_pubkey.clone();
    intent.party_0.owner_address = address_from_public_key(&party_0_input.owner_pubkey);
    intent.party_0.asset_inputs = vec![party_0_input.key];
    intent.party_1.owner_pubkey = party_1_input.owner_pubkey.clone();
    intent.party_1.owner_address = address_from_public_key(&party_1_input.owner_pubkey);
    intent.party_1.asset_inputs = vec![party_1_input.key];
    intent
        .validate_canonical_shape()
        .map_err(|error| format!("constructed non-canonical intent: {error:?}"))?;
    Ok(intent)
}

fn backup_for_owner<'a>(
    owner_pubkey: &[u8],
    backups: &'a [(Vec<u8>, WalletBackupFile)],
) -> Result<&'a WalletBackupFile, String> {
    backups
        .iter()
        .find(|(public_key, _)| public_key == owner_pubkey)
        .map(|(_, backup)| backup)
        .ok_or_else(|| "no configured demo key owns the current FastSwap object".to_owned())
}

fn assert_conserved(
    intent: &FastSwapIntentV1,
    created: &[FastAssetObjectV1],
) -> Result<(), String> {
    let mut expected = BTreeMap::new();
    *expected
        .entry(intent.party_0.offered_asset_id)
        .or_insert(0u64) += intent.party_0.offered_amount;
    *expected
        .entry(intent.party_1.offered_asset_id)
        .or_insert(0u64) += intent.party_1.offered_amount;
    let mut actual = BTreeMap::new();
    for object in created {
        *actual.entry(object.asset_id).or_insert(0u64) += object.amount_atoms;
    }
    if actual != expected {
        return Err("FastSwap asset totals were not atom-for-atom conserved".to_owned());
    }
    Ok(())
}

fn quote_nonce(prior_swap_id: &[u8], quote_counter: u64) -> [u8; 32] {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let mut hasher = Sha3_384::new();
    hasher.update(b"postfiat.fastswap.wallet_demo.quote.v1");
    hasher.update(prior_swap_id);
    hasher.update(quote_counter.to_be_bytes());
    hasher.update(now.to_be_bytes());
    let digest = hasher.finalize();
    let mut nonce = [0u8; 32];
    nonce.copy_from_slice(&digest[..32]);
    nonce
}

#[derive(Clone)]
struct PendingQuote {
    id: String,
    intent: FastSwapIntentV1,
    observed_height: u64,
}

struct DemoState {
    session: FastSwapWalletSessionV1,
    current_objects: Vec<FastAssetObjectV1>,
    pending_quotes: BTreeMap<String, PendingQuote>,
    quote_counter: u64,
    latest_result: Option<Value>,
}

struct Service {
    committee: FastSwapCommitteeV1,
    endpoints: BTreeMap<String, String>,
    transport: TcpFastSwapTransportV1,
    backups: Vec<(Vec<u8>, WalletBackupFile)>,
    state_path: PathBuf,
    evidence_root: PathBuf,
    faucet_python: PathBuf,
    faucet_runner: PathBuf,
    faucet_stakehub_root: PathBuf,
    faucet_profile: PathBuf,
    faucet_evidence_root: PathBuf,
    api_token: String,
    state: Mutex<DemoState>,
    faucet_lock: Mutex<()>,
}

#[derive(Debug, Deserialize)]
struct SwapRequest {
    quote_id: String,
    confirm: bool,
}

#[derive(Debug, Deserialize)]
struct FaucetRequest {
    address: String,
}

fn valid_postfiat_address(address: &str) -> bool {
    address.len() == 42
        && address.starts_with("pf")
        && address
            .as_bytes()
            .iter()
            .skip(2)
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
}

fn validator_values(
    service: &Service,
    request_for: impl Fn(&str) -> RpcRequest + Sync,
) -> Result<BTreeMap<String, Value>, String> {
    let mut values = BTreeMap::new();
    std::thread::scope(|scope| {
        let workers =
            service
                .committee
                .validators
                .iter()
                .map(|validator| {
                    let validator_id = validator.validator_id.clone();
                    let endpoint = service
                        .endpoints
                        .get(&validator_id)
                        .ok_or_else(|| format!("missing endpoint for {validator_id}"))?
                        .clone();
                    let request = request_for(&validator_id);
                    Ok::<_, String>(scope.spawn(move || {
                        raw_rpc(&endpoint, &request).map(|value| (validator_id, value))
                    }))
                })
                .collect::<Result<Vec<_>, String>>()?;
        for worker in workers {
            let (validator_id, value) = worker
                .join()
                .map_err(|_| "validator read worker panicked".to_owned())??;
            values.insert(validator_id, value);
        }
        Ok::<(), String>(())
    })?;
    Ok(values)
}

fn require_exact_six(values: &BTreeMap<String, Value>, label: &str) -> Result<Value, String> {
    if values.len() != 6 {
        return Err(format!("{label}: expected six validator responses"));
    }
    let first = values
        .values()
        .next()
        .ok_or("empty validator response set")?;
    let normalized = |value: &Value| {
        let mut value = value.clone();
        if let Some(object) = value.as_object_mut() {
            object.remove("validator_id");
        }
        value
    };
    let expected = normalized(first);
    if values.values().any(|value| normalized(value) != expected) {
        return Err(format!("{label}: validators returned different views"));
    }
    Ok(first.clone())
}

fn raw_rpc(endpoint: &str, request: &RpcRequest) -> Result<Value, String> {
    let mut stream =
        TcpStream::connect(endpoint).map_err(|error| format!("raw RPC connect failed: {error}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(15)))
        .and_then(|_| stream.set_write_timeout(Some(Duration::from_secs(15))))
        .and_then(|_| stream.set_nodelay(true))
        .map_err(|error| format!("raw RPC socket setup failed: {error}"))?;
    serde_json::to_writer(&mut stream, request).map_err(|error| error.to_string())?;
    stream.write_all(b"\n").map_err(|error| error.to_string())?;
    stream.flush().map_err(|error| error.to_string())?;
    let mut reader = BufReader::new(stream);
    let mut response = Vec::new();
    reader
        .by_ref()
        .take(MAX_RAW_RPC_RESPONSE_BYTES as u64)
        .read_until(b'\n', &mut response)
        .map_err(|error| error.to_string())?;
    if response.len() >= MAX_RAW_RPC_RESPONSE_BYTES {
        return Err("raw RPC response exceeded bound".to_owned());
    }
    let value: Value = serde_json::from_slice(&response).map_err(|error| error.to_string())?;
    if value.get("ok") != Some(&Value::Bool(true)) {
        return Err("market status RPC returned an error".to_owned());
    }
    value
        .get("result")
        .cloned()
        .ok_or_else(|| "market status RPC omitted result".to_owned())
}

fn market_views(service: &Service, asset_id: &str, epoch: u64) -> Result<Value, String> {
    let mut values = BTreeMap::new();
    std::thread::scope(|scope| {
        let workers = service
            .endpoints
            .iter()
            .map(|(validator_id, endpoint)| {
                let validator_id = validator_id.clone();
                let endpoint = endpoint.clone();
                let request =
                    RpcRequest::empty(format!("wallet-market-{validator_id}"), "market_ops_status")
                        .with_param("asset_id", asset_id)
                        .map_err(|error| error.to_string())?
                        .with_param("epoch", epoch.to_string())
                        .map_err(|error| error.to_string())?;
                Ok::<_, String>(
                    scope.spawn(move || {
                        raw_rpc(&endpoint, &request).map(|value| (validator_id, value))
                    }),
                )
            })
            .collect::<Result<Vec<_>, String>>()?;
        for worker in workers {
            let (validator_id, value) = worker
                .join()
                .map_err(|_| "market status worker panicked".to_owned())??;
            values.insert(validator_id, value);
        }
        Ok::<(), String>(())
    })?;
    require_exact_six(&values, "NAV market status")
}

fn live_current_objects(
    service: &Service,
    template: &FastSwapIntentV1,
) -> Result<Vec<FastAssetObjectV1>, String> {
    let owners = service
        .backups
        .iter()
        .map(|(public_key, _)| public_key)
        .collect::<Vec<_>>();
    let assets = [
        template.party_0.offered_asset_id,
        template.party_1.offered_asset_id,
    ];
    let mut objects = Vec::new();
    for owner in owners {
        for asset in assets {
            let owner_hex = hex(owner);
            let asset_hex = hex(&asset.0);
            let views = validator_values(service, |validator_id| {
                fastswap_objects_request(
                    format!("wallet-objects-{validator_id}"),
                    owner_hex.clone(),
                    Some(asset_hex.clone()),
                    None,
                    16,
                )
            })?;
            let view = require_exact_six(&views, "FastSwap owned objects")?;
            let response: FastSwapObjectsResponseV1 =
                serde_json::from_value(view).map_err(|error| error.to_string())?;
            if response.next_cursor.is_some() {
                return Err("FastSwap demo object query exceeded its bounded page".to_owned());
            }
            for object in response.objects {
                if object.owner_pubkey == *owner && object.asset_id == asset {
                    objects.push(object);
                }
            }
        }
    }
    objects.sort_by_key(|object| object.asset_id);
    objects.dedup_by_key(|object| object.key);
    if objects.len() != 2
        || object_for_asset(&objects, &template.party_0.offered_asset_id)?.amount_atoms
            != template.party_0.offered_amount
        || object_for_asset(&objects, &template.party_1.offered_asset_id)?.amount_atoms
            != template.party_1.offered_amount
    {
        return Err("live FastSwap demo objects do not match the certified dust quote".to_owned());
    }
    Ok(objects)
}

fn exact_six_terminal_audit(
    service: &Service,
    session: &FastSwapWalletSessionV1,
) -> Result<(), String> {
    let swap_id = hex(&session.expected_effects.swap_id.0);
    for validator in &service.committee.validators {
        let endpoint = service
            .endpoints
            .get(&validator.validator_id)
            .ok_or_else(|| format!("missing endpoint for {}", validator.validator_id))?;
        let status: FastSwapStatusResponseV1 = serde_json::from_value(raw_rpc(
            endpoint,
            &fastswap_status_request(
                format!("wallet-terminal-status-{}", validator.validator_id),
                swap_id.clone(),
            ),
        )?)
        .map_err(|error| format!("terminal status decode: {error}"))?;
        if status.record.as_ref().map(|record| record.status)
            != Some(FastSwapLocalStatusV1::Applied)
        {
            return Err(format!("{} is not applied", validator.validator_id));
        }
        let effects: FastSwapEffectsResponseV1 = serde_json::from_value(raw_rpc(
            endpoint,
            &fastswap_effects_request(
                format!("wallet-terminal-effects-{}", validator.validator_id),
                swap_id.clone(),
            ),
        )?)
        .map_err(|error| format!("terminal effects decode: {error}"))?;
        if effects.effects.as_ref() != Some(&session.expected_effects) {
            return Err(format!("{} effects mismatch", validator.validator_id));
        }
    }
    Ok(())
}

fn recovery_candidate(
    evidence_root: &Path,
) -> Result<Option<(PathBuf, FastSwapWalletSessionV1)>, String> {
    let mut candidates = Vec::new();
    for entry in fs::read_dir(evidence_root).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path().join("session.json");
        if !path.is_file() {
            continue;
        }
        let bytes = fs::read(&path).map_err(|error| error.to_string())?;
        let value: Value = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
        if value.get("state").and_then(Value::as_str) != Some("accepted")
            || value
                .pointer("/expected_effects/receipt/accepted")
                .and_then(Value::as_bool)
                != Some(true)
            || value
                .pointer("/expected_effects/receipt/code")
                .and_then(Value::as_str)
                != Some("fastswap_applied")
        {
            continue;
        }
        let modified = fs::metadata(&path)
            .and_then(|metadata| metadata.modified())
            .unwrap_or(UNIX_EPOCH);
        let session = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
        candidates.push((modified, entry.path(), session));
    }
    candidates.sort_by_key(|(modified, _, _)| *modified);
    Ok(candidates
        .pop()
        .map(|(_, evidence_dir, session)| (evidence_dir, session)))
}

fn acceptance_result(
    session: &FastSwapWalletSessionV1,
    evidence_dir: &Path,
    recovered: bool,
    exact_six_audit_ms: u64,
) -> Result<Value, String> {
    let timings = session
        .last_timings
        .as_ref()
        .ok_or("accepted session omitted stage timings")?;
    let votes = |certificate: &Option<postfiat_types::FastSwapCertificateV1>| {
        certificate.as_ref().map_or(0, |value| value.votes.len())
    };
    Ok(json!({
        "status": "accepted",
        "receipt": {"accepted": true, "code": session.expected_effects.receipt.code},
        "swap_id": hex(&session.expected_effects.swap_id.0),
        "sell": {"symbol": "pfUSDC", "amount_atoms": session.signed_intent.intent.party_0.offered_amount},
        "buy": {"symbol": "a651", "amount_atoms": session.signed_intent.intent.party_1.offered_amount},
        "checks": {
            "dual_owner_signatures": true,
            "lock_certificate_votes": votes(&session.lock_qc),
            "decision_certificate_votes": votes(&session.decision_qc),
            "effects_certificate_votes": votes(&session.effects_qc),
            "exact_six_applied": true,
            "atom_for_atom_conserved": true,
            "both_or_neither": true,
        },
        "timings_ms": {
            "prepare_qc": timings.prepare_qc_ms,
            "decision_qc": timings.decision_qc_ms,
            "effects_qc": timings.effects_qc_ms,
            "settlement": timings.total_ms,
            "exact_six_audit": exact_six_audit_ms,
            "total_with_exact_six_audit": timings.total_ms.saturating_add(exact_six_audit_ms),
        },
        "recovered_after_client_disconnect": recovered,
        "evidence_path": evidence_dir.display().to_string(),
    }))
}

fn live_facts(service: &Service, intent: &FastSwapIntentV1) -> Result<Value, String> {
    let raw_statuses = validator_values(service, |validator_id| {
        status_request(format!("wallet-status-{validator_id}"))
    })?;
    let statuses = raw_statuses
        .into_iter()
        .map(|(validator_id, status)| {
            (
                validator_id,
                json!({
                    "chain_id": status.get("chain_id"),
                    "block_height": status.get("block_height"),
                    "block_tip_hash": status.get("block_tip_hash"),
                    "state_root": status.get("state_root"),
                    "mempool_pending": status.get("mempool_pending"),
                    "validator_count": status.get("validator_count"),
                    "build_git_revision": status.get("build_git_revision"),
                }),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let status = require_exact_six(&statuses, "chain status")?;
    let height = status
        .get("block_height")
        .and_then(Value::as_u64)
        .ok_or("status omitted block_height")?;
    let policy_hash = hex(&intent.policy_hash.0);
    let policies = validator_values(service, |validator_id| {
        fastswap_policy_by_hash_request(
            format!("wallet-policy-{validator_id}"),
            policy_hash.clone(),
        )
    })?;
    let policy_value = require_exact_six(&policies, "FastSwap policy")?;
    let policy_response: FastSwapPolicyResponseV1 =
        serde_json::from_value(policy_value).map_err(|error| error.to_string())?;
    let policy = policy_response
        .policy
        .ok_or("certified FastSwap policy is not active")?;
    policy
        .validate()
        .map_err(|error| format!("invalid certified policy: {error:?}"))?;
    let market = market_views(service, &hex(&policy.pair_asset_1.0), policy.nav_epoch)?;
    let nav_usd_e8 = market
        .get("nav_floor_usd_e8")
        .and_then(Value::as_u64)
        .ok_or("market status omitted NAV")?;
    if policy.price_numerator != 100_000_000
        || policy.price_denominator != u128::from(nav_usd_e8)
        || policy.nav_epoch != intent.nav_epoch
        || policy.market_envelope_hash != intent.market_envelope_hash
    {
        return Err("FastSwap policy does not bind the live certified NAV".to_owned());
    }
    let reserve_fresh = market
        .get("reserve_packet_fresh")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let supply_fresh = market
        .get("supply_packet_fresh")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let market_active = market
        .get("market_operations_status")
        .and_then(Value::as_str)
        == Some("active");
    let active = !policy.paused
        && reserve_fresh
        && supply_fresh
        && market_active
        && height >= policy.valid_from_height
        && height <= policy.valid_through_height
        && height <= intent.expires_at_height;
    Ok(json!({
        "chain": {
            "chain_id": intent.domain.chain.chain_id,
            "height": height,
            "state_root": status.get("state_root"),
            "validators_reporting": 6,
            "exact_six": true,
            "quorum_required": service.committee.domain.quorum,
        },
        "nav": {
            "active": active,
            "epoch": policy.nav_epoch,
            "usd_e8": nav_usd_e8,
            "reserve_packet_age_blocks": market.get("reserve_packet_age_blocks"),
            "supply_packet_age_blocks": market.get("supply_packet_age_blocks"),
            "reserve_packet_fresh": reserve_fresh,
            "supply_packet_fresh": supply_fresh,
            "packet_expires_at": market.get("packet_expires_at"),
            "market_operations_status": market.get("market_operations_status"),
            "policy_epoch": policy.policy_epoch,
            "policy_valid_from_height": policy.valid_from_height,
            "policy_valid_through_height": policy.valid_through_height,
            "policy_blocks_remaining": policy.valid_through_height.saturating_sub(height),
            "policy_paused": policy.paused,
            "policy_hash": hex(&policy.policy_hash.0),
            "market_envelope_hash": hex(&policy.market_envelope_hash.0),
            "source": "certified PFTL validator policy plus exact-six live market status",
        }
    }))
}

impl Service {
    fn status(&self) -> Result<Value, String> {
        let state = self.state.lock().map_err(|_| "state lock poisoned")?;
        let intent = &state.session.signed_intent.intent;
        let mut facts = live_facts(self, intent)?;
        facts["service"] = json!({
            "ready": true,
            "wire_v2": self.transport.compact_payloads_enabled(),
            "settlement": "three dependent 5-of-6 FastSwap certificates",
            "custody": "controlled devnet demo accounts",
        });
        facts["latest_swap"] = state.latest_result.clone().unwrap_or(Value::Null);
        Ok(facts)
    }

    fn run_faucet_adapter(&self, address: &str, status_only: bool) -> Result<Value, String> {
        let run_dir = self.faucet_evidence_root.join(address);
        let mut command = Command::new(&self.faucet_python);
        command
            .arg(&self.faucet_runner)
            .arg("--stakehub-root")
            .arg(&self.faucet_stakehub_root)
            .arg("--profile")
            .arg(&self.faucet_profile)
            .arg("--run-dir")
            .arg(&run_dir)
            .arg("--address")
            .arg(address)
            .arg("--amount-atoms")
            .arg(DEVNET_FAUCET_AMOUNT_ATOMS.to_string());
        if status_only {
            command.arg("--status-only");
        }
        let output = command
            .output()
            .map_err(|error| format!("devnet faucet adapter failed to start: {error}"))?;
        if output.stdout.len() > MAX_RAW_RPC_RESPONSE_BYTES
            || output.stderr.len() > MAX_RAW_RPC_RESPONSE_BYTES
        {
            return Err("devnet faucet adapter output exceeded its bound".to_owned());
        }
        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(format!("devnet faucet request failed: {}", error.trim()));
        }
        serde_json::from_slice(&output.stdout)
            .map_err(|error| format!("devnet faucet returned invalid JSON: {error}"))
    }

    fn proven_faucet_result(&self, address: &str, result: &Value) -> Result<Value, String> {
        let proven = result.get("ok") == Some(&Value::Bool(true))
            && result.get("recipient").and_then(Value::as_str) == Some(address)
            && result.get("amount_atoms").and_then(Value::as_u64)
                == Some(DEVNET_FAUCET_AMOUNT_ATOMS)
            && result.get("valid_quorum_certificate") == Some(&Value::Bool(true))
            && result.get("exact_6") == Some(&Value::Bool(true))
            && result.get("accepted_receipts_6_of_6") == Some(&Value::Bool(true))
            && result.get("accepted_receipt_codes_6_of_6") == Some(&Value::Bool(true));
        if !proven {
            return Err("devnet faucet did not return the complete finality proof".to_owned());
        }
        Ok(json!({
            "claimed": true,
            "status": "accepted",
            "recipient": address,
            "amount_atoms": DEVNET_FAUCET_AMOUNT_ATOMS,
            "balance_atoms": result.get("balance_atoms"),
            "tx_id": result.get("tx_id"),
            "target": result.get("target"),
            "receipt": {"accepted": true, "code": "accepted"},
            "checks": {
                "valid_quorum_certificate": true,
                "exact_six": true,
                "accepted_receipt_code_6_of_6": true,
                "recipient_balance_delta": DEVNET_FAUCET_AMOUNT_ATOMS,
            },
            "idempotent_per_address": true,
        }))
    }

    fn faucet_status(&self, request: FaucetRequest) -> Result<Value, String> {
        if !valid_postfiat_address(&request.address) {
            return Err("enter a valid lowercase PostFiat address".to_owned());
        }
        let _guard = self
            .faucet_lock
            .lock()
            .map_err(|_| "faucet request lock poisoned")?;
        let result = self.run_faucet_adapter(&request.address, true)?;
        if result.get("ok") == Some(&Value::Bool(true))
            && result.get("claimed") == Some(&Value::Bool(false))
        {
            return Ok(json!({"claimed": false, "recipient": request.address}));
        }
        self.proven_faucet_result(&request.address, &result)
    }

    fn faucet(&self, request: FaucetRequest) -> Result<Value, String> {
        if !valid_postfiat_address(&request.address) {
            return Err("enter a valid lowercase PostFiat address".to_owned());
        }
        let _guard = self
            .faucet_lock
            .lock()
            .map_err(|_| "faucet request lock poisoned")?;
        let run_dir = self.faucet_evidence_root.join(&request.address);
        let prior_receipt = run_dir.join("receipts/native-gas/receipt.json");
        if !prior_receipt.is_file() {
            let claim_count = fs::read_dir(&self.faucet_evidence_root)
                .map_err(|error| error.to_string())?
                .filter_map(Result::ok)
                .filter(|entry| entry.path().is_dir())
                .count();
            if claim_count >= MAX_DEVNET_FAUCET_CLAIMS {
                return Err("devnet faucet campaign claim cap reached".to_owned());
            }
        }
        fs::create_dir_all(&run_dir).map_err(|error| error.to_string())?;
        let result = self.run_faucet_adapter(&request.address, false)?;
        self.proven_faucet_result(&request.address, &result)
    }

    fn quote(&self) -> Result<Value, String> {
        let mut state = self.state.lock().map_err(|_| "state lock poisoned")?;
        let facts = live_facts(self, &state.session.signed_intent.intent)?;
        if facts["nav"]["active"] != Value::Bool(true) {
            return Err("certified NAV is not currently usable; quote disabled".to_owned());
        }
        state.quote_counter = state.quote_counter.saturating_add(1);
        let nonce = quote_nonce(
            &state.session.expected_effects.swap_id.0,
            state.quote_counter,
        );
        let intent = next_intent(
            &state.session.signed_intent.intent,
            &state.current_objects,
            nonce,
        )?;
        let quote_id = hex(&intent
            .intent_id()
            .map_err(|error| format!("quote id: {error:?}"))?
            .0);
        let observed_height = facts["chain"]["height"].as_u64().unwrap_or_default();
        state.pending_quotes.insert(
            quote_id.clone(),
            PendingQuote {
                id: quote_id.clone(),
                intent: intent.clone(),
                observed_height,
            },
        );
        while state.pending_quotes.len() > 8 {
            if let Some(oldest) = state.pending_quotes.keys().next().cloned() {
                state.pending_quotes.remove(&oldest);
            }
        }
        Ok(json!({
            "quote_id": quote_id,
            "observed_height": observed_height,
            "expires_at_height": intent.expires_at_height,
            "sell": {
                "symbol": "pfUSDC",
                "amount_atoms": intent.party_0.offered_amount,
                "decimals": 8,
                "owner": intent.party_0.owner_address,
            },
            "buy": {
                "symbol": "a651",
                "amount_atoms": intent.party_1.offered_amount,
                "decimals": 8,
                "owner": intent.party_0.owner_address,
            },
            "liquidity_provider": intent.party_1.owner_address,
            "fees": {"uniswap_usd": "0.00", "fastswap_pft_atoms_per_party": 1},
            "execution": {
                "both_or_neither": true,
                "owner_signatures": 2,
                "certificate_rounds": 3,
                "votes_required_per_round": self.committee.domain.quorum,
                "validator_count": self.committee.domain.validator_count,
            },
            "nav": facts["nav"],
            "chain": facts["chain"],
            "rounding_note": "This is a dust-size proof. Atomic-unit rounding is visible; the certified whole-a651 NAV remains the headline price.",
        }))
    }

    fn execute(&self, request: SwapRequest) -> Result<Value, String> {
        if !request.confirm {
            return Err("explicit confirmation is required".to_owned());
        }
        let mut state = self.state.lock().map_err(|_| "state lock poisoned")?;
        let pending = state
            .pending_quotes
            .remove(&request.quote_id)
            .ok_or("quote id mismatch; refresh the quote")?;
        if pending.id != request.quote_id {
            return Err("quote identity check failed".to_owned());
        }
        let current_intent = next_intent(
            &state.session.signed_intent.intent,
            &state.current_objects,
            pending.intent.nonce,
        )?;
        if current_intent != pending.intent {
            return Err("quote inputs are no longer current; refresh the quote".to_owned());
        }
        let facts = live_facts(self, &pending.intent)?;
        let current_height = facts["chain"]["height"].as_u64().unwrap_or_default();
        if facts["nav"]["active"] != Value::Bool(true) || current_height != pending.observed_height
        {
            return Err("live height or NAV changed after review; refresh the quote".to_owned());
        }
        let owner_0 = backup_for_owner(&pending.intent.party_0.owner_pubkey, &self.backups)?;
        let owner_1 = backup_for_owner(&pending.intent.party_1.owner_pubkey, &self.backups)?;
        let operation_started = Instant::now();
        let sign_started = Instant::now();
        let signed = wallet_dual_sign_fastswap_intent(owner_0, owner_1, pending.intent.clone())
            .map_err(|error| format!("dual signing failed: {error}"))?;
        let sign_ms = sign_started.elapsed().as_millis() as u64;
        let preview_started = Instant::now();
        let expected = preview_fastswap(&signed, &self.committee, &self.transport)
            .map_err(|error| format!("FastSwap preview failed: {error:?}"))?;
        let preview_ms = preview_started.elapsed().as_millis() as u64;
        assert_conserved(&pending.intent, &expected.created)?;
        let swap_id = hex(&signed
            .swap_id()
            .map_err(|error| format!("swap id: {error:?}"))?
            .0);
        let swap_dir = self.evidence_root.join(&swap_id);
        if swap_dir.exists() {
            return Err("evidence directory already exists; refusing replay".to_owned());
        }
        fs::create_dir_all(&swap_dir).map_err(|error| error.to_string())?;
        let working_path = swap_dir.join("session.json");
        let mut session = FastSwapWalletSessionV1::new(
            SwapSettlementModeV1::FastSwapV1,
            signed,
            expected.clone(),
        )
        .map_err(|error| format!("session creation failed: {error:?}"))?;
        persist_json(&working_path, &session)?;
        let settlement_started = Instant::now();
        let terminal =
            drive_fastswap_three_wave(&mut session, &self.committee, &self.transport, |current| {
                persist_json(&working_path, current)
            })
            .map_err(|error| format!("FastSwap settlement failed: {error:?}"))?;
        let settlement_ms = settlement_started.elapsed().as_millis() as u64;
        if !terminal.effects.receipt.accepted
            || terminal.effects.receipt.code != "fastswap_applied"
            || terminal.lock_qc.votes.len() < usize::from(self.committee.domain.quorum)
            || terminal.decision_qc.votes.len() < usize::from(self.committee.domain.quorum)
            || terminal.effects_qc.votes.len() < usize::from(self.committee.domain.quorum)
        {
            return Err("terminal receipt or quorum certificate check failed".to_owned());
        }
        assert_conserved(&pending.intent, &terminal.effects.created)?;
        let replication = reconcile_fastswap_replication(
            &mut session,
            &self.committee,
            &self.transport,
            |current| persist_json(&working_path, current),
        )
        .map_err(|error| format!("exact-six replication failed: {error:?}"))?;
        if !replication.failed.is_empty() || !replication.pending.is_empty() {
            return Err(format!("exact-six replication incomplete: {replication:?}"));
        }
        exact_six_terminal_audit(self, &session)?;
        persist_json(&self.state_path, &session)?;
        let result = json!({
            "status": "accepted",
            "receipt": {"accepted": true, "code": terminal.effects.receipt.code},
            "swap_id": swap_id,
            "sell": {"symbol": "pfUSDC", "amount_atoms": pending.intent.party_0.offered_amount},
            "buy": {"symbol": "a651", "amount_atoms": pending.intent.party_1.offered_amount},
            "checks": {
                "dual_owner_signatures": true,
                "lock_certificate_votes": terminal.lock_qc.votes.len(),
                "decision_certificate_votes": terminal.decision_qc.votes.len(),
                "effects_certificate_votes": terminal.effects_qc.votes.len(),
                "exact_six_applied": true,
                "atom_for_atom_conserved": true,
                "both_or_neither": true,
            },
            "timings_ms": {
                "dual_sign": sign_ms,
                "preview": preview_ms,
                "settlement": settlement_ms,
                "total_with_exact_six_audit": operation_started.elapsed().as_millis() as u64,
            },
            "evidence_path": swap_dir.display().to_string(),
        });
        persist_json(&swap_dir.join("ACCEPTANCE.json"), &result)?;
        persist_json(&self.evidence_root.join("LATEST.json"), &result)?;
        state.session = session;
        state.current_objects = terminal.effects.created.clone();
        state.pending_quotes.clear();
        state.latest_result = Some(result.clone());
        Ok(result)
    }
}

struct HttpRequest {
    method: String,
    path: String,
    token: Option<String>,
    body: Vec<u8>,
}

fn read_http_request(stream: &mut TcpStream) -> Result<HttpRequest, String> {
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .map_err(|error| error.to_string())?;
    let mut reader = BufReader::new(stream.try_clone().map_err(|error| error.to_string())?);
    let mut request_line = String::new();
    reader
        .read_line(&mut request_line)
        .map_err(|error| error.to_string())?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().ok_or("missing HTTP method")?.to_owned();
    let path = parts.next().ok_or("missing HTTP path")?.to_owned();
    let mut content_length = 0usize;
    let mut token = None;
    let mut header_bytes = request_line.len();
    loop {
        let mut line = String::new();
        reader
            .read_line(&mut line)
            .map_err(|error| error.to_string())?;
        header_bytes = header_bytes.saturating_add(line.len());
        if header_bytes > MAX_HTTP_REQUEST_BYTES {
            return Err("HTTP headers exceed bound".to_owned());
        }
        if line == "\r\n" || line == "\n" {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            match name.trim().to_ascii_lowercase().as_str() {
                "content-length" => {
                    content_length = value
                        .trim()
                        .parse::<usize>()
                        .map_err(|_| "invalid content length")?;
                }
                "x-fastswap-demo-token" => token = Some(value.trim().to_owned()),
                _ => {}
            }
        }
    }
    if content_length > MAX_HTTP_REQUEST_BYTES.saturating_sub(header_bytes) {
        return Err("HTTP body exceeds bound".to_owned());
    }
    let mut body = vec![0u8; content_length];
    reader
        .read_exact(&mut body)
        .map_err(|error| error.to_string())?;
    Ok(HttpRequest {
        method,
        path,
        token,
        body,
    })
}

fn write_http(stream: &mut TcpStream, status: u16, body: &Value) -> Result<(), String> {
    let payload = serde_json::to_vec(body).map_err(|error| error.to_string())?;
    let (reason, cache) = if status == 200 {
        ("OK", "no-store")
    } else {
        ("Error", "no-store")
    };
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nCache-Control: {cache}\r\nX-Content-Type-Options: nosniff\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        payload.len()
    )
    .and_then(|_| stream.write_all(&payload))
    .and_then(|_| stream.flush())
    .map_err(|error| error.to_string())
}

fn handle_connection(service: &Service, mut stream: TcpStream) -> Result<(), String> {
    let request = read_http_request(&mut stream)?;
    if request.token.as_deref() != Some(service.api_token.as_str()) {
        return write_http(
            &mut stream,
            403,
            &json!({"ok": false, "error": "local demo authorization failed"}),
        );
    }
    let result = match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/api/fastswap-demo/status") => service.status(),
        ("GET", "/api/fastswap-demo/quote") => service.quote(),
        ("POST", "/api/fastswap-demo/faucet-status") => {
            serde_json::from_slice::<FaucetRequest>(&request.body)
                .map_err(|error| format!("invalid faucet status request: {error}"))
                .and_then(|body| service.faucet_status(body))
        }
        ("POST", "/api/fastswap-demo/faucet") => {
            serde_json::from_slice::<FaucetRequest>(&request.body)
                .map_err(|error| format!("invalid faucet request: {error}"))
                .and_then(|body| service.faucet(body))
        }
        ("POST", "/api/fastswap-demo/swap") => serde_json::from_slice::<SwapRequest>(&request.body)
            .map_err(|error| format!("invalid swap request: {error}"))
            .and_then(|body| service.execute(body)),
        _ => {
            return write_http(
                &mut stream,
                404,
                &json!({"ok": false, "error": "route not found"}),
            )
        }
    };
    match result {
        Ok(value) => write_http(&mut stream, 200, &json!({"ok": true, "result": value})),
        Err(error) => write_http(&mut stream, 409, &json!({"ok": false, "error": error})),
    }
}

fn main() -> Result<(), String> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let committee: FastSwapCommitteeV1 = read_json(Path::new(&flag(&args, "--committee")?))?;
    committee
        .validate()
        .map_err(|error| format!("invalid committee: {error:?}"))?;
    if committee.domain.validator_count != 6 || committee.domain.quorum != 5 {
        return Err("demo requires the exact six-validator, five-vote committee".to_owned());
    }
    let endpoints: BTreeMap<String, String> = read_json(Path::new(&flag(&args, "--endpoints")?))?;
    let template_path = PathBuf::from(flag(&args, "--template-session")?);
    let state_path = PathBuf::from(flag(&args, "--state")?);
    let evidence_root = PathBuf::from(flag(&args, "--evidence-root")?);
    let faucet_python = PathBuf::from(flag(&args, "--faucet-python")?);
    let faucet_runner = PathBuf::from(flag(&args, "--faucet-runner")?);
    let faucet_stakehub_root = PathBuf::from(flag(&args, "--faucet-stakehub-root")?);
    let faucet_profile = PathBuf::from(flag(&args, "--faucet-profile")?);
    let faucet_evidence_root = PathBuf::from(flag(&args, "--faucet-evidence-root")?);
    if !faucet_python.is_file()
        || !faucet_runner.is_file()
        || !faucet_stakehub_root.is_dir()
        || !faucet_profile.is_file()
    {
        return Err("configured devnet faucet authority is unavailable".to_owned());
    }
    let bind = flag(&args, "--bind")?;
    if !bind.starts_with("127.0.0.1:") {
        return Err("FastSwap wallet service is loopback-only".to_owned());
    }
    let api_token =
        env::var("FASTSWAP_DEMO_API_TOKEN").map_err(|_| "FASTSWAP_DEMO_API_TOKEN must be set")?;
    if api_token.len() < 24 {
        return Err("FASTSWAP_DEMO_API_TOKEN must contain at least 24 characters".to_owned());
    }
    let mut session = if state_path.exists() {
        read_json(&state_path)?
    } else {
        let session: FastSwapWalletSessionV1 = read_json(&template_path)?;
        persist_json(&state_path, &session)?;
        session
    };
    let backup_a: WalletBackupFile = read_json(Path::new(&flag(&args, "--owner-a-backup")?))?;
    let backup_b: WalletBackupFile = read_json(Path::new(&flag(&args, "--owner-b-backup")?))?;
    let backups = vec![
        (
            derive_wallet_key_pair(&backup_a)
                .map_err(|error| error.to_string())?
                .public_key,
            backup_a,
        ),
        (
            derive_wallet_key_pair(&backup_b)
                .map_err(|error| error.to_string())?
                .public_key,
            backup_b,
        ),
    ];
    if backups[0].0 == backups[1].0 {
        return Err("demo owners must be distinct".to_owned());
    }
    let transport = TcpFastSwapTransportV1::new(endpoints.clone(), Duration::from_secs(30))?;
    transport.prewarm_fastswap_runtime_v2(&committee)?;
    fs::create_dir_all(&evidence_root).map_err(|error| error.to_string())?;
    fs::create_dir_all(&faucet_evidence_root).map_err(|error| error.to_string())?;
    let recovery = recovery_candidate(&evidence_root)?;
    if let Some((_, recovered)) = &recovery {
        if recovered.expected_effects.swap_id != session.expected_effects.swap_id {
            session = recovered.clone();
        }
    }
    let latest_result = fs::read(evidence_root.join("LATEST.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok());
    let initial_objects = session.expected_effects.created.clone();
    let service = Arc::new(Service {
        committee,
        endpoints,
        transport,
        backups,
        state_path,
        evidence_root,
        faucet_python,
        faucet_runner,
        faucet_stakehub_root,
        faucet_profile,
        faucet_evidence_root,
        api_token,
        state: Mutex::new(DemoState {
            session,
            current_objects: initial_objects,
            pending_quotes: BTreeMap::new(),
            quote_counter: 0,
            latest_result,
        }),
        faucet_lock: Mutex::new(()),
    });
    let template = {
        service
            .state
            .lock()
            .map_err(|_| "state lock poisoned")?
            .session
            .signed_intent
            .intent
            .clone()
    };
    let current_objects = live_current_objects(&service, &template)?;
    assert_conserved(&template, &current_objects)?;
    if let Some((evidence_dir, recovered)) = recovery {
        if recovered.expected_effects.swap_id
            == service
                .state
                .lock()
                .map_err(|_| "state lock poisoned")?
                .session
                .expected_effects
                .swap_id
        {
            let audit_started = Instant::now();
            exact_six_terminal_audit(&service, &recovered)?;
            let audit_ms = audit_started.elapsed().as_millis() as u64;
            let result = acceptance_result(&recovered, &evidence_dir, true, audit_ms)?;
            persist_json(&service.state_path, &recovered)?;
            persist_json(&evidence_dir.join("ACCEPTANCE.json"), &result)?;
            persist_json(&service.evidence_root.join("LATEST.json"), &result)?;
            service
                .state
                .lock()
                .map_err(|_| "state lock poisoned")?
                .latest_result = Some(result);
        }
    }
    service
        .state
        .lock()
        .map_err(|_| "state lock poisoned")?
        .current_objects = current_objects;
    let listener = TcpListener::bind(&bind).map_err(|error| format!("bind {bind}: {error}"))?;
    eprintln!("FastSwap wallet service ready on {bind} (loopback, authenticated)");
    for incoming in listener.incoming() {
        match incoming {
            Ok(stream) => {
                let service = service.clone();
                std::thread::spawn(move || {
                    if let Err(error) = handle_connection(&service, stream) {
                        eprintln!("FastSwap wallet request failed: {error}");
                    }
                });
            }
            Err(error) => eprintln!("FastSwap wallet accept failed: {error}"),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quote_nonce_changes_with_counter_and_prior_swap() {
        assert_ne!(quote_nonce(b"prior", 1), quote_nonce(b"prior", 2));
        assert_ne!(quote_nonce(b"prior-a", 1), quote_nonce(b"prior-b", 1));
    }

    #[test]
    fn faucet_accepts_only_canonical_postfiat_addresses() {
        assert!(valid_postfiat_address(
            "pfde0ba09f38b1748f8d77709715e1095a0ff74d0f"
        ));
        assert!(!valid_postfiat_address(
            "pfDE0ba09f38b1748f8d77709715e1095a0ff74d0f"
        ));
        assert!(!valid_postfiat_address(
            "pfde0ba09f38b1748f8d77709715e1095a0ff74d0"
        ));
        assert!(!valid_postfiat_address("../../faucet"));
    }
}
