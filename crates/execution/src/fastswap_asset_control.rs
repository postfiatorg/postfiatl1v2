use crate::fastswap::PreparedFastSwapV1;
use postfiat_crypto_provider::{address_from_public_key, ml_dsa_65_verify_with_context};
use postfiat_types::{
    FastAssetControlActionV1, FastAssetControlStateV1, FastLaneStateV1, FastObjectOriginV1,
    FastSwapControlCertificateIdV1, FastSwapDecisionV1, FastSwapEffectsV1, FastSwapReceiptV1,
    SignedFastAssetControlCommandV1, FASTLANE_ASSET_CONTROL_CONTEXT_V1, FASTSWAP_ML_DSA_65,
};

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum FastAssetControlError {
    Codec,
    DomainMismatch,
    Expired,
    InvalidAuthorization,
    InputMissing,
    InputReserved,
    RuleMissing,
    RuleMismatch,
    UnsupportedAction,
    InvalidControlState,
    VersionOverflow,
    OutputCollision,
}

/// Validate one issuer command without mutation and deterministically derive
/// its one-input/one-output effects. The returned consumed key is reserved by
/// the same WAL record and lock table used by owner swaps and exits.
pub fn validate_fast_asset_control(
    state: &FastLaneStateV1,
    signed: &SignedFastAssetControlCommandV1,
    finalized_height: u64,
) -> Result<PreparedFastSwapV1, FastAssetControlError> {
    let bytes = signed
        .command
        .canonical_bytes()
        .map_err(|_| FastAssetControlError::Codec)?;
    if signed.command.domain != state.committee {
        return Err(FastAssetControlError::DomainMismatch);
    }
    if finalized_height > signed.command.expires_at_height {
        return Err(FastAssetControlError::Expired);
    }
    if signed.algorithm_id != FASTSWAP_ML_DSA_65
        || !ml_dsa_65_verify_with_context(
            &signed.command.issuer_control_pubkey,
            &bytes,
            &signed.signature,
            FASTLANE_ASSET_CONTROL_CONTEXT_V1,
        )
        || address_from_public_key(&signed.command.issuer_control_pubkey)
            != signed.command.issuer_address
    {
        return Err(FastAssetControlError::InvalidAuthorization);
    }
    let operation_id = signed
        .command
        .operation_id()
        .map_err(|_| FastAssetControlError::Codec)?;
    let intent_id = signed
        .command
        .intent_id()
        .map_err(|_| FastAssetControlError::Codec)?;
    let input = state
        .objects
        .get(&signed.command.input)
        .ok_or(FastAssetControlError::InputMissing)?;
    if state
        .reservations
        .get(&input.key)
        .is_some_and(|reservation| reservation.swap_id != operation_id)
    {
        return Err(FastAssetControlError::InputReserved);
    }
    let rule = state
        .asset_rules
        .get(&input.asset_rule_hash)
        .ok_or(FastAssetControlError::RuleMissing)?;
    if rule.asset_id != input.asset_id
        || rule.rule_hash().ok() != Some(input.asset_rule_hash)
        || rule.issuer_address != signed.command.issuer_address
        || rule.issuer_control_pubkey != signed.command.issuer_control_pubkey
        || !rule.fast_lane_enabled
        || finalized_height < rule.valid_from_height
        || finalized_height > rule.valid_through_height
    {
        return Err(FastAssetControlError::RuleMismatch);
    }
    let control_state = match signed.command.action {
        FastAssetControlActionV1::Freeze => {
            if !rule.freeze_enabled {
                return Err(FastAssetControlError::UnsupportedAction);
            }
            if input.control_state != FastAssetControlStateV1::Spendable {
                return Err(FastAssetControlError::InvalidControlState);
            }
            FastAssetControlStateV1::Frozen {
                control_certificate_id: FastSwapControlCertificateIdV1(operation_id.0),
            }
        }
        FastAssetControlActionV1::Unfreeze => {
            if !rule.freeze_enabled {
                return Err(FastAssetControlError::UnsupportedAction);
            }
            if !matches!(input.control_state, FastAssetControlStateV1::Frozen { .. }) {
                return Err(FastAssetControlError::InvalidControlState);
            }
            FastAssetControlStateV1::Spendable
        }
        FastAssetControlActionV1::Clawback => {
            if !rule.clawback_enabled {
                return Err(FastAssetControlError::UnsupportedAction);
            }
            FastAssetControlStateV1::Spendable
        }
    };
    let output_key = postfiat_types::FastObjectKeyV1 {
        object_id: input.key.object_id,
        version: input
            .key
            .version
            .checked_add(1)
            .ok_or(FastAssetControlError::VersionOverflow)?,
    };
    if state.objects.contains_key(&output_key) {
        return Err(FastAssetControlError::OutputCollision);
    }
    let output = postfiat_types::FastAssetObjectV1 {
        key: output_key,
        owner_pubkey: if signed.command.action == FastAssetControlActionV1::Clawback {
            signed.command.issuer_control_pubkey.clone()
        } else {
            input.owner_pubkey.clone()
        },
        asset_id: input.asset_id,
        asset_rule_hash: input.asset_rule_hash,
        amount_atoms: input.amount_atoms,
        control_state,
        origin: FastObjectOriginV1::Change {
            operation_id,
            output_index: 0,
        },
    };
    let receipt = FastSwapReceiptV1 {
        swap_id: operation_id,
        accepted: true,
        code: "fastlane_asset_control_applied".to_owned(),
        consumed_count: 1,
        created_count: 1,
    };
    let effects = FastSwapEffectsV1 {
        domain: signed.command.domain.clone(),
        swap_id: operation_id,
        policy_hash: postfiat_types::FastSwapPolicyHashV1::ZERO,
        decision: FastSwapDecisionV1::Confirm,
        consumed: vec![input.key],
        created: vec![output],
        fee_burns: Vec::new(),
        receipt,
    };
    effects.digest().map_err(|_| FastAssetControlError::Codec)?;
    Ok(PreparedFastSwapV1 {
        swap_id: operation_id,
        intent_id,
        effects,
    })
}
