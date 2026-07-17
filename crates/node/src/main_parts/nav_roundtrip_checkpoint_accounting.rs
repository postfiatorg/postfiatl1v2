fn nav_roundtrip_vault_bridge_reserve_accounts(
    profile: &postfiat_types::NavProofProfile,
) -> Result<Vec<String>, String> {
    let source_domain = profile
        .source_class
        .strip_prefix(postfiat_types::VAULT_BRIDGE_PROFILE_SOURCE_CLASS_PREFIX)
        .ok_or_else(|| {
            format!(
                "vault bridge NAV profile source_class `{}` is missing `{}`",
                profile.source_class,
                postfiat_types::VAULT_BRIDGE_PROFILE_SOURCE_CLASS_PREFIX
            )
        })?;
    let parts = source_domain.split(':').collect::<Vec<_>>();
    if parts.len() != 4
        || parts[0] != postfiat_types::VAULT_BRIDGE_DEPOSIT_SOURCE_DOMAIN_PREFIX
        || parts[1].is_empty()
        || parts[2].is_empty()
        || parts[3].is_empty()
    {
        return Err(format!(
            "vault bridge NAV profile source_class must be `vault_bridge:{}:<evm_chain_id>:<vault>:<token>`",
            postfiat_types::VAULT_BRIDGE_DEPOSIT_SOURCE_DOMAIN_PREFIX
        ));
    }
    Ok(vec![format!("evm:{}:{}:{}", parts[1], parts[2], parts[3])])
}

fn nav_roundtrip_checkpoint_attestor_root(
    asset_id: &str,
    epoch: u64,
    source_root: &str,
    verified_net_assets: u64,
) -> String {
    let preimage = format!(
        "asset_id={asset_id}\nepoch={epoch}\nsource_root={source_root}\nverified_net_assets={verified_net_assets}\n"
    );
    hash_hex(
        "postfiat.nav_roundtrip.checkpoint_attestor_root.v1",
        preimage.as_bytes(),
    )
}

fn nav_roundtrip_checkpoint_reserve_packet_hash(
    nav_asset: &postfiat_types::NavTrackedAsset,
    epoch: u64,
    nav_per_unit: u64,
    circulating_supply: u64,
    verified_net_assets: u64,
    source_root: &str,
    attestor_root: &str,
) -> String {
    let preimage = format!(
        "asset_id={}\nissuer={}\nreserve_operator={}\nepoch={epoch}\nnav_per_unit={nav_per_unit}\ncirculating_supply={circulating_supply}\nverified_net_assets={verified_net_assets}\nproof_profile={}\nsource_root={source_root}\nattestor_root={attestor_root}\n",
        nav_asset.asset_id, nav_asset.issuer, nav_asset.reserve_operator, nav_asset.proof_profile,
    );
    hash_hex(
        "postfiat.nav_roundtrip.reserve_packet_hash.v1",
        preimage.as_bytes(),
    )
}

fn nav_roundtrip_find_sp1_base_source_root(
    ledger: &postfiat_types::LedgerState,
    nav_asset: &postfiat_types::NavTrackedAsset,
    proof: &[u8],
    public_values: &[u8],
    base_verified_net_assets: u64,
) -> Result<String, String> {
    ledger
        .nav_reserve_packets
        .iter()
        .filter(|packet| {
            packet.asset_id == nav_asset.asset_id
                && packet.state == postfiat_types::NAV_RESERVE_STATE_FINALIZED
                && packet.verified_net_assets == base_verified_net_assets
                && packet.sp1_proof_bytes == proof
                && packet.sp1_public_values == public_values
        })
        .max_by_key(|packet| packet.epoch)
        .map(|packet| packet.source_root.clone())
        .ok_or_else(|| {
            format!(
                "could not find a finalized base SP1 source_root for NAV asset `{}`",
                nav_asset.asset_id
            )
        })
}

fn nav_roundtrip_subscription_overlay(
    ledger: &postfiat_types::LedgerState,
    nav_asset: &postfiat_types::NavTrackedAsset,
) -> Result<Option<NavRoundtripSubscriptionOverlay>, String> {
    let mut rows = Vec::new();
    for allocation in ledger.vault_bridge_allocations.iter().filter(|allocation| {
        allocation.purpose == postfiat_types::VAULT_BRIDGE_ALLOCATION_PURPOSE_NAV_SUBSCRIPTION
            && nav_roundtrip_nav_subscription_consumer_matches(
                &allocation.consumer_id,
                &nav_asset.asset_id,
            )
            && allocation.retired_at_height != 0
    }) {
        let remaining_atoms = allocation
            .amount_atoms
            .checked_sub(allocation.released_atoms)
            .ok_or_else(|| {
                "nav subscription allocation released_atoms exceeds amount_atoms".to_string()
            })?;
        if remaining_atoms == 0 {
            continue;
        }
        let settlement_nav_asset = ledger
            .nav_asset(&allocation.asset_id)
            .ok_or_else(|| {
                "nav subscription overlay references missing settlement NAV asset".to_string()
            })?;
        let settlement_asset = ledger.asset_definition(&allocation.asset_id).ok_or_else(|| {
            "nav subscription overlay references missing settlement asset definition".to_string()
        })?;
        let bucket = ledger
            .vault_bridge_bucket(&allocation.bucket_id)
            .ok_or_else(|| "nav subscription overlay references missing bucket".to_string())?;
        if bucket.asset_id != allocation.asset_id {
            return Err("nav subscription overlay bucket asset mismatch".to_string());
        }
        if bucket.status != postfiat_types::VAULT_BRIDGE_BUCKET_STATUS_ACTIVE {
            continue;
        }
        let receipt = ledger
            .vault_bridge_receipt(&allocation.receipt_id)
            .ok_or_else(|| "nav subscription overlay references missing receipt".to_string())?;
        if receipt.asset_id != allocation.asset_id
            || receipt.bucket_id != allocation.bucket_id
            || receipt.status != postfiat_types::VAULT_BRIDGE_RECEIPT_STATUS_COUNTED
        {
            return Err(
                "nav subscription overlay allocation must reference a counted receipt in the settlement bucket"
                    .to_string(),
            );
        }
        let value_nav_units = nav_roundtrip_vault_bridge_atoms_to_nav_value(
            remaining_atoms,
            &nav_asset.valuation_unit,
            &settlement_nav_asset.valuation_unit,
            settlement_asset.precision,
        )?;
        rows.push(NavRoundtripSubscriptionOverlayRow {
            allocation_id: allocation.allocation_id.clone(),
            settlement_asset_id: allocation.asset_id.clone(),
            bucket_id: allocation.bucket_id.clone(),
            receipt_id: allocation.receipt_id.clone(),
            amount_atoms: allocation.amount_atoms,
            released_atoms: allocation.released_atoms,
            remaining_atoms,
            value_nav_units,
            retired_at_height: allocation.retired_at_height,
            bucket_source_domain: bucket.source_domain.clone(),
            bucket_policy_hash: bucket.policy_hash.clone(),
            bucket_gross_receipt_atoms: bucket.gross_receipt_atoms,
            bucket_counted_value_atoms: bucket.counted_value_atoms,
            bucket_nav_subscription_allocations_atoms: bucket.nav_subscription_allocations_atoms,
            bucket_redemption_queue_atoms: bucket.redemption_queue_atoms,
            bucket_outstanding_vault_bridge_atoms: bucket.outstanding_vault_bridge_atoms,
            bucket_status: bucket.status.clone(),
        });
    }
    if rows.is_empty() {
        return Ok(None);
    }
    rows.sort_by(|left, right| left.allocation_id.cmp(&right.allocation_id));
    let mut value_nav_units = 0_u64;
    let mut preimage = format!(
        "nav_asset_id={}\nnav_valuation_unit_bytes={}\nnav_valuation_unit={}\nallocation_count={}\n",
        nav_asset.asset_id,
        nav_asset.valuation_unit.len(),
        nav_asset.valuation_unit,
        rows.len()
    );
    for (index, row) in rows.iter().enumerate() {
        value_nav_units = value_nav_units
            .checked_add(row.value_nav_units)
            .ok_or_else(|| "nav subscription overlay value would overflow".to_string())?;
        preimage.push_str(&format!(
            "allocation[{index}].allocation_id={}\nallocation[{index}].settlement_asset_id={}\nallocation[{index}].bucket_id={}\nallocation[{index}].receipt_id={}\nallocation[{index}].amount_atoms={}\nallocation[{index}].released_atoms={}\nallocation[{index}].remaining_atoms={}\nallocation[{index}].value_nav_units={}\nallocation[{index}].retired_at_height={}\nallocation[{index}].bucket_source_domain_bytes={}\nallocation[{index}].bucket_source_domain={}\nallocation[{index}].bucket_policy_hash={}\nallocation[{index}].bucket_gross_receipt_atoms={}\nallocation[{index}].bucket_counted_value_atoms={}\nallocation[{index}].bucket_nav_subscription_allocations_atoms={}\nallocation[{index}].bucket_redemption_queue_atoms={}\nallocation[{index}].bucket_outstanding_vault_bridge_atoms={}\nallocation[{index}].bucket_status={}\n",
            row.allocation_id,
            row.settlement_asset_id,
            row.bucket_id,
            row.receipt_id,
            row.amount_atoms,
            row.released_atoms,
            row.remaining_atoms,
            row.value_nav_units,
            row.retired_at_height,
            row.bucket_source_domain.len(),
            row.bucket_source_domain,
            row.bucket_policy_hash,
            row.bucket_gross_receipt_atoms,
            row.bucket_counted_value_atoms,
            row.bucket_nav_subscription_allocations_atoms,
            row.bucket_redemption_queue_atoms,
            row.bucket_outstanding_vault_bridge_atoms,
            row.bucket_status,
        ));
    }
    Ok(Some(NavRoundtripSubscriptionOverlay {
        value_nav_units,
        source_root: hash_hex(
            "postfiat.nav_subscription_source_root.v1",
            preimage.as_bytes(),
        ),
    }))
}

fn nav_roundtrip_sp1_subscription_source_root(
    nav_asset: &postfiat_types::NavTrackedAsset,
    profile: &postfiat_types::NavProofProfile,
    decoded: &NavRoundtripSp1Decoded,
    sp1_public_values: &[u8],
    overlay: &NavRoundtripSubscriptionOverlay,
) -> Result<String, String> {
    let total_verified_net_assets = decoded
        .verified_net_assets
        .checked_add(overlay.value_nav_units)
        .ok_or_else(|| {
            "nav subscription overlay value would overflow SP1 base assets".to_string()
        })?;
    let sp1_public_values_hash =
        hash_hex("postfiat.nav_sp1_public_values.v1", sp1_public_values);
    let preimage = format!(
        "asset_id={}\nprofile_id={}\nprofile_source_class_bytes={}\nprofile_source_class={}\npolicy_hash={}\nsp1_public_values_hash={}\nsp1_verified_net_assets={}\nsubscription_overlay_source_root={}\nsubscription_overlay_value_nav_units={}\ntotal_verified_net_assets={}\n",
        nav_asset.asset_id,
        profile.profile_id,
        profile.source_class.len(),
        profile.source_class,
        decoded.policy_hash_hex,
        sp1_public_values_hash,
        decoded.verified_net_assets,
        overlay.source_root,
        overlay.value_nav_units,
        total_verified_net_assets,
    );
    Ok(hash_hex(
        "postfiat.nav_sp1_subscription_composite_source_root.v1",
        preimage.as_bytes(),
    ))
}

fn nav_roundtrip_vault_bridge_atoms_to_nav_value(
    amount_atoms: u64,
    nav_valuation_unit: &str,
    settlement_valuation_unit: &str,
    settlement_asset_precision: u8,
) -> Result<u64, String> {
    let amount_atoms = u128::from(amount_atoms);
    let value = match (
        nav_roundtrip_valuation_unit_scale(nav_valuation_unit, settlement_asset_precision),
        nav_roundtrip_valuation_unit_scale(settlement_valuation_unit, settlement_asset_precision),
    ) {
        (Some(nav_scale), Some(settlement_scale)) if nav_scale != settlement_scale => amount_atoms
            .checked_mul(nav_scale)
            .ok_or_else(|| {
                "nav subscription overlay valuation-scale conversion would overflow".to_string()
            })?
            / settlement_scale,
        _ => amount_atoms,
    };
    u64::try_from(value).map_err(|_| "nav subscription overlay value exceeds u64".to_string())
}

