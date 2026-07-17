use postfiat_crypto_provider::{
    address_from_public_key, bytes_to_hex, hash_bytes, hex_to_bytes, ml_dsa_65_verify_with_context,
};
use postfiat_types::{
    AssetDefinition, FastAssetDefinitionHashV1, FastAssetIdV1, FastAssetObjectV1, FastAssetRuleV1,
    FastLaneDepositReceiptV1, FastLaneExitCertificateV1, FastLaneExitClaimV1,
    FastLaneExitEffectsV1, FastLaneRedeemReceiptV1, FastLaneReserveBalanceV1, FastLaneStateV1,
    FastObjectIdV1, FastObjectKeyV1, FastObjectOriginV1, FastSwapCommitteeV1,
    FastSwapExitClaimIdV1, LedgerState, SignedFastLaneDepositV1, SignedFastLaneExitIntentV1,
    SignedFastLaneRedeemV1, FASTLANE_DEPOSIT_CONTEXT_V1, FASTLANE_EXIT_CONTEXT_V1,
    FASTLANE_EXIT_VOTE_CONTEXT_V1, FASTSWAP_ML_DSA_65,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum FastLaneBridgeError {
    DomainMismatch,
    InvalidSignature,
    SourceKeyMismatch,
    InvalidSequence,
    ZeroAmount,
    AssetRuleMismatch,
    AssetRuleInactive,
    AssetDefinitionMismatch,
    AccountMissing,
    TrustlineMissing,
    TrustlineUnauthorized,
    TrustlineFrozen,
    HolderPermitInvalid,
    InsufficientBalance,
    AmountOverflow,
    DuplicateDeposit,
    DepositReceiptRejected,
    DepositObjectCollision,
    InvalidExit,
    ExitObjectMissing,
    ExitObjectReserved,
    ExitObjectFrozen,
    ExitOwnerMismatch,
    ExitAmountMismatch,
    ExitClaimCollision,
    ExitCertificateBelowQuorum,
    ExitCertificateMixed,
    UnknownExitValidator,
    InvalidExitVoteSignature,
    ExitClaimMismatch,
    ExitAlreadyRedeemed,
    RetiredCommitteeNotCheckpointed,
    DestinationLimitExceeded,
    SolvencyMismatch(FastAssetIdV1),
    Codec,
}

pub fn validate_fastlane_exit(
    state: &FastLaneStateV1,
    signed: &SignedFastLaneExitIntentV1,
) -> Result<FastLaneExitEffectsV1, FastLaneBridgeError> {
    validate_fastlane_exit_authorization(state, signed)?;
    let intent = &signed.intent;
    let mut total = 0_u64;
    for key in &intent.inputs {
        let object = state
            .objects
            .get(key)
            .ok_or(FastLaneBridgeError::ExitObjectMissing)?;
        if state.reservations.contains_key(key) {
            return Err(FastLaneBridgeError::ExitObjectReserved);
        }
        if !matches!(
            object.control_state,
            postfiat_types::FastAssetControlStateV1::Spendable
        ) {
            return Err(FastLaneBridgeError::ExitObjectFrozen);
        }
        if object.owner_pubkey != intent.owner_pubkey {
            return Err(FastLaneBridgeError::ExitOwnerMismatch);
        }
        if object.asset_id != intent.asset_id || object.asset_rule_hash != intent.asset_rule_hash {
            return Err(FastLaneBridgeError::AssetRuleMismatch);
        }
        total = total
            .checked_add(object.amount_atoms)
            .ok_or(FastLaneBridgeError::AmountOverflow)?;
    }
    if total != intent.amount_atoms {
        return Err(FastLaneBridgeError::ExitAmountMismatch);
    }
    let exit_id = intent.exit_id().map_err(|_| FastLaneBridgeError::Codec)?;
    let mut claim_preimage = Vec::new();
    claim_preimage.extend_from_slice(&exit_id.0);
    claim_preimage.extend_from_slice(&intent.asset_id.0);
    claim_preimage.extend_from_slice(&intent.asset_rule_hash.0);
    claim_preimage.extend_from_slice(&intent.amount_atoms.to_be_bytes());
    append_string(&mut claim_preimage, &intent.destination_address)?;
    let claim_digest = hash_bytes("postfiat.fastlane.exit_claim_id.v1", &claim_preimage);
    let claim = FastLaneExitClaimV1 {
        exit_claim_id: FastSwapExitClaimIdV1(
            claim_digest
                .try_into()
                .map_err(|_| FastLaneBridgeError::Codec)?,
        ),
        committee: intent.committee.clone(),
        owner_pubkey: intent.owner_pubkey.clone(),
        destination_address: intent.destination_address.clone(),
        asset_id: intent.asset_id,
        asset_rule_hash: intent.asset_rule_hash,
        amount_atoms: intent.amount_atoms,
    };
    Ok(FastLaneExitEffectsV1 {
        exit_id,
        consumed: intent.inputs.clone(),
        claim,
    })
}

pub fn validate_fastlane_exit_authorization(
    state: &FastLaneStateV1,
    signed: &SignedFastLaneExitIntentV1,
) -> Result<(), FastLaneBridgeError> {
    let intent = &signed.intent;
    if intent.committee != state.committee
        || intent.amount_atoms == 0
        || intent.destination_address.is_empty()
    {
        return Err(FastLaneBridgeError::InvalidExit);
    }
    let canonical = intent
        .canonical_bytes()
        .map_err(|_| FastLaneBridgeError::Codec)?;
    if signed.algorithm_id != FASTSWAP_ML_DSA_65
        || address_from_public_key(&intent.owner_pubkey) != intent.owner_address
        || !ml_dsa_65_verify_with_context(
            &intent.owner_pubkey,
            &canonical,
            &signed.signature,
            FASTLANE_EXIT_CONTEXT_V1,
        )
    {
        return Err(FastLaneBridgeError::InvalidSignature);
    }
    let rule = state
        .asset_rules
        .get(&intent.asset_rule_hash)
        .ok_or(FastLaneBridgeError::AssetRuleMismatch)?;
    if !rule.fast_lane_enabled || rule.asset_id != intent.asset_id {
        return Err(FastLaneBridgeError::AssetRuleInactive);
    }
    Ok(())
}

pub fn apply_fastlane_exit(
    state: &mut FastLaneStateV1,
    effects: &FastLaneExitEffectsV1,
) -> Result<(), FastLaneBridgeError> {
    effects
        .canonical_bytes()
        .map_err(|_| FastLaneBridgeError::Codec)?;
    if let Some(existing) = state.exit_claims.get(&effects.claim.exit_claim_id) {
        return if existing == &effects.claim
            && effects
                .consumed
                .iter()
                .all(|key| !state.objects.contains_key(key))
        {
            Ok(())
        } else {
            Err(FastLaneBridgeError::ExitClaimCollision)
        };
    }
    let mut next = state.clone();
    let mut total = 0_u64;
    for key in &effects.consumed {
        if next.reservations.contains_key(key) {
            return Err(FastLaneBridgeError::ExitObjectReserved);
        }
        let object = next
            .objects
            .remove(key)
            .ok_or(FastLaneBridgeError::ExitObjectMissing)?;
        if object.owner_pubkey != effects.claim.owner_pubkey
            || object.asset_id != effects.claim.asset_id
            || object.asset_rule_hash != effects.claim.asset_rule_hash
            || !matches!(
                object.control_state,
                postfiat_types::FastAssetControlStateV1::Spendable
            )
        {
            return Err(FastLaneBridgeError::ExitClaimMismatch);
        }
        total = total
            .checked_add(object.amount_atoms)
            .ok_or(FastLaneBridgeError::AmountOverflow)?;
    }
    if total != effects.claim.amount_atoms {
        return Err(FastLaneBridgeError::ExitAmountMismatch);
    }
    if next
        .exit_claims
        .insert(effects.claim.exit_claim_id, effects.claim.clone())
        .is_some()
    {
        return Err(FastLaneBridgeError::ExitClaimCollision);
    }
    *state = next;
    Ok(())
}

pub fn verify_fastlane_exit_certificate(
    committee: &FastSwapCommitteeV1,
    certificate: &FastLaneExitCertificateV1,
) -> Result<(), FastLaneBridgeError> {
    committee
        .validate()
        .map_err(|_| FastLaneBridgeError::ExitCertificateMixed)?;
    certificate
        .validate_canonical_order()
        .map_err(|_| FastLaneBridgeError::ExitCertificateMixed)?;
    if certificate.effects.claim.committee != committee.domain
        || certificate.votes.len() < usize::from(committee.domain.quorum)
    {
        return Err(FastLaneBridgeError::ExitCertificateBelowQuorum);
    }
    let exit_id = certificate.effects.exit_id;
    let effects_digest = certificate
        .effects
        .digest()
        .map_err(|_| FastLaneBridgeError::Codec)?;
    for vote in &certificate.votes {
        if vote.committee != committee.domain
            || vote.exit_id != exit_id
            || vote.effects_digest != effects_digest
        {
            return Err(FastLaneBridgeError::ExitCertificateMixed);
        }
        let validator = committee
            .validators
            .iter()
            .find(|row| row.validator_id == vote.validator_id)
            .ok_or(FastLaneBridgeError::UnknownExitValidator)?;
        if !ml_dsa_65_verify_with_context(
            &validator.public_key,
            &vote
                .signing_bytes()
                .map_err(|_| FastLaneBridgeError::Codec)?,
            &vote.signature,
            FASTLANE_EXIT_VOTE_CONTEXT_V1,
        ) {
            return Err(FastLaneBridgeError::InvalidExitVoteSignature);
        }
    }
    Ok(())
}

pub fn execute_fastlane_redeem(
    ledger: &mut LedgerState,
    signed: &SignedFastLaneRedeemV1,
    committee: &FastSwapCommitteeV1,
    committee_redemption_authorized: bool,
) -> Result<FastLaneRedeemReceiptV1, FastLaneBridgeError> {
    if signed.claim.committee != committee.domain
        || signed.exit_effects_qc.effects.claim != signed.claim
    {
        return Err(FastLaneBridgeError::ExitClaimMismatch);
    }
    if !committee_redemption_authorized {
        return Err(FastLaneBridgeError::RetiredCommitteeNotCheckpointed);
    }
    verify_fastlane_exit_certificate(committee, &signed.exit_effects_qc)?;
    if ledger
        .redeemed_fast_lane_exit_claims
        .contains(&signed.claim.exit_claim_id)
    {
        return Err(FastLaneBridgeError::ExitAlreadyRedeemed);
    }

    let mut next = ledger.clone();
    debit_reserve(
        &mut next.fast_lane_reserves,
        signed.claim.asset_id,
        signed.claim.amount_atoms,
    )?;
    if signed.claim.asset_id == FastAssetIdV1::native_pft() {
        let account = next
            .accounts
            .iter_mut()
            .find(|row| row.address == signed.claim.destination_address)
            .ok_or(FastLaneBridgeError::AccountMissing)?;
        account.balance = account
            .balance
            .checked_add(signed.claim.amount_atoms)
            .ok_or(FastLaneBridgeError::AmountOverflow)?;
    } else {
        let asset_hex = bytes_to_hex(&signed.claim.asset_id.0);
        let definition = next
            .asset_definitions
            .iter()
            .find(|row| row.asset_id == asset_hex)
            .ok_or(FastLaneBridgeError::AssetDefinitionMismatch)?;
        let line = next
            .trustlines
            .iter_mut()
            .find(|row| {
                row.account == signed.claim.destination_address && row.asset_id == asset_hex
            })
            .ok_or(FastLaneBridgeError::TrustlineMissing)?;
        if definition.requires_authorization && !line.authorized {
            return Err(FastLaneBridgeError::TrustlineUnauthorized);
        }
        if line.frozen {
            return Err(FastLaneBridgeError::TrustlineFrozen);
        }
        line.balance = line
            .balance
            .checked_add(signed.claim.amount_atoms)
            .ok_or(FastLaneBridgeError::AmountOverflow)?;
        if line.balance > line.limit {
            return Err(FastLaneBridgeError::DestinationLimitExceeded);
        }
    }
    next.redeemed_fast_lane_exit_claims
        .push(signed.claim.exit_claim_id);
    next.redeemed_fast_lane_exit_claims.sort();
    *ledger = next;
    Ok(FastLaneRedeemReceiptV1 {
        exit_claim_id: signed.claim.exit_claim_id,
        accepted: true,
        code: "fastlane_exit_redeemed".to_owned(),
        destination_address: signed.claim.destination_address.clone(),
        asset_id: signed.claim.asset_id,
        amount_atoms: signed.claim.amount_atoms,
    })
}

pub fn asset_definition_hash(
    definition: &AssetDefinition,
) -> Result<FastAssetDefinitionHashV1, FastLaneBridgeError> {
    let asset_id = hex_to_bytes(&definition.asset_id)
        .map_err(|_| FastLaneBridgeError::AssetDefinitionMismatch)?;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&(asset_id.len() as u32).to_be_bytes());
    bytes.extend_from_slice(&asset_id);
    append_string(&mut bytes, &definition.issuer)?;
    append_string(&mut bytes, &definition.code)?;
    bytes.extend_from_slice(&definition.version.to_be_bytes());
    bytes.push(definition.precision);
    append_string(&mut bytes, &definition.display_name)?;
    match definition.max_supply {
        Some(value) => {
            bytes.push(1);
            bytes.extend_from_slice(&value.to_be_bytes());
        }
        None => bytes.push(0),
    }
    bytes.push(u8::from(definition.requires_authorization));
    bytes.push(u8::from(definition.freeze_enabled));
    bytes.push(u8::from(definition.clawback_enabled));
    let digest = hash_bytes("postfiat.fastlane.asset_definition.v1", &bytes);
    Ok(FastAssetDefinitionHashV1(
        digest.try_into().map_err(|_| FastLaneBridgeError::Codec)?,
    ))
}

