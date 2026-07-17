use postfiat_crypto_provider::{
    address_from_public_key, hash_bytes, ml_dsa_65_verify_with_context,
};
use postfiat_types::{
    FastAssetAmountV1, FastAssetControlStateV1, FastAssetIdV1, FastAssetObjectV1, FastLaneStateV1,
    FastObjectIdV1, FastObjectKeyV1, FastObjectOriginV1, FastSwapCodecError, FastSwapDecisionV1,
    FastSwapEffectsV1, FastSwapIdV1, FastSwapIntentIdV1, FastSwapPartyV1, FastSwapPolicySnapshotV1,
    FastSwapQuoteRoundingV1, FastSwapReceiptV1, SignedFastSwapIntentV1, FASTSWAP_INTENT_CONTEXT_V1,
    FASTSWAP_MAX_OUTPUTS, FASTSWAP_ML_DSA_65,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FastSwapErrorClass {
    ByzantineInput,
    Retryable,
    LocalFatal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum FastSwapAdmissionError {
    Codec(FastSwapCodecError),
    DomainMismatch,
    PolicyMissing,
    PolicyMismatch,
    PolicyPaused,
    PolicyFenced,
    PolicyOutsideHeightWindow,
    PolicyRatioInvalid,
    AuthorizationMalformed(u8),
    AuthorizationInvalid(u8),
    OwnerAddressMismatch(u8),
    NotReciprocal,
    UnsupportedHolderPermit,
    InputMissing(FastObjectKeyV1),
    InputFrozen(FastObjectKeyV1),
    InputOwnerMismatch(FastObjectKeyV1),
    InputAssetMismatch(FastObjectKeyV1),
    DuplicateInput(FastObjectKeyV1),
    InputReserved(FastObjectKeyV1),
    AmountOverflow,
    AssetConservation,
    FeeConservation,
    OutputLimit,
    OutputCollision(FastObjectKeyV1),
}

impl FastSwapAdmissionError {
    pub fn class(&self) -> FastSwapErrorClass {
        match self {
            Self::InputReserved(_) => FastSwapErrorClass::Retryable,
            Self::OutputCollision(_) => FastSwapErrorClass::LocalFatal,
            _ => FastSwapErrorClass::ByzantineInput,
        }
    }
}

impl From<FastSwapCodecError> for FastSwapAdmissionError {
    fn from(value: FastSwapCodecError) -> Self {
        Self::Codec(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedFastSwapV1 {
    pub swap_id: FastSwapIdV1,
    pub intent_id: FastSwapIntentIdV1,
    pub effects: FastSwapEffectsV1,
}

pub fn validate_fastswap_admission(
    state: &FastLaneStateV1,
    signed: &SignedFastSwapIntentV1,
    finalized_height: u64,
) -> Result<PreparedFastSwapV1, FastSwapAdmissionError> {
    signed.intent.validate_canonical_shape()?;
    if signed.intent.domain != state.committee {
        return Err(FastSwapAdmissionError::DomainMismatch);
    }
    let policy = state
        .policy_snapshots
        .get(&signed.intent.policy_hash)
        .ok_or(FastSwapAdmissionError::PolicyMissing)?;
    if state.prepare_fences.contains_key(&policy.policy_epoch) {
        return Err(FastSwapAdmissionError::PolicyFenced);
    }
    validate_policy(policy, signed, finalized_height)?;
    validate_asset_rules(state, signed, finalized_height)?;
    validate_reciprocal(signed)?;
    validate_authorization(signed, 0)?;
    validate_authorization(signed, 1)?;

    let swap_id = signed.swap_id()?;
    let intent_id = signed.intent.intent_id()?;
    let mut consumed = BTreeSet::new();
    let mut input_totals = BTreeMap::<FastAssetIdV1, u128>::new();
    for party in [&signed.intent.party_0, &signed.intent.party_1] {
        validate_party_inputs(state, party, swap_id, &mut consumed, &mut input_totals)?;
    }

    let mut created = Vec::with_capacity(6);
    let mut output_index = 0_u16;
    create_party_outputs(
        swap_id,
        &signed.intent.party_0,
        &signed.intent.party_1,
        &mut output_index,
        &mut created,
    )?;
    create_party_outputs(
        swap_id,
        &signed.intent.party_1,
        &signed.intent.party_0,
        &mut output_index,
        &mut created,
    )?;
    if created.len() > FASTSWAP_MAX_OUTPUTS {
        return Err(FastSwapAdmissionError::OutputLimit);
    }
    let mut created_keys = BTreeSet::new();
    for object in &created {
        if state.objects.contains_key(&object.key) || !created_keys.insert(object.key) {
            return Err(FastSwapAdmissionError::OutputCollision(object.key));
        }
    }

    let total_fee = signed
        .intent
        .party_0
        .fee_burn_pft
        .checked_add(signed.intent.party_1.fee_burn_pft)
        .ok_or(FastSwapAdmissionError::AmountOverflow)?;
    let fee_burns = if total_fee == 0 {
        Vec::new()
    } else {
        vec![FastAssetAmountV1 {
            asset_id: FastAssetIdV1::native_pft(),
            amount_atoms: total_fee,
        }]
    };
    verify_conservation(&input_totals, &created, &fee_burns)?;

    let consumed = consumed.into_iter().collect::<Vec<_>>();
    let receipt = FastSwapReceiptV1 {
        swap_id,
        accepted: true,
        code: "fastswap_applied".to_owned(),
        consumed_count: consumed
            .len()
            .try_into()
            .map_err(|_| FastSwapAdmissionError::OutputLimit)?,
        created_count: created
            .len()
            .try_into()
            .map_err(|_| FastSwapAdmissionError::OutputLimit)?,
    };
    let effects = FastSwapEffectsV1 {
        domain: signed.intent.domain.clone(),
        swap_id,
        policy_hash: signed.intent.policy_hash,
        decision: FastSwapDecisionV1::Confirm,
        consumed,
        created,
        fee_burns,
        receipt,
    };
    effects.digest()?;
    Ok(PreparedFastSwapV1 {
        swap_id,
        intent_id,
        effects,
    })
}

fn validate_policy(
    policy: &FastSwapPolicySnapshotV1,
    signed: &SignedFastSwapIntentV1,
    finalized_height: u64,
) -> Result<(), FastSwapAdmissionError> {
    policy.validate()?;
    if policy.policy_hash != signed.intent.policy_hash
        || policy.domain != signed.intent.domain.chain
        || policy.nav_epoch != signed.intent.nav_epoch
        || policy.market_envelope_hash != signed.intent.market_envelope_hash
        || policy.pair_asset_0 != signed.intent.party_0.offered_asset_id
        || policy.pair_asset_1 != signed.intent.party_1.offered_asset_id
        || policy.asset_rule_hash_0 != signed.intent.party_0.offered_asset_rule_hash
        || policy.asset_rule_hash_1 != signed.intent.party_1.offered_asset_rule_hash
    {
        return Err(FastSwapAdmissionError::PolicyMismatch);
    }
    if policy.paused {
        return Err(FastSwapAdmissionError::PolicyPaused);
    }
    if finalized_height < policy.valid_from_height
        || finalized_height > policy.valid_through_height
        || finalized_height > signed.intent.expires_at_height
        || signed.intent.expires_at_height > policy.valid_through_height
    {
        return Err(FastSwapAdmissionError::PolicyOutsideHeightWindow);
    }
    if policy.price_denominator == 0 {
        return Err(FastSwapAdmissionError::PolicyRatioInvalid);
    }
    let offered = u128::from(signed.intent.party_0.offered_amount);
    let expected_numerator = offered
        .checked_mul(policy.price_numerator)
        .ok_or(FastSwapAdmissionError::AmountOverflow)?;
    let expected = match policy.rounding {
        FastSwapQuoteRoundingV1::Exact => {
            if expected_numerator % policy.price_denominator != 0 {
                return Err(FastSwapAdmissionError::PolicyRatioInvalid);
            }
            expected_numerator / policy.price_denominator
        }
        FastSwapQuoteRoundingV1::Down => expected_numerator / policy.price_denominator,
    };
    if expected != u128::from(signed.intent.party_1.offered_amount) {
        return Err(FastSwapAdmissionError::PolicyRatioInvalid);
    }
    Ok(())
}

fn validate_reciprocal(signed: &SignedFastSwapIntentV1) -> Result<(), FastSwapAdmissionError> {
    let first = &signed.intent.party_0;
    let second = &signed.intent.party_1;
    if first.receives_asset_id != second.offered_asset_id
        || second.receives_asset_id != first.offered_asset_id
        || first.receives_asset_rule_hash != second.offered_asset_rule_hash
        || second.receives_asset_rule_hash != first.offered_asset_rule_hash
        || first.receives_amount != second.offered_amount
        || second.receives_amount != first.offered_amount
    {
        return Err(FastSwapAdmissionError::NotReciprocal);
    }
    Ok(())
}

fn validate_asset_rules(
    state: &FastLaneStateV1,
    signed: &SignedFastSwapIntentV1,
    finalized_height: u64,
) -> Result<(), FastSwapAdmissionError> {
    for party in [&signed.intent.party_0, &signed.intent.party_1] {
        let rule = state
            .asset_rules
            .get(&party.offered_asset_rule_hash)
            .ok_or(FastSwapAdmissionError::PolicyMismatch)?;
        if rule.asset_id != party.offered_asset_id
            || rule.rule_hash()? != party.offered_asset_rule_hash
            || !rule.fast_lane_enabled
            || finalized_height < rule.valid_from_height
            || finalized_height > rule.valid_through_height
        {
            return Err(FastSwapAdmissionError::PolicyMismatch);
        }
    }
    for recipient in [&signed.intent.party_0, &signed.intent.party_1] {
        let received_rule_hash = recipient.receives_asset_rule_hash;
        let rule = state
            .asset_rules
            .get(&received_rule_hash)
            .ok_or(FastSwapAdmissionError::PolicyMismatch)?;
        match (
            rule.requires_authorization,
            recipient.receives_holder_permit_id,
        ) {
            (false, None) => {}
            (false, Some(_)) => return Err(FastSwapAdmissionError::UnsupportedHolderPermit),
            (true, Some(permit_id)) => {
                let permit = state
                    .holder_permits
                    .get(&permit_id)
                    .ok_or(FastSwapAdmissionError::UnsupportedHolderPermit)?;
                if permit.asset_id != recipient.receives_asset_id
                    || permit.owner_pubkey != recipient.owner_pubkey
                    || finalized_height < permit.valid_from_height
                    || finalized_height > permit.valid_through_height
                {
                    return Err(FastSwapAdmissionError::UnsupportedHolderPermit);
                }
            }
            (true, None) => return Err(FastSwapAdmissionError::UnsupportedHolderPermit),
        }
    }
    Ok(())
}

fn validate_authorization(
    signed: &SignedFastSwapIntentV1,
    role: u8,
) -> Result<(), FastSwapAdmissionError> {
    let (party, authorization) = if role == 0 {
        (&signed.intent.party_0, &signed.authorization_0)
    } else {
        (&signed.intent.party_1, &signed.authorization_1)
    };
    if authorization.role != role
        || authorization.algorithm_id != FASTSWAP_ML_DSA_65
        || authorization.public_key != party.owner_pubkey
    {
        return Err(FastSwapAdmissionError::AuthorizationMalformed(role));
    }
    if address_from_public_key(&authorization.public_key) != party.owner_address {
        return Err(FastSwapAdmissionError::OwnerAddressMismatch(role));
    }
    let bytes = signed.intent.canonical_bytes()?;
    if !ml_dsa_65_verify_with_context(
        &authorization.public_key,
        &bytes,
        &authorization.signature,
        FASTSWAP_INTENT_CONTEXT_V1,
    ) {
        return Err(FastSwapAdmissionError::AuthorizationInvalid(role));
    }
    Ok(())
}

fn validate_party_inputs(
    state: &FastLaneStateV1,
    party: &FastSwapPartyV1,
    swap_id: FastSwapIdV1,
    consumed: &mut BTreeSet<FastObjectKeyV1>,
    input_totals: &mut BTreeMap<FastAssetIdV1, u128>,
) -> Result<(), FastSwapAdmissionError> {
    let mut offered_total = 0_u128;
    for key in &party.asset_inputs {
        let object = validate_input(
            state,
            *key,
            party,
            party.offered_asset_id,
            party.offered_asset_rule_hash,
            swap_id,
            consumed,
        )?;
        offered_total = offered_total
            .checked_add(u128::from(object.amount_atoms))
            .ok_or(FastSwapAdmissionError::AmountOverflow)?;
        add_total(input_totals, object.asset_id, object.amount_atoms)?;
    }
    if offered_total
        != u128::from(party.offered_amount)
            .checked_add(u128::from(party.asset_change))
            .ok_or(FastSwapAdmissionError::AmountOverflow)?
    {
        return Err(FastSwapAdmissionError::AssetConservation);
    }

    let native = FastAssetIdV1::native_pft();
    let mut fee_total = 0_u128;
    for key in &party.fee_inputs {
        let object = validate_input(
            state,
            *key,
            party,
            native,
            postfiat_types::FastAssetRuleHashV1::ZERO,
            swap_id,
            consumed,
        )?;
        fee_total = fee_total
            .checked_add(u128::from(object.amount_atoms))
            .ok_or(FastSwapAdmissionError::AmountOverflow)?;
        add_total(input_totals, object.asset_id, object.amount_atoms)?;
    }
    if fee_total
        != u128::from(party.fee_burn_pft)
            .checked_add(u128::from(party.fee_change))
            .ok_or(FastSwapAdmissionError::AmountOverflow)?
    {
        return Err(FastSwapAdmissionError::FeeConservation);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_input<'state>(
    state: &'state FastLaneStateV1,
    key: FastObjectKeyV1,
    party: &FastSwapPartyV1,
    expected_asset: FastAssetIdV1,
    expected_rule: postfiat_types::FastAssetRuleHashV1,
    swap_id: FastSwapIdV1,
    consumed: &mut BTreeSet<FastObjectKeyV1>,
) -> Result<&'state FastAssetObjectV1, FastSwapAdmissionError> {
    if !consumed.insert(key) {
        return Err(FastSwapAdmissionError::DuplicateInput(key));
    }
    let object = state
        .objects
        .get(&key)
        .ok_or(FastSwapAdmissionError::InputMissing(key))?;
    if object.control_state != FastAssetControlStateV1::Spendable {
        return Err(FastSwapAdmissionError::InputFrozen(key));
    }
    if object.owner_pubkey != party.owner_pubkey {
        return Err(FastSwapAdmissionError::InputOwnerMismatch(key));
    }
    if object.asset_id != expected_asset || object.asset_rule_hash != expected_rule {
        return Err(FastSwapAdmissionError::InputAssetMismatch(key));
    }
    if let Some(reservation) = state.reservations.get(&key) {
        if reservation.swap_id != swap_id {
            return Err(FastSwapAdmissionError::InputReserved(key));
        }
    }
    Ok(object)
}

fn create_party_outputs(
    swap_id: FastSwapIdV1,
    offered_by: &FastSwapPartyV1,
    recipient: &FastSwapPartyV1,
    output_index: &mut u16,
    created: &mut Vec<FastAssetObjectV1>,
) -> Result<(), FastSwapAdmissionError> {
    push_output(
        swap_id,
        recipient.owner_pubkey.clone(),
        offered_by.offered_asset_id,
        offered_by.offered_asset_rule_hash,
        offered_by.offered_amount,
        false,
        output_index,
        created,
    )?;
    if offered_by.asset_change > 0 {
        push_output(
            swap_id,
            offered_by.owner_pubkey.clone(),
            offered_by.offered_asset_id,
            offered_by.offered_asset_rule_hash,
            offered_by.asset_change,
            true,
            output_index,
            created,
        )?;
    }
    if offered_by.fee_change > 0 {
        push_output(
            swap_id,
            offered_by.owner_pubkey.clone(),
            FastAssetIdV1::native_pft(),
            postfiat_types::FastAssetRuleHashV1::ZERO,
            offered_by.fee_change,
            true,
            output_index,
            created,
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn push_output(
    swap_id: FastSwapIdV1,
    owner_pubkey: Vec<u8>,
    asset_id: FastAssetIdV1,
    asset_rule_hash: postfiat_types::FastAssetRuleHashV1,
    amount_atoms: u64,
    is_change: bool,
    output_index: &mut u16,
    created: &mut Vec<FastAssetObjectV1>,
) -> Result<(), FastSwapAdmissionError> {
    if amount_atoms == 0 {
        return Err(FastSwapAdmissionError::AssetConservation);
    }
    let index = *output_index;
    *output_index = output_index
        .checked_add(1)
        .ok_or(FastSwapAdmissionError::OutputLimit)?;
    let mut preimage = Vec::with_capacity(48 + 2 + owner_pubkey.len() + 48 + 48 + 8 + 1);
    preimage.extend_from_slice(&swap_id.0);
    preimage.extend_from_slice(&index.to_be_bytes());
    preimage.extend_from_slice(&(owner_pubkey.len() as u32).to_be_bytes());
    preimage.extend_from_slice(&owner_pubkey);
    preimage.extend_from_slice(&asset_id.0);
    preimage.extend_from_slice(&asset_rule_hash.0);
    preimage.extend_from_slice(&amount_atoms.to_be_bytes());
    preimage.push(u8::from(is_change));
    let digest = hash_bytes("postfiat.fastswap.output.v1", &preimage);
    let object_id: [u8; 32] = digest[..32]
        .try_into()
        .map_err(|_| FastSwapAdmissionError::AmountOverflow)?;
    created.push(FastAssetObjectV1 {
        key: FastObjectKeyV1 {
            object_id: FastObjectIdV1(object_id),
            version: 1,
        },
        owner_pubkey,
        asset_id,
        asset_rule_hash,
        amount_atoms,
        control_state: FastAssetControlStateV1::Spendable,
        origin: if is_change {
            FastObjectOriginV1::Change {
                operation_id: swap_id,
                output_index: index,
            }
        } else {
            FastObjectOriginV1::FastSwapOutput {
                swap_id,
                output_index: index,
            }
        },
    });
    Ok(())
}

fn add_total(
    totals: &mut BTreeMap<FastAssetIdV1, u128>,
    asset: FastAssetIdV1,
    amount: u64,
) -> Result<(), FastSwapAdmissionError> {
    let value = totals.entry(asset).or_default();
    *value = value
        .checked_add(u128::from(amount))
        .ok_or(FastSwapAdmissionError::AmountOverflow)?;
    Ok(())
}

fn verify_conservation(
    inputs: &BTreeMap<FastAssetIdV1, u128>,
    created: &[FastAssetObjectV1],
    burns: &[FastAssetAmountV1],
) -> Result<(), FastSwapAdmissionError> {
    let mut outputs = BTreeMap::<FastAssetIdV1, u128>::new();
    for object in created {
        add_total(&mut outputs, object.asset_id, object.amount_atoms)?;
    }
    for burn in burns {
        add_total(&mut outputs, burn.asset_id, burn.amount_atoms)?;
    }
    if inputs != &outputs {
        return Err(FastSwapAdmissionError::AssetConservation);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use postfiat_crypto_provider::{ml_dsa_65_keygen_from_seed, ml_dsa_65_sign_with_context};
    use postfiat_types::{
        FastAssetDefinitionHashV1, FastAssetRuleHashV1, FastAssetRuleV1, FastHolderPermitIdV1,
        FastHolderPermitV1, FastSwapAuthorizationV1, FastSwapChainDomainV1,
        FastSwapCommitteeDomainV1, FastSwapCommitteeRootV1, FastSwapMarketEnvelopeHashV1,
        FastSwapOpaqueHashV1, FastSwapPolicyHashV1, FastSwapReservationV1, FastSwapRfqHashV1,
        FASTSWAP_SCHEMA_VERSION_V1,
    };

    struct Fixture {
        state: FastLaneStateV1,
        signed: SignedFastSwapIntentV1,
    }

    fn object(
        id: u8,
        owner: &[u8],
        asset: FastAssetIdV1,
        rule: FastAssetRuleHashV1,
        amount: u64,
    ) -> FastAssetObjectV1 {
        FastAssetObjectV1 {
            key: FastObjectKeyV1 {
                object_id: FastObjectIdV1([id; 32]),
                version: 1,
            },
            owner_pubkey: owner.to_vec(),
            asset_id: asset,
            asset_rule_hash: rule,
            amount_atoms: amount,
            control_state: FastAssetControlStateV1::Spendable,
            origin: FastObjectOriginV1::Deposit {
                deposit_id: postfiat_types::FastSwapDepositIdV1([id; 48]),
            },
        }
    }

    fn fixture() -> Fixture {
        let first_key = ml_dsa_65_keygen_from_seed(&[1; 32]);
        let second_key = ml_dsa_65_keygen_from_seed(&[2; 32]);
        let asset_0 = FastAssetIdV1([1; 48]);
        let asset_1 = FastAssetIdV1([2; 48]);
        let asset_rule_0 = FastAssetRuleV1 {
            asset_id: asset_0,
            asset_definition_hash: FastAssetDefinitionHashV1([11; 48]),
            issuer_address: "issuer-0".to_owned(),
            issuer_control_pubkey: vec![21; 64],
            requires_authorization: false,
            freeze_enabled: false,
            clawback_enabled: false,
            fast_lane_enabled: true,
            valid_from_height: 100,
            valid_through_height: 120,
        };
        let asset_rule_1 = FastAssetRuleV1 {
            asset_id: asset_1,
            asset_definition_hash: FastAssetDefinitionHashV1([12; 48]),
            issuer_address: "issuer-1".to_owned(),
            issuer_control_pubkey: vec![22; 64],
            requires_authorization: false,
            freeze_enabled: false,
            clawback_enabled: false,
            fast_lane_enabled: true,
            valid_from_height: 100,
            valid_through_height: 120,
        };
        let rule_0 = asset_rule_0.rule_hash().expect("rule 0");
        let rule_1 = asset_rule_1.rule_hash().expect("rule 1");
        let native = FastAssetIdV1::native_pft();
        let domain = FastSwapCommitteeDomainV1 {
            chain: FastSwapChainDomainV1 {
                chain_id: "postfiat-test".to_owned(),
                genesis_hash: FastSwapOpaqueHashV1([3; 48]),
                protocol_version: 1,
            },
            fastswap_schema_version: FASTSWAP_SCHEMA_VERSION_V1,
            committee_epoch: 1,
            committee_root: FastSwapCommitteeRootV1([4; 48]),
            validator_count: 6,
            quorum: 5,
        };
        let envelope = FastSwapMarketEnvelopeHashV1([6; 48]);
        let mut policy = FastSwapPolicySnapshotV1 {
            domain: domain.chain.clone(),
            policy_epoch: 1,
            policy_hash: FastSwapPolicyHashV1::ZERO,
            pair_asset_0: asset_0,
            pair_asset_1: asset_1,
            asset_rule_hash_0: rule_0,
            asset_rule_hash_1: rule_1,
            price_numerator: 1,
            price_denominator: 8,
            rounding: FastSwapQuoteRoundingV1::Exact,
            nav_epoch: 59,
            market_envelope_hash: envelope,
            valid_from_height: 100,
            valid_through_height: 120,
            fee_schedule_hash: FastSwapOpaqueHashV1([9; 48]),
            max_inputs_per_party: 16,
            max_outputs: 8,
            paused: false,
        };
        let policy_hash = policy.computed_hash().expect("policy hash");
        policy.policy_hash = policy_hash;
        let first_asset = object(1, &first_key.public_key, asset_0, rule_0, 10);
        let first_fee = object(
            2,
            &first_key.public_key,
            native,
            FastAssetRuleHashV1::ZERO,
            10,
        );
        let second_asset = object(3, &second_key.public_key, asset_1, rule_1, 3);
        let second_fee = object(
            4,
            &second_key.public_key,
            native,
            FastAssetRuleHashV1::ZERO,
            10,
        );
        let party_0 = FastSwapPartyV1 {
            owner_address: address_from_public_key(&first_key.public_key),
            owner_pubkey: first_key.public_key.clone(),
            offered_asset_id: asset_0,
            offered_asset_rule_hash: rule_0,
            offered_amount: 8,
            receives_asset_id: asset_1,
            receives_asset_rule_hash: rule_1,
            receives_holder_permit_id: None,
            receives_amount: 1,
            asset_inputs: vec![first_asset.key],
            fee_inputs: vec![first_fee.key],
            asset_change: 2,
            fee_change: 9,
            fee_burn_pft: 1,
        };
        let party_1 = FastSwapPartyV1 {
            owner_address: address_from_public_key(&second_key.public_key),
            owner_pubkey: second_key.public_key.clone(),
            offered_asset_id: asset_1,
            offered_asset_rule_hash: rule_1,
            offered_amount: 1,
            receives_asset_id: asset_0,
            receives_asset_rule_hash: rule_0,
            receives_holder_permit_id: None,
            receives_amount: 8,
            asset_inputs: vec![second_asset.key],
            fee_inputs: vec![second_fee.key],
            asset_change: 2,
            fee_change: 9,
            fee_burn_pft: 1,
        };
        let intent = postfiat_types::FastSwapIntentV1 {
            domain: domain.clone(),
            policy_hash,
            rfq_hash: FastSwapRfqHashV1([7; 48]),
            market_envelope_hash: envelope,
            nav_epoch: 59,
            expires_at_height: 120,
            nonce: [8; 32],
            party_0,
            party_1,
        };
        let bytes = intent.canonical_bytes().expect("intent bytes");
        let authorization_0 = FastSwapAuthorizationV1 {
            role: 0,
            algorithm_id: FASTSWAP_ML_DSA_65.to_owned(),
            public_key: first_key.public_key,
            signature: ml_dsa_65_sign_with_context(
                &first_key.private_key,
                &bytes,
                FASTSWAP_INTENT_CONTEXT_V1,
            )
            .expect("first signature"),
        };
        let authorization_1 = FastSwapAuthorizationV1 {
            role: 1,
            algorithm_id: FASTSWAP_ML_DSA_65.to_owned(),
            public_key: second_key.public_key,
            signature: ml_dsa_65_sign_with_context(
                &second_key.private_key,
                &bytes,
                FASTSWAP_INTENT_CONTEXT_V1,
            )
            .expect("second signature"),
        };
        let state = FastLaneStateV1 {
            schema_version: 1,
            committee: domain,
            objects: [first_asset, first_fee, second_asset, second_fee]
                .into_iter()
                .map(|object| (object.key, object))
                .collect(),
            reservations: BTreeMap::new(),
            swaps: BTreeMap::new(),
            imported_deposits: BTreeSet::new(),
            exit_claims: BTreeMap::new(),
            terminal_tombstones: BTreeMap::new(),
            asset_rules: BTreeMap::from([(rule_0, asset_rule_0), (rule_1, asset_rule_1)]),
            holder_permits: BTreeMap::new(),
            policy_snapshots: BTreeMap::from([(policy_hash, policy)]),
            prepare_fences: BTreeMap::new(),
            pending_fee_burns: BTreeMap::new(),
            anchored_checkpoints: BTreeSet::new(),
        };
        Fixture {
            state,
            signed: SignedFastSwapIntentV1 {
                intent,
                authorization_0,
                authorization_1,
            },
        }
    }

    #[test]
    fn valid_dual_auth_swap_produces_deterministic_conserved_effects() {
        let fixture = self::fixture();
        let first = validate_fastswap_admission(&fixture.state, &fixture.signed, 110)
            .expect("valid admission");
        let second = validate_fastswap_admission(&fixture.state, &fixture.signed, 110)
            .expect("deterministic repeat");
        assert_eq!(first, second);
        assert_eq!(first.effects.receipt.code, "fastswap_applied");
        assert!(first.effects.receipt.accepted);
        assert_eq!(first.effects.consumed.len(), 4);
        assert_eq!(first.effects.created.len(), 6);
        assert_eq!(first.effects.fee_burns[0].amount_atoms, 2);
    }

    #[test]
    fn conservation_property_holds_across_ten_thousand_amount_splits() {
        let asset_0 = FastAssetIdV1([41; 48]);
        let asset_1 = FastAssetIdV1([42; 48]);
        let native = FastAssetIdV1::native_pft();
        let rule_0 = FastAssetRuleHashV1([51; 48]);
        let rule_1 = FastAssetRuleHashV1([52; 48]);
        let mut state = 0x9e37_79b9_7f4a_7c15_u64;
        for case in 0_u64..10_000 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let offered_0 = state % 1_000_000 + 1;
            let change_0 = state.rotate_left(11) % 1_000_000 + 1;
            let offered_1 = state.rotate_left(23) % 1_000_000 + 1;
            let change_1 = state.rotate_left(37) % 1_000_000 + 1;
            let fee_0 = state.rotate_left(43) % 10_000 + 1;
            let fee_1 = state.rotate_left(53) % 10_000 + 1;
            let fee_change_0 = state.rotate_left(3) % 10_000 + 1;
            let fee_change_1 = state.rotate_left(29) % 10_000 + 1;
            let inputs = BTreeMap::from([
                (asset_0, u128::from(offered_0) + u128::from(change_0)),
                (asset_1, u128::from(offered_1) + u128::from(change_1)),
                (
                    native,
                    u128::from(fee_0)
                        + u128::from(fee_1)
                        + u128::from(fee_change_0)
                        + u128::from(fee_change_1),
                ),
            ]);
            let mut created = vec![
                object(61, &[1], asset_0, rule_0, offered_0),
                object(62, &[2], asset_0, rule_0, change_0),
                object(63, &[3], asset_1, rule_1, offered_1),
                object(64, &[4], asset_1, rule_1, change_1),
                object(65, &[5], native, FastAssetRuleHashV1::ZERO, fee_change_0),
                object(66, &[6], native, FastAssetRuleHashV1::ZERO, fee_change_1),
            ];
            let burns = vec![FastAssetAmountV1 {
                asset_id: native,
                amount_atoms: fee_0 + fee_1,
            }];
            verify_conservation(&inputs, &created, &burns).expect("conserved split");
            created[usize::try_from(case % 6).expect("small index")].amount_atoms += 1;
            assert_eq!(
                verify_conservation(&inputs, &created, &burns),
                Err(FastSwapAdmissionError::AssetConservation)
            );
        }
    }

    #[test]
    fn second_leg_signature_substitution_fails_before_effects() {
        let mut fixture = fixture();
        fixture.signed.authorization_1.signature[0] ^= 1;
        assert_eq!(
            validate_fastswap_admission(&fixture.state, &fixture.signed, 110),
            Err(FastSwapAdmissionError::AuthorizationInvalid(1))
        );
    }

    #[test]
    fn one_reserved_input_rejects_the_complete_swap_without_mutation() {
        let mut fixture = fixture();
        let before = fixture.state.clone();
        let prepared = validate_fastswap_admission(&fixture.state, &fixture.signed, 110)
            .expect("initial admission");
        let key = fixture.signed.intent.party_0.asset_inputs[0];
        fixture.state.reservations.insert(
            key,
            FastSwapReservationV1 {
                swap_id: FastSwapIdV1([99; 48]),
                intent_id: prepared.intent_id,
                effects_digest: prepared.effects.digest().expect("digest"),
            },
        );
        assert_eq!(
            validate_fastswap_admission(&fixture.state, &fixture.signed, 110),
            Err(FastSwapAdmissionError::InputReserved(key))
        );
        assert_eq!(fixture.state.objects, before.objects);
    }

    #[test]
    fn quote_ratio_and_expiry_fail_closed() {
        let mut mispriced = fixture();
        mispriced
            .state
            .policy_snapshots
            .get_mut(&mispriced.signed.intent.policy_hash)
            .expect("policy")
            .price_numerator = 2;
        assert_eq!(
            validate_fastswap_admission(&mispriced.state, &mispriced.signed, 110),
            Err(FastSwapAdmissionError::Codec(
                FastSwapCodecError::NonCanonical("policy snapshot")
            ))
        );
        let fixture = fixture();
        assert_eq!(
            validate_fastswap_admission(&fixture.state, &fixture.signed, 121),
            Err(FastSwapAdmissionError::PolicyOutsideHeightWindow)
        );
    }

    #[test]
    fn authorization_required_asset_accepts_only_exact_live_holder_permit() {
        let mut fixture = fixture();
        let old_hash = fixture.signed.intent.party_1.offered_asset_rule_hash;
        let mut rule = fixture
            .state
            .asset_rules
            .remove(&old_hash)
            .expect("asset 1 rule");
        rule.requires_authorization = true;
        let new_hash = rule.rule_hash().expect("authorized rule hash");
        fixture.state.asset_rules.insert(new_hash, rule);
        fixture.signed.intent.party_1.offered_asset_rule_hash = new_hash;
        fixture.signed.intent.party_0.receives_asset_rule_hash = new_hash;

        assert_eq!(
            validate_asset_rules(&fixture.state, &fixture.signed, 110),
            Err(FastSwapAdmissionError::UnsupportedHolderPermit)
        );

        let mut permit = FastHolderPermitV1 {
            permit_id: FastHolderPermitIdV1::ZERO,
            asset_id: fixture.signed.intent.party_0.receives_asset_id,
            owner_pubkey: fixture.signed.intent.party_0.owner_pubkey.clone(),
            valid_from_height: 100,
            valid_through_height: 120,
            consensus_receipt_digest: FastSwapOpaqueHashV1([91; 48]),
        };
        permit.permit_id = permit.computed_id().expect("permit id");
        fixture.signed.intent.party_0.receives_holder_permit_id = Some(permit.permit_id);
        fixture
            .state
            .holder_permits
            .insert(permit.permit_id, permit.clone());
        validate_asset_rules(&fixture.state, &fixture.signed, 110).expect("live exact permit");

        fixture
            .state
            .holder_permits
            .get_mut(&permit.permit_id)
            .expect("permit")
            .valid_through_height = 109;
        assert_eq!(
            validate_asset_rules(&fixture.state, &fixture.signed, 110),
            Err(FastSwapAdmissionError::UnsupportedHolderPermit)
        );
    }
}
