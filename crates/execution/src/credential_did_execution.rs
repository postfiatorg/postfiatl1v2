use std::collections::BTreeSet;

use postfiat_crypto_provider::{
    address_from_public_key, hex_to_bytes, ml_dsa_65_verify_with_context,
};
use postfiat_types::{
    credential_delegated_accept_preimage, credential_id, credential_issuer_id,
    validate_issuer_domain, validate_namespaced_credential_type, AssetAccessPolicyV1,
    AssetAccessSetOperation, AssetApprovalMode, AssetLapsedExitRule, AssetWrapperMode,
    CredentialAcceptOperation, CredentialCreateAndAcceptOperation, CredentialCreateOperation,
    CredentialDeleteOperation, CredentialIssuerV1, CredentialPolicyV1, CredentialRecordV1,
    CredentialRevokeOperation, Genesis, IssuerAdminTransferAction, IssuerAdminTransferOperation,
    IssuerDeregisterOperation, IssuerRegisterOperation, IssuerSigningAccountV1,
    IssuerUpdateOperation, LedgerState, PendingAssetAccessPolicyV1,
    ASSET_ACCESS_MIN_TIGHTEN_NOTICE_BLOCKS, CREDENTIAL_DELEGATED_ACCEPT_DOMAIN,
    CREDENTIAL_DEPOSIT_ATOMS, MAX_CREDENTIAL_POLICY_AND_PAIRS, MAX_CREDENTIAL_POLICY_OR_CLAUSES,
    MAX_ISSUER_CREDENTIAL_TYPES, MAX_ISSUER_SIGNING_ACCOUNTS, MAX_ISSUER_VENDOR_DIDS,
};

use crate::{account_reserve_violation, genesis_hash, AssetExecutionCompatibility};

pub(crate) fn is_credential_valid_at(
    ledger: &LedgerState,
    record: &CredentialRecordV1,
    block_height: u64,
) -> Result<(), &'static str> {
    if record.status != "accepted" {
        return Err(if record.status == "revoked" {
            "credential_revoked"
        } else {
            "credential_pending"
        });
    }
    if block_height > record.expires_at_height {
        return Err("credential_expired");
    }
    let Some(issuer) = ledger.credential_issuers.get(&record.issuer_id) else {
        return Err("issuer_not_found");
    };
    if issuer.status != "active" {
        return Err("issuer_deregistered");
    }
    let Some(signer) = issuer
        .signing_accounts
        .iter()
        .find(|s| s.account == record.signing_account)
    else {
        return Err("issuer_signing_account_missing");
    };
    if let Some(cutoff) = signer.compromise_cutoff_height {
        if record.issued_height <= cutoff {
            return Err("issuer_signing_account_compromised");
        }
    }
    Ok(())
}

pub(crate) fn credential_policy_satisfied_at(
    ledger: &LedgerState,
    account: &str,
    policy: &CredentialPolicyV1,
    block_height: u64,
) -> bool {
    if policy.any_of.is_empty() {
        return false;
    }
    policy.any_of.iter().any(|rule| {
        !rule.all_of.is_empty()
            && rule.all_of.iter().all(|pair| {
                let cid = credential_id(account, &pair.issuer_id, &pair.credential_type);
                ledger
                    .credentials
                    .get(&cid)
                    .is_some_and(|rec| is_credential_valid_at(ledger, rec, block_height).is_ok())
            })
    })
}

fn lock_credential_deposit(
    ledger: &mut LedgerState,
    payer: &str,
    deposit: u64,
) -> Result<(), (&'static str, String)> {
    let account = ledger.account_mut(payer).ok_or_else(|| {
        (
            "missing_sender",
            format!("credential deposit payer `{payer}` does not exist"),
        )
    })?;
    let after = account.balance.checked_sub(deposit).ok_or_else(|| {
        (
            "insufficient_funds",
            "signer balance too low for credential deposit".to_string(),
        )
    })?;
    if let Some(message) = account_reserve_violation(payer, after) {
        return Err(("below_account_reserve", message));
    }
    account.balance = after;
    Ok(())
}

