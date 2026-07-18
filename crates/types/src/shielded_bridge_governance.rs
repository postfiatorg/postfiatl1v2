
pub const DEFAULT_SHIELDED_ASSET_ID: &str = "POSTFIAT";
pub const GOVERNANCE_KIND_VALIDATOR_SET: &str = "validator_set";
pub const GOVERNANCE_KIND_CRYPTO_POLICY: &str = "crypto_policy";
pub const GOVERNANCE_KIND_BRIDGE_WITNESS_EPOCH: &str = "bridge_witness_epoch";
pub const GOVERNANCE_KIND_AUTHORITY_MODE: &str = "authority_mode";
pub const GOVERNANCE_KIND_BRIDGE_VERIFICATION_ACTIVATION_HEIGHT: &str =
    "bridge_verification_activation_height";
pub const GOVERNANCE_KIND_VAULT_BRIDGE_ROUTE_AUTHORITY_ACTIVATION_HEIGHT: &str =
    "vault_bridge_route_authority_activation_height";
pub const GOVERNANCE_VAULT_BRIDGE_ROUTE_KIND_PREFIX_V1: &str = "vault_bridge_route_v1";
pub const VAULT_BRIDGE_ROUTE_PROFILE_ACTIVATION_SCHEMA_V1: &str =
    "postfiat.vault_bridge.route_profile_activation.v1";
pub const VAULT_BRIDGE_ROUTE_PROFILE_RECORD_SCHEMA_V1: &str =
    "postfiat.vault_bridge.route_profile_record.v1";
pub const GOVERNANCE_KIND_ATOMIC_SWAP_ACTIVATION_HEIGHT: &str =
    "atomic_swap_activation_height";
pub const GOVERNANCE_KIND_REPLICATED_STATE_V2_ACTIVATION_HEIGHT: &str =
    "replicated_state_v2_activation_height";
pub const GOVERNANCE_KIND_BRIDGE_EXIT_ROOT_ACTIVATION_HEIGHT: &str =
    "bridge_exit_root_activation_height";
pub const GOVERNANCE_KIND_ATOMIC_SWAP_PAUSE: &str = "atomic_swap_pause";
pub const GOVERNANCE_KIND_ORCHARD_POOL_PAUSE: &str = "orchard_pool_pause";
pub const GOVERNANCE_AUTHORITY_MODE_FOUNDATION: u32 = 0;
pub const GOVERNANCE_AUTHORITY_MODE_COBALT_RATIFIED: u32 = 1;
pub const GOVERNANCE_AMENDMENT_ACTIVATION_SCHEMA: &str =
    "postfiat.governance_amendment_activation.v1";
pub const GOVERNANCE_AMENDMENT_SUPERSESSION_SCHEMA: &str =
    "postfiat.governance_amendment_supersession.v1";
pub const GOVERNANCE_AMENDMENT_ROLLBACK_SCHEMA: &str = "postfiat.governance_amendment_rollback.v1";
pub const GOVERNANCE_AGENT_DRY_RUN_AMENDMENT_SCHEMA: &str =
    "postfiat.governance_agent_dry_run_amendment.v1";
pub const GOVERNANCE_AGENT_DRY_RUN_RECORD_SCHEMA: &str =
    "postfiat.governance_agent_dry_run_record.v1";
pub const GOVERNANCE_AGENT_ACTION_MODE_DRY_RUN_VALIDATE: &str = "DryRunValidate";
pub const TURNSTILE_KIND_BOOTSTRAP_DEPOSIT: &str = "bootstrap_deposit";
pub const TURNSTILE_KIND_POOL_MIGRATION: &str = "pool_migration";
pub const TURNSTILE_KIND_ORCHARD_DEPOSIT: &str = "orchard_deposit";
pub const DEBUG_SHIELDED_POOL_ID: &str = "debug-shielded-pool-v1";

