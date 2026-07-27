use super::*;
use alloy_rlp::Encodable;
use alloy_sol_types::sol;
use clap::Args;
use pfusdc_ingress_program::bonded::{
    verify_bonded_age_release_witness_v1, verify_bonded_confirmation_witness_v1,
    verify_bonded_ingress_witness_v1,
    BondedAssertionPathItemV1, NitroAssertionConfigWitnessV1, PfUsdcBondedConfirmationWitnessV1,
    PfUsdcBondedAgeReleaseWitnessV1, PfUsdcBondedIngressPolicyV1,
    PfUsdcBondedIngressWitnessV1, PfUsdcBondedReversionWitnessV1,
};
use pfusdc_ingress_program::{
    NITRO_LATEST_CONFIRMED_STORAGE_SLOT, PFUSDC_INGRESS_PROOF_POLICY_SCHEMA_V2,
};
use postfiat_types::{
    EthereumArbitrumFinalityStateV2, VaultBridgeRouteProfileV1,
    NAV_PROFILE_VERIFIER_SP1_ARBITRUM_BONDED_V1, NAV_PROFILE_VERIFIER_SP1_ARBITRUM_FINALITY_V1,
};

sol! {
    function stakerCount() external view returns (uint64);
    function getStakerAddress(uint64 stakerNum) external view returns (address);
    function latestStakedAssertion(address staker) external view returns (bytes32);
    function latestConfirmed() external view returns (bytes32);
}

#[derive(Debug, Clone, Args)]
pub struct BondedIngressCaptureArgs {
    #[arg(long)]
    pub route_profile: PathBuf,
    #[arg(long)]
    pub policy: PathBuf,
    #[arg(long)]
    pub prior_finality_state: PathBuf,
    #[arg(long)]
    pub ethereum_rpc: String,
    #[arg(long)]
    pub ethereum_consensus_rpc: String,
    #[arg(long)]
    pub arbitrum_rpc: String,
    #[arg(long)]
    pub deposit_tx: String,
    #[arg(long)]
    pub pftl_chain_id: String,
    #[arg(long)]
    pub pftl_genesis_hash: String,
    #[arg(long)]
    pub pftl_protocol_version: u32,
    #[arg(long)]
    pub output: PathBuf,
}

#[derive(Debug, Clone, Args)]
pub struct BondedConfirmationCaptureArgs {
    #[arg(long)]
    pub route_profile: PathBuf,
    #[arg(long)]
    pub policy: PathBuf,
    #[arg(long)]
    pub prior_finality_state: PathBuf,
    #[arg(long)]
    pub ethereum_rpc: String,
    #[arg(long)]
    pub ethereum_consensus_rpc: String,
    #[arg(long)]
    pub source_assertion_id: String,
    #[arg(long)]
    pub pftl_chain_id: String,
    #[arg(long)]
    pub pftl_genesis_hash: String,
    #[arg(long)]
    pub pftl_protocol_version: u32,
    #[arg(long)]
    pub output: PathBuf,
}

#[derive(Debug, Clone, Args)]
pub struct BondedReversionCaptureArgs {
    #[arg(long)]
    pub route_profile: PathBuf,
    #[arg(long)]
    pub policy: PathBuf,
    #[arg(long)]
    pub prior_finality_state: PathBuf,
    #[arg(long)]
    pub ethereum_rpc: String,
    #[arg(long)]
    pub ethereum_consensus_rpc: String,
    #[arg(long)]
    pub source_assertion_id: String,
    #[arg(long)]
    pub common_ancestor_assertion_id: String,
    #[arg(long)]
    pub pftl_chain_id: String,
    #[arg(long)]
    pub pftl_genesis_hash: String,
    #[arg(long)]
    pub pftl_protocol_version: u32,
    #[arg(long)]
    pub output: PathBuf,
}

#[derive(Debug, Clone, Args)]
pub struct BondedAgeReleaseCaptureArgs {
    #[arg(long)]
    pub ingress_witness: PathBuf,
    #[arg(long)]
    pub prior_finality_state: PathBuf,
    #[arg(long)]
    pub ethereum_rpc: String,
    #[arg(long)]
    pub ethereum_consensus_rpc: String,
    #[arg(long)]
    pub source_assertion_id: String,
    #[arg(long)]
    pub output: PathBuf,
}