fn refund_credential_deposit(
    ledger: &mut LedgerState,
    payer: &str,
    deposit: u64,
) -> Result<(), (&'static str, String)> {
    if deposit == 0 {
        return Ok(());
    }
    let account = ledger.ensure_account(payer);
    account.balance = account.balance.checked_add(deposit).ok_or_else(|| {
        (
            "balance_overflow",
            "credential deposit refund overflow".to_string(),
        )
    })?;
    Ok(())
}

pub(crate) fn apply_issuer_register(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    operation: &IssuerRegisterOperation,
    block_height: u64,
    compatibility: AssetExecutionCompatibility,
) -> Result<(), (&'static str, String)> {
    if !compatibility.credential_did_v1_active(block_height) {
        return Err((
            "credential_did_not_active",
            format!("issuer_register is not active at block height {block_height}"),
        ));
    }
    validate_issuer_domain(&operation.domain).map_err(|err| ("bad_issuer_domain", err))?;
    if operation.signing_accounts.is_empty()
        || operation.signing_accounts.len() > MAX_ISSUER_SIGNING_ACCOUNTS
        || operation.allowed_credential_types.len() > MAX_ISSUER_CREDENTIAL_TYPES
        || operation.vendor_dids.len() > MAX_ISSUER_VENDOR_DIDS
    {
        return Err((
            "issuer_limit_exceeded",
            "issuer signing accounts, credential types, or vendor DIDs exceed consensus limits"
                .to_string(),
        ));
    }
    for ct in &operation.allowed_credential_types {
        validate_namespaced_credential_type(&operation.domain, ct)
            .map_err(|err| ("issuer_invalid_credential_type", err))?;
    }
    let issuer_id = credential_issuer_id(
        &genesis.chain_id,
        &operation.admin_account,
        operation.registration_nonce,
    );
    if ledger.credential_issuers.contains_key(&issuer_id) {
        return Err((
            "issuer_already_exists",
            format!("issuer `{issuer_id}` already exists"),
        ));
    }
    let mut seen = BTreeSet::new();
    let mut signing_accounts = Vec::with_capacity(operation.signing_accounts.len());
    for acct in &operation.signing_accounts {
        if !seen.insert(acct.as_str()) {
            return Err((
                "duplicate_signing_account",
                format!("duplicate signing account `{acct}`"),
            ));
        }
        signing_accounts.push(IssuerSigningAccountV1 {
            account: acct.clone(),
            added_at_height: block_height,
            revoked_at_height: None,
            compromise_cutoff_height: None,
        });
    }
    let issuer = CredentialIssuerV1 {
        issuer_id: issuer_id.clone(),
        admin_account: operation.admin_account.clone(),
        pending_admin_account: None,
        domain: operation.domain.clone(),
        signing_accounts,
        allowed_credential_types: operation.allowed_credential_types.clone(),
        vendor_dids: operation.vendor_dids.clone(),
        status: "active".to_string(),
        created_at_height: block_height,
        updated_at_height: block_height,
    };
    issuer
        .validate()
        .map_err(|err| ("bad_credential_issuer", err))?;
    ledger.credential_issuers.insert(issuer_id, issuer);
    Ok(())
}