pub fn execute_fastlane_deposit(
    ledger: &mut LedgerState,
    signed: &SignedFastLaneDepositV1,
    expected_domain: &postfiat_types::FastSwapChainDomainV1,
    rule: &FastAssetRuleV1,
    finalized_height: u64,
) -> Result<FastLaneDepositReceiptV1, FastLaneBridgeError> {
    let deposit = &signed.deposit;
    if &deposit.domain != expected_domain {
        return Err(FastLaneBridgeError::DomainMismatch);
    }
    if deposit.amount_atoms == 0 || deposit.fee_pft == 0 {
        return Err(FastLaneBridgeError::ZeroAmount);
    }
    if signed.algorithm_id != FASTSWAP_ML_DSA_65
        || !ml_dsa_65_verify_with_context(
            &deposit.source_pubkey,
            &deposit
                .signing_bytes()
                .map_err(|_| FastLaneBridgeError::Codec)?,
            &signed.signature,
            FASTLANE_DEPOSIT_CONTEXT_V1,
        )
    {
        return Err(FastLaneBridgeError::InvalidSignature);
    }
    if address_from_public_key(&deposit.source_pubkey) != deposit.source_address {
        return Err(FastLaneBridgeError::SourceKeyMismatch);
    }
    if rule.asset_id != deposit.asset_id
        || rule.rule_hash().map_err(|_| FastLaneBridgeError::Codec)? != deposit.asset_rule_hash
    {
        return Err(FastLaneBridgeError::AssetRuleMismatch);
    }
    if !rule.fast_lane_enabled
        || finalized_height < rule.valid_from_height
        || finalized_height > rule.valid_through_height
    {
        return Err(FastLaneBridgeError::AssetRuleInactive);
    }
    match (
        rule.requires_authorization,
        deposit.destination_holder_permit_id,
    ) {
        (false, None) => {}
        (false, Some(_)) | (true, None) => return Err(FastLaneBridgeError::HolderPermitInvalid),
        (true, Some(permit_id)) => {
            let permit = ledger
                .fast_lane_holder_permits
                .iter()
                .find(|permit| permit.permit_id == permit_id)
                .ok_or(FastLaneBridgeError::HolderPermitInvalid)?;
            if permit.computed_id().ok() != Some(permit_id)
                || permit.asset_id != deposit.asset_id
                || permit.owner_pubkey != deposit.destination_owner_pubkey
                || finalized_height < permit.valid_from_height
                || finalized_height > permit.valid_through_height
            {
                return Err(FastLaneBridgeError::HolderPermitInvalid);
            }
        }
    }
    let deposit_id = deposit
        .deposit_id()
        .map_err(|_| FastLaneBridgeError::Codec)?;
    if ledger
        .fast_lane_deposit_receipts
        .iter()
        .any(|receipt| receipt.deposit_id == deposit_id)
    {
        return Err(FastLaneBridgeError::DuplicateDeposit);
    }

    let mut next = ledger.clone();
    let account_index = next
        .accounts
        .iter()
        .position(|account| account.address == deposit.source_address)
        .ok_or(FastLaneBridgeError::AccountMissing)?;
    let account = &next.accounts[account_index];
    if account.sequence.checked_add(1) != Some(deposit.sequence) {
        return Err(FastLaneBridgeError::InvalidSequence);
    }
    if account
        .public_key_hex
        .as_ref()
        .is_some_and(|value| value != &bytes_to_hex(&deposit.source_pubkey))
    {
        return Err(FastLaneBridgeError::SourceKeyMismatch);
    }
    if deposit.asset_id == FastAssetIdV1::native_pft() {
        let debit = deposit
            .amount_atoms
            .checked_add(deposit.fee_pft)
            .ok_or(FastLaneBridgeError::AmountOverflow)?;
        let account = &mut next.accounts[account_index];
        account.balance = account
            .balance
            .checked_sub(debit)
            .ok_or(FastLaneBridgeError::InsufficientBalance)?;
        account.sequence = deposit.sequence;
    } else {
        let asset_id_hex = bytes_to_hex(&deposit.asset_id.0);
        let definition = next
            .asset_definitions
            .iter()
            .find(|definition| definition.asset_id == asset_id_hex)
            .ok_or(FastLaneBridgeError::AssetDefinitionMismatch)?;
        if asset_definition_hash(definition)? != rule.asset_definition_hash
            || definition.issuer != rule.issuer_address
            || definition.requires_authorization != rule.requires_authorization
            || definition.freeze_enabled != rule.freeze_enabled
            || definition.clawback_enabled != rule.clawback_enabled
        {
            return Err(FastLaneBridgeError::AssetDefinitionMismatch);
        }
        let line_index = next
            .trustlines
            .iter()
            .position(|line| {
                line.account == deposit.source_address && line.asset_id == asset_id_hex
            })
            .ok_or(FastLaneBridgeError::TrustlineMissing)?;
        if rule.requires_authorization && !next.trustlines[line_index].authorized {
            return Err(FastLaneBridgeError::TrustlineUnauthorized);
        }
        if next.trustlines[line_index].frozen {
            return Err(FastLaneBridgeError::TrustlineFrozen);
        }
        next.trustlines[line_index].balance = next.trustlines[line_index]
            .balance
            .checked_sub(deposit.amount_atoms)
            .ok_or(FastLaneBridgeError::InsufficientBalance)?;
        let account = &mut next.accounts[account_index];
        account.balance = account
            .balance
            .checked_sub(deposit.fee_pft)
            .ok_or(FastLaneBridgeError::InsufficientBalance)?;
        account.sequence = deposit.sequence;
    }
    add_reserve(
        &mut next.fast_lane_reserves,
        deposit.asset_id,
        deposit.amount_atoms,
    )?;
    let initial_object_key = initial_deposit_object_key(deposit_id, deposit)?;
    let receipt = FastLaneDepositReceiptV1 {
        deposit_id,
        accepted: true,
        code: "fastlane_deposit_applied".to_owned(),
        destination_owner_pubkey: deposit.destination_owner_pubkey.clone(),
        asset_id: deposit.asset_id,
        asset_rule_hash: deposit.asset_rule_hash,
        amount_atoms: deposit.amount_atoms,
        initial_object_key,
    };
    next.fast_lane_deposit_receipts.push(receipt.clone());
    *ledger = next;
    Ok(receipt)
}