fn nav_roundtrip_issued_asset_supply(
    ledger: &postfiat_types::LedgerState,
    asset_id: &str,
) -> Result<u64, String> {
    let trustline_supply = ledger
        .trustlines
        .iter()
        .filter(|line| line.asset_id == asset_id)
        .try_fold(0_u64, |total, line| {
            total
                .checked_add(line.balance)
                .ok_or_else(|| "issued asset trustline supply overflowed".to_string())
        })?;
    let open_escrow_supply = ledger
        .escrows
        .iter()
        .filter(|escrow| {
            escrow.asset_id == asset_id && escrow.state == postfiat_types::ESCROW_STATE_OPEN
        })
        .try_fold(0_u64, |total, escrow| {
            total
                .checked_add(escrow.amount)
                .ok_or_else(|| "issued asset escrow supply overflowed".to_string())
        })?;
    let open_offer_supply = ledger
        .offers
        .iter()
        .filter(|offer| {
            offer.taker_gets_asset_id == asset_id && offer.state == postfiat_types::OFFER_STATE_OPEN
        })
        .try_fold(0_u64, |total, offer| {
            total
                .checked_add(offer.taker_gets_amount_remaining)
                .ok_or_else(|| "issued asset offer supply overflowed".to_string())
        })?;
    trustline_supply
        .checked_add(open_escrow_supply)
        .and_then(|value| value.checked_add(open_offer_supply))
        .ok_or_else(|| "issued asset supply total overflowed".to_string())
}

fn nav_roundtrip_decode_sp1_public_values(bytes: &[u8]) -> Result<NavRoundtripSp1Decoded, String> {
    if bytes.len() < 32 + 96 + 512 {
        return Err("SP1 public values are too short for AggregatePublicValuesV2".to_string());
    }
    let tuple_offset = nav_roundtrip_read_word_usize(bytes, 0)?;
    if tuple_offset != 32 || tuple_offset >= bytes.len() {
        return Err("SP1 public values tuple offset is invalid".to_string());
    }
    let schema_version = nav_roundtrip_read_word_u32(bytes, tuple_offset)?;
    if schema_version != postfiat_types::AGGREGATE_PUBLIC_VALUES_V2_SCHEMA_VERSION {
        return Err(format!(
            "SP1 public values schema_version was {}, expected {}",
            schema_version,
            postfiat_types::AGGREGATE_PUBLIC_VALUES_V2_SCHEMA_VERSION
        ));
    }
    let policy_hash = nav_roundtrip_read_word_bytes32(bytes, tuple_offset + 64)?;
    let totals_offset = tuple_offset + 96;
    let spot_total = nav_roundtrip_read_word_u128(bytes, totals_offset)?;
    let cash_total = nav_roundtrip_read_word_u128(bytes, totals_offset + 96)?;
    let liability = nav_roundtrip_read_word_u128(bytes, totals_offset + 224)?;
    let verified_net_assets = spot_total
        .checked_add(cash_total)
        .ok_or_else(|| "SP1 public values spot plus cash exceeds u128".to_string())?
        .checked_sub(liability)
        .ok_or_else(|| "SP1 public values liability exceeds spot plus cash".to_string())?;
    let verified_net_assets = u64::try_from(verified_net_assets)
        .map_err(|_| "SP1 public values verified_net_assets exceeds u64".to_string())?;
    Ok(NavRoundtripSp1Decoded {
        policy_hash_hex: bytes_to_hex(&policy_hash),
        verified_net_assets,
    })
}

fn nav_roundtrip_read_word_usize(bytes: &[u8], offset: usize) -> Result<usize, String> {
    let value = nav_roundtrip_read_word_u128(bytes, offset)?;
    usize::try_from(value).map_err(|_| "ABI word does not fit usize".to_string())
}

fn nav_roundtrip_read_word_u32(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let value = nav_roundtrip_read_word_u128(bytes, offset)?;
    u32::try_from(value).map_err(|_| "ABI word does not fit u32".to_string())
}

fn nav_roundtrip_read_word_u128(bytes: &[u8], offset: usize) -> Result<u128, String> {
    let end = offset
        .checked_add(32)
        .ok_or_else(|| "ABI word offset overflow".to_string())?;
    let word = bytes
        .get(offset..end)
        .ok_or_else(|| "ABI word is out of bounds".to_string())?;
    if word[..16].iter().any(|byte| *byte != 0) {
        return Err("ABI word exceeds u128".to_string());
    }
    let mut out = [0_u8; 16];
    out.copy_from_slice(&word[16..]);
    Ok(u128::from_be_bytes(out))
}

fn nav_roundtrip_read_word_bytes32(bytes: &[u8], offset: usize) -> Result<[u8; 32], String> {
    let end = offset
        .checked_add(32)
        .ok_or_else(|| "ABI word offset overflow".to_string())?;
    let word = bytes
        .get(offset..end)
        .ok_or_else(|| "ABI word is out of bounds".to_string())?;
    let mut out = [0_u8; 32];
    out.copy_from_slice(word);
    Ok(out)
}