pub(crate) fn apply_issuer_update(
    ledger: &mut LedgerState,
    operation: &IssuerUpdateOperation,
    block_height: u64,
    compatibility: AssetExecutionCompatibility,
) -> Result<(), (&'static str, String)> {
    if !compatibility.credential_did_v1_active(block_height) {
        return Err((
            "credential_did_not_active",
            format!("issuer_update is not active at block height {block_height}"),
        ));
    }
    let mut issuer = ledger
        .credential_issuers
        .get(&operation.issuer_id)
        .cloned()
        .ok_or_else(|| {
            (
                "issuer_not_found",
                format!("issuer `{}` does not exist", operation.issuer_id),
            )
        })?;
    if issuer.status != "active" {
        return Err((
            "issuer_deregistered",
            "cannot update a deregistered issuer".to_string(),
        ));
    }
    if issuer.admin_account != operation.admin_account {
        return Err((
            "issuer_unauthorized",
            "only issuer admin_account may update issuer".to_string(),
        ));
    }
    if let Some(new_domain) = &operation.domain {
        validate_issuer_domain(new_domain).map_err(|err| ("bad_issuer_domain", err))?;
        issuer.domain = new_domain.clone();
    }
    for acct in &operation.add_signing_accounts {
        if let Some(existing) = issuer
            .signing_accounts
            .iter_mut()
            .find(|s| s.account == *acct)
        {
            existing.revoked_at_height = None;
        } else {
            issuer.signing_accounts.push(IssuerSigningAccountV1 {
                account: acct.clone(),
                added_at_height: block_height,
                revoked_at_height: None,
                compromise_cutoff_height: None,
            });
        }
    }
    if issuer.signing_accounts.len() > MAX_ISSUER_SIGNING_ACCOUNTS {
        return Err((
            "issuer_limit_exceeded",
            format!("signing_accounts exceeds {MAX_ISSUER_SIGNING_ACCOUNTS}"),
        ));
    }
    for acct in &operation.revoke_signing_accounts {
        if let Some(existing) = issuer
            .signing_accounts
            .iter_mut()
            .find(|s| s.account == *acct)
        {
            existing.revoked_at_height = Some(block_height);
        }
    }
    for (acct, cutoff) in &operation.compromise_cutoffs {
        if let Some(existing) = issuer
            .signing_accounts
            .iter_mut()
            .find(|s| s.account == *acct)
        {
            existing.compromise_cutoff_height = Some(
                existing
                    .compromise_cutoff_height
                    .map_or(*cutoff, |prev| prev.max(*cutoff)),
            );
        }
    }
    if let Some(types) = &operation.allowed_credential_types {
        if types.len() > MAX_ISSUER_CREDENTIAL_TYPES {
            return Err((
                "issuer_limit_exceeded",
                format!("allowed_credential_types exceeds {MAX_ISSUER_CREDENTIAL_TYPES}"),
            ));
        }
        for ct in types {
            validate_namespaced_credential_type(&issuer.domain, ct)
                .map_err(|err| ("issuer_invalid_credential_type", err))?;
        }
        issuer.allowed_credential_types = types.clone();
    }
    if let Some(dids) = &operation.vendor_dids {
        if dids.len() > MAX_ISSUER_VENDOR_DIDS {
            return Err((
                "issuer_limit_exceeded",
                format!("vendor_dids exceeds {MAX_ISSUER_VENDOR_DIDS}"),
            ));
        }
        issuer.vendor_dids = dids.clone();
    }
    issuer.updated_at_height = block_height;
    issuer
        .validate()
        .map_err(|err| ("bad_credential_issuer", err))?;
    ledger
        .credential_issuers
        .insert(operation.issuer_id.clone(), issuer);
    Ok(())
}

