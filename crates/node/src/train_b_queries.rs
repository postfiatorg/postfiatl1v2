use std::io;
use std::path::PathBuf;

use postfiat_crypto_provider::is_canonical_address;
use postfiat_execution::genesis_hash;
use postfiat_storage::NodeStore;
use postfiat_types::{
    credential_id, identity_handle_key, AssetAccessPolicyV1, CredentialIssuerV1,
    CredentialRecordV1, IdentityHistoryEventV1, IdentityLinkKind, IdentityRecordV1, IdentityStatus,
    LedgerState, NavFeeAccrualV1, NavFeeStateV1, PROTOCOL_BLOCKS_PER_YEAR,
};

const DEFAULT_HANDLE_NAMESPACE: &str = "tasknode.postfiat.org";
use serde::{Deserialize, Serialize};

use crate::{
    read_chain_tip_or_reconstruct_for_genesis, validate_issued_asset_query_id,
    validate_query_text_field,
};

const TRAIN_B_DEFAULT_QUERY_LIMIT: usize = 64;
const TRAIN_B_MAX_QUERY_LIMIT: usize = 256;

fn train_b_query_limit(limit: Option<usize>, label: &str) -> io::Result<usize> {
    let resolved = limit.unwrap_or(TRAIN_B_DEFAULT_QUERY_LIMIT);
    if resolved == 0 || resolved > TRAIN_B_MAX_QUERY_LIMIT {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{label} limit must be between 1 and {TRAIN_B_MAX_QUERY_LIMIT}"),
        ));
    }
    Ok(resolved)
}