fn certified_asset_ops_from_bundle(
    options: CertifiedAssetOpsFromBundleOptions,
) -> Result<CertifiedAssetOpsFromBundleReport, String> {
    if options.output_file.exists() && !options.overwrite {
        return Err(format!(
            "certified asset ops output `{}` already exists; pass --overwrite to replace it",
            options.output_file.display()
        ));
    }
    let mut operations = Vec::new();
    maybe_push_bundle_operation(
        &mut operations,
        &options.bundle_dir,
        "propose",
        "propose.operation.json",
        options.proposer_key_file.as_ref(),
        "--proposer-key-file",
    )?;
    maybe_push_bundle_operation(
        &mut operations,
        &options.bundle_dir,
        "attest",
        "attest.operation.json",
        options.attestor_key_file.as_ref(),
        "--attestor-key-file",
    )?;
    maybe_push_bundle_operation(
        &mut operations,
        &options.bundle_dir,
        "finalize",
        "finalize.operation.json",
        options.finalizer_key_file.as_ref(),
        "--finalizer-key-file",
    )?;
    if options.include_deposit_claim {
        maybe_push_bundle_operation(
            &mut operations,
            &options.bundle_dir,
            "claim",
            "claim.operation.json",
            options.claimer_key_file.as_ref(),
            "--claimer-key-file",
        )?;
    }
    maybe_push_bundle_operation(
        &mut operations,
        &options.bundle_dir,
        "burn-to-redeem",
        "burn-to-redeem.operation.json",
        options.owner_key_file.as_ref(),
        "--owner-key-file",
    )?;
    if operations.is_empty() {
        return Err(format!(
            "bundle `{}` did not contain supported asset operation files",
            options.bundle_dir.display()
        ));
    }
    let labels = operations
        .iter()
        .map(|operation: &serde_json::Value| {
            operation
                .get("label")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .to_string()
        })
        .collect::<Vec<_>>();
    let request = serde_json::json!({
        "schema": CERTIFIED_ASSET_OPS_REQUEST_SCHEMA,
        "operations": operations,
    });
    if let Some(parent) = options
        .output_file
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create certified asset ops output parent `{}`: {error}",
                parent.display()
            )
        })?;
    }
    write_json_file(&options.output_file, &request)?;
    Ok(CertifiedAssetOpsFromBundleReport {
        schema: CERTIFIED_ASSET_OPS_FROM_BUNDLE_REPORT_SCHEMA.to_string(),
        bundle_dir: options.bundle_dir.display().to_string(),
        output_file: options.output_file.display().to_string(),
        operation_count: labels.len(),
        labels,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NavRoundtripBridgeAbi {
    bridge_class: String,
    withdrawal_digest_signature: Option<String>,
    submit_withdrawal_signature: Option<String>,
}

#[derive(Debug, Clone)]
struct NavRoundtripEvmWithdrawalCallPlan {
    pftl_finalized_height: u64,
    withdrawal_packet_tuple_arg: String,
    withdrawal_packet_digest: String,
    pftl_withdrawal_hash: String,
    pftl_withdrawal_hash_commitment: String,
    verifier_proof_digest_to_sign: String,
    verifier_pending_proof_id: String,
    vault_pending_withdrawal_id: String,
    signatures_arg: String,
    verifier_submit_proof_signature: String,
    vault_submit_withdrawal_signature: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct NavRoundtripWithdrawalAutoSignatureReport {
    schema: String,
    signature_request_file: String,
    signatures_file: String,
    signer_key_file: String,
    signer_address: String,
    verifier_proof_digest_to_sign: String,
    signature_count: usize,
    private_key_material_redacted: bool,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct NavRoundtripWithdrawalSignerKeyRecord {
    address: Option<String>,
    private_key: Option<String>,
    private_key_hex: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct NavRoundtripWithdrawalSignatureRequest {
    verifier_proof_digest_to_sign: String,
}

fn nav_roundtrip_align_withdrawal_signature_request_with_live_abi(
    cast_binary: &str,
    source_rpc_url: &str,
    vault_address: &str,
    verifier_address: &str,
    usdc_address: &str,
    stakehub_wallet: &str,
    plan_file: &std::path::Path,
    signature_request_file: &std::path::Path,
) -> Result<(), String> {
    let plan_raw = std::fs::read_to_string(plan_file)
        .map_err(|error| format!("failed to read withdrawal plan `{}`: {error}", plan_file.display()))?;
    let plan = serde_json::from_str::<postfiat_node::VaultBridgeWithdrawalPlanReport>(&plan_raw)
        .map_err(|error| format!("withdrawal plan `{}` is invalid: {error}", plan_file.display()))?;
    let request_raw = std::fs::read_to_string(signature_request_file).map_err(|error| {
        format!(
            "failed to read withdrawal signature request `{}`: {error}",
            signature_request_file.display()
        )
    })?;
    let mut request = serde_json::from_str::<postfiat_node::VaultBridgeWithdrawalSignatureRequest>(
        &request_raw,
    )
    .map_err(|error| {
        format!(
            "withdrawal signature request `{}` is invalid: {error}",
            signature_request_file.display()
        )
    })?;
    let bridge_abi = classify_nav_roundtrip_vault_abi(
        cast_binary,
        source_rpc_url,
        vault_address,
        usdc_address,
        stakehub_wallet,
    )?;
    let call_plan = nav_roundtrip_evm_withdrawal_call_plan(
        cast_binary,
        source_rpc_url,
        vault_address,
        verifier_address,
        &plan,
        &bridge_abi,
        &[],
    )?;
    if request.withdrawal_packet_evm_digest == call_plan.withdrawal_packet_digest
        && request.verifier_proof_digest_to_sign == call_plan.verifier_proof_digest_to_sign
        && request.verifier_pending_proof_id == call_plan.verifier_pending_proof_id
    {
        return Ok(());
    }

    request.withdrawal_packet_evm_digest = call_plan.withdrawal_packet_digest.clone();
    request.verifier_proof_digest_to_sign = call_plan.verifier_proof_digest_to_sign.clone();
    request.verifier_pending_proof_id = call_plan.verifier_pending_proof_id.clone();
    request.cast_wallet_sign_command = format!(
        "cast wallet sign --no-hash '{}' --private-key \"$PFTL_WITHDRAWAL_SIGNER_PRIVATE_KEY\"",
        call_plan.verifier_proof_digest_to_sign
    );
    write_json_file(signature_request_file, &request).map_err(|error| {
        format!(
            "failed to write ABI-aligned withdrawal signature request `{}`: {error}",
            signature_request_file.display()
        )
    })
}

fn nav_roundtrip_evm_withdrawal_call_plan(
    cast_binary: &str,
    source_rpc_url: &str,
    vault_address: &str,
    verifier_address: &str,
    plan: &postfiat_node::VaultBridgeWithdrawalPlanReport,
    bridge_abi: &NavRoundtripBridgeAbi,
    signatures: &[String],
) -> Result<NavRoundtripEvmWithdrawalCallPlan, String> {
    let signatures_arg = nav_roundtrip_evm_bytes_array_arg(signatures);
    if bridge_abi.bridge_class == NAV_ROUNDTRIP_BRIDGE_CLASS_FIXED_REDEPLOYED {
        return Ok(NavRoundtripEvmWithdrawalCallPlan {
            pftl_finalized_height: plan.pftl_finalized_height,
            withdrawal_packet_tuple_arg: plan.withdrawal_packet_tuple_arg.clone(),
            withdrawal_packet_digest: plan.withdrawal_packet_evm_digest.clone(),
            pftl_withdrawal_hash: plan.pftl_withdrawal_hash.clone(),
            pftl_withdrawal_hash_commitment: plan.pftl_withdrawal_hash_commitment.clone(),
            verifier_proof_digest_to_sign: plan
                .verifier_proof_digest_to_sign
                .clone()
                .ok_or_else(|| "withdrawal plan is missing verifier proof digest".to_string())?,
            verifier_pending_proof_id: plan.verifier_pending_proof_id.clone(),
            vault_pending_withdrawal_id: plan.vault_pending_withdrawal_id.clone(),
            signatures_arg,
            verifier_submit_proof_signature: plan.verifier_submit_proof_signature.clone(),
            vault_submit_withdrawal_signature: plan.vault_submit_withdrawal_signature.clone(),
        });
    }

    if bridge_abi.bridge_class != NAV_ROUNDTRIP_BRIDGE_CLASS_CONTROLLED_LAUNCH {
        return Err(format!(
            "unsupported bridge class `{}` for EVM withdrawal",
            bridge_abi.bridge_class
        ));
    }

    let old_packet_tuple = nav_roundtrip_old_withdrawal_packet_tuple_arg(plan);
    let packet_digest = cast_bytes32_call(
        cast_binary,
        source_rpc_url,
        vault_address,
        NAV_ROUNDTRIP_OLD_WITHDRAWAL_DIGEST_SIGNATURE,
        &[old_packet_tuple.as_str()],
    )?;
    let verifier_pending_proof_id = cast_bytes32_call(
        cast_binary,
        source_rpc_url,
        verifier_address,
        "pendingProofId(bytes32,bytes32,uint64)(bytes32)",
        &[
            packet_digest.as_str(),
            plan.pftl_withdrawal_hash_commitment.as_str(),
            &plan.pftl_finalized_height.to_string(),
        ],
    )?;
    let verifier_proof_digest_to_sign = cast_bytes32_call(
        cast_binary,
        source_rpc_url,
        verifier_address,
        "proofDigest(bytes32,bytes32,uint64)(bytes32)",
        &[
            packet_digest.as_str(),
            plan.pftl_withdrawal_hash_commitment.as_str(),
            &plan.pftl_finalized_height.to_string(),
        ],
    )?;
    let vault_pending_withdrawal_id = cast_bytes32_call(
        cast_binary,
        source_rpc_url,
        vault_address,
        "withdrawalPendingId((uint64,bytes,bytes,bytes,address,uint256,bytes,bytes,uint64,bytes),bytes32)(bytes32)",
        &[old_packet_tuple.as_str(), plan.pftl_withdrawal_hash_commitment.as_str()],
    )?;

    Ok(NavRoundtripEvmWithdrawalCallPlan {
        pftl_finalized_height: plan.pftl_finalized_height,
        withdrawal_packet_tuple_arg: old_packet_tuple,
        withdrawal_packet_digest: packet_digest,
        pftl_withdrawal_hash: plan.pftl_withdrawal_hash.clone(),
        pftl_withdrawal_hash_commitment: plan.pftl_withdrawal_hash_commitment.clone(),
        verifier_proof_digest_to_sign,
        verifier_pending_proof_id,
        vault_pending_withdrawal_id,
        signatures_arg,
        verifier_submit_proof_signature: plan.verifier_submit_proof_signature.clone(),
        vault_submit_withdrawal_signature: NAV_ROUNDTRIP_OLD_SUBMIT_WITHDRAWAL_SIGNATURE.to_string(),
    })
}

fn nav_roundtrip_old_withdrawal_packet_tuple_arg(
    plan: &postfiat_node::VaultBridgeWithdrawalPlanReport,
) -> String {
    let args = &plan.withdrawal_packet_evm_args;
    format!(
        "({},{},{},{},{},{},{},{},{},{})",
        args.pftl_chain_id,
        args.vault_bridge_asset_id,
        args.burn_tx_id,
        args.withdrawal_id,
        args.recipient,
        args.amount,
        args.source_bucket_id,
        args.destination_hash,
        args.finalized_height,
        args.evidence_root
    )
}

fn classify_nav_roundtrip_vault_abi(
    cast_binary: &str,
    source_rpc_url: &str,
    vault_address: &str,
    usdc_address: &str,
    stakehub_wallet: &str,
) -> Result<NavRoundtripBridgeAbi, String> {
    let fixed_packet = format!(
        "(1,42161,{vault_address},{usdc_address},{},{},{},{stakehub_wallet},1,{},{},1,{})",
        nav_roundtrip_dummy_hex(48, 0x33),
        nav_roundtrip_dummy_hex(32, 0x44),
        nav_roundtrip_dummy_hex(32, 0x55),
        nav_roundtrip_dummy_hex(32, 0x66),
        nav_roundtrip_dummy_hex(32, 0x77),
        nav_roundtrip_dummy_hex(32, 0x88)
    );
    if cast_call_returns_hex(
        cast_binary,
        source_rpc_url,
        vault_address,
        NAV_ROUNDTRIP_FIXED_WITHDRAWAL_DIGEST_SIGNATURE,
        &[fixed_packet.as_str()],
    )? {
        return Ok(NavRoundtripBridgeAbi {
            bridge_class: NAV_ROUNDTRIP_BRIDGE_CLASS_FIXED_REDEPLOYED.to_string(),
            withdrawal_digest_signature: Some(
                NAV_ROUNDTRIP_FIXED_WITHDRAWAL_DIGEST_SIGNATURE.to_string(),
            ),
            submit_withdrawal_signature: Some(NAV_ROUNDTRIP_FIXED_SUBMIT_WITHDRAWAL_SIGNATURE.to_string()),
        });
    }

    let old_packet = format!(
        "(1,{},{},{},{stakehub_wallet},1,{},{},1,{})",
        nav_roundtrip_dummy_hex(48, 0x33),
        nav_roundtrip_dummy_hex(32, 0x44),
        nav_roundtrip_dummy_hex(32, 0x55),
        nav_roundtrip_dummy_hex(32, 0x66),
        nav_roundtrip_dummy_hex(32, 0x77),
        nav_roundtrip_dummy_hex(32, 0x88)
    );
    if cast_call_returns_hex(
        cast_binary,
        source_rpc_url,
        vault_address,
        NAV_ROUNDTRIP_OLD_WITHDRAWAL_DIGEST_SIGNATURE,
        &[old_packet.as_str()],
    )? {
        return Ok(NavRoundtripBridgeAbi {
            bridge_class: NAV_ROUNDTRIP_BRIDGE_CLASS_CONTROLLED_LAUNCH.to_string(),
            withdrawal_digest_signature: Some(
                NAV_ROUNDTRIP_OLD_WITHDRAWAL_DIGEST_SIGNATURE.to_string(),
            ),
            submit_withdrawal_signature: Some(NAV_ROUNDTRIP_OLD_SUBMIT_WITHDRAWAL_SIGNATURE.to_string()),
        });
    }

    Ok(NavRoundtripBridgeAbi {
        bridge_class: NAV_ROUNDTRIP_BRIDGE_CLASS_UNKNOWN.to_string(),
        withdrawal_digest_signature: None,
        submit_withdrawal_signature: None,
    })
}

fn nav_roundtrip_select_settlement_receipt(
    status: &postfiat_node::VaultBridgeStatusReport,
    settlement_amount_atoms: u64,
    explicit_receipt_id: Option<&str>,
    matched_deposit_tx: Option<&str>,
) -> Result<postfiat_node::VaultBridgeReceiptStatusRow, String> {
    let active_bucket_ids = status
        .buckets
        .iter()
        .filter(|bucket| bucket.status == postfiat_types::VAULT_BRIDGE_BUCKET_STATUS_ACTIVE)
        .map(|bucket| bucket.bucket_id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let mut candidates = status
        .receipts
        .iter()
        .filter(|receipt| {
            receipt.status == postfiat_types::VAULT_BRIDGE_RECEIPT_STATUS_COUNTED
                && active_bucket_ids.contains(receipt.bucket_id.as_str())
                && receipt.unallocated_value_atoms >= settlement_amount_atoms
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        right
            .counted_at_height
            .cmp(&left.counted_at_height)
            .then_with(|| right.created_at_height.cmp(&left.created_at_height))
            .then_with(|| left.receipt_id.cmp(&right.receipt_id))
    });

    if let Some(receipt_id) = explicit_receipt_id {
        return candidates
            .into_iter()
            .find(|receipt| receipt.receipt_id == receipt_id)
            .cloned()
            .ok_or_else(|| {
                format!(
                    "settlement receipt `{receipt_id}` is not counted, active, and unallocated for {settlement_amount_atoms} atoms"
                )
            });
    }

    if let Some(matched_deposit_tx) = matched_deposit_tx {
        let matching_evidence_roots = status
            .bridge_deposits
            .iter()
            .filter(|deposit| nav_roundtrip_normalize_hex_text(&deposit.tx_hash) == matched_deposit_tx)
            .map(|deposit| deposit.evidence_root.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        if !matching_evidence_roots.is_empty() {
            if let Some(receipt) = candidates.iter().find(|receipt| {
                receipt
                    .bridge_deposit_evidence_root
                    .as_deref()
                    .is_some_and(|root| matching_evidence_roots.contains(root))
            }) {
                return Ok((*receipt).clone());
            }
            return Err(format!(
                "deposit tx `{matched_deposit_tx}` finalized but has no counted unallocated receipt for {settlement_amount_atoms} atoms"
            ));
        }
    }

    candidates
        .into_iter()
        .next()
        .cloned()
        .ok_or_else(|| {
            format!(
                "no counted active settlement receipt has {settlement_amount_atoms} unallocated atoms"
            )
        })
}

#[derive(Clone)]
struct NavRoundtripIssuedSettlementSource {
    receipt: postfiat_node::VaultBridgeReceiptStatusRow,
    supply_allocation_id: String,
}

fn nav_roundtrip_select_issued_settlement_source(
    ledger: &postfiat_types::LedgerState,
    status: &postfiat_node::VaultBridgeStatusReport,
    settlement_asset_id: &str,
    settlement_amount_atoms: u64,
    explicit_receipt_id: Option<&str>,
    explicit_supply_allocation_id: Option<&str>,
    matched_deposit_tx: Option<&str>,
) -> Result<NavRoundtripIssuedSettlementSource, String> {
    let active_bucket_ids = status
        .buckets
        .iter()
        .filter(|bucket| bucket.status == postfiat_types::VAULT_BRIDGE_BUCKET_STATUS_ACTIVE)
        .map(|bucket| bucket.bucket_id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let matching_evidence_roots = matched_deposit_tx
        .map(|matched_deposit_tx| {
            status
                .bridge_deposits
                .iter()
                .filter(|deposit| {
                    nav_roundtrip_normalize_hex_text(&deposit.tx_hash) == matched_deposit_tx
                })
                .map(|deposit| deposit.evidence_root.as_str())
                .collect::<std::collections::BTreeSet<_>>()
        })
        .unwrap_or_default();

    let mut candidates = Vec::new();
    for receipt in status.receipts.iter().filter(|receipt| {
        receipt.status == postfiat_types::VAULT_BRIDGE_RECEIPT_STATUS_COUNTED
            && active_bucket_ids.contains(receipt.bucket_id.as_str())
            && explicit_receipt_id.map_or(true, |id| receipt.receipt_id == id)
            && (matching_evidence_roots.is_empty()
                || receipt
                    .bridge_deposit_evidence_root
                    .as_deref()
                    .is_some_and(|root| matching_evidence_roots.contains(root)))
    }) {
        for allocation in ledger.vault_bridge_allocations.iter().filter(|allocation| {
            allocation.asset_id == settlement_asset_id
                && allocation.receipt_id == receipt.receipt_id
                && allocation.bucket_id == receipt.bucket_id
                && allocation.purpose == postfiat_types::VAULT_BRIDGE_ALLOCATION_PURPOSE_SUPPLY
                && allocation.retired_at_height == 0
                && explicit_supply_allocation_id
                    .map_or(true, |id| allocation.allocation_id == id)
        }) {
            let remaining = allocation
                .amount_atoms
                .checked_sub(allocation.released_atoms)
                .ok_or_else(|| {
                    format!(
                        "supply allocation `{}` released atoms exceed amount",
                        allocation.allocation_id
                    )
                })?;
            if remaining >= settlement_amount_atoms {
                candidates.push((receipt.clone(), allocation.clone(), remaining));
            }
        }
    }

    candidates.sort_by(|(left_receipt, left_allocation, left_remaining), (right_receipt, right_allocation, right_remaining)| {
        right_receipt
            .counted_at_height
            .cmp(&left_receipt.counted_at_height)
            .then_with(|| right_allocation.created_at_height.cmp(&left_allocation.created_at_height))
            .then_with(|| right_remaining.cmp(left_remaining))
            .then_with(|| left_allocation.allocation_id.cmp(&right_allocation.allocation_id))
    });

    candidates
        .into_iter()
        .next()
        .map(|(receipt, allocation, _)| NavRoundtripIssuedSettlementSource {
            receipt,
            supply_allocation_id: allocation.allocation_id,
        })
        .ok_or_else(|| {
            let mut reason = format!(
                "no live vault_bridge_supply allocation has {settlement_amount_atoms} atoms for issued settlement asset `{settlement_asset_id}`"
            );
            if let Some(receipt_id) = explicit_receipt_id {
                reason.push_str(&format!(" and receipt `{receipt_id}`"));
            }
            if let Some(allocation_id) = explicit_supply_allocation_id {
                reason.push_str(&format!(" and supply allocation `{allocation_id}`"));
            }
            if let Some(tx) = matched_deposit_tx {
                reason.push_str(&format!(" matching deposit tx `{tx}`"));
            }
            reason
        })
}

fn nav_roundtrip_required_vault_bridge_settlement_atoms(
    amount_atoms: u64,
    nav_asset_precision: u8,
    nav_per_unit: u64,
    nav_valuation_unit: &str,
    settlement_valuation_unit: &str,
    settlement_asset_precision: u8,
) -> Result<u64, String> {
    let nav_asset_scale = 10_u128
        .checked_pow(nav_asset_precision.into())
        .ok_or_else(|| "nav asset precision scale would overflow".to_string())?;
    let raw = u128::from(amount_atoms)
        .checked_mul(u128::from(nav_per_unit))
        .ok_or_else(|| "nav mint amount times nav_per_unit would overflow".to_string())?;
    let (numerator, denominator) = match (
        nav_roundtrip_valuation_unit_scale(nav_valuation_unit, settlement_asset_precision),
        nav_roundtrip_valuation_unit_scale(settlement_valuation_unit, settlement_asset_precision),
    ) {
        (Some(nav_scale), Some(settlement_scale)) if nav_scale != settlement_scale => {
            let numerator = raw
                .checked_mul(settlement_scale)
                .ok_or_else(|| "nav settlement valuation-scale conversion would overflow".to_string())?;
            let denominator = nav_scale
                .checked_mul(nav_asset_scale)
                .ok_or_else(|| "nav settlement denominator scale would overflow".to_string())?;
            (numerator, denominator)
        }
        _ => (raw, nav_asset_scale),
    };
    let required = numerator
        .checked_add(denominator - 1)
        .ok_or_else(|| "nav settlement valuation-scale rounding would overflow".to_string())?
        / denominator;
    u64::try_from(required)
        .map_err(|_| "nav required settlement amount exceeds u64".to_string())
}

fn nav_roundtrip_valuation_unit_scale(valuation_unit: &str, asset_precision: u8) -> Option<u128> {
    let unit = valuation_unit.trim().to_ascii_lowercase();
    if let Some(scale) = unit.strip_prefix("usd_1e") {
        return scale
            .parse::<u32>()
            .ok()
            .and_then(|exponent| 10_u128.checked_pow(exponent));
    }
    match unit.as_str() {
        "usdc" | "usd_1e6" | "micro_usd" => 10_u128.checked_pow(asset_precision.into()),
        _ => None,
    }
}

fn nav_roundtrip_nav_subscription_consumer_id(nav_asset_id: &str) -> String {
    format!("nav_subscription:{nav_asset_id}")
}

fn nav_roundtrip_nav_subscription_consumer_matches(consumer_id: &str, nav_asset_id: &str) -> bool {
    consumer_id == nav_roundtrip_nav_subscription_consumer_id(nav_asset_id)
        || consumer_id
            .strip_prefix(&format!("nav_subscription:{nav_asset_id}:"))
            .map_or(false, |recipient| !recipient.is_empty())
}

fn nav_roundtrip_nav_subscription_recipient_consumer_id(
    nav_asset_id: &str,
    recipient: &str,
) -> String {
    format!("nav_subscription:{nav_asset_id}:{recipient}")
}

fn nav_roundtrip_nav_subscription_recipient_order_consumer_id(
    nav_asset_id: &str,
    recipient: &str,
    subscription_id: &str,
) -> String {
    format!("nav_subscription:{nav_asset_id}:{recipient}:{subscription_id}")
}

fn nav_roundtrip_primary_mint_subscription_id(artifact_dir: &std::path::Path) -> String {
    let own = artifact_dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("run");
    let parent = artifact_dir
        .parent()
        .and_then(|path| path.file_name())
        .and_then(|name| name.to_str())
        .unwrap_or("");
    let raw = if parent.is_empty() {
        own.to_string()
    } else {
        format!("{parent}-{own}")
    };
    let mut sanitized = raw
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    if sanitized.is_empty() {
        sanitized = "run".to_string();
    }
    if sanitized.len() > 64 {
        sanitized.truncate(64);
    }
    format!("navrt-{sanitized}")
}

fn nav_roundtrip_normalize_hex_text(value: &str) -> String {
    value.trim().trim_start_matches("0x").to_ascii_lowercase()
}

fn nav_roundtrip_normalize_destination_ref(value: String) -> String {
    let mut parts = value.split(':').collect::<Vec<_>>();
    if parts.len() == 3 && parts[0] == "evm-erc20" {
        parts[2] = parts[2].trim();
        return format!("{}:{}:{}", parts[0], parts[1], parts[2].to_ascii_lowercase());
    }
    value
}

fn nav_roundtrip_trustline_balance(
    ledger: &postfiat_types::LedgerState,
    account: &str,
    asset_id: &str,
) -> Option<u64> {
    ledger
        .trustline_for_account_asset(account, asset_id)
        .map(|line| line.balance)
}

fn nav_roundtrip_find_matching_redemption(
    ledger: &postfiat_types::LedgerState,
    owner: &str,
    asset_id: &str,
    amount: u64,
    epoch: u64,
    reserve_packet_hash: &str,
) -> Option<postfiat_types::NavRedemption> {
    ledger
        .nav_redemptions
        .iter()
        .filter(|redemption| {
            redemption.owner == owner
                && redemption.asset_id == asset_id
                && redemption.amount == amount
                && redemption.epoch == epoch
                && redemption.reserve_packet_hash == reserve_packet_hash
                && redemption.state == postfiat_types::NAV_REDEMPTION_STATE_PENDING
        })
        .max_by(|left, right| {
            left.created_at_height
                .cmp(&right.created_at_height)
                .then_with(|| left.owner_sequence.cmp(&right.owner_sequence))
                .then_with(|| left.redemption_id.cmp(&right.redemption_id))
        })
        .cloned()
}

fn nav_roundtrip_nav_exit_settlement_receipt_hash(
    chain_id: &str,
    nav_asset_id: &str,
    settlement_asset_id: &str,
    redemption_id: &str,
    settlement_allocation_id: &str,
    settlement_amount_atoms: u64,
) -> String {
    let preimage = format!(
        "chain_id={chain_id}\nnav_asset_id={nav_asset_id}\nsettlement_asset_id={settlement_asset_id}\nredemption_id={redemption_id}\nsettlement_allocation_id={settlement_allocation_id}\nsettlement_amount_atoms={settlement_amount_atoms}\n",
    );
    hash_hex(
        "postfiat.nav_roundtrip.nav_exit_settlement_receipt.v1",
        preimage.as_bytes(),
    )
}

fn nav_roundtrip_vault_bridge_settlement_receipt_hash(
    report: &NavRoundtripEvmWithdrawalReport,
) -> String {
    let preimage = format!(
        "source_chain_id={}\nvault_address={}\nverifier_address={}\nusdc_address={}\nstakehub_wallet={}\nsettlement_asset_id={}\nredemption_id={}\nsettled_atoms={}\nwithdrawal_packet_digest={}\npftl_withdrawal_hash={}\npftl_withdrawal_hash_commitment={}\nverifier_pending_proof_id={}\nvault_pending_withdrawal_id={}\nsubmit_proof_tx={}\nfinalize_proof_tx={}\nsubmit_withdrawal_tx={}\nfinalize_withdrawal_tx={}\nclaim_withdrawal_tx={}\nwallet_usdc_before_atoms={}\nwallet_usdc_after_atoms={}\nvault_usdc_before_atoms={}\nvault_usdc_after_atoms={}\n",
        report.source_chain_id,
        report.vault_address,
        report.verifier_address,
        report.usdc_address,
        report.stakehub_wallet,
        report.settlement_asset_id,
        report.redemption_id,
        report.amount_atoms,
        report.withdrawal_packet_digest,
        report.pftl_withdrawal_hash,
        report.pftl_withdrawal_hash_commitment,
        report.verifier_pending_proof_id,
        report.vault_pending_withdrawal_id,
        report.submit_proof_tx,
        report.finalize_proof_tx,
        report.submit_withdrawal_tx,
        report.finalize_withdrawal_tx,
        report.claim_withdrawal_tx,
        report.wallet_usdc_before_atoms,
        report.wallet_usdc_after_atoms,
        report.vault_usdc_before_atoms,
        report.vault_usdc_after_atoms,
    );
    hash_hex(
        "postfiat.nav_roundtrip.vault_bridge_redeem_settlement_receipt.v1",
        preimage.as_bytes(),
    )
}

fn nav_roundtrip_auto_sign_withdrawal_bundle(
    signature_request_file: &std::path::Path,
    signatures_file: &std::path::Path,
    signer_key_file: &std::path::Path,
) -> Result<NavRoundtripWithdrawalAutoSignatureReport, String> {
    let request_raw = std::fs::read_to_string(signature_request_file).map_err(|error| {
        format!(
            "failed to read withdrawal signature request `{}`: {error}",
            signature_request_file.display()
        )
    })?;
    let request = serde_json::from_str::<NavRoundtripWithdrawalSignatureRequest>(&request_raw)
        .map_err(|error| {
            format!(
                "withdrawal signature request `{}` is invalid: {error}",
                signature_request_file.display()
            )
        })?;
    let signer_key_raw = std::fs::read_to_string(signer_key_file).map_err(|error| {
        format!(
            "failed to read withdrawal signer key file `{}`: {error}",
            signer_key_file.display()
        )
    })?;
    let signer_key = serde_json::from_str::<NavRoundtripWithdrawalSignerKeyRecord>(&signer_key_raw)
        .map_err(|error| {
            format!(
                "withdrawal signer key file `{}` is invalid: {error}",
                signer_key_file.display()
            )
        })?;
    let private_key_hex = signer_key
        .private_key
        .as_deref()
        .or(signer_key.private_key_hex.as_deref())
        .ok_or_else(|| {
            format!(
                "withdrawal signer key file `{}` must contain private_key or private_key_hex",
                signer_key_file.display()
            )
        })?;
    let (signer_address, signature) =
        nav_roundtrip_sign_evm_digest(private_key_hex, &request.verifier_proof_digest_to_sign)?;
    if let Some(expected_address) = signer_key.address.as_deref() {
        let expected = nav_roundtrip_normalize_evm_address(expected_address, "signer address")?;
        if signer_address != expected {
            return Err(format!(
                "withdrawal signer key file `{}` derives `{}` but declares `{}`",
                signer_key_file.display(),
                signer_address,
                expected
            ));
        }
    }
    let recovered =
        nav_roundtrip_recover_evm_signer(&request.verifier_proof_digest_to_sign, &signature)?;
    if recovered != signer_address {
        return Err(format!(
            "withdrawal signer self-check failed: signature recovers `{recovered}` but key derives `{signer_address}`"
        ));
    }

    write_json_file(signatures_file, &vec![signature])?;
    Ok(NavRoundtripWithdrawalAutoSignatureReport {
        schema: NAV_ROUNDTRIP_WITHDRAWAL_AUTO_SIGNATURE_REPORT_SCHEMA.to_string(),
        signature_request_file: signature_request_file.display().to_string(),
        signatures_file: signatures_file.display().to_string(),
        signer_key_file: signer_key_file.display().to_string(),
        signer_address,
        verifier_proof_digest_to_sign: request.verifier_proof_digest_to_sign,
        signature_count: 1,
        private_key_material_redacted: true,
    })
}

fn nav_roundtrip_sign_evm_digest(
    private_key_hex: &str,
    digest_hex: &str,
) -> Result<(String, String), String> {
    let private_key = nav_roundtrip_parse_fixed_hex(private_key_hex, 32, "private_key")?;
    let digest = nav_roundtrip_parse_fixed_hex(digest_hex, 32, "verifier_proof_digest_to_sign")?;
    let private_key = zeroize::Zeroizing::new(private_key);
    let signing_key = k256::ecdsa::SigningKey::from_slice(&private_key[..])
        .map_err(|error| format!("withdrawal signer private key is invalid: {error}"))?;
    let address = nav_roundtrip_evm_address_from_verifying_key(signing_key.verifying_key())?;
    let (signature, recovery_id) = signing_key
        .sign_prehash_recoverable(&digest)
        .map_err(|error| format!("withdrawal digest signing failed: {error}"))?;
    let signature_bytes = signature.to_bytes();
    let mut out = Vec::with_capacity(65);
    out.extend_from_slice(signature_bytes.as_ref());
    out.push(27_u8 + recovery_id.to_byte());
    Ok((address, format!("0x{}", bytes_to_hex(&out))))
}

fn nav_roundtrip_recover_evm_signer(digest_hex: &str, signature_hex: &str) -> Result<String, String> {
    let digest = nav_roundtrip_parse_fixed_hex(digest_hex, 32, "digest")?;
    let signature_bytes = nav_roundtrip_parse_fixed_hex(signature_hex, 65, "signature")?;
    let v = signature_bytes[64];
    if v != 27 && v != 28 {
        return Err(format!("signature recovery id must be 27 or 28, got {v}"));
    }
    let signature = k256::ecdsa::Signature::try_from(&signature_bytes[..64])
        .map_err(|error| format!("signature r/s bytes are invalid: {error}"))?;
    let recovery_id = k256::ecdsa::RecoveryId::try_from(v - 27)
        .map_err(|error| format!("signature recovery id is invalid: {error}"))?;
    let verifying_key =
        k256::ecdsa::VerifyingKey::recover_from_prehash(&digest, &signature, recovery_id)
            .map_err(|error| format!("signature recovery failed: {error}"))?;
    nav_roundtrip_evm_address_from_verifying_key(&verifying_key)
}

fn nav_roundtrip_evm_address_from_verifying_key(
    verifying_key: &k256::ecdsa::VerifyingKey,
) -> Result<String, String> {
    use sha3::Digest as _;

    let point = verifying_key.to_encoded_point(false);
    let point_bytes = point.as_bytes();
    if point_bytes.len() != 65 || point_bytes[0] != 0x04 {
        return Err("secp256k1 verifying key did not encode as uncompressed SEC1".to_string());
    }
    let mut hasher = sha3::Keccak256::new();
    hasher.update(&point_bytes[1..]);
    let hash = hasher.finalize();
    Ok(format!("0x{}", bytes_to_hex(&hash[12..])))
}

fn nav_roundtrip_parse_fixed_hex(
    value: &str,
    expected_len: usize,
    field: &str,
) -> Result<Vec<u8>, String> {
    let stripped = value
        .trim()
        .strip_prefix("0x")
        .or_else(|| value.trim().strip_prefix("0X"))
        .unwrap_or_else(|| value.trim());
    let bytes = hex_to_bytes(stripped).map_err(|error| format!("{field} has invalid hex: {error}"))?;
    if bytes.len() != expected_len {
        return Err(format!(
            "{field} must be {expected_len} bytes, got {}",
            bytes.len()
        ));
    }
    Ok(bytes)
}

fn nav_roundtrip_normalize_evm_address(value: &str, field: &str) -> Result<String, String> {
    let stripped = value
        .trim()
        .strip_prefix("0x")
        .or_else(|| value.trim().strip_prefix("0X"))
        .unwrap_or_else(|| value.trim());
    if stripped.len() != 40 || !stripped.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err(format!("{field} must be a 20-byte 0x EVM address"));
    }
    Ok(format!("0x{}", stripped.to_ascii_lowercase()))
}

fn nav_roundtrip_require_verifier_signer(
    cast_binary: &str,
    rpc_url: &str,
    verifier_address: &str,
    signer_address: &str,
) -> Result<(), String> {
    let approved = cast_bool_call(
        cast_binary,
        rpc_url,
        verifier_address,
        "is_signer(address)(bool)",
        &[signer_address],
    )?;
    if approved {
        return Ok(());
    }
    Err(format!(
        "withdrawal signer `{signer_address}` is not approved by verifier `{verifier_address}`"
    ))
}

fn nav_roundtrip_read_evm_signatures(path: &std::path::Path) -> Result<Vec<String>, String> {
    let raw = std::fs::read_to_string(path).map_err(|error| {
        format!(
            "failed to read withdrawal signatures file `{}`: {error}",
            path.display()
        )
    })?;
    let signatures = serde_json::from_str::<Vec<String>>(&raw).map_err(|error| {
        format!(
            "withdrawal signatures file `{}` is not a JSON string array: {error}",
            path.display()
        )
    })?;
    signatures
        .into_iter()
        .enumerate()
        .map(|(index, signature)| {
            let stripped = signature
                .trim()
                .strip_prefix("0x")
                .or_else(|| signature.trim().strip_prefix("0X"))
                .unwrap_or_else(|| signature.trim());
            let bytes = hex_to_bytes(stripped)
                .map_err(|error| format!("signatures[{index}] has invalid hex: {error}"))?;
            if bytes.len() != 65 {
                return Err(format!(
                    "signatures[{index}] must be 65 bytes, got {}",
                    bytes.len()
                ));
            }
            Ok(format!("0x{}", bytes_to_hex(&bytes)))
        })
        .collect()
}

fn nav_roundtrip_evm_bytes_array_arg(values: &[String]) -> String {
    if values.is_empty() {
        return "[]".to_string();
    }
    format!("[{}]", values.join(","))
}

fn maybe_push_bundle_operation(
    operations: &mut Vec<serde_json::Value>,
    bundle_dir: &std::path::Path,
    label: &str,
    file_name: &str,
    key_file: Option<&std::path::PathBuf>,
    key_flag: &str,
) -> Result<(), String> {
    let operation_file = bundle_dir.join(file_name);
    if !operation_file.exists() {
        return Ok(());
    }
    let key_file = key_file.ok_or_else(|| {
        format!(
            "bundle operation `{}` exists but {key_flag} was not provided",
            operation_file.display()
        )
    })?;
    let raw = std::fs::read_to_string(&operation_file).map_err(|error| {
        format!(
            "failed to read bundle operation `{}`: {error}",
            operation_file.display()
        )
    })?;
    let operation = serde_json::from_str::<postfiat_types::AssetTransactionOperation>(&raw)
        .map_err(|error| {
            format!(
                "bundle operation `{}` is not a valid asset operation: {error}",
                operation_file.display()
            )
        })?;
    operation
        .validate()
        .map_err(|error| format!("bundle operation `{}` is invalid: {error}", operation_file.display()))?;
    let source = certified_asset_op_source(&operation)?;
    operations.push(serde_json::json!({
        "label": label,
        "source": source,
        "key_file": key_file.display().to_string(),
        "operation": operation,
    }));
    Ok(())
}

fn certified_asset_op_source(
    operation: &postfiat_types::AssetTransactionOperation,
) -> Result<&str, String> {
    match operation {
        postfiat_types::AssetTransactionOperation::AssetCreate(operation) => Ok(&operation.issuer),
        postfiat_types::AssetTransactionOperation::TrustSet(operation) => Ok(&operation.account),
        postfiat_types::AssetTransactionOperation::IssuedPayment(operation) => Ok(&operation.from),
        postfiat_types::AssetTransactionOperation::AssetBurn(operation) => Ok(&operation.owner),
        postfiat_types::AssetTransactionOperation::AssetClawback(operation) => Ok(&operation.issuer),
        postfiat_types::AssetTransactionOperation::NavAssetRegister(operation) => Ok(&operation.issuer),
        postfiat_types::AssetTransactionOperation::NavReserveSubmit(operation) => Ok(&operation.submitter),
        postfiat_types::AssetTransactionOperation::NavReserveChallenge(operation) => Ok(&operation.challenger),
        postfiat_types::AssetTransactionOperation::NavEpochFinalize(operation) => Ok(&operation.issuer),
        postfiat_types::AssetTransactionOperation::MarketOpsPolicyRegister(operation) => Ok(&operation.issuer),
        postfiat_types::AssetTransactionOperation::MarketOpsFinalize(operation) => Ok(&operation.issuer),
        postfiat_types::AssetTransactionOperation::NavMintAtNav(operation) => Ok(&operation.issuer),
        postfiat_types::AssetTransactionOperation::NavRedeemAtNav(operation) => Ok(&operation.owner),
        postfiat_types::AssetTransactionOperation::NavHalt(operation) => Ok(&operation.issuer),
        postfiat_types::AssetTransactionOperation::NavProfileRegister(operation) => Ok(&operation.registrant),
        postfiat_types::AssetTransactionOperation::NavRedeemSettle(operation) => Ok(&operation.issuer),
        postfiat_types::AssetTransactionOperation::NavReserveAttest(operation) => Ok(&operation.attestor),
        postfiat_types::AssetTransactionOperation::NavAttestorRegister(operation) => Ok(&operation.attestor),
        postfiat_types::AssetTransactionOperation::VaultBridgeDepositPropose(operation) => Ok(&operation.proposer),
        postfiat_types::AssetTransactionOperation::VaultBridgeDepositChallenge(operation) => Ok(&operation.challenger),
        postfiat_types::AssetTransactionOperation::VaultBridgeDepositAttest(operation) => Ok(&operation.attestor),
        postfiat_types::AssetTransactionOperation::VaultBridgeDepositFinalize(operation) => Ok(&operation.finalizer),
        postfiat_types::AssetTransactionOperation::VaultBridgeDepositClaim(operation) => Ok(&operation.claimer),
        postfiat_types::AssetTransactionOperation::VaultBridgeReceiptSubmit(operation) => Ok(&operation.operator),
        postfiat_types::AssetTransactionOperation::VaultBridgeReceiptCount(operation) => Ok(&operation.operator),
        postfiat_types::AssetTransactionOperation::VaultBridgeMintFromReceipts(operation) => Ok(&operation.issuer),
        postfiat_types::AssetTransactionOperation::VaultBridgeBurnToRedeem(operation) => Ok(&operation.owner),
        postfiat_types::AssetTransactionOperation::VaultBridgeRedeemSettle(operation) => Ok(&operation.issuer_or_redemption_account),
        postfiat_types::AssetTransactionOperation::VaultBridgeBucketImpair(operation) => Ok(&operation.operator),
        postfiat_types::AssetTransactionOperation::VaultBridgeNavSubscriptionAllocate(operation) => Ok(&operation.operator),
        postfiat_types::AssetTransactionOperation::PftlUniswapRouteInit(operation) => Ok(&operation.operator),
        postfiat_types::AssetTransactionOperation::PftlUniswapPrimarySubscribe(operation) => Ok(&operation.subscriber),
        postfiat_types::AssetTransactionOperation::PftlUniswapExportDebit(operation) => Ok(&operation.owner),
        postfiat_types::AssetTransactionOperation::PftlUniswapDestinationConsume(operation) => Ok(&operation.operator),
        postfiat_types::AssetTransactionOperation::PftlUniswapRefundSource(operation) => Ok(&operation.operator),
        postfiat_types::AssetTransactionOperation::PftlUniswapReturnImport(operation) => Ok(&operation.operator),
    }
}

fn run_certified_asset_op_stage(
    op: &CertifiedAssetOpRequest,
    options: &CertifiedAssetOpsBatchOptions,
    submit_to_mempool: bool,
    sequence_override: Option<u64>,
) -> Result<CertifiedAssetOpStageReport, String> {
    let op_dir = options.artifact_dir.join(&op.label);
    std::fs::create_dir_all(&op_dir).map_err(|error| {
        format!(
            "certified asset ops operation artifact dir `{}` create failed: {error}",
            op_dir.display()
        )
    })?;

    let prepare_start = std::time::Instant::now();
    let operation_file = op_dir.join("operation.json");
    let operation_json = serde_json::to_string(&op.operation).map_err(|error| {
        format!("certified asset ops operation `{}` serialization failed: {error}", op.label)
    })?;
    write_json_file(&operation_file, &op.operation)?;
    let prepare_ms = monotonic_elapsed_ms(prepare_start);

    let mut timings = CertifiedAssetOpTimingsReport {
        prepare_ms,
        ..CertifiedAssetOpTimingsReport::default()
    };
    let mut quote_file = None;
    let mut signed_file = None;
    let mut submit_file = None;
    let mut tx_id = None;
    let mut sequence = None;
    let mut fee = None;

    if !options.prepare_only {
        let quote_start = std::time::Instant::now();
        let quote = asset_fee_quote(AssetFeeQuoteOptions {
            data_dir: options.data_dir.clone(),
            source: op.source.clone(),
            operation_json,
            sequence: sequence_override,
        })
        .map_err(|error| {
            format!(
                "certified asset ops quote `{}` from `{}` failed: {error}",
                op.label, op.source
            )
        })?;
        timings.quote_ms = monotonic_elapsed_ms(quote_start);
        let quote_path = op_dir.join("quote.json");
        write_json_file(&quote_path, &quote)?;
        sequence = Some(quote.sequence);
        fee = Some(quote.minimum_fee);
        quote_file = Some(quote_path.display().to_string());

        let sign_start = std::time::Instant::now();
        let signed = wallet_sign_asset_transaction(WalletSignAssetTransactionOptions {
            key_file: op.key_file.clone(),
            chain_id: quote.chain_id,
            genesis_hash: quote.genesis_hash,
            protocol_version: quote.protocol_version,
            fee: quote.minimum_fee,
            sequence: quote.sequence,
            expected_source: Some(quote.source),
            operation: quote.operation,
        })
        .map_err(|error| format!("certified asset ops sign `{}` failed: {error}", op.label))?;
        timings.sign_ms = monotonic_elapsed_ms(sign_start);
        let signed_path = op_dir.join("signed.json");
        write_json_file(&signed_path, &signed)?;
        signed_file = Some(signed_path.display().to_string());

        if submit_to_mempool {
            let submit_start = std::time::Instant::now();
            let signed_json = serde_json::to_string(&signed).map_err(|error| {
                format!(
                    "certified asset ops signed transaction `{}` serialization failed: {error}",
                    op.label
                )
            })?;
            let entry = submit_signed_asset_transaction_json_to_mempool(
                SignedAssetTransactionJsonSubmitOptions {
                    data_dir: options.data_dir.clone(),
                    signed_asset_transaction_json: signed_json,
                },
            )
            .map_err(|error| format!("certified asset ops submit `{}` failed: {error}", op.label))?;
            timings.submit_ms = monotonic_elapsed_ms(submit_start);
            let submit_path = op_dir.join("submit.json");
            write_json_file(&submit_path, &entry)?;
            tx_id = Some(entry.tx_id);
            submit_file = Some(submit_path.display().to_string());
        }
    }

    Ok(CertifiedAssetOpStageReport {
        label: op.label.clone(),
        source: op.source.clone(),
        transaction_kind: op.operation.transaction_kind().to_string(),
        operation_file: operation_file.display().to_string(),
        quote_file,
        signed_file,
        submit_file,
        tx_id,
        sequence,
        fee,
        timings_ms: timings,
    })
}

fn read_certified_asset_ops_request(path: &std::path::Path) -> Result<CertifiedAssetOpsRequest, String> {
    let raw = std::fs::read_to_string(path).map_err(|error| {
        format!(
            "failed to read certified asset ops request `{}`: {error}",
            path.display()
        )
    })?;
    let request = serde_json::from_str::<CertifiedAssetOpsRequest>(&raw).map_err(|error| {
        format!(
            "certified asset ops request `{}` is invalid JSON: {error}",
            path.display()
        )
    })?;
    if let Some(schema) = &request.schema {
        if schema != CERTIFIED_ASSET_OPS_REQUEST_SCHEMA {
            return Err(format!(
                "certified asset ops request uses unsupported schema `{schema}`"
            ));
        }
    }
    Ok(request)
}

fn validate_certified_asset_ops_request(request: &CertifiedAssetOpsRequest) -> Result<(), String> {
    if request.operations.is_empty() {
        return Err("certified asset ops request must contain at least one operation".to_string());
    }
    let mut labels = std::collections::BTreeSet::new();
    for op in &request.operations {
        if !labels.insert(op.label.clone()) {
            return Err(format!("duplicate certified asset op label `{}`", op.label));
        }
        validate_artifact_label(&op.label)?;
        if op.source.trim().is_empty() {
            return Err(format!("certified asset op `{}` has empty source", op.label));
        }
        op.operation
            .validate()
            .map_err(|error| format!("certified asset op `{}` is invalid: {error}", op.label))?;
        if !op.key_file.is_file() {
            return Err(format!(
                "certified asset op `{}` key file `{}` is not a file",
                op.label,
                op.key_file.display()
            ));
        }
    }
    let mut label_positions = std::collections::BTreeMap::<String, usize>::new();
    for (index, op) in request.operations.iter().enumerate() {
        label_positions.insert(op.label.clone(), index);
    }
    for (index, op) in request.operations.iter().enumerate() {
        let mut dependency_labels = std::collections::BTreeSet::new();
        for dependency in &op.dependencies {
            validate_artifact_label(&dependency.label)?;
            if !dependency_labels.insert(dependency.label.clone()) {
                return Err(format!(
                    "certified asset op `{}` declares duplicate dependency `{}`",
                    op.label, dependency.label
                ));
            }
            if dependency.label == op.label {
                return Err(format!(
                    "certified asset op `{}` cannot depend on itself",
                    op.label
                ));
            }
            match dependency.mode.as_str() {
                "same_round" => {
                    let Some(dependency_index) = label_positions.get(&dependency.label).copied()
                    else {
                        return Err(format!(
                            "certified asset op `{}` same_round dependency `{}` is not present in this request",
                            op.label, dependency.label
                        ));
                    };
                    if dependency_index >= index {
                        return Err(format!(
                            "certified asset op `{}` same_round dependency `{}` must appear earlier in the request",
                            op.label, dependency.label
                        ));
                    }
                }
                "prior_round" => {
                    if label_positions.contains_key(&dependency.label) {
                        return Err(format!(
                            "certified asset op `{}` dependency `{}` requires prior_round but is present in the same request",
                            op.label, dependency.label
                        ));
                    }
                }
                other => {
                    return Err(format!(
                        "certified asset op `{}` dependency `{}` uses unsupported mode `{other}`",
                        op.label, dependency.label
                    ));
                }
            }
        }
    }
    Ok(())
}

fn certified_asset_ops_dependency_report(
    request: &CertifiedAssetOpsRequest,
) -> CertifiedAssetOpsDependencyReport {
    let mut declarations = Vec::new();
    let mut candidate_batch_classes = Vec::new();
    let mut same_round_dependency_count = 0usize;
    let mut prior_round_dependency_count = 0usize;
    for op in &request.operations {
        for dependency in &op.dependencies {
            let candidate_batch_class =
                certified_asset_ops_candidate_batch_class(request, op, dependency);
            match dependency.mode.as_str() {
                "same_round" => {
                    same_round_dependency_count += 1;
                    if let Some(candidate_batch_class) = candidate_batch_class.as_ref() {
                        candidate_batch_classes.push(candidate_batch_class.clone());
                    }
                }
                "prior_round" => prior_round_dependency_count += 1,
                _ => {}
            }
            declarations.push(CertifiedAssetOpsDependencyDeclarationReport {
                operation: op.label.clone(),
                depends_on: dependency.label.clone(),
                mode: dependency.mode.clone(),
                candidate_batch_class,
                reason: dependency.reason.clone(),
            });
        }
    }
    candidate_batch_classes.sort();
    candidate_batch_classes.dedup();
    let mut live_round_compression_blockers = Vec::new();
    if prior_round_dependency_count > 0 {
        live_round_compression_blockers.push(
            "request contains prior_round dependencies that must remain separate certified rounds"
                .to_string(),
        );
    }
    if same_round_dependency_count > 0 {
        live_round_compression_blockers.push(
            "same_round dependency candidates require replay-equivalence corpus evidence before live round compression"
                .to_string(),
        );
    }
    CertifiedAssetOpsDependencyReport {
        declared_dependency_count: declarations.len(),
        same_round_dependency_count,
        prior_round_dependency_count,
        same_round_batch_eligible: prior_round_dependency_count == 0,
        candidate_batch_classes,
        replay_equivalence_required: same_round_dependency_count > 0,
        live_round_compression_ready: live_round_compression_blockers.is_empty(),
        live_round_compression_blockers,
        declarations,
    }
}

fn certified_asset_ops_candidate_batch_class(
    request: &CertifiedAssetOpsRequest,
    operation: &CertifiedAssetOpRequest,
    dependency: &CertifiedAssetOpDependency,
) -> Option<String> {
    if dependency.mode != "same_round" {
        return None;
    }
    let dependency_op = request
        .operations
        .iter()
        .find(|candidate| candidate.label == dependency.label)?;
    Some(certified_asset_ops_candidate_batch_class_from_kinds(
        dependency_op.operation.transaction_kind(),
        operation.operation.transaction_kind(),
    ))
}

fn certified_asset_ops_dependency_report_candidate_batch_classes(
    report: &CertifiedAssetOpsDependencyReport,
) -> Vec<String> {
    let mut classes = report.candidate_batch_classes.clone();
    for declaration in &report.declarations {
        if declaration.mode != "same_round" {
            continue;
        }
        if let Some(candidate_batch_class) = declaration.candidate_batch_class.as_ref() {
            classes.push(candidate_batch_class.clone());
        } else {
            classes.push(certified_asset_ops_candidate_batch_class_from_labels(
                &declaration.depends_on,
                &declaration.operation,
            ));
        }
    }
    classes.sort();
    classes.dedup();
    classes
}

fn certified_asset_ops_candidate_batch_class_from_kinds(
    dependency_kind: &str,
    operation_kind: &str,
) -> String {
    match (dependency_kind, operation_kind) {
        (
            postfiat_types::VAULT_BRIDGE_DEPOSIT_PROPOSE_TRANSACTION_KIND,
            postfiat_types::VAULT_BRIDGE_DEPOSIT_ATTEST_TRANSACTION_KIND,
        ) => "vault_bridge_deposit_propose_attest".to_string(),
        (
            postfiat_types::VAULT_BRIDGE_DEPOSIT_FINALIZE_TRANSACTION_KIND,
            postfiat_types::VAULT_BRIDGE_DEPOSIT_CLAIM_TRANSACTION_KIND,
        ) => "vault_bridge_deposit_finalize_claim".to_string(),
        (
            postfiat_types::VAULT_BRIDGE_RECEIPT_SUBMIT_TRANSACTION_KIND,
            postfiat_types::VAULT_BRIDGE_RECEIPT_COUNT_TRANSACTION_KIND,
        ) => "vault_bridge_receipt_submit_count".to_string(),
        (
            postfiat_types::VAULT_BRIDGE_NAV_SUBSCRIPTION_ALLOCATE_TRANSACTION_KIND,
            postfiat_types::NAV_MINT_AT_NAV_TRANSACTION_KIND,
        ) => "nav_subscription_allocate_mint_at_nav".to_string(),
        (
            postfiat_types::NAV_REDEEM_AT_NAV_TRANSACTION_KIND,
            postfiat_types::NAV_REDEEM_SETTLE_TRANSACTION_KIND,
        ) => "nav_redeem_at_nav_settle".to_string(),
        (
            postfiat_types::NAV_RESERVE_SUBMIT_TRANSACTION_KIND,
            postfiat_types::NAV_EPOCH_FINALIZE_TRANSACTION_KIND,
        ) => "nav_reserve_submit_epoch_finalize".to_string(),
        _ => format!("{dependency_kind}_then_{operation_kind}"),
    }
}

fn certified_asset_ops_candidate_batch_class_from_labels(
    dependency_label: &str,
    operation_label: &str,
) -> String {
    match (dependency_label, operation_label) {
        ("propose", "attest") => "vault_bridge_deposit_propose_attest".to_string(),
        ("finalize", "claim") => "vault_bridge_deposit_finalize_claim".to_string(),
        ("receipt-submit", "receipt-count") => "vault_bridge_receipt_submit_count".to_string(),
        ("nav-subscription-allocate", "nav-mint-at-nav") => {
            "nav_subscription_allocate_mint_at_nav".to_string()
        }
        ("nav-redeem-at-nav", "nav-redeem-settle") => "nav_redeem_at_nav_settle".to_string(),
        ("nav-reserve-submit", "nav-epoch-finalize") => {
            "nav_reserve_submit_epoch_finalize".to_string()
        }
        _ => format!(
            "{}_then_{}",
            certified_asset_ops_sanitize_candidate_class_label(dependency_label),
            certified_asset_ops_sanitize_candidate_class_label(operation_label)
        ),
    }
}

fn certified_asset_ops_sanitize_candidate_class_label(label: &str) -> String {
    let mut out = String::with_capacity(label.len());
    let mut last_was_underscore = false;
    for ch in label.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_was_underscore = false;
        } else if !last_was_underscore {
            out.push('_');
            last_was_underscore = true;
        }
    }
    while out.ends_with('_') {
        out.pop();
    }
    if out.is_empty() {
        "unknown".to_string()
    } else {
        out
    }
}

fn certified_asset_op_add_dependency(
    operations: &mut [CertifiedAssetOpRequest],
    operation_label: &str,
    dependency_label: &str,
    mode: &str,
    reason: &str,
) -> Result<(), String> {
    validate_artifact_label(operation_label)?;
    validate_artifact_label(dependency_label)?;
    let Some(operation) = operations
        .iter_mut()
        .find(|operation| operation.label == operation_label)
    else {
        return Ok(());
    };
    operation.dependencies.push(CertifiedAssetOpDependency {
        label: dependency_label.to_string(),
        mode: mode.to_string(),
        reason: Some(reason.to_string()),
    });
    Ok(())
}

fn validate_artifact_label(label: &str) -> Result<(), String> {
    if label.is_empty() {
        return Err("certified asset op label must not be empty".to_string());
    }
    if label == "." || label == ".." {
        return Err(format!("certified asset op label `{label}` is not allowed"));
    }
    if !label
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.')
    {
        return Err(format!(
            "certified asset op label `{label}` must contain only ASCII letters, numbers, '.', '-' or '_'"
        ));
    }
    Ok(())
}

fn prepare_artifact_dir(path: &std::path::Path, overwrite: bool, resume: bool) -> Result<(), String> {
    if path.exists() {
        if overwrite {
            std::fs::remove_dir_all(path).map_err(|error| {
                format!(
                    "failed to remove existing artifact dir `{}`: {error}",
                    path.display()
                )
            })?;
        } else if !resume {
            let mut entries = std::fs::read_dir(path)
                .map_err(|error| format!("failed to inspect artifact dir `{}`: {error}", path.display()))?;
            if entries.next().is_some() {
                return Err(format!(
                    "artifact dir `{}` is not empty; use --resume or --overwrite",
                    path.display()
                ));
            }
        }
    }
    std::fs::create_dir_all(path)
        .map_err(|error| format!("failed to create artifact dir `{}`: {error}", path.display()))
}

fn prepare_nav_roundtrip_artifact_dir(path: &std::path::Path, overwrite: bool) -> Result<(), String> {
    if path.exists() {
        if overwrite {
            std::fs::remove_dir_all(path).map_err(|error| {
                format!(
                    "failed to remove existing NAV roundtrip artifact dir `{}`: {error}",
                    path.display()
                )
            })?;
        } else if path.join("preflight.json").exists() {
            return Err(format!(
                "NAV roundtrip artifact `{}` already exists; use --overwrite or a new artifact dir",
                path.join("preflight.json").display()
            ));
        }
    }
    std::fs::create_dir_all(path).map_err(|error| {
        format!(
            "failed to create NAV roundtrip artifact dir `{}`: {error}",
            path.display()
        )
    })
}

fn cast_balance(cast_binary: &str, rpc_url: &str, address: &str) -> Result<u128, String> {
    let output = run_nav_roundtrip_cast(
        cast_binary,
        &["balance", address, "--rpc-url", rpc_url],
        true,
    )?;
    parse_cast_u128(&output.stdout, "cast balance")
}

fn cast_u128_call(
    cast_binary: &str,
    rpc_url: &str,
    contract: &str,
    signature: &str,
    args: &[&str],
) -> Result<u128, String> {
    let mut command_args = Vec::with_capacity(5 + args.len());
    command_args.extend(["call", contract, signature]);
    command_args.extend(args.iter().copied());
    command_args.extend(["--rpc-url", rpc_url]);
    let output = run_nav_roundtrip_cast(cast_binary, &command_args, true)?;
    parse_cast_u128(&output.stdout, signature)
}

fn cast_optional_u64_call(
    cast_binary: &str,
    rpc_url: &str,
    contract: &str,
    signature: &str,
    args: &[&str],
) -> Result<Option<u64>, String> {
    let mut command_args = Vec::with_capacity(5 + args.len());
    command_args.extend(["call", contract, signature]);
    command_args.extend(args.iter().copied());
    command_args.extend(["--rpc-url", rpc_url]);
    let output = run_nav_roundtrip_cast(cast_binary, &command_args, false)?;
    if !output.status_success {
        return Ok(None);
    }
    parse_cast_u128(&output.stdout, signature)
        .and_then(|value| {
            u64::try_from(value)
                .map_err(|_| format!("{signature} returned value too large for u64: {value}"))
        })
        .map(Some)
}

fn cast_bool_call(
    cast_binary: &str,
    rpc_url: &str,
    contract: &str,
    signature: &str,
    args: &[&str],
) -> Result<bool, String> {
    let mut command_args = Vec::with_capacity(5 + args.len());
    command_args.extend(["call", contract, signature]);
    command_args.extend(args.iter().copied());
    command_args.extend(["--rpc-url", rpc_url]);
    let output = run_nav_roundtrip_cast(cast_binary, &command_args, true)?;
    let token = output
        .stdout
        .split_whitespace()
        .next()
        .ok_or_else(|| format!("{signature} returned empty output"))?;
    match token {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        other if other.starts_with("0x") => {
            let stripped = other.trim_start_matches("0x");
            if stripped.chars().all(|ch| ch == '0') {
                return Ok(false);
            }
            if stripped.len() <= 64
                && stripped[..stripped.len().saturating_sub(1)]
                    .chars()
                    .all(|ch| ch == '0')
                && stripped.ends_with('1')
            {
                return Ok(true);
            }
            Err(format!("{signature} returned invalid bool `{other}`"))
        }
        other => Err(format!("{signature} returned invalid bool `{other}`")),
    }
}

fn cast_bytes32_call(
    cast_binary: &str,
    rpc_url: &str,
    contract: &str,
    signature: &str,
    args: &[&str],
) -> Result<String, String> {
    let mut command_args = Vec::with_capacity(5 + args.len());
    command_args.extend(["call", contract, signature]);
    command_args.extend(args.iter().copied());
    command_args.extend(["--rpc-url", rpc_url]);
    let output = run_nav_roundtrip_cast(cast_binary, &command_args, true)?;
    let token = output
        .stdout
        .split_whitespace()
        .next()
        .ok_or_else(|| format!("{signature} returned empty output"))?;
    let stripped = token.strip_prefix("0x").unwrap_or(token);
    if stripped.len() != 64 || !stripped.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err(format!(
            "{signature} returned invalid bytes32 `{}`",
            output.stdout.trim()
        ));
    }
    Ok(format!("0x{}", stripped.to_ascii_lowercase()))
}

fn cast_code_bytes(cast_binary: &str, rpc_url: &str, address: &str) -> Result<usize, String> {
    let output = run_nav_roundtrip_cast(
        cast_binary,
        &["code", address, "--rpc-url", rpc_url],
        true,
    )?;
    let code = output.stdout.trim();
    if code == "0x" || code.is_empty() {
        return Ok(0);
    }
    let hex = code.strip_prefix("0x").unwrap_or(code);
    if hex.len() % 2 != 0 {
        return Err(format!("cast code for `{address}` returned odd-length hex"));
    }
    Ok(hex.len() / 2)
}

fn cast_call_returns_hex(
    cast_binary: &str,
    rpc_url: &str,
    contract: &str,
    signature: &str,
    args: &[&str],
) -> Result<bool, String> {
    let mut command_args = Vec::with_capacity(5 + args.len());
    command_args.extend(["call", contract, signature]);
    command_args.extend(args.iter().copied());
    command_args.extend(["--rpc-url", rpc_url]);
    let output = run_nav_roundtrip_cast(cast_binary, &command_args, false)?;
    if !output.status_success {
        return Ok(false);
    }
    let first = output.stdout.split_whitespace().next().unwrap_or("");
    Ok(first.starts_with("0x") && first.len() > 2)
}

fn cast_calldata(cast_binary: &str, signature: &str, args: &[&str]) -> Result<String, String> {
    let mut command_args = Vec::with_capacity(2 + args.len());
    command_args.extend(["calldata", signature]);
    command_args.extend(args.iter().copied());
    let output = run_nav_roundtrip_cast(cast_binary, &command_args, true)?;
    let calldata = output.stdout.trim();
    if !calldata.starts_with("0x") || calldata.len() <= 2 {
        return Err(format!(
            "cast calldata `{signature}` returned invalid calldata `{calldata}`"
        ));
    }
    Ok(calldata.to_string())
}

#[derive(Debug)]
struct NavRoundtripStakeHubLaunchSessionGuard {
    stakehub_home: std::path::PathBuf,
    session_id: String,
    timeout_secs: u64,
    open_file: std::path::PathBuf,
    close_file: std::path::PathBuf,
    active: bool,
}

impl NavRoundtripStakeHubLaunchSessionGuard {
    #[allow(clippy::too_many_arguments)]
    fn open(
        stakehub_home: &std::path::Path,
        artifact_dir: &std::path::Path,
        session_id: &str,
        source_chain_id: u64,
        stakehub_wallet: &str,
        usdc_address: &str,
        vault_address: &str,
        verifier_address: &str,
        usdc_budget_atoms: u64,
        timeout_secs: u64,
    ) -> Result<Self, String> {
        std::fs::create_dir_all(artifact_dir).map_err(|error| {
            format!(
                "failed to create StakeHub launch-session artifact dir `{}`: {error}",
                artifact_dir.display()
            )
        })?;
        let status_file = artifact_dir.join("agent-status.json");
        let close_existing_file = artifact_dir.join("agent-close-existing-session.json");
        let open_file = artifact_dir.join("agent-open-session.json");
        let close_file = artifact_dir.join("agent-close-session.json");

        let status_response =
            stakehub_agent_call(stakehub_home, &serde_json::json!({ "op": "status" }), timeout_secs)?;
        require_agent_ok(&status_response, "status")?;
        write_json_file(&status_file, &status_response)?;
        if status_response
            .get("unlocked")
            .and_then(serde_json::Value::as_bool)
            != Some(true)
        {
            return Err("StakeHub agent is locked; run `stakehub agent unlock` first".to_string());
        }

        let close_existing = stakehub_agent_call(
            stakehub_home,
            &serde_json::json!({
                "op": "close_launch_session",
                "session_id": session_id,
            }),
            timeout_secs,
        )?;
        require_agent_ok(&close_existing, "close existing launch session")?;
        write_json_file(&close_existing_file, &close_existing)?;

        let open_request = serde_json::json!({
            "op": "open_launch_session",
            "session_id": session_id,
            "chain_id": source_chain_id,
            "allowlist": [
                stakehub_wallet,
                usdc_address,
                vault_address,
                verifier_address,
            ],
            "expected_deploys": [{
                "label": "nav_roundtrip_noop_deploy",
                "bytecode_hash": "0x0000000000000000000000000000000000000000000000000000000000000000",
                "bytecode_len": 1,
            }],
            "usdc_address": usdc_address,
            "usdc_budget": usdc_budget_atoms,
            "close_after_action": "claim-withdrawal",
            "ttl_seconds": 3600,
        });
        let open_response = stakehub_agent_call(stakehub_home, &open_request, timeout_secs)?;
        require_agent_ok(&open_response, "open launch session")?;
        write_json_file(&open_file, &open_response)?;

        Ok(Self {
            stakehub_home: stakehub_home.to_path_buf(),
            session_id: session_id.to_string(),
            timeout_secs,
            open_file,
            close_file,
            active: true,
        })
    }

    fn close(&mut self) -> Result<(), String> {
        if !self.active {
            return Ok(());
        }
        let close_response = stakehub_agent_call(
            &self.stakehub_home,
            &serde_json::json!({
                "op": "close_launch_session",
                "session_id": self.session_id,
            }),
            self.timeout_secs,
        )?;
        require_agent_ok(&close_response, "close launch session")?;
        write_json_file(&self.close_file, &close_response)?;
        self.active = false;
        Ok(())
    }
}

impl Drop for NavRoundtripStakeHubLaunchSessionGuard {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        let close_result = stakehub_agent_call(
            &self.stakehub_home,
            &serde_json::json!({
                "op": "close_launch_session",
                "session_id": self.session_id,
            }),
            self.timeout_secs,
        )
        .and_then(|response| {
            require_agent_ok(&response, "close launch session")?;
            Ok(response)
        });
        let artifact = match close_result {
            Ok(response) => response,
            Err(error) => serde_json::json!({
                "ok": false,
                "drop_cleanup": true,
                "error": error,
            }),
        };
        let _ = write_json_file(&self.close_file, &artifact);
        self.active = false;
    }
}

fn stakehub_agent_call(
    stakehub_home: &std::path::Path,
    request: &serde_json::Value,
    timeout_secs: u64,
) -> Result<serde_json::Value, String> {
    let socket_path = stakehub_home.join("agent.sock");
    if !socket_path.exists() {
        return Err(format!(
            "StakeHub agent socket `{}` does not exist; run `stakehub agent run` and `stakehub agent unlock`",
            socket_path.display()
        ));
    }
    let mut stream = std::os::unix::net::UnixStream::connect(&socket_path).map_err(|error| {
        format!(
            "failed to connect to StakeHub agent socket `{}`: {error}",
            socket_path.display()
        )
    })?;
    let timeout = std::time::Duration::from_secs(timeout_secs.max(1));
    stream
        .set_read_timeout(Some(timeout))
        .map_err(|error| format!("failed to set StakeHub agent read timeout: {error}"))?;
    stream
        .set_write_timeout(Some(timeout))
        .map_err(|error| format!("failed to set StakeHub agent write timeout: {error}"))?;
    let request_json = serde_json::to_string(request)
        .map_err(|error| format!("failed to serialize StakeHub agent request: {error}"))?;
    {
        use std::io::Write as _;
        stream
            .write_all(request_json.as_bytes())
            .and_then(|_| stream.write_all(b"\n"))
            .map_err(|error| format!("failed to write StakeHub agent request: {error}"))?;
    }
    let mut line = String::new();
    {
        use std::io::BufRead as _;
        let mut reader = std::io::BufReader::new(stream);
        reader
            .read_line(&mut line)
            .map_err(|error| format!("failed to read StakeHub agent response: {error}"))?;
    }
    if line.trim().is_empty() {
        return Err("StakeHub agent returned an empty response".to_string());
    }
    serde_json::from_str::<serde_json::Value>(&line)
        .map_err(|error| format!("StakeHub agent response was not valid JSON: {error}"))
}

fn nav_roundtrip_agent_evm_tx(
    options: &NavRoundtripEvmWithdrawalOptions,
    to: &str,
    data: &str,
    label: &str,
    session_action: &str,
) -> Result<serde_json::Value, String> {
    let response = stakehub_agent_call(
        &options.stakehub_home,
        &serde_json::json!({
            "op": "evm_contract_tx",
            "to": to,
            "data": data,
            "rpc_url": options.source_rpc_url,
            "chain_id": options.source_chain_id,
            "label": label,
            "session_id": options.session_id,
            "session_action": session_action,
            "gas_usd": 10,
        }),
        options.agent_timeout_secs,
    )?;
    require_agent_ok(&response, label)?;
    Ok(response)
}

fn require_agent_ok(response: &serde_json::Value, label: &str) -> Result<(), String> {
    if response.get("ok").and_then(serde_json::Value::as_bool) == Some(true) {
        return Ok(());
    }
    let error = response
        .get("error")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("missing error");
    Err(format!("StakeHub agent {label} failed: {error}"))
}

fn agent_tx_hash(response: &serde_json::Value, label: &str) -> Result<String, String> {
    response
        .get("tx")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("StakeHub agent {label} response did not include tx"))
}

fn agent_gas_used(response: &serde_json::Value, label: &str) -> Result<u64, String> {
    response
        .get("gas_used")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| format!("StakeHub agent {label} response did not include gas_used"))
}

fn nav_roundtrip_evm_receipt_watch(
    label: &str,
    response: &serde_json::Value,
    source_rpc_provider_class: &str,
    elapsed_ms: f64,
) -> Result<NavRoundtripEvmReceiptWatchReport, String> {
    Ok(NavRoundtripEvmReceiptWatchReport {
        label: label.to_string(),
        tx_hash: agent_tx_hash(response, label)?,
        source_rpc_provider_class: source_rpc_provider_class.to_string(),
        confirmation_source: "stakehub_agent_response".to_string(),
        status: "confirmed".to_string(),
        gas_used: agent_gas_used(response, label)?,
        elapsed_ms,
    })
}

#[derive(Debug, Clone)]
struct NavRoundtripCastOutput {
    status_success: bool,
    stdout: String,
}

fn run_nav_roundtrip_cast(
    cast_binary: &str,
    args: &[&str],
    require_success: bool,
) -> Result<NavRoundtripCastOutput, String> {
    let output = std::process::Command::new(cast_binary)
        .args(args)
        .output()
        .map_err(|error| format!("failed to run `{cast_binary}`: {error}"))?;
    let stdout = String::from_utf8(output.stdout)
        .map_err(|error| format!("`{cast_binary}` stdout was not UTF-8: {error}"))?;
    let stderr = String::from_utf8(output.stderr)
        .map_err(|error| format!("`{cast_binary}` stderr was not UTF-8: {error}"))?;
    let status_success = output.status.success();
    if require_success && !status_success {
        return Err(format!(
            "`{cast_binary} {}` failed: {}",
            args.join(" "),
            stderr.trim()
        ));
    }
    Ok(NavRoundtripCastOutput {
        status_success,
        stdout,
    })
}

fn parse_cast_u128(output: &str, label: &str) -> Result<u128, String> {
    let token = output
        .split_whitespace()
        .next()
        .ok_or_else(|| format!("{label} returned empty output"))?;
    if let Some(hex) = token.strip_prefix("0x") {
        u128::from_str_radix(hex, 16)
            .map_err(|error| format!("{label} returned invalid hex integer `{token}`: {error}"))
    } else {
        token
            .parse::<u128>()
            .map_err(|error| format!("{label} returned invalid integer `{token}`: {error}"))
    }
}

fn nav_roundtrip_dummy_hex(bytes: usize, byte: u8) -> String {
    let mut out = String::with_capacity(2 + bytes * 2);
    out.push_str("0x");
    for _ in 0..bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut out, "{byte:02x}");
    }
    out
}