fn validate_bonded_route_config(
    route_profile: &VaultBridgeRouteProfileV1,
    policy: &PfUsdcBondedIngressPolicyV1,
    finality_state: &EthereumArbitrumFinalityStateV2,
    profile_hash: &str,
) -> Result<()> {
    let config = finality_state
        .fast_ingress_verifier
        .as_ref()
        .context("finality state has no governed fast-ingress verifier")?;
    anyhow::ensure!(
        route_profile.verifier_kind == NAV_PROFILE_VERIFIER_SP1_ARBITRUM_FINALITY_V1
            && finality_state.route_profile_hash == profile_hash
            && finality_state.route_epoch == u64::from(route_profile.route_epoch)
            && config.base_route_profile_hash == profile_hash
            && config.route_epoch == u64::from(route_profile.route_epoch)
            && config.verifier_kind == NAV_PROFILE_VERIFIER_SP1_ARBITRUM_BONDED_V1
            && config.verifier_policy_hash
                == pfusdc_ingress_program::bonded::bonded_ingress_policy_hash_v1(policy)
            // PFTL canonical hex fields are lowercase and unprefixed; the
            // policy's B256 serde form is 0x-prefixed, but its governed public
            // value and FastIngressVerifierConfigV1 are not.
            && config.deployment_manifest_hash == format!("{:x}", policy.deployment_manifest_hash)
            && config.asset_id == policy.asset_id
            && config.cap_atoms == policy.cap_atoms
            && config.age_margin_blocks == policy.age_margin_blocks,
        "bonded policy is not exactly bound to the secondary verifier authority on the base Tier-4 route"
    );
    Ok(())
}

