use std::{fs, path::PathBuf, str::FromStr};

use alloy::primitives::{keccak256, Address, B256};
use anyhow::{bail, Context, Result};
use pfusdc_ingress_program::{
    ingress_policy_hash_v2, PfUsdcIngressProofPolicyV2, ARBITRUM_SEPOLIA_CHAIN_ID,
    ARBITRUM_SEPOLIA_ROLLUP_ADDRESS, ETHEREUM_SEPOLIA_CHAIN_ID,
    ETHEREUM_SEPOLIA_GENESIS_VALIDATORS_ROOT, NITRO_LATEST_CONFIRMED_STORAGE_SLOT,
    PFUSDC_INGRESS_PROOF_POLICY_SCHEMA_V2,
};
use postfiat_types::{
    issued_asset_id, vault_bridge_route_binding, NavProofProfile, VaultBridgeRouteProfileV1,
    NAV_PROFILE_VERIFIER_SP1_ARBITRUM_FINALITY_V1, NAV_SP1_PROOF_ENCODING_GROTH16,
    VAULT_BRIDGE_EVIDENCE_TIER_RECEIPT_PROVEN, VAULT_BRIDGE_ROUTE_PROFILE_SCHEMA_V1,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest as ShaDigest, Sha256};

const INPUT_SCHEMA: &str = "postfiat.pfusdc.tier4_deployment_manifest_input.v1";
const OUTPUT_SCHEMA: &str = "postfiat.pfusdc.tier4_deployment_manifest.v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentManifestInputV1 {
    pub schema: String,
    pub deployment_id: String,
    pub deployer: String,
    pub ethereum_deployer_nonce: u64,
    pub arbitrum_deployer_nonce: u64,
    pub pftl: PftlInputV1,
    pub asset: AssetInputV1,
    pub network: NetworkInputV1,
    pub route: RouteInputV1,
    pub programs: ProgramInputV1,
    pub contracts: ContractInputV1,
    #[serde(default)]
    pub contract_artifacts: Vec<ContractArtifactInputV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PftlInputV1 {
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub initial_checkpoint_block_id: String,
    pub initial_finalized_height: u64,
    pub initial_committee_root: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetInputV1 {
    pub issuer: String,
    pub reserve_operator: String,
    pub redemption_account: String,
    pub code: String,
    pub version: u32,
    pub precision: u8,
    pub display_name: String,
    pub max_supply: u64,
    pub valuation_unit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInputV1 {
    pub ethereum_chain_id: u64,
    pub ethereum_genesis_validators_root: String,
    pub ethereum_arbitrum_bridge: String,
    pub ethereum_arbitrum_bridge_runtime_code_hash: String,
    pub arbitrum_chain_id: u64,
    pub arbitrum_rollup: String,
    pub arbitrum_rollup_runtime_code_hash: String,
    pub rollup_latest_confirmed_storage_slot: String,
    pub arbitrum_token: String,
    pub arbitrum_token_runtime_code_hash: String,
    pub arbitrum_sp1_verifier: String,
    pub arbitrum_sp1_verifier_runtime_code_hash: String,
    pub arbitrum_arb_sys: String,
    pub arbitrum_arb_sys_runtime_code_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteInputV1 {
    pub route_id: String,
    pub route_epoch: u32,
    pub activation_height: u64,
    pub expires_at_height: u64,
    pub max_snapshot_age_blocks: u64,
    pub challenge_window_blocks: u64,
    pub max_epoch_gap_blocks: u64,
    pub settle_deadline_blocks: u64,
    pub min_challenge_bond: u64,
    pub min_attestations: u64,
    pub tolerance_bp: u64,
    pub minimum_confirmations: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgramInputV1 {
    pub ingress_elf_sha256: String,
    pub ingress_program_vkey: String,
    pub egress_elf_sha256: String,
    pub egress_program_vkey: String,
    pub proof_encoding: String,
    pub max_proof_bytes: u64,
    pub max_public_values_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractInputV1 {
    pub finality_verifier_runtime_code_hash: String,
    pub vault_runtime_code_hash: String,
    pub ingress_anchor_runtime_code_hash: String,
    pub compiler: String,
    pub optimizer_runs: u64,
    pub evm_version: String,
    pub metadata_bytecode_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractArtifactInputV1 {
    pub contract: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ContractArtifactCommitmentV1 {
    contract: String,
    path: PathBuf,
    artifact_sha256: String,
    creation_bytecode_keccak256: String,
    unlinked_deployed_bytecode_keccak256: String,
    compiler: String,
    optimizer_enabled: bool,
    optimizer_runs: u64,
    evm_version: String,
    metadata_bytecode_hash: String,
}

pub fn run(input_path: PathBuf, output_path: PathBuf) -> Result<()> {
    let bytes = fs::read(&input_path)
        .with_context(|| format!("read deployment manifest input {}", input_path.display()))?;
    let input: DeploymentManifestInputV1 = serde_json::from_slice(&bytes)
        .with_context(|| format!("decode deployment manifest input {}", input_path.display()))?;
    let output = build_manifest(&input)?;
    let encoded = serde_json::to_vec_pretty(&output)?;
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&output_path, &encoded)
        .with_context(|| format!("write deployment manifest {}", output_path.display()))?;
    println!("{}", String::from_utf8(encoded)?);
    Ok(())
}

fn build_manifest(input: &DeploymentManifestInputV1) -> Result<Value> {
    if input.schema != INPUT_SCHEMA {
        bail!("deployment manifest input schema mismatch");
    }
    if input.deployment_id.trim().is_empty() {
        bail!("deployment_id must be nonempty");
    }
    if input.pftl.chain_id.trim().is_empty() {
        bail!("PFTL chain ID must be nonempty");
    }
    if input.pftl.protocol_version != 1 || input.pftl.initial_finalized_height == 0 {
        bail!("PFTL protocol version must be 1 and initial finalized height must be nonzero");
    }
    if input.asset.issuer.trim().is_empty()
        || input.asset.reserve_operator != input.asset.issuer
        || input.asset.redemption_account != input.asset.issuer
        || input.asset.code != "PFUSDC"
        || input.asset.version != 1
        || input.asset.precision != 6
        || input.asset.max_supply == 0
        || input.asset.valuation_unit != "USDC"
    {
        bail!("pfUSDC asset configuration is not the Tier-4 v1 bootstrap configuration");
    }
    if input.route.route_epoch == 0
        || input.route.activation_height == 0
        || input.route.expires_at_height <= input.route.activation_height
        || input.route.expires_at_height <= input.pftl.initial_finalized_height
    {
        bail!("route epoch/lifetime is inconsistent with the initial finalized checkpoint");
    }
    if input.route.min_challenge_bond != 0
        || input.route.min_attestations != 0
        || input.route.tolerance_bp != 0
        || input.route.minimum_confirmations != 0
    {
        bail!("Tier-4 proof route must not retain observer, confirmation, bond, or tolerance authority");
    }
    validate_hex("pftl.genesis_hash", &input.pftl.genesis_hash, 48, false)?;
    validate_hex(
        "pftl.initial_checkpoint_block_id",
        &input.pftl.initial_checkpoint_block_id,
        48,
        false,
    )?;
    validate_hex(
        "pftl.initial_committee_root",
        &input.pftl.initial_committee_root,
        48,
        false,
    )?;
    validate_hex(
        "programs.ingress_elf_sha256",
        &input.programs.ingress_elf_sha256,
        32,
        false,
    )?;
    validate_hex(
        "programs.egress_elf_sha256",
        &input.programs.egress_elf_sha256,
        32,
        false,
    )?;
    validate_hex(
        "programs.ingress_program_vkey",
        &input.programs.ingress_program_vkey,
        32,
        true,
    )?;
    validate_hex(
        "programs.egress_program_vkey",
        &input.programs.egress_program_vkey,
        32,
        true,
    )?;
    if input.programs.proof_encoding != NAV_SP1_PROOF_ENCODING_GROTH16 {
        bail!("proof encoding must be {NAV_SP1_PROOF_ENCODING_GROTH16}");
    }
    if input.programs.max_proof_bytes != 4_096 || input.programs.max_public_values_bytes != 16_384 {
        bail!("Tier-4 proof byte bounds must be exactly 4096/16384");
    }

    let deployer = parse_address("deployer", &input.deployer)?;
    if deployer == Address::ZERO {
        bail!("deployer must be nonzero");
    }
    let anchor_address = deployer.create(input.ethereum_deployer_nonce);
    let verifier_address = deployer.create(input.arbitrum_deployer_nonce);
    let vault_nonce = input
        .arbitrum_deployer_nonce
        .checked_add(1)
        .context("Arbitrum deployment nonce overflow")?;
    let vault_address = deployer.create(vault_nonce);
    let token_address = parse_address("network.arbitrum_token", &input.network.arbitrum_token)?;
    let bridge_address = parse_address(
        "network.ethereum_arbitrum_bridge",
        &input.network.ethereum_arbitrum_bridge,
    )?;
    let rollup_address = parse_address("network.arbitrum_rollup", &input.network.arbitrum_rollup)?;
    let sp1_verifier = parse_address(
        "network.arbitrum_sp1_verifier",
        &input.network.arbitrum_sp1_verifier,
    )?;
    let arb_sys = parse_address("network.arbitrum_arb_sys", &input.network.arbitrum_arb_sys)?;

    let ethereum_genesis_validators_root = parse_b256(
        "network.ethereum_genesis_validators_root",
        &input.network.ethereum_genesis_validators_root,
    )?;
    let bridge_code_hash = parse_b256(
        "network.ethereum_arbitrum_bridge_runtime_code_hash",
        &input.network.ethereum_arbitrum_bridge_runtime_code_hash,
    )?;
    let rollup_code_hash = parse_b256(
        "network.arbitrum_rollup_runtime_code_hash",
        &input.network.arbitrum_rollup_runtime_code_hash,
    )?;
    let rollup_slot = parse_b256(
        "network.rollup_latest_confirmed_storage_slot",
        &input.network.rollup_latest_confirmed_storage_slot,
    )?;
    let token_code_hash = parse_b256(
        "network.arbitrum_token_runtime_code_hash",
        &input.network.arbitrum_token_runtime_code_hash,
    )?;
    let sp1_verifier_code_hash = parse_b256(
        "network.arbitrum_sp1_verifier_runtime_code_hash",
        &input.network.arbitrum_sp1_verifier_runtime_code_hash,
    )?;
    let arb_sys_code_hash = parse_b256(
        "network.arbitrum_arb_sys_runtime_code_hash",
        &input.network.arbitrum_arb_sys_runtime_code_hash,
    )?;
    let finality_verifier_code_hash = parse_b256(
        "contracts.finality_verifier_runtime_code_hash",
        &input.contracts.finality_verifier_runtime_code_hash,
    )?;
    let vault_code_hash = parse_b256(
        "contracts.vault_runtime_code_hash",
        &input.contracts.vault_runtime_code_hash,
    )?;
    let anchor_code_hash = parse_b256(
        "contracts.ingress_anchor_runtime_code_hash",
        &input.contracts.ingress_anchor_runtime_code_hash,
    )?;

    if input.network.ethereum_chain_id != ETHEREUM_SEPOLIA_CHAIN_ID
        || ethereum_genesis_validators_root != ETHEREUM_SEPOLIA_GENESIS_VALIDATORS_ROOT
        || input.network.arbitrum_chain_id != ARBITRUM_SEPOLIA_CHAIN_ID
        || rollup_address != ARBITRUM_SEPOLIA_ROLLUP_ADDRESS
        || rollup_slot != NITRO_LATEST_CONFIRMED_STORAGE_SLOT
    {
        bail!("network inputs do not match the supported Ethereum/Arbitrum Sepolia binding");
    }
    if bridge_address == Address::ZERO
        || rollup_address == Address::ZERO
        || token_address == Address::ZERO
        || sp1_verifier == Address::ZERO
        || arb_sys == Address::ZERO
        || bridge_code_hash == B256::ZERO
        || rollup_code_hash == B256::ZERO
        || token_code_hash == B256::ZERO
        || sp1_verifier_code_hash == B256::ZERO
        || arb_sys_code_hash == B256::ZERO
        || finality_verifier_code_hash == B256::ZERO
        || vault_code_hash == B256::ZERO
        || anchor_code_hash == B256::ZERO
    {
        bail!("network and contract addresses/code hashes must be nonzero");
    }

    let policy = PfUsdcIngressProofPolicyV2 {
        schema: PFUSDC_INGRESS_PROOF_POLICY_SCHEMA_V2.to_string(),
        ethereum_chain_id: input.network.ethereum_chain_id,
        ethereum_genesis_validators_root,
        arbitrum_chain_id: input.network.arbitrum_chain_id,
        arbitrum_rollup_address: rollup_address,
        arbitrum_rollup_runtime_code_hash: rollup_code_hash,
        rollup_latest_confirmed_storage_slot: rollup_slot,
        arbitrum_vault_address: vault_address,
        arbitrum_vault_runtime_code_hash: vault_code_hash,
        arbitrum_token_address: token_address,
        arbitrum_token_runtime_code_hash: token_code_hash,
        ethereum_ingress_anchor_address: anchor_address,
        ethereum_ingress_anchor_runtime_code_hash: anchor_code_hash,
    };
    let ingress_policy_hash = ingress_policy_hash_v2(&policy);

    let asset_id = issued_asset_id(
        &input.pftl.chain_id,
        &input.asset.issuer,
        &input.asset.code,
        input.asset.version,
    )
    .map_err(anyhow::Error::msg)?;
    let vault_address_text = lower_address(vault_address);
    let token_address_text = lower_address(token_address);
    let route_profile = VaultBridgeRouteProfileV1 {
        schema: VAULT_BRIDGE_ROUTE_PROFILE_SCHEMA_V1.to_string(),
        route_id: input.route.route_id.clone(),
        asset_id: asset_id.clone(),
        source_chain_id: input.network.arbitrum_chain_id,
        vault_address: vault_address_text.clone(),
        vault_runtime_code_hash: lower_b256(vault_code_hash),
        token_address: token_address_text.clone(),
        token_runtime_code_hash: lower_b256(token_code_hash),
        route_epoch: input.route.route_epoch,
        verifier_kind: NAV_PROFILE_VERIFIER_SP1_ARBITRUM_FINALITY_V1.to_string(),
        evidence_tier: VAULT_BRIDGE_EVIDENCE_TIER_RECEIPT_PROVEN.to_string(),
        verifier_policy_hash: ingress_policy_hash.clone(),
        verifier_program_vkey: normalize_prefixed_hex(&input.programs.ingress_program_vkey),
        verifier_proof_encoding: input.programs.proof_encoding.clone(),
        max_proof_bytes: input.programs.max_proof_bytes,
        max_public_values_bytes: input.programs.max_public_values_bytes,
        max_snapshot_age_blocks: input.route.max_snapshot_age_blocks,
        challenge_window_blocks: input.route.challenge_window_blocks,
        max_epoch_gap_blocks: input.route.max_epoch_gap_blocks,
        settle_deadline_blocks: input.route.settle_deadline_blocks,
        min_challenge_bond: input.route.min_challenge_bond,
        min_attestations: input.route.min_attestations,
        minimum_confirmations: input.route.minimum_confirmations,
        activation_height: input.route.activation_height,
        expires_at_height: input.route.expires_at_height,
    };
    route_profile.validate().map_err(anyhow::Error::msg)?;
    let route_profile_hash = route_profile.profile_hash().map_err(anyhow::Error::msg)?;
    let route_binding = vault_bridge_route_binding(&route_profile_hash, input.route.route_epoch)
        .map_err(anyhow::Error::msg)?;
    let source_domain = route_profile.source_domain();
    let source_class = format!("vault_bridge:{source_domain}");
    let nav_profile = NavProofProfile::new_with_bridge_observer_min_confirmations(
        &input.asset.issuer,
        NAV_PROFILE_VERIFIER_SP1_ARBITRUM_FINALITY_V1,
        &source_class,
        input.route.max_snapshot_age_blocks,
        input.route.challenge_window_blocks,
        input.route.max_epoch_gap_blocks,
        input.route.settle_deadline_blocks,
        input.route.min_challenge_bond,
        input.route.min_attestations,
        input.route.tolerance_bp,
        input.route.minimum_confirmations,
        &ingress_policy_hash,
        &input.programs.ingress_program_vkey,
        &input.programs.proof_encoding,
        input.programs.max_proof_bytes,
        input.programs.max_public_values_bytes,
    )
    .and_then(|profile| profile.with_vault_bridge_route_policy_hash(&route_profile_hash))
    .map_err(anyhow::Error::msg)?;
    nav_profile.validate().map_err(anyhow::Error::msg)?;

    let pftl_chain_id_hash = keccak256(input.pftl.chain_id.as_bytes());
    let pftl_genesis_hash_commitment = keccak_hex48(&input.pftl.genesis_hash)?;
    let route_profile_hash_commitment = keccak_hex48(&route_profile_hash)?;
    let asset_id_commitment = keccak_hex48(&asset_id)?;
    let initial_checkpoint_commitment = keccak_hex48(&input.pftl.initial_checkpoint_block_id)?;
    let initial_committee_root_commitment = keccak_hex48(&input.pftl.initial_committee_root)?;

    let artifacts = input
        .contract_artifacts
        .iter()
        .map(|artifact| artifact_commitment(artifact, &input.contracts))
        .collect::<Result<Vec<_>>>()?;

    Ok(json!({
        "schema": OUTPUT_SCHEMA,
        "deployment_id": input.deployment_id,
        "deployer": lower_address(deployer),
        "deployment_sequence": {
            "ethereum": [
                {"nonce": input.ethereum_deployer_nonce, "contract": "PfUsdcIngressAnchorV1", "address": lower_address(anchor_address)}
            ],
            "arbitrum": [
                {"nonce": input.arbitrum_deployer_nonce, "contract": "PFTLFinalityVerifierV1", "address": lower_address(verifier_address)},
                {"nonce": vault_nonce, "contract": "ERC20BridgeVaultV2", "address": vault_address_text}
            ]
        },
        "pftl": input.pftl,
        "asset": {
            "asset_id": asset_id,
            "configuration": input.asset
        },
        "ingress_policy": {
            "policy": policy,
            "policy_hash": ingress_policy_hash
        },
        "route_profile": {
            "profile": route_profile,
            "profile_hash": route_profile_hash,
            "route_binding": route_binding
        },
        "nav_profile": nav_profile,
        "network": input.network,
        "programs": input.programs,
        "contracts": {
            "configuration": input.contracts,
            "artifacts": artifacts,
            "constructors": {
                "ingress_anchor": {
                    "bridge": lower_address(bridge_address),
                    "l2_vault": lower_address(vault_address),
                    "l2_token": token_address_text,
                    "l2_chain_id": input.network.arbitrum_chain_id,
                    "governed_route_binding": format!("0x{route_binding}")
                },
                "vault": {
                    "token": lower_address(token_address),
                    "finality_verifier": lower_address(verifier_address),
                    "token_runtime_code_hash": lower_b256(token_code_hash),
                    "arb_sys": lower_address(arb_sys),
                    "ingress_anchor": lower_address(anchor_address),
                    "initial_owner": lower_address(deployer)
                },
                "finality_verifier": {
                    "sp1_verifier": lower_address(sp1_verifier),
                    "program_vkey": normalize_prefixed_hex(&input.programs.egress_program_vkey),
                    "pftl_chain_id_hash": lower_b256(pftl_chain_id_hash),
                    "pftl_genesis_hash_commitment": lower_b256(pftl_genesis_hash_commitment),
                    "pftl_protocol_version": input.pftl.protocol_version,
                    "route_profile_hash_commitment": lower_b256(route_profile_hash_commitment),
                    "route_epoch": input.route.route_epoch,
                    "asset_id_commitment": lower_b256(asset_id_commitment),
                    "arbitrum_chain_id": input.network.arbitrum_chain_id,
                    "vault_runtime_code_hash": lower_b256(vault_code_hash),
                    "token": lower_address(token_address),
                    "token_runtime_code_hash": lower_b256(token_code_hash),
                    "max_proof_bytes": input.programs.max_proof_bytes,
                    "max_public_values_bytes": input.programs.max_public_values_bytes,
                    "initial_checkpoint_commitment": lower_b256(initial_checkpoint_commitment),
                    "initial_finalized_height": input.pftl.initial_finalized_height,
                    "initial_committee_root_commitment": lower_b256(initial_committee_root_commitment)
                }
            }
        },
        "evm_commitments": {
            "pftl_chain_id_hash": lower_b256(pftl_chain_id_hash),
            "pftl_genesis_hash_commitment": lower_b256(pftl_genesis_hash_commitment),
            "route_profile_hash_commitment": lower_b256(route_profile_hash_commitment),
            "asset_id_commitment": lower_b256(asset_id_commitment),
            "initial_checkpoint_commitment": lower_b256(initial_checkpoint_commitment),
            "initial_committee_root_commitment": lower_b256(initial_committee_root_commitment)
        },
        "invariants": {
            "observer_attestations_required": input.route.min_attestations != 0,
            "observer_confirmations_required": input.route.minimum_confirmations != 0,
            "proof_encoding_is_groth16": input.programs.proof_encoding == NAV_SP1_PROOF_ENCODING_GROTH16,
            "anchor_runtime_hash_excludes_route_binding": true,
            "vault_runtime_hash_excludes_finality_verifier_and_owner": true,
            "route_and_nav_profile_cross_bound": true
        }
    }))
}

fn artifact_commitment(
    input: &ContractArtifactInputV1,
    expected: &ContractInputV1,
) -> Result<ContractArtifactCommitmentV1> {
    let bytes = fs::read(&input.path)
        .with_context(|| format!("read contract artifact {}", input.path.display()))?;
    let value: Value = serde_json::from_slice(&bytes)
        .with_context(|| format!("decode contract artifact {}", input.path.display()))?;
    let creation = artifact_hex(&value, "/bytecode/object", &input.path)?;
    let deployed = artifact_hex(&value, "/deployedBytecode/object", &input.path)?;
    let compiler = value
        .pointer("/metadata/compiler/version")
        .and_then(Value::as_str)
        .context("contract artifact omitted compiler version")?;
    let optimizer_enabled = value
        .pointer("/metadata/settings/optimizer/enabled")
        .and_then(Value::as_bool)
        .context("contract artifact omitted optimizer.enabled")?;
    let optimizer_runs = value
        .pointer("/metadata/settings/optimizer/runs")
        .and_then(Value::as_u64)
        .context("contract artifact omitted optimizer.runs")?;
    let evm_version = value
        .pointer("/metadata/settings/evmVersion")
        .and_then(Value::as_str)
        .context("contract artifact omitted evmVersion")?;
    let metadata_bytecode_hash = value
        .pointer("/metadata/settings/metadata/bytecodeHash")
        .and_then(Value::as_str)
        .context("contract artifact omitted metadata.bytecodeHash")?;
    if compiler != expected.compiler
        || !optimizer_enabled
        || optimizer_runs != expected.optimizer_runs
        || evm_version != expected.evm_version
        || metadata_bytecode_hash != expected.metadata_bytecode_hash
    {
        bail!(
            "contract artifact {} compiler settings do not match manifest input",
            input.contract
        );
    }
    Ok(ContractArtifactCommitmentV1 {
        contract: input.contract.clone(),
        path: input.path.clone(),
        artifact_sha256: hex::encode(Sha256::digest(&bytes)),
        creation_bytecode_keccak256: lower_b256(keccak256(creation)),
        unlinked_deployed_bytecode_keccak256: lower_b256(keccak256(deployed)),
        compiler: compiler.to_string(),
        optimizer_enabled,
        optimizer_runs,
        evm_version: evm_version.to_string(),
        metadata_bytecode_hash: metadata_bytecode_hash.to_string(),
    })
}

fn artifact_hex(value: &Value, pointer: &str, path: &std::path::Path) -> Result<Vec<u8>> {
    let text = value
        .pointer(pointer)
        .and_then(Value::as_str)
        .with_context(|| format!("artifact {} omitted {pointer}", path.display()))?;
    hex::decode(text.strip_prefix("0x").unwrap_or(text))
        .with_context(|| format!("artifact {} has invalid hex at {pointer}", path.display()))
}

fn parse_address(field: &str, value: &str) -> Result<Address> {
    let address =
        Address::from_str(value).with_context(|| format!("{field} must be an EVM address"))?;
    if value != lower_address(address) {
        bail!("{field} must be a canonical lowercase EVM address");
    }
    Ok(address)
}

fn parse_b256(field: &str, value: &str) -> Result<B256> {
    validate_hex(field, value, 32, true)?;
    B256::from_str(value).with_context(|| format!("{field} must be a 32-byte hex value"))
}

fn validate_hex(field: &str, value: &str, bytes: usize, prefixed: bool) -> Result<()> {
    let actual_prefixed = value.starts_with("0x");
    if actual_prefixed != prefixed {
        bail!("{field} prefix policy mismatch");
    }
    let text = value.strip_prefix("0x").unwrap_or(value);
    if text.len() != bytes * 2
        || !text.bytes().all(|byte| byte.is_ascii_hexdigit())
        || text.bytes().any(|byte| byte.is_ascii_uppercase())
    {
        bail!("{field} must be canonical lowercase {bytes}-byte hex");
    }
    Ok(())
}

fn keccak_hex48(value: &str) -> Result<B256> {
    validate_hex("48-byte commitment input", value, 48, false)?;
    Ok(keccak256(hex::decode(value)?))
}

fn normalize_prefixed_hex(value: &str) -> String {
    format!(
        "0x{}",
        value
            .strip_prefix("0x")
            .unwrap_or(value)
            .to_ascii_lowercase()
    )
}

fn lower_address(value: Address) -> String {
    format!("{value:#x}")
}

fn lower_b256(value: B256) -> String {
    format!("{value:#x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> DeploymentManifestInputV1 {
        DeploymentManifestInputV1 {
            schema: INPUT_SCHEMA.to_string(),
            deployment_id: "tier4-test".to_string(),
            deployer: "0x1455bd7fbfbf92a171ef36025e13959e3b0ad8c0".to_string(),
            ethereum_deployer_nonce: 0,
            arbitrum_deployer_nonce: 0,
            pftl: PftlInputV1 {
                chain_id: "postfiat-wan-devnet-2".to_string(),
                genesis_hash: "11".repeat(48),
                protocol_version: 1,
                initial_checkpoint_block_id: "22".repeat(48),
                initial_finalized_height: 1,
                initial_committee_root: "33".repeat(48),
            },
            asset: AssetInputV1 {
                issuer: "pf-tier4-issuer".to_string(),
                reserve_operator: "pf-tier4-issuer".to_string(),
                redemption_account: "pf-tier4-issuer".to_string(),
                code: "PFUSDC".to_string(),
                version: 1,
                precision: 6,
                display_name: "proof-native pfUSDC".to_string(),
                max_supply: 1_000_000_000_000_000,
                valuation_unit: "USDC".to_string(),
            },
            network: NetworkInputV1 {
                ethereum_chain_id: 11_155_111,
                ethereum_genesis_validators_root: lower_b256(
                    ETHEREUM_SEPOLIA_GENESIS_VALIDATORS_ROOT,
                ),
                ethereum_arbitrum_bridge: "0x38f918d0e9f1b721edaa41302e399fa1b79333a9".to_string(),
                ethereum_arbitrum_bridge_runtime_code_hash: format!("0x{}", "45".repeat(32)),
                arbitrum_chain_id: 421_614,
                arbitrum_rollup: lower_address(ARBITRUM_SEPOLIA_ROLLUP_ADDRESS),
                arbitrum_rollup_runtime_code_hash: format!("0x{}", "46".repeat(32)),
                rollup_latest_confirmed_storage_slot: lower_b256(
                    NITRO_LATEST_CONFIRMED_STORAGE_SLOT,
                ),
                arbitrum_token: "0x75faf114eafb1bdbe2f0316df893fd58ce46aa4d".to_string(),
                arbitrum_token_runtime_code_hash: format!("0x{}", "48".repeat(32)),
                arbitrum_sp1_verifier: "0x3b6041173b80e77f038f3f2c0f9744f04837185e".to_string(),
                arbitrum_sp1_verifier_runtime_code_hash: format!("0x{}", "49".repeat(32)),
                arbitrum_arb_sys: "0x0000000000000000000000000000000000000064".to_string(),
                arbitrum_arb_sys_runtime_code_hash: format!("0x{}", "4a".repeat(32)),
            },
            route: RouteInputV1 {
                route_id: "pfusdc-tier4-arbitrum-sepolia-v1".to_string(),
                route_epoch: 1,
                activation_height: 20,
                expires_at_height: 100_000,
                max_snapshot_age_blocks: 100,
                challenge_window_blocks: 1,
                max_epoch_gap_blocks: 1_000,
                settle_deadline_blocks: 1_000,
                min_challenge_bond: 0,
                min_attestations: 0,
                tolerance_bp: 0,
                minimum_confirmations: 0,
            },
            programs: ProgramInputV1 {
                ingress_elf_sha256: "50".repeat(32),
                ingress_program_vkey: format!("0x{}", "51".repeat(32)),
                egress_elf_sha256: "52".repeat(32),
                egress_program_vkey: format!("0x{}", "53".repeat(32)),
                proof_encoding: NAV_SP1_PROOF_ENCODING_GROTH16.to_string(),
                max_proof_bytes: 4_096,
                max_public_values_bytes: 16_384,
            },
            contracts: ContractInputV1 {
                finality_verifier_runtime_code_hash: format!("0x{}", "56".repeat(32)),
                vault_runtime_code_hash: format!("0x{}", "54".repeat(32)),
                ingress_anchor_runtime_code_hash: format!("0x{}", "55".repeat(32)),
                compiler: "0.8.24+commit.e11b9ed9".to_string(),
                optimizer_runs: 200,
                evm_version: "cancun".to_string(),
                metadata_bytecode_hash: "ipfs".to_string(),
            },
            contract_artifacts: Vec::new(),
        }
    }

    #[test]
    fn manifest_derives_cross_bound_route_and_create_addresses() {
        let output = build_manifest(&input()).expect("build manifest");
        assert_eq!(
            output["deployment_sequence"]["ethereum"][0]["address"],
            "0x89ec019b4aa5423b8d96152a502a0db52cf48164"
        );
        assert_eq!(
            output["deployment_sequence"]["arbitrum"][1]["address"],
            "0xa796dc3c9308f9c855a0659153b7afc2006cf27b"
        );
        assert_eq!(
            output["route_profile"]["profile"]["verifier_policy_hash"],
            output["ingress_policy"]["policy_hash"]
        );
        assert_eq!(
            output["nav_profile"]["vault_bridge_route_policy_hash"],
            output["route_profile"]["profile_hash"]
        );
        assert_eq!(
            output["invariants"]["observer_attestations_required"],
            false
        );
        assert_eq!(
            output["invariants"]["observer_confirmations_required"],
            false
        );
    }

    #[test]
    fn manifest_rejects_mixed_network_binding() {
        let mut input = input();
        input.network.arbitrum_chain_id = 42_161;
        let error = build_manifest(&input).expect_err("mixed Sepolia/mainnet binding must fail");
        assert!(error
            .to_string()
            .contains("supported Ethereum/Arbitrum Sepolia binding"));
    }

    #[test]
    fn manifest_accepts_honest_checkpoint_pinned_after_route_activation() {
        let mut input = input();
        input.pftl.initial_finalized_height = 25;
        input.route.activation_height = 20;

        let pinned = build_manifest(&input).expect("route remains live at pinned checkpoint");
        assert_eq!(pinned["route_profile"]["profile"]["activation_height"], 20);
        assert_eq!(pinned["pftl"]["initial_finalized_height"], 25);
    }

    #[test]
    fn manifest_rejects_route_expired_at_pinned_checkpoint() {
        let mut input = input();
        input.pftl.initial_finalized_height = 25;
        input.route.activation_height = 20;
        input.route.expires_at_height = 25;

        let error = build_manifest(&input).expect_err("expired pinned route must fail");
        assert!(error
            .to_string()
            .contains("route epoch/lifetime is inconsistent"));
    }
}