pub fn import_finalized_fastlane_deposit(
    state: &mut FastLaneStateV1,
    receipt: &FastLaneDepositReceiptV1,
) -> Result<(), FastLaneBridgeError> {
    if !receipt.accepted || receipt.code != "fastlane_deposit_applied" {
        return Err(FastLaneBridgeError::DepositReceiptRejected);
    }
    if state.imported_deposits.contains(&receipt.deposit_id) {
        if let Some(existing) = state.objects.get(&receipt.initial_object_key) {
            if existing.asset_id != receipt.asset_id
                || existing.asset_rule_hash != receipt.asset_rule_hash
                || existing.amount_atoms != receipt.amount_atoms
                || existing.owner_pubkey != receipt.destination_owner_pubkey
            {
                return Err(FastLaneBridgeError::DuplicateDeposit);
            }
        }
        // An absent initial object is the expected state after a finalized
        // swap or exit consumes it. `imported_deposits` is durable and the
        // receipt comes from canonical consensus state, so replay must verify
        // without resurrecting the already-consumed object.
        return Ok(());
    }
    if state.objects.contains_key(&receipt.initial_object_key) {
        return Err(FastLaneBridgeError::DepositObjectCollision);
    }
    let rule = state
        .asset_rules
        .get(&receipt.asset_rule_hash)
        .ok_or(FastLaneBridgeError::AssetRuleMismatch)?;
    if rule.asset_id != receipt.asset_id || !rule.fast_lane_enabled {
        return Err(FastLaneBridgeError::AssetRuleMismatch);
    }
    state.objects.insert(
        receipt.initial_object_key,
        FastAssetObjectV1 {
            key: receipt.initial_object_key,
            owner_pubkey: receipt.destination_owner_pubkey.clone(),
            asset_id: receipt.asset_id,
            asset_rule_hash: receipt.asset_rule_hash,
            amount_atoms: receipt.amount_atoms,
            control_state: postfiat_types::FastAssetControlStateV1::Spendable,
            origin: FastObjectOriginV1::Deposit {
                deposit_id: receipt.deposit_id,
            },
        },
    );
    state.imported_deposits.insert(receipt.deposit_id);
    Ok(())
}

