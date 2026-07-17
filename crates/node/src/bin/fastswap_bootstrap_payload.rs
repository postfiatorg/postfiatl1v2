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

const PFUSDC_ASSET_ID: &str =
    "34ce77d07099872d5691ead3842bfb3d6cc8678ff62cc68d887dad7f8645128351e72b9ae76f88ed1854a5e8d3372c8b";
const A651_ASSET_ID: &str =
    "8584aa713209eb8253293c891f7269e35841f004080e06414db019f868610e9cb57dfb7aca3d51427fbe369b6ebde127";
const MARKET_ENVELOPE_HASH: &str =
    "c281435501ebcc921eacd263f588ae5f24ba7225929b850f36980e4544d88b3618a1396674872812ead659376dbadec7";

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
        [PFUSDC_ASSET_ID, A651_ASSET_ID]
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

    let pfusdc_id = FastAssetIdV1(fixed48(PFUSDC_ASSET_ID, "pfUSDC asset id")?);
    let a651_id = FastAssetIdV1(fixed48(A651_ASSET_ID, "a651 asset id")?);
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
        pair_asset_0: pfusdc_id,
        pair_asset_1: a651_id,
        asset_rule_hash_0: rule_hash(pfusdc_id)?,
        asset_rule_hash_1: rule_hash(a651_id)?,
        // Exact certified epoch-59 NAV is 820102177 pfUSDC atoms per 1e8 a651 atoms.
        // With pfUSDC as party-0, this reciprocal computes party-1 a651 atoms.
        price_numerator: 100_000_000,
        price_denominator: 820_102_177,
        rounding: FastSwapQuoteRoundingV1::Down,
        nav_epoch: 59,
        market_envelope_hash: FastSwapMarketEnvelopeHashV1(fixed48(
            MARKET_ENVELOPE_HASH,
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
