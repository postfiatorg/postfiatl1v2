// Provider-neutral certified asset-operation runner.
//
// This is intentionally separate from the retired NAV roundtrip campaign harness.


const CERTIFIED_ASSET_OPS_REQUEST_SCHEMA: &str = "postfiat-certified-asset-ops-request-v1";
const CERTIFIED_ASSET_OPS_REPORT_SCHEMA: &str = "postfiat-certified-asset-ops-report-v1";
const CERTIFIED_ASSET_OPS_FROM_BUNDLE_REPORT_SCHEMA: &str =
    "postfiat-certified-asset-ops-from-bundle-report-v1";
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct CertifiedAssetOpsRequest {
    #[serde(default)]
    schema: Option<String>,
    operations: Vec<CertifiedAssetOpRequest>,
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct CertifiedAssetOpRequest {
    label: String,
    source: String,
    key_file: std::path::PathBuf,
    operation: postfiat_types::AssetTransactionOperation,
    #[serde(default)]
    dependencies: Vec<CertifiedAssetOpDependency>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct CertifiedAssetOpDependency {
    label: String,
    mode: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

#[derive(Debug, Clone)]
struct CertifiedAssetOpsBatchOptions {
    data_dir: std::path::PathBuf,
    topology_file: std::path::PathBuf,
    key_file: std::path::PathBuf,
    proposal_key_file: Option<std::path::PathBuf>,
    ops_file: std::path::PathBuf,
    artifact_dir: std::path::PathBuf,
    max_transactions: Option<usize>,
    require_local_proposer: bool,
    require_signed_proposal: bool,
    allow_peer_failures: bool,
    quorum_early_full_propagation: bool,
    local_apply_before_certified_send: bool,
    defer_certified_sends: bool,
    block_height: Option<u64>,
    view: Option<u64>,
    timeout_certificate_file: Option<std::path::PathBuf>,
    timeout_ms: u64,
    send_retries: usize,
    retry_backoff_ms: u64,
    allow_existing_mempool: bool,
    resume: bool,
    overwrite: bool,
    prepare_only: bool,
    batch_only: bool,
}

#[derive(Debug, Clone)]
struct CertifiedAssetOpsFromBundleOptions {
    bundle_dir: std::path::PathBuf,
    output_file: std::path::PathBuf,
    proposer_key_file: Option<std::path::PathBuf>,
    attestor_key_file: Option<std::path::PathBuf>,
    finalizer_key_file: Option<std::path::PathBuf>,
    claimer_key_file: Option<std::path::PathBuf>,
    owner_key_file: Option<std::path::PathBuf>,
    include_deposit_claim: bool,
    overwrite: bool,
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct CertifiedAssetOpsBatchReport {
    schema: String,
    request_schema: Option<String>,
    data_dir: String,
    topology_file: String,
    artifact_dir: String,
    operation_count: usize,
    max_transactions: usize,
    allow_existing_mempool: bool,
    prepare_only: bool,
    batch_only: bool,
    start_height: u64,
    start_state_root: String,
    start_mempool_pending: u64,
    end_height: Option<u64>,
    end_state_root: Option<String>,
    end_mempool_pending: Option<u64>,
    operations: Vec<CertifiedAssetOpStageReport>,
    #[serde(default)]
    dependency_report: CertifiedAssetOpsDependencyReport,
    batch_file: Option<String>,
    round_artifact_dir: Option<String>,
    round_ok: Option<bool>,
    timings_ms: CertifiedAssetOpsTimingsReport,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct CertifiedAssetOpsDependencyReport {
    declared_dependency_count: usize,
    same_round_dependency_count: usize,
    prior_round_dependency_count: usize,
    same_round_batch_eligible: bool,
    #[serde(default)]
    candidate_batch_classes: Vec<String>,
    #[serde(default)]
    replay_equivalence_required: bool,
    #[serde(default = "serde_default_true")]
    live_round_compression_ready: bool,
    #[serde(default)]
    live_round_compression_blockers: Vec<String>,
    declarations: Vec<CertifiedAssetOpsDependencyDeclarationReport>,
}

fn serde_default_true() -> bool {
    true
}

impl Default for CertifiedAssetOpsDependencyReport {
    fn default() -> Self {
        Self {
            declared_dependency_count: 0,
            same_round_dependency_count: 0,
            prior_round_dependency_count: 0,
            same_round_batch_eligible: true,
            candidate_batch_classes: Vec::new(),
            replay_equivalence_required: false,
            live_round_compression_ready: true,
            live_round_compression_blockers: Vec::new(),
            declarations: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct CertifiedAssetOpsDependencyDeclarationReport {
    operation: String,
    depends_on: String,
    mode: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    candidate_batch_class: Option<String>,
    reason: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct CertifiedAssetOpStageReport {
    label: String,
    source: String,
    transaction_kind: String,
    operation_file: String,
    quote_file: Option<String>,
    signed_file: Option<String>,
    submit_file: Option<String>,
    tx_id: Option<String>,
    sequence: Option<u64>,
    fee: Option<u64>,
    timings_ms: CertifiedAssetOpTimingsReport,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct CertifiedAssetOpTimingsReport {
    prepare_ms: f64,
    quote_ms: f64,
    sign_ms: f64,
    submit_ms: f64,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct CertifiedAssetOpsTimingsReport {
    total_ms: f64,
    preflight_ms: f64,
    operations_ms: f64,
    certify_ms: f64,
    final_status_ms: f64,
}

#[derive(Debug, Clone, serde::Serialize)]
struct CertifiedAssetOpsFromBundleReport {
    schema: String,
    bundle_dir: String,
    output_file: String,
    operation_count: usize,
    labels: Vec<String>,
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
        postfiat_types::AssetTransactionOperation::FxFixRegisterV1(operation) => Ok(&operation.operator),
        postfiat_types::AssetTransactionOperation::FxFixPauseV1(operation) => Ok(&operation.operator),
        postfiat_types::AssetTransactionOperation::FxFixReservationCreateV1(operation) => Ok(&operation.operator),
        postfiat_types::AssetTransactionOperation::FxFixReservationReleaseV1(operation) => Ok(&operation.operator),
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
        postfiat_types::AssetTransactionOperation::VaultBridgeFastIngressLifecycle(operation) => Ok(&operation.operator),
        postfiat_types::AssetTransactionOperation::VaultBridgeReceiptSubmit(operation) => Ok(&operation.operator),
        postfiat_types::AssetTransactionOperation::VaultBridgeReceiptCount(operation) => Ok(&operation.operator),
        postfiat_types::AssetTransactionOperation::VaultBridgeMintFromReceipts(operation) => Ok(&operation.issuer),
        postfiat_types::AssetTransactionOperation::VaultBridgeBurnToRedeem(operation) => Ok(&operation.owner),
        postfiat_types::AssetTransactionOperation::VaultBridgeRedeemSettle(operation) => Ok(&operation.issuer_or_redemption_account),
        postfiat_types::AssetTransactionOperation::VaultBridgeBucketImpair(operation) => Ok(&operation.operator),
        postfiat_types::AssetTransactionOperation::VaultBridgeNavSubscriptionAllocate(operation) => Ok(&operation.operator),
        postfiat_types::AssetTransactionOperation::PftlUniswapRouteInit(operation) => Ok(&operation.operator),
        postfiat_types::AssetTransactionOperation::PftlUniswapPrimarySubscribe(operation) => Ok(&operation.subscriber),
        postfiat_types::AssetTransactionOperation::PftlUniswapRouteInitV2(operation) => Ok(&operation.operator),
        postfiat_types::AssetTransactionOperation::PftlUniswapOrderReserve(operation) => Ok(&operation.subscriber),
        postfiat_types::AssetTransactionOperation::PftlUniswapOrderRelease(operation) => Ok(&operation.releaser),
        postfiat_types::AssetTransactionOperation::PftlUniswapPrimarySubscribeV2(operation) => Ok(&operation.subscriber),
        postfiat_types::AssetTransactionOperation::PftlUniswapRedemptionFund(operation) => Ok(&operation.funder),
        postfiat_types::AssetTransactionOperation::PftlUniswapPrimaryRedeem(operation) => Ok(&operation.owner),
        postfiat_types::AssetTransactionOperation::PftlUniswapRouteEpochAdvance(operation) => Ok(&operation.operator),
        postfiat_types::AssetTransactionOperation::PftlUniswapRoutePause(operation) => Ok(&operation.operator),
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

fn certified_asset_ops_batch(options: CertifiedAssetOpsBatchOptions) -> Result<CertifiedAssetOpsBatchReport, String> {
    let total_start = std::time::Instant::now();
    let summary_file = options.artifact_dir.join("summary.json");
    if options.resume && summary_file.is_file() {
        let raw = std::fs::read_to_string(&summary_file).map_err(|error| {
            format!(
                "failed to read existing summary `{}`: {error}",
                summary_file.display()
            )
        })?;
        let report = serde_json::from_str::<CertifiedAssetOpsBatchReport>(&raw).map_err(|error| {
            format!(
                "existing summary `{}` is not a certified asset ops report: {error}",
                summary_file.display()
            )
        })?;
        return Ok(report);
    }
    prepare_artifact_dir(&options.artifact_dir, options.overwrite, options.resume)?;

    let preflight_start = std::time::Instant::now();
    if options.prepare_only && options.batch_only {
        return Err("--prepare-only and --batch-only cannot be used together".to_string());
    }
    let request = read_certified_asset_ops_request(&options.ops_file)?;
    validate_certified_asset_ops_request(&request)?;
    let dependency_report = certified_asset_ops_dependency_report(&request);
    let start_status = status(NodeOptions {
        data_dir: options.data_dir.clone(),
    })
    .map_err(|error| format!("certified asset ops preflight status failed: {error}"))?;
    if start_status.mempool_pending != 0 && !options.allow_existing_mempool {
        return Err(format!(
            "mempool has {} pending transactions; rerun with --allow-existing-mempool only after confirming they belong in this batch",
            start_status.mempool_pending
        ));
    }
    let preflight_ms = monotonic_elapsed_ms(preflight_start);

    write_json_file(
        &options.artifact_dir.join("request.normalized.json"),
        &request_to_json(&request)?,
    )?;

    let operations_start = std::time::Instant::now();
    let mut operation_reports = Vec::new();
    let mut next_sequences = std::collections::BTreeMap::<String, u64>::new();
    for op in &request.operations {
        let sequence_override = next_sequences.get(&op.source).copied();
        let report = run_certified_asset_op_stage(op, &options, true, sequence_override)?;
        if let Some(sequence) = report.sequence {
            let next_sequence = sequence.checked_add(1).ok_or_else(|| {
                format!(
                    "certified asset ops sequence overflow after `{}` from `{}`",
                    op.label, op.source
                )
            })?;
            next_sequences.insert(op.source.clone(), next_sequence);
        }
        operation_reports.push(report);
    }
    let operations_ms = monotonic_elapsed_ms(operations_start);

    let max_transactions = options
        .max_transactions
        .unwrap_or(request.operations.len());
    if max_transactions < request.operations.len() {
        return Err(format!(
            "--max-transactions {max_transactions} is smaller than operation count {}",
            request.operations.len()
        ));
    }

    let mut batch_file = None;
    let mut round_artifact_dir = None;
    let mut round_ok = None;
    let certify_start = std::time::Instant::now();
    if options.batch_only {
        let batch_path = options.artifact_dir.join("mempool-batch.json");
        create_mempool_batch(MempoolBatchOptions {
            data_dir: options.data_dir.clone(),
            batch_file: batch_path.clone(),
            max_transactions,
        })
        .map_err(|error| format!("certified asset ops batch create failed: {error}"))?;
        batch_file = Some(batch_path.display().to_string());
    } else if !options.prepare_only {
        let round_dir = options.artifact_dir.join("peer-certified-mempool-round");
        let round = transport_peer_certified_mempool_round(TransportPeerCertifiedMempoolRoundOptions {
            data_dir: options.data_dir.clone(),
            topology_file: options.topology_file.clone(),
            key_file: options.key_file.clone(),
            proposal_key_file: options.proposal_key_file.clone().or_else(|| Some(options.key_file.clone())),
            require_local_proposer: options.require_local_proposer,
            require_signed_proposal: options.require_signed_proposal,
            allow_peer_failures: options.allow_peer_failures,
            quorum_early_full_propagation: options.quorum_early_full_propagation,
            artifact_dir: round_dir.clone(),
            block_height: options.block_height,
            view: options.view,
            timeout_certificate_file: options.timeout_certificate_file.clone(),
            timeout_ms: options.timeout_ms,
            send_retries: options.send_retries,
            retry_backoff_ms: options.retry_backoff_ms,
            local_apply_before_certified_send: options.local_apply_before_certified_send,
            defer_certified_sends: options.defer_certified_sends,
            required_parent: None,
            max_transactions,
            signed_transfer_file: None,
            signed_transfer_json: None,
            signed_payment_v2_json: None,
            signed_asset_transaction_json: None,
            signed_atomic_swap_transaction_json: None,
            signed_escrow_transaction_json: None,
        })?;
        let round_report_file = options.artifact_dir.join("peer-certified-mempool-round.report.json");
        write_json_file(&round_report_file, &round)?;
        batch_file = Some(round.batch_file.clone());
        round_artifact_dir = Some(round.artifact_dir.clone());
        round_ok = Some(round.round_ok);
    }
    let certify_ms = monotonic_elapsed_ms(certify_start);

    let final_status_start = std::time::Instant::now();
    let end_status = if options.prepare_only {
        None
    } else {
        Some(
            status(NodeOptions {
                data_dir: options.data_dir.clone(),
            })
            .map_err(|error| format!("certified asset ops final status failed: {error}"))?,
        )
    };
    let final_status_ms = monotonic_elapsed_ms(final_status_start);

    let report = CertifiedAssetOpsBatchReport {
        schema: CERTIFIED_ASSET_OPS_REPORT_SCHEMA.to_string(),
        request_schema: request.schema,
        data_dir: options.data_dir.display().to_string(),
        topology_file: options.topology_file.display().to_string(),
        artifact_dir: options.artifact_dir.display().to_string(),
        operation_count: operation_reports.len(),
        max_transactions,
        allow_existing_mempool: options.allow_existing_mempool,
        prepare_only: options.prepare_only,
        batch_only: options.batch_only,
        start_height: start_status.block_height,
        start_state_root: start_status.state_root,
        start_mempool_pending: start_status.mempool_pending,
        end_height: end_status.as_ref().map(|status| status.block_height),
        end_state_root: end_status.as_ref().map(|status| status.state_root.clone()),
        end_mempool_pending: end_status.as_ref().map(|status| status.mempool_pending),
        operations: operation_reports,
        dependency_report,
        batch_file,
        round_artifact_dir,
        round_ok,
        timings_ms: CertifiedAssetOpsTimingsReport {
            total_ms: monotonic_elapsed_ms(total_start),
            preflight_ms,
            operations_ms,
            certify_ms,
            final_status_ms,
        },
    };
    write_json_file(&summary_file, &report)?;
    Ok(report)
}