mod u128_hex_serde {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(value: &u128, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("{value:032x}"))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<u128, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        if value.len() != 32 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(serde::de::Error::custom(
                "expected a 32-character hexadecimal u128 string",
            ));
        }
        u128::from_str_radix(&value, 16).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShieldedNote {
    pub note_id: String,
    pub commitment: String,
    pub position: u64,
    pub owner: String,
    pub asset_id: String,
    pub value: u64,
    pub rho: String,
    pub memo: String,
    pub created_by: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShieldedState {
    #[serde(default)]
    pub next_note_position: u64,
    pub notes: Vec<ShieldedNote>,
    pub nullifiers: Vec<String>,
    #[serde(default)]
    pub turnstile_events: Vec<TurnstileEvent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orchard: Option<OrchardPoolState>,
}

impl ShieldedState {
    pub fn empty() -> Self {
        Self {
            next_note_position: 0,
            notes: Vec::new(),
            nullifiers: Vec::new(),
            turnstile_events: Vec::new(),
            orchard: None,
        }
    }

    pub fn note(&self, note_id: &str) -> Option<&ShieldedNote> {
        self.notes.iter().find(|note| note.note_id == note_id)
    }

    pub fn is_nullified(&self, nullifier: &str) -> bool {
        self.nullifiers.iter().any(|existing| existing == nullifier)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchardPoolState {
    pub pool_id: String,
    pub nullifiers: Vec<String>,
    pub output_commitments: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frontier_cache: Option<OrchardFrontierCache>,
    pub encrypted_outputs: Vec<OrchardEncryptedOutputRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub asset_commitment_records: Vec<OrchardAssetCommitmentRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub asset_orchard_outputs: Vec<AssetOrchardEncryptedOutputRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub asset_orchard_balances: Vec<AssetOrchardAssetBalance>,
    #[serde(default)]
    pub root_history: Vec<OrchardRootRecord>,
    pub accepted_anchors: Vec<String>,
    #[serde(default)]
    pub value_balance_total: i64,
    #[serde(default)]
    pub turnstile_deposit_total: u64,
    #[serde(default)]
    pub fee_burn_total: u64,
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub withdraw_total: u64,
}

impl OrchardPoolState {
    pub fn empty(pool_id: impl Into<String>) -> Self {
        Self {
            pool_id: pool_id.into(),
            nullifiers: Vec::new(),
            output_commitments: Vec::new(),
            frontier_cache: None,
            encrypted_outputs: Vec::new(),
            asset_commitment_records: Vec::new(),
            asset_orchard_outputs: Vec::new(),
            asset_orchard_balances: Vec::new(),
            root_history: Vec::new(),
            accepted_anchors: Vec::new(),
            value_balance_total: 0,
            turnstile_deposit_total: 0,
            fee_burn_total: 0,
            withdraw_total: 0,
        }
    }

    pub fn is_nullified(&self, nullifier: &str) -> bool {
        self.nullifiers.iter().any(|existing| existing == nullifier)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchardFrontierCache {
    pub output_count: u64,
    pub root: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_leaf: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ommers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchardRootRecord {
    pub root: String,
    pub output_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchardEncryptedOutputRecord {
    pub cmx: String,
    pub epk: String,
    pub enc_ciphertext: String,
    pub out_ciphertext: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compact_ciphertext: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchardAssetCommitmentRecord {
    pub output_commitment: String,
    pub asset_commitment: String,
    pub value_commitment: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetOrchardEncryptedOutputRecord {
    pub output_commitment: String,
    pub encrypted_output: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetOrchardAssetBalance {
    pub asset_id: String,
    pub ingress_total: u64,
    pub egress_total: u64,
    pub live_total: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnstileEvent {
    pub event_id: String,
    pub kind: String,
    pub owner: String,
    pub asset_id: String,
    pub amount: u64,
    pub note_id: String,
    pub source_pool: String,
    pub target_pool: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnstileSummary {
    pub event_count: u64,
    pub bootstrap_deposit_total: u64,
    pub migration_total: u64,
    pub orchard_deposit_total: u64,
    pub events: Vec<TurnstileEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShieldedSpendResult {
    pub spend_id: String,
    pub spent_note_id: String,
    pub nullifier: String,
    pub outputs: Vec<ShieldedNote>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShieldMintAction {
    pub owner: String,
    pub asset_id: String,
    pub amount: u64,
    pub memo: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShieldSpendAction {
    pub note_id: String,
    pub to: String,
    pub amount: u64,
    pub memo: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShieldMigrateAction {
    pub note_id: String,
    pub target_pool: String,
    pub memo: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchardActionPayload {
    pub action_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchardWithdrawActionPayload {
    pub action_json: String,
    pub to: String,
    pub amount: u64,
    pub fee: u64,
    pub policy_id: String,
    pub disclosure_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchardDepositActionPayload {
    pub action_json: String,
    pub funding_transfer: SignedTransfer,
    pub amount: u64,
    pub fee: u64,
    pub policy_id: String,
    pub disclosure_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShieldedSwapActionPayload {
    pub swap_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetOrchardIngressNote {
    pub diversifier: String,
    pub g_d: String,
    pub pk_d: String,
    #[serde(with = "u128_hex_serde")]
    pub asset_tag_lo: u128,
    #[serde(with = "u128_hex_serde")]
    pub asset_tag_hi: u128,
    pub value: u64,
    pub rho: String,
    pub psi: String,
    pub rcm: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetOrchardIngressActionPayload {
    pub burn_transaction: SignedAssetTransaction,
    pub pool_id: String,
    pub asset_id: String,
    pub amount: u64,
    pub output_commitment: String,
    pub encrypted_output: String,
    pub note: AssetOrchardIngressNote,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetOrchardIngressV2ActionPayload {
    pub burn_transaction: SignedAssetTransaction,
    pub pool_id: String,
    pub asset_id: String,
    pub amount: u64,
    pub output_commitment: String,
    pub encrypted_output: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetOrchardEgressActionPayload {
    pub pool_id: String,
    pub to: String,
    pub asset_id: String,
    pub amount: u64,
    pub output_commitment: String,
    pub nullifier: String,
    pub note: AssetOrchardIngressNote,
    pub nk: String,
    pub rivk: String,
    pub spend_auth_verification_key: String,
    pub spend_auth_randomizer: String,
    pub randomized_verification_key: String,
    pub spend_authorization_signature: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetOrchardPrivateEgressActionPayload {
    pub version: u16,
    pub schema: String,
    pub pool_id: String,
    pub to: String,
    pub asset_id: String,
    pub amount: u64,
    pub fee: u64,
    pub policy_id: String,
    pub disclosure_hash: String,
    pub proof_system_id: String,
    pub circuit_id: String,
    pub pool_domain: String,
    pub anchor: String,
    pub nullifier: String,
    pub randomized_verification_key: String,
    #[serde(with = "u128_hex_serde")]
    pub asset_tag_lo: u128,
    #[serde(with = "u128_hex_serde")]
    pub asset_tag_hi: u128,
    pub exit_binding_hash: String,
    pub proof: String,
    pub spend_authorization_signature: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
#[allow(clippy::large_enum_variant)]
pub enum ShieldedAction {
    #[serde(rename = "shield_mint")]
    Mint(ShieldMintAction),
    #[serde(rename = "shield_spend")]
    Spend(ShieldSpendAction),
    #[serde(rename = "shield_migrate")]
    Migrate(ShieldMigrateAction),
    #[serde(rename = "orchard_action_v1")]
    OrchardV1(OrchardActionPayload),
    #[serde(rename = "orchard_withdraw_v1")]
    OrchardWithdrawV1(OrchardWithdrawActionPayload),
    #[serde(rename = "orchard_deposit_v1")]
    OrchardDepositV1(OrchardDepositActionPayload),
    #[serde(rename = "shielded_swap_v1")]
    ShieldedSwapV1(ShieldedSwapActionPayload),
    #[serde(rename = "asset_orchard_ingress_v1")]
    AssetOrchardIngressV1(AssetOrchardIngressActionPayload),
    #[serde(rename = "asset_orchard_ingress_v2")]
    AssetOrchardIngressV2(AssetOrchardIngressV2ActionPayload),
    #[serde(rename = "asset_orchard_egress_v1")]
    AssetOrchardEgressV1(AssetOrchardEgressActionPayload),
    #[serde(rename = "asset_orchard_private_egress_v1")]
    AssetOrchardPrivateEgressV1(AssetOrchardPrivateEgressActionPayload),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShieldedActionBatch {
    pub batch_id: String,
    pub actions: Vec<ShieldedAction>,
}

impl ShieldedActionBatch {
    pub fn new(batch_id: impl Into<String>, actions: Vec<ShieldedAction>) -> Self {
        Self {
            batch_id: batch_id.into(),
            actions,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShieldedDisclosure {
    pub note: ShieldedNote,
    pub nullifier: String,
    pub spent: bool,
}

pub const DEFAULT_BRIDGE_DOMAIN_ID: &str = "local-sim";
pub const BRIDGE_DIRECTION_INBOUND: &str = "inbound";
pub const BRIDGE_DIRECTION_OUTBOUND: &str = "outbound";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeDomain {
    pub domain_id: String,
    pub name: String,
    #[serde(default)]
    pub source_chain: String,
    #[serde(default)]
    pub target_chain: String,
    #[serde(default)]
    pub bridge_id: String,
    #[serde(default)]
    pub door_account: String,
    pub inbound_cap: u64,
    pub outbound_cap: u64,
    pub inbound_used: u64,
    pub outbound_used: u64,
    pub paused: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeDomainSpec {
    pub domain_id: String,
    pub name: String,
    pub source_chain: String,
    pub target_chain: String,
    pub bridge_id: String,
    pub door_account: String,
    pub inbound_cap: u64,
    pub outbound_cap: u64,
}

impl BridgeDomainSpec {
    pub fn new(
        domain_id: impl Into<String>,
        name: impl Into<String>,
        inbound_cap: u64,
        outbound_cap: u64,
    ) -> Self {
        let domain_id = domain_id.into();
        let source_chain = domain_id.clone();
        let target_chain = "postfiat-local".to_string();
        let bridge_id = domain_id.clone();
        let door_account = format!("door:{domain_id}");
        Self {
            domain_id,
            name: name.into(),
            source_chain,
            target_chain,
            bridge_id,
            door_account,
            inbound_cap,
            outbound_cap,
        }
    }
}

impl BridgeDomain {
    pub fn new(
        domain_id: impl Into<String>,
        name: impl Into<String>,
        inbound_cap: u64,
        outbound_cap: u64,
    ) -> Self {
        Self::with_metadata(BridgeDomainSpec::new(
            domain_id,
            name,
            inbound_cap,
            outbound_cap,
        ))
    }

    pub fn with_metadata(spec: BridgeDomainSpec) -> Self {
        Self {
            domain_id: spec.domain_id,
            name: spec.name,
            source_chain: spec.source_chain,
            target_chain: spec.target_chain,
            bridge_id: spec.bridge_id,
            door_account: spec.door_account,
            inbound_cap: spec.inbound_cap,
            outbound_cap: spec.outbound_cap,
            inbound_used: 0,
            outbound_used: 0,
            paused: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeWitnessAttestation {
    pub attestation_id: String,
    #[serde(default)]
    pub chain_id: String,
    #[serde(default)]
    pub genesis_hash: String,
    #[serde(default)]
    pub protocol_version: u32,
    pub signer: String,
    pub algorithm_id: String,
    pub public_key_hex: String,
    pub signature_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeTransfer {
    pub transfer_id: String,
    pub domain_id: String,
    #[serde(default)]
    pub source_chain: String,
    #[serde(default)]
    pub target_chain: String,
    #[serde(default)]
    pub bridge_id: String,
    #[serde(default)]
    pub door_account: String,
    pub direction: String,
    pub from: String,
    pub to: String,
    pub asset_id: String,
    pub amount: u64,
    pub witness_id: String,
    pub witness_epoch: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub witness_attestation: Option<BridgeWitnessAttestation>,
    pub sequence: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeDomainAction {
    pub domain_id: String,
    pub name: String,
    #[serde(default)]
    pub source_chain: String,
    #[serde(default)]
    pub target_chain: String,
    #[serde(default)]
    pub bridge_id: String,
    #[serde(default)]
    pub door_account: String,
    pub inbound_cap: u64,
    pub outbound_cap: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeTransferAction {
    pub domain_id: String,
    pub direction: String,
    pub from: String,
    pub to: String,
    pub asset_id: String,
    pub amount: u64,
    pub witness_id: String,
    pub witness_epoch: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub witness_attestation: Option<BridgeWitnessAttestation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgePauseAction {
    pub domain_id: String,
    pub paused: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum BridgeAction {
    #[serde(rename = "bridge_domain")]
    Domain(BridgeDomainAction),
    #[serde(rename = "bridge_transfer")]
    Transfer(BridgeTransferAction),
    #[serde(rename = "bridge_pause")]
    Pause(BridgePauseAction),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeActionBatch {
    pub batch_id: String,
    pub actions: Vec<BridgeAction>,
}

impl BridgeActionBatch {
    pub fn new(batch_id: impl Into<String>, actions: Vec<BridgeAction>) -> Self {
        Self {
            batch_id: batch_id.into(),
            actions,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeState {
    pub domains: Vec<BridgeDomain>,
    pub transfers: Vec<BridgeTransfer>,
    pub replay_cache: Vec<String>,
}

pub const BRIDGE_REPLAY_CACHE_MAX_ENTRIES: usize = 4_096;
pub const BRIDGE_REPLAY_CACHE_EPOCH_RETENTION: u32 = 2;

impl BridgeState {
    pub fn empty() -> Self {
        Self {
            domains: Vec::new(),
            transfers: Vec::new(),
            replay_cache: Vec::new(),
        }
    }

    pub fn domain(&self, domain_id: &str) -> Option<&BridgeDomain> {
        self.domains
            .iter()
            .find(|domain| domain.domain_id == domain_id)
    }

    pub fn domain_mut(&mut self, domain_id: &str) -> Option<&mut BridgeDomain> {
        self.domains
            .iter_mut()
            .find(|domain| domain.domain_id == domain_id)
    }

    pub fn has_witness(&self, witness_id: &str) -> bool {
        self.replay_cache
            .iter()
            .any(|existing| existing == witness_id)
    }

    pub fn has_witness_replay(
        &self,
        domain_id: &str,
        witness_epoch: u32,
        witness_id: &str,
    ) -> bool {
        let replay_key = bridge_witness_replay_key(domain_id, witness_epoch, witness_id);
        self.has_witness(&replay_key)
            || self.transfers.iter().any(|transfer| {
                transfer.domain_id == domain_id
                    && transfer.witness_epoch == witness_epoch
                    && transfer.witness_id == witness_id
            })
    }

    pub fn record_witness_replay(&mut self, domain_id: &str, witness_epoch: u32, witness_id: &str) {
        let replay_key = bridge_witness_replay_key(domain_id, witness_epoch, witness_id);
        if !self.has_witness(&replay_key) {
            self.replay_cache.push(replay_key);
        }
        self.prune_replay_cache(domain_id, witness_epoch);
    }

    fn prune_replay_cache(&mut self, domain_id: &str, witness_epoch: u32) {
        let minimum_epoch = witness_epoch.saturating_sub(BRIDGE_REPLAY_CACHE_EPOCH_RETENTION);
        self.replay_cache.retain(|entry| {
            let Some((entry_domain, entry_epoch, _)) = parse_bridge_witness_replay_key(entry)
            else {
                return false;
            };
            entry_domain != domain_id || entry_epoch >= minimum_epoch
        });
        if self.replay_cache.len() > BRIDGE_REPLAY_CACHE_MAX_ENTRIES {
            let remove_count = self.replay_cache.len() - BRIDGE_REPLAY_CACHE_MAX_ENTRIES;
            self.replay_cache.drain(..remove_count);
        }
    }
}

fn bridge_witness_replay_key(domain_id: &str, witness_epoch: u32, witness_id: &str) -> String {
    format!("{domain_id}:{witness_epoch}:{witness_id}")
}

fn parse_bridge_witness_replay_key(value: &str) -> Option<(&str, u32, &str)> {
    let (domain_id, rest) = value.split_once(':')?;
    let (epoch, witness_id) = rest.split_once(':')?;
    Some((domain_id, epoch.parse().ok()?, witness_id))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultBridgeRouteProfileActivationV1 {
    pub schema: String,
    pub profile: VaultBridgeRouteProfileV1,
    pub amendment: GovernanceAmendment,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tier4_finality_bootstrap: Option<EthereumArbitrumFinalityStateV2>,
}

impl VaultBridgeRouteProfileActivationV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != VAULT_BRIDGE_ROUTE_PROFILE_ACTIVATION_SCHEMA_V1 {
            return Err("vault bridge route profile activation schema mismatch".to_string());
        }
        self.profile.validate()?;
        let profile_hash = self.profile.profile_hash()?;
        match (
            self.profile.verifier_kind.as_str(),
            self.tier4_finality_bootstrap.as_ref(),
        ) {
            (NAV_PROFILE_VERIFIER_SP1_ARBITRUM_FINALITY_V1, Some(state)) => {
                state.validate()?;
                if state.route_profile_hash != profile_hash
                    || state.route_epoch != u64::from(self.profile.route_epoch)
                    || state.arbitrum_chain_id != self.profile.source_chain_id
                    || state.vault_address != self.profile.vault_address
                    || state.vault_runtime_code_hash != self.profile.vault_runtime_code_hash
                    || state.token_address != self.profile.token_address
                    || state.token_runtime_code_hash != self.profile.token_runtime_code_hash
                {
                    return Err("Tier-4 route finality bootstrap does not match route profile"
                        .to_string());
                }
            }
            (NAV_PROFILE_VERIFIER_SP1_ARBITRUM_FINALITY_V1, None) => {
                return Err("Tier-4 route activation requires a finality bootstrap".to_string());
            }
            (_, Some(_)) => {
                return Err("non-Tier-4 route activation cannot carry a finality bootstrap"
                    .to_string());
            }
            (_, None) => {}
        }
        let expected_kind = vault_bridge_route_amendment_kind(&self.profile)?;
        if self.amendment.kind != expected_kind
            || self.amendment.value != self.profile.route_epoch
            || self.amendment.activation_height != self.profile.activation_height
            || self.amendment.paused
        {
            return Err(
                "vault bridge route profile does not match its governance amendment".to_string(),
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultBridgeRouteProfileRecordV1 {
    pub schema: String,
    pub profile_hash: String,
    pub profile: VaultBridgeRouteProfileV1,
    pub governance_amendment_id: String,
    pub authorized_height: u64,
}

impl VaultBridgeRouteProfileRecordV1 {
    pub fn new(
        activation: &VaultBridgeRouteProfileActivationV1,
        authorized_height: u64,
    ) -> Result<Self, String> {
        activation.validate()?;
        if authorized_height != activation.profile.activation_height {
            return Err(
                "vault bridge route profile must be recorded at its activation height"
                    .to_string(),
            );
        }
        Ok(Self {
            schema: VAULT_BRIDGE_ROUTE_PROFILE_RECORD_SCHEMA_V1.to_string(),
            profile_hash: activation.profile.profile_hash()?,
            profile: activation.profile.clone(),
            governance_amendment_id: activation.amendment.amendment_id.clone(),
            authorized_height,
        })
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema != VAULT_BRIDGE_ROUTE_PROFILE_RECORD_SCHEMA_V1 {
            return Err("vault bridge route profile record schema mismatch".to_string());
        }
        self.profile.validate()?;
        if self.profile_hash != self.profile.profile_hash()? {
            return Err("vault bridge route profile record hash mismatch".to_string());
        }
        if self.governance_amendment_id.is_empty()
            || self.authorized_height != self.profile.activation_height
        {
            return Err("vault bridge route profile record authorization mismatch".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceState {
    pub active_validator_count: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub active_validators: Vec<String>,
    #[serde(default = "default_crypto_policy_version")]
    pub crypto_policy_version: u32,
    #[serde(default = "default_bridge_witness_epoch")]
    pub bridge_witness_epoch: u32,
    #[serde(default = "default_authority_mode")]
    pub authority_mode: u32,
    #[serde(default, skip_serializing_if = "is_false")]
    pub orchard_pool_paused: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub atomic_swap_paused: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub validator_registry_updates: Vec<ValidatorRegistryUpdateRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub amendment_activation_records: Vec<GovernanceAmendmentActivationRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub amendment_supersession_records: Vec<GovernanceAmendmentSupersessionRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub amendment_rollback_records: Vec<GovernanceAmendmentRollbackRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub governance_agent_dry_run_records: Vec<GovernanceAgentDryRunRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vault_bridge_route_profiles: Vec<VaultBridgeRouteProfileRecordV1>,
    pub amendments: Vec<GovernanceAmendment>,
}

impl GovernanceState {
    pub fn new(active_validator_count: u32) -> Self {
        Self {
            active_validator_count,
            active_validators: Vec::new(),
            crypto_policy_version: default_crypto_policy_version(),
            bridge_witness_epoch: default_bridge_witness_epoch(),
            authority_mode: default_authority_mode(),
            orchard_pool_paused: false,
            atomic_swap_paused: false,
            validator_registry_updates: Vec::new(),
            amendment_activation_records: Vec::new(),
            amendment_supersession_records: Vec::new(),
            amendment_rollback_records: Vec::new(),
            governance_agent_dry_run_records: Vec::new(),
            vault_bridge_route_profiles: Vec::new(),
            amendments: Vec::new(),
        }
    }

    pub fn apply(&mut self, amendment: GovernanceAmendment) {
        match amendment.kind.as_str() {
            GOVERNANCE_KIND_VALIDATOR_SET => {
                self.active_validator_count = amendment.value;
                self.active_validators.clear();
            }
            GOVERNANCE_KIND_CRYPTO_POLICY => self.crypto_policy_version = amendment.value,
            GOVERNANCE_KIND_BRIDGE_WITNESS_EPOCH => self.bridge_witness_epoch = amendment.value,
            GOVERNANCE_KIND_AUTHORITY_MODE => self.authority_mode = amendment.value,
            GOVERNANCE_KIND_ORCHARD_POOL_PAUSE => self.orchard_pool_paused = amendment.value == 1,
            GOVERNANCE_KIND_ATOMIC_SWAP_PAUSE => self.atomic_swap_paused = amendment.value == 1,
            _ => {}
        }
        self.amendments.push(amendment);
    }

    pub fn bridge_verification_activation_height(&self) -> Option<u64> {
        self.amendments
            .iter()
            .rev()
            .find(|amendment| {
                amendment.kind == GOVERNANCE_KIND_BRIDGE_VERIFICATION_ACTIVATION_HEIGHT
            })
            .map(|amendment| u64::from(amendment.value))
    }

    pub fn vault_bridge_route_authority_activation_height(&self) -> Option<u64> {
        self.amendments
            .iter()
            .filter(|amendment| {
                amendment.kind
                    == GOVERNANCE_KIND_VAULT_BRIDGE_ROUTE_AUTHORITY_ACTIVATION_HEIGHT
            })
            .map(|amendment| u64::from(amendment.value))
            .min()
    }

    pub fn active_vault_bridge_route_policy_hash(
        &self,
        asset_id: &str,
        current_height: u64,
    ) -> Result<(String, &GovernanceAmendment), String> {
        let record = self.active_vault_bridge_route_profile(asset_id, current_height)?;
        let amendment = self
            .amendments
            .iter()
            .find(|amendment| amendment.amendment_id == record.governance_amendment_id)
            .ok_or_else(|| {
                "vault bridge route profile authorization amendment is missing".to_string()
            })?;
        if amendment.kind != vault_bridge_route_amendment_kind(&record.profile)?
            || amendment.value != record.profile.route_epoch
            || amendment.activation_height != record.profile.activation_height
            || amendment.paused
        {
            return Err(
                "vault bridge route profile authorization amendment is inconsistent".to_string(),
            );
        }
        Ok((record.profile_hash.clone(), amendment))
    }

    /// Resolve any historically authorized route by its immutable profile
    /// hash. This is deliberately distinct from `active_*`: new ingress must
    /// use the active route, while an already-pinned deposit or redemption
    /// must remain finishable after a later route becomes active.
    pub fn authorized_vault_bridge_route_profile(
        &self,
        asset_id: &str,
        profile_hash: &str,
    ) -> Result<&VaultBridgeRouteProfileRecordV1, String> {
        let mut matches = self.vault_bridge_route_profiles.iter().filter(|record| {
            record.profile.asset_id == asset_id && record.profile_hash == profile_hash
        });
        let record = matches
            .next()
            .ok_or_else(|| "vault bridge route profile is not governed".to_string())?;
        if matches.next().is_some() {
            return Err("vault bridge route profile hash resolves ambiguously".to_string());
        }
        record.validate()?;
        let amendment = self
            .amendments
            .iter()
            .find(|amendment| amendment.amendment_id == record.governance_amendment_id)
            .ok_or_else(|| {
                "vault bridge route profile authorization amendment is missing".to_string()
            })?;
        if amendment.kind != vault_bridge_route_amendment_kind(&record.profile)?
            || amendment.value != record.profile.route_epoch
            || amendment.activation_height != record.profile.activation_height
            || amendment.paused
        {
            return Err(
                "vault bridge route profile authorization amendment is inconsistent".to_string(),
            );
        }
        Ok(record)
    }

    pub fn active_vault_bridge_route_profile(
        &self,
        asset_id: &str,
        current_height: u64,
    ) -> Result<&VaultBridgeRouteProfileRecordV1, String> {
        let authority_height = self
            .vault_bridge_route_authority_activation_height()
            .ok_or_else(|| "vault bridge route authority is not activated".to_string())?;
        if current_height < authority_height {
            return Err("vault bridge route authority is not active at this height".to_string());
        }
        let mut latest = None::<&VaultBridgeRouteProfileRecordV1>;
        for record in &self.vault_bridge_route_profiles {
            if record.profile.asset_id != asset_id
                || record.profile.activation_height > current_height
            {
                continue;
            }
            record.validate()?;
            let candidate_key = (record.profile.route_epoch, record.profile.activation_height);
            if let Some(existing) = latest {
                let existing_key = (
                    existing.profile.route_epoch,
                    existing.profile.activation_height,
                );
                if candidate_key == existing_key && record.profile_hash != existing.profile_hash {
                    return Err(
                        "vault bridge route governance is ambiguous at the latest epoch"
                            .to_string(),
                    );
                }
                if candidate_key <= existing_key {
                    continue;
                }
            }
            latest = Some(record);
        }
        let latest = latest
            .ok_or_else(|| "vault bridge route has no active governed profile".to_string())?;
        if current_height >= latest.profile.expires_at_height {
            return Err("latest vault bridge route profile is expired".to_string());
        }
        Ok(latest)
    }

    pub fn active_vault_bridge_route_amendment(
        &self,
        profile: &VaultBridgeRouteProfileV1,
        current_height: u64,
    ) -> Result<&GovernanceAmendment, String> {
        profile.validate()?;
        let active = self.active_vault_bridge_route_profile(&profile.asset_id, current_height)?;
        let (active_policy_hash, amendment) =
            self.active_vault_bridge_route_policy_hash(&profile.asset_id, current_height)?;
        if active.profile != *profile || active_policy_hash != profile.profile_hash()?
        {
            return Err(
                "vault bridge route profile does not match the active governed profile"
                    .to_string(),
            );
        }
        Ok(amendment)
    }

    pub fn atomic_swap_activation_height(&self) -> Option<u64> {
        self.amendments
            .iter()
            .rev()
            .find(|amendment| amendment.kind == GOVERNANCE_KIND_ATOMIC_SWAP_ACTIVATION_HEIGHT)
            .map(|amendment| u64::from(amendment.value))
    }

    pub fn replicated_state_v2_activation_height(&self) -> Option<u64> {
        self.amendments
            .iter()
            .filter(|amendment| {
                amendment.kind == GOVERNANCE_KIND_REPLICATED_STATE_V2_ACTIVATION_HEIGHT
            })
            .map(|amendment| u64::from(amendment.value))
            .min()
    }

    pub fn bridge_exit_root_activation_height(&self) -> Option<u64> {
        self.amendments
            .iter()
            .filter(|amendment| {
                amendment.kind == GOVERNANCE_KIND_BRIDGE_EXIT_ROOT_ACTIVATION_HEIGHT
                    && !amendment.paused
            })
            .map(|amendment| u64::from(amendment.value))
            .min()
    }
}

pub fn vault_bridge_route_amendment_prefix(asset_id: &str) -> String {
    format!("{GOVERNANCE_VAULT_BRIDGE_ROUTE_KIND_PREFIX_V1}:{asset_id}:")
}

pub fn vault_bridge_route_amendment_kind(
    profile: &VaultBridgeRouteProfileV1,
) -> Result<String, String> {
    profile.validate()?;
    Ok(format!(
        "{}{}",
        vault_bridge_route_amendment_prefix(&profile.asset_id),
        profile.profile_hash()?
    ))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceAgentDryRunAmendment {
    pub schema: String,
    pub dry_run_id: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub action_mode: String,
    pub expected_previous_dry_run_id: String,
    pub bundle_hash: String,
    pub architecture_statement_hash: String,
    pub objective_statement_hash: String,
    pub ruleset_source_bundle_hash: String,
    pub ruleset_hash: String,
    pub compiled_policy_ruleset_hash: String,
    pub compiled_policy_hash: String,
    pub replay_bundle_root: String,
    pub replay_bundle_uri: String,
    pub report_root: String,
    pub report_uri: String,
    pub validator_registry_root_before: String,
    pub validator_registry_root_after: String,
    pub registry_mutation_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceAgentDryRunRecord {
    pub schema: String,
    pub record_id: String,
    pub dry_run_id: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub batch_id: String,
    pub recorded_height: u64,
    pub action_mode: String,
    pub previous_dry_run_id: String,
    pub bundle_hash: String,
    pub architecture_statement_hash: String,
    pub objective_statement_hash: String,
    pub ruleset_hash: String,
    pub compiled_policy_hash: String,
    pub replay_bundle_root: String,
    pub replay_bundle_uri: String,
    pub report_root: String,
    pub report_uri: String,
    pub validator_registry_root_before: String,
    pub validator_registry_root_after: String,
    pub registry_mutation_count: u32,
}

fn default_crypto_policy_version() -> u32 {
    1
}

fn default_bridge_witness_epoch() -> u32 {
    1
}

fn default_authority_mode() -> u32 {
    GOVERNANCE_AUTHORITY_MODE_FOUNDATION
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatorRegistryEntry {
    pub node_id: String,
    pub algorithm_id: String,
    pub public_key_hex: String,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceAmendment {
    pub amendment_id: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub instance_id: String,
    pub proposal_id: String,
    pub certificate_id: String,
    pub proposer: String,
    pub validators: Vec<String>,
    pub quorum: usize,
    pub kind: String,
    pub value: u32,
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub activation_height: u64,
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub veto_until_height: u64,
    #[serde(default, skip_serializing_if = "is_false")]
    pub paused: bool,
    pub support: Vec<String>,
    pub votes: Vec<GovernanceVote>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub signed_authorizations: Vec<SignedGovernanceAuthorizationV2>,
}

fn is_zero_u64(value: &u64) -> bool {
    *value == 0
}

fn is_zero_u32(value: &u32) -> bool {
    *value == 0
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceVote {
    pub vote_id: String,
    pub validator: String,
    pub accept: bool,
}

pub const SIGNED_GOVERNANCE_AUTHORIZATION_SCHEMA_V2: &str =
    "postfiat.signed_governance_authorization.v2";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedGovernanceAuthorizationV2 {
    pub schema: String,
    pub validator: String,
    pub vote_id: String,
    pub old_registry_root: String,
    pub committee_epoch: u64,
    pub proposal_slot: u64,
    pub expires_at_height: u64,
    pub algorithm_id: String,
    pub signature_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceAmendmentActivationRecord {
    pub schema: String,
    pub activation_record_id: String,
    pub amendment_id: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub batch_id: String,
    pub kind: String,
    pub value: u32,
    pub previous_value: u32,
    pub new_value: u32,
    pub activation_height: u64,
    pub veto_until_height: u64,
    pub activated_height: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceAmendmentSupersessionRecord {
    pub schema: String,
    pub supersession_record_id: String,
    pub superseded_amendment_id: String,
    pub superseding_amendment_id: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub batch_id: String,
    pub kind: String,
    pub previous_value: u32,
    pub new_value: u32,
    pub supersession_height: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceAmendmentRollbackRecord {
    pub schema: String,
    pub rollback_record_id: String,
    pub rolled_back_amendment_id: String,
    pub restored_amendment_id: String,
    pub rollback_amendment_id: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub batch_id: String,
    pub kind: String,
    pub previous_value: u32,
    pub restored_value: u32,
    pub rollback_height: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatorRegistryUpdateRecord {
    pub schema: String,
    pub update_id: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub instance_id: String,
    pub proposal_id: String,
    pub certificate_id: String,
    pub proposer: String,
    pub validators: Vec<String>,
    pub quorum: usize,
    pub support: Vec<String>,
    pub votes: Vec<GovernanceVote>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub signed_authorizations: Vec<SignedGovernanceAuthorizationV2>,
    pub activation_height: u64,
    pub previous_registry_root: String,
    pub new_registry_root: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_trust_graph_root: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_trust_graph_root: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trust_graph_transition_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub previous_validators: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub new_validators: Vec<String>,
    pub operation: String,
    pub subject_node_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_record: Option<ValidatorRegistryEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_record: Option<ValidatorRegistryEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceActionBatch {
    pub batch_id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub amendments: Vec<GovernanceAmendment>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub validator_registry_updates: Vec<ValidatorRegistryUpdateRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub governance_agent_dry_runs: Vec<GovernanceAgentDryRunAmendment>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fastswap_bootstraps: Vec<FastSwapGovernanceBootstrapV1>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fastpay_recovery_bootstraps: Vec<FastPayRecoveryGovernanceBootstrapV1>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vault_bridge_route_profile_activations: Vec<VaultBridgeRouteProfileActivationV1>,
}

impl GovernanceActionBatch {
    pub fn new(batch_id: impl Into<String>, amendments: Vec<GovernanceAmendment>) -> Self {
        Self {
            batch_id: batch_id.into(),
            amendments,
            validator_registry_updates: Vec::new(),
            governance_agent_dry_runs: Vec::new(),
            fastswap_bootstraps: Vec::new(),
            fastpay_recovery_bootstraps: Vec::new(),
            vault_bridge_route_profile_activations: Vec::new(),
        }
    }

    pub fn with_registry_updates(
        batch_id: impl Into<String>,
        amendments: Vec<GovernanceAmendment>,
        validator_registry_updates: Vec<ValidatorRegistryUpdateRecord>,
    ) -> Self {
        Self {
            batch_id: batch_id.into(),
            amendments,
            validator_registry_updates,
            governance_agent_dry_runs: Vec::new(),
            fastswap_bootstraps: Vec::new(),
            fastpay_recovery_bootstraps: Vec::new(),
            vault_bridge_route_profile_activations: Vec::new(),
        }
    }

    pub fn with_governance_agent_dry_runs(
        batch_id: impl Into<String>,
        amendments: Vec<GovernanceAmendment>,
        validator_registry_updates: Vec<ValidatorRegistryUpdateRecord>,
        governance_agent_dry_runs: Vec<GovernanceAgentDryRunAmendment>,
    ) -> Self {
        Self {
            batch_id: batch_id.into(),
            amendments,
            validator_registry_updates,
            governance_agent_dry_runs,
            fastswap_bootstraps: Vec::new(),
            fastpay_recovery_bootstraps: Vec::new(),
            vault_bridge_route_profile_activations: Vec::new(),
        }
    }

    pub fn with_fastswap_bootstrap(
        batch_id: impl Into<String>,
        bootstrap: FastSwapGovernanceBootstrapV1,
    ) -> Self {
        Self {
            batch_id: batch_id.into(),
            amendments: Vec::new(),
            validator_registry_updates: Vec::new(),
            governance_agent_dry_runs: Vec::new(),
            fastswap_bootstraps: vec![bootstrap],
            fastpay_recovery_bootstraps: Vec::new(),
            vault_bridge_route_profile_activations: Vec::new(),
        }
    }

    pub fn with_vault_bridge_route_profile_activation(
        batch_id: impl Into<String>,
        activation: VaultBridgeRouteProfileActivationV1,
    ) -> Self {
        Self {
            batch_id: batch_id.into(),
            amendments: Vec::new(),
            validator_registry_updates: Vec::new(),
            governance_agent_dry_runs: Vec::new(),
            fastswap_bootstraps: Vec::new(),
            fastpay_recovery_bootstraps: Vec::new(),
            vault_bridge_route_profile_activations: vec![activation],
        }
    }

    pub fn with_fastpay_recovery_bootstrap(
        batch_id: impl Into<String>,
        bootstrap: FastPayRecoveryGovernanceBootstrapV1,
    ) -> Self {
        Self {
            batch_id: batch_id.into(),
            amendments: Vec::new(),
            validator_registry_updates: Vec::new(),
            governance_agent_dry_runs: Vec::new(),
            fastswap_bootstraps: Vec::new(),
            fastpay_recovery_bootstraps: vec![bootstrap],
            vault_bridge_route_profile_activations: Vec::new(),
        }
    }
}

#[cfg(test)]
mod shielded_bridge_governance_tests {
    use super::*;

    fn vault_route(epoch: u32, activation_height: u64) -> VaultBridgeRouteProfileV1 {
        VaultBridgeRouteProfileV1 {
            schema: VAULT_BRIDGE_ROUTE_PROFILE_SCHEMA_V1.to_string(),
            route_id: "arbitrum-pfusdc".to_string(),
            asset_id: "11".repeat(48),
            source_chain_id: 42_161,
            vault_address: "0x1111111111111111111111111111111111111111".to_string(),
            vault_runtime_code_hash: format!("0x{}", "22".repeat(32)),
            token_address: "0x3333333333333333333333333333333333333333".to_string(),
            token_runtime_code_hash: format!("0x{}", "44".repeat(32)),
            route_epoch: epoch,
            verifier_kind: NAV_PROFILE_VERIFIER_MULTI_FETCH.to_string(),
            evidence_tier: VAULT_BRIDGE_EVIDENCE_TIER_INDEPENDENTLY_OBSERVED.to_string(),
            verifier_policy_hash: String::new(),
            verifier_program_vkey: String::new(),
            verifier_proof_encoding: String::new(),
            max_proof_bytes: 0,
            max_public_values_bytes: 0,
            max_snapshot_age_blocks: 100,
            challenge_window_blocks: 6,
            max_epoch_gap_blocks: 1_000,
            settle_deadline_blocks: 1_000,
            min_challenge_bond: 1,
            min_attestations: 3,
            minimum_confirmations: 64,
            activation_height,
            expires_at_height: activation_height + 10_000,
        }
    }

    fn amendment(kind: &str, value: u32) -> GovernanceAmendment {
        GovernanceAmendment {
            amendment_id: format!("{kind}:{value}"),
            chain_id: "postfiat-local".to_string(),
            genesis_hash: "aa".repeat(48),
            protocol_version: 1,
            instance_id: "instance".to_string(),
            proposal_id: "proposal".to_string(),
            certificate_id: "certificate".to_string(),
            proposer: "proposer".to_string(),
            validators: vec!["validator-0".to_string()],
            quorum: 1,
            kind: kind.to_string(),
            value,
            activation_height: 0,
            veto_until_height: 0,
            paused: false,
            support: vec!["validator-0".to_string()],
            votes: Vec::new(),
            signed_authorizations: Vec::new(),
        }
    }

    fn activate_route(governance: &mut GovernanceState, profile: &VaultBridgeRouteProfileV1) {
        let mut amendment = amendment(
            &vault_bridge_route_amendment_kind(profile).expect("route amendment kind"),
            profile.route_epoch,
        );
        amendment.activation_height = profile.activation_height;
        let activation = VaultBridgeRouteProfileActivationV1 {
            schema: VAULT_BRIDGE_ROUTE_PROFILE_ACTIVATION_SCHEMA_V1.to_string(),
            profile: profile.clone(),
            amendment: amendment.clone(),
            tier4_finality_bootstrap: None,
        };
        let record = VaultBridgeRouteProfileRecordV1::new(&activation, profile.activation_height)
            .expect("route profile record");
        governance.apply(amendment);
        governance.vault_bridge_route_profiles.push(record);
    }

    #[test]
    fn bridge_verification_activation_height_derives_from_committed_amendment_log() {
        let mut governance = GovernanceState::new(6);
        assert_eq!(governance.bridge_verification_activation_height(), None);

        governance.apply(amendment(
            GOVERNANCE_KIND_BRIDGE_VERIFICATION_ACTIVATION_HEIGHT,
            300,
        ));
        governance.apply(amendment(GOVERNANCE_KIND_BRIDGE_WITNESS_EPOCH, 4));
        assert_eq!(governance.active_validator_count, 6);
        assert_eq!(governance.bridge_witness_epoch, 4);
        assert_eq!(governance.bridge_verification_activation_height(), Some(300));

        governance.apply(amendment(
            GOVERNANCE_KIND_BRIDGE_VERIFICATION_ACTIVATION_HEIGHT,
            512,
        ));
        assert_eq!(governance.bridge_verification_activation_height(), Some(512));
    }

    #[test]
    fn orchard_pool_pause_defaults_off_and_round_trips_only_when_active() {
        let mut governance = GovernanceState::new(6);
        let legacy_shape = serde_json::to_value(&governance).expect("serialize governance");
        assert!(legacy_shape.get("orchard_pool_paused").is_none());

        governance.apply(amendment(GOVERNANCE_KIND_ORCHARD_POOL_PAUSE, 1));
        assert!(governance.orchard_pool_paused);
        let paused = serde_json::to_value(&governance).expect("serialize paused governance");
        assert_eq!(paused.get("orchard_pool_paused"), Some(&serde_json::json!(true)));

        governance.apply(amendment(GOVERNANCE_KIND_ORCHARD_POOL_PAUSE, 0));
        assert!(!governance.orchard_pool_paused);
    }

    #[test]
    fn atomic_swap_governance_is_backward_compatible_latest_wins_and_pause_round_trips() {
        let mut governance = GovernanceState::new(6);
        let legacy = serde_json::to_value(&governance).expect("serialize legacy governance");
        assert!(legacy.get("atomic_swap_paused").is_none());
        assert_eq!(governance.atomic_swap_activation_height(), None);

        governance.apply(amendment(GOVERNANCE_KIND_ATOMIC_SWAP_ACTIVATION_HEIGHT, 300));
        governance.apply(amendment(GOVERNANCE_KIND_BRIDGE_WITNESS_EPOCH, 4));
        governance.apply(amendment(GOVERNANCE_KIND_ATOMIC_SWAP_ACTIVATION_HEIGHT, 512));
        assert_eq!(governance.atomic_swap_activation_height(), Some(512));

        governance.apply(amendment(GOVERNANCE_KIND_ATOMIC_SWAP_PAUSE, 1));
        assert!(governance.atomic_swap_paused);
        let paused = serde_json::to_value(&governance).expect("serialize paused governance");
        assert_eq!(paused.get("atomic_swap_paused"), Some(&serde_json::json!(true)));
        governance.apply(amendment(GOVERNANCE_KIND_ATOMIC_SWAP_PAUSE, 0));
        assert!(!governance.atomic_swap_paused);
    }

    #[test]
    fn replicated_state_v2_activation_is_absent_for_legacy_and_irreversible_once_scheduled() {
        let mut governance = GovernanceState::new(6);
        assert_eq!(governance.replicated_state_v2_activation_height(), None);

        governance.apply(amendment(
            GOVERNANCE_KIND_REPLICATED_STATE_V2_ACTIVATION_HEIGHT,
            500,
        ));
        assert_eq!(governance.replicated_state_v2_activation_height(), Some(500));

        governance.apply(amendment(
            GOVERNANCE_KIND_REPLICATED_STATE_V2_ACTIVATION_HEIGHT,
            700,
        ));
        assert_eq!(
            governance.replicated_state_v2_activation_height(),
            Some(500),
            "a later amendment must not postpone an already scheduled root transition"
        );
    }

    #[test]
    fn governed_vault_bridge_route_requires_exact_latest_active_profile() {
        let mut governance = GovernanceState::new(6);
        governance.apply(amendment(
            GOVERNANCE_KIND_VAULT_BRIDGE_ROUTE_AUTHORITY_ACTIVATION_HEIGHT,
            100,
        ));

        let first = vault_route(1, 110);
        activate_route(&mut governance, &first);
        assert!(governance
            .active_vault_bridge_route_amendment(&first, 110)
            .is_ok());

        let second = vault_route(2, 120);
        activate_route(&mut governance, &second);
        assert!(governance
            .active_vault_bridge_route_amendment(&first, 120)
            .is_err());
        assert!(governance
            .active_vault_bridge_route_amendment(&second, 120)
            .is_ok());

        let mut substituted = second.clone();
        substituted.vault_runtime_code_hash = format!("0x{}", "55".repeat(32));
        assert!(governance
            .active_vault_bridge_route_amendment(&substituted, 120)
            .is_err());

        governance.apply(amendment(
            GOVERNANCE_KIND_VAULT_BRIDGE_ROUTE_AUTHORITY_ACTIVATION_HEIGHT,
            999,
        ));
        assert_eq!(
            governance.vault_bridge_route_authority_activation_height(),
            Some(100),
            "route authority activation cannot be postponed after it is scheduled"
        );
    }

    #[test]
    fn governed_vault_bridge_route_policy_selection_fails_closed_on_ambiguity() {
        let mut governance = GovernanceState::new(6);
        governance.apply(amendment(
            GOVERNANCE_KIND_VAULT_BRIDGE_ROUTE_AUTHORITY_ACTIVATION_HEIGHT,
            100,
        ));
        let first = vault_route(1, 110);
        activate_route(&mut governance, &first);

        let mut conflicting = first.clone();
        conflicting.vault_runtime_code_hash = format!("0x{}", "55".repeat(32));
        activate_route(&mut governance, &conflicting);

        let error = governance
            .active_vault_bridge_route_policy_hash(&first.asset_id, 110)
            .expect_err("same-epoch conflicting routes must fail closed");
        assert!(error.contains("ambiguous"), "{error}");
    }

    #[test]
    fn vault_bridge_route_profile_rejects_retired_vault_and_expiry_errors() {
        let mut route = vault_route(1, 110);
        route.vault_address = RETIRED_VAULT_BRIDGE_ADDRESS.to_string();
        assert!(route.validate().is_err());

        let mut route = vault_route(1, 110);
        route.expires_at_height = route.activation_height;
        assert!(route.validate().is_err());
    }
}
