use postfiat_crypto_provider::{hash_bytes, hex_to_bytes};
use postfiat_execution::fastswap_bridge::asset_definition_hash;
use postfiat_types::{
    FastAssetIdV1, FastAssetRuleHashV1, FastAssetRuleV1, FastSwapChainDomainV1,
    FastSwapCommitteeDomainV1, FastSwapCommitteeRootV1, FastSwapCommitteeV1,
    FastSwapGovernanceBootstrapPayloadV1, FastSwapMarketEnvelopeHashV1, FastSwapOpaqueHashV1,
    FastSwapPolicyHashV1, FastSwapPolicySnapshotV1, FastSwapQuoteRoundingV1, FastSwapValidatorV1,
    LedgerState, FASTSWAP_SCHEMA_VERSION_V1,
};
use serde::Deserialize;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
struct ChainTip {
    chain_id: String,
    genesis_hash: String,
    protocol_version: u32,
    height: u64,
}

#[derive(Deserialize)]
struct ValidatorRegistry {
    validators: Vec<ValidatorRecord>,
}

#[derive(Deserialize)]
struct ValidatorRecord {
    node_id: String,
    algorithm_id: String,
    public_key_hex: String,
}

struct Options {
    ledger: PathBuf,
    registry: PathBuf,
    tip: PathBuf,
    activation_height: u64,
    valid_through_height: u64,
    asset_0_id: String,
    asset_1_id: String,
    price_numerator: u128,
    price_denominator: u128,
    nav_epoch: u64,
    market_envelope_hash: String,
    output: PathBuf,
}

fn required_flag(args: &[String], flag: &str) -> Result<String, String> {
    let index = args
        .iter()
        .position(|value| value == flag)
        .ok_or_else(|| format!("missing {flag}"))?;
    args.get(index + 1)
        .cloned()
        .ok_or_else(|| format!("missing value for {flag}"))
}

fn parse_options() -> Result<Options, String> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    Ok(Options {
        ledger: PathBuf::from(required_flag(&args, "--ledger")?),
        registry: PathBuf::from(required_flag(&args, "--registry")?),
        tip: PathBuf::from(required_flag(&args, "--tip")?),
        activation_height: required_flag(&args, "--activation-height")?
            .parse()
            .map_err(|_| "--activation-height must be a u64".to_owned())?,
        valid_through_height: required_flag(&args, "--valid-through-height")?
            .parse()
            .map_err(|_| "--valid-through-height must be a u64".to_owned())?,
        asset_0_id: required_flag(&args, "--asset-0-id")?,
        asset_1_id: required_flag(&args, "--asset-1-id")?,
        price_numerator: required_flag(&args, "--price-numerator")?
            .parse()
            .map_err(|_| "--price-numerator must be a u128".to_owned())?,
        price_denominator: required_flag(&args, "--price-denominator")?
            .parse()
            .map_err(|_| "--price-denominator must be a u128".to_owned())?,
        nav_epoch: required_flag(&args, "--nav-epoch")?
            .parse()
            .map_err(|_| "--nav-epoch must be a u64".to_owned())?,
        market_envelope_hash: required_flag(&args, "--market-envelope-hash")?,
        output: PathBuf::from(required_flag(&args, "--output")?),
    })
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, String> {
    let bytes = fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|error| format!("{}: {error}", path.display()))
}

fn fixed48(value: &str, label: &str) -> Result<[u8; 48], String> {
    hex_to_bytes(value)
        .map_err(|error| format!("invalid {label}: {error}"))?
        .try_into()
        .map_err(|_| format!("{label} must be 48 bytes"))
}

fn validate_height_window(
    tip_height: u64,
    activation_height: u64,
    valid_through_height: u64,
) -> Result<(), String> {
    let first_valid_activation = tip_height
        .checked_add(1)
        .ok_or_else(|| "chain tip height is exhausted; no activation window remains".to_owned())?;
    if activation_height <= first_valid_activation || valid_through_height <= activation_height {
        return Err("activation must leave one governance-commit height and precede expiry".into());
    }
    Ok(())
}