pub fn verify_fastlane_solvency(
    ledger: &LedgerState,
    state: &FastLaneStateV1,
) -> Result<(), FastLaneBridgeError> {
    let mut liabilities = BTreeMap::<FastAssetIdV1, u128>::new();
    for object in state.objects.values() {
        add_total(
            &mut liabilities,
            object.asset_id,
            u128::from(object.amount_atoms),
        )?;
    }
    let redeemed = ledger
        .redeemed_fast_lane_exit_claims
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    for claim in state.exit_claims.values() {
        if redeemed.contains(&claim.exit_claim_id) {
            continue;
        }
        add_total(
            &mut liabilities,
            claim.asset_id,
            u128::from(claim.amount_atoms),
        )?;
    }
    for (asset_id, amount) in &state.pending_fee_burns {
        add_total(&mut liabilities, *asset_id, *amount)?;
    }
    let reserves = ledger
        .fast_lane_reserves
        .iter()
        .map(|reserve| (reserve.asset_id, reserve.amount_atoms))
        .collect::<BTreeMap<_, _>>();
    let assets = reserves
        .keys()
        .chain(liabilities.keys())
        .copied()
        .collect::<BTreeSet<_>>();
    for asset in assets {
        if reserves.get(&asset).copied().unwrap_or(0)
            != liabilities.get(&asset).copied().unwrap_or(0)
        {
            return Err(FastLaneBridgeError::SolvencyMismatch(asset));
        }
    }
    Ok(())
}