pub async fn capture(args: BondedIngressCaptureArgs) -> Result<()> {
    let route_profile: VaultBridgeRouteProfileV1 = read_json(&args.route_profile)?;
    let policy: PfUsdcBondedIngressPolicyV1 = read_json(&args.policy)?;
    let finality_state: EthereumArbitrumFinalityStateV2 = read_json(&args.prior_finality_state)?;
    route_profile.validate().map_err(|error| anyhow!(error))?;
    finality_state.validate().map_err(|error| anyhow!(error))?;
    let profile_hash = route_profile
        .profile_hash()
        .map_err(|error| anyhow!(error))?;
    validate_bonded_route_config(&route_profile, &policy, &finality_state, &profile_hash)?;

    let rpc = RpcClient::new()?;
    let deposit_tx: B256 = args
        .deposit_tx
        .parse()
        .context("--deposit-tx must be an EVM bytes32 hash")?;
    let receipt: Value = rpc
        .call(
            &args.arbitrum_rpc,
            "eth_getTransactionReceipt",
            json!([deposit_tx]),
        )
        .await?;
    ensure_successful_receipt(&receipt, deposit_tx)?;
    let deposit_l2_block_number = parse_quantity_value(
        receipt
            .get("blockNumber")
            .context("deposit receipt is missing blockNumber")?,
    )?;
    let confirmed_policy = host_finality_policy(&policy, &finality_state)?;
    let (evidence, _, _) = decode_deposit_receipt(&receipt, deposit_tx, &confirmed_policy)?;

    let helios = capture_helios_inputs(
        &rpc,
        &confirmed_policy,
        &args.ethereum_consensus_rpc,
        Some(&finality_state.latest),
    )
    .await?;
    let verified_finality = verify_helios_inputs_host(&helios, &confirmed_policy)?;
    let ethereum_block = quantity(verified_finality.execution_block_number);

    let count_bytes = eth_call(
        &rpc,
        &args.ethereum_rpc,
        policy.arbitrum_rollup_address,
        stakerCountCall {}.abi_encode(),
        &ethereum_block,
    )
    .await?;
    let count = stakerCountCall::abi_decode_returns(&count_bytes).context("decode stakerCount")?;
    anyhow::ensure!(
        count == 1,
        "bonded ingress currently requires one active Rollup staker"
    );
    let staker_bytes = eth_call(
        &rpc,
        &args.ethereum_rpc,
        policy.arbitrum_rollup_address,
        getStakerAddressCall { stakerNum: 0 }.abi_encode(),
        &ethereum_block,
    )
    .await?;
    let source_staker = getStakerAddressCall::abi_decode_returns(&staker_bytes)
        .context("decode getStakerAddress")?;
    let latest_bytes = eth_call(
        &rpc,
        &args.ethereum_rpc,
        policy.arbitrum_rollup_address,
        latestStakedAssertionCall {
            staker: source_staker,
        }
        .abi_encode(),
        &ethereum_block,
    )
    .await?;
    let source_assertion_id = latestStakedAssertionCall::abi_decode_returns(&latest_bytes)
        .context("decode latestStakedAssertion")?;
    let confirmed_bytes = eth_call(
        &rpc,
        &args.ethereum_rpc,
        policy.arbitrum_rollup_address,
        latestConfirmedCall {}.abi_encode(),
        &ethereum_block,
    )
    .await?;
    let latest_confirmed = latestConfirmedCall::abi_decode_returns(&confirmed_bytes)
        .context("decode latestConfirmed")?;
    anyhow::ensure!(
        latest_confirmed != B256::ZERO,
        "Rollup latestConfirmed is zero"
    );

    let mut reversed = Vec::new();
    let mut cursor = source_assertion_id;
    while cursor != latest_confirmed {
        anyhow::ensure!(
            reversed.len() < 256,
            "assertion path exceeds the guest's bounded maximum"
        );
        let item = capture_bonded_assertion_item(
            &rpc,
            &args.ethereum_rpc,
            policy.arbitrum_rollup_address,
            cursor,
            &ethereum_block,
        )
        .await?;
        cursor = item.assertion.parent_assertion_hash;
        reversed.push(item);
    }
    anyhow::ensure!(
        !reversed.is_empty(),
        "source assertion is already confirmed"
    );
    reversed.reverse();
    let assertion_path = reversed;
    let assertion = &assertion_path
        .last()
        .ok_or_else(|| anyhow!("empty assertion path"))?
        .assertion;

    let staker_base = address_mapping_slot(source_staker, policy.rollup_staker_map_slot);
    let staker_list_item = keccak256(policy.rollup_staker_list_slot.as_slice());
    let mut rollup_slots = vec![
        policy.rollup_primary_implementation_slot,
        policy.rollup_secondary_implementation_slot,
        policy.rollup_paused_storage_slot,
        policy.rollup_chain_config_storage_slot,
        policy.rollup_assertion_config_storage_slot,
        policy.rollup_base_stake_storage_slot,
        policy.rollup_wasm_module_root_storage_slot,
        policy.rollup_challenge_config_storage_slot,
        policy.rollup_stake_token_storage_slot,
        policy.rollup_latest_confirmed_storage_slot,
        policy.rollup_staker_list_slot,
        staker_list_item,
        staker_base,
        add_slot(staker_base, 1)?,
        add_slot(staker_base, 2)?,
        policy.rollup_validator_policy_storage_slot,
    ];
    let confirmed_base = mapping_slot(latest_confirmed, policy.rollup_assertions_mapping_slot);
    rollup_slots.push(confirmed_base);
    rollup_slots.push(add_slot(confirmed_base, 1)?);
    for item in &assertion_path {
        let assertion_id = pfusdc_ingress_program::nitro_assertion_hash(&item.assertion);
        let base = mapping_slot(assertion_id, policy.rollup_assertions_mapping_slot);
        rollup_slots.push(base);
        rollup_slots.push(add_slot(base, 1)?);
    }
    let rollup_storage = get_account_proof(
        &rpc,
        &args.ethereum_rpc,
        policy.arbitrum_rollup_address,
        &rollup_slots,
        &ethereum_block,
    )
    .await?;
    let rollup_admin_implementation_account = get_account_proof(
        &rpc,
        &args.ethereum_rpc,
        policy.rollup_admin_implementation_address,
        &[],
        &ethereum_block,
    )
    .await?;
    let rollup_user_implementation_account = get_account_proof(
        &rpc,
        &args.ethereum_rpc,
        policy.rollup_user_implementation_address,
        &[],
        &ethereum_block,
    )
    .await?;
    let challenge_manager_account = get_account_proof(
        &rpc,
        &args.ethereum_rpc,
        policy.challenge_manager_address,
        &[policy.challenge_manager_implementation_slot],
        &ethereum_block,
    )
    .await?;
    let challenge_manager_implementation_account = get_account_proof(
        &rpc,
        &args.ethereum_rpc,
        policy.challenge_manager_implementation_address,
        &[],
        &ethereum_block,
    )
    .await?;

    let asserted_block: Block = rpc
        .call(
            &args.arbitrum_rpc,
            "eth_getBlockByHash",
            json!([assertion.block_hash, false]),
        )
        .await?;
    anyhow::ensure!(
        asserted_block.header.hash == assertion.block_hash
            && asserted_block.header.inner.hash_slow() == assertion.block_hash,
        "Arbitrum RPC returned the wrong asserted block"
    );
    anyhow::ensure!(
        asserted_block.header.inner.number >= deposit_l2_block_number,
        "newest Ethereum-finalized bonded assertion covers Arbitrum block {}, before deposit block {}; wait for a descendant assertion that covers the deposit",
        asserted_block.header.inner.number,
        deposit_l2_block_number
    );
    let mut asserted_l2_header_rlp = Vec::new();
    asserted_block
        .header
        .inner
        .encode(&mut asserted_l2_header_rlp);
    let asserted_block_number = quantity(asserted_block.header.inner.number);

    // The transaction receipt is discovery/debug metadata only. Authorization
    // comes exclusively from depositSeen[deposit_id] under the assertion-bound
    // L2 state root, so preserve the real transaction coordinates here rather
    // than manufacturing a receipt-shaped identifier for the asserted block.
    evidence.validate().map_err(|error| anyhow!(error))?;
    let deposit_id = parse_hex32(&evidence.deposit_id)?;
    let deposit_seen_slot = mapping_slot(deposit_id, policy.vault_deposit_seen_mapping_slot);
    let asserted_l2_vault_account = get_account_proof(
        &rpc,
        &args.arbitrum_rpc,
        policy.arbitrum_vault_address,
        &[deposit_seen_slot],
        &asserted_block_number,
    )
    .await?;
    let asserted_l2_token_account = get_account_proof(
        &rpc,
        &args.arbitrum_rpc,
        policy.arbitrum_token_address,
        &[],
        &asserted_block_number,
    )
    .await?;

    let witness = PfUsdcBondedIngressWitnessV1 {
        schema: pfusdc_ingress_program::bonded::PFUSDC_BONDED_INGRESS_WITNESS_SCHEMA_V1.to_string(),
        route_profile,
        policy,
        helios,
        rollup_storage,
        rollup_admin_implementation_account,
        rollup_user_implementation_account,
        challenge_manager_account,
        challenge_manager_implementation_account,
        source_staker,
        assertion_path,
        asserted_l2_header_rlp: Bytes::from(asserted_l2_header_rlp),
        asserted_l2_vault_account,
        asserted_l2_token_account,
        evidence,
        pftl_chain_id: args.pftl_chain_id,
        pftl_genesis_hash: args.pftl_genesis_hash,
        pftl_protocol_version: args.pftl_protocol_version,
    };
    let values = verify_bonded_ingress_witness_v1(&witness)
        .map_err(|error| anyhow!("captured bonded witness failed native verification: {error}"))?;
    let mut advanced = finality_state;
    advanced
        .verify_and_advance_bonded(&values)
        .map_err(|error| anyhow!("bonded witness cannot advance finality state: {error}"))?;
    write_new_json(&args.output, &witness)?;
    println!(
        "captured bonded assertion {} at L1 block {} covering L2 block {}",
        values.source_assertion_id,
        values.source_assertion_created_at_l1_block,
        values.source_assertion_l2_block_hash
    );
    Ok(())
}

