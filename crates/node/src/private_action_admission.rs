//! Validator-owned admission of user-proven private actions (W-1, S3).
//!
//! A wallet proves a shielded action on its device. `private_action_submit`
//! recomputes its intent id, strictly decodes the action, runs the execution
//! path's own verification against the finalized state, builds the canonical
//! atomic batch and reserves it durably before any transport. The round
//! service later dispatches exactly that reserved batch; `private_action_receipt`
//! is a read-only view of the record and of the chain.

use super::*;
use std::fs::{self, File, OpenOptions};

// The intent id, action hash, strict decode and receipt shape are the
// contract in `postfiat_types::private_actions_v1` (fixtures under
// crates/node/tests/fixtures/private-actions-v1/).
pub const PRIVATE_ACTION_RECORD_SCHEMA: &str = "postfiat.private_action_admission.record.v1";
pub const PRIVATE_ACTION_RECEIPT_SCHEMA: &str = "postfiat.private_action_receipt.v1";
pub const PRIVATE_ACTION_POOL_ID: &str = "asset-orchard-v1";
pub const PRIVATE_ACTION_MAX_JSON_BYTES: usize = 1024 * 1024;
const ADMISSION_DIR: &str = "private_action_admission";
const ADMISSION_LOG: &str = "records.jsonl";
const ADMISSION_LOCK: &str = "admission.lock";

/// The submit domain: the intent domain plus the pool and the gateway's session
/// scope, which are checked and recorded but not part of the intent hash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrivateActionSubmitDomain {
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub pool_id: String,
    pub session_scope: String,
}

