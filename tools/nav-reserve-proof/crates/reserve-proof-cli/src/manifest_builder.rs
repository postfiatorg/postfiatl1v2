use std::{collections::BTreeMap, collections::BTreeSet, path::PathBuf};

use alloy_primitives::{Address, B256};
use anyhow::{anyhow, bail, Context, Result};
use reserve_proof_types::{
    aave_v3::{aave_v3_owner_commitment, AaveV3PolicyV1, AAVE_V3_ADAPTER_KIND_V1},
    bft_checkpoint::BftCheckpointCommitteeV1,
    evm_chainlink_valuation::EvmChainlinkValuationPolicyV1,
    evm_spot::{evm_spot_owner_commitment, EvmSpotPolicyV1, EVM_SPOT_ADAPTER_KIND_V1},
    hyperliquid_receipt::{
        hyperliquid_owner_commitment, HyperliquidReceiptPolicyV1,
        HYPERLIQUID_RECEIPT_ADAPTER_KIND_V1,
    },
    monero_reserve::{
        monero_reserve_owner_commitment, MoneroReservePolicyV1, MONERO_RESERVE_ADAPTER_KIND_V1,
    },
    near_receipt::{NearReceiptPolicyV1, NEAR_RECEIPT_QUANTITY_ADAPTER_KIND_V1},
    portfolio_valuation::{PortfolioValuationMethodV1, PortfolioValuationPolicyV1},
    solana_stake::{
        solana_stake_owner_commitment, SolanaStakeReaderPolicyV1,
        SOLANA_STAKE_READER_ADAPTER_KIND_V1,
    },
    FreshnessPolicyV1, LiabilityTreatmentV1, SourceManifestEntryV1, SourceManifestV1, TrustClassV1,
    MANIFEST_SCHEMA_V1,
};
use serde::{Deserialize, Serialize};

use crate::{read_json, write_new};

