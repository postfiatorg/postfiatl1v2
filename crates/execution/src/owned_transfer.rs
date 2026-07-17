// On-chain execution of FastPay owned-value transfers.
//
// Applies a certified `OwnedTransferOrder` to `LedgerState.owned_objects`,
// enforcing single-consumption (version monotonicity) and single-asset value
// conservation. The certificate itself is produced + verified off-chain by the
// consensusless fast path (`crates/fastpay-prototype`); this is the
// deterministic state transition that lands the certified transfer in the
// ledger. Replays are rejected because a consumed input's version no longer
// matches (the object is retired).
//
// NOTE: this file is `include!`-ed at the execution crate root alongside the
// other source components, so it shares their scope — `LedgerState` is already in scope
// (via entrypoints.rs). We use full paths for everything else to avoid
// duplicate-import conflicts across the shared root scope.

pub const OWNED_TRANSFER_CONTEXT: &[u8] = b"postfiat-l1-v2/owned-transfer/v2";
pub const OWNED_UNWRAP_CONTEXT: &[u8] = b"postfiat-l1-v2/owned-unwrap/v2";

/// The only asset the wrap bridge may mint. `wrap_to_owned` debits the native
/// `Account.balance`, so a non-native `asset` label would mint an object that
/// claims to be an issued asset while actually being native-PFT-backed.
/// Issued-asset deposits require a trustline-debiting consensus deposit path
/// (FastSwapV1 packet P6); until then they fail closed here.
pub const OWNED_NATIVE_ASSET: &str = "PFT";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnedTransferError {
    EmptyInputs,
    EmptyOutputs,
    DuplicateInput,
    UnknownInput,
    VersionMismatch,
    NotOwner,
    Overflow,
    MixedAssets,
    NotConserved,
    OwnerAuthFailed,
    InsufficientQuorum { have: usize, need: usize },
    ResourceLimitExceeded,
    UnsupportedAsset,
    InvalidDomain,
    InvalidSequence,
    Expired,
    DuplicateOutput,
    InvalidRecovery,
    NotYetValid,
    VersionFenced,
}

fn append_owned_certificate_domain(
    out: &mut Vec<u8>,
    domain: &postfiat_types::OwnedCertificateDomain,
) {
    for value in [
        domain.schema.as_str(),
        domain.chain_id.as_str(),
        domain.genesis_hash.as_str(),
        domain.registry_id.as_str(),
    ] {
        out.extend(&(value.len() as u64).to_le_bytes());
        out.extend(value.as_bytes());
    }
    out.extend(&domain.protocol_version.to_le_bytes());
}

fn valid_owned_certificate_domain(domain: &postfiat_types::OwnedCertificateDomain) -> bool {
    domain.schema == postfiat_types::OWNED_CERTIFICATE_DOMAIN_SCHEMA_V2
        && !domain.chain_id.is_empty()
        && domain.chain_id.len() <= 128
        && domain.protocol_version > 0
        && postfiat_crypto_provider::hex_to_bytes(&domain.genesis_hash)
            .is_ok_and(|bytes| bytes.len() == 48)
        && postfiat_crypto_provider::hex_to_bytes(&domain.registry_id)
            .is_ok_and(|bytes| bytes.len() == 48)
}

/// Canonical, domain-separated bytes covered by the owner authorization AND
/// every validator vote on an owned-transfer order. Owner and validators sign
/// these exact bytes, so a validator vote binds the validator to the order.
pub fn owned_transfer_signing_bytes(order: &postfiat_types::OwnedTransferOrder) -> Vec<u8> {
    let mut out = b"postfiat.owned-transfer.v2\0".to_vec();
    append_owned_certificate_domain(&mut out, &order.domain);
    out.extend(&(order.inputs.len() as u64).to_le_bytes());
    for r in &order.inputs {
        out.extend(&(r.id.len() as u64).to_le_bytes());
        out.extend(r.id.as_bytes());
        out.extend(&r.version.to_le_bytes());
    }
    out.extend(&(order.outputs.len() as u64).to_le_bytes());
    for o in &order.outputs {
        out.extend(&(o.owner_pubkey_hex.len() as u64).to_le_bytes());
        out.extend(o.owner_pubkey_hex.as_bytes());
        out.extend(&o.value.to_le_bytes());
        out.extend(&(o.asset.len() as u64).to_le_bytes());
        out.extend(o.asset.as_bytes());
    }
    out.extend(&order.fee.to_le_bytes());
    out.extend(&order.nonce.to_le_bytes());
    out.extend(&(order.memos.len() as u64).to_le_bytes());
    for m in &order.memos {
        out.extend(m.memo_type.as_bytes());
        out.push(0);
        out.extend(m.memo_format.as_bytes());
        out.push(0);
        out.extend(m.memo_data.as_bytes());
        out.push(0);
    }
    out
}

/// Canonical bytes covered by the owner authorization and validator votes on
/// an owned-unwrap order.
pub fn owned_unwrap_signing_bytes(order: &postfiat_types::OwnedUnwrapOrder) -> Vec<u8> {
    let mut out = b"postfiat.owned-unwrap.v2\0".to_vec();
    append_owned_certificate_domain(&mut out, &order.domain);
    out.extend(&(order.inputs.len() as u64).to_le_bytes());
    for r in &order.inputs {
        out.extend(&(r.id.len() as u64).to_le_bytes());
        out.extend(r.id.as_bytes());
        out.extend(&r.version.to_le_bytes());
    }
    out.extend(&(order.to_address.len() as u64).to_le_bytes());
    out.extend(order.to_address.as_bytes());
    out.extend(&order.amount.to_le_bytes());
    out.extend(&(order.asset.len() as u64).to_le_bytes());
    out.extend(order.asset.as_bytes());
    out.extend(&order.fee.to_le_bytes());
    out.extend(&order.nonce.to_le_bytes());
    out.extend(&(order.memos.len() as u64).to_le_bytes());
    for m in &order.memos {
        out.extend(m.memo_type.as_bytes());
        out.push(0);
        out.extend(m.memo_format.as_bytes());
        out.push(0);
        out.extend(m.memo_data.as_bytes());
        out.push(0);
    }
    out
}

/// Verify a consensusless certificate: owner authorization + count valid
/// validator votes (each checked against the provided pubkey set). Returns
/// `Some(valid_count)` if owner auth passes (caller checks >= quorum), or `None`
/// if owner auth fails.
pub fn verify_owned_certificate(
    cert: &postfiat_types::OwnedTransferCertificate,
    validator_pks: &[(String, String)],
    expected_domain: &postfiat_types::OwnedCertificateDomain,
) -> Option<usize> {
    if &cert.order.domain != expected_domain || !valid_owned_certificate_domain(expected_domain) {
        return None;
    }
    let bytes = owned_transfer_signing_bytes(&cert.order);
    let owner_pk = postfiat_crypto_provider::hex_to_bytes(&cert.owner_pubkey_hex).ok()?;
    let owner_sig = postfiat_crypto_provider::hex_to_bytes(&cert.owner_signature_hex).ok()?;
    if !postfiat_crypto_provider::ml_dsa_65_verify_with_context(
        &owner_pk,
        &bytes,
        &owner_sig,
        OWNED_TRANSFER_CONTEXT,
    ) {
        return None;
    }
    let mut valid = 0usize;
    let mut seen_validators = std::collections::BTreeSet::new();
    for vote in &cert.votes {
        if !seen_validators.insert(vote.validator_id.as_str()) {
            return None;
        }
        let Some((_, pk_hex)) = validator_pks.iter().find(|(id, _)| *id == vote.validator_id) else {
            continue;
        };
        let (Ok(pk), Ok(sig)) = (
            postfiat_crypto_provider::hex_to_bytes(pk_hex),
            postfiat_crypto_provider::hex_to_bytes(&vote.signature_hex),
        ) else {
            continue;
        };
        if postfiat_crypto_provider::ml_dsa_65_verify_with_context(&pk, &bytes, &sig, OWNED_TRANSFER_CONTEXT) {
            valid += 1;
        }
    }
    Some(valid)
}

/// Verify an owned-unwrap certificate. Returns `Some(valid_count)` if owner
/// auth passes; caller enforces quorum.
pub fn verify_owned_unwrap_certificate(
    cert: &postfiat_types::OwnedUnwrapCertificate,
    validator_pks: &[(String, String)],
    expected_domain: &postfiat_types::OwnedCertificateDomain,
) -> Option<usize> {
    if &cert.order.domain != expected_domain || !valid_owned_certificate_domain(expected_domain) {
        return None;
    }
    let bytes = owned_unwrap_signing_bytes(&cert.order);
    let owner_pk = postfiat_crypto_provider::hex_to_bytes(&cert.owner_pubkey_hex).ok()?;
    let owner_sig = postfiat_crypto_provider::hex_to_bytes(&cert.owner_signature_hex).ok()?;
    if !postfiat_crypto_provider::ml_dsa_65_verify_with_context(
        &owner_pk,
        &bytes,
        &owner_sig,
        OWNED_UNWRAP_CONTEXT,
    ) {
        return None;
    }
    let mut valid = 0usize;
    let mut seen_validators = std::collections::BTreeSet::new();
    for vote in &cert.votes {
        if !seen_validators.insert(vote.validator_id.as_str()) {
            return None;
        }
        let Some((_, pk_hex)) = validator_pks.iter().find(|(id, _)| *id == vote.validator_id) else {
            continue;
        };
        let (Ok(pk), Ok(sig)) = (
            postfiat_crypto_provider::hex_to_bytes(pk_hex),
            postfiat_crypto_provider::hex_to_bytes(&vote.signature_hex),
        ) else {
            continue;
        };
        if postfiat_crypto_provider::ml_dsa_65_verify_with_context(&pk, &bytes, &sig, OWNED_UNWRAP_CONTEXT) {
            valid += 1;
        }
    }
    Some(valid)
}