// ============================================================================
// Pillar 6: Hive Identity Read RPC Options & Reports
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityInfoOptions {
    pub data_dir: PathBuf,
    pub account: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityByNostrOptions {
    pub data_dir: PathBuf,
    pub nostr_pubkey: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityByHandleOptions {
    pub data_dir: PathBuf,
    pub handle: String,
    pub namespace: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityByLinkOptions {
    pub data_dir: PathBuf,
    pub link_kind: String,
    pub target: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityResolveOptions {
    pub data_dir: PathBuf,
    pub account: Option<String>,
    pub nostr_pubkey: Option<String>,
    pub handle: Option<String>,
    pub namespace: Option<String>,
    pub link_kind: Option<String>,
    pub target: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityHistoryOptions {
    pub data_dir: PathBuf,
    pub account: String,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityLookupReport {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub block_height: u64,
    pub found: bool,
    pub compromised_nostr_pubkey: bool,
    pub identity: Option<IdentityRecordV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityHistoryReport {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub block_height: u64,
    pub account: String,
    pub event_count: u64,
    pub truncated: bool,
    pub events: Vec<IdentityHistoryEventV1>,
}

fn parse_identity_link_kind(raw: &str) -> io::Result<IdentityLinkKind> {
    match raw {
        "validator_node" => Ok(IdentityLinkKind::ValidatorNode),
        "old_chain_account" => Ok(IdentityLinkKind::OldChainAccount),
        "agent_controller" => Ok(IdentityLinkKind::AgentController),
        "custom_nip05" => Ok(IdentityLinkKind::CustomNip05),
        "evm_address" => Ok(IdentityLinkKind::EvmAddress),
        "solana_address" => Ok(IdentityLinkKind::SolanaAddress),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unsupported identity link_kind `{raw}`"),
        )),
    }
}

fn build_identity_lookup_report(
    genesis: &postfiat_types::Genesis,
    block_height: u64,
    compromised_nostr_pubkey: bool,
    identity: Option<IdentityRecordV1>,
) -> IdentityLookupReport {
    IdentityLookupReport {
        schema: "postfiat-identity-lookup-v1".to_string(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(genesis),
        protocol_version: genesis.protocol_version,
        block_height,
        found: identity.is_some(),
        compromised_nostr_pubkey,
        identity,
    }
}

pub fn identity_info(options: IdentityInfoOptions) -> io::Result<IdentityLookupReport> {
    validate_query_text_field("account", &options.account)?;
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let ledger = store.read_ledger()?;
    let block_height = read_chain_tip_or_reconstruct_for_genesis(&store, &genesis)?.height;
    let identity = ledger.identities_by_account.get(&options.account).cloned();
    let compromised = identity
        .as_ref()
        .is_some_and(|id| ledger.compromised_nostr_pubkeys.contains(&id.nostr_pubkey));
    Ok(build_identity_lookup_report(
        &genesis,
        block_height,
        compromised,
        identity,
    ))
}

pub fn identity_by_nostr(options: IdentityByNostrOptions) -> io::Result<IdentityLookupReport> {
    validate_query_text_field("nostr_pubkey", &options.nostr_pubkey)?;
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let ledger = store.read_ledger()?;
    let block_height = read_chain_tip_or_reconstruct_for_genesis(&store, &genesis)?.height;
    let nostr = options.nostr_pubkey.to_ascii_lowercase();
    let compromised = ledger.compromised_nostr_pubkeys.contains(&nostr);
    let identity = ledger
        .identity_account_by_nostr
        .get(&nostr)
        .and_then(|account| ledger.identities_by_account.get(account))
        .cloned();
    Ok(build_identity_lookup_report(
        &genesis,
        block_height,
        compromised,
        identity,
    ))
}

pub fn identity_by_handle(options: IdentityByHandleOptions) -> io::Result<IdentityLookupReport> {
    validate_query_text_field("handle", &options.handle)?;
    if let Some(ns) = options.namespace.as_deref() {
        validate_query_text_field("namespace", ns)?;
    }
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let ledger = store.read_ledger()?;
    let block_height = read_chain_tip_or_reconstruct_for_genesis(&store, &genesis)?.height;
    let (handle_part, ns_part) = if options.namespace.is_none() {
        if let Some((h, ns)) = options.handle.split_once('@') {
            (h.to_string(), ns.to_string())
        } else {
            (options.handle.clone(), DEFAULT_HANDLE_NAMESPACE.to_string())
        }
    } else {
        (
            options.handle.clone(),
            options
                .namespace
                .clone()
                .unwrap_or_else(|| DEFAULT_HANDLE_NAMESPACE.to_string()),
        )
    };
    let key = identity_handle_key(&ns_part, &handle_part);
    let identity = ledger
        .identity_account_by_handle
        .get(&key)
        .and_then(|account| ledger.identities_by_account.get(account))
        .cloned();
    let compromised = identity
        .as_ref()
        .is_some_and(|id| ledger.compromised_nostr_pubkeys.contains(&id.nostr_pubkey));
    Ok(build_identity_lookup_report(
        &genesis,
        block_height,
        compromised,
        identity,
    ))
}

pub fn identity_by_link(options: IdentityByLinkOptions) -> io::Result<IdentityLookupReport> {
    validate_query_text_field("link_kind", &options.link_kind)?;
    validate_query_text_field("target", &options.target)?;
    let kind = parse_identity_link_kind(&options.link_kind)?;
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let ledger = store.read_ledger()?;
    let block_height = read_chain_tip_or_reconstruct_for_genesis(&store, &genesis)?.height;
    let identity = match kind {
        IdentityLinkKind::ValidatorNode => ledger
            .identity_validator_bindings
            .get(&options.target)
            .filter(|binding| binding.unbound_at_height.is_none())
            .and_then(|binding| binding.account.as_deref())
            .and_then(|account| ledger.identities_by_account.get(account))
            .cloned(),
        IdentityLinkKind::OldChainAccount => ledger
            .identity_account_by_old_chain
            .get(&options.target)
            .and_then(|account| ledger.identities_by_account.get(account))
            .cloned(),
        _ => ledger
            .identities_by_account
            .values()
            .find(|record| {
                record.status == IdentityStatus::Active
                    && record
                        .links
                        .iter()
                        .any(|link| link.kind == kind && link.target == options.target)
            })
            .cloned(),
    };
    let compromised = identity
        .as_ref()
        .is_some_and(|id| ledger.compromised_nostr_pubkeys.contains(&id.nostr_pubkey));
    Ok(build_identity_lookup_report(
        &genesis,
        block_height,
        compromised,
        identity,
    ))
}

pub fn identity_resolve(options: IdentityResolveOptions) -> io::Result<IdentityLookupReport> {
    let selectors = usize::from(options.account.is_some())
        + usize::from(options.nostr_pubkey.is_some())
        + usize::from(options.handle.is_some())
        + usize::from(options.link_kind.is_some() || options.target.is_some());
    if selectors != 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "identity_resolve requires exactly one selector: account, nostr_pubkey, handle, or (link_kind + target)",
        ));
    }
    if let Some(account) = options.account {
        identity_info(IdentityInfoOptions {
            data_dir: options.data_dir,
            account,
        })
    } else if let Some(nostr_pubkey) = options.nostr_pubkey {
        identity_by_nostr(IdentityByNostrOptions {
            data_dir: options.data_dir,
            nostr_pubkey,
        })
    } else if let Some(handle) = options.handle {
        identity_by_handle(IdentityByHandleOptions {
            data_dir: options.data_dir,
            handle,
            namespace: options.namespace,
        })
    } else {
        let link_kind = options.link_kind.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "identity_resolve link selector requires both link_kind and target",
            )
        })?;
        let target = options.target.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "identity_resolve link selector requires both link_kind and target",
            )
        })?;
        identity_by_link(IdentityByLinkOptions {
            data_dir: options.data_dir,
            link_kind,
            target,
        })
    }
}

