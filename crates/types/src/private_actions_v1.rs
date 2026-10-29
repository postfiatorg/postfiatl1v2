// Private actions contract v1 (W-1): the request and response types of the
// gateway's snapshot, submit and receipt routes, shared by the gateway and the
// node. Fixtures: crates/node/tests/fixtures/private-actions-v1/.

pub const PRIVATE_ACTIONS_SCHEMA_V1: &str = "postfiat-private-actions-v1";
/// The wallet's intent hash domain (tasknode-apple `core/src/private_actions.rs`).
pub const PRIVATE_INTENT_HASH_DOMAIN: &str = "tasknode.private-intent.v1";
/// Deduplication hash of the canonical typed action, independent of the intent.
pub const PRIVATE_ACTION_HASH_DOMAIN: &str = "postfiat.private-actions.action.v1";
/// The exact top-level fields of a public `shielded_swap_v1` `swap_json`.
pub const PRIVATE_SWAP_JSON_FIELDS: [&str; 18] = [
    "version",
    "schema",
    "pool_id",
    "proof_system_id",
    "circuit_id",
    "pool_domain",
    "anchor",
    "nullifiers",
    "randomized_verification_keys",
    "output_commitments",
    "encrypted_outputs",
    "accounting_inputs",
    "accounting_outputs",
    "pricing_claim",
    "swap_binding_hash",
    "fee",
    "proof",
    "spend_authorization_signatures",
];

/// What an intent id binds besides the action text. Field order is part of the
/// hash preimage and matches the wallet's `PrivateDomain`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrivateActionDomain {
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrivateSnapshotRequest {
    pub address: String,
}

/// The body of both `submit` and `receipt`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrivateActionRequest {
    pub address: String,
    pub intent_id: String,
    pub action_json: String,
}

/// One finalized view: every page of a snapshot carries the same token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrivateSnapshotToken {
    pub height: u64,
    pub block_hash: String,
    pub state_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrivateSnapshotOutput {
    pub commitment: String,
    /// Lowercase hex of the encrypted output bytes.
    pub ciphertext: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrivateSnapshotAsset {
    pub asset_id: String,
    pub issuer: String,
    pub code: String,
    pub precision: u8,
    pub requires_authorization: bool,
    pub proof_profile_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrivateSnapshotCompleteness {
    pub commitments: bool,
    pub nullifiers: bool,
    pub accepted_anchors: bool,
    pub outputs: bool,
    pub assets: bool,
}

/// Next-page cursors; `None` once the list is complete.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrivateSnapshotCursors {
    pub commitments: Option<String>,
    pub nullifiers: Option<String>,
    pub accepted_anchors: Option<String>,
    pub outputs: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrivateSnapshotResponse {
    pub schema: String,
    pub domain: PrivateActionDomain,
    pub address: String,
    pub snapshot_token: PrivateSnapshotToken,
    pub commitments: Vec<String>,
    pub nullifiers: Vec<String>,
    pub accepted_anchors: Vec<String>,
    pub outputs: Vec<PrivateSnapshotOutput>,
    pub assets: Vec<PrivateSnapshotAsset>,
    pub base_asset_id: String,
    pub quote_asset_id: String,
    pub pricing_claim_json: Option<String>,
    pub policy_id: Option<String>,
    pub disclosure_hash: Option<String>,
    pub complete: PrivateSnapshotCompleteness,
    pub cursors: PrivateSnapshotCursors,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrivateAdmissionState {
    Queued,
    Dispatched,
    Accepted,
    Rejected,
    Unknown,
    /// Abandoned by the operator; the same action bytes may be resubmitted
    /// under the same intent.
    Abandoned,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrivateSubmitResponse {
    pub schema: String,
    pub intent_id: String,
    pub state: PrivateAdmissionState,
    pub batch_id: String,
    pub action_hash: String,
}

/// Finality the wallet can check itself: the block certificate of a validator
/// quorum, in the node's certificate serialization, for the inclusion block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrivateFinalityEvidence {
    pub kind: String,
    pub height: u64,
    pub block_hash: String,
    pub state_root: String,
    pub quorum: u32,
    pub validator_count: u32,
    pub certificate_json: String,
}

/// Bindings are immutable once set; outcome fields stay `None` while pending.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrivateReceiptResponse {
    pub schema: String,
    pub intent_id: String,
    pub state: PrivateAdmissionState,
    pub action_hash: String,
    pub batch_id: String,
    pub transaction_id: Option<String>,
    pub inclusion_height: Option<u64>,
    pub block_hash: Option<String>,
    pub state_root: Option<String>,
    pub accepted: Option<bool>,
    pub code: Option<String>,
    pub fee: Option<u64>,
    pub finality: Option<PrivateFinalityEvidence>,
    /// Only for `abandoned`: the operator's reason and Unix time. Omitted
    /// otherwise, so every other receipt serializes as before.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub abandon_reason: Option<String>,
}

fn private_actions_hash_hex(domain: &str, bytes: &[u8]) -> String {
    let mut hasher = Sha3_384::new();
    hasher.update(domain.as_bytes());
    hasher.update([0u8]);
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// The wallet's intent id: SHA3-384 under `tasknode.private-intent.v1` over the
/// JSON pair `(domain, action_json)`, 96 lowercase hex.
pub fn private_intent_id(
    domain: &PrivateActionDomain,
    action_json: &str,
) -> Result<String, serde_json::Error> {
    let bytes = serde_json::to_vec(&(domain, action_json))?;
    Ok(private_actions_hash_hex(PRIVATE_INTENT_HASH_DOMAIN, &bytes))
}

/// The deduplication hash of a strictly parsed action's canonical bytes.
pub fn private_action_hash(action: &ShieldedAction) -> Result<String, serde_json::Error> {
    let bytes = serde_json::to_vec(action)?;
    Ok(private_actions_hash_hex(PRIVATE_ACTION_HASH_DOMAIN, &bytes))
}

/// Strictly decodes a public action: one of the three user-proven kinds, no
/// unknown fields at either JSON level, and the exact swap field set.
pub fn parse_private_public_action(action_json: &str) -> Result<ShieldedAction, &'static str> {
    let value: serde_json::Value =
        serde_json::from_str(action_json).map_err(|_| "private_action_malformed")?;
    let action: ShieldedAction =
        serde_json::from_value(value.clone()).map_err(|_| "private_action_malformed")?;
    match &action {
        ShieldedAction::AssetOrchardIngressV2(_)
        | ShieldedAction::AssetOrchardPrivateEgressV1(_) => {}
        ShieldedAction::ShieldedSwapV1(swap) => {
            let inner: serde_json::Value =
                serde_json::from_str(&swap.swap_json).map_err(|_| "private_swap_json_malformed")?;
            let fields = inner.as_object().ok_or("private_swap_json_malformed")?;
            if fields.len() != PRIVATE_SWAP_JSON_FIELDS.len()
                || fields
                    .keys()
                    .any(|key| !PRIVATE_SWAP_JSON_FIELDS.contains(&key.as_str()))
            {
                return Err("private_swap_json_fields_refused");
            }
        }
        ShieldedAction::AssetOrchardIngressV1(_) | ShieldedAction::AssetOrchardEgressV1(_) => {
            return Err("private_action_legacy_refused");
        }
        _ => return Err("private_action_kind_refused"),
    }
    if serde_json::to_value(&action).map_err(|_| "private_action_malformed")? != value {
        return Err("private_action_unknown_fields");
    }
    Ok(action)
}