/// Validate owner authorization and all live-state/resource rules before a
/// validator persists a transfer lock.
pub fn validate_owned_transfer_admission(
    ledger: &LedgerState,
    signed: &postfiat_types::SignedOwnedTransferOrder,
    expected_domain: &postfiat_types::OwnedCertificateDomain,
) -> Result<(), OwnedTransferError> {
    if &signed.order.domain != expected_domain || !valid_owned_certificate_domain(expected_domain) {
        return Err(OwnedTransferError::InvalidDomain);
    }
    let signing_bytes = owned_transfer_signing_bytes(&signed.order);
    if !verify_owned_owner_authorization(
        &signed.owner_pubkey_hex,
        &signed.owner_signature_hex,
        &signing_bytes,
        OWNED_TRANSFER_CONTEXT,
    ) {
        return Err(OwnedTransferError::OwnerAuthFailed);
    }
    prepare_owned_transfer(ledger, &signed.order, &signed.owner_pubkey_hex).map(|_| ())
}

/// Unwrap shares transfer input locks and therefore receives the same
/// fail-closed admission validation.
pub fn validate_owned_unwrap_admission(
    ledger: &LedgerState,
    signed: &postfiat_types::SignedOwnedUnwrapOrder,
    expected_domain: &postfiat_types::OwnedCertificateDomain,
) -> Result<(), OwnedTransferError> {
    if &signed.order.domain != expected_domain || !valid_owned_certificate_domain(expected_domain) {
        return Err(OwnedTransferError::InvalidDomain);
    }
    let signing_bytes = owned_unwrap_signing_bytes(&signed.order);
    if !verify_owned_owner_authorization(
        &signed.owner_pubkey_hex,
        &signed.owner_signature_hex,
        &signing_bytes,
        OWNED_UNWRAP_CONTEXT,
    ) {
        return Err(OwnedTransferError::OwnerAuthFailed);
    }
    prepare_owned_unwrap(ledger, &signed.order, &signed.owner_pubkey_hex).map(|_| ())
}

fn verify_owned_owner_authorization(
    owner_pubkey_hex: &str,
    owner_signature_hex: &str,
    signing_bytes: &[u8],
    context: &[u8],
) -> bool {
    let (Ok(owner_pk), Ok(owner_sig)) = (
        postfiat_crypto_provider::hex_to_bytes(owner_pubkey_hex),
        postfiat_crypto_provider::hex_to_bytes(owner_signature_hex),
    ) else {
        return false;
    };
    postfiat_crypto_provider::ml_dsa_65_verify_with_context(
        &owner_pk,
        signing_bytes,
        &owner_sig,
        context,
    )
}

/// Apply a CERTIFIED owned-transfer — the production path. Verifies the
/// certificate (owner auth + >= `quorum` valid validator votes) before applying;
/// a bare submitted order is never trusted.
pub fn apply_owned_certificate(
    ledger: &mut LedgerState,
    cert: &postfiat_types::OwnedTransferCertificate,
    validator_pks: &[(String, String)],
    expected_domain: &postfiat_types::OwnedCertificateDomain,
    quorum: usize,
) -> Result<OwnedTransferOutcome, OwnedTransferError> {
    if &cert.order.domain != expected_domain || !valid_owned_certificate_domain(expected_domain) {
        return Err(OwnedTransferError::InvalidDomain);
    }
    let valid_votes = verify_owned_certificate(cert, validator_pks, expected_domain)
        .ok_or(OwnedTransferError::OwnerAuthFailed)?;
    if valid_votes < quorum {
        return Err(OwnedTransferError::InsufficientQuorum {
            have: valid_votes,
            need: quorum,
        });
    }
    apply_owned_transfer(ledger, &cert.order, &cert.owner_pubkey_hex)
}