pub async fn capture_age_release(args: BondedAgeReleaseCaptureArgs) -> Result<()> {
    let mut branch_witness: PfUsdcBondedIngressWitnessV1 = read_json(&args.ingress_witness)?;
    let finality_state: EthereumArbitrumFinalityStateV2 = read_json(&args.prior_finality_state)?;
    let source_assertion_id: B256 = args
        .source_assertion_id
        .parse()
        .context("--source-assertion-id must be bytes32")?;
    let profile_hash = branch_witness
        .route_profile
        .profile_hash()
        .map_err(|error| anyhow!(error))?;
    validate_bonded_route_config(
        &branch_witness.route_profile,
        &branch_witness.policy,
        &finality_state,
        &profile_hash,
    )?;
    let rpc = RpcClient::new()?;
    let confirmed_policy = host_finality_policy(&branch_witness.policy, &finality_state)?;
    let helios = capture_helios_inputs(
        &rpc,
        &confirmed_policy,
        &args.ethereum_consensus_rpc,
        Some(&finality_state.latest),
    )
    .await?;
    let verified_finality = verify_helios_inputs_host(&helios, &confirmed_policy)?;
    let ethereum_block = quantity(verified_finality.execution_block_number);
    let policy = &branch_witness.policy;

    let count_bytes = eth_call(
        &rpc,
        &args.ethereum_rpc,
        policy.arbitrum_rollup_address,
        stakerCountCall {}.abi_encode(),
        &ethereum_block,
    )
    .await?;
    let count = stakerCountCall::abi_decode_returns(&count_bytes).context("decode stakerCount")?;
    anyhow::ensure!(count == 1, "age release requires one active Rollup staker");
    let staker_bytes = eth_call(
        &rpc,
        &args.ethereum_rpc,
        policy.arbitrum_rollup_address,
        getStakerAddressCall { stakerNum: 0 }.abi_encode(),
        &ethereum_block,
    )
    .await?;
    let source_staker = getStakerAddressCall::abi_decode_returns(&staker_bytes)
        .context("decode getStakerAddress")?;
    let latest_staked_bytes = eth_call(
        &rpc,
        &args.ethereum_rpc,
        policy.arbitrum_rollup_address,
        latestStakedAssertionCall {
            staker: source_staker,
        }
        .abi_encode(),
        &ethereum_block,
    )
    .await?;
    let latest_staked = latestStakedAssertionCall::abi_decode_returns(&latest_staked_bytes)
        .context("decode latestStakedAssertion")?;
    let confirmed_bytes = eth_call(
        &rpc,
        &args.ethereum_rpc,
        policy.arbitrum_rollup_address,
        latestConfirmedCall {}.abi_encode(),
        &ethereum_block,
    )
    .await?;
    let latest_confirmed = latestConfirmedCall::abi_decode_returns(&confirmed_bytes)
        .context("decode latestConfirmed")?;
    anyhow::ensure!(latest_confirmed != B256::ZERO, "Rollup latestConfirmed is zero");

    let mut reversed = Vec::new();
    let mut cursor = latest_staked;
    while cursor != latest_confirmed {
        anyhow::ensure!(
            reversed.len() < 256,
            "age-release assertion path exceeds guest bound"
        );
        let item = capture_bonded_assertion_item(
            &rpc,
            &args.ethereum_rpc,
            policy.arbitrum_rollup_address,
            cursor,
            &ethereum_block,
        )
        .await?;
        cursor = item.assertion.parent_assertion_hash;
        reversed.push(item);
    }
    reversed.reverse();
    anyhow::ensure!(
        reversed.iter().any(|item| {
            pfusdc_ingress_program::nitro_assertion_hash(&item.assertion)
                == source_assertion_id
        }),
        "source assertion is no longer on the unique live bonded branch"
    );
    let latest_confirmed_assertion = capture_bonded_assertion_item(
        &rpc,
        &args.ethereum_rpc,
        policy.arbitrum_rollup_address,
        latest_confirmed,
        &ethereum_block,
    )
    .await?
    .assertion;

    let staker_base = address_mapping_slot(source_staker, policy.rollup_staker_map_slot);
    let staker_list_item = keccak256(policy.rollup_staker_list_slot.as_slice());
    let mut rollup_slots = vec![
        policy.rollup_primary_implementation_slot,
        policy.rollup_secondary_implementation_slot,
        policy.rollup_paused_storage_slot,
        policy.rollup_chain_config_storage_slot,
        policy.rollup_assertion_config_storage_slot,
        policy.rollup_base_stake_storage_slot,
        policy.rollup_wasm_module_root_storage_slot,
        policy.rollup_challenge_config_storage_slot,
        policy.rollup_stake_token_storage_slot,
        policy.rollup_latest_confirmed_storage_slot,
        policy.rollup_staker_list_slot,
        staker_list_item,
        staker_base,
        add_slot(staker_base, 1)?,
        add_slot(staker_base, 2)?,
        policy.rollup_validator_policy_storage_slot,
    ];
    let confirmed_base = mapping_slot(latest_confirmed, policy.rollup_assertions_mapping_slot);
    rollup_slots.push(confirmed_base);
    rollup_slots.push(add_slot(confirmed_base, 1)?);
    for item in &reversed {
        let assertion_id = pfusdc_ingress_program::nitro_assertion_hash(&item.assertion);
        let base = mapping_slot(assertion_id, policy.rollup_assertions_mapping_slot);
        rollup_slots.push(base);
        rollup_slots.push(add_slot(base, 1)?);
    }
    let rollup_storage = get_account_proof(
        &rpc,
        &args.ethereum_rpc,
        policy.arbitrum_rollup_address,
        &rollup_slots,
        &ethereum_block,
    )
    .await?;
    let rollup_admin_implementation_account = get_account_proof(
        &rpc,
        &args.ethereum_rpc,
        policy.rollup_admin_implementation_address,
        &[],
        &ethereum_block,
    )
    .await?;
    let rollup_user_implementation_account = get_account_proof(
        &rpc,
        &args.ethereum_rpc,
        policy.rollup_user_implementation_address,
        &[],
        &ethereum_block,
    )
    .await?;
    let challenge_manager_account = get_account_proof(
        &rpc,
        &args.ethereum_rpc,
        policy.challenge_manager_address,
        &[policy.challenge_manager_implementation_slot],
        &ethereum_block,
    )
    .await?;
    let challenge_manager_implementation_account = get_account_proof(
        &rpc,
        &args.ethereum_rpc,
        policy.challenge_manager_implementation_address,
        &[],
        &ethereum_block,
    )
    .await?;

    branch_witness.helios = helios;
    branch_witness.rollup_storage = rollup_storage;
    branch_witness.rollup_admin_implementation_account = rollup_admin_implementation_account;
    branch_witness.rollup_user_implementation_account = rollup_user_implementation_account;
    branch_witness.challenge_manager_account = challenge_manager_account;
    branch_witness.challenge_manager_implementation_account =
        challenge_manager_implementation_account;
    branch_witness.source_staker = source_staker;
    branch_witness.assertion_path = reversed;
    let witness = PfUsdcBondedAgeReleaseWitnessV1 {
        schema: pfusdc_ingress_program::bonded::PFUSDC_BONDED_INGRESS_WITNESS_SCHEMA_V1.to_string(),
        branch_witness,
        source_assertion_id,
        latest_confirmed_assertion,
    };
    let values = verify_bonded_age_release_witness_v1(&witness)
        .map_err(|error| anyhow!("captured age release failed native verification: {error}"))?;
    write_new_json(&args.output, &witness)?;
    println!(
        "captured age release for assertion {} at age {} blocks (threshold {})",
        values.source_assertion_id,
        values.source_assertion_age_blocks,
        values.age_release_after_blocks
    );
    Ok(())
}