fn main() -> Result<(), String> {
    let options = parse_options()?;
    let ledger: LedgerState = read_json(&options.ledger)?;
    let mut registry: ValidatorRegistry = read_json(&options.registry)?;
    let tip: ChainTip = read_json(&options.tip)?;
    validate_height_window(
        tip.height,
        options.activation_height,
        options.valid_through_height,
    )?;

    registry
        .validators
        .sort_by(|left, right| left.node_id.cmp(&right.node_id));
    if registry.validators.len() != 6
        || registry
            .validators
            .iter()
            .any(|record| record.algorithm_id != "ML-DSA-65")
        || !registry
            .validators
            .windows(2)
            .all(|pair| pair[0].node_id < pair[1].node_id)
    {
        return Err("canonical six-member ML-DSA-65 validator registry required".into());
    }
    let chain = FastSwapChainDomainV1 {
        chain_id: tip.chain_id,
        genesis_hash: FastSwapOpaqueHashV1(fixed48(&tip.genesis_hash, "genesis hash")?),
        protocol_version: tip.protocol_version,
    };
    let validators = registry
        .validators
        .into_iter()
        .map(|record| {
            Ok(FastSwapValidatorV1 {
                validator_id: record.node_id,
                public_key: hex_to_bytes(&record.public_key_hex)
                    .map_err(|error| format!("invalid validator public key: {error}"))?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let mut committee = FastSwapCommitteeV1 {
        domain: FastSwapCommitteeDomainV1 {
            chain: chain.clone(),
            fastswap_schema_version: FASTSWAP_SCHEMA_VERSION_V1,
            committee_epoch: 1,
            committee_root: FastSwapCommitteeRootV1::ZERO,
            validator_count: 6,
            quorum: 5,
        },
        validators,
    };
    committee.domain.committee_root = committee
        .computed_root()
        .map_err(|error| format!("committee root: {error:?}"))?;
    committee
        .validate()
        .map_err(|error| format!("committee: {error:?}"))?;

    let mut rules =
        [options.asset_0_id.as_str(), options.asset_1_id.as_str()]
            .into_iter()
            .map(|asset_id_hex| {
                let definition = ledger
                    .asset_definitions
                    .iter()
                    .find(|definition| definition.asset_id == asset_id_hex)
                    .ok_or_else(|| format!("missing exact asset definition {asset_id_hex}"))?;
                let issuer = ledger
                    .accounts
                    .iter()
                    .find(|account| account.address == definition.issuer)
                    .ok_or_else(|| format!("missing issuer account {}", definition.issuer))?;
                let issuer_control_pubkey =
                    hex_to_bytes(issuer.public_key_hex.as_deref().ok_or_else(|| {
                        format!("issuer {} has no public key", definition.issuer)
                    })?)
                    .map_err(|error| format!("invalid issuer public key: {error}"))?;
                Ok(FastAssetRuleV1 {
                    asset_id: FastAssetIdV1(fixed48(asset_id_hex, "asset id")?),
                    asset_definition_hash: asset_definition_hash(definition)
                        .map_err(|error| format!("asset definition hash: {error:?}"))?,
                    issuer_address: definition.issuer.clone(),
                    issuer_control_pubkey,
                    requires_authorization: definition.requires_authorization,
                    freeze_enabled: definition.freeze_enabled,
                    clawback_enabled: definition.clawback_enabled,
                    fast_lane_enabled: true,
                    valid_from_height: options.activation_height,
                    valid_through_height: options.valid_through_height,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
    rules.sort_by_key(|rule| rule.rule_hash().expect("validated asset rule"));

    let asset_0_id = FastAssetIdV1(fixed48(&options.asset_0_id, "asset 0 id")?);
    let asset_1_id = FastAssetIdV1(fixed48(&options.asset_1_id, "asset 1 id")?);
    let rule_hash = |asset_id: FastAssetIdV1| -> Result<FastAssetRuleHashV1, String> {
        rules
            .iter()
            .find(|rule| rule.asset_id == asset_id)
            .ok_or_else(|| "missing rule after canonical sort".to_owned())?
            .rule_hash()
            .map_err(|error| format!("rule hash: {error:?}"))
    };
    let fee_schedule_hash: [u8; 48] = hash_bytes(
        "postfiat.fastswap.fee_schedule.v1",
        b"party_0_fee_pft=1\nparty_1_fee_pft=1\n",
    )
    .try_into()
    .map_err(|_| "fee schedule hash must be 48 bytes".to_owned())?;
    let mut policy = FastSwapPolicySnapshotV1 {
        domain: chain,
        policy_epoch: 1,
        policy_hash: FastSwapPolicyHashV1::ZERO,
        pair_asset_0: asset_0_id,
        pair_asset_1: asset_1_id,
        asset_rule_hash_0: rule_hash(asset_0_id)?,
        asset_rule_hash_1: rule_hash(asset_1_id)?,
        price_numerator: options.price_numerator,
        price_denominator: options.price_denominator,
        rounding: FastSwapQuoteRoundingV1::Down,
        nav_epoch: options.nav_epoch,
        market_envelope_hash: FastSwapMarketEnvelopeHashV1(fixed48(
            &options.market_envelope_hash,
            "market envelope hash",
        )?),
        valid_from_height: options.activation_height,
        valid_through_height: options.valid_through_height,
        fee_schedule_hash: FastSwapOpaqueHashV1(fee_schedule_hash),
        max_inputs_per_party: 16,
        max_outputs: 8,
        paused: false,
    };
    policy.policy_hash = policy
        .computed_hash()
        .map_err(|error| format!("policy hash: {error:?}"))?;
    policy
        .validate()
        .map_err(|error| format!("policy: {error:?}"))?;

    let payload = FastSwapGovernanceBootstrapPayloadV1 {
        committee,
        asset_rules: rules,
        policies: vec![policy],
        activation_height: options.activation_height,
    };
    payload
        .validate_payload()
        .map_err(|error| format!("bootstrap payload: {error:?}"))?;
    let encoded = serde_json::to_vec_pretty(&payload)
        .map_err(|error| format!("serialize bootstrap payload: {error}"))?;
    fs::write(&options.output, encoded)
        .map_err(|error| format!("{}: {error}", options.output.display()))?;
    println!(
        "wrote {}: tip={} activation={} expiry={} committee=6 quorum=5 rules=2 policies=1",
        options.output.display(),
        tip.height,
        options.activation_height,
        options.valid_through_height
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exhausted_tip_height_is_rejected_without_panicking() {
        assert!(validate_height_window(u64::MAX, u64::MAX, u64::MAX).is_err());
    }
}