impl PrivateActionSubmitDomain {
    pub fn intent_domain(&self) -> postfiat_types::PrivateActionDomain {
        postfiat_types::PrivateActionDomain {
            chain_id: self.chain_id.clone(),
            genesis_hash: self.genesis_hash.clone(),
            protocol_version: self.protocol_version,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrivateActionSubmitRequest {
    pub domain: PrivateActionSubmitDomain,
    pub intent_id: String,
    pub action_json: String,
    pub action_hash: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrivateActionState {
    Queued,
    Dispatched,
    Accepted,
    Rejected,
    Unknown,
    /// Set only by the operator command `private-action-resolve --abandon`
    /// while the batch is not on chain; a resubmit of the same bytes
    /// supersedes it with a fresh record.
    Abandoned,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivateActionInclusion {
    pub height: u64,
    pub block_hash: String,
    pub state_root: String,
    pub receipt_ids: Vec<String>,
    /// The block certificate and consensus-v2 commit exactly as the `blocks`
    /// read exposes them for a finalized block.
    pub certificate_id: String,
    pub certificate: serde_json::Value,
    /// The certificate in the node's own serialization (field order kept).
    pub certificate_json: String,
    pub consensus_v2_commit: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivateActionOutcome {
    pub accepted: bool,
    pub code: String,
    pub message: String,
    pub fee_charged: u64,
    /// `applied` for an included batch; `none` for a refused atomic batch,
    /// which is never committed and so charges no fee.
    pub state_effects: String,
    pub inclusion: Option<PrivateActionInclusion>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivateActionRecord {
    pub schema: String,
    pub intent_id: String,
    pub domain: PrivateActionSubmitDomain,
    pub action_hash: String,
    pub action_kind: String,
    pub action_json: String,
    pub batch_id: String,
    /// The exact canonical batch bytes; every dispatch sends these.
    pub batch_json: String,
    pub state: PrivateActionState,
    pub received_height: u64,
    pub dispatch_count: u32,
    pub outcome: Option<PrivateActionOutcome>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivateActionSubmitReport {
    pub schema: String,
    pub existing: bool,
    pub record: PrivateActionRecord,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivateActionReceiptReport {
    pub schema: String,
    pub intent_id: String,
    /// `pending_unknown` when this node holds no record for the intent.
    pub state: String,
    pub record: Option<PrivateActionRecord>,
    /// The same outcome in the wallet contract's receipt shape.
    pub contract_receipt: Option<postfiat_types::PrivateReceiptResponse>,
}

pub const PRIVATE_ACTION_FINALITY_KIND: &str = "validator_quorum_block_certificate_v1";

fn contract_receipt(record: &PrivateActionRecord) -> postfiat_types::PrivateReceiptResponse {
    let state = match record.state {
        PrivateActionState::Queued => postfiat_types::PrivateAdmissionState::Queued,
        PrivateActionState::Dispatched => postfiat_types::PrivateAdmissionState::Dispatched,
        PrivateActionState::Accepted => postfiat_types::PrivateAdmissionState::Accepted,
        PrivateActionState::Rejected => postfiat_types::PrivateAdmissionState::Rejected,
        PrivateActionState::Unknown | PrivateActionState::Abandoned => {
            postfiat_types::PrivateAdmissionState::Unknown
        }
    };
    let outcome = record.outcome.as_ref();
    let inclusion = outcome.and_then(|outcome| outcome.inclusion.as_ref());
    postfiat_types::PrivateReceiptResponse {
        schema: postfiat_types::PRIVATE_ACTIONS_SCHEMA_V1.to_string(),
        intent_id: record.intent_id.clone(),
        state,
        action_hash: record.action_hash.clone(),
        batch_id: record.batch_id.clone(),
        transaction_id: inclusion.and_then(|inclusion| inclusion.receipt_ids.first().cloned()),
        inclusion_height: inclusion.map(|inclusion| inclusion.height),
        block_hash: inclusion.map(|inclusion| inclusion.block_hash.clone()),
        state_root: inclusion.map(|inclusion| inclusion.state_root.clone()),
        accepted: outcome.map(|outcome| outcome.accepted),
        code: outcome.map(|outcome| outcome.code.clone()),
        fee: outcome.map(|outcome| outcome.fee_charged),
        finality: inclusion.map(|inclusion| postfiat_types::PrivateFinalityEvidence {
            kind: PRIVATE_ACTION_FINALITY_KIND.to_string(),
            height: inclusion.height,
            block_hash: inclusion.block_hash.clone(),
            state_root: inclusion.state_root.clone(),
            quorum: inclusion.certificate["quorum"].as_u64().unwrap_or(0) as u32,
            validator_count: inclusion.certificate["validators"]
                .as_array()
                .map_or(0, |validators| validators.len() as u32),
            certificate_json: inclusion.certificate_json.clone(),
        }),
    }
}

/// A refusal that carries a stable code (the execution's own code for a
/// verification failure).
fn refused(code: &str, message: impl Into<String>) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        format!("{code}: {}", message.into()),
    )
}

pub fn private_action_intent_id(
    domain: &postfiat_types::PrivateActionDomain,
    action_json: &str,
) -> io::Result<String> {
    postfiat_types::private_intent_id(domain, action_json)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

/// The contract's deduplication hash: the canonical bytes of the strictly
/// decoded action, so a re-encoded copy of a reserved action is a duplicate.
pub fn private_action_hash_for_json(action_json: &str) -> io::Result<String> {
    let action = decode_private_action(action_json)?;
    postfiat_types::private_action_hash(&action)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn is_lower_hex(value: &str, len: usize) -> bool {
    value.len() == len
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Decodes one action with the contract's strict parser: the three admitted
/// kinds only, legacy v1 kinds refused, no unknown fields at either JSON level.
pub fn decode_private_action(action_json: &str) -> io::Result<ShieldedAction> {
    postfiat_types::parse_private_public_action(action_json)
        .map_err(|code| refused(code, "action_json is not an admissible public action"))
}

pub fn private_action_kind(action: &ShieldedAction) -> &'static str {
    match action {
        ShieldedAction::AssetOrchardIngressV2(_) => "asset_orchard_ingress_v2",
        ShieldedAction::AssetOrchardPrivateEgressV1(_) => "asset_orchard_private_egress_v1",
        ShieldedAction::ShieldedSwapV1(_) => "shielded_swap_v1",
        _ => "not_admitted",
    }
}

/// The append-only, fsync'd admission log. The last line for an intent is its
/// current record.
pub struct PrivateActionAdmissionStore {
    dir: PathBuf,
    records: BTreeMap<String, PrivateActionRecord>,
    by_action_hash: BTreeMap<String, String>,
    by_batch_id: BTreeMap<String, String>,
    _lock: File,
}

impl PrivateActionAdmissionStore {
    pub fn open(data_dir: &Path) -> io::Result<Self> {
        let dir = data_dir.join(ADMISSION_DIR);
        fs::create_dir_all(&dir)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(dir.join(ADMISSION_LOCK))?;
        lock_exclusive(&lock)?;
        let mut store = Self {
            dir,
            records: BTreeMap::new(),
            by_action_hash: BTreeMap::new(),
            by_batch_id: BTreeMap::new(),
            _lock: lock,
        };
        let log = store.dir.join(ADMISSION_LOG);
        if log.exists() {
            let text = fs::read_to_string(&log)?;
            let mut good_len = 0;
            let mut lines = text.split_inclusive('\n').peekable();
            while let Some(line) = lines.next() {
                // Only a torn final line (a crash mid-append) is tolerated; it
                // is cut off so the next append starts on a clean line.
                if !line.ends_with('\n') && lines.peek().is_none() {
                    let file = OpenOptions::new().write(true).open(&log)?;
                    file.set_len(good_len as u64)?;
                    file.sync_all()?;
                    break;
                }
                let record =
                    serde_json::from_str::<PrivateActionRecord>(line.trim_end_matches('\n'))
                        .map_err(|error| {
                            io::Error::new(
                                io::ErrorKind::InvalidData,
                                format!("private action admission log is corrupt: {error}"),
                            )
                        })?;
                store.index(record);
                good_len += line.len();
            }
        }
        Ok(store)
    }

    fn index(&mut self, record: PrivateActionRecord) {
        self.by_action_hash
            .insert(record.action_hash.clone(), record.intent_id.clone());
        self.by_batch_id
            .insert(record.batch_id.clone(), record.intent_id.clone());
        self.records.insert(record.intent_id.clone(), record);
    }

    pub fn get(&self, intent_id: &str) -> Option<&PrivateActionRecord> {
        self.records.get(intent_id)
    }

    pub fn intent_for_action_hash(&self, action_hash: &str) -> Option<&str> {
        self.by_action_hash.get(action_hash).map(String::as_str)
    }

    pub fn intent_for_batch_id(&self, batch_id: &str) -> Option<&str> {
        self.by_batch_id.get(batch_id).map(String::as_str)
    }

    pub fn records(&self) -> impl Iterator<Item = &PrivateActionRecord> {
        self.records.values()
    }

    /// Every logged record for the intent, oldest first: the audit trail.
    pub fn history(&self, intent_id: &str) -> io::Result<Vec<PrivateActionRecord>> {
        let log = self.dir.join(ADMISSION_LOG);
        if !log.exists() {
            return Ok(Vec::new());
        }
        let mut history = Vec::new();
        for line in fs::read_to_string(log)?.lines() {
            if let Ok(record) = serde_json::from_str::<PrivateActionRecord>(line) {
                if record.intent_id == intent_id {
                    history.push(record);
                }
            }
        }
        Ok(history)
    }

    /// Appends the record and fsyncs before returning.
    pub fn write(&mut self, record: PrivateActionRecord) -> io::Result<()> {
        let mut line = serde_json::to_vec(&record)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        line.push(b'\n');
        let path = self.dir.join(ADMISSION_LOG);
        let created = !path.exists();
        let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
        file.write_all(&line)?;
        file.sync_data()?;
        if created {
            File::open(&self.dir)?.sync_all()?;
        }
        self.index(record);
        Ok(())
    }
}

fn lock_exclusive(file: &File) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;
        loop {
            // SAFETY: the descriptor stays open in the store for the whole
            // critical section; flock only reads the borrowed descriptor.
            if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) } == 0 {
                return Ok(());
            }
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::Interrupted {
                return Err(error);
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = file;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "private action admission requires Unix flock",
        ))
    }
}

struct FinalizedAdmissionState {
    genesis: Genesis,
    governance: GovernanceState,
    ledger: LedgerState,
    shielded: ShieldedState,
    bridge: BridgeState,
    ordered_batches: Vec<String>,
    tip_height: u64,
}

fn read_finalized_admission_state(data_dir: &Path) -> io::Result<FinalizedAdmissionState> {
    let store = NodeStore::new(data_dir);
    let genesis = store.read_genesis()?;
    let tip = read_chain_tip_or_reconstruct_for_genesis_read_only(&store, &genesis)?;
    Ok(FinalizedAdmissionState {
        governance: store.read_governance()?,
        ledger: read_fastpay_ledger(&store)?,
        shielded: store.read_shielded()?,
        bridge: store.read_bridge()?,
        ordered_batches: store.read_ordered_batches()?,
        tip_height: tip.height,
        genesis,
    })
}

/// Runs the execution path's verification for the next height against the
/// finalized state. Returns the first rejection's own code and message.
fn verify_private_batch(
    state: &FinalizedAdmissionState,
    batch: &ShieldedActionBatch,
) -> io::Result<Result<(), (String, String)>> {
    let height = state
        .tip_height
        .checked_add(1)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "block height overflow"))?;
    let report = simulate_shielded_batch_against_state(
        &state.genesis,
        &state.governance,
        &state.ledger,
        &state.ordered_batches,
        &state.shielded,
        &state.bridge,
        height,
        batch,
    )?;
    if report.all_accepted {
        return Ok(Ok(()));
    }
    let rejection = report
        .receipts
        .iter()
        .find(|receipt| !receipt.accepted && receipt.code != "atomic_batch_aborted")
        .or_else(|| report.receipts.iter().find(|receipt| !receipt.accepted))
        .map(|receipt| (receipt.code.clone(), receipt.message.clone()))
        .unwrap_or_else(|| ("private_action_rejected".to_string(), String::new()));
    Ok(Err(rejection))
}

pub fn private_action_submit(
    data_dir: &Path,
    request: &PrivateActionSubmitRequest,
) -> io::Result<PrivateActionSubmitReport> {
    if !is_lower_hex(&request.intent_id, 96) || !is_lower_hex(&request.action_hash, 96) {
        return Err(refused(
            "private_action_request_invalid",
            "intent_id and action_hash must be 96 lowercase hex characters",
        ));
    }
    if request.action_json.is_empty() || request.action_json.len() > PRIVATE_ACTION_MAX_JSON_BYTES {
        return Err(refused(
            "private_action_request_invalid",
            "action_json must be 1 to 1048576 bytes",
        ));
    }
    let scope = &request.domain.session_scope;
    if scope.is_empty()
        || scope.len() > 128
        || !scope
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._:-".contains(&b))
    {
        return Err(refused(
            "private_action_request_invalid",
            "session_scope must be 1 to 128 characters of [A-Za-z0-9._:-]",
        ));
    }
    let state = read_finalized_admission_state(data_dir)?;
    let domain = &request.domain;
    if domain.chain_id != state.genesis.chain_id
        || domain.genesis_hash != genesis_hash(&state.genesis)
        || domain.protocol_version != state.genesis.protocol_version
        || domain.pool_id != PRIVATE_ACTION_POOL_ID
    {
        return Err(refused(
            "private_action_wrong_domain",
            "domain does not name this chain, genesis, protocol and pool",
        ));
    }
    if private_action_intent_id(&domain.intent_domain(), &request.action_json)? != request.intent_id
    {
        return Err(refused(
            "private_intent_mismatch",
            "intent_id is not the tasknode.private-intent.v1 hash of (domain, action_json)",
        ));
    }
    let action = decode_private_action(&request.action_json)?;
    let action_hash = postfiat_types::private_action_hash(&action)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    if action_hash != request.action_hash {
        return Err(refused(
            "private_action_hash_mismatch",
            "action_hash is not the canonical hash of the decoded action",
        ));
    }

    let mut store = PrivateActionAdmissionStore::open(data_dir)?;
    // The same bytes after an operator abandonment reserve again under the same
    // intent; the indexes keep pointing at that intent and the abandoned
    // record stays in the log.
    let supersedes_abandoned = store.get(&request.intent_id).is_some_and(|existing| {
        existing.state == PrivateActionState::Abandoned
            && existing.action_json == request.action_json
    });
    let owned_by_this_intent =
        |intent: &str| supersedes_abandoned && intent == request.intent_id.as_str();
    if let Some(existing) = store
        .get(&request.intent_id)
        .filter(|_| !supersedes_abandoned)
    {
        if existing.action_json == request.action_json && existing.domain == request.domain {
            return Ok(PrivateActionSubmitReport {
                schema: PRIVATE_ACTION_RECORD_SCHEMA.to_string(),
                existing: true,
                record: existing.clone(),
            });
        }
        return Err(refused(
            "private_intent_conflict",
            "a different action is already reserved under this intent",
        ));
    }
    if store
        .intent_for_action_hash(&request.action_hash)
        .is_some_and(|intent| !owned_by_this_intent(intent))
    {
        return Err(refused(
            "private_action_duplicate",
            "this action is already reserved under another intent",
        ));
    }
    let action_kind = private_action_kind(&action).to_string();
    let batch =
        consensus_artifacts::build_atomic_shielded_action_batch(&state.genesis, vec![action])?;
    consensus_artifacts::verify_shielded_action_batch_id(&state.genesis, &batch)?;
    if store
        .intent_for_batch_id(&batch.batch_id)
        .is_some_and(|intent| !owned_by_this_intent(intent))
    {
        return Err(refused(
            "private_action_duplicate",
            "this canonical batch is already reserved under another intent",
        ));
    }
    if state.ordered_batches.contains(&batch.batch_id) {
        return Err(refused(
            "private_action_duplicate",
            "this canonical batch is already on chain",
        ));
    }
    if let Err((code, message)) = verify_private_batch(&state, &batch)? {
        return Err(refused(&code, message));
    }
    let batch_json = serde_json::to_string(&batch)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let record = PrivateActionRecord {
        schema: PRIVATE_ACTION_RECORD_SCHEMA.to_string(),
        intent_id: request.intent_id.clone(),
        domain: request.domain.clone(),
        action_hash: request.action_hash.clone(),
        action_kind,
        action_json: request.action_json.clone(),
        batch_id: batch.batch_id.clone(),
        batch_json,
        state: PrivateActionState::Queued,
        received_height: state.tip_height,
        dispatch_count: 0,
        outcome: None,
    };
    store.write(record.clone())?;
    Ok(PrivateActionSubmitReport {
        schema: PRIVATE_ACTION_RECORD_SCHEMA.to_string(),
        existing: false,
        record,
    })
}

/// The committed block carrying `batch_id`, with its receipts.
pub fn private_action_chain_inclusion(
    data_dir: &Path,
    batch_id: &str,
) -> io::Result<Option<(PrivateActionInclusion, Vec<Receipt>)>> {
    let store = NodeStore::new(data_dir);
    let blocks = store.read_blocks()?;
    let Some(block) = blocks
        .blocks
        .iter()
        .rev()
        .find(|block| block.header.batch_id == batch_id)
    else {
        return Ok(None);
    };
    let receipts = store
        .read_receipts()?
        .into_iter()
        .filter(|receipt| block.receipt_ids.contains(&receipt.tx_id))
        .collect::<Vec<_>>();
    let to_value = |value: serde_json::Result<serde_json::Value>| {
        value.map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
    };
    Ok(Some((
        PrivateActionInclusion {
            height: block.header.height,
            block_hash: block.header.block_hash.clone(),
            state_root: block.header.state_root.clone(),
            receipt_ids: block.receipt_ids.clone(),
            certificate_id: block.header.certificate_id.clone(),
            certificate: to_value(serde_json::to_value(&block.header.certificate))?,
            certificate_json: serde_json::to_string(&block.header.certificate)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?,
            consensus_v2_commit: to_value(serde_json::to_value(&block.header.consensus_v2_commit))?,
        },
        receipts,
    )))
}

fn included_outcome(
    inclusion: PrivateActionInclusion,
    receipts: &[Receipt],
) -> PrivateActionOutcome {
    let accepted = !receipts.is_empty() && receipts.iter().all(|receipt| receipt.accepted);
    let first_rejected = receipts.iter().find(|receipt| !receipt.accepted);
    PrivateActionOutcome {
        accepted,
        code: first_rejected
            .or_else(|| receipts.first())
            .map(|receipt| receipt.code.clone())
            .unwrap_or_else(|| "included".to_string()),
        message: first_rejected
            .map(|receipt| receipt.message.clone())
            .unwrap_or_default(),
        fee_charged: receipts.iter().map(|receipt| receipt.fee_charged).sum(),
        state_effects: "applied".to_string(),
        inclusion: Some(inclusion),
    }
}

pub fn private_action_receipt(
    data_dir: &Path,
    intent_id: &str,
) -> io::Result<PrivateActionReceiptReport> {
    if !is_lower_hex(intent_id, 96) {
        return Err(refused(
            "private_action_request_invalid",
            "intent_id must be 96 lowercase hex characters",
        ));
    }
    let store = PrivateActionAdmissionStore::open(data_dir)?;
    let Some(record) = store.get(intent_id).cloned() else {
        return Ok(PrivateActionReceiptReport {
            schema: PRIVATE_ACTION_RECEIPT_SCHEMA.to_string(),
            intent_id: intent_id.to_string(),
            state: "pending_unknown".to_string(),
            record: None,
            contract_receipt: None,
        });
    };
    drop(store);
    let mut record = record;
    // Read-only: the chain is authoritative for an included batch even before
    // the round service records it.
    if record.outcome.is_none() {
        if let Some((inclusion, receipts)) =
            private_action_chain_inclusion(data_dir, &record.batch_id)?
        {
            let outcome = included_outcome(inclusion, &receipts);
            record.state = if outcome.accepted {
                PrivateActionState::Accepted
            } else {
                PrivateActionState::Rejected
            };
            record.outcome = Some(outcome);
        }
    }
    Ok(PrivateActionReceiptReport {
        schema: PRIVATE_ACTION_RECEIPT_SCHEMA.to_string(),
        intent_id: intent_id.to_string(),
        state: serde_json::to_value(record.state)
            .ok()
            .and_then(|value| value.as_str().map(str::to_string))
            .unwrap_or_default(),
        contract_receipt: Some(contract_receipt(&record)),
        record: Some(record),
    })
}

/// What one dispatch of the reserved batch produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrivateActionDispatch {
    /// The round finished; the outcome is read from the chain by batch id.
    Finished,
    /// This node is not the proposer for the next height; the record stays queued.
    NotProposer,
    /// The round refused the batch with a code; nothing was committed.
    Refused { code: String, message: String },
    /// The round failed without a known result.
    Failed { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivateActionServiceStep {
    pub intent_id: String,
    pub batch_id: String,
    pub from: PrivateActionState,
    pub to: PrivateActionState,
    pub code: String,
}

/// One pass of the round service.
///
/// 1. `dispatched` records (a crash mid-round) are reconciled from the chain by
///    batch id and never dispatched again: included → accepted/rejected,
///    absent → `unknown`, which stays pending until the batch appears on
///    chain (for example proposed by another validator holding it queued).
/// 2. `queued` records whose batch is already on chain (another validator
///    proposed it) are reconciled without a round.
/// 3. The oldest other `queued` record is re-verified at the current state;
///    a failure is a final rejection with the execution's code. Otherwise,
///    if `is_local_proposer` holds, it is written `dispatched` (fsync'd) and
///    the exact reserved batch is handed to `dispatch`; the result is written.
pub fn private_action_service_step<P, D>(
    data_dir: &Path,
    mut is_local_proposer: P,
    mut dispatch: D,
) -> io::Result<Vec<PrivateActionServiceStep>>
where
    P: FnMut(u64) -> io::Result<bool>,
    D: FnMut(&PrivateActionRecord, &ShieldedActionBatch) -> PrivateActionDispatch,
{
    let mut steps = Vec::new();
    let mut store = PrivateActionAdmissionStore::open(data_dir)?;
    let pending = store
        .records()
        .filter(|record| {
            matches!(
                record.state,
                PrivateActionState::Queued
                    | PrivateActionState::Dispatched
                    | PrivateActionState::Unknown
            )
        })
        .cloned()
        .collect::<Vec<_>>();
    let mut next = None;
    for record in pending {
        if let Some((inclusion, receipts)) =
            private_action_chain_inclusion(data_dir, &record.batch_id)?
        {
            steps.push(finish(
                &mut store,
                record,
                included_outcome(inclusion, &receipts),
            )?);
            continue;
        }
        // `unknown` is only ever settled by the chain; it is never dispatched.
        if record.state == PrivateActionState::Unknown {
            continue;
        }
        if record.state == PrivateActionState::Dispatched {
            let mut updated = record.clone();
            updated.state = PrivateActionState::Unknown;
            store.write(updated)?;
            steps.push(PrivateActionServiceStep {
                intent_id: record.intent_id,
                batch_id: record.batch_id,
                from: PrivateActionState::Dispatched,
                to: PrivateActionState::Unknown,
                code: "private_action_dispatch_interrupted".to_string(),
            });
            continue;
        }
        let older = next.as_ref().is_none_or(|current: &PrivateActionRecord| {
            record.received_height < current.received_height
        });
        if older {
            next = Some(record);
        }
    }
    let Some(record) = next else {
        return Ok(steps);
    };
    let batch: ShieldedActionBatch = serde_json::from_str(&record.batch_json)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let state = read_finalized_admission_state(data_dir)?;
    consensus_artifacts::verify_shielded_action_batch_id(&state.genesis, &batch)?;
    if let Err((code, message)) = verify_private_batch(&state, &batch)? {
        let outcome = refused_outcome(code, message);
        steps.push(finish(&mut store, record, outcome)?);
        return Ok(steps);
    }
    let next_height = state
        .tip_height
        .checked_add(1)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "block height overflow"))?;
    if !is_local_proposer(next_height)? {
        return Ok(steps);
    }
    let mut dispatched = record.clone();
    dispatched.state = PrivateActionState::Dispatched;
    dispatched.dispatch_count = dispatched.dispatch_count.saturating_add(1);
    store.write(dispatched.clone())?;
    // The round runs outside the admission lock; the durable `dispatched`
    // state keeps any other pass from dispatching this record again.
    drop(store);
    let result = dispatch(&dispatched, &batch);
    let mut store = PrivateActionAdmissionStore::open(data_dir)?;
    let included = private_action_chain_inclusion(data_dir, &dispatched.batch_id)?;
    let step = match (included, result) {
        (Some((inclusion, receipts)), _) => finish(
            &mut store,
            dispatched,
            included_outcome(inclusion, &receipts),
        )?,
        (None, PrivateActionDispatch::Refused { code, message }) => {
            finish(&mut store, dispatched, refused_outcome(code, message))?
        }
        (None, PrivateActionDispatch::NotProposer) => {
            let mut queued = dispatched.clone();
            queued.state = PrivateActionState::Queued;
            store.write(queued)?;
            PrivateActionServiceStep {
                intent_id: dispatched.intent_id,
                batch_id: dispatched.batch_id,
                from: PrivateActionState::Dispatched,
                to: PrivateActionState::Queued,
                code: "private_action_not_proposer".to_string(),
            }
        }
        (None, PrivateActionDispatch::Finished | PrivateActionDispatch::Failed { .. }) => {
            let mut unknown = dispatched.clone();
            unknown.state = PrivateActionState::Unknown;
            store.write(unknown)?;
            PrivateActionServiceStep {
                intent_id: dispatched.intent_id,
                batch_id: dispatched.batch_id,
                from: PrivateActionState::Dispatched,
                to: PrivateActionState::Unknown,
                code: "private_action_result_unknown".to_string(),
            }
        }
    };
    steps.push(step);
    Ok(steps)
}

fn refused_outcome(code: String, message: String) -> PrivateActionOutcome {
    PrivateActionOutcome {
        accepted: false,
        code,
        message,
        fee_charged: 0,
        state_effects: "none".to_string(),
        inclusion: None,
    }
}

fn finish(
    store: &mut PrivateActionAdmissionStore,
    record: PrivateActionRecord,
    outcome: PrivateActionOutcome,
) -> io::Result<PrivateActionServiceStep> {
    let from = record.state;
    let mut updated = record;
    updated.state = if outcome.accepted {
        PrivateActionState::Accepted
    } else {
        PrivateActionState::Rejected
    };
    let step = PrivateActionServiceStep {
        intent_id: updated.intent_id.clone(),
        batch_id: updated.batch_id.clone(),
        from,
        to: updated.state,
        code: outcome.code.clone(),
    };
    updated.outcome = Some(outcome);
    store.write(updated)?;
    Ok(step)
}

pub const PRIVATE_ACTION_RESOLVE_SCHEMA: &str = "postfiat.private_action_resolve.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivateActionResolveReport {
    pub schema: String,
    pub intent_id: String,
    /// `settled` (from the chain), `unchanged` or `abandoned`.
    pub action: String,
    pub record: PrivateActionRecord,
    /// Every logged record for the intent, oldest first.
    pub history: Vec<PrivateActionRecord>,
}

/// The operator rule for a stuck record. Without `abandon_reason`, settles
/// the record from the chain if its batch is there and otherwise changes
/// nothing. With it, marks a `queued` or `unknown` record `abandoned` only if
/// the batch is not on chain. Never dispatches. Holding the store lock means
/// no service step, and so no round, is running.
pub fn private_action_resolve(
    data_dir: &Path,
    intent_id: &str,
    abandon_reason: Option<&str>,
    now_unix: u64,
) -> io::Result<PrivateActionResolveReport> {
    let mut store = PrivateActionAdmissionStore::open(data_dir)?;
    let record = store.get(intent_id).cloned().ok_or_else(|| {
        refused(
            "private_action_unknown_intent",
            "this node holds no admission record for the intent",
        )
    })?;
    let on_chain = private_action_chain_inclusion(data_dir, &record.batch_id)?;
    let (action, record) = match abandon_reason {
        None => match on_chain {
            Some((inclusion, receipts))
                if !matches!(
                    record.state,
                    PrivateActionState::Accepted | PrivateActionState::Rejected
                ) =>
            {
                finish(&mut store, record, included_outcome(inclusion, &receipts))?;
                let settled = store.get(intent_id).cloned().expect("settled record");
                ("settled", settled)
            }
            _ => ("unchanged", record),
        },
        Some(reason) => {
            let reason = reason.trim();
            if reason.is_empty() {
                return Err(refused(
                    "private_action_request_invalid",
                    "--abandon needs a non-empty --reason",
                ));
            }
            if on_chain.is_some() {
                return Err(refused(
                    "private_action_on_chain",
                    "the batch is on chain; resolve without --abandon to settle the record",
                ));
            }
            match record.state {
                PrivateActionState::Queued | PrivateActionState::Unknown => {}
                PrivateActionState::Dispatched => {
                    return Err(refused(
                        "private_action_round_in_flight",
                        "the record is dispatched; let the round service settle it first",
                    ))
                }
                _ => {
                    return Err(refused(
                        "private_action_final",
                        "the record is already accepted, rejected or abandoned",
                    ))
                }
            }
            let mut abandoned = record;
            abandoned.state = PrivateActionState::Abandoned;
            abandoned.outcome = Some(PrivateActionOutcome {
                accepted: false,
                code: "private_action_abandoned".to_string(),
                message: format!("abandoned by the operator at unix {now_unix}: {reason}"),
                fee_charged: 0,
                state_effects: "none".to_string(),
                inclusion: None,
            });
            store.write(abandoned.clone())?;
            ("abandoned", abandoned)
        }
    };
    Ok(PrivateActionResolveReport {
        schema: PRIVATE_ACTION_RESOLVE_SCHEMA.to_string(),
        intent_id: intent_id.to_string(),
        action: action.to_string(),
        history: store.history(intent_id)?,
        record,
    })
}