pub async fn capture_confirmation(args: BondedConfirmationCaptureArgs) -> Result<()> {
    let route_profile: VaultBridgeRouteProfileV1 = read_json(&args.route_profile)?;
    let policy: PfUsdcBondedIngressPolicyV1 = read_json(&args.policy)?;
    let finality_state: EthereumArbitrumFinalityStateV2 = read_json(&args.prior_finality_state)?;
    let source_assertion_id: B256 = args
        .source_assertion_id
        .parse()
        .context("--source-assertion-id must be bytes32")?;
    let profile_hash = route_profile
        .profile_hash()
        .map_err(|error| anyhow!(error))?;
    validate_bonded_route_config(&route_profile, &policy, &finality_state, &profile_hash)?;
    let rpc = RpcClient::new()?;
    let confirmed_policy = host_finality_policy(&policy, &finality_state)?;
    let helios = capture_helios_inputs(
        &rpc,
        &confirmed_policy,
        &args.ethereum_consensus_rpc,
        Some(&finality_state.latest),
    )
    .await?;
    let verified_finality = verify_helios_inputs_host(&helios, &confirmed_policy)?;
    let ethereum_block = quantity(verified_finality.execution_block_number);
    let confirmed_bytes = eth_call(
        &rpc,
        &args.ethereum_rpc,
        policy.arbitrum_rollup_address,
        latestConfirmedCall {}.abi_encode(),
        &ethereum_block,
    )
    .await?;
    let latest_confirmed = latestConfirmedCall::abi_decode_returns(&confirmed_bytes)
        .context("decode latestConfirmed")?;
    let mut reversed = Vec::new();
    let mut cursor = latest_confirmed;
    loop {
        anyhow::ensure!(
            reversed.len() < 256,
            "source assertion is not within the bounded latestConfirmed ancestry"
        );
        let item = capture_bonded_assertion_item(
            &rpc,
            &args.ethereum_rpc,
            policy.arbitrum_rollup_address,
            cursor,
            &ethereum_block,
        )
        .await?;
        let parent = item.assertion.parent_assertion_hash;
        reversed.push(item.assertion);
        if cursor == source_assertion_id {
            break;
        }
        cursor = parent;
    }
    reversed.reverse();
    let confirmation_path = reversed;
    let confirmed_base = mapping_slot(latest_confirmed, policy.rollup_assertions_mapping_slot);
    let rollup_slots = vec![
        policy.rollup_primary_implementation_slot,
        policy.rollup_secondary_implementation_slot,
        policy.rollup_paused_storage_slot,
        policy.rollup_chain_config_storage_slot,
        policy.rollup_assertion_config_storage_slot,
        policy.rollup_base_stake_storage_slot,
        policy.rollup_wasm_module_root_storage_slot,
        policy.rollup_challenge_config_storage_slot,
        policy.rollup_stake_token_storage_slot,
        policy.rollup_latest_confirmed_storage_slot,
        policy.rollup_validator_policy_storage_slot,
        confirmed_base,
        add_slot(confirmed_base, 1)?,
    ];
    let rollup_storage = get_account_proof(
        &rpc,
        &args.ethereum_rpc,
        policy.arbitrum_rollup_address,
        &rollup_slots,
        &ethereum_block,
    )
    .await?;
    let rollup_admin_implementation_account = get_account_proof(
        &rpc,
        &args.ethereum_rpc,
        policy.rollup_admin_implementation_address,
        &[],
        &ethereum_block,
    )
    .await?;
    let rollup_user_implementation_account = get_account_proof(
        &rpc,
        &args.ethereum_rpc,
        policy.rollup_user_implementation_address,
        &[],
        &ethereum_block,
    )
    .await?;
    let challenge_manager_account = get_account_proof(
        &rpc,
        &args.ethereum_rpc,
        policy.challenge_manager_address,
        &[policy.challenge_manager_implementation_slot],
        &ethereum_block,
    )
    .await?;
    let challenge_manager_implementation_account = get_account_proof(
        &rpc,
        &args.ethereum_rpc,
        policy.challenge_manager_implementation_address,
        &[],
        &ethereum_block,
    )
    .await?;
    let witness = PfUsdcBondedConfirmationWitnessV1 {
        schema: pfusdc_ingress_program::bonded::PFUSDC_BONDED_INGRESS_WITNESS_SCHEMA_V1.to_string(),
        route_profile,
        policy,
        helios,
        rollup_storage,
        rollup_admin_implementation_account,
        rollup_user_implementation_account,
        challenge_manager_account,
        challenge_manager_implementation_account,
        source_assertion_id,
        confirmation_path,
        pftl_chain_id: args.pftl_chain_id,
        pftl_genesis_hash: args.pftl_genesis_hash,
        pftl_protocol_version: args.pftl_protocol_version,
    };
    let values = verify_bonded_confirmation_witness_v1(&witness)
        .map_err(|error| anyhow!("captured confirmation failed native verification: {error}"))?;
    let mut advanced = finality_state;
    advanced
        .verify_and_advance_bonded_lifecycle(&values)
        .map_err(|error| anyhow!("confirmation cannot advance finality state: {error}"))?;
    write_new_json(&args.output, &witness)?;
    println!(
        "captured source assertion {} confirmed by {}",
        values.source_assertion_id, values.latest_confirmed_assertion_id
    );
    Ok(())
}