pub fn identity_history(options: IdentityHistoryOptions) -> io::Result<IdentityHistoryReport> {
    validate_query_text_field("account", &options.account)?;
    let limit = train_b_query_limit(options.limit, "identity_history")?;
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let ledger = store.read_ledger()?;
    let block_height = read_chain_tip_or_reconstruct_for_genesis(&store, &genesis)?.height;
    let mut events = ledger
        .identity_history_by_account
        .get(&options.account)
        .cloned()
        .unwrap_or_default();
    let truncated = events.len() > limit;
    if truncated {
        let start = events.len().saturating_sub(limit);
        events = events.split_off(start);
    }
    Ok(IdentityHistoryReport {
        schema: "postfiat-identity-history-v1".to_string(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        block_height,
        account: options.account,
        event_count: events.len() as u64,
        truncated,
        events,
    })
}

// ============================================================================
// Pillar 7: NAV Fee Schedule & History Read RPC Options & Reports
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavFeeScheduleOptions {
    pub data_dir: PathBuf,
    pub asset_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavFeeHistoryOptions {
    pub data_dir: PathBuf,
    pub asset_id: String,
    pub from_epoch: Option<u64>,
    pub cursor: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavFeeScheduleReport {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub block_height: u64,
    pub asset_id: String,
    pub found: bool,
    pub has_fee_schedule: bool,
    pub blocks_per_year: u64,
    pub fee_state: Option<NavFeeStateV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavFeeHistoryReport {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub asset_id: String,
    pub accrual_count: u64,
    pub truncated: bool,
    pub next_cursor: Option<String>,
    pub accruals: Vec<NavFeeAccrualV1>,
}

pub fn nav_fee_schedule(options: NavFeeScheduleOptions) -> io::Result<NavFeeScheduleReport> {
    validate_issued_asset_query_id("asset_id", &options.asset_id)?;
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let ledger = store.read_ledger()?;
    let block_height = read_chain_tip_or_reconstruct_for_genesis(&store, &genesis)?.height;
    let nav_asset = ledger.nav_asset(&options.asset_id);
    let fee_state = nav_asset.and_then(|asset| asset.fee_state.clone());
    Ok(NavFeeScheduleReport {
        schema: "postfiat-nav-fee-schedule-v1".to_string(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        block_height,
        asset_id: options.asset_id,
        found: nav_asset.is_some(),
        has_fee_schedule: fee_state.is_some(),
        blocks_per_year: PROTOCOL_BLOCKS_PER_YEAR,
        fee_state,
    })
}

pub fn nav_fee_history(options: NavFeeHistoryOptions) -> io::Result<NavFeeHistoryReport> {
    validate_issued_asset_query_id("asset_id", &options.asset_id)?;
    if let Some(cursor) = options.cursor.as_deref() {
        validate_query_text_field("cursor", cursor)?;
    }
    let limit = train_b_query_limit(options.limit, "nav_fee_history")?;
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let ledger = store.read_ledger()?;
    let mut matching: Vec<(String, NavFeeAccrualV1)> = ledger
        .nav_fee_accruals
        .iter()
        .filter(|(key, accrual)| {
            accrual.asset_id == options.asset_id
                && options
                    .from_epoch
                    .is_none_or(|min_epoch| accrual.epoch >= min_epoch)
                && options
                    .cursor
                    .as_deref()
                    .is_none_or(|cursor| key.as_str() > cursor)
        })
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let truncated = matching.len() > limit;
    matching.truncate(limit);
    let next_cursor = if truncated {
        matching.last().map(|(k, _)| k.clone())
    } else {
        None
    };
    let accruals: Vec<NavFeeAccrualV1> = matching.into_iter().map(|(_, v)| v).collect();
    Ok(NavFeeHistoryReport {
        schema: "postfiat-nav-fee-history-v1".to_string(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        asset_id: options.asset_id,
        accrual_count: accruals.len() as u64,
        truncated,
        next_cursor,
        accruals,
    })
}

// ============================================================================
// Pillar 8: did:pftl, Issuer Registry, Credentials & Asset Access Read RPCs
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DidResolveOptions {
    pub data_dir: PathBuf,
    pub did: Option<String>,
    pub account: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DidVerificationMethodV1 {
    pub id: String,
    #[serde(rename = "type")]
    pub method_type: String,
    pub controller: String,
    #[serde(rename = "publicKeyHex")]
    pub public_key_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DidServiceV1 {
    pub id: String,
    #[serde(rename = "type")]
    pub service_type: String,
    #[serde(rename = "serviceEndpoint")]
    pub service_endpoint: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DidDocumentV1 {
    #[serde(rename = "@context")]
    pub context: Vec<String>,
    pub id: String,
    #[serde(rename = "alsoKnownAs", default, skip_serializing_if = "Vec::is_empty")]
    pub also_known_as: Vec<String>,
    #[serde(rename = "verificationMethod")]
    pub verification_method: Vec<DidVerificationMethodV1>,
    pub authentication: Vec<String>,
    #[serde(rename = "assertionMethod")]
    pub assertion_method: Vec<String>,
    #[serde(
        rename = "keyAgreement",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub key_agreement: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub service: Vec<DidServiceV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DidDocumentMetadataV1 {
    pub chain_id: String,
    pub block_height: u64,
    pub account: String,
    pub account_exists: bool,
    pub key_revealed: bool,
    pub identity_minted: bool,
    pub deactivated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DidResolveReport {
    pub schema: String,
    #[serde(rename = "didDocument")]
    pub did_document: DidDocumentV1,
    #[serde(rename = "didDocumentMetadata")]
    pub did_document_metadata: DidDocumentMetadataV1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssuerInfoOptions {
    pub data_dir: PathBuf,
    pub issuer_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssuerInfoReport {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub issuer_id: String,
    pub found: bool,
    pub issuer: Option<CredentialIssuerV1>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssuerListOptions {
    pub data_dir: PathBuf,
    pub domain: Option<String>,
    pub cursor: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssuerListReport {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub issuer_count: u64,
    pub truncated: bool,
    pub next_cursor: Option<String>,
    pub issuers: Vec<CredentialIssuerV1>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CredentialInfoOptions {
    pub data_dir: PathBuf,
    pub credential_id: Option<String>,
    pub subject: Option<String>,
    pub issuer_id: Option<String>,
    pub credential_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialInfoReport {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub block_height: u64,
    pub found: bool,
    pub currently_valid: bool,
    pub invalid_reason: Option<String>,
    pub credential: Option<CredentialRecordV1>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountCredentialsOptions {
    pub data_dir: PathBuf,
    pub account: String,
    pub issuer_id: Option<String>,
    pub include_expired: bool,
    pub cursor: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountCredentialsReport {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub block_height: u64,
    pub account: String,
    pub credential_count: u64,
    pub truncated: bool,
    pub next_cursor: Option<String>,
    pub credentials: Vec<CredentialRecordV1>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetAccessInfoOptions {
    pub data_dir: PathBuf,
    pub asset_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetAccessInfoReport {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub block_height: u64,
    pub asset_id: String,
    pub found: bool,
    pub requires_authorization: bool,
    pub freeze_enabled: bool,
    pub clawback_enabled: bool,
    pub access_policy: Option<AssetAccessPolicyV1>,
}

fn parse_did_or_account(
    expected_chain_id: &str,
    did: Option<&str>,
    account: Option<&str>,
) -> io::Result<String> {
    match (did, account) {
        (Some(did_str), None) => {
            let rest = did_str.strip_prefix("did:pftl:").ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "did must start with `did:pftl:`",
                )
            })?;
            let acct = if let Some((chain, addr)) = rest.split_once(':') {
                if chain != expected_chain_id {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!(
                            "did chain_id `{chain}` does not match node chain_id `{expected_chain_id}`"
                        ),
                    ));
                }
                addr
            } else {
                rest
            };
            if !is_canonical_address(acct) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("did account `{acct}` is not a canonical pf address"),
                ));
            }
            Ok(acct.to_string())
        }
        (None, Some(acct)) => {
            if !is_canonical_address(acct) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("account `{acct}` is not a canonical pf address"),
                ));
            }
            Ok(acct.to_string())
        }
        (Some(_), Some(_)) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "provide either --did or --account, not both",
        )),
        (None, None) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "did_resolve requires --did or --account",
        )),
    }
}

pub fn did_resolve(options: DidResolveOptions) -> io::Result<DidResolveReport> {
    if let Some(did) = options.did.as_deref() {
        validate_query_text_field("did", did)?;
    }
    if let Some(account) = options.account.as_deref() {
        validate_query_text_field("account", account)?;
    }
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let ledger = store.read_ledger()?;
    let block_height = read_chain_tip_or_reconstruct_for_genesis(&store, &genesis)?.height;
    let account = parse_did_or_account(
        &genesis.chain_id,
        options.did.as_deref(),
        options.account.as_deref(),
    )?;
    let did = format!("did:pftl:{}:{account}", genesis.chain_id);

    let ledger_account = ledger.account(&account);
    let account_exists = ledger_account.is_some();
    let pubkey_hex = ledger_account.and_then(|acc| acc.public_key_hex.clone());
    let key_revealed = pubkey_hex.is_some();

    let identity = ledger.identities_by_account.get(&account);
    let identity_minted = identity.is_some();
    let active_identity = identity.filter(|id| id.status == IdentityStatus::Active);
    let deactivated = identity.is_some_and(|id| id.status == IdentityStatus::Revoked);

    let mut also_known_as = Vec::new();
    let mut verification_method = Vec::new();
    let mut authentication = Vec::new();
    let mut assertion_method = Vec::new();
    let mut key_agreement = Vec::new();
    let mut service = Vec::new();

    if let Some(pk_hex) = pubkey_hex {
        let vm_id = format!("{did}#account-mldsa65");
        verification_method.push(DidVerificationMethodV1 {
            id: vm_id.clone(),
            method_type: "MLDSA65VerificationKey2025".to_string(),
            controller: did.clone(),
            public_key_hex: pk_hex,
        });
        authentication.push(vm_id.clone());
        assertion_method.push(vm_id);
    }

    if let Some(id) = active_identity {
        if let (Some(ns), Some(handle)) = (&id.registered_namespace, &id.handle) {
            also_known_as.push(format!("acct:{handle}@{ns}"));
        }
        if let Some(msg_key) = &id.message_key {
            let vm_id = format!("{did}#message-xwing");
            verification_method.push(DidVerificationMethodV1 {
                id: vm_id.clone(),
                method_type: "XWingKeyAgreementKey2025".to_string(),
                controller: did.clone(),
                public_key_hex: msg_key.clone(),
            });
            key_agreement.push(vm_id);
        }
        if !ledger.compromised_nostr_pubkeys.contains(&id.nostr_pubkey) {
            verification_method.push(DidVerificationMethodV1 {
                id: format!("{did}#nostr-secp256k1"),
                method_type: "Bip340Secp256k1VerificationKey2025".to_string(),
                controller: did.clone(),
                public_key_hex: id.nostr_pubkey.clone(),
            });
        }
        if !id.relays.is_empty() {
            service.push(DidServiceV1 {
                id: format!("{did}#nostr-relays"),
                service_type: "NostrRelayService".to_string(),
                service_endpoint: serde_json::Value::Array(
                    id.relays
                        .iter()
                        .map(|r| serde_json::Value::String(r.clone()))
                        .collect(),
                ),
            });
        }
        if let Some(pq_addr) = &id.pq_receive_address {
            service.push(DidServiceV1 {
                id: format!("{did}#pq-receive"),
                service_type: "PostFiatShieldedReceive".to_string(),
                service_endpoint: serde_json::Value::String(pq_addr.clone()),
            });
        }
    }

    Ok(DidResolveReport {
        schema: "postfiat-did-resolve-v1".to_string(),
        did_document: DidDocumentV1 {
            context: vec![
                "https://www.w3.org/ns/did/v1".to_string(),
                "https://postfiat.org/ns/did-pftl/v1".to_string(),
            ],
            id: did,
            also_known_as,
            verification_method,
            authentication,
            assertion_method,
            key_agreement,
            service,
        },
        did_document_metadata: DidDocumentMetadataV1 {
            chain_id: genesis.chain_id,
            block_height,
            account,
            account_exists,
            key_revealed,
            identity_minted,
            deactivated,
        },
    })
}

fn evaluate_credential_live_validity(
    ledger: &LedgerState,
    record: &CredentialRecordV1,
    block_height: u64,
) -> (bool, Option<String>) {
    if record.status != "accepted" {
        let reason = if record.status == "revoked" {
            "credential_revoked"
        } else {
            "credential_pending"
        };
        return (false, Some(reason.to_string()));
    }
    if block_height > record.expires_at_height {
        return (false, Some("credential_expired".to_string()));
    }
    let Some(issuer) = ledger.credential_issuers.get(&record.issuer_id) else {
        return (false, Some("issuer_not_found".to_string()));
    };
    if issuer.status != "active" {
        return (false, Some("issuer_deregistered".to_string()));
    }
    let Some(signing_entry) = issuer
        .signing_accounts
        .iter()
        .find(|entry| entry.account == record.signing_account)
    else {
        return (false, Some("issuer_signing_account_unknown".to_string()));
    };
    if let Some(cutoff) = signing_entry.compromise_cutoff_height {
        if record.issued_height <= cutoff {
            return (false, Some("credential_issuer_key_compromised".to_string()));
        }
    }
    (true, None)
}

pub fn issuer_info(options: IssuerInfoOptions) -> io::Result<IssuerInfoReport> {
    validate_issued_asset_query_id("issuer_id", &options.issuer_id)?;
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let ledger = store.read_ledger()?;
    let issuer = ledger.credential_issuers.get(&options.issuer_id).cloned();
    Ok(IssuerInfoReport {
        schema: "postfiat-issuer-info-v1".to_string(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        issuer_id: options.issuer_id,
        found: issuer.is_some(),
        issuer,
    })
}

pub fn issuer_list(options: IssuerListOptions) -> io::Result<IssuerListReport> {
    if let Some(domain) = options.domain.as_deref() {
        validate_query_text_field("domain", domain)?;
    }
    if let Some(cursor) = options.cursor.as_deref() {
        validate_query_text_field("cursor", cursor)?;
    }
    let limit = train_b_query_limit(options.limit, "issuer_list")?;
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let ledger = store.read_ledger()?;
    let mut matching: Vec<(String, CredentialIssuerV1)> = ledger
        .credential_issuers
        .iter()
        .filter(|(key, issuer)| {
            options.domain.as_deref().is_none_or(|d| issuer.domain == d)
                && options
                    .cursor
                    .as_deref()
                    .is_none_or(|cursor| key.as_str() > cursor)
        })
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let truncated = matching.len() > limit;
    matching.truncate(limit);
    let next_cursor = if truncated {
        matching.last().map(|(k, _)| k.clone())
    } else {
        None
    };
    let issuers: Vec<CredentialIssuerV1> = matching.into_iter().map(|(_, v)| v).collect();
    Ok(IssuerListReport {
        schema: "postfiat-issuer-list-v1".to_string(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        issuer_count: issuers.len() as u64,
        truncated,
        next_cursor,
        issuers,
    })
}

pub fn credential_info(options: CredentialInfoOptions) -> io::Result<CredentialInfoReport> {
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let ledger = store.read_ledger()?;
    let block_height = read_chain_tip_or_reconstruct_for_genesis(&store, &genesis)?.height;
    let resolved_id = match (
        options.credential_id.as_deref(),
        options.subject.as_deref(),
        options.issuer_id.as_deref(),
        options.credential_type.as_deref(),
    ) {
        (Some(cred_id), None, None, None) => {
            validate_issued_asset_query_id("credential_id", cred_id)?;
            cred_id.to_string()
        }
        (None, Some(subject), Some(issuer_id), Some(credential_type)) => {
            validate_query_text_field("subject", subject)?;
            validate_issued_asset_query_id("issuer_id", issuer_id)?;
            validate_query_text_field("credential_type", credential_type)?;
            credential_id(subject, issuer_id, credential_type)
        }
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "credential_info requires either --credential-id OR (--subject, --issuer-id, --credential-type)",
            ));
        }
    };
    let credential = ledger.credentials.get(&resolved_id).cloned();
    let (currently_valid, invalid_reason) = match &credential {
        Some(record) => evaluate_credential_live_validity(&ledger, record, block_height),
        None => (false, Some("credential_not_found".to_string())),
    };
    Ok(CredentialInfoReport {
        schema: "postfiat-credential-info-v1".to_string(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        block_height,
        found: credential.is_some(),
        currently_valid,
        invalid_reason,
        credential,
    })
}

pub fn account_credentials(
    options: AccountCredentialsOptions,
) -> io::Result<AccountCredentialsReport> {
    validate_query_text_field("account", &options.account)?;
    if let Some(issuer_id) = options.issuer_id.as_deref() {
        validate_issued_asset_query_id("issuer_id", issuer_id)?;
    }
    if let Some(cursor) = options.cursor.as_deref() {
        validate_query_text_field("cursor", cursor)?;
    }
    let limit = train_b_query_limit(options.limit, "account_credentials")?;
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let ledger = store.read_ledger()?;
    let block_height = read_chain_tip_or_reconstruct_for_genesis(&store, &genesis)?.height;
    let mut matching: Vec<(String, CredentialRecordV1)> = ledger
        .credentials
        .iter()
        .filter(|(key, record)| {
            record.subject == options.account
                && options
                    .issuer_id
                    .as_deref()
                    .is_none_or(|id| record.issuer_id == id)
                && (options.include_expired || block_height <= record.expires_at_height)
                && options
                    .cursor
                    .as_deref()
                    .is_none_or(|cursor| key.as_str() > cursor)
        })
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let truncated = matching.len() > limit;
    matching.truncate(limit);
    let next_cursor = if truncated {
        matching.last().map(|(k, _)| k.clone())
    } else {
        None
    };
    let credentials: Vec<CredentialRecordV1> = matching.into_iter().map(|(_, v)| v).collect();
    Ok(AccountCredentialsReport {
        schema: "postfiat-account-credentials-v1".to_string(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        block_height,
        account: options.account,
        credential_count: credentials.len() as u64,
        truncated,
        next_cursor,
        credentials,
    })
}

pub fn asset_access_info(options: AssetAccessInfoOptions) -> io::Result<AssetAccessInfoReport> {
    validate_issued_asset_query_id("asset_id", &options.asset_id)?;
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let ledger = store.read_ledger()?;
    let block_height = read_chain_tip_or_reconstruct_for_genesis(&store, &genesis)?.height;
    let asset = ledger.asset_definition(&options.asset_id);
    Ok(AssetAccessInfoReport {
        schema: "postfiat-asset-access-info-v1".to_string(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        block_height,
        asset_id: options.asset_id,
        found: asset.is_some(),
        requires_authorization: asset.is_some_and(|a| a.requires_authorization),
        freeze_enabled: asset.is_some_and(|a| a.freeze_enabled),
        clawback_enabled: asset.is_some_and(|a| a.clawback_enabled),
        access_policy: asset.and_then(|a| a.access_policy.clone()),
    })
}