const BUILD_SCHEMA_V1: &str = "postfiat.reserve_manifest_build.v1";
const MAX_MANIFEST_BUILD_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestBuildV1 {
    schema: String,
    valuation_policy: PortfolioValuationPolicyV1,
    sources: Vec<ManifestSourceBuildV1>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestSourceBuildV1 {
    source_id: String,
    asset_or_position_id: String,
    reserve_owner: ReserveOwnerBuildV1,
    quantity_verifier: QuantityVerifierBuildV1,
    valuation_verifier: ValuationVerifierBuildV1,
    freshness_policy: FreshnessPolicyV1,
    liability_treatment: LiabilityTreatmentV1,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum ReserveOwnerBuildV1 {
    EvmAddress {
        address: Address,
    },
    NearAccount {
        account_id: String,
    },
    SolanaWallet {
        wallet_pubkey: [u8; 32],
    },
    MoneroAddress {
        spend_public_key: B256,
        view_public_key: B256,
    },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum QuantityVerifierBuildV1 {
    AaveV3 {
        policy: AaveV3PolicyV1,
        committee: BftCheckpointCommitteeV1,
    },
    EvmSpot {
        policy: EvmSpotPolicyV1,
        committees: Vec<BftCheckpointCommitteeV1>,
    },
    HyperliquidReceipt {
        policy: HyperliquidReceiptPolicyV1,
        committee: BftCheckpointCommitteeV1,
    },
    NearReceipt {
        policy: NearReceiptPolicyV1,
        committee: BftCheckpointCommitteeV1,
    },
    SolanaStakeReader {
        policy: SolanaStakeReaderPolicyV1,
        committee: BftCheckpointCommitteeV1,
    },
    MoneroReserve {
        policy: MoneroReservePolicyV1,
        committee: BftCheckpointCommitteeV1,
    },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum ValuationVerifierBuildV1 {
    /// Aave and Hyperliquid derive valuation from the same cryptographic proof
    /// as quantity. No separate operator-entered price is accepted.
    SameAsQuantity,
    EvmChainlink {
        policy: Box<EvmChainlinkValuationPolicyV1>,
        committee: Box<BftCheckpointCommitteeV1>,
    },
}

#[derive(Debug)]
struct QuantityBindings {
    adapter_kind: &'static str,
    source_domain: String,
    reserve_owner_commitment: String,
    verifier_commitment: String,
    supports_integrated_valuation: bool,
    valuation_rows: Option<BTreeMap<String, u8>>,
}

#[derive(Debug, Serialize)]
struct ManifestBuildReportV1 {
    schema: &'static str,
    source_count: usize,
    source_manifest_hash: String,
    valuation_policy_hash: String,
    valuation_unit_id: String,
    valuation_scale: u64,
    output: PathBuf,
}

#[derive(Debug, Serialize)]
struct ValuationPolicyHashReportV1 {
    schema: &'static str,
    valuation_policy_hash: String,
    nav_asset_id: String,
    valuation_unit_id: String,
    valuation_scale: u64,
    source_count: usize,
}

pub fn run_valuation_policy_hash(input: PathBuf, output: Option<PathBuf>) -> Result<()> {
    let policy: PortfolioValuationPolicyV1 = read_json(&input)?;
    let report = ValuationPolicyHashReportV1 {
        schema: "postfiat.reserve_portfolio_valuation_policy_hash_report.v1",
        valuation_policy_hash: policy.hash().map_err(anyhow::Error::msg)?,
        nav_asset_id: policy.nav_asset_id,
        valuation_unit_id: policy.valuation_unit_id,
        valuation_scale: policy.valuation_scale,
        source_count: policy.sources.len(),
    };
    let json = serde_json::to_string_pretty(&report)?;
    if let Some(output) = output {
        write_new(&output, format!("{json}\n").as_bytes())?;
    }
    println!("{json}");
    Ok(())
}

pub fn run(input: PathBuf, output: PathBuf) -> Result<()> {
    let build: ManifestBuildV1 = read_json(&input)?;
    let valuation_policy_hash = build.valuation_policy.hash().map_err(anyhow::Error::msg)?;
    let valuation_unit_id = build.valuation_policy.valuation_unit_id.clone();
    let valuation_scale = build.valuation_policy.valuation_scale;
    let manifest = build_manifest(build)?;
    let source_manifest_hash = manifest.hash().map_err(anyhow::Error::msg)?;
    write_new(&output, &serde_json::to_vec_pretty(&manifest)?)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&ManifestBuildReportV1 {
            schema: "postfiat.reserve_manifest_build_report.v1",
            source_count: manifest.sources.len(),
            source_manifest_hash,
            valuation_policy_hash,
            valuation_unit_id,
            valuation_scale,
            output,
        })?
    );
    Ok(())
}

pub(crate) fn fuzz_external_input(data: &[u8]) {
    if data.len() <= MAX_MANIFEST_BUILD_BYTES {
        if let Ok(build) = serde_json::from_slice::<ManifestBuildV1>(data) {
            let _ = build_manifest(build);
        }
    }
}

fn build_manifest(build: ManifestBuildV1) -> Result<SourceManifestV1> {
    if build.schema != BUILD_SCHEMA_V1 {
        bail!("reserve manifest build schema mismatch");
    }
    build
        .valuation_policy
        .validate()
        .map_err(anyhow::Error::msg)?;
    let valuation_policy_hash = build.valuation_policy.hash().map_err(anyhow::Error::msg)?;
    if build.sources.len() != build.valuation_policy.sources.len() {
        bail!("manifest sources do not exactly match valuation policy sources");
    }
    let valuation_sources = build
        .valuation_policy
        .sources
        .iter()
        .map(|source| (source.source_id.as_str(), source))
        .collect::<BTreeMap<_, _>>();
    let mut sources = build
        .sources
        .into_iter()
        .map(|source| {
            let valuation_source = valuation_sources
                .get(source.source_id.as_str())
                .ok_or_else(|| {
                    anyhow!(
                        "source {} is absent from valuation policy",
                        source.source_id
                    )
                })?;
            if source.asset_or_position_id != valuation_source.asset_or_position_id
                || source.liability_treatment != valuation_source.liability_treatment
                || valuation_method(&source.valuation_verifier) != valuation_source.valuation_method
            {
                bail!(
                    "source {} does not match its typed valuation-policy row",
                    source.source_id
                );
            }
            build_source(
                source,
                &valuation_policy_hash,
                &build.valuation_policy.valuation_unit_id,
                build.valuation_policy.valuation_scale,
            )
        })
        .collect::<Result<Vec<_>>>()?;
    sources.sort_by(|left, right| left.source_id.cmp(&right.source_id));
    let manifest = SourceManifestV1 {
        schema: MANIFEST_SCHEMA_V1.to_string(),
        sources,
    };
    manifest.validate().map_err(anyhow::Error::msg)?;
    Ok(manifest)
}

fn valuation_method(verifier: &ValuationVerifierBuildV1) -> PortfolioValuationMethodV1 {
    match verifier {
        ValuationVerifierBuildV1::SameAsQuantity => {
            PortfolioValuationMethodV1::IntegratedSourceProof
        }
        ValuationVerifierBuildV1::EvmChainlink { .. } => {
            PortfolioValuationMethodV1::EvmChainlinkStateProof
        }
    }
}

fn build_source(
    source: ManifestSourceBuildV1,
    valuation_policy_hash: &str,
    valuation_unit_id: &str,
    valuation_scale: u64,
) -> Result<SourceManifestEntryV1> {
    let quantity = quantity_bindings(
        &source.quantity_verifier,
        &source.reserve_owner,
        &source.asset_or_position_id,
    )
    .with_context(|| format!("build quantity bindings for {}", source.source_id))?;
    let valuation_verifier_commitment = match source.valuation_verifier {
        ValuationVerifierBuildV1::SameAsQuantity => {
            if !quantity.supports_integrated_valuation {
                bail!(
                    "source {} quantity verifier does not cryptographically derive valuation",
                    source.source_id
                );
            }
            quantity.verifier_commitment.clone()
        }
        ValuationVerifierBuildV1::EvmChainlink { policy, committee } => {
            require_committee_root(&committee, &policy.committee_root)?;
            if policy.valuation_policy_hash != valuation_policy_hash
                || policy.valuation_unit_id != valuation_unit_id
                || policy.valuation_scale != valuation_scale
            {
                bail!(
                    "source {} Chainlink valuation context does not match manifest build context",
                    source.source_id
                );
            }
            let expected_rows = quantity.valuation_rows.as_ref().ok_or_else(|| {
                anyhow!(
                    "source {} quantity verifier cannot feed Chainlink valuation",
                    source.source_id
                )
            })?;
            let actual_rows = policy
                .rows
                .iter()
                .map(|row| (row.position_id.clone(), row.quantity_decimals))
                .collect::<BTreeMap<_, _>>();
            if actual_rows.len() != policy.rows.len() || &actual_rows != expected_rows {
                bail!(
                    "source {} Chainlink rows do not exactly match quantity positions and decimals",
                    source.source_id
                );
            }
            policy
                .commitment()
                .map_err(|error| anyhow!("invalid Chainlink valuation policy: {error:?}"))?
        }
    };

    // The valuation verifier commitment includes every haircut. Reusing it as
    // the manifest haircut-policy hash prevents an unverified side document or
    // pasted operator hash from changing the economic policy.
    Ok(SourceManifestEntryV1 {
        source_id: source.source_id,
        adapter_kind: quantity.adapter_kind.to_string(),
        source_domain: quantity.source_domain,
        asset_or_position_id: source.asset_or_position_id,
        reserve_owner_commitment: quantity.reserve_owner_commitment,
        quantity_verifier_commitment: quantity.verifier_commitment,
        valuation_verifier_commitment: valuation_verifier_commitment.clone(),
        quantity_evidence_class: TrustClassV1::Cryptographic,
        valuation_evidence_class: TrustClassV1::Cryptographic,
        freshness_policy: source.freshness_policy,
        haircut_policy_hash: valuation_verifier_commitment,
        liability_treatment: source.liability_treatment,
        adapter_schema_version: 1,
    })
}

fn quantity_bindings(
    verifier: &QuantityVerifierBuildV1,
    owner: &ReserveOwnerBuildV1,
    asset_or_position_id: &str,
) -> Result<QuantityBindings> {
    match verifier {
        QuantityVerifierBuildV1::AaveV3 { policy, committee } => {
            let owner = require_evm_owner(owner)?;
            let committee_root = committee_root(committee)?;
            Ok(QuantityBindings {
                adapter_kind: AAVE_V3_ADAPTER_KIND_V1,
                source_domain: policy.source_domain.clone(),
                reserve_owner_commitment: aave_v3_owner_commitment(owner),
                verifier_commitment: policy
                    .commitment(&committee_root)
                    .map_err(|error| anyhow!("invalid Aave policy: {error:?}"))?,
                supports_integrated_valuation: true,
                valuation_rows: None,
            })
        }
        QuantityVerifierBuildV1::EvmSpot { policy, committees } => {
            let owner = require_evm_owner(owner)?;
            if asset_or_position_id != policy.aggregate_position_id {
                bail!("EVM spot asset_or_position_id must equal aggregate_position_id");
            }
            require_exact_committee_roots(
                committees,
                policy
                    .chains
                    .iter()
                    .map(|chain| chain.committee_root.as_str()),
            )?;
            Ok(QuantityBindings {
                adapter_kind: EVM_SPOT_ADAPTER_KIND_V1,
                source_domain: policy.aggregate_source_domain.clone(),
                reserve_owner_commitment: evm_spot_owner_commitment(owner),
                verifier_commitment: policy
                    .commitment()
                    .map_err(|error| anyhow!("invalid EVM spot policy: {error:?}"))?,
                supports_integrated_valuation: false,
                valuation_rows: Some(evm_spot_valuation_rows(policy)?),
            })
        }
        QuantityVerifierBuildV1::HyperliquidReceipt { policy, committee } => {
            let owner = require_evm_owner(owner)?;
            let committee_root = committee_root(committee)?;
            Ok(QuantityBindings {
                adapter_kind: HYPERLIQUID_RECEIPT_ADAPTER_KIND_V1,
                source_domain: policy.source_domain.clone(),
                reserve_owner_commitment: hyperliquid_owner_commitment(owner),
                verifier_commitment: policy
                    .commitment(&committee_root)
                    .map_err(|error| anyhow!("invalid Hyperliquid policy: {error:?}"))?,
                supports_integrated_valuation: true,
                valuation_rows: None,
            })
        }
        QuantityVerifierBuildV1::NearReceipt { policy, committee } => {
            let account_id = require_near_owner(owner)?;
            let committee_root = committee_root(committee)?;
            Ok(QuantityBindings {
                adapter_kind: NEAR_RECEIPT_QUANTITY_ADAPTER_KIND_V1,
                source_domain: policy.source_domain.clone(),
                reserve_owner_commitment: policy
                    .reserve_owner_commitment(account_id)
                    .map_err(|error| anyhow!("invalid NEAR owner: {error:?}"))?,
                verifier_commitment: policy
                    .commitment(&committee_root)
                    .map_err(|error| anyhow!("invalid NEAR policy: {error:?}"))?,
                supports_integrated_valuation: false,
                valuation_rows: Some(BTreeMap::from([(asset_or_position_id.to_string(), 24)])),
            })
        }
        QuantityVerifierBuildV1::SolanaStakeReader { policy, committee } => {
            let wallet_pubkey = require_solana_owner(owner)?;
            if wallet_pubkey != policy.wallet_pubkey
                || asset_or_position_id != policy.position_set_id
            {
                bail!("Solana owner or position set does not match reader policy");
            }
            require_committee_root(committee, &policy.checkpoint_committee_root)?;
            Ok(QuantityBindings {
                adapter_kind: SOLANA_STAKE_READER_ADAPTER_KIND_V1,
                source_domain: policy.source_domain.clone(),
                reserve_owner_commitment: solana_stake_owner_commitment(wallet_pubkey),
                verifier_commitment: policy
                    .commitment()
                    .map_err(|error| anyhow!("invalid Solana reader policy: {error:?}"))?,
                supports_integrated_valuation: false,
                valuation_rows: Some(BTreeMap::from([(asset_or_position_id.to_string(), 9)])),
            })
        }
        QuantityVerifierBuildV1::MoneroReserve { policy, committee } => {
            let (spend, view) = require_monero_owner(owner)?;
            if spend != policy.address_spend_public_key
                || view != policy.address_view_public_key
                || asset_or_position_id != policy.position_id
            {
                bail!("Monero owner or position does not match reserve policy");
            }
            require_committee_root(committee, &policy.checkpoint_committee_root)?;
            Ok(QuantityBindings {
                adapter_kind: MONERO_RESERVE_ADAPTER_KIND_V1,
                source_domain: policy.source_domain.clone(),
                reserve_owner_commitment: monero_reserve_owner_commitment(spend, view),
                verifier_commitment: policy
                    .commitment()
                    .map_err(|error| anyhow!("invalid Monero policy: {error:?}"))?,
                supports_integrated_valuation: false,
                valuation_rows: Some(BTreeMap::from([(asset_or_position_id.to_string(), 12)])),
            })
        }
    }
}

fn evm_spot_valuation_rows(policy: &EvmSpotPolicyV1) -> Result<BTreeMap<String, u8>> {
    let rows = policy
        .chains
        .iter()
        .flat_map(|chain| {
            std::iter::once((chain.native_position_id.clone(), chain.native_decimals)).chain(
                chain
                    .tokens
                    .iter()
                    .map(|token| (token.position_id.clone(), token.decimals)),
            )
        })
        .collect::<Vec<_>>();
    let unique = rows.iter().cloned().collect::<BTreeMap<_, _>>();
    if unique.len() != rows.len() {
        bail!("EVM spot valuation position IDs must be globally unique");
    }
    Ok(unique)
}

fn committee_root(committee: &BftCheckpointCommitteeV1) -> Result<String> {
    let minimum_bft_quorum = committee
        .validators
        .len()
        .checked_mul(2)
        .map(|doubled| (doubled / 3) + 1)
        .ok_or_else(|| anyhow!("checkpoint committee quorum overflow"))?;
    if usize::from(committee.quorum) < minimum_bft_quorum {
        bail!(
            "checkpoint committee quorum {} is below BFT minimum {minimum_bft_quorum}",
            committee.quorum
        );
    }
    committee.root().map_err(anyhow::Error::msg)
}

fn require_committee_root(committee: &BftCheckpointCommitteeV1, expected_root: &str) -> Result<()> {
    if committee_root(committee)? != expected_root {
        bail!("checkpoint committee does not match policy root");
    }
    Ok(())
}

fn require_exact_committee_roots<'a>(
    committees: &[BftCheckpointCommitteeV1],
    expected: impl Iterator<Item = &'a str>,
) -> Result<()> {
    let expected = expected.map(str::to_string).collect::<BTreeSet<_>>();
    let roots = committees
        .iter()
        .map(committee_root)
        .collect::<Result<Vec<_>>>()?;
    let actual = roots.iter().cloned().collect::<BTreeSet<_>>();
    if roots.len() != actual.len() || actual != expected {
        bail!("EVM spot committee set does not exactly match policy roots");
    }
    Ok(())
}

fn require_evm_owner(owner: &ReserveOwnerBuildV1) -> Result<Address> {
    match owner {
        ReserveOwnerBuildV1::EvmAddress { address } if *address != Address::ZERO => Ok(*address),
        _ => bail!("quantity verifier requires a nonzero EVM reserve owner"),
    }
}

fn require_near_owner(owner: &ReserveOwnerBuildV1) -> Result<&str> {
    match owner {
        ReserveOwnerBuildV1::NearAccount { account_id } => Ok(account_id),
        _ => bail!("NEAR verifier requires a NEAR account reserve owner"),
    }
}

fn require_solana_owner(owner: &ReserveOwnerBuildV1) -> Result<[u8; 32]> {
    match owner {
        ReserveOwnerBuildV1::SolanaWallet { wallet_pubkey } if *wallet_pubkey != [0; 32] => {
            Ok(*wallet_pubkey)
        }
        _ => bail!("Solana verifier requires a nonzero Solana wallet owner"),
    }
}

fn require_monero_owner(owner: &ReserveOwnerBuildV1) -> Result<(B256, B256)> {
    match owner {
        ReserveOwnerBuildV1::MoneroAddress {
            spend_public_key,
            view_public_key,
        } if *spend_public_key != B256::ZERO && *view_public_key != B256::ZERO => {
            Ok((*spend_public_key, *view_public_key))
        }
        _ => bail!("Monero verifier requires nonzero public spend and view keys"),
    }
}

#[cfg(test)]
mod tests {
    use alloy_primitives::{Address, B256, U256};
    use postfiat_crypto_provider::ml_dsa_65_keygen_from_seed;
    use reserve_proof_types::{
        bft_checkpoint::BftCheckpointValidatorV1,
        evm_chainlink_valuation::EvmChainlinkValuationRowPolicyV1,
        evm_spot::{EvmSpotChainPolicyV1, EvmSpotTokenPolicyV1},
        portfolio_valuation::{PortfolioValuationSourceV1, PORTFOLIO_VALUATION_POLICY_SCHEMA_V1},
    };

    use super::*;

    fn committee() -> BftCheckpointCommitteeV1 {
        let key = ml_dsa_65_keygen_from_seed(&[7; 32]);
        BftCheckpointCommitteeV1 {
            epoch: 1,
            quorum: 1,
            validators: vec![BftCheckpointValidatorV1 {
                validator_id: "validator-0".to_string(),
                public_key: key.public_key,
            }],
        }
    }

    fn evm_spot_build() -> ManifestBuildV1 {
        let committee = committee();
        let committee_root = committee.root().unwrap();
        let valuation_policy = PortfolioValuationPolicyV1 {
            schema: PORTFOLIO_VALUATION_POLICY_SCHEMA_V1.to_string(),
            nav_asset_id: "10".repeat(48),
            valuation_unit_id: "22".repeat(48),
            valuation_scale: 100_000_000,
            sources: vec![PortfolioValuationSourceV1 {
                source_id: "evm-spot".to_string(),
                asset_or_position_id: "evm-spot-set:a666-v1".to_string(),
                valuation_method: PortfolioValuationMethodV1::EvmChainlinkStateProof,
                liability_treatment: LiabilityTreatmentV1::Asset,
            }],
        };
        let valuation_policy_hash = valuation_policy.hash().unwrap();
        let valuation = EvmChainlinkValuationPolicyV1 {
            source_domain: "eip155:1".to_string(),
            chain_id: 1,
            committee_root: committee_root.clone(),
            valuation_policy_hash,
            valuation_unit_id: "22".repeat(48),
            valuation_scale: 100_000_000,
            proxy_phase_slot_index: 2,
            hot_vars_slot_index: 13,
            transmissions_slot_index: 17,
            max_oracle_age_seconds: 3_600,
            rows: vec![
                EvmChainlinkValuationRowPolicyV1 {
                    position_id: "ethereum-native-eth".to_string(),
                    quantity_decimals: 18,
                    price_decimals: 8,
                    haircut_bps: 10_000,
                    proxy_address: Address::repeat_byte(0x31),
                    proxy_code_hash: B256::repeat_byte(0x32),
                    aggregator_code_hash: B256::repeat_byte(0x33),
                },
                EvmChainlinkValuationRowPolicyV1 {
                    position_id: "ethereum-usdc".to_string(),
                    quantity_decimals: 6,
                    price_decimals: 8,
                    haircut_bps: 10_000,
                    proxy_address: Address::repeat_byte(0x34),
                    proxy_code_hash: B256::repeat_byte(0x35),
                    aggregator_code_hash: B256::repeat_byte(0x36),
                },
            ],
        };
        ManifestBuildV1 {
            schema: BUILD_SCHEMA_V1.to_string(),
            valuation_policy,
            sources: vec![ManifestSourceBuildV1 {
                source_id: "evm-spot".to_string(),
                asset_or_position_id: "evm-spot-set:a666-v1".to_string(),
                reserve_owner: ReserveOwnerBuildV1::EvmAddress {
                    address: Address::repeat_byte(0x21),
                },
                quantity_verifier: QuantityVerifierBuildV1::EvmSpot {
                    policy: EvmSpotPolicyV1 {
                        aggregate_source_domain: "evm-multichain:a666-v1".to_string(),
                        aggregate_position_id: "evm-spot-set:a666-v1".to_string(),
                        maximum_timestamp_skew_ms: 60_000,
                        chains: vec![EvmSpotChainPolicyV1 {
                            chain_id: 1,
                            source_domain: "eip155:1".to_string(),
                            committee_root,
                            native_position_id: "ethereum-native-eth".to_string(),
                            native_account_code_hash: B256::repeat_byte(0x41),
                            native_decimals: 18,
                            tokens: vec![EvmSpotTokenPolicyV1 {
                                position_id: "ethereum-usdc".to_string(),
                                token: Address::repeat_byte(0x42),
                                token_code_hash: B256::repeat_byte(0x43),
                                balance_slot_index: U256::from(9),
                                decimals: 6,
                            }],
                        }],
                    },
                    committees: vec![committee.clone()],
                },
                valuation_verifier: ValuationVerifierBuildV1::EvmChainlink {
                    policy: Box::new(valuation),
                    committee: Box::new(committee),
                },
                freshness_policy: FreshnessPolicyV1 {
                    max_age_blocks: 20,
                    max_observation_span_blocks: 8,
                },
                liability_treatment: LiabilityTreatmentV1::Asset,
            }],
        }
    }

    #[test]
    fn derives_commitments_and_binds_haircut_to_valuation_policy() {
        let manifest = build_manifest(evm_spot_build()).unwrap();
        let entry = &manifest.sources[0];
        assert_eq!(entry.quantity_evidence_class, TrustClassV1::Cryptographic);
        assert_eq!(entry.valuation_evidence_class, TrustClassV1::Cryptographic);
        assert_eq!(
            entry.haircut_policy_hash,
            entry.valuation_verifier_commitment
        );
        assert_ne!(
            entry.quantity_verifier_commitment,
            entry.valuation_verifier_commitment
        );
    }

    #[test]
    fn rejects_missing_committee_and_unverified_integrated_valuation() {
        let mut build = evm_spot_build();
        if let QuantityVerifierBuildV1::EvmSpot { committees, .. } =
            &mut build.sources[0].quantity_verifier
        {
            committees.clear();
        }
        assert!(build_manifest(build).is_err());

        let mut build = evm_spot_build();
        build.sources[0].valuation_verifier = ValuationVerifierBuildV1::SameAsQuantity;
        assert!(build_manifest(build).is_err());

        let mut build = evm_spot_build();
        build.valuation_policy.sources[0].asset_or_position_id = "wrong-position".to_string();
        assert!(build_manifest(build).is_err());

        let mut weak = committee();
        weak.validators.push(BftCheckpointValidatorV1 {
            validator_id: "validator-1".to_string(),
            public_key: ml_dsa_65_keygen_from_seed(&[8; 32]).public_key,
        });
        assert!(committee_root(&weak).is_err());
    }
}