pub async fn capture_reversion(args: BondedReversionCaptureArgs) -> Result<()> {
    let route_profile: VaultBridgeRouteProfileV1 = read_json(&args.route_profile)?;
    let policy: PfUsdcBondedIngressPolicyV1 = read_json(&args.policy)?;
    let finality_state: EthereumArbitrumFinalityStateV2 = read_json(&args.prior_finality_state)?;
    let source_assertion_id: B256 = args
        .source_assertion_id
        .parse()
        .context("--source-assertion-id must be bytes32")?;
    let common_ancestor: B256 = args
        .common_ancestor_assertion_id
        .parse()
        .context("--common-ancestor-assertion-id must be bytes32")?;
    let profile_hash = route_profile
        .profile_hash()
        .map_err(|error| anyhow!(error))?;
    validate_bonded_route_config(&route_profile, &policy, &finality_state, &profile_hash)?;
    let rpc = RpcClient::new()?;
    let confirmed_policy = host_finality_policy(&policy, &finality_state)?;
    let helios = capture_helios_inputs(
        &rpc,
        &confirmed_policy,
        &args.ethereum_consensus_rpc,
        Some(&finality_state.latest),
    )
    .await?;
    let verified_finality = verify_helios_inputs_host(&helios, &confirmed_policy)?;
    let ethereum_block = quantity(verified_finality.execution_block_number);
    let confirmed_bytes = eth_call(
        &rpc,
        &args.ethereum_rpc,
        policy.arbitrum_rollup_address,
        latestConfirmedCall {}.abi_encode(),
        &ethereum_block,
    )
    .await?;
    let latest_confirmed = latestConfirmedCall::abi_decode_returns(&confirmed_bytes)
        .context("decode latestConfirmed")?;
    let source_path = capture_path_after_ancestor(
        &rpc,
        &args.ethereum_rpc,
        policy.arbitrum_rollup_address,
        source_assertion_id,
        common_ancestor,
        &ethereum_block,
    )
    .await?;
    let winning_path = capture_path_after_ancestor(
        &rpc,
        &args.ethereum_rpc,
        policy.arbitrum_rollup_address,
        latest_confirmed,
        common_ancestor,
        &ethereum_block,
    )
    .await?;
    anyhow::ensure!(
        pfusdc_ingress_program::nitro_assertion_hash(&source_path[0])
            != pfusdc_ingress_program::nitro_assertion_hash(&winning_path[0]),
        "source and current confirmed paths did not diverge"
    );
    let confirmed_base = mapping_slot(latest_confirmed, policy.rollup_assertions_mapping_slot);
    let rollup_slots = vec![
        policy.rollup_primary_implementation_slot,
        policy.rollup_secondary_implementation_slot,
        policy.rollup_paused_storage_slot,
        policy.rollup_chain_config_storage_slot,
        policy.rollup_assertion_config_storage_slot,
        policy.rollup_base_stake_storage_slot,
        policy.rollup_wasm_module_root_storage_slot,
        policy.rollup_challenge_config_storage_slot,
        policy.rollup_stake_token_storage_slot,
        policy.rollup_latest_confirmed_storage_slot,
        policy.rollup_validator_policy_storage_slot,
        confirmed_base,
        add_slot(confirmed_base, 1)?,
    ];
    let rollup_storage = get_account_proof(
        &rpc,
        &args.ethereum_rpc,
        policy.arbitrum_rollup_address,
        &rollup_slots,
        &ethereum_block,
    )
    .await?;
    let rollup_admin_implementation_account = get_account_proof(
        &rpc,
        &args.ethereum_rpc,
        policy.rollup_admin_implementation_address,
        &[],
        &ethereum_block,
    )
    .await?;
    let rollup_user_implementation_account = get_account_proof(
        &rpc,
        &args.ethereum_rpc,
        policy.rollup_user_implementation_address,
        &[],
        &ethereum_block,
    )
    .await?;
    let challenge_manager_account = get_account_proof(
        &rpc,
        &args.ethereum_rpc,
        policy.challenge_manager_address,
        &[policy.challenge_manager_implementation_slot],
        &ethereum_block,
    )
    .await?;
    let challenge_manager_implementation_account = get_account_proof(
        &rpc,
        &args.ethereum_rpc,
        policy.challenge_manager_implementation_address,
        &[],
        &ethereum_block,
    )
    .await?;
    let witness = PfUsdcBondedReversionWitnessV1 {
        schema: pfusdc_ingress_program::bonded::PFUSDC_BONDED_INGRESS_WITNESS_SCHEMA_V1.to_string(),
        route_profile,
        policy,
        helios,
        rollup_storage,
        rollup_admin_implementation_account,
        rollup_user_implementation_account,
        challenge_manager_account,
        challenge_manager_implementation_account,
        source_assertion_id,
        source_path,
        winning_path,
        pftl_chain_id: args.pftl_chain_id,
        pftl_genesis_hash: args.pftl_genesis_hash,
        pftl_protocol_version: args.pftl_protocol_version,
    };
    let values = pfusdc_ingress_program::bonded::verify_bonded_reversion_witness_v1(&witness)
        .map_err(|error| anyhow!("captured reversion failed native verification: {error}"))?;
    let mut advanced = finality_state;
    advanced
        .verify_and_advance_bonded_lifecycle(&values)
        .map_err(|error| anyhow!("reversion cannot advance finality state: {error}"))?;
    write_new_json(&args.output, &witness)?;
    println!(
        "captured source assertion {} reverted by canonical assertion {}",
        values.source_assertion_id, values.latest_confirmed_assertion_id
    );
    Ok(())
}