fn initial_deposit_object_key(
    deposit_id: postfiat_types::FastSwapDepositIdV1,
    deposit: &postfiat_types::FastLaneDepositV1,
) -> Result<FastObjectKeyV1, FastLaneBridgeError> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&deposit_id.0);
    bytes.extend_from_slice(&deposit.asset_id.0);
    bytes.extend_from_slice(&deposit.asset_rule_hash.0);
    bytes.extend_from_slice(&deposit.amount_atoms.to_be_bytes());
    bytes.extend_from_slice(&(deposit.destination_owner_pubkey.len() as u32).to_be_bytes());
    bytes.extend_from_slice(&deposit.destination_owner_pubkey);
    let digest = hash_bytes("postfiat.fastlane.deposit_object.v1", &bytes);
    Ok(FastObjectKeyV1 {
        object_id: FastObjectIdV1(
            digest[..32]
                .try_into()
                .map_err(|_| FastLaneBridgeError::Codec)?,
        ),
        version: 1,
    })
}

fn add_reserve(
    reserves: &mut Vec<FastLaneReserveBalanceV1>,
    asset_id: FastAssetIdV1,
    amount: u64,
) -> Result<(), FastLaneBridgeError> {
    if let Some(reserve) = reserves.iter_mut().find(|row| row.asset_id == asset_id) {
        reserve.amount_atoms = reserve
            .amount_atoms
            .checked_add(u128::from(amount))
            .ok_or(FastLaneBridgeError::AmountOverflow)?;
    } else {
        reserves.push(FastLaneReserveBalanceV1 {
            asset_id,
            amount_atoms: u128::from(amount),
        });
        reserves.sort_by_key(|row| row.asset_id);
    }
    Ok(())
}

fn debit_reserve(
    reserves: &mut [FastLaneReserveBalanceV1],
    asset_id: FastAssetIdV1,
    amount: u64,
) -> Result<(), FastLaneBridgeError> {
    let reserve = reserves
        .iter_mut()
        .find(|row| row.asset_id == asset_id)
        .ok_or(FastLaneBridgeError::InsufficientBalance)?;
    reserve.amount_atoms = reserve
        .amount_atoms
        .checked_sub(u128::from(amount))
        .ok_or(FastLaneBridgeError::InsufficientBalance)?;
    Ok(())
}

fn add_total(
    totals: &mut BTreeMap<FastAssetIdV1, u128>,
    asset_id: FastAssetIdV1,
    amount: u128,
) -> Result<(), FastLaneBridgeError> {
    let total = totals.entry(asset_id).or_default();
    *total = total
        .checked_add(amount)
        .ok_or(FastLaneBridgeError::AmountOverflow)?;
    Ok(())
}