/// Apply a CERTIFIED owned-unwrap. Verifies owner auth and validator quorum
/// before crediting the account lane.
pub fn apply_owned_unwrap_certificate(
    ledger: &mut LedgerState,
    cert: &postfiat_types::OwnedUnwrapCertificate,
    validator_pks: &[(String, String)],
    expected_domain: &postfiat_types::OwnedCertificateDomain,
    quorum: usize,
) -> Result<OwnedUnwrapOutcome, OwnedTransferError> {
    if &cert.order.domain != expected_domain || !valid_owned_certificate_domain(expected_domain) {
        return Err(OwnedTransferError::InvalidDomain);
    }
    let valid_votes = verify_owned_unwrap_certificate(cert, validator_pks, expected_domain)
        .ok_or(OwnedTransferError::OwnerAuthFailed)?;
    if valid_votes < quorum {
        return Err(OwnedTransferError::InsufficientQuorum {
            have: valid_votes,
            need: quorum,
        });
    }
    apply_owned_unwrap(ledger, &cert.order, &cert.owner_pubkey_hex)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedTransferOutcome {
    pub consumed: usize,
    pub created: Vec<postfiat_types::OwnedObject>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedUnwrapOutcome {
    pub consumed: usize,
    pub credited: u64,
    pub credited_to: String,
    pub change_object: Option<postfiat_types::OwnedObject>,
}

/// Apply a certified owned-transfer to the ledger. Consume input objects at
/// their current versions, mint outputs at fresh content-addressed ids, enforce
/// single-asset value conservation. `owner_pubkey_hex` is the certificate owner
/// and must own every input.
pub fn apply_owned_transfer(
    ledger: &mut LedgerState,
    order: &postfiat_types::OwnedTransferOrder,
    owner_pubkey_hex: &str,
) -> Result<OwnedTransferOutcome, OwnedTransferError> {
    let mut consume_indices = prepare_owned_transfer(ledger, order, owner_pubkey_hex)?;

    // Retire consumed inputs (descending so swap_remove keeps earlier indices valid).
    consume_indices.sort_unstable_by(|a, b| b.cmp(a));
    for idx in consume_indices {
        ledger.owned_objects.swap_remove(idx);
    }
    // Mint outputs at fresh content-addressed ids.
    let mut created = Vec::new();
    for (i, spec) in order.outputs.iter().enumerate() {
        let id = owned_output_id(owner_pubkey_hex, order.nonce, i, spec);
        let obj = postfiat_types::OwnedObject {
            id,
            version: 1,
            owner_pubkey_hex: spec.owner_pubkey_hex.clone(),
            value: spec.value,
            asset: spec.asset.clone(),
        };
        ledger.owned_objects.push(obj.clone());
        created.push(obj);
    }
    Ok(OwnedTransferOutcome {
        consumed: order.inputs.len(),
        created,
    })
}

fn prepare_owned_transfer(
    ledger: &LedgerState,
    order: &postfiat_types::OwnedTransferOrder,
    owner_pubkey_hex: &str,
) -> Result<Vec<usize>, OwnedTransferError> {
    if order.inputs.is_empty() {
        return Err(OwnedTransferError::EmptyInputs);
    }
    if order.outputs.is_empty() {
        return Err(OwnedTransferError::EmptyOutputs);
    }
    if order.inputs.len() > postfiat_types::MAX_OWNED_INPUTS_PER_TRANSFER
        || order.outputs.len() > postfiat_types::MAX_OWNED_OUTPUTS_PER_TRANSFER
    {
        return Err(OwnedTransferError::ResourceLimitExceeded);
    }
    validate_owned_memos(&order.memos)?;
    let net_new = order.outputs.len() as isize - order.inputs.len() as isize;
    if (ledger.owned_objects.len() as isize + net_new) > postfiat_types::MAX_OWNED_OBJECTS as isize {
        return Err(OwnedTransferError::ResourceLimitExceeded);
    }

    let mut input_value = 0u64;
    let mut asset: Option<&str> = None;
    let mut consume_indices = Vec::with_capacity(order.inputs.len());
    let mut input_ids = std::collections::BTreeSet::new();
    for input in &order.inputs {
        if !input_ids.insert(input.id.as_str()) {
            return Err(OwnedTransferError::DuplicateInput);
        }
        let (index, object) = ledger
            .owned_objects
            .iter()
            .enumerate()
            .find(|(_, object)| object.id == input.id)
            .ok_or(OwnedTransferError::UnknownInput)?;
        if object.version != input.version {
            return Err(OwnedTransferError::VersionMismatch);
        }
        if object.owner_pubkey_hex != owner_pubkey_hex {
            return Err(OwnedTransferError::NotOwner);
        }
        input_value = input_value
            .checked_add(object.value)
            .ok_or(OwnedTransferError::Overflow)?;
        match asset {
            None => asset = Some(object.asset.as_str()),
            Some(expected) if expected == object.asset => {}
            Some(_) => return Err(OwnedTransferError::MixedAssets),
        }
        consume_indices.push(index);
    }
    let asset = asset.ok_or(OwnedTransferError::EmptyInputs)?;
    let mut output_ids = std::collections::BTreeSet::new();
    for (index, output) in order.outputs.iter().enumerate() {
        if output.asset != asset {
            return Err(OwnedTransferError::MixedAssets);
        }
        if output.value == 0 {
            return Err(OwnedTransferError::NotConserved);
        }
        let output_id = owned_output_id(owner_pubkey_hex, order.nonce, index, output);
        if !output_ids.insert(output_id.clone())
            || ledger
                .owned_objects
                .iter()
                .any(|object| object.id == output_id)
        {
            return Err(OwnedTransferError::DuplicateOutput);
        }
    }
    let output_value = order.outputs.iter().try_fold(0u64, |total, output| {
        total.checked_add(output.value).ok_or(OwnedTransferError::Overflow)
    })?;
    if output_value.checked_add(order.fee) != Some(input_value) {
        return Err(OwnedTransferError::NotConserved);
    }
    Ok(consume_indices)
}

/// Apply a certified owned-unwrap to the ledger. Consumes one or more owned
/// objects, credits `order.amount` to the account lane, and mints one owned
/// change object back to the owner when the selected input value exceeds
/// amount + fee.
pub fn apply_owned_unwrap(
    ledger: &mut LedgerState,
    order: &postfiat_types::OwnedUnwrapOrder,
    owner_pubkey_hex: &str,
) -> Result<OwnedUnwrapOutcome, OwnedTransferError> {
    let (mut consume_indices, input_value) =
        prepare_owned_unwrap(ledger, order, owner_pubkey_hex)?;
    let required = order
        .amount
        .checked_add(order.fee)
        .ok_or(OwnedTransferError::Overflow)?;
    let change = input_value - required;
    let net_new = (if change > 0 { 1isize } else { 0isize }) - order.inputs.len() as isize;
    if (ledger.owned_objects.len() as isize + net_new) > postfiat_types::MAX_OWNED_OBJECTS as isize {
        return Err(OwnedTransferError::ResourceLimitExceeded);
    }

    let credited_balance = ledger
        .account(&order.to_address)
        .map(|account| account.balance)
        .unwrap_or_default()
        .checked_add(order.amount)
        .ok_or(OwnedTransferError::Overflow)?;

    let change_object = if change > 0 {
        let spec = postfiat_types::OwnedOutputSpec {
            owner_pubkey_hex: owner_pubkey_hex.to_string(),
            value: change,
            asset: order.asset.clone(),
        };
        let id = owned_output_id(owner_pubkey_hex, order.nonce, 0, &spec);
        if ledger.owned_objects.iter().any(|object| object.id == id) {
            return Err(OwnedTransferError::DuplicateOutput);
        }
        Some(postfiat_types::OwnedObject {
            id,
            version: 1,
            owner_pubkey_hex: owner_pubkey_hex.to_string(),
            value: change,
            asset: order.asset.clone(),
        })
    } else {
        None
    };

    consume_indices.sort_unstable_by(|a, b| b.cmp(a));
    for idx in consume_indices {
        ledger.owned_objects.swap_remove(idx);
    }

    let account = ledger.ensure_account(&order.to_address);
    account.balance = credited_balance;

    if let Some(object) = &change_object {
        ledger.owned_objects.push(object.clone());
    }

    Ok(OwnedUnwrapOutcome {
        consumed: order.inputs.len(),
        credited: order.amount,
        credited_to: order.to_address.clone(),
        change_object,
    })
}

fn prepare_owned_unwrap(
    ledger: &LedgerState,
    order: &postfiat_types::OwnedUnwrapOrder,
    owner_pubkey_hex: &str,
) -> Result<(Vec<usize>, u64), OwnedTransferError> {
    if order.inputs.is_empty() {
        return Err(OwnedTransferError::EmptyInputs);
    }
    if order.amount == 0 || order.asset.is_empty() || order.to_address.is_empty() {
        return Err(OwnedTransferError::NotConserved);
    }
    if order.asset != OWNED_NATIVE_ASSET {
        return Err(OwnedTransferError::UnsupportedAsset);
    }
    if order.inputs.len() > postfiat_types::MAX_OWNED_INPUTS_PER_TRANSFER {
        return Err(OwnedTransferError::ResourceLimitExceeded);
    }
    validate_owned_memos(&order.memos)?;

    let mut input_value = 0u64;
    let mut consume_indices = Vec::with_capacity(order.inputs.len());
    let mut input_ids = std::collections::BTreeSet::new();
    for input in &order.inputs {
        if !input_ids.insert(input.id.as_str()) {
            return Err(OwnedTransferError::DuplicateInput);
        }
        let (index, object) = ledger
            .owned_objects
            .iter()
            .enumerate()
            .find(|(_, object)| object.id == input.id)
            .ok_or(OwnedTransferError::UnknownInput)?;
        if object.version != input.version {
            return Err(OwnedTransferError::VersionMismatch);
        }
        if object.owner_pubkey_hex != owner_pubkey_hex {
            return Err(OwnedTransferError::NotOwner);
        }
        if object.asset != order.asset {
            return Err(OwnedTransferError::MixedAssets);
        }
        input_value = input_value
            .checked_add(object.value)
            .ok_or(OwnedTransferError::Overflow)?;
        consume_indices.push(index);
    }

    let required = order
        .amount
        .checked_add(order.fee)
        .ok_or(OwnedTransferError::Overflow)?;
    if input_value < required {
        return Err(OwnedTransferError::NotConserved);
    }
    let change = input_value - required;
    let net_new = (if change > 0 { 1isize } else { 0isize }) - order.inputs.len() as isize;
    if (ledger.owned_objects.len() as isize + net_new) > postfiat_types::MAX_OWNED_OBJECTS as isize {
        return Err(OwnedTransferError::ResourceLimitExceeded);
    }
    Ok((consume_indices, input_value))
}

fn validate_owned_memos(
    memos: &[postfiat_types::PaymentMemo],
) -> Result<(), OwnedTransferError> {
    if memos.len() > postfiat_types::MAX_PAYMENT_MEMOS {
        return Err(OwnedTransferError::ResourceLimitExceeded);
    }
    let mut total_bytes = 0usize;
    for memo in memos {
        memo.validate()
            .map_err(|_| OwnedTransferError::ResourceLimitExceeded)?;
        total_bytes = total_bytes
            .checked_add(memo.byte_len())
            .ok_or(OwnedTransferError::ResourceLimitExceeded)?;
    }
    if total_bytes > postfiat_types::MAX_PAYMENT_MEMO_TOTAL_BYTES {
        return Err(OwnedTransferError::ResourceLimitExceeded);
    }
    Ok(())
}

fn owned_output_id(owner: &str, nonce: u64, index: usize, spec: &postfiat_types::OwnedOutputSpec) -> String {
    let mut material = Vec::new();
    material.extend_from_slice(owner.as_bytes());
    material.extend(&nonce.to_le_bytes());
    material.extend(&(index as u64).to_le_bytes());
    material.extend_from_slice(spec.owner_pubkey_hex.as_bytes());
    material.extend(&spec.value.to_le_bytes());
    material.extend_from_slice(spec.asset.as_bytes());
    let h = postfiat_crypto_provider::hash_bytes("postfiat.owned-output.v1", &material);
    postfiat_crypto_provider::bytes_to_hex(&h[..32])
}

/// Wrap: debit an account balance and mint an owned object of equal value
/// (account lane -> owned lane). Value-conserving within the ledger. Native
/// PFT only: the debit always comes from `Account.balance`, so any other
/// asset label is rejected rather than minting a mislabeled object.
pub fn wrap_to_owned(
    ledger: &mut LedgerState,
    from_address: &str,
    owner_pubkey_hex: String,
    amount: u64,
    asset: String,
    object_id: String,
) -> Result<postfiat_types::OwnedObject, OwnedTransferError> {
    if asset != OWNED_NATIVE_ASSET {
        return Err(OwnedTransferError::UnsupportedAsset);
    }
    if amount == 0 {
        return Err(OwnedTransferError::NotConserved);
    }
    if ledger
        .owned_objects
        .iter()
        .any(|object| object.id == object_id)
    {
        return Err(OwnedTransferError::DuplicateOutput);
    }
    {
        let account = ledger
            .account_mut(from_address)
            .ok_or(OwnedTransferError::UnknownInput)?;
        if account.balance < amount {
            return Err(OwnedTransferError::NotConserved);
        }
        account.balance -= amount;
    }
    let obj = postfiat_types::OwnedObject {
        id: object_id,
        version: 1,
        owner_pubkey_hex,
        value: amount,
        asset,
    };
    ledger.owned_objects.push(obj.clone());
    Ok(obj)
}

/// Apply a source-account-signed deposit through the consensus-ordered primary
/// lane. The transition is constructed on a clone and published only after all
/// authorization, sequence, expiry, balance, and collision checks succeed.
pub fn apply_owned_deposit(
    ledger: &mut LedgerState,
    signed: &postfiat_types::SignedOwnedDepositV1,
    finalized_height: u64,
) -> Result<postfiat_types::OwnedObject, OwnedTransferError> {
    let deposit = &signed.deposit;
    if deposit.asset != OWNED_NATIVE_ASSET {
        return Err(OwnedTransferError::UnsupportedAsset);
    }
    if deposit.amount_atoms == 0 || deposit.fee_pft == 0 {
        return Err(OwnedTransferError::NotConserved);
    }
    if deposit.valid_through_height < finalized_height {
        return Err(OwnedTransferError::Expired);
    }
    if signed.algorithm_id != postfiat_crypto_provider::ML_DSA_65_ALGORITHM
        || !postfiat_crypto_provider::ml_dsa_65_verify_with_context(
            &deposit.source_pubkey,
            &deposit
                .signing_bytes()
                .map_err(|_| OwnedTransferError::InvalidDomain)?,
            &signed.signature,
            postfiat_types::OWNED_DEPOSIT_CONTEXT_V1,
        )
    {
        return Err(OwnedTransferError::OwnerAuthFailed);
    }
    if postfiat_crypto_provider::address_from_public_key(&deposit.source_pubkey)
        != deposit.source_address
        || deposit.destination_owner_pubkey.len()
            != postfiat_crypto_provider::ML_DSA_65_PUBLIC_KEY_BYTES
    {
        return Err(OwnedTransferError::OwnerAuthFailed);
    }
    let signing_bytes = deposit
        .signing_bytes()
        .map_err(|_| OwnedTransferError::InvalidDomain)?;
    let object_hash = postfiat_crypto_provider::hash_bytes(
        "postfiat.owned-deposit-object.v1",
        &signing_bytes,
    );
    let object_id = postfiat_crypto_provider::bytes_to_hex(&object_hash[..32]);
    if ledger
        .owned_objects
        .iter()
        .any(|object| object.id == object_id)
    {
        return Err(OwnedTransferError::DuplicateOutput);
    }
    let mut next = ledger.clone();
    let account = next
        .account_mut(&deposit.source_address)
        .ok_or(OwnedTransferError::UnknownInput)?;
    if account.sequence.checked_add(1) != Some(deposit.sequence) {
        return Err(OwnedTransferError::InvalidSequence);
    }
    let source_pubkey_hex =
        postfiat_crypto_provider::bytes_to_hex(&deposit.source_pubkey);
    if account
        .public_key_hex
        .as_ref()
        .is_some_and(|public_key| public_key != &source_pubkey_hex)
    {
        return Err(OwnedTransferError::NotOwner);
    }
    let debit = deposit
        .amount_atoms
        .checked_add(deposit.fee_pft)
        .ok_or(OwnedTransferError::Overflow)?;
    account.balance = account
        .balance
        .checked_sub(debit)
        .ok_or(OwnedTransferError::NotConserved)?;
    account.sequence = deposit.sequence;
    if account.public_key_hex.is_none() {
        account.public_key_hex = Some(source_pubkey_hex);
    }
    let object = postfiat_types::OwnedObject {
        id: object_id,
        version: 1,
        owner_pubkey_hex: postfiat_crypto_provider::bytes_to_hex(
            &deposit.destination_owner_pubkey,
        ),
        value: deposit.amount_atoms,
        asset: deposit.asset.clone(),
    };
    next.owned_objects.push(object.clone());
    *ledger = next;
    Ok(object)
}

/// Unwrap: retire an owned object and credit its value to an account balance
/// (owned lane -> account lane). Only the object's owner may unwrap it.
pub fn unwrap_from_owned(
    ledger: &mut LedgerState,
    object_id: &str,
    owner_pubkey_hex: &str,
    to_address: &str,
) -> Result<u64, OwnedTransferError> {
    let idx = ledger
        .owned_objects
        .iter()
        .position(|o| o.id == object_id)
        .ok_or(OwnedTransferError::UnknownInput)?;
    if ledger.owned_objects[idx].owner_pubkey_hex != owner_pubkey_hex {
        return Err(OwnedTransferError::NotOwner);
    }
    if ledger.owned_objects[idx].asset != OWNED_NATIVE_ASSET {
        return Err(OwnedTransferError::UnsupportedAsset);
    }
    if to_address.is_empty() {
        return Err(OwnedTransferError::NotConserved);
    }
    let credited_balance = ledger
        .account(to_address)
        .map(|account| account.balance)
        .unwrap_or_default()
        .checked_add(ledger.owned_objects[idx].value)
        .ok_or(OwnedTransferError::Overflow)?;
    let obj = ledger.owned_objects.swap_remove(idx);
    let account = ledger.ensure_account(to_address);
    account.balance = credited_balance;
    Ok(obj.value)
}

#[cfg(test)]
mod owned_transfer_tests {
    use super::*;

    fn domain() -> postfiat_types::OwnedCertificateDomain {
        postfiat_types::OwnedCertificateDomain {
            schema: postfiat_types::OWNED_CERTIFICATE_DOMAIN_SCHEMA_V2.to_string(),
            chain_id: "postfiat-owned-test".to_string(),
            genesis_hash: "ab".repeat(48),
            protocol_version: 1,
            registry_id: "cd".repeat(48),
        }
    }

    fn object(id: &str, owner: &str, value: u64, asset: &str) -> postfiat_types::OwnedObject {
        postfiat_types::OwnedObject {
            id: id.into(),
            version: 1,
            owner_pubkey_hex: owner.into(),
            value,
            asset: asset.into(),
        }
    }

    #[test]
    fn applies_conserving_transfer() {
        let mut ledger = LedgerState::empty();
        ledger.owned_objects.push(object("aa", "ownerA", 100, "PFT"));
        let order = postfiat_types::OwnedTransferOrder {
            domain: domain(),
            inputs: vec![postfiat_types::OwnedObjectRef { id: "aa".into(), version: 1 }],
            outputs: vec![
                postfiat_types::OwnedOutputSpec { owner_pubkey_hex: "recipient".into(), value: 90, asset: "PFT".into() },
                postfiat_types::OwnedOutputSpec { owner_pubkey_hex: "ownerA".into(), value: 9, asset: "PFT".into() },
            ],
            fee: 1,
            nonce: 1,
            memos: Vec::new(),
        };
        let out = apply_owned_transfer(&mut ledger, &order, "ownerA").expect("apply");
        assert_eq!(out.consumed, 1);
        assert_eq!(out.created.len(), 2);
        assert_eq!(out.created.iter().map(|o| o.value).sum::<u64>(), 99);
        assert!(ledger.owned_objects.iter().all(|o| o.id != "aa"));
        assert_eq!(ledger.owned_objects.len(), 2);
    }

    #[test]
    fn rejects_replay_of_consumed_input() {
        let mut ledger = LedgerState::empty();
        ledger.owned_objects.push(object("aa", "ownerA", 100, "PFT"));
        let order = postfiat_types::OwnedTransferOrder {
            domain: domain(),
            inputs: vec![postfiat_types::OwnedObjectRef { id: "aa".into(), version: 1 }],
            outputs: vec![postfiat_types::OwnedOutputSpec {
                owner_pubkey_hex: "ownerA".into(),
                value: 100,
                asset: "PFT".into(),
            }],
            fee: 0,
            nonce: 1,
            memos: Vec::new(),
        };
        apply_owned_transfer(&mut ledger, &order, "ownerA").expect("first");
        assert_eq!(
            apply_owned_transfer(&mut ledger, &order, "ownerA").unwrap_err(),
            OwnedTransferError::UnknownInput
        );
    }

    #[test]
    fn rejects_non_conserving_wrong_owner_and_version() {
        let mut ledger = LedgerState::empty();
        ledger.owned_objects.push(object("aa", "ownerA", 100, "PFT"));
        // non-conserving
        let bad = postfiat_types::OwnedTransferOrder {
            domain: domain(),
            inputs: vec![postfiat_types::OwnedObjectRef { id: "aa".into(), version: 1 }],
            outputs: vec![postfiat_types::OwnedOutputSpec { owner_pubkey_hex: "ownerA".into(), value: 200, asset: "PFT".into() }],
            fee: 0, nonce: 1,
            memos: Vec::new(),
        };
        assert_eq!(apply_owned_transfer(&mut ledger, &bad, "ownerA").unwrap_err(), OwnedTransferError::NotConserved);
        // wrong owner
        let wrong = postfiat_types::OwnedTransferOrder {
            domain: domain(),
            inputs: vec![postfiat_types::OwnedObjectRef { id: "aa".into(), version: 1 }],
            outputs: vec![postfiat_types::OwnedOutputSpec { owner_pubkey_hex: "x".into(), value: 100, asset: "PFT".into() }],
            fee: 0, nonce: 2,
            memos: Vec::new(),
        };
        assert_eq!(apply_owned_transfer(&mut ledger, &wrong, "notTheOwner").unwrap_err(), OwnedTransferError::NotOwner);
        // version mismatch
        let vm = postfiat_types::OwnedTransferOrder {
            domain: domain(),
            inputs: vec![postfiat_types::OwnedObjectRef { id: "aa".into(), version: 99 }],
            outputs: vec![postfiat_types::OwnedOutputSpec { owner_pubkey_hex: "ownerA".into(), value: 100, asset: "PFT".into() }],
            fee: 0, nonce: 3,
            memos: Vec::new(),
        };
        assert_eq!(apply_owned_transfer(&mut ledger, &vm, "ownerA").unwrap_err(), OwnedTransferError::VersionMismatch);
    }

    #[test]
    fn wrap_and_unwrap_bridge_is_value_conserving() {
        let mut ledger = LedgerState::empty();
        ledger.accounts.push(postfiat_types::Account {
            address: "alice".into(),
            balance: 100,
            sequence: 0,
            public_key_hex: None,
        });
        // wrap: 100 balance -> 100 owned object
        let obj = wrap_to_owned(&mut ledger, "alice", "alicepk".into(), 100, "PFT".into(), "obj1".into()).expect("wrap");
        assert_eq!(obj.value, 100);
        assert_eq!(ledger.account("alice").map(|a| a.balance).unwrap_or(0), 0);
        assert_eq!(ledger.owned_objects.len(), 1);
        // unwrap: 100 owned -> 100 balance credited to bob
        let credited = unwrap_from_owned(&mut ledger, "obj1", "alicepk", "bob").expect("unwrap");
        assert_eq!(credited, 100);
        assert!(ledger.owned_objects.is_empty());
        assert_eq!(ledger.account("bob").map(|a| a.balance).unwrap_or(0), 100);
    }

    #[test]
    fn wrap_rejects_non_native_asset_without_mutation() {
        let mut ledger = LedgerState::empty();
        ledger.accounts.push(postfiat_types::Account {
            address: "alice".into(),
            balance: 100,
            sequence: 0,
            public_key_hex: None,
        });
        for asset in ["pfUSDC", "a651", "pft", ""] {
            assert_eq!(
                wrap_to_owned(&mut ledger, "alice", "alicepk".into(), 50, asset.into(), "obj".into())
                    .unwrap_err(),
                OwnedTransferError::UnsupportedAsset,
                "asset {asset:?} must be rejected"
            );
        }
        assert_eq!(ledger.account("alice").map(|a| a.balance), Some(100));
        assert!(ledger.owned_objects.is_empty());
    }

    #[test]
    fn wrap_rejects_duplicate_object_id_without_mutation() {
        let mut ledger = LedgerState::empty();
        ledger.accounts.push(postfiat_types::Account {
            address: "alice".into(),
            balance: 100,
            sequence: 0,
            public_key_hex: None,
        });
        ledger
            .owned_objects
            .push(object("already-live", "existing-owner", 25, "PFT"));
        let before = ledger.clone();

        assert_eq!(
            wrap_to_owned(
                &mut ledger,
                "alice",
                "new-owner".into(),
                50,
                "PFT".into(),
                "already-live".into(),
            )
            .expect_err("a live object id must never be overwritten or duplicated"),
            OwnedTransferError::DuplicateOutput
        );
        assert_eq!(ledger, before, "collision rejection must be atomic");
    }

    #[test]
    fn unwrap_rejects_non_native_asset_without_mutation() {
        let mut ledger = LedgerState::empty();
        ledger
            .owned_objects
            .push(object("issued", "owner", 70, "pfUSDC"));
        let before = ledger.clone();

        assert_eq!(
            unwrap_from_owned(&mut ledger, "issued", "owner", "recipient")
                .expect_err("issued custody must never credit the native PFT account field"),
            OwnedTransferError::UnsupportedAsset
        );
        assert_eq!(ledger, before, "wrong-lane unwrap must not consume its input");
    }

    #[test]
    fn unwrap_overflow_rejects_without_consuming_input() {
        let mut ledger = LedgerState::empty();
        ledger.accounts.push(postfiat_types::Account {
            address: "recipient".into(),
            balance: u64::MAX,
            sequence: 0,
            public_key_hex: None,
        });
        ledger
            .owned_objects
            .push(object("native", "owner", 1, OWNED_NATIVE_ASSET));
        let before = ledger.clone();

        assert_eq!(
            unwrap_from_owned(&mut ledger, "native", "owner", "recipient")
                .expect_err("credit overflow must fail before consuming the object"),
            OwnedTransferError::Overflow
        );
        assert_eq!(ledger, before, "overflow rejection must be atomic");
    }

    #[test]
    fn certified_unwrap_rejects_non_native_asset_and_overflow_without_mutation() {
        let mut ledger = LedgerState::empty();
        ledger.accounts.push(postfiat_types::Account {
            address: "recipient".into(),
            balance: u64::MAX,
            sequence: 0,
            public_key_hex: None,
        });
        ledger
            .owned_objects
            .push(object("issued", "owner", 1, "pfUSDC"));
        let wrong_asset = postfiat_types::OwnedUnwrapOrder {
            domain: domain(),
            inputs: vec![postfiat_types::OwnedObjectRef {
                id: "issued".into(),
                version: 1,
            }],
            to_address: "recipient".into(),
            amount: 1,
            asset: "pfUSDC".into(),
            fee: 0,
            nonce: 1,
            memos: Vec::new(),
        };
        let before_wrong_asset = ledger.clone();
        assert_eq!(
            apply_owned_unwrap(&mut ledger, &wrong_asset, "owner")
                .expect_err("issued custody must not unwrap into native PFT"),
            OwnedTransferError::UnsupportedAsset
        );
        assert_eq!(ledger, before_wrong_asset);

        ledger.owned_objects[0].asset = OWNED_NATIVE_ASSET.into();
        let mut native = wrong_asset;
        native.asset = OWNED_NATIVE_ASSET.into();
        let before_overflow = ledger.clone();
        assert_eq!(
            apply_owned_unwrap(&mut ledger, &native, "owner")
                .expect_err("credit overflow must fail before certified input consumption"),
            OwnedTransferError::Overflow
        );
        assert_eq!(ledger, before_overflow, "certified unwrap must be atomic");
    }

    #[test]
    fn wrap_rejects_insufficient_balance_and_unwrap_rejects_non_owner() {
        let mut ledger = LedgerState::empty();
        ledger.accounts.push(postfiat_types::Account {
            address: "alice".into(),
            balance: 10,
            sequence: 0,
            public_key_hex: None,
        });
        // insufficient balance
        assert_eq!(
            wrap_to_owned(&mut ledger, "alice", "k".into(), 100, "PFT".into(), "o".into()).unwrap_err(),
            OwnedTransferError::NotConserved
        );
        // unknown account
        assert_eq!(
            wrap_to_owned(&mut ledger, "ghost", "k".into(), 1, "PFT".into(), "o".into()).unwrap_err(),
            OwnedTransferError::UnknownInput
        );
        // unwrap as non-owner rejected
        ledger.accounts[0].balance = 100;
        wrap_to_owned(&mut ledger, "alice", "alicepk".into(), 50, "PFT".into(), "obj".into()).expect("wrap");
        assert_eq!(
            unwrap_from_owned(&mut ledger, "obj", "wrongpk", "bob").unwrap_err(),
            OwnedTransferError::NotOwner
        );
    }

    #[test]
    fn certified_unwrap_credits_amount_and_mints_change() {
        let mut ledger = LedgerState::empty();
        let owner_kp = postfiat_crypto_provider::ml_dsa_65_keygen().expect("owner keygen");
        let owner_pk_hex = postfiat_crypto_provider::bytes_to_hex(&owner_kp.public_key);
        ledger.owned_objects.push(postfiat_types::OwnedObject {
            id: "obj".into(),
            version: 1,
            owner_pubkey_hex: owner_pk_hex.clone(),
            value: 200,
            asset: "PFT".into(),
        });
        let order = postfiat_types::OwnedUnwrapOrder {
            domain: domain(),
            inputs: vec![postfiat_types::OwnedObjectRef { id: "obj".into(), version: 1 }],
            to_address: "alice".into(),
            amount: 75,
            asset: "PFT".into(),
            fee: 5,
            nonce: 42,
            memos: Vec::new(),
        };
        let sb = owned_unwrap_signing_bytes(&order);
        let owner_sig = postfiat_crypto_provider::ml_dsa_65_sign_with_context(
            &owner_kp.private_key,
            &sb,
            OWNED_UNWRAP_CONTEXT,
        )
        .unwrap();
        let vs: Vec<(String, postfiat_crypto_provider::MlDsa65KeyPair)> =
            (0..3).map(|i| (format!("v{i}"), postfiat_crypto_provider::ml_dsa_65_keygen().unwrap())).collect();
        let pks: Vec<(String, String)> = vs.iter().map(|(id, kp)| (id.clone(), postfiat_crypto_provider::bytes_to_hex(&kp.public_key))).collect();
        let votes = vs
            .iter()
            .map(|(id, kp)| {
                let sig = postfiat_crypto_provider::ml_dsa_65_sign_with_context(
                    &kp.private_key,
                    &sb,
                    OWNED_UNWRAP_CONTEXT,
                )
                .unwrap();
                postfiat_types::OwnedUnwrapVote {
                    validator_id: id.clone(),
                    signature_hex: postfiat_crypto_provider::bytes_to_hex(&sig),
                }
            })
            .collect();
        let cert = postfiat_types::OwnedUnwrapCertificate {
            order,
            owner_pubkey_hex: owner_pk_hex.clone(),
            owner_signature_hex: postfiat_crypto_provider::bytes_to_hex(&owner_sig),
            votes,
        };

        let out = apply_owned_unwrap_certificate(&mut ledger, &cert, &pks, &domain(), 3)
            .expect("unwrap apply");
        assert_eq!(out.consumed, 1);
        assert_eq!(out.credited, 75);
        assert_eq!(ledger.account("alice").map(|a| a.balance), Some(75));
        assert!(ledger.owned_objects.iter().all(|o| o.id != "obj"));
        let change = out.change_object.expect("change");
        assert_eq!(change.owner_pubkey_hex, owner_pk_hex);
        assert_eq!(change.value, 120);
        assert_eq!(ledger.owned_objects, vec![change]);
    }

    #[test]
    fn certified_unwrap_combines_fragmented_inputs() {
        let mut ledger = LedgerState::empty();
        let owner_kp = postfiat_crypto_provider::ml_dsa_65_keygen().expect("owner keygen");
        let owner_pk_hex = postfiat_crypto_provider::bytes_to_hex(&owner_kp.public_key);
        for index in 0..20 {
            ledger.owned_objects.push(postfiat_types::OwnedObject {
                id: format!("obj-{index}"),
                version: 1,
                owner_pubkey_hex: owner_pk_hex.clone(),
                value: 100,
                asset: "PFT".into(),
            });
        }
        let order = postfiat_types::OwnedUnwrapOrder {
            domain: domain(),
            inputs: (0..20)
                .map(|index| postfiat_types::OwnedObjectRef { id: format!("obj-{index}"), version: 1 })
                .collect(),
            to_address: "alice".into(),
            amount: 1950,
            asset: "PFT".into(),
            fee: 0,
            nonce: 43,
            memos: Vec::new(),
        };
        let sb = owned_unwrap_signing_bytes(&order);
        let owner_sig = postfiat_crypto_provider::ml_dsa_65_sign_with_context(
            &owner_kp.private_key,
            &sb,
            OWNED_UNWRAP_CONTEXT,
        )
        .unwrap();
        let vs: Vec<(String, postfiat_crypto_provider::MlDsa65KeyPair)> =
            (0..3).map(|i| (format!("v{i}"), postfiat_crypto_provider::ml_dsa_65_keygen().unwrap())).collect();
        let pks: Vec<(String, String)> = vs.iter().map(|(id, kp)| (id.clone(), postfiat_crypto_provider::bytes_to_hex(&kp.public_key))).collect();
        let votes = vs
            .iter()
            .map(|(id, kp)| {
                let sig = postfiat_crypto_provider::ml_dsa_65_sign_with_context(
                    &kp.private_key,
                    &sb,
                    OWNED_UNWRAP_CONTEXT,
                )
                .unwrap();
                postfiat_types::OwnedUnwrapVote {
                    validator_id: id.clone(),
                    signature_hex: postfiat_crypto_provider::bytes_to_hex(&sig),
                }
            })
            .collect();
        let cert = postfiat_types::OwnedUnwrapCertificate {
            order,
            owner_pubkey_hex: owner_pk_hex.clone(),
            owner_signature_hex: postfiat_crypto_provider::bytes_to_hex(&owner_sig),
            votes,
        };

        let out = apply_owned_unwrap_certificate(&mut ledger, &cert, &pks, &domain(), 3)
            .expect("unwrap apply");
        assert_eq!(out.consumed, 20);
        assert_eq!(out.credited, 1950);
        assert_eq!(ledger.account("alice").map(|a| a.balance), Some(1950));
        let change = out.change_object.expect("change");
        assert_eq!(change.owner_pubkey_hex, owner_pk_hex);
        assert_eq!(change.value, 50);
        assert_eq!(ledger.owned_objects, vec![change]);
    }

    #[test]
    fn certified_unwrap_rejects_bad_owner_auth_and_low_quorum() {
        let mut ledger = LedgerState::empty();
        let owner_kp = postfiat_crypto_provider::ml_dsa_65_keygen().expect("owner keygen");
        let owner_pk_hex = postfiat_crypto_provider::bytes_to_hex(&owner_kp.public_key);
        ledger.owned_objects.push(postfiat_types::OwnedObject {
            id: "obj".into(),
            version: 1,
            owner_pubkey_hex: owner_pk_hex.clone(),
            value: 100,
            asset: "PFT".into(),
        });
        let order = postfiat_types::OwnedUnwrapOrder {
            domain: domain(),
            inputs: vec![postfiat_types::OwnedObjectRef { id: "obj".into(), version: 1 }],
            to_address: "alice".into(),
            amount: 100,
            asset: "PFT".into(),
            fee: 0,
            nonce: 7,
            memos: Vec::new(),
        };
        let sb = owned_unwrap_signing_bytes(&order);
        let owner_sig = postfiat_crypto_provider::ml_dsa_65_sign_with_context(
            &owner_kp.private_key,
            &sb,
            OWNED_UNWRAP_CONTEXT,
        )
        .unwrap();
        let vs: Vec<(String, postfiat_crypto_provider::MlDsa65KeyPair)> =
            (0..2).map(|i| (format!("v{i}"), postfiat_crypto_provider::ml_dsa_65_keygen().unwrap())).collect();
        let pks: Vec<(String, String)> = vs.iter().map(|(id, kp)| (id.clone(), postfiat_crypto_provider::bytes_to_hex(&kp.public_key))).collect();
        let votes = vs
            .iter()
            .map(|(id, kp)| {
                let sig = postfiat_crypto_provider::ml_dsa_65_sign_with_context(
                    &kp.private_key,
                    &sb,
                    OWNED_UNWRAP_CONTEXT,
                )
                .unwrap();
                postfiat_types::OwnedUnwrapVote {
                    validator_id: id.clone(),
                    signature_hex: postfiat_crypto_provider::bytes_to_hex(&sig),
                }
            })
            .collect();
        let cert = postfiat_types::OwnedUnwrapCertificate {
            order,
            owner_pubkey_hex: owner_pk_hex,
            owner_signature_hex: postfiat_crypto_provider::bytes_to_hex(&owner_sig),
            votes,
        };

        assert_eq!(
            apply_owned_unwrap_certificate(&mut ledger, &cert, &pks, &domain(), 3).unwrap_err(),
            OwnedTransferError::InsufficientQuorum { have: 2, need: 3 }
        );
        let mut bad = cert.clone();
        bad.owner_signature_hex = "00".into();
        assert_eq!(
            apply_owned_unwrap_certificate(&mut ledger, &bad, &pks, &domain(), 2).unwrap_err(),
            OwnedTransferError::OwnerAuthFailed
        );
    }

    #[test]
    fn certified_apply_verifies_owner_auth_and_quorum() {
        let mut ledger = LedgerState::empty();
        let owner_kp = postfiat_crypto_provider::ml_dsa_65_keygen().expect("owner keygen");
        let owner_pk_hex = postfiat_crypto_provider::bytes_to_hex(&owner_kp.public_key);
        let mint = |id: &str| postfiat_types::OwnedObject {
            id: id.into(), version: 1, owner_pubkey_hex: owner_pk_hex.clone(), value: 100, asset: "PFT".into(),
        };
        let order_over = |input_id: &str, nonce: u64| postfiat_types::OwnedTransferOrder {
            domain: domain(),
            inputs: vec![postfiat_types::OwnedObjectRef { id: input_id.into(), version: 1 }],
            outputs: vec![postfiat_types::OwnedOutputSpec { owner_pubkey_hex: owner_pk_hex.clone(), value: 100, asset: "PFT".into() }],
            fee: 0, nonce, memos: Vec::new(),
        };
        let sign_cert = |order: postfiat_types::OwnedTransferOrder, vs: &[(String, postfiat_crypto_provider::MlDsa65KeyPair)]| {
            let sb = owned_transfer_signing_bytes(&order);
            let owner_sig = postfiat_crypto_provider::ml_dsa_65_sign_with_context(&owner_kp.private_key, &sb, OWNED_TRANSFER_CONTEXT).unwrap();
            let votes: Vec<postfiat_types::OwnedTransferVote> = vs.iter().map(|(id, kp)| {
                let sig = postfiat_crypto_provider::ml_dsa_65_sign_with_context(&kp.private_key, &sb, OWNED_TRANSFER_CONTEXT).unwrap();
                postfiat_types::OwnedTransferVote { validator_id: id.clone(), signature_hex: postfiat_crypto_provider::bytes_to_hex(&sig) }
            }).collect();
            postfiat_types::OwnedTransferCertificate {
                order, owner_pubkey_hex: owner_pk_hex.clone(),
                owner_signature_hex: postfiat_crypto_provider::bytes_to_hex(&owner_sig), votes,
            }
        };
        let vs: Vec<(String, postfiat_crypto_provider::MlDsa65KeyPair)> =
            (0..3).map(|i| (format!("v{i}"), postfiat_crypto_provider::ml_dsa_65_keygen().unwrap())).collect();
        let pks: Vec<(String, String)> = vs.iter().map(|(id, kp)| (id.clone(), postfiat_crypto_provider::bytes_to_hex(&kp.public_key))).collect();

        // quorum 3 with 3 valid votes -> applies
        ledger.owned_objects.push(mint("obj"));
        let cert = sign_cert(order_over("obj", 1), &vs);
        let out = apply_owned_certificate(&mut ledger, &cert, &pks, &domain(), 3)
            .expect("cert apply");
        assert_eq!(out.consumed, 1);

        // insufficient quorum (need 4, have 3) -> rejected
        ledger.owned_objects.push(mint("obj2"));
        let cert2 = sign_cert(order_over("obj2", 2), &vs);
        assert_eq!(
            apply_owned_certificate(&mut ledger, &cert2, &pks, &domain(), 4).unwrap_err(),
            OwnedTransferError::InsufficientQuorum { have: 3, need: 4 }
        );
        // tampered owner signature -> OwnerAuthFailed
        let mut bad = cert2.clone();
        bad.owner_signature_hex = "00".into();
        assert_eq!(
            apply_owned_certificate(&mut ledger, &bad, &pks, &domain(), 3).unwrap_err(),
            OwnedTransferError::OwnerAuthFailed
        );
    }

    #[test]
    fn foreign_domain_certificate_is_rejected_without_state_mutation() {
        let owner = postfiat_crypto_provider::ml_dsa_65_keygen().expect("owner keygen");
        let validator = postfiat_crypto_provider::ml_dsa_65_keygen().expect("validator keygen");
        let owner_pubkey_hex = postfiat_crypto_provider::bytes_to_hex(&owner.public_key);
        let order = postfiat_types::OwnedTransferOrder {
            domain: domain(),
            inputs: vec![postfiat_types::OwnedObjectRef {
                id: "domain-object".to_string(),
                version: 1,
            }],
            outputs: vec![postfiat_types::OwnedOutputSpec {
                owner_pubkey_hex: owner_pubkey_hex.clone(),
                value: 100,
                asset: "PFT".to_string(),
            }],
            fee: 0,
            nonce: 7,
            memos: Vec::new(),
        };
        let signing_bytes = owned_transfer_signing_bytes(&order);
        let owner_signature = postfiat_crypto_provider::ml_dsa_65_sign_with_context(
            &owner.private_key,
            &signing_bytes,
            OWNED_TRANSFER_CONTEXT,
        )
        .expect("owner sign");
        let vote_signature = postfiat_crypto_provider::ml_dsa_65_sign_with_context(
            &validator.private_key,
            &signing_bytes,
            OWNED_TRANSFER_CONTEXT,
        )
        .expect("validator sign");
        let cert = postfiat_types::OwnedTransferCertificate {
            order,
            owner_pubkey_hex: owner_pubkey_hex.clone(),
            owner_signature_hex: postfiat_crypto_provider::bytes_to_hex(&owner_signature),
            votes: vec![postfiat_types::OwnedTransferVote {
                validator_id: "validator-0".to_string(),
                signature_hex: postfiat_crypto_provider::bytes_to_hex(&vote_signature),
            }],
        };
        let validator_pks = vec![(
            "validator-0".to_string(),
            postfiat_crypto_provider::bytes_to_hex(&validator.public_key),
        )];
        let mut expected_domain = domain();
        expected_domain.chain_id = "postfiat-other-chain".to_string();
        let mut ledger = LedgerState::empty();
        ledger.owned_objects.push(object(
            "domain-object",
            &owner_pubkey_hex,
            100,
            "PFT",
        ));
        let before = ledger.clone();

        assert_eq!(
            apply_owned_certificate(&mut ledger, &cert, &validator_pks, &expected_domain, 1)
                .expect_err("foreign-domain certificate must fail closed"),
            OwnedTransferError::InvalidDomain
        );
        assert_eq!(ledger, before);
    }

    #[test]
    fn duplicate_validator_vote_is_rejected_in_transfer_and_unwrap_certificates() {
        let owner = postfiat_crypto_provider::ml_dsa_65_keygen().expect("owner");
        let owner_pubkey_hex = postfiat_crypto_provider::bytes_to_hex(&owner.public_key);
        let validator = postfiat_crypto_provider::ml_dsa_65_keygen().expect("validator");
        let validator_pks = vec![(
            "validator-0".to_string(),
            postfiat_crypto_provider::bytes_to_hex(&validator.public_key),
        )];

        let transfer_order = postfiat_types::OwnedTransferOrder {
            domain: domain(),
            inputs: vec![postfiat_types::OwnedObjectRef {
                id: "object-0".to_string(),
                version: 1,
            }],
            outputs: vec![postfiat_types::OwnedOutputSpec {
                owner_pubkey_hex: owner_pubkey_hex.clone(),
                value: 100,
                asset: "PFT".to_string(),
            }],
            fee: 0,
            nonce: 1,
            memos: Vec::new(),
        };
        let transfer_bytes = owned_transfer_signing_bytes(&transfer_order);
        let transfer_owner_signature = postfiat_crypto_provider::ml_dsa_65_sign_with_context(
            &owner.private_key,
            &transfer_bytes,
            OWNED_TRANSFER_CONTEXT,
        )
        .expect("owner transfer sign");
        let transfer_vote_signature = postfiat_crypto_provider::ml_dsa_65_sign_with_context(
            &validator.private_key,
            &transfer_bytes,
            OWNED_TRANSFER_CONTEXT,
        )
        .expect("validator transfer sign");
        let transfer_vote = postfiat_types::OwnedTransferVote {
            validator_id: "validator-0".to_string(),
            signature_hex: postfiat_crypto_provider::bytes_to_hex(&transfer_vote_signature),
        };
        let transfer_cert = postfiat_types::OwnedTransferCertificate {
            order: transfer_order,
            owner_pubkey_hex: owner_pubkey_hex.clone(),
            owner_signature_hex: postfiat_crypto_provider::bytes_to_hex(
                &transfer_owner_signature,
            ),
            votes: vec![transfer_vote.clone(), transfer_vote],
        };
        assert_eq!(
            verify_owned_certificate(&transfer_cert, &validator_pks, &domain()),
            None
        );

        let unwrap_order = postfiat_types::OwnedUnwrapOrder {
            domain: domain(),
            inputs: vec![postfiat_types::OwnedObjectRef {
                id: "object-0".to_string(),
                version: 1,
            }],
            to_address: "pf-recipient".to_string(),
            amount: 100,
            asset: "PFT".to_string(),
            fee: 0,
            nonce: 2,
            memos: Vec::new(),
        };
        let unwrap_bytes = owned_unwrap_signing_bytes(&unwrap_order);
        let unwrap_owner_signature = postfiat_crypto_provider::ml_dsa_65_sign_with_context(
            &owner.private_key,
            &unwrap_bytes,
            OWNED_UNWRAP_CONTEXT,
        )
        .expect("owner unwrap sign");
        let unwrap_vote_signature = postfiat_crypto_provider::ml_dsa_65_sign_with_context(
            &validator.private_key,
            &unwrap_bytes,
            OWNED_UNWRAP_CONTEXT,
        )
        .expect("validator unwrap sign");
        let unwrap_vote = postfiat_types::OwnedUnwrapVote {
            validator_id: "validator-0".to_string(),
            signature_hex: postfiat_crypto_provider::bytes_to_hex(&unwrap_vote_signature),
        };
        let unwrap_cert = postfiat_types::OwnedUnwrapCertificate {
            order: unwrap_order,
            owner_pubkey_hex,
            owner_signature_hex: postfiat_crypto_provider::bytes_to_hex(&unwrap_owner_signature),
            votes: vec![unwrap_vote.clone(), unwrap_vote],
        };
        assert_eq!(
            verify_owned_unwrap_certificate(&unwrap_cert, &validator_pks, &domain()),
            None
        );
    }

    #[test]
    fn duplicate_owned_input_is_rejected_without_mutation() {
        let mut ledger = LedgerState::empty();
        ledger.owned_objects.push(object("object-0", "owner", 100, "PFT"));
        let order = postfiat_types::OwnedTransferOrder {
            domain: domain(),
            inputs: vec![
                postfiat_types::OwnedObjectRef {
                    id: "object-0".to_string(),
                    version: 1,
                },
                postfiat_types::OwnedObjectRef {
                    id: "object-0".to_string(),
                    version: 1,
                },
            ],
            outputs: vec![postfiat_types::OwnedOutputSpec {
                owner_pubkey_hex: "owner".to_string(),
                value: 200,
                asset: "PFT".to_string(),
            }],
            fee: 0,
            nonce: 3,
            memos: Vec::new(),
        };
        assert_eq!(
            apply_owned_transfer(&mut ledger, &order, "owner").unwrap_err(),
            OwnedTransferError::DuplicateInput
        );
        assert_eq!(ledger.owned_objects, vec![object("object-0", "owner", 100, "PFT")]);
    }

    #[test]
    fn zero_value_and_existing_output_id_are_rejected_without_mutation() {
        let zero_output = postfiat_types::OwnedOutputSpec {
            owner_pubkey_hex: "recipient".to_string(),
            value: 0,
            asset: OWNED_NATIVE_ASSET.to_string(),
        };
        let mut ledger = LedgerState::empty();
        ledger
            .owned_objects
            .push(object("input-zero", "owner", 10, OWNED_NATIVE_ASSET));
        let zero_order = postfiat_types::OwnedTransferOrder {
            domain: domain(),
            inputs: vec![postfiat_types::OwnedObjectRef {
                id: "input-zero".to_string(),
                version: 1,
            }],
            outputs: vec![
                zero_output,
                postfiat_types::OwnedOutputSpec {
                    owner_pubkey_hex: "recipient".to_string(),
                    value: 10,
                    asset: OWNED_NATIVE_ASSET.to_string(),
                },
            ],
            fee: 0,
            nonce: 11,
            memos: Vec::new(),
        };
        let before_zero = ledger.clone();
        assert_eq!(
            apply_owned_transfer(&mut ledger, &zero_order, "owner")
                .expect_err("zero-value owned outputs must fail closed"),
            OwnedTransferError::NotConserved
        );
        assert_eq!(ledger, before_zero);

        let output = postfiat_types::OwnedOutputSpec {
            owner_pubkey_hex: "recipient".to_string(),
            value: 10,
            asset: OWNED_NATIVE_ASSET.to_string(),
        };
        let nonce = 12;
        let collision_id = owned_output_id("owner", nonce, 0, &output);
        ledger
            .owned_objects
            .push(object(&collision_id, "someone-else", 99, OWNED_NATIVE_ASSET));
        let collision_order = postfiat_types::OwnedTransferOrder {
            domain: domain(),
            inputs: vec![postfiat_types::OwnedObjectRef {
                id: "input-zero".to_string(),
                version: 1,
            }],
            outputs: vec![output],
            fee: 0,
            nonce,
            memos: Vec::new(),
        };
        let before_collision = ledger.clone();
        assert_eq!(
            apply_owned_transfer(&mut ledger, &collision_order, "owner")
                .expect_err("content-addressed output must not collide with live state"),
            OwnedTransferError::DuplicateOutput
        );
        assert_eq!(ledger, before_collision);
    }

    #[test]
    fn fastpay_safety_chaos_gate() {
        // Adversarial scenarios against the certified-apply path.
        let mut ledger = LedgerState::empty();
        let owner_kp = postfiat_crypto_provider::ml_dsa_65_keygen().expect("owner");
        let owner_pk_hex = postfiat_crypto_provider::bytes_to_hex(&owner_kp.public_key);
        let vs: Vec<(String, postfiat_crypto_provider::MlDsa65KeyPair)> = (0..4)
            .map(|i| (format!("v{i}"), postfiat_crypto_provider::ml_dsa_65_keygen().expect("v")))
            .collect();
        let pks: Vec<(String, String)> = vs
            .iter()
            .map(|(id, kp)| (id.clone(), postfiat_crypto_provider::bytes_to_hex(&kp.public_key)))
            .collect();
        let forger = postfiat_crypto_provider::ml_dsa_65_keygen().expect("forger");

        let mk_order = |obj_id: &str, nonce: u64| postfiat_types::OwnedTransferOrder {
            domain: domain(),
            inputs: vec![postfiat_types::OwnedObjectRef { id: obj_id.into(), version: 1 }],
            outputs: vec![postfiat_types::OwnedOutputSpec {
                owner_pubkey_hex: owner_pk_hex.clone(),
                value: 100,
                asset: "PFT".into(),
            }],
            fee: 0,
            nonce,
            memos: Vec::new(),
        };
        let mk_vote = |order: &postfiat_types::OwnedTransferOrder, id: String, kp: &postfiat_crypto_provider::MlDsa65KeyPair| {
            let sb = owned_transfer_signing_bytes(order);
            let sig = postfiat_crypto_provider::ml_dsa_65_sign_with_context(&kp.private_key, &sb, OWNED_TRANSFER_CONTEXT).expect("sign");
            postfiat_types::OwnedTransferVote { validator_id: id, signature_hex: postfiat_crypto_provider::bytes_to_hex(&sig) }
        };
        let owner_sig_hex = |order: &postfiat_types::OwnedTransferOrder| {
            let sb = owned_transfer_signing_bytes(order);
            postfiat_crypto_provider::bytes_to_hex(
                &postfiat_crypto_provider::ml_dsa_65_sign_with_context(&owner_kp.private_key, &sb, OWNED_TRANSFER_CONTEXT).expect("owner sign"),
            )
        };
        let mint = |id: &str| postfiat_types::OwnedObject {
            id: id.into(),
            version: 1,
            owner_pubkey_hex: owner_pk_hex.clone(),
            value: 100,
            asset: "PFT".into(),
        };

        // 4 valid votes (quorum 3) -> applies.
        ledger.owned_objects.push(mint("a"));
        let order_a = mk_order("a", 1);
        let votes_a: Vec<_> = vs.iter().map(|(id, kp)| mk_vote(&order_a, id.clone(), kp)).collect();
        let cert_a = postfiat_types::OwnedTransferCertificate {
            owner_signature_hex: owner_sig_hex(&order_a),
            order: order_a,
            owner_pubkey_hex: owner_pk_hex.clone(),
            votes: votes_a,
        };
        assert!(
            apply_owned_certificate(&mut ledger, &cert_a, &pks, &domain(), 3).is_ok(),
            "4 valid -> applies"
        );

        // 2 valid + 2 FORGED votes (forger's key labeled v0/v1) -> only 2 valid < 3 -> InsufficientQuorum.
        ledger.owned_objects.push(mint("b"));
        let order_b = mk_order("b", 2);
        let votes_b = vec![
            mk_vote(&order_b, "v2".into(), &vs[2].1),
            mk_vote(&order_b, "v3".into(), &vs[3].1),
            mk_vote(&order_b, "v0".into(), &forger),
            mk_vote(&order_b, "v1".into(), &forger),
        ];
        let cert_b = postfiat_types::OwnedTransferCertificate {
            owner_signature_hex: owner_sig_hex(&order_b),
            order: order_b,
            owner_pubkey_hex: owner_pk_hex.clone(),
            votes: votes_b,
        };
        assert_eq!(
            apply_owned_certificate(&mut ledger, &cert_b, &pks, &domain(), 3).unwrap_err(),
            OwnedTransferError::InsufficientQuorum { have: 2, need: 3 },
            "forged votes don't count"
        );

        // exactly quorum (3 valid) -> applies (boundary).
        ledger.owned_objects.push(mint("c"));
        let order_c = mk_order("c", 3);
        let votes_c = vec![
            mk_vote(&order_c, "v0".into(), &vs[0].1),
            mk_vote(&order_c, "v1".into(), &vs[1].1),
            mk_vote(&order_c, "v2".into(), &vs[2].1),
        ];
        let cert_c = postfiat_types::OwnedTransferCertificate {
            owner_signature_hex: owner_sig_hex(&order_c),
            order: order_c,
            owner_pubkey_hex: owner_pk_hex.clone(),
            votes: votes_c,
        };
        assert!(
            apply_owned_certificate(&mut ledger, &cert_c, &pks, &domain(), 3).is_ok(),
            "exactly quorum -> applies"
        );

        // replay cert_a -> rejected (single-consumption; object a already consumed).
        assert!(
            apply_owned_certificate(&mut ledger, &cert_a, &pks, &domain(), 3).is_err(),
            "replay rejected"
        );
    }

    #[test]
    fn rejects_oversized_transfer_resource_limit() {
        let mut ledger = LedgerState::empty();
        let opk = "ownerA".to_string();
        // One more than MAX_OWNED_INPUTS_PER_TRANSFER -> ResourceLimitExceeded
        let too_many_input_count = postfiat_types::MAX_OWNED_INPUTS_PER_TRANSFER + 1;
        let too_many_inputs = postfiat_types::OwnedTransferOrder {
            domain: domain(),
            inputs: (0..too_many_input_count)
                .map(|i| postfiat_types::OwnedObjectRef { id: format!("in{i}"), version: 1 })
                .collect(),
            outputs: vec![postfiat_types::OwnedOutputSpec { owner_pubkey_hex: opk.clone(), value: 99, asset: "PFT".into() }],
            fee: 1, nonce: 1, memos: Vec::new(),
        };
        assert_eq!(
            apply_owned_transfer(&mut ledger, &too_many_inputs, &opk).unwrap_err(),
            OwnedTransferError::ResourceLimitExceeded
        );
        // 9 outputs (> MAX_OWNED_OUTPUTS_PER_TRANSFER=8) -> rejected
        let too_many_outputs = postfiat_types::OwnedTransferOrder {
            domain: domain(),
            inputs: vec![postfiat_types::OwnedObjectRef { id: "x".into(), version: 1 }],
            outputs: (0..9).map(|_| postfiat_types::OwnedOutputSpec { owner_pubkey_hex: opk.clone(), value: 1, asset: "PFT".into() }).collect(),
            fee: 1, nonce: 2, memos: Vec::new(),
        };
        assert_eq!(
            apply_owned_transfer(&mut ledger, &too_many_outputs, &opk).unwrap_err(),
            OwnedTransferError::ResourceLimitExceeded
        );
    }
}