async fn capture_path_after_ancestor(
    rpc: &RpcClient,
    url: &str,
    rollup: Address,
    leaf: B256,
    ancestor: B256,
    block: &str,
) -> Result<Vec<NitroAssertionWitnessV1>> {
    anyhow::ensure!(leaf != ancestor, "path leaf equals common ancestor");
    let mut reversed = Vec::new();
    let mut cursor = leaf;
    while cursor != ancestor {
        anyhow::ensure!(
            reversed.len() < 256,
            "assertion ancestry exceeds guest bound"
        );
        let item = capture_bonded_assertion_item(rpc, url, rollup, cursor, block).await?;
        cursor = item.assertion.parent_assertion_hash;
        reversed.push(item.assertion);
    }
    reversed.reverse();
    Ok(reversed)
}

async fn capture_bonded_assertion_item(
    rpc: &RpcClient,
    url: &str,
    rollup: Address,
    assertion_hash: B256,
    block: &str,
) -> Result<BondedAssertionPathItemV1> {
    let creation_call = getAssertionCreationBlockForLogLookupCall {
        assertionHash: assertion_hash,
    }
    .abi_encode();
    let creation_bytes = eth_call(rpc, url, rollup, creation_call, block).await?;
    let creation = getAssertionCreationBlockForLogLookupCall::abi_decode_returns(&creation_bytes)
        .context("decode assertion creation block")?;
    let creation_block = quantity_u256(creation);
    let logs: Vec<Value> = rpc
        .call(
            url,
            "eth_getLogs",
            json!([{
                "address": rollup,
                "fromBlock": creation_block,
                "toBlock": creation_block,
                "topics": [AssertionCreated::SIGNATURE_HASH, assertion_hash]
            }]),
        )
        .await?;
    anyhow::ensure!(logs.len() == 1, "expected one AssertionCreated log");
    let (topics, data) = raw_log(&logs[0])?;
    let event = AssertionCreated::decode_raw_log_validate(&topics, &data)
        .context("decode AssertionCreated log")?;
    anyhow::ensure!(event.assertionHash == assertion_hash, "wrong assertion log");
    let config = &event.assertion.beforeStateData.configData;
    Ok(BondedAssertionPathItemV1 {
        assertion: NitroAssertionWitnessV1 {
            parent_assertion_hash: event.parentAssertionHash,
            block_hash: event.assertion.afterState.globalState.bytes32Vals[0],
            send_root: event.assertion.afterState.globalState.bytes32Vals[1],
            inbox_position: event.assertion.afterState.globalState.u64Vals[0],
            position_in_message: event.assertion.afterState.globalState.u64Vals[1],
            machine_status: event.assertion.afterState.machineStatus,
            end_history_root: event.assertion.afterState.endHistoryRoot,
            inbox_accumulator: event.afterInboxBatchAcc,
        },
        parent_config: NitroAssertionConfigWitnessV1 {
            wasm_module_root: config.wasmModuleRoot,
            required_stake: config.requiredStake,
            challenge_manager: config.challengeManager,
            confirm_period_blocks: config.confirmPeriodBlocks,
            next_inbox_position: config.nextInboxPosition,
        },
    })
}