fn default_stakehub_home() -> std::path::PathBuf {
    if let Ok(value) = std::env::var("STAKEHUB_HOME") {
        return expand_tilde_path(&value);
    }
    expand_tilde_path("~/.stakehub")
}

fn expand_tilde_path(value: &str) -> std::path::PathBuf {
    if value == "~" {
        if let Ok(home) = std::env::var("HOME") {
            return std::path::PathBuf::from(home);
        }
    }
    if let Some(rest) = value.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return std::path::PathBuf::from(home).join(rest);
        }
    }
    std::path::PathBuf::from(value)
}

fn request_to_json(request: &CertifiedAssetOpsRequest) -> Result<serde_json::Value, String> {
    let operations = request
        .operations
        .iter()
        .map(|op| {
            let dependencies = op
                .dependencies
                .iter()
                .map(|dependency| {
                    serde_json::json!({
                        "label": dependency.label,
                        "mode": dependency.mode,
                        "reason": dependency.reason,
                    })
                })
                .collect::<Vec<_>>();
            Ok(serde_json::json!({
                "label": op.label,
                "source": op.source,
                "key_file": op.key_file.display().to_string(),
                "operation": op.operation,
                "dependencies": dependencies,
            }))
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(serde_json::json!({
        "schema": request.schema.as_deref().unwrap_or(CERTIFIED_ASSET_OPS_REQUEST_SCHEMA),
        "operations": operations,
    }))
}

fn write_json_file<T: serde::Serialize>(path: &std::path::Path, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create parent directory `{}` for `{}`: {error}",
                parent.display(),
                path.display()
            )
        })?;
    }
    let json = serde_json::to_string_pretty(value).map_err(|error| {
        format!("failed to serialize JSON for `{}`: {error}", path.display())
    })?;
    std::fs::write(path, format!("{json}\n"))
        .map_err(|error| format!("failed to write `{}`: {error}", path.display()))
}