fn append_string(output: &mut Vec<u8>, value: &str) -> Result<(), FastLaneBridgeError> {
    let length: u32 = value
        .len()
        .try_into()
        .map_err(|_| FastLaneBridgeError::Codec)?;
    output.extend_from_slice(&length.to_be_bytes());
    output.extend_from_slice(value.as_bytes());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use postfiat_crypto_provider::{ml_dsa_65_keygen_from_seed, ml_dsa_65_sign_with_context};
    use postfiat_types::{
        Account, FastHolderPermitIdV1, FastHolderPermitV1, FastLaneExitCertificateV1,
        FastLaneExitIntentV1, FastLaneExitVoteV1, FastSwapCommitteeDomainV1,
        FastSwapCommitteeRootV1, FastSwapCommitteeV1, FastSwapOpaqueHashV1, FastSwapValidatorV1,
        TrustLine, FASTLANE_EXIT_CONTEXT_V1, FASTLANE_EXIT_VOTE_CONTEXT_V1,
        FASTSWAP_SCHEMA_VERSION_V1,
    };

    fn domain() -> postfiat_types::FastSwapChainDomainV1 {
        postfiat_types::FastSwapChainDomainV1 {
            chain_id: "postfiat-fastlane-test".to_owned(),
            genesis_hash: FastSwapOpaqueHashV1([1; 48]),
            protocol_version: 1,
        }
    }

    fn state(rule: FastAssetRuleV1) -> FastLaneStateV1 {
        let rule_hash = rule.rule_hash().expect("rule hash");
        FastLaneStateV1 {
            schema_version: 1,
            committee: FastSwapCommitteeDomainV1 {
                chain: domain(),
                fastswap_schema_version: FASTSWAP_SCHEMA_VERSION_V1,
                committee_epoch: 1,
                committee_root: FastSwapCommitteeRootV1([2; 48]),
                validator_count: 6,
                quorum: 5,
            },
            objects: BTreeMap::new(),
            reservations: BTreeMap::new(),
            swaps: BTreeMap::new(),
            imported_deposits: BTreeSet::new(),
            exit_claims: BTreeMap::new(),
            terminal_tombstones: BTreeMap::new(),
            asset_rules: BTreeMap::from([(rule_hash, rule)]),
            holder_permits: BTreeMap::new(),
            policy_snapshots: BTreeMap::new(),
            prepare_fences: BTreeMap::new(),
            pending_fee_burns: BTreeMap::new(),
            anchored_checkpoints: BTreeSet::new(),
        }
    }

    fn committee() -> (FastSwapCommitteeV1, Vec<Vec<u8>>) {
        let pairs = (0..6)
            .map(|index| ml_dsa_65_keygen_from_seed(&[index + 20; 32]))
            .collect::<Vec<_>>();
        let mut committee = FastSwapCommitteeV1 {
            domain: FastSwapCommitteeDomainV1 {
                chain: domain(),
                fastswap_schema_version: FASTSWAP_SCHEMA_VERSION_V1,
                committee_epoch: 1,
                committee_root: FastSwapCommitteeRootV1::ZERO,
                validator_count: 6,
                quorum: 5,
            },
            validators: pairs
                .iter()
                .enumerate()
                .map(|(index, pair)| FastSwapValidatorV1 {
                    validator_id: format!("validator-{index}"),
                    public_key: pair.public_key.clone(),
                })
                .collect(),
        };
        committee.domain.committee_root = committee.computed_root().expect("committee root");
        let private_keys = pairs
            .into_iter()
            .map(|pair| pair.private_key.to_vec())
            .collect();
        (committee, private_keys)
    }

    #[test]
    fn issued_deposit_debits_trustline_credits_reserve_and_imports_once() {
        let owner = ml_dsa_65_keygen_from_seed(&[7; 32]);
        let issuer = ml_dsa_65_keygen_from_seed(&[8; 32]);
        let owner_address = address_from_public_key(&owner.public_key);
        let issuer_address = address_from_public_key(&issuer.public_key);
        let mut definition =
            AssetDefinition::new(&domain().chain_id, issuer_address.clone(), "USD", 1, 6)
                .expect("asset");
        definition.requires_authorization = true;
        let asset_bytes: [u8; 48] = hex_to_bytes(&definition.asset_id)
            .expect("asset hex")
            .try_into()
            .expect("asset width");
        let asset_id = FastAssetIdV1(asset_bytes);
        let rule = FastAssetRuleV1 {
            asset_id,
            asset_definition_hash: asset_definition_hash(&definition).expect("definition hash"),
            issuer_address: issuer_address.clone(),
            issuer_control_pubkey: issuer.public_key,
            requires_authorization: true,
            freeze_enabled: false,
            clawback_enabled: false,
            fast_lane_enabled: true,
            valid_from_height: 100,
            valid_through_height: 200,
        };
        let rule_hash = rule.rule_hash().expect("rule hash");
        let mut line = TrustLine::new(
            owner_address.clone(),
            issuer_address,
            definition.asset_id.clone(),
            1_000,
            0,
        )
        .expect("trustline");
        line.balance = 50;
        line.authorized = true;
        let mut ledger = LedgerState::new_with_assets(
            vec![Account {
                address: owner_address.clone(),
                balance: 100,
                sequence: 0,
                public_key_hex: Some(bytes_to_hex(&owner.public_key)),
            }],
            vec![definition],
            vec![line],
        );
        let mut permit = FastHolderPermitV1 {
            permit_id: FastHolderPermitIdV1::ZERO,
            asset_id,
            owner_pubkey: owner.public_key.clone(),
            valid_from_height: 100,
            valid_through_height: 200,
            consensus_receipt_digest: FastSwapOpaqueHashV1([10; 48]),
        };
        permit.permit_id = permit.computed_id().expect("permit id");
        let deposit = postfiat_types::FastLaneDepositV1 {
            domain: domain(),
            source_address: owner_address,
            source_pubkey: owner.public_key.clone(),
            sequence: 1,
            fee_pft: 2,
            destination_owner_pubkey: owner.public_key.clone(),
            destination_holder_permit_id: Some(permit.permit_id),
            asset_id,
            asset_rule_hash: rule_hash,
            amount_atoms: 20,
            nonce: [9; 32],
        };
        let signed = SignedFastLaneDepositV1 {
            signature: ml_dsa_65_sign_with_context(
                &owner.private_key,
                &deposit.signing_bytes().expect("deposit bytes"),
                FASTLANE_DEPOSIT_CONTEXT_V1,
            )
            .expect("deposit sign"),
            algorithm_id: FASTSWAP_ML_DSA_65.to_owned(),
            deposit,
        };
        let before = ledger.clone();
        assert_eq!(
            execute_fastlane_deposit(&mut ledger, &signed, &domain(), &rule, 110),
            Err(FastLaneBridgeError::HolderPermitInvalid)
        );
        assert_eq!(ledger, before);
        ledger.fast_lane_holder_permits.push(permit);
        let receipt =
            execute_fastlane_deposit(&mut ledger, &signed, &domain(), &rule, 110).expect("deposit");
        assert_eq!(ledger.accounts[0].balance, 98);
        assert_eq!(ledger.accounts[0].sequence, 1);
        assert_eq!(ledger.trustlines[0].balance, 30);
        assert_eq!(ledger.fast_lane_reserves[0].amount_atoms, 20);

        let mut fast_state = state(rule);
        import_finalized_fastlane_deposit(&mut fast_state, &receipt).expect("import");
        let after = fast_state.clone();
        import_finalized_fastlane_deposit(&mut fast_state, &receipt).expect("idempotent import");
        assert_eq!(fast_state, after);
        verify_fastlane_solvency(&ledger, &fast_state).expect("solvent");
        ledger.fast_lane_reserves[0].amount_atoms -= 1;
        assert!(matches!(
            verify_fastlane_solvency(&ledger, &fast_state),
            Err(FastLaneBridgeError::SolvencyMismatch(id)) if id == asset_id
        ));
        ledger.fast_lane_reserves[0].amount_atoms += 1;
        let mut conflicting = receipt.clone();
        conflicting.amount_atoms += 1;
        assert_eq!(
            import_finalized_fastlane_deposit(&mut fast_state, &conflicting),
            Err(FastLaneBridgeError::DuplicateDeposit),
            "a live initial object still detects a conflicting replay"
        );
        fast_state.objects.remove(&receipt.initial_object_key);
        let consumed = fast_state.clone();
        import_finalized_fastlane_deposit(&mut fast_state, &receipt)
            .expect("consumed deposit replay is idempotent");
        assert_eq!(
            fast_state, consumed,
            "replay must not resurrect a consumed object"
        );
    }

    #[test]
    fn rejected_or_invalid_deposit_never_mutates_ledger_or_fast_state() {
        let owner = ml_dsa_65_keygen_from_seed(&[3; 32]);
        let native = FastAssetIdV1::native_pft();
        let rule = FastAssetRuleV1 {
            asset_id: native,
            asset_definition_hash: FastAssetDefinitionHashV1::ZERO,
            issuer_address: "native".to_owned(),
            issuer_control_pubkey: vec![1],
            requires_authorization: false,
            freeze_enabled: false,
            clawback_enabled: false,
            fast_lane_enabled: true,
            valid_from_height: 1,
            valid_through_height: 100,
        };
        let address = address_from_public_key(&owner.public_key);
        let mut ledger = LedgerState::new(vec![Account {
            address: address.clone(),
            balance: 5,
            sequence: 0,
            public_key_hex: Some(bytes_to_hex(&owner.public_key)),
        }]);
        let before = ledger.clone();
        let deposit = postfiat_types::FastLaneDepositV1 {
            domain: domain(),
            source_address: address,
            source_pubkey: owner.public_key,
            sequence: 1,
            fee_pft: 1,
            destination_owner_pubkey: vec![7; 64],
            destination_holder_permit_id: None,
            asset_id: native,
            asset_rule_hash: rule.rule_hash().expect("rule"),
            amount_atoms: 5,
            nonce: [4; 32],
        };
        let signed = SignedFastLaneDepositV1 {
            deposit,
            algorithm_id: FASTSWAP_ML_DSA_65.to_owned(),
            signature: vec![0; 3],
        };
        assert_eq!(
            execute_fastlane_deposit(&mut ledger, &signed, &domain(), &rule, 10),
            Err(FastLaneBridgeError::InvalidSignature)
        );
        assert_eq!(ledger, before);
    }

    #[test]
    fn certified_exit_redeems_exactly_once_and_preserves_solvency() {
        let owner = ml_dsa_65_keygen_from_seed(&[11; 32]);
        let owner_address = address_from_public_key(&owner.public_key);
        let native = FastAssetIdV1::native_pft();
        let rule = FastAssetRuleV1 {
            asset_id: native,
            asset_definition_hash: FastAssetDefinitionHashV1::ZERO,
            issuer_address: "native".to_owned(),
            issuer_control_pubkey: vec![1],
            requires_authorization: false,
            freeze_enabled: false,
            clawback_enabled: false,
            fast_lane_enabled: true,
            valid_from_height: 1,
            valid_through_height: 1_000,
        };
        let rule_hash = rule.rule_hash().expect("rule hash");
        let (committee, private_keys) = committee();
        let key = FastObjectKeyV1 {
            object_id: FastObjectIdV1([12; 32]),
            version: 1,
        };
        let mut fast_state = state(rule);
        fast_state.committee = committee.domain.clone();
        fast_state.objects.insert(
            key,
            FastAssetObjectV1 {
                key,
                owner_pubkey: owner.public_key.clone(),
                asset_id: native,
                asset_rule_hash: rule_hash,
                amount_atoms: 10,
                control_state: postfiat_types::FastAssetControlStateV1::Spendable,
                origin: FastObjectOriginV1::Deposit {
                    deposit_id: postfiat_types::FastSwapDepositIdV1([13; 48]),
                },
            },
        );
        let mut ledger = LedgerState::new(vec![Account {
            address: owner_address.clone(),
            balance: 5,
            sequence: 0,
            public_key_hex: Some(bytes_to_hex(&owner.public_key)),
        }]);
        ledger.fast_lane_reserves.push(FastLaneReserveBalanceV1 {
            asset_id: native,
            amount_atoms: 10,
        });
        verify_fastlane_solvency(&ledger, &fast_state).expect("initial solvency");

        let intent = FastLaneExitIntentV1 {
            committee: committee.domain.clone(),
            owner_address: owner_address.clone(),
            owner_pubkey: owner.public_key.clone(),
            inputs: vec![key],
            asset_id: native,
            asset_rule_hash: rule_hash,
            amount_atoms: 10,
            destination_address: owner_address,
            nonce: [14; 32],
        };
        let signed = SignedFastLaneExitIntentV1 {
            signature: ml_dsa_65_sign_with_context(
                &owner.private_key,
                &intent.canonical_bytes().expect("intent bytes"),
                FASTLANE_EXIT_CONTEXT_V1,
            )
            .expect("exit signature"),
            algorithm_id: FASTSWAP_ML_DSA_65.to_owned(),
            intent,
        };
        let effects = validate_fastlane_exit(&fast_state, &signed).expect("valid exit");
        apply_fastlane_exit(&mut fast_state, &effects).expect("apply exit");
        assert!(!fast_state.objects.contains_key(&key));
        assert!(fast_state
            .exit_claims
            .contains_key(&effects.claim.exit_claim_id));
        verify_fastlane_solvency(&ledger, &fast_state).expect("claim remains liability");

        let digest = effects.digest().expect("effects digest");
        let votes = (0..5)
            .map(|index| {
                let mut vote = FastLaneExitVoteV1 {
                    committee: committee.domain.clone(),
                    exit_id: effects.exit_id,
                    effects_digest: digest,
                    validator_id: format!("validator-{index}"),
                    signature: Vec::new(),
                };
                vote.signature = ml_dsa_65_sign_with_context(
                    &private_keys[index],
                    &vote.signing_bytes().expect("vote bytes"),
                    FASTLANE_EXIT_VOTE_CONTEXT_V1,
                )
                .expect("vote signature");
                vote
            })
            .collect();
        let certificate = FastLaneExitCertificateV1 {
            effects: effects.clone(),
            votes,
        };
        verify_fastlane_exit_certificate(&committee, &certificate).expect("exit QC");
        let mut under_quorum = certificate.clone();
        under_quorum.votes.pop();
        assert_eq!(
            verify_fastlane_exit_certificate(&committee, &under_quorum),
            Err(FastLaneBridgeError::ExitCertificateBelowQuorum)
        );
        let mut duplicate_validator = certificate.clone();
        duplicate_validator.votes[4] = duplicate_validator.votes[3].clone();
        assert_eq!(
            verify_fastlane_exit_certificate(&committee, &duplicate_validator),
            Err(FastLaneBridgeError::ExitCertificateMixed)
        );
        let redeem = SignedFastLaneRedeemV1 {
            claim: effects.claim.clone(),
            exit_effects_qc: certificate,
        };
        let receipt =
            execute_fastlane_redeem(&mut ledger, &redeem, &committee, true).expect("redeem exit");
        assert!(receipt.accepted);
        assert_eq!(receipt.code, "fastlane_exit_redeemed");
        assert_eq!(ledger.accounts[0].balance, 15);
        assert_eq!(ledger.fast_lane_reserves[0].amount_atoms, 0);
        verify_fastlane_solvency(&ledger, &fast_state).expect("redeemed claim excluded");
        let after = ledger.clone();
        assert_eq!(
            execute_fastlane_redeem(&mut ledger, &redeem, &committee, true),
            Err(FastLaneBridgeError::ExitAlreadyRedeemed)
        );
        assert_eq!(ledger, after);
    }

    #[test]
    fn exit_loses_cleanly_to_an_existing_swap_reservation() {
        let owner = ml_dsa_65_keygen_from_seed(&[15; 32]);
        let native = FastAssetIdV1::native_pft();
        let rule = FastAssetRuleV1 {
            asset_id: native,
            asset_definition_hash: FastAssetDefinitionHashV1::ZERO,
            issuer_address: "native".to_owned(),
            issuer_control_pubkey: vec![1],
            requires_authorization: false,
            freeze_enabled: false,
            clawback_enabled: false,
            fast_lane_enabled: true,
            valid_from_height: 1,
            valid_through_height: 1_000,
        };
        let rule_hash = rule.rule_hash().expect("rule hash");
        let (committee, _) = committee();
        let key = FastObjectKeyV1 {
            object_id: FastObjectIdV1([16; 32]),
            version: 1,
        };
        let mut fast_state = state(rule);
        fast_state.committee = committee.domain.clone();
        fast_state.objects.insert(
            key,
            FastAssetObjectV1 {
                key,
                owner_pubkey: owner.public_key.clone(),
                asset_id: native,
                asset_rule_hash: rule_hash,
                amount_atoms: 10,
                control_state: postfiat_types::FastAssetControlStateV1::Spendable,
                origin: FastObjectOriginV1::Deposit {
                    deposit_id: postfiat_types::FastSwapDepositIdV1([17; 48]),
                },
            },
        );
        fast_state.reservations.insert(
            key,
            postfiat_types::FastSwapReservationV1 {
                swap_id: postfiat_types::FastSwapIdV1([18; 48]),
                intent_id: postfiat_types::FastSwapIntentIdV1([19; 48]),
                effects_digest: postfiat_types::FastSwapEffectsDigestV1([20; 48]),
            },
        );
        let intent = FastLaneExitIntentV1 {
            committee: committee.domain,
            owner_address: address_from_public_key(&owner.public_key),
            owner_pubkey: owner.public_key.clone(),
            inputs: vec![key],
            asset_id: native,
            asset_rule_hash: rule_hash,
            amount_atoms: 10,
            destination_address: "destination".to_owned(),
            nonce: [21; 32],
        };
        let signed = SignedFastLaneExitIntentV1 {
            signature: ml_dsa_65_sign_with_context(
                &owner.private_key,
                &intent.canonical_bytes().expect("intent bytes"),
                FASTLANE_EXIT_CONTEXT_V1,
            )
            .expect("exit signature"),
            algorithm_id: FASTSWAP_ML_DSA_65.to_owned(),
            intent,
        };
        assert_eq!(
            validate_fastlane_exit(&fast_state, &signed),
            Err(FastLaneBridgeError::ExitObjectReserved)
        );
    }
}