fn host_finality_policy(
    policy: &PfUsdcBondedIngressPolicyV1,
    state: &EthereumArbitrumFinalityStateV2,
) -> Result<PfUsdcIngressProofPolicyV2> {
    Ok(PfUsdcIngressProofPolicyV2 {
        schema: PFUSDC_INGRESS_PROOF_POLICY_SCHEMA_V2.to_string(),
        ethereum_chain_id: policy.ethereum_chain_id,
        ethereum_genesis_validators_root: policy.ethereum_genesis_validators_root,
        arbitrum_chain_id: policy.arbitrum_chain_id,
        arbitrum_rollup_address: policy.arbitrum_rollup_address,
        arbitrum_rollup_runtime_code_hash: policy.arbitrum_rollup_runtime_code_hash,
        rollup_latest_confirmed_storage_slot: NITRO_LATEST_CONFIRMED_STORAGE_SLOT,
        arbitrum_vault_address: policy.arbitrum_vault_address,
        arbitrum_vault_runtime_code_hash: policy.arbitrum_vault_runtime_code_hash,
        arbitrum_token_address: policy.arbitrum_token_address,
        arbitrum_token_runtime_code_hash: policy.arbitrum_token_runtime_code_hash,
        ethereum_ingress_anchor_address: state
            .ethereum_ingress_anchor_address
            .parse()
            .context("decode finality-state anchor address")?,
        ethereum_ingress_anchor_runtime_code_hash: parse_hex32(
            &state.ethereum_ingress_anchor_runtime_code_hash,
        )?,
    })
}

fn mapping_slot(key: B256, base: B256) -> B256 {
    let mut preimage = [0_u8; 64];
    preimage[..32].copy_from_slice(key.as_slice());
    preimage[32..].copy_from_slice(base.as_slice());
    keccak256(preimage)
}

fn address_mapping_slot(key: Address, base: B256) -> B256 {
    let mut padded = [0_u8; 32];
    padded[12..].copy_from_slice(key.as_slice());
    mapping_slot(B256::from(padded), base)
}

fn add_slot(value: B256, offset: u64) -> Result<B256> {
    U256::from_be_bytes(value.0)
        .checked_add(U256::from(offset))
        .map(|sum| B256::from(sum.to_be_bytes::<32>()))
        .ok_or_else(|| anyhow!("storage slot overflow"))
}