fn write_text_file(path: &std::path::Path, value: &str) -> Result<(), String> {
    std::fs::write(path, format!("{value}\n"))
        .map_err(|error| format!("failed to write `{}`: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_word_u128(bytes: &mut [u8], offset: usize, value: u128) {
        bytes[offset + 16..offset + 32].copy_from_slice(&value.to_be_bytes());
    }

    #[test]
    fn sp1_public_values_decoder_includes_cash_in_verified_net_assets() {
        let mut bytes = vec![0_u8; 32 + 96 + 512];
        write_word_u128(&mut bytes, 0, 32);
        write_word_u128(
            &mut bytes,
            32,
            u128::from(postfiat_types::AGGREGATE_PUBLIC_VALUES_V2_SCHEMA_VERSION),
        );
        bytes[96..128].copy_from_slice(&[0x11; 32]);

        let totals_offset = 128;
        write_word_u128(&mut bytes, totals_offset, 1_000);
        write_word_u128(&mut bytes, totals_offset + 96, 300);
        write_word_u128(&mut bytes, totals_offset + 192, 9_999);
        write_word_u128(&mut bytes, totals_offset + 224, 125);

        let decoded = nav_roundtrip_decode_sp1_public_values(&bytes).unwrap();

        assert_eq!(decoded.policy_hash_hex, "11".repeat(32));
        assert_eq!(decoded.verified_net_assets, 1_175);
    }
}