pub(crate) fn apply_issuer_admin_transfer(
    ledger: &mut LedgerState,
    operation: &IssuerAdminTransferOperation,
    block_height: u64,
    compatibility: AssetExecutionCompatibility,
) -> Result<(), (&'static str, String)> {
    if !compatibility.credential_did_v1_active(block_height) {
        return Err((
            "credential_did_not_active",
            format!("issuer_admin_transfer is not active at block height {block_height}"),
        ));
    }
    let issuer = ledger
        .credential_issuers
        .get_mut(&operation.issuer_id)
        .ok_or_else(|| {
            (
                "issuer_not_found",
                format!("issuer `{}` does not exist", operation.issuer_id),
            )
        })?;
    if issuer.status != "active" {
        return Err((
            "issuer_deregistered",
            "cannot transfer admin on a deregistered issuer".to_string(),
        ));
    }
    match operation.action {
        IssuerAdminTransferAction::Propose => {
            if operation.caller != issuer.admin_account {
                return Err((
                    "issuer_unauthorized",
                    "only current admin_account may propose admin transfer".to_string(),
                ));
            }
            let new_admin = operation.new_admin_account.clone().ok_or_else(|| {
                (
                    "bad_issuer_admin_transfer",
                    "new_admin_account is required for propose".to_string(),
                )
            })?;
            issuer.pending_admin_account = Some(new_admin);
        }
        IssuerAdminTransferAction::Accept => {
            if issuer.pending_admin_account.as_deref() != Some(operation.caller.as_str()) {
                return Err((
                    "issuer_unauthorized",
                    "only pending_admin_account may accept admin transfer".to_string(),
                ));
            }
            issuer.admin_account = operation.caller.clone();
            issuer.pending_admin_account = None;
        }
        IssuerAdminTransferAction::Cancel => {
            if operation.caller != issuer.admin_account
                && issuer.pending_admin_account.as_deref() != Some(operation.caller.as_str())
            {
                return Err((
                    "issuer_unauthorized",
                    "only admin_account or pending_admin_account may cancel transfer".to_string(),
                ));
            }
            issuer.pending_admin_account = None;
        }
    }
    issuer.updated_at_height = block_height;
    Ok(())
}

pub(crate) fn apply_issuer_deregister(
    ledger: &mut LedgerState,
    operation: &IssuerDeregisterOperation,
    block_height: u64,
    compatibility: AssetExecutionCompatibility,
) -> Result<(), (&'static str, String)> {
    if !compatibility.credential_did_v1_active(block_height) {
        return Err((
            "credential_did_not_active",
            format!("issuer_deregister is not active at block height {block_height}"),
        ));
    }
    let issuer = ledger
        .credential_issuers
        .get_mut(&operation.issuer_id)
        .ok_or_else(|| {
            (
                "issuer_not_found",
                format!("issuer `{}` does not exist", operation.issuer_id),
            )
        })?;
    if issuer.admin_account != operation.admin_account {
        return Err((
            "issuer_unauthorized",
            "only issuer admin_account may deregister issuer".to_string(),
        ));
    }
    issuer.status = "deregistered".to_string();
    issuer.updated_at_height = block_height;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn upsert_credential_record(
    ledger: &mut LedgerState,
    signing_account: &str,
    issuer_id: &str,
    subject: &str,
    credential_type: &str,
    commitment_hash: &str,
    expires_at_height: u64,
    accepted: bool,
    block_height: u64,
) -> Result<(), (&'static str, String)> {
    let issuer = ledger
        .credential_issuers
        .get(issuer_id)
        .cloned()
        .ok_or_else(|| {
            (
                "issuer_not_found",
                format!("issuer `{issuer_id}` does not exist"),
            )
        })?;
    if issuer.status != "active" {
        return Err((
            "issuer_deregistered",
            "deregistered issuer cannot issue credentials".to_string(),
        ));
    }
    if !issuer.is_active_signing_account(signing_account) {
        return Err((
            "issuer_unauthorized",
            format!(
                "`{signing_account}` is not an active signing account for issuer `{issuer_id}`"
            ),
        ));
    }
    validate_namespaced_credential_type(&issuer.domain, credential_type)
        .map_err(|err| ("issuer_invalid_credential_type", err))?;
    if !issuer
        .allowed_credential_types
        .iter()
        .any(|ct| ct == credential_type)
    {
        return Err((
            "issuer_invalid_credential_type",
            format!("credential_type `{credential_type}` is not allowed by issuer `{issuer_id}`"),
        ));
    }
    if expires_at_height <= block_height {
        return Err((
            "credential_invalid_expiry",
            format!(
                "credential expires_at_height ({expires_at_height}) must exceed current block height ({block_height})"
            ),
        ));
    }
    let cid = credential_id(subject, issuer_id, credential_type);
    let (deposit_paid, deposit_payer) = if let Some(existing) = ledger.credentials.get(&cid) {
        // Safe renewal: reuse existing deposit without double charging, while overwriting
        // issued_height, signing_account, commitment_hash, expires_at_height, and status!
        (existing.deposit_paid, existing.deposit_payer.clone())
    } else {
        lock_credential_deposit(ledger, signing_account, CREDENTIAL_DEPOSIT_ATOMS)?;
        (CREDENTIAL_DEPOSIT_ATOMS, signing_account.to_string())
    };
    let record = CredentialRecordV1 {
        credential_id: cid.clone(),
        subject: subject.to_string(),
        issuer_id: issuer_id.to_string(),
        credential_type: credential_type.to_string(),
        signing_account: signing_account.to_string(),
        commitment_hash: commitment_hash.to_string(),
        issued_height: block_height,
        expires_at_height,
        status: if accepted {
            "accepted".to_string()
        } else {
            "pending".to_string()
        },
        deposit_paid,
        deposit_payer,
        accepted_at_height: if accepted { Some(block_height) } else { None },
        revoked_at_height: None,
    };
    record
        .validate()
        .map_err(|err| ("bad_credential_record", err))?;
    ledger.credentials.insert(cid, record);
    Ok(())
}

pub(crate) fn apply_credential_create(
    ledger: &mut LedgerState,
    operation: &CredentialCreateOperation,
    block_height: u64,
    compatibility: AssetExecutionCompatibility,
) -> Result<(), (&'static str, String)> {
    if !compatibility.credential_did_v1_active(block_height) {
        return Err((
            "credential_did_not_active",
            format!("credential_create is not active at block height {block_height}"),
        ));
    }
    upsert_credential_record(
        ledger,
        &operation.signing_account,
        &operation.issuer_id,
        &operation.subject,
        &operation.credential_type,
        &operation.commitment_hash,
        operation.expires_at_height,
        false,
        block_height,
    )
}

pub(crate) fn apply_credential_accept(
    ledger: &mut LedgerState,
    operation: &CredentialAcceptOperation,
    block_height: u64,
    compatibility: AssetExecutionCompatibility,
) -> Result<(), (&'static str, String)> {
    if !compatibility.credential_did_v1_active(block_height) {
        return Err((
            "credential_did_not_active",
            format!("credential_accept is not active at block height {block_height}"),
        ));
    }
    let record = ledger
        .credentials
        .get_mut(&operation.credential_id)
        .ok_or_else(|| {
            (
                "credential_not_found",
                format!("credential `{}` does not exist", operation.credential_id),
            )
        })?;
    if record.subject != operation.subject {
        return Err((
            "issuer_unauthorized",
            "only the credential subject may accept this credential".to_string(),
        ));
    }
    if record.status == "revoked" {
        return Err((
            "credential_revoked",
            "cannot accept a revoked credential".to_string(),
        ));
    }
    if block_height > record.expires_at_height {
        return Err((
            "credential_invalid_expiry",
            "cannot accept an expired credential".to_string(),
        ));
    }
    record.status = "accepted".to_string();
    record.accepted_at_height = Some(block_height);
    Ok(())
}

pub(crate) fn apply_credential_create_and_accept(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    operation: &CredentialCreateAndAcceptOperation,
    block_height: u64,
    compatibility: AssetExecutionCompatibility,
) -> Result<(), (&'static str, String)> {
    if !compatibility.credential_did_v1_active(block_height) {
        return Err((
            "credential_did_not_active",
            format!("credential_create_and_accept is not active at block height {block_height}"),
        ));
    }
    let pk = hex_to_bytes(&operation.subject_pubkey_hex).map_err(|_| {
        (
            "credential_delegated_accept_invalid",
            "subject_pubkey_hex is invalid hex".to_string(),
        )
    })?;
    let sig = hex_to_bytes(&operation.subject_accept_signature_hex).map_err(|_| {
        (
            "credential_delegated_accept_invalid",
            "subject_accept_signature_hex is invalid hex".to_string(),
        )
    })?;
    if address_from_public_key(&pk) != operation.subject {
        return Err((
            "credential_delegated_accept_invalid",
            "subject_pubkey_hex does not derive to operation.subject".to_string(),
        ));
    }
    let preimage = credential_delegated_accept_preimage(
        &genesis.chain_id,
        &genesis_hash(genesis),
        &operation.subject,
        &operation.issuer_id,
        &operation.credential_type,
        &operation.commitment_hash,
        operation.expires_at_height,
        operation.subject_accept_nonce,
    );
    if !ml_dsa_65_verify_with_context(
        &pk,
        &preimage,
        &sig,
        CREDENTIAL_DELEGATED_ACCEPT_DOMAIN.as_bytes(),
    ) {
        return Err((
            "credential_delegated_accept_invalid",
            "subject delegated acceptance ML-DSA-65 signature failed".to_string(),
        ));
    }
    // Zero-PFT onboarding: subject does NOT need an existing funded account on chain!
    upsert_credential_record(
        ledger,
        &operation.signing_account,
        &operation.issuer_id,
        &operation.subject,
        &operation.credential_type,
        &operation.commitment_hash,
        operation.expires_at_height,
        true,
        block_height,
    )
}

pub(crate) fn apply_credential_revoke(
    ledger: &mut LedgerState,
    operation: &CredentialRevokeOperation,
    block_height: u64,
    compatibility: AssetExecutionCompatibility,
) -> Result<(), (&'static str, String)> {
    if !compatibility.credential_did_v1_active(block_height) {
        return Err((
            "credential_did_not_active",
            format!("credential_revoke is not active at block height {block_height}"),
        ));
    }
    let issuer_id = ledger
        .credentials
        .get(&operation.credential_id)
        .ok_or_else(|| {
            (
                "credential_not_found",
                format!("credential `{}` does not exist", operation.credential_id),
            )
        })?
        .issuer_id
        .clone();
    let issuer = ledger.credential_issuers.get(&issuer_id).ok_or_else(|| {
        (
            "issuer_not_found",
            format!("issuer `{issuer_id}` does not exist"),
        )
    })?;
    if operation.caller != issuer.admin_account
        && !issuer.is_active_signing_account(&operation.caller)
    {
        return Err((
            "issuer_unauthorized",
            "caller must be issuer admin_account or an active signing_account".to_string(),
        ));
    }
    let record = ledger
        .credentials
        .get_mut(&operation.credential_id)
        .expect("credential verified above");
    record.status = "revoked".to_string();
    record.revoked_at_height = Some(block_height);
    Ok(())
}

pub(crate) fn apply_credential_delete(
    ledger: &mut LedgerState,
    operation: &CredentialDeleteOperation,
    block_height: u64,
    compatibility: AssetExecutionCompatibility,
) -> Result<(), (&'static str, String)> {
    if !compatibility.credential_did_v1_active(block_height) {
        return Err((
            "credential_did_not_active",
            format!("credential_delete is not active at block height {block_height}"),
        ));
    }
    let record = ledger
        .credentials
        .get(&operation.credential_id)
        .cloned()
        .ok_or_else(|| {
            (
                "credential_not_found",
                format!("credential `{}` does not exist", operation.credential_id),
            )
        })?;
    let is_expired = block_height > record.expires_at_height;
    let is_subject = operation.caller == record.subject;
    let is_issuer_auth = ledger
        .credential_issuers
        .get(&record.issuer_id)
        .is_some_and(|issuer| {
            operation.caller == issuer.admin_account
                || issuer.is_active_signing_account(&operation.caller)
        });
    if !is_expired && !is_subject && !is_issuer_auth {
        return Err((
            "credential_delete_unauthorized",
            "only subject, issuer, or anyone post-expiry may delete a credential".to_string(),
        ));
    }
    ledger.credentials.remove(&operation.credential_id);
    refund_credential_deposit(ledger, &record.deposit_payer, record.deposit_paid)?;
    Ok(())
}

fn is_credential_policy_tightening(
    old_mode: AssetApprovalMode,
    old_exit: AssetLapsedExitRule,
    old_policy: &CredentialPolicyV1,
    new_mode: AssetApprovalMode,
    new_exit: AssetLapsedExitRule,
    new_policy: &CredentialPolicyV1,
) -> bool {
    if old_mode == AssetApprovalMode::Either && new_mode == AssetApprovalMode::Both {
        return true;
    }
    if old_exit == AssetLapsedExitRule::ReturnOnly
        && new_exit == AssetLapsedExitRule::StrictFreezeClawback
    {
        return true;
    }
    // Every previously accepted OR-rule must still be satisfied by at least one new OR-rule
    // whose required AND-pairs are a subset of the old rule's AND-pairs; otherwise it tightens.
    for old_rule in &old_policy.any_of {
        let covered = new_policy.any_of.iter().any(|new_rule| {
            new_rule
                .all_of
                .iter()
                .all(|req| old_rule.all_of.contains(req))
        });
        if !covered {
            return true;
        }
    }
    false
}

pub(crate) fn apply_asset_access_set(
    ledger: &mut LedgerState,
    operation: &AssetAccessSetOperation,
    block_height: u64,
    compatibility: AssetExecutionCompatibility,
) -> Result<(), (&'static str, String)> {
    if !compatibility.credential_did_v1_active(block_height) {
        return Err((
            "credential_did_not_active",
            format!("asset_access_set is not active at block height {block_height}"),
        ));
    }
    let asset_idx = ledger
        .asset_definitions
        .iter()
        .position(|a| a.asset_id == operation.asset_id)
        .ok_or_else(|| {
            (
                "missing_asset",
                format!("asset `{}` does not exist", operation.asset_id),
            )
        })?;
    if ledger.asset_definitions[asset_idx].issuer != operation.issuer {
        return Err((
            "asset_issuer_mismatch",
            "asset_access_set issuer does not match asset issuer".to_string(),
        ));
    }
    if !ledger.asset_definitions[asset_idx].requires_authorization {
        return Err((
            "asset_gating_retroactive_enable_forbidden",
            "cannot enable gating or attach access_policy on a permissionless asset".to_string(),
        ));
    }

    if operation.disable_authorization_permanently {
        ledger.asset_definitions[asset_idx].requires_authorization = false;
        ledger.asset_definitions[asset_idx].access_policy = None;
        return Ok(());
    }

    if let Some(notice) = operation.notice_blocks {
        if notice < ASSET_ACCESS_MIN_TIGHTEN_NOTICE_BLOCKS {
            return Err((
                "asset_access_policy_bounds_exceeded",
                format!("notice_blocks must be >= {ASSET_ACCESS_MIN_TIGHTEN_NOTICE_BLOCKS}"),
            ));
        }
    }
    if let Some(policy) = &operation.credential_policy {
        if policy.any_of.len() > MAX_CREDENTIAL_POLICY_OR_CLAUSES
            || policy
                .any_of
                .iter()
                .any(|r| r.all_of.is_empty() || r.all_of.len() > MAX_CREDENTIAL_POLICY_AND_PAIRS)
        {
            return Err((
                "asset_access_policy_bounds_exceeded",
                "credential_policy exceeds OR/AND consensus bounds".to_string(),
            ));
        }
        policy
            .validate()
            .map_err(|err| ("asset_access_policy_bounds_exceeded", err))?;
    }

    let mut current = ledger.asset_definitions[asset_idx]
        .access_policy
        .clone()
        .unwrap_or(AssetAccessPolicyV1 {
            approval_mode: AssetApprovalMode::Either,
            lapsed_exit_rule: AssetLapsedExitRule::ReturnOnly,
            wrapper_mode: AssetWrapperMode::PftlOnly,
            credential_policy: CredentialPolicyV1 { any_of: Vec::new() },
            notice_blocks: ASSET_ACCESS_MIN_TIGHTEN_NOTICE_BLOCKS,
            pending_policy: None,
        });

    // Promote any already-matured pending policy before computing delta.
    if let Some(pending) = current.pending_policy.clone() {
        if block_height >= pending.effective_at_height {
            current.approval_mode = pending.approval_mode;
            current.lapsed_exit_rule = pending.lapsed_exit_rule;
            current.wrapper_mode = pending.wrapper_mode;
            current.credential_policy = pending.credential_policy;
            current.pending_policy = None;
        }
    }

    // Emergency escape hatch: immediately strip a deregistered or compromised issuer_id.
    if let Some(remove_id) = &operation.emergency_remove_issuer_id {
        let eligible = ledger
            .credential_issuers
            .get(remove_id)
            .is_some_and(|issuer| {
                issuer.status == "deregistered" || issuer.has_compromised_signing_account()
            });
        if !eligible {
            return Err((
                "asset_access_emergency_remove_not_eligible",
                format!("issuer `{remove_id}` is neither deregistered nor compromised"),
            ));
        }
        current
            .credential_policy
            .any_of
            .retain(|rule| !rule.all_of.iter().any(|pair| &pair.issuer_id == remove_id));
        if let Some(pending) = &mut current.pending_policy {
            pending
                .credential_policy
                .any_of
                .retain(|rule| !rule.all_of.iter().any(|pair| &pair.issuer_id == remove_id));
        }
        ledger.asset_definitions[asset_idx].access_policy = Some(current);
        return Ok(());
    }

    let new_mode = operation.approval_mode.unwrap_or(current.approval_mode);
    let new_exit = operation
        .lapsed_exit_rule
        .unwrap_or(current.lapsed_exit_rule);
    let new_wrapper = operation.wrapper_mode.unwrap_or(current.wrapper_mode);
    let new_cred_policy = operation
        .credential_policy
        .clone()
        .unwrap_or_else(|| current.credential_policy.clone());
    let new_notice = operation.notice_blocks.unwrap_or(current.notice_blocks);
    let notice_reduced = new_notice < current.notice_blocks;
    let tightens = notice_reduced
        || is_credential_policy_tightening(
            current.approval_mode,
            current.lapsed_exit_rule,
            &current.credential_policy,
            new_mode,
            new_exit,
            &new_cred_policy,
        );

    if tightens {
        let delay = current
            .notice_blocks
            .max(ASSET_ACCESS_MIN_TIGHTEN_NOTICE_BLOCKS);
        current.pending_policy = Some(PendingAssetAccessPolicyV1 {
            approval_mode: new_mode,
            lapsed_exit_rule: new_exit,
            wrapper_mode: new_wrapper,
            credential_policy: new_cred_policy,
            proposed_at_height: block_height,
            effective_at_height: block_height.saturating_add(delay),
        });
    } else {
        current.approval_mode = new_mode;
        current.lapsed_exit_rule = new_exit;
        current.wrapper_mode = new_wrapper;
        current.credential_policy = new_cred_policy;
        current.notice_blocks = new_notice;
        current.pending_policy = None;
    }

    current
        .validate()
        .map_err(|err| ("asset_access_policy_bounds_exceeded", err))?;
    ledger.asset_definitions[asset_idx].access_policy = Some(current);
    Ok(())
}
