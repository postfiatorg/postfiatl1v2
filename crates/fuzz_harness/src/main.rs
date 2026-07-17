use std::collections::HashSet;
use std::env;
use std::error::Error;

use orchard::{
    builder::{Builder, BundleType},
    keys::{FullViewingKey, Scope, SpendingKey},
    value::NoteValue,
    Anchor,
};
use postfiat_bridge::{
    apply_simulated_transfer, bridge_witness_attestation_id, bridge_witness_attestation_message,
    upsert_domain, BridgeTransferRequest, BridgeWitnessChainDomain,
};
use postfiat_consensus_cobalt::{
    ratify_governance_amendment, verify_governance_amendment, CobaltDomain, EssentialSubsetConfig,
};
use postfiat_crypto_provider::{
    address_from_public_key, bytes_to_hex, hash_bytes, hex_to_bytes, ml_dsa_65_keygen,
    ml_dsa_65_sign, ml_dsa_65_sign_with_context, ml_dsa_65_sign_with_context_seed,
    ml_dsa_65_verify, MlDsa65KeyPair, BRIDGE_WITNESS_SIGNATURE_CONTEXT, ML_DSA_65_ALGORITHM,
};
use postfiat_execution::{
    apply_owned_transfer, apply_owned_unwrap, execute_transfer, genesis_hash, minimum_transfer_fee,
    transfer_tx_id, unwrap_from_owned, wrap_to_owned, OwnedTransferError, ACCOUNT_RESERVE,
    FEE_COLLECTOR_ADDRESS, MIN_TRANSFER_FEE, NATIVE_PFT_ESCROW_ASSET_ID, OWNED_NATIVE_ASSET,
    TRANSFER_ACCOUNT_CREATION_FEE,
};
use postfiat_mempool_dag::BatchReference;
use postfiat_network::{
    apply_fault_plan, frame_message, verify_message_payload, FaultPlan, FramedMessage,
    NetworkDomain,
};
use postfiat_node::{global_issued_asset_supply, native_pft_live_total};
use postfiat_ordering_fast::{next_reference, order_references};
use postfiat_privacy::{
    debug_nullifier, mint_debug_note, scan_owner, spend_debug_note, turnstile_summary,
};
use postfiat_privacy_orchard::{
    EncryptedShieldedOutput, OrchardAnchor, OrchardBindingSignature, OrchardCircuitId,
    OrchardFlags, OrchardNullifier, OrchardOutputCommitment, OrchardProofBytes,
    OrchardProofSystemId, OrchardRandomizedVerificationKey, OrchardShieldedAction,
    OrchardSpendAuthSignature, OrchardValueCommitment, ORCHARD_ANCHOR_BYTES,
    ORCHARD_COMMITMENT_BYTES, ORCHARD_NULLIFIER_BYTES, ORCHARD_REDPALLAS_SIGNATURE_BYTES,
};
use postfiat_proofs::{
    DebugProofSystem, ProofArtifact, ProofStatement, ProofSystem, PublicInput,
    DEBUG_SHIELDED_SPEND_CIRCUIT_ID,
};
use postfiat_types::{
    Account, AssetDefinition, AssetOrchardAssetBalance, AtomicSwapAuthorization, AtomicSwapLeg,
    BridgeState, BridgeWitnessAttestation, Escrow, FastAssetControlActionV1,
    FastAssetControlCommandV1, FastAssetIdV1, FastAssetRuleHashV1, FastHolderPermitIdV1,
    FastHolderPermitV1, FastLaneCheckpointV1, FastLaneControlActionV1, FastLaneDepositV1,
    FastLaneExitClaimV1, FastLaneExitIntentV1, FastObjectIdV1, FastObjectKeyV1,
    FastSwapAuthorizationV1, FastSwapCertificateV1, FastSwapChainDomainV1,
    FastSwapCommitteeDomainV1, FastSwapCommitteeRootV1, FastSwapDecisionV1,
    FastSwapEffectsDigestV1, FastSwapEffectsV1, FastSwapIntentV1, FastSwapOpaqueHashV1,
    FastSwapPartyV1, FastSwapPhaseV1, FastSwapPolicyHashV1, FastSwapPolicySnapshotV1,
    FastSwapQuoteRoundingV1, FastSwapReceiptV1, FastSwapStatusResponseV1, FastSwapVoteV1, Genesis,
    GovernanceAmendment, GovernanceState, LedgerState, MempoolEntry, MempoolState, Offer,
    OrchardPoolState, OwnedObject, PftlUniswapConsensusRouteState, ShieldedState,
    SignedAtomicSwapTransaction, SignedFastAssetControlCommandV1, SignedFastSwapIntentV1,
    SignedTransfer, TrustLine, UnsignedAtomicSwapTransaction, UnsignedTransfer, ADDRESS_NAMESPACE,
    BRIDGE_DIRECTION_INBOUND, BRIDGE_DIRECTION_OUTBOUND, DEFAULT_SHIELDED_ASSET_ID,
    FASTSWAP_INTENT_CONTEXT_V1, FASTSWAP_ML_DSA_65, GOVERNANCE_KIND_BRIDGE_WITNESS_EPOCH,
    GOVERNANCE_KIND_CRYPTO_POLICY, GOVERNANCE_KIND_VALIDATOR_SET, TRANSFER_TRANSACTION_KIND,
};
use rand::{rngs::StdRng, SeedableRng};
use serde::Serialize;

const DEFAULT_ITERATIONS: usize = 64;
const MAX_SIGNED_ATOMIC_SWAP_JSON_BYTES: usize = 64 * 1024;
const MAX_FASTSWAP_JSON_BYTES: usize = 64 * 1024;

#[derive(Debug, Serialize)]
struct FuzzSuiteReport {
    schema: &'static str,
    iterations: usize,
    targets: Vec<FuzzTargetReport>,
}

#[derive(Debug, Serialize)]
struct FuzzTargetReport {
    target: &'static str,
    iterations: usize,
    corpus_cases: usize,
    parsed_cases: usize,
    rejected_cases: usize,
    invariant_failures: usize,
}

impl FuzzTargetReport {
    fn new(target: &'static str, iterations: usize, corpus_cases: usize) -> Self {
        Self {
            target,
            iterations,
            corpus_cases,
            parsed_cases: 0,
            rejected_cases: 0,
            invariant_failures: 0,
        }
    }

    fn record_parse(&mut self, parsed: bool) {
        if parsed {
            self.parsed_cases += 1;
        } else {
            self.rejected_cases += 1;
        }
    }

    fn assert_invariant(&mut self, invariant: bool) {
        if !invariant {
            self.invariant_failures += 1;
        }
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let target = args.first().map(String::as_str).unwrap_or("all");
    let iterations = flag_value(&args, "--iterations")
        .map(str::parse::<usize>)
        .transpose()?
        .unwrap_or(DEFAULT_ITERATIONS);

    let targets = match target {
        "all" => vec![
            fuzz_transaction_codec(iterations)?,
            fuzz_atomic_swap_codec(iterations)?,
            fuzz_fastswap_codec(iterations)?,
            fuzz_fastswap_auxiliary_codecs(iterations)?,
            fuzz_ledger_transfer_invariants(iterations)?,
            fuzz_owned_object_asset_invariants(iterations)?,
            fuzz_native_supply_invariants(iterations)?,
            fuzz_issued_supply_invariants(iterations)?,
            fuzz_mempool_sequence_invariants(iterations)?,
            fuzz_ordering_reference_invariants(iterations)?,
            fuzz_network_frame(iterations)?,
            fuzz_network_faults(iterations)?,
            fuzz_bridge_attestation(iterations)?,
            fuzz_bridge_supply_invariants(iterations)?,
            fuzz_shielded_nullifier_invariants(iterations)?,
            fuzz_orchard_parser(iterations)?,
            fuzz_governance_amendment_invariants(iterations)?,
            fuzz_proof_adapter(iterations)?,
        ],
        "transaction-codec" => vec![fuzz_transaction_codec(iterations)?],
        "atomic-swap-codec" => vec![fuzz_atomic_swap_codec(iterations)?],
        "fastswap-codec" => vec![fuzz_fastswap_codec(iterations)?],
        "fastswap-aux-codecs" => vec![fuzz_fastswap_auxiliary_codecs(iterations)?],
        "ledger-transfer-invariants" => vec![fuzz_ledger_transfer_invariants(iterations)?],
        "owned-object-asset-invariants" => {
            vec![fuzz_owned_object_asset_invariants(iterations)?]
        }
        "native-supply-invariants" => vec![fuzz_native_supply_invariants(iterations)?],
        "issued-supply-invariants" => vec![fuzz_issued_supply_invariants(iterations)?],
        "mempool-sequence-invariants" => vec![fuzz_mempool_sequence_invariants(iterations)?],
        "ordering-reference-invariants" => vec![fuzz_ordering_reference_invariants(iterations)?],
        "network-frame" => vec![fuzz_network_frame(iterations)?],
        "network-faults" => vec![fuzz_network_faults(iterations)?],
        "bridge-attestation" => vec![fuzz_bridge_attestation(iterations)?],
        "bridge-supply-invariants" => vec![fuzz_bridge_supply_invariants(iterations)?],
        "shielded-nullifier-invariants" => {
            vec![fuzz_shielded_nullifier_invariants(iterations)?]
        }
        "orchard-parser" => vec![fuzz_orchard_parser(iterations)?],
        "governance-amendment-invariants" => {
            vec![fuzz_governance_amendment_invariants(iterations)?]
        }
        "proof-adapter" => vec![fuzz_proof_adapter(iterations)?],
        other => return Err(format!("unknown fuzz target `{other}`").into()),
    };

    let report = FuzzSuiteReport {
        schema: "postfiat-fuzz-report-v1",
        iterations,
        targets,
    };
    println!("{}", serde_json::to_string_pretty(&report)?);

    if report
        .targets
        .iter()
        .any(|target| target.invariant_failures > 0)
    {
        return Err("fuzz invariant failure".into());
    }

    Ok(())
}

fn fuzz_transaction_codec(iterations: usize) -> Result<FuzzTargetReport, Box<dyn Error>> {
    let key_pair = ml_dsa_65_keygen()?;
    let genesis = Genesis::new("postfiat-fuzz");
    let unsigned = UnsignedTransfer {
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        address_namespace: ADDRESS_NAMESPACE.to_string(),
        transaction_kind: TRANSFER_TRANSACTION_KIND.to_string(),
        signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        from: address_from_public_key(&key_pair.public_key),
        to: "pffuzz00000000000000000000000000000001".to_string(),
        amount: 7,
        fee: MIN_TRANSFER_FEE,
        sequence: 1,
    };
    let signature = ml_dsa_65_sign(&key_pair.private_key, &unsigned.signing_bytes())?;
    let valid = SignedTransfer {
        unsigned,
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        public_key_hex: bytes_to_hex(&key_pair.public_key),
        signature_hex: bytes_to_hex(&signature),
    };
    let seed = serde_json::to_vec(&valid)?;
    let mut report = FuzzTargetReport::new("transaction-codec", iterations, 1);

    for input in mutated_inputs(&seed, iterations) {
        match serde_json::from_slice::<SignedTransfer>(&input) {
            Ok(candidate) => {
                report.record_parse(true);
                let signature_valid = verify_signed_transfer_shape(&candidate);
                if candidate == valid {
                    report.assert_invariant(signature_valid);
                }
            }
            Err(_) => report.record_parse(false),
        }
    }

    Ok(report)
}

fn verify_signed_transfer_shape(transfer: &SignedTransfer) -> bool {
    if transfer.algorithm_id != ML_DSA_65_ALGORITHM {
        return false;
    }
    let Ok(public_key) = hex_to_bytes(&transfer.public_key_hex) else {
        return false;
    };
    let Ok(signature) = hex_to_bytes(&transfer.signature_hex) else {
        return false;
    };
    ml_dsa_65_verify(&public_key, &transfer.unsigned.signing_bytes(), &signature)
}

fn fuzz_atomic_swap_codec(iterations: usize) -> Result<FuzzTargetReport, Box<dyn Error>> {
    let valid = signed_fuzz_atomic_swap()?;
    let seed = serde_json::to_vec(&valid)?;
    let explicit_cases = 5;
    let mut report = FuzzTargetReport::new(
        "atomic-swap-codec",
        iterations,
        iterations.saturating_add(1).saturating_add(explicit_cases),
    );

    report.assert_invariant(seed.len() <= MAX_SIGNED_ATOMIC_SWAP_JSON_BYTES);
    report.assert_invariant(valid.validate().is_ok());

    let mut oversized_hex = valid.clone();
    oversized_hex.unsigned.leg_0.asset_id.push_str("00");
    let oversized_hex_bytes = serde_json::to_vec(&oversized_hex)?;
    report.assert_invariant(oversized_hex_bytes.len() <= MAX_SIGNED_ATOMIC_SWAP_JSON_BYTES);
    report.assert_invariant(
        serde_json::from_slice::<SignedAtomicSwapTransaction>(&oversized_hex_bytes).is_ok(),
    );
    let oversized_hex_result = decode_and_validate_atomic_swap(&oversized_hex_bytes);
    report.record_parse(oversized_hex_result.is_ok());
    report.assert_invariant(oversized_hex_result.is_err());

    let mut oversized_payload = valid.clone();
    oversized_payload.authorization_0.signature_hex =
        "aa".repeat(MAX_SIGNED_ATOMIC_SWAP_JSON_BYTES);
    let oversized_payload_bytes = serde_json::to_vec(&oversized_payload)?;
    report.assert_invariant(oversized_payload_bytes.len() > MAX_SIGNED_ATOMIC_SWAP_JSON_BYTES);
    let oversized_payload_result = decode_and_validate_atomic_swap(&oversized_payload_bytes);
    report.record_parse(oversized_payload_result.is_ok());
    report.assert_invariant(oversized_payload_result.is_err());

    for cut in [1, seed.len() / 2, seed.len().saturating_sub(1)] {
        let truncated_result = decode_and_validate_atomic_swap(&seed[..cut]);
        report.record_parse(truncated_result.is_ok());
        report.assert_invariant(truncated_result.is_err());
    }

    for input in mutated_inputs(&seed, iterations) {
        match decode_and_validate_atomic_swap(&input) {
            Ok(candidate) => {
                report.record_parse(true);
                report.assert_invariant(input.len() <= MAX_SIGNED_ATOMIC_SWAP_JSON_BYTES);
                report.assert_invariant(candidate.validate().is_ok());
                if candidate == valid {
                    report.assert_invariant(
                        candidate.tx_id_preimage_bytes() == valid.tx_id_preimage_bytes(),
                    );
                }
            }
            Err(_) => report.record_parse(false),
        }
    }

    Ok(report)
}

fn decode_and_validate_atomic_swap(input: &[u8]) -> Result<SignedAtomicSwapTransaction, String> {
    if input.len() > MAX_SIGNED_ATOMIC_SWAP_JSON_BYTES {
        return Err(format!(
            "atomic swap JSON exceeds {} bytes",
            MAX_SIGNED_ATOMIC_SWAP_JSON_BYTES
        ));
    }
    let transaction = serde_json::from_slice::<SignedAtomicSwapTransaction>(input)
        .map_err(|error| error.to_string())?;
    transaction.validate()?;
    Ok(transaction)
}

fn fuzz_fastswap_codec(iterations: usize) -> Result<FuzzTargetReport, Box<dyn Error>> {
    let valid = signed_fuzz_fastswap()?;
    let json_seed = serde_json::to_vec(&valid)?;
    let canonical_seed = valid
        .intent
        .canonical_bytes()
        .map_err(|error| format!("FastSwap seed encoding failed: {error:?}"))?;
    let swap_id = valid
        .swap_id()
        .map_err(|error| format!("FastSwap seed ID failed: {error:?}"))?;
    let vote = |validator_id: &str| FastSwapVoteV1 {
        domain: valid.intent.domain.clone(),
        swap_id,
        phase: FastSwapPhaseV1::Precommit,
        round: 0,
        decision: Some(FastSwapDecisionV1::Confirm),
        justification_digest: None,
        effects_digest: FastSwapEffectsDigestV1([9; 48]),
        receipt_digest: None,
        validator_id: validator_id.to_owned(),
        signature: vec![11; 64],
    };
    let certificate = FastSwapCertificateV1 {
        votes: vec![vote("validator-0"), vote("validator-1")],
    };
    let certificate_seed = serde_json::to_vec(&certificate)?;
    let mut report = FuzzTargetReport::new(
        "fastswap-codec",
        iterations.saturating_mul(3),
        iterations.saturating_mul(3).saturating_add(9),
    );

    report.assert_invariant(json_seed.len() <= MAX_FASTSWAP_JSON_BYTES);
    report.assert_invariant(
        FastSwapIntentV1::decode_canonical(&canonical_seed)
            .is_ok_and(|decoded| decoded == valid.intent),
    );

    for cut in [0, 1, canonical_seed.len() / 2, canonical_seed.len() - 1] {
        let decoded = FastSwapIntentV1::decode_canonical(&canonical_seed[..cut]);
        report.record_parse(decoded.is_ok());
        report.assert_invariant(decoded.is_err());
    }
    for input in mutated_inputs(&canonical_seed, iterations) {
        match FastSwapIntentV1::decode_canonical(&input) {
            Ok(decoded) => {
                report.record_parse(true);
                report
                    .assert_invariant(decoded.canonical_bytes().is_ok_and(|bytes| bytes == input));
                report.assert_invariant(decoded.intent_id().is_ok());
            }
            Err(_) => report.record_parse(false),
        }
    }

    for cut in [0, 1, json_seed.len() / 2, json_seed.len() - 1] {
        let decoded = decode_and_validate_fastswap(&json_seed[..cut]);
        report.record_parse(decoded.is_ok());
        report.assert_invariant(decoded.is_err());
    }
    for input in mutated_inputs(&json_seed, iterations) {
        match decode_and_validate_fastswap(&input) {
            Ok(decoded) => {
                report.record_parse(true);
                report.assert_invariant(input.len() <= MAX_FASTSWAP_JSON_BYTES);
                report.assert_invariant(decoded.intent.canonical_bytes().is_ok());
                report.assert_invariant(decoded.swap_id().is_ok());
            }
            Err(_) => report.record_parse(false),
        }
    }

    let duplicate_certificate = FastSwapCertificateV1 {
        votes: vec![vote("validator-0"), vote("validator-0")],
    };
    report.assert_invariant(duplicate_certificate.digest().is_err());
    for input in mutated_inputs(&certificate_seed, iterations) {
        match serde_json::from_slice::<FastSwapCertificateV1>(&input) {
            Ok(decoded) => {
                let valid = decoded.digest().is_ok();
                report.record_parse(valid);
                if valid {
                    report.assert_invariant(
                        decoded
                            .votes
                            .windows(2)
                            .all(|pair| pair[0].validator_id < pair[1].validator_id),
                    );
                    report.assert_invariant(
                        decoded
                            .votes
                            .iter()
                            .all(|vote| vote.signing_bytes().is_ok()),
                    );
                }
            }
            Err(_) => report.record_parse(false),
        }
    }
    Ok(report)
}

fn fuzz_fastswap_auxiliary_codecs(iterations: usize) -> Result<FuzzTargetReport, Box<dyn Error>> {
    let signed = signed_fuzz_fastswap()?;
    let swap_id = signed
        .swap_id()
        .map_err(|error| format!("FastSwap seed ID failed: {error:?}"))?;
    let object_key = FastObjectKeyV1 {
        object_id: FastObjectIdV1([31; 32]),
        version: 7,
    };
    let effects = FastSwapEffectsV1 {
        domain: signed.intent.domain.clone(),
        swap_id,
        policy_hash: signed.intent.policy_hash,
        decision: FastSwapDecisionV1::Confirm,
        consumed: vec![object_key],
        created: Vec::new(),
        fee_burns: Vec::new(),
        receipt: FastSwapReceiptV1 {
            swap_id,
            accepted: true,
            code: "fastswap_applied".to_owned(),
            consumed_count: 1,
            created_count: 0,
        },
    };
    let status = FastSwapStatusResponseV1 {
        schema: "postfiat-fastswap-status-v1".to_owned(),
        swap_id,
        record: None,
        terminal_tombstone: None,
    };
    let deposit = FastLaneDepositV1 {
        domain: signed.intent.domain.chain.clone(),
        source_address: signed.intent.party_0.owner_address.clone(),
        source_pubkey: signed.intent.party_0.owner_pubkey.clone(),
        sequence: 9,
        fee_pft: 1,
        destination_owner_pubkey: signed.intent.party_1.owner_pubkey.clone(),
        destination_holder_permit_id: None,
        asset_id: signed.intent.party_0.offered_asset_id,
        asset_rule_hash: signed.intent.party_0.offered_asset_rule_hash,
        amount_atoms: signed.intent.party_0.offered_amount,
        nonce: [32; 32],
    };
    let exit_claim = FastLaneExitClaimV1 {
        exit_claim_id: postfiat_types::FastSwapExitClaimIdV1([33; 48]),
        committee: signed.intent.domain.clone(),
        owner_pubkey: signed.intent.party_0.owner_pubkey.clone(),
        destination_address: "fuzz-primary-destination".to_owned(),
        asset_id: signed.intent.party_0.offered_asset_id,
        asset_rule_hash: signed.intent.party_0.offered_asset_rule_hash,
        amount_atoms: signed.intent.party_0.offered_amount,
    };
    let exit_intent = FastLaneExitIntentV1 {
        committee: signed.intent.domain.clone(),
        owner_address: signed.intent.party_0.owner_address.clone(),
        owner_pubkey: signed.intent.party_0.owner_pubkey.clone(),
        inputs: vec![object_key],
        asset_id: signed.intent.party_0.offered_asset_id,
        asset_rule_hash: signed.intent.party_0.offered_asset_rule_hash,
        amount_atoms: signed.intent.party_0.offered_amount,
        destination_address: "fuzz-primary-destination".to_owned(),
        nonce: [34; 32],
    };
    let checkpoint = FastLaneCheckpointV1 {
        previous_checkpoint_id: None,
        committee: signed.intent.domain.clone(),
        live_object_root: FastSwapOpaqueHashV1([35; 48]),
        live_object_totals: Vec::new(),
        exit_claim_root: FastSwapOpaqueHashV1([36; 48]),
        exit_claim_totals: Vec::new(),
        pending_fee_burn_totals: Vec::new(),
        terminal_root: FastSwapOpaqueHashV1([37; 48]),
        highest_wal_sequence: 0,
        active_policy_hashes: vec![signed.intent.policy_hash],
        imported_deposit_root: FastSwapOpaqueHashV1([38; 48]),
        redeemed_exit_claim_root: FastSwapOpaqueHashV1([39; 48]),
        drain_ready: false,
        fenced_policy_epochs: Vec::new(),
    };
    let asset_control = SignedFastAssetControlCommandV1 {
        command: FastAssetControlCommandV1 {
            domain: signed.intent.domain.clone(),
            action: FastAssetControlActionV1::Freeze,
            input: object_key,
            issuer_address: "pf-fuzz-issuer".to_owned(),
            issuer_control_pubkey: vec![41; 64],
            expires_at_height: signed.intent.expires_at_height,
            nonce: [42; 32],
        },
        algorithm_id: FASTSWAP_ML_DSA_65.to_owned(),
        signature: vec![43; 64],
    };
    let mut holder_permit = FastHolderPermitV1 {
        permit_id: FastHolderPermitIdV1::ZERO,
        asset_id: signed.intent.party_0.offered_asset_id,
        owner_pubkey: signed.intent.party_1.owner_pubkey.clone(),
        valid_from_height: 1,
        valid_through_height: signed.intent.expires_at_height,
        consensus_receipt_digest: FastSwapOpaqueHashV1([44; 48]),
    };
    holder_permit.permit_id = holder_permit
        .computed_id()
        .map_err(|error| format!("FastSwap holder permit seed failed: {error:?}"))?;
    let holder_control = FastLaneControlActionV1::RegisterHolderPermit {
        permit: holder_permit.clone(),
    };
    let mut policy = FastSwapPolicySnapshotV1 {
        domain: signed.intent.domain.chain.clone(),
        policy_epoch: 1,
        policy_hash: FastSwapPolicyHashV1::ZERO,
        pair_asset_0: signed.intent.party_0.offered_asset_id,
        pair_asset_1: signed.intent.party_1.offered_asset_id,
        asset_rule_hash_0: signed.intent.party_0.offered_asset_rule_hash,
        asset_rule_hash_1: signed.intent.party_1.offered_asset_rule_hash,
        price_numerator: 1,
        price_denominator: 8,
        rounding: FastSwapQuoteRoundingV1::Exact,
        nav_epoch: signed.intent.nav_epoch,
        market_envelope_hash: signed.intent.market_envelope_hash,
        valid_from_height: 1,
        valid_through_height: signed.intent.expires_at_height,
        fee_schedule_hash: FastSwapOpaqueHashV1([45; 48]),
        max_inputs_per_party: 8,
        max_outputs: 8,
        paused: false,
    };
    policy.policy_hash = policy
        .computed_hash()
        .map_err(|error| format!("FastSwap policy seed failed: {error:?}"))?;
    let mut report = FuzzTargetReport::new(
        "fastswap-aux-codecs",
        iterations.saturating_mul(10),
        iterations.saturating_add(1).saturating_mul(10),
    );

    macro_rules! exercise {
        ($value:expr, $ty:ty, $validate:expr) => {{
            let seed = serde_json::to_vec(&$value)?;
            for input in mutated_inputs(&seed, iterations) {
                match serde_json::from_slice::<$ty>(&input) {
                    Ok(decoded) => {
                        let valid = ($validate)(&decoded);
                        report.record_parse(valid);
                        report.assert_invariant(!valid || input.len() <= MAX_FASTSWAP_JSON_BYTES);
                    }
                    Err(_) => report.record_parse(false),
                }
            }
        }};
    }
    exercise!(effects, FastSwapEffectsV1, |value: &FastSwapEffectsV1| {
        value.digest().is_ok()
    });
    exercise!(
        status,
        FastSwapStatusResponseV1,
        |value: &FastSwapStatusResponseV1| { value.schema == "postfiat-fastswap-status-v1" }
    );
    exercise!(deposit, FastLaneDepositV1, |value: &FastLaneDepositV1| {
        value.signing_bytes().is_ok()
    });
    exercise!(
        exit_claim,
        FastLaneExitClaimV1,
        |value: &FastLaneExitClaimV1| value.canonical_bytes().is_ok()
    );
    exercise!(
        exit_intent,
        FastLaneExitIntentV1,
        |value: &FastLaneExitIntentV1| value.canonical_bytes().is_ok()
    );
    exercise!(
        checkpoint,
        FastLaneCheckpointV1,
        |value: &FastLaneCheckpointV1| value.canonical_bytes().is_ok()
    );
    exercise!(
        asset_control,
        SignedFastAssetControlCommandV1,
        |value: &SignedFastAssetControlCommandV1| {
            value.algorithm_id == FASTSWAP_ML_DSA_65
                && value.command.canonical_bytes().is_ok()
                && value.operation_id().is_ok()
                && value.signature.len() <= MAX_FASTSWAP_JSON_BYTES
        }
    );
    exercise!(
        holder_permit,
        FastHolderPermitV1,
        |value: &FastHolderPermitV1| value.computed_id().ok() == Some(value.permit_id)
    );
    exercise!(
        holder_control,
        FastLaneControlActionV1,
        |value: &FastLaneControlActionV1| value.canonical_bytes().is_ok()
    );
    exercise!(
        policy,
        FastSwapPolicySnapshotV1,
        |value: &FastSwapPolicySnapshotV1| { value.validate().is_ok() }
    );
    Ok(report)
}

fn decode_and_validate_fastswap(input: &[u8]) -> Result<SignedFastSwapIntentV1, String> {
    if input.len() > MAX_FASTSWAP_JSON_BYTES {
        return Err(format!(
            "FastSwap JSON exceeds {MAX_FASTSWAP_JSON_BYTES} bytes"
        ));
    }
    let signed = serde_json::from_slice::<SignedFastSwapIntentV1>(input)
        .map_err(|error| format!("{error:?}"))?;
    signed
        .intent
        .canonical_bytes()
        .map_err(|error| format!("{error:?}"))?;
    for (role, party, authorization) in [
        (0, &signed.intent.party_0, &signed.authorization_0),
        (1, &signed.intent.party_1, &signed.authorization_1),
    ] {
        if authorization.role != role
            || authorization.algorithm_id != FASTSWAP_ML_DSA_65
            || authorization.public_key != party.owner_pubkey
        {
            return Err(format!("malformed FastSwap authorization {role}"));
        }
    }
    signed.swap_id().map_err(|error| format!("{error:?}"))?;
    Ok(signed)
}

fn signed_fuzz_fastswap() -> Result<SignedFastSwapIntentV1, Box<dyn Error>> {
    let first_key = ml_dsa_65_keygen()?;
    let second_key = ml_dsa_65_keygen()?;
    let object_key = |byte| FastObjectKeyV1 {
        object_id: FastObjectIdV1([byte; 32]),
        version: 1,
    };
    let party = |key_pair: &MlDsa65KeyPair,
                 offered: u8,
                 received: u8,
                 input: u8,
                 fee: u8,
                 offered_amount: u64,
                 received_amount: u64| FastSwapPartyV1 {
        owner_address: address_from_public_key(&key_pair.public_key),
        owner_pubkey: key_pair.public_key.clone(),
        offered_asset_id: FastAssetIdV1([offered; 48]),
        offered_asset_rule_hash: FastAssetRuleHashV1([offered + 10; 48]),
        offered_amount,
        receives_asset_id: FastAssetIdV1([received; 48]),
        receives_asset_rule_hash: FastAssetRuleHashV1([received + 10; 48]),
        receives_holder_permit_id: None,
        receives_amount: received_amount,
        asset_inputs: vec![object_key(input)],
        fee_inputs: vec![object_key(fee)],
        asset_change: 2,
        fee_change: 9,
        fee_burn_pft: 1,
    };
    let intent = FastSwapIntentV1 {
        domain: FastSwapCommitteeDomainV1 {
            chain: FastSwapChainDomainV1 {
                chain_id: "postfiat-fuzz-fastswap".to_owned(),
                genesis_hash: FastSwapOpaqueHashV1([3; 48]),
                protocol_version: 1,
            },
            fastswap_schema_version: 1,
            committee_epoch: 7,
            committee_root: FastSwapCommitteeRootV1([4; 48]),
            validator_count: 6,
            quorum: 5,
        },
        policy_hash: postfiat_types::FastSwapPolicyHashV1([5; 48]),
        rfq_hash: postfiat_types::FastSwapRfqHashV1([6; 48]),
        market_envelope_hash: postfiat_types::FastSwapMarketEnvelopeHashV1([7; 48]),
        nav_epoch: 59,
        expires_at_height: 100,
        nonce: [8; 32],
        party_0: party(&first_key, 1, 2, 1, 21, 8, 1),
        party_1: party(&second_key, 2, 1, 2, 22, 1, 8),
    };
    let bytes = intent
        .canonical_bytes()
        .map_err(|error| format!("FastSwap seed encoding failed: {error:?}"))?;
    Ok(SignedFastSwapIntentV1 {
        intent,
        authorization_0: FastSwapAuthorizationV1 {
            role: 0,
            algorithm_id: FASTSWAP_ML_DSA_65.to_owned(),
            public_key: first_key.public_key,
            signature: ml_dsa_65_sign_with_context(
                &first_key.private_key,
                &bytes,
                FASTSWAP_INTENT_CONTEXT_V1,
            )?,
        },
        authorization_1: FastSwapAuthorizationV1 {
            role: 1,
            algorithm_id: FASTSWAP_ML_DSA_65.to_owned(),
            public_key: second_key.public_key,
            signature: ml_dsa_65_sign_with_context(
                &second_key.private_key,
                &bytes,
                FASTSWAP_INTENT_CONTEXT_V1,
            )?,
        },
    })
}

fn signed_fuzz_atomic_swap() -> Result<SignedAtomicSwapTransaction, Box<dyn Error>> {
    let owner_0_key = ml_dsa_65_keygen()?;
    let owner_1_key = ml_dsa_65_keygen()?;
    let issuer_0_key = ml_dsa_65_keygen()?;
    let issuer_1_key = ml_dsa_65_keygen()?;
    let owner_0 = address_from_public_key(&owner_0_key.public_key);
    let owner_1 = address_from_public_key(&owner_1_key.public_key);
    let unsigned = UnsignedAtomicSwapTransaction {
        chain_id: "postfiat-fuzz-atomic-swap".to_string(),
        genesis_hash: repeated_hex('0'),
        protocol_version: 1,
        address_namespace: ADDRESS_NAMESPACE.to_string(),
        signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        rfq_hash: repeated_hex('1'),
        market_envelope_hash: repeated_hex('0'),
        nav_epoch: 0,
        expires_at_height: 100,
        swap_nonce: repeated_hex('2'),
        leg_0: AtomicSwapLeg {
            owner: owner_0.clone(),
            recipient: owner_1.clone(),
            issuer: address_from_public_key(&issuer_0_key.public_key),
            asset_id: repeated_hex('a'),
            amount: 7,
            sequence: 1,
            fee: 1_000,
        },
        leg_1: AtomicSwapLeg {
            owner: owner_1.clone(),
            recipient: owner_0.clone(),
            issuer: address_from_public_key(&issuer_1_key.public_key),
            asset_id: repeated_hex('b'),
            amount: 11,
            sequence: 1,
            fee: 1_000,
        },
    };
    let signing_bytes = unsigned.signing_bytes();
    let signature_0 = ml_dsa_65_sign(&owner_0_key.private_key, &signing_bytes)?;
    let signature_1 = ml_dsa_65_sign(&owner_1_key.private_key, &signing_bytes)?;
    let transaction = SignedAtomicSwapTransaction {
        unsigned,
        authorization_0: AtomicSwapAuthorization {
            owner: owner_0,
            algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            public_key_hex: bytes_to_hex(&owner_0_key.public_key),
            signature_hex: bytes_to_hex(&signature_0),
        },
        authorization_1: AtomicSwapAuthorization {
            owner: owner_1,
            algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            public_key_hex: bytes_to_hex(&owner_1_key.public_key),
            signature_hex: bytes_to_hex(&signature_1),
        },
    };
    transaction.validate()?;
    Ok(transaction)
}

fn fuzz_ledger_transfer_invariants(iterations: usize) -> Result<FuzzTargetReport, Box<dyn Error>> {
    let genesis = Genesis::new("postfiat-fuzz-ledger");
    let key_pairs = (0..3)
        .map(|_| ml_dsa_65_keygen())
        .collect::<Result<Vec<_>, _>>()?;
    let accounts = key_pairs
        .iter()
        .map(|key_pair| {
            let public_key_hex = bytes_to_hex(&key_pair.public_key);
            Account::new(
                address_from_public_key(&key_pair.public_key),
                10_000,
                Some(public_key_hex),
            )
        })
        .collect::<Vec<_>>();
    let mut ledger = LedgerState::new(accounts);
    let mut expected_supply = ledger_total_balance(&ledger);
    let mut report = FuzzTargetReport::new("ledger-transfer-invariants", iterations, iterations);

    for i in 0..iterations {
        let key_pair = &key_pairs[i % key_pairs.len()];
        let from = address_from_public_key(&key_pair.public_key);
        let sender = ledger
            .account(&from)
            .ok_or("fuzz sender missing from ledger")?;
        let scenario = i % 9;
        let to = match scenario {
            5 => from.clone(),
            6 => FEE_COLLECTOR_ADDRESS.to_string(),
            _ => format!("pfrecipient{i:036}"),
        };
        let amount = match scenario {
            1 => 0,
            4 => sender.balance.saturating_add(1),
            _ => ACCOUNT_RESERVE + 1 + (i % 7) as u64,
        };
        let sequence = if scenario == 3 {
            sender.sequence.saturating_add(2)
        } else {
            sender.sequence.saturating_add(1)
        };
        let mut transfer = if scenario == 2 {
            signed_fuzz_transfer(&genesis, key_pair, to, amount, 0, sequence)?
        } else {
            signed_fuzz_transfer_with_minimum_fee(&genesis, key_pair, to, amount, sequence)?
        };
        if scenario == 7 {
            transfer.unsigned.genesis_hash = repeated_hex('e');
        } else if scenario == 8 {
            transfer.signature_hex.push_str("00");
        }

        let before = ledger.clone();
        let before_supply = ledger_total_balance(&before);
        let expected_tx_id = transfer_tx_id(&transfer);
        let receipt = execute_transfer(&genesis, &mut ledger, &transfer);
        report.assert_invariant(receipt.tx_id == expected_tx_id);

        if receipt.accepted {
            report.record_parse(true);
            report.assert_invariant(receipt.code == "accepted");
            assert_accepted_transfer_delta(&mut report, &before, &ledger, &transfer);
            expected_supply = expected_supply.saturating_sub(receipt.fee_burned as u128);
            report.assert_invariant(receipt.fee_charged == transfer.unsigned.fee);
            report.assert_invariant(receipt.fee_burned == transfer.unsigned.fee);
        } else {
            report.record_parse(false);
            report.assert_invariant(receipt.code != "accepted");
            report.assert_invariant(ledger == before);
        }

        let expected_supply_after = if receipt.accepted {
            before_supply.saturating_sub(receipt.fee_burned as u128)
        } else {
            before_supply
        };
        report.assert_invariant(ledger_total_balance(&ledger) == expected_supply_after);
        assert_ledger_transfer_invariants(&mut report, &ledger, expected_supply);
    }

    Ok(report)
}

fn signed_fuzz_transfer(
    genesis: &Genesis,
    key_pair: &MlDsa65KeyPair,
    to: String,
    amount: u64,
    fee: u64,
    sequence: u64,
) -> Result<SignedTransfer, Box<dyn Error>> {
    let public_key_hex = bytes_to_hex(&key_pair.public_key);
    let unsigned = UnsignedTransfer {
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(genesis),
        protocol_version: genesis.protocol_version,
        address_namespace: ADDRESS_NAMESPACE.to_string(),
        transaction_kind: TRANSFER_TRANSACTION_KIND.to_string(),
        signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        from: address_from_public_key(&key_pair.public_key),
        to,
        amount,
        fee,
        sequence,
    };
    let signature = ml_dsa_65_sign(&key_pair.private_key, &unsigned.signing_bytes())?;
    Ok(SignedTransfer {
        unsigned,
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        public_key_hex,
        signature_hex: bytes_to_hex(&signature),
    })
}

fn signed_fuzz_transfer_with_minimum_fee(
    genesis: &Genesis,
    key_pair: &MlDsa65KeyPair,
    to: String,
    amount: u64,
    sequence: u64,
) -> Result<SignedTransfer, Box<dyn Error>> {
    let mut fee = MIN_TRANSFER_FEE;
    for _ in 0..8 {
        let transfer = signed_fuzz_transfer(genesis, key_pair, to.clone(), amount, fee, sequence)?;
        let state_expansion_fee = if transfer.unsigned.to != transfer.unsigned.from {
            TRANSFER_ACCOUNT_CREATION_FEE
        } else {
            0
        };
        let minimum_fee = minimum_transfer_fee(&transfer).saturating_add(state_expansion_fee);
        if fee >= minimum_fee {
            return Ok(transfer);
        }
        fee = minimum_fee;
    }
    Err("minimum transfer fee did not converge".into())
}

fn assert_accepted_transfer_delta(
    report: &mut FuzzTargetReport,
    before: &LedgerState,
    after: &LedgerState,
    transfer: &SignedTransfer,
) {
    let Some(sender_before) = before.account(&transfer.unsigned.from) else {
        report.assert_invariant(false);
        return;
    };
    let Some(sender_after) = after.account(&transfer.unsigned.from) else {
        report.assert_invariant(false);
        return;
    };
    report.assert_invariant(sender_after.sequence == sender_before.sequence.saturating_add(1));

    let expected_sender_debit = if transfer.unsigned.to == transfer.unsigned.from {
        transfer.unsigned.fee
    } else {
        transfer
            .unsigned
            .amount
            .saturating_add(transfer.unsigned.fee)
    };
    report.assert_invariant(
        sender_before.balance.saturating_sub(sender_after.balance) == expected_sender_debit,
    );

    if transfer.unsigned.to != transfer.unsigned.from
        && transfer.unsigned.to != FEE_COLLECTOR_ADDRESS
    {
        let recipient_before = before
            .account(&transfer.unsigned.to)
            .map(|account| account.balance)
            .unwrap_or_default();
        let Some(recipient_after) = after.account(&transfer.unsigned.to) else {
            report.assert_invariant(false);
            return;
        };
        report.assert_invariant(
            recipient_after.balance.saturating_sub(recipient_before) == transfer.unsigned.amount,
        );
    }

    if transfer.unsigned.to == FEE_COLLECTOR_ADDRESS {
        let fee_collector_before = before
            .account(FEE_COLLECTOR_ADDRESS)
            .map(|account| account.balance)
            .unwrap_or_default();
        let fee_collector_after = after
            .account(FEE_COLLECTOR_ADDRESS)
            .map(|account| account.balance)
            .unwrap_or_default();
        report.assert_invariant(
            fee_collector_after.saturating_sub(fee_collector_before) == transfer.unsigned.amount,
        );
    }
}

fn assert_ledger_transfer_invariants(
    report: &mut FuzzTargetReport,
    ledger: &LedgerState,
    expected_supply: u128,
) {
    let addresses = ledger
        .accounts
        .iter()
        .map(|account| &account.address)
        .collect::<HashSet<_>>();
    report.assert_invariant(addresses.len() == ledger.accounts.len());
    report.assert_invariant(ledger_total_balance(ledger) == expected_supply);

    for account in &ledger.accounts {
        if let Some(public_key_hex) = &account.public_key_hex {
            let Ok(public_key) = hex_to_bytes(public_key_hex) else {
                report.assert_invariant(false);
                continue;
            };
            report.assert_invariant(address_from_public_key(&public_key) == account.address);
        }
    }
}

fn ledger_total_balance(ledger: &LedgerState) -> u128 {
    ledger
        .accounts
        .iter()
        .map(|account| account.balance as u128)
        .sum()
}

fn fuzz_owned_object_asset_invariants(
    iterations: usize,
) -> Result<FuzzTargetReport, Box<dyn Error>> {
    let cases_per_iteration = 11usize;
    let mut report = FuzzTargetReport::new(
        "owned-object-asset-invariants",
        iterations,
        iterations.saturating_mul(cases_per_iteration),
    );
    let domain = postfiat_types::OwnedCertificateDomain {
        schema: postfiat_types::OWNED_CERTIFICATE_DOMAIN_SCHEMA_V2.to_string(),
        chain_id: "postfiat-owned-fuzz".to_string(),
        genesis_hash: "ab".repeat(48),
        protocol_version: 1,
        registry_id: "cd".repeat(48),
    };

    for iteration in 0..iterations {
        let wrong_asset = format!("issued-fuzz-{iteration:016x}");
        let amount = (iteration as u64 % 999).saturating_add(1);

        let fresh_ledger = || {
            LedgerState::new(vec![Account {
                address: "source".to_string(),
                balance: 1_000,
                sequence: 0,
                public_key_hex: None,
            }])
        };

        let mut wrong_label_ledger = fresh_ledger();
        let before = wrong_label_ledger.clone();
        let wrong_label = wrap_to_owned(
            &mut wrong_label_ledger,
            "source",
            "owner".to_string(),
            amount,
            wrong_asset.clone(),
            format!("wrong-label-{iteration}"),
        );
        report.record_parse(wrong_label.is_ok());
        report.assert_invariant(matches!(
            wrong_label,
            Err(OwnedTransferError::UnsupportedAsset)
        ));
        report.assert_invariant(wrong_label_ledger == before);

        let mut zero_ledger = fresh_ledger();
        let before = zero_ledger.clone();
        let zero = wrap_to_owned(
            &mut zero_ledger,
            "source",
            "owner".to_string(),
            0,
            OWNED_NATIVE_ASSET.to_string(),
            format!("zero-{iteration}"),
        );
        report.record_parse(zero.is_ok());
        report.assert_invariant(matches!(zero, Err(OwnedTransferError::NotConserved)));
        report.assert_invariant(zero_ledger == before);

        let mut overflow_ledger = fresh_ledger();
        let before = overflow_ledger.clone();
        let overflow = wrap_to_owned(
            &mut overflow_ledger,
            "source",
            "owner".to_string(),
            u64::MAX,
            OWNED_NATIVE_ASSET.to_string(),
            format!("overflow-{iteration}"),
        );
        report.record_parse(overflow.is_ok());
        report.assert_invariant(matches!(overflow, Err(OwnedTransferError::NotConserved)));
        report.assert_invariant(overflow_ledger == before);

        let mut collision_ledger = fresh_ledger();
        collision_ledger
            .owned_objects
            .push(postfiat_types::OwnedObject {
                id: "same-id".to_string(),
                version: 1,
                owner_pubkey_hex: "existing-owner".to_string(),
                value: 17,
                asset: OWNED_NATIVE_ASSET.to_string(),
            });
        let before = collision_ledger.clone();
        let collision = wrap_to_owned(
            &mut collision_ledger,
            "source",
            "owner".to_string(),
            amount,
            OWNED_NATIVE_ASSET.to_string(),
            "same-id".to_string(),
        );
        report.record_parse(collision.is_ok());
        report.assert_invariant(matches!(
            collision,
            Err(OwnedTransferError::DuplicateOutput)
        ));
        report.assert_invariant(collision_ledger == before);

        let mut issued_unwrap_ledger = LedgerState::empty();
        issued_unwrap_ledger
            .owned_objects
            .push(postfiat_types::OwnedObject {
                id: "issued".to_string(),
                version: 1,
                owner_pubkey_hex: "owner".to_string(),
                value: amount,
                asset: wrong_asset.clone(),
            });
        let before = issued_unwrap_ledger.clone();
        let issued_unwrap =
            unwrap_from_owned(&mut issued_unwrap_ledger, "issued", "owner", "recipient");
        report.record_parse(issued_unwrap.is_ok());
        report.assert_invariant(matches!(
            issued_unwrap,
            Err(OwnedTransferError::UnsupportedAsset)
        ));
        report.assert_invariant(issued_unwrap_ledger == before);

        let mut credit_overflow_ledger = LedgerState::new(vec![Account {
            address: "recipient".to_string(),
            balance: u64::MAX,
            sequence: 0,
            public_key_hex: None,
        }]);
        credit_overflow_ledger
            .owned_objects
            .push(postfiat_types::OwnedObject {
                id: "native".to_string(),
                version: 1,
                owner_pubkey_hex: "owner".to_string(),
                value: 1,
                asset: OWNED_NATIVE_ASSET.to_string(),
            });
        let before = credit_overflow_ledger.clone();
        let credit_overflow =
            unwrap_from_owned(&mut credit_overflow_ledger, "native", "owner", "recipient");
        report.record_parse(credit_overflow.is_ok());
        report.assert_invariant(matches!(credit_overflow, Err(OwnedTransferError::Overflow)));
        report.assert_invariant(credit_overflow_ledger == before);

        let mut valid_wrap_ledger = fresh_ledger();
        let native_total_before = owned_native_total(&valid_wrap_ledger);
        let object_id = format!("valid-{iteration}");
        let valid_wrap = wrap_to_owned(
            &mut valid_wrap_ledger,
            "source",
            "owner".to_string(),
            amount,
            OWNED_NATIVE_ASSET.to_string(),
            object_id.clone(),
        );
        report.record_parse(valid_wrap.is_ok());
        report.assert_invariant(valid_wrap.is_ok());
        report.assert_invariant(owned_native_total(&valid_wrap_ledger) == native_total_before);
        let before_replay = valid_wrap_ledger.clone();
        let replay = wrap_to_owned(
            &mut valid_wrap_ledger,
            "source",
            "owner".to_string(),
            amount,
            OWNED_NATIVE_ASSET.to_string(),
            object_id.clone(),
        );
        report.record_parse(replay.is_ok());
        report.assert_invariant(matches!(replay, Err(OwnedTransferError::DuplicateOutput)));
        report.assert_invariant(valid_wrap_ledger == before_replay);

        let transfer_order = postfiat_types::OwnedTransferOrder {
            domain: domain.clone(),
            inputs: vec![postfiat_types::OwnedObjectRef {
                id: object_id,
                version: 1,
            }],
            outputs: vec![postfiat_types::OwnedOutputSpec {
                owner_pubkey_hex: "recipient-owner".to_string(),
                value: amount,
                asset: OWNED_NATIVE_ASSET.to_string(),
            }],
            fee: 0,
            nonce: iteration as u64,
            memos: Vec::new(),
        };
        let before_transfer_total = owned_native_total(&valid_wrap_ledger);
        let transfer = apply_owned_transfer(&mut valid_wrap_ledger, &transfer_order, "owner");
        report.record_parse(transfer.is_ok());
        report.assert_invariant(transfer.is_ok());
        report.assert_invariant(owned_native_total(&valid_wrap_ledger) == before_transfer_total);
        let before_transfer_replay = valid_wrap_ledger.clone();
        let transfer_replay =
            apply_owned_transfer(&mut valid_wrap_ledger, &transfer_order, "owner");
        report.record_parse(transfer_replay.is_ok());
        report.assert_invariant(matches!(
            transfer_replay,
            Err(OwnedTransferError::UnknownInput)
        ));
        report.assert_invariant(valid_wrap_ledger == before_transfer_replay);

        let mut certified_unwrap_ledger = LedgerState::empty();
        certified_unwrap_ledger
            .owned_objects
            .push(postfiat_types::OwnedObject {
                id: "issued-certified".to_string(),
                version: 1,
                owner_pubkey_hex: "owner".to_string(),
                value: amount,
                asset: wrong_asset.clone(),
            });
        let issued_order = postfiat_types::OwnedUnwrapOrder {
            domain: domain.clone(),
            inputs: vec![postfiat_types::OwnedObjectRef {
                id: "issued-certified".to_string(),
                version: 1,
            }],
            to_address: "recipient".to_string(),
            amount,
            asset: wrong_asset,
            fee: 0,
            nonce: iteration as u64,
            memos: Vec::new(),
        };
        let before = certified_unwrap_ledger.clone();
        let issued_apply = apply_owned_unwrap(&mut certified_unwrap_ledger, &issued_order, "owner");
        report.record_parse(issued_apply.is_ok());
        report.assert_invariant(matches!(
            issued_apply,
            Err(OwnedTransferError::UnsupportedAsset)
        ));
        report.assert_invariant(certified_unwrap_ledger == before);
    }

    Ok(report)
}

fn owned_native_total(ledger: &LedgerState) -> u128 {
    ledger
        .accounts
        .iter()
        .map(|account| account.balance as u128)
        .chain(
            ledger
                .owned_objects
                .iter()
                .filter(|object| object.asset == OWNED_NATIVE_ASSET)
                .map(|object| object.value as u128),
        )
        .sum()
}

fn fuzz_native_supply_invariants(iterations: usize) -> Result<FuzzTargetReport, Box<dyn Error>> {
    let cases_per_iteration = 9usize;
    let mut report = FuzzTargetReport::new(
        "native-supply-invariants",
        iterations,
        iterations.saturating_mul(cases_per_iteration),
    );

    for iteration in 0..iterations {
        let seed = u64::try_from(iteration).unwrap_or(u64::MAX);
        let account_value = seed % 10_000;
        let escrow_value = seed.saturating_mul(3).saturating_add(1);
        let offer_value = seed.saturating_mul(5).saturating_add(1);
        let owned_value = seed.saturating_mul(7).saturating_add(1);
        let reserve_value = u128::from(seed).saturating_mul(11).saturating_add(1);
        let orchard_value = seed.saturating_mul(13).saturating_add(1);

        let mut ledger = LedgerState::new(vec![Account::new(
            format!("native-account-{iteration}"),
            account_value,
            None,
        )]);
        ledger.escrows.push(Escrow::new(
            "postfiat-native-supply-fuzz",
            format!("escrow-owner-{iteration}"),
            1,
            format!("escrow-recipient-{iteration}"),
            NATIVE_PFT_ESCROW_ASSET_ID,
            escrow_value,
            1,
            "condition",
            0,
            0,
            1,
        )?);
        ledger.offers.push(Offer::new(
            "postfiat-native-supply-fuzz",
            format!("offer-owner-{iteration}"),
            1,
            NATIVE_PFT_ESCROW_ASSET_ID,
            offer_value,
            "ab".repeat(48),
            1,
            1,
            0,
        )?);
        ledger.owned_objects.push(OwnedObject {
            id: format!("native-owned-{iteration}"),
            version: 1,
            owner_pubkey_hex: "owner-key".to_string(),
            value: owned_value,
            asset: OWNED_NATIVE_ASSET.to_string(),
        });
        ledger
            .fast_lane_reserves
            .push(postfiat_types::FastLaneReserveBalanceV1 {
                asset_id: FastAssetIdV1::native_pft(),
                amount_atoms: reserve_value,
            });
        let mut pool = OrchardPoolState::empty(format!("native-pool-{iteration}"));
        pool.turnstile_deposit_total = orchard_value;
        let mut shielded = ShieldedState::empty();
        shielded.orchard = Some(pool);
        let expected = u128::from(account_value)
            .checked_add(u128::from(escrow_value))
            .and_then(|value| value.checked_add(u128::from(offer_value)))
            .and_then(|value| value.checked_add(u128::from(ledger.offers[0].reserve_paid)))
            .and_then(|value| value.checked_add(u128::from(owned_value)))
            .and_then(|value| value.checked_add(reserve_value))
            .and_then(|value| value.checked_add(u128::from(orchard_value)))
            .ok_or("native fuzz expected total overflow")?;
        let valid = native_pft_live_total(&ledger, &shielded);
        report.record_parse(valid.is_ok());
        report.assert_invariant(valid.as_ref().is_ok_and(|value| *value == expected));

        for duplicate_lane in 0..5 {
            let mut duplicate = ledger.clone();
            match duplicate_lane {
                0 => duplicate.accounts.push(duplicate.accounts[0].clone()),
                1 => duplicate.escrows.push(duplicate.escrows[0].clone()),
                2 => duplicate.offers.push(duplicate.offers[0].clone()),
                3 => duplicate
                    .owned_objects
                    .push(duplicate.owned_objects[0].clone()),
                4 => duplicate
                    .fast_lane_reserves
                    .push(duplicate.fast_lane_reserves[0].clone()),
                _ => unreachable!(),
            }
            let duplicate_result = native_pft_live_total(&duplicate, &shielded);
            report.record_parse(duplicate_result.is_ok());
            report.assert_invariant(
                duplicate_result
                    .as_ref()
                    .is_err_and(|error| error.to_string().contains("duplicate native custody")),
            );
        }

        let mut overflow = LedgerState::new(vec![Account::new("overflow", 1, None)]);
        overflow
            .fast_lane_reserves
            .push(postfiat_types::FastLaneReserveBalanceV1 {
                asset_id: FastAssetIdV1::native_pft(),
                amount_atoms: u128::MAX,
            });
        let overflow_result = native_pft_live_total(&overflow, &ShieldedState::empty());
        report.record_parse(overflow_result.is_ok());
        report.assert_invariant(overflow_result.is_err());

        let mut issued = LedgerState::new(vec![Account::new("issued-exclusion", 1, None)]);
        issued.owned_objects.push(OwnedObject {
            id: format!("issued-owned-{iteration}"),
            version: 1,
            owner_pubkey_hex: "owner-key".to_string(),
            value: u64::MAX,
            asset: format!("issued-{iteration}"),
        });
        issued
            .fast_lane_reserves
            .push(postfiat_types::FastLaneReserveBalanceV1 {
                asset_id: FastAssetIdV1([0xA5; 48]),
                amount_atoms: u128::MAX,
            });
        let issued_result = native_pft_live_total(&issued, &ShieldedState::empty());
        report.record_parse(issued_result.is_ok());
        report.assert_invariant(issued_result.as_ref().is_ok_and(|value| *value == 1));

        let mut impossible_pool = OrchardPoolState::empty("impossible-native-pool");
        impossible_pool.fee_burn_total = 1;
        let mut impossible_shielded = ShieldedState::empty();
        impossible_shielded.orchard = Some(impossible_pool);
        let impossible_result = native_pft_live_total(&LedgerState::empty(), &impossible_shielded);
        report.record_parse(impossible_result.is_ok());
        report.assert_invariant(impossible_result.is_err());
    }

    Ok(report)
}

fn fuzz_issued_supply_invariants(iterations: usize) -> Result<FuzzTargetReport, Box<dyn Error>> {
    const CASES_PER_ITERATION: usize = 17;
    let mut report = FuzzTargetReport::new(
        "issued-supply-invariants",
        iterations,
        iterations.saturating_mul(CASES_PER_ITERATION),
    );

    for iteration in 0..iterations {
        let (ledger, shielded, asset_id, expected) = issued_supply_fixture(iteration)?;
        let valid = global_issued_asset_supply(&ledger, &shielded, &asset_id);
        report.record_parse(valid.is_ok());
        report.assert_invariant(valid.as_ref().is_ok_and(|value| *value == expected));

        for duplicate_lane in 0..7 {
            let mut duplicate_ledger = ledger.clone();
            let mut duplicate_shielded = shielded.clone();
            match duplicate_lane {
                0 => duplicate_ledger
                    .asset_definitions
                    .push(duplicate_ledger.asset_definitions[0].clone()),
                1 => duplicate_ledger
                    .trustlines
                    .push(duplicate_ledger.trustlines[0].clone()),
                2 => duplicate_ledger
                    .escrows
                    .push(duplicate_ledger.escrows[0].clone()),
                3 => duplicate_ledger
                    .offers
                    .push(duplicate_ledger.offers[0].clone()),
                4 => duplicate_ledger
                    .fast_lane_reserves
                    .push(duplicate_ledger.fast_lane_reserves[0].clone()),
                5 => duplicate_ledger
                    .pftl_uniswap_routes
                    .push(duplicate_ledger.pftl_uniswap_routes[0].clone()),
                6 => {
                    let pool = duplicate_shielded
                        .orchard
                        .as_mut()
                        .expect("issued fuzz Orchard fixture");
                    pool.asset_orchard_balances
                        .push(pool.asset_orchard_balances[0].clone());
                }
                _ => unreachable!(),
            }
            let result =
                global_issued_asset_supply(&duplicate_ledger, &duplicate_shielded, &asset_id);
            report.record_parse(result.is_ok());
            report.assert_invariant(result.is_err());
        }

        for unknown_lane in 0..6 {
            let unknown_asset = "ef".repeat(48);
            let mut unknown_ledger = ledger.clone();
            let mut unknown_shielded = shielded.clone();
            match unknown_lane {
                0 => unknown_ledger.trustlines[0].asset_id = unknown_asset,
                1 => unknown_ledger.escrows[0].asset_id = unknown_asset,
                2 => unknown_ledger.offers[0].taker_gets_asset_id = unknown_asset,
                3 => {
                    unknown_ledger.fast_lane_reserves[0].asset_id = FastAssetIdV1([0xef; 48]);
                }
                4 => unknown_ledger.pftl_uniswap_routes[0].native_nav_asset_id = unknown_asset,
                5 => {
                    unknown_shielded
                        .orchard
                        .as_mut()
                        .expect("issued fuzz Orchard fixture")
                        .asset_orchard_balances[0]
                        .asset_id = unknown_asset;
                }
                _ => unreachable!(),
            }
            let result = global_issued_asset_supply(&unknown_ledger, &unknown_shielded, &asset_id);
            report.record_parse(result.is_ok());
            report.assert_invariant(result.is_err());
        }

        let mut unsupported_owned = ledger.clone();
        unsupported_owned.owned_objects.push(OwnedObject {
            id: format!("issued-owned-{iteration}"),
            version: 1,
            owner_pubkey_hex: "owner-key".to_string(),
            value: 1,
            asset: asset_id.clone(),
        });
        let result = global_issued_asset_supply(&unsupported_owned, &shielded, &asset_id);
        report.record_parse(result.is_ok());
        report.assert_invariant(result.is_err());

        let mut reserve_overflow = ledger.clone();
        reserve_overflow.fast_lane_reserves[0].amount_atoms = u128::MAX;
        let result = global_issued_asset_supply(&reserve_overflow, &shielded, &asset_id);
        report.record_parse(result.is_ok());
        report.assert_invariant(result.is_err());

        let mut aggregate_overflow = ledger.clone();
        aggregate_overflow.trustlines[0].balance = u64::MAX;
        let result = global_issued_asset_supply(&aggregate_overflow, &shielded, &asset_id);
        report.record_parse(result.is_ok());
        report.assert_invariant(result.is_err());
    }

    Ok(report)
}

fn issued_supply_fixture(
    iteration: usize,
) -> Result<(LedgerState, ShieldedState, String, u64), Box<dyn Error>> {
    let chain_id = "postfiat-issued-supply-fuzz";
    let issuer = format!("issued-fuzz-issuer-{iteration}");
    let holder = format!("issued-fuzz-holder-{iteration}");
    let asset = AssetDefinition::new(chain_id, &issuer, "FSUP", 1, 0)?;
    let asset_id = asset.asset_id.clone();
    let base = u64::try_from(iteration % 1_000)?.saturating_add(1);
    let trustline_value = base;
    let escrow_value = base.saturating_add(1);
    let offer_value = base.saturating_add(2);
    let fast_lane_value = base.saturating_add(3);
    let outstanding_value = base.saturating_add(4);
    let return_value = base.saturating_add(5);
    let ethereum_value = base.saturating_add(6);
    let venue_value = base.saturating_add(7);
    let orchard_value = base.saturating_add(8);

    let mut line = TrustLine::new(&holder, &issuer, &asset_id, u64::MAX, 0)?;
    line.balance = trustline_value;
    line.authorized = true;
    let mut ledger = LedgerState::empty();
    ledger.asset_definitions.push(asset);
    ledger.trustlines.push(line);
    ledger.escrows.push(Escrow::new(
        chain_id,
        &holder,
        1,
        format!("issued-fuzz-recipient-{iteration}"),
        &asset_id,
        escrow_value,
        1,
        "condition",
        0,
        0,
        1,
    )?);
    ledger.offers.push(Offer::new(
        chain_id,
        &holder,
        2,
        &asset_id,
        offer_value,
        NATIVE_PFT_ESCROW_ASSET_ID,
        1,
        1,
        0,
    )?);
    ledger
        .fast_lane_reserves
        .push(postfiat_types::FastLaneReserveBalanceV1 {
            asset_id: FastAssetIdV1(
                hex_to_bytes(&asset_id)?
                    .try_into()
                    .map_err(|_| "issued fuzz asset id width")?,
            ),
            amount_atoms: u128::from(fast_lane_value),
        });
    ledger
        .pftl_uniswap_routes
        .push(PftlUniswapConsensusRouteState {
            route_id: format!("issued-fuzz-route-{iteration}"),
            route_family: "primary_pftl_mint".to_string(),
            route_config_digest: "ab".repeat(48),
            route_trust_class: "BFT_CHECKPOINT".to_string(),
            native_nav_asset_id: asset_id.clone(),
            settlement_asset_id: "cd".repeat(48),
            handoff_controller: format!("0x{}", "11".repeat(20)),
            settlement_adapter: format!("0x{}", "22".repeat(20)),
            wrapped_navcoin_token: format!("0x{}", "33".repeat(20)),
            ethereum_chain_id: 1,
            route_supply_cap_atoms: u64::MAX,
            packet_notional_cap_atoms: u64::MAX,
            latest_finalized_nav_epoch: 1,
            return_finality_blocks: 1,
            ethereum_verification_policy: None,
            authorized_valid_supply_atoms: outstanding_value
                .checked_add(return_value)
                .and_then(|value| value.checked_add(ethereum_value))
                .and_then(|value| value.checked_add(venue_value))
                .ok_or("issued fuzz external authorized total overflow")?,
            pftl_spendable_supply_atoms: 0,
            native_spendable_balances_atoms: std::collections::BTreeMap::new(),
            ethereum_spendable_supply_atoms: ethereum_value,
            other_registered_venue_supply_atoms: venue_value,
            outstanding_bridge_claims_atoms: outstanding_value,
            pending_return_import_claims_atoms: return_value,
            settlement_reserve_atoms: 0,
            primary_subscription_nonces: std::collections::BTreeMap::new(),
            export_packets: std::collections::BTreeMap::new(),
            export_nonces: std::collections::BTreeMap::new(),
            return_imports: std::collections::BTreeMap::new(),
            paused: false,
        });

    let mut pool = OrchardPoolState::empty(format!("issued-fuzz-pool-{iteration}"));
    pool.asset_orchard_balances.push(AssetOrchardAssetBalance {
        asset_id: asset_id.clone(),
        ingress_total: orchard_value,
        egress_total: 0,
        live_total: orchard_value,
    });
    let mut shielded = ShieldedState::empty();
    shielded.orchard = Some(pool);

    let expected = trustline_value
        .checked_add(escrow_value)
        .and_then(|value| value.checked_add(offer_value))
        .and_then(|value| value.checked_add(fast_lane_value))
        .and_then(|value| value.checked_add(outstanding_value))
        .and_then(|value| value.checked_add(return_value))
        .and_then(|value| value.checked_add(ethereum_value))
        .and_then(|value| value.checked_add(venue_value))
        .and_then(|value| value.checked_add(orchard_value))
        .ok_or("issued fuzz expected total overflow")?;
    Ok((ledger, shielded, asset_id, expected))
}

fn fuzz_mempool_sequence_invariants(iterations: usize) -> Result<FuzzTargetReport, Box<dyn Error>> {
    let genesis = Genesis::new("postfiat-fuzz-mempool");
    let key_pairs = (0..3)
        .map(|_| ml_dsa_65_keygen())
        .collect::<Result<Vec<_>, _>>()?;
    let accounts = key_pairs
        .iter()
        .map(|key_pair| {
            let public_key_hex = bytes_to_hex(&key_pair.public_key);
            Account::new(
                address_from_public_key(&key_pair.public_key),
                10_000,
                Some(public_key_hex),
            )
        })
        .collect::<Vec<_>>();
    let ledger = LedgerState::new(accounts);
    let mut mempool = MempoolState::empty();
    let mut report = FuzzTargetReport::new("mempool-sequence-invariants", iterations, iterations);

    for i in 0..iterations {
        let sender_index = i % key_pairs.len();
        let key_pair = &key_pairs[sender_index];
        let sender = address_from_public_key(&key_pair.public_key);
        let ledger_sequence = ledger
            .account(&sender)
            .ok_or("mempool fuzz sender missing from ledger")?
            .sequence;
        let next_sequence = next_fuzz_mempool_sequence(&mempool, &sender, ledger_sequence)
            .ok_or("mempool fuzz sequence overflow")?;
        let scenario = i % 8;
        let mut transfer = match scenario {
            2 => {
                let duplicate_sequence = mempool
                    .pending
                    .iter()
                    .find(|entry| entry.transfer.unsigned.from == sender)
                    .map(|entry| entry.transfer.unsigned.sequence)
                    .unwrap_or(next_sequence);
                signed_fuzz_transfer_with_minimum_fee(
                    &genesis,
                    key_pair,
                    format!("pfmempooldup{i:032}"),
                    ACCOUNT_RESERVE + 3,
                    duplicate_sequence,
                )?
            }
            3 => {
                if let Some(entry) = mempool.pending.first() {
                    entry.transfer.clone()
                } else {
                    signed_fuzz_transfer_with_minimum_fee(
                        &genesis,
                        key_pair,
                        format!("pfmempoolreplay{i:029}"),
                        ACCOUNT_RESERVE + 2,
                        next_sequence,
                    )?
                }
            }
            4 => signed_fuzz_transfer_with_minimum_fee(
                &genesis,
                key_pair,
                format!("pfmempoolgap{i:032}"),
                ACCOUNT_RESERVE + 4,
                next_sequence.saturating_add(1),
            )?,
            5 => signed_fuzz_transfer_with_minimum_fee(
                &genesis,
                key_pair,
                format!("pfmempooloverdraw{i:027}"),
                20_000,
                next_sequence,
            )?,
            _ => signed_fuzz_transfer_with_minimum_fee(
                &genesis,
                key_pair,
                format!("pfmempool{i:035}"),
                ACCOUNT_RESERVE + 1 + (i % 5) as u64,
                next_sequence,
            )?,
        };
        if scenario == 6 {
            transfer.signature_hex.push_str("00");
        } else if scenario == 7 {
            transfer.unsigned.chain_id = "wrong-mempool-chain".to_string();
        }

        let before = mempool.clone();
        match admit_fuzz_mempool_transfer(&genesis, &ledger, &mut mempool, transfer) {
            Ok(entry) => {
                report.record_parse(true);
                report.assert_invariant(entry.tx_id == transfer_tx_id(&entry.transfer));
                report.assert_invariant(!before.pending.iter().any(|old| old.tx_id == entry.tx_id));
                report.assert_invariant(!before.pending.iter().any(|old| {
                    old.transfer.unsigned.from == entry.transfer.unsigned.from
                        && old.transfer.unsigned.sequence == entry.transfer.unsigned.sequence
                }));
                report.assert_invariant(mempool.pending.len() == before.pending.len() + 1);
            }
            Err(_) => {
                report.record_parse(false);
                report.assert_invariant(mempool == before);
            }
        }

        assert_mempool_sequence_invariants(&mut report, &genesis, &ledger, &mempool);
    }

    Ok(report)
}

fn next_fuzz_mempool_sequence(
    mempool: &MempoolState,
    sender: &str,
    ledger_sequence: u64,
) -> Option<u64> {
    let pending_sequence = mempool
        .pending
        .iter()
        .filter(|entry| entry.transfer.unsigned.from == sender)
        .map(|entry| entry.transfer.unsigned.sequence)
        .max()
        .unwrap_or(ledger_sequence);
    pending_sequence.checked_add(1)
}

fn admit_fuzz_mempool_transfer(
    genesis: &Genesis,
    ledger: &LedgerState,
    mempool: &mut MempoolState,
    transfer: SignedTransfer,
) -> Result<MempoolEntry, String> {
    let tx_id = transfer_tx_id(&transfer);
    let mut dry_run_ledger = ledger.clone();
    for pending in &mempool.pending {
        let receipt = execute_transfer(genesis, &mut dry_run_ledger, &pending.transfer);
        if !receipt.accepted {
            return Err(format!("stale pending {}: {}", pending.tx_id, receipt.code));
        }
    }
    let receipt = execute_transfer(genesis, &mut dry_run_ledger, &transfer);
    if !receipt.accepted {
        return Err(format!("candidate rejected: {}", receipt.code));
    }
    if mempool.pending.iter().any(|entry| entry.tx_id == tx_id) {
        return Err(format!("duplicate mempool tx id {tx_id}"));
    }
    if mempool.has_sender_sequence(&transfer.unsigned.from, transfer.unsigned.sequence) {
        return Err(format!(
            "duplicate sender sequence {}:{}",
            transfer.unsigned.from, transfer.unsigned.sequence
        ));
    }

    let entry = MempoolEntry::new(tx_id, transfer);
    mempool.pending.push(entry.clone());
    Ok(entry)
}

fn assert_mempool_sequence_invariants(
    report: &mut FuzzTargetReport,
    genesis: &Genesis,
    ledger: &LedgerState,
    mempool: &MempoolState,
) {
    let tx_ids = mempool
        .pending
        .iter()
        .map(|entry| &entry.tx_id)
        .collect::<HashSet<_>>();
    report.assert_invariant(tx_ids.len() == mempool.pending.len());

    let sender_sequences = mempool
        .pending
        .iter()
        .map(|entry| {
            (
                entry.transfer.unsigned.from.as_str(),
                entry.transfer.unsigned.sequence,
            )
        })
        .collect::<HashSet<_>>();
    report.assert_invariant(sender_sequences.len() == mempool.pending.len());

    let mut dry_run_ledger = ledger.clone();
    let mut total_amount = 0_u64;
    let mut total_fee = 0_u64;
    for entry in &mempool.pending {
        report.assert_invariant(entry.tx_id == transfer_tx_id(&entry.transfer));
        let receipt = execute_transfer(genesis, &mut dry_run_ledger, &entry.transfer);
        report.assert_invariant(receipt.accepted);
        total_amount = total_amount.saturating_add(entry.transfer.unsigned.amount);
        total_fee = total_fee.saturating_add(entry.transfer.unsigned.fee);
    }
    report.assert_invariant(
        ledger_total_balance(&dry_run_ledger).saturating_add(total_fee as u128)
            == ledger_total_balance(ledger),
    );
    report.assert_invariant(total_amount >= mempool.pending.len() as u64);
    report.assert_invariant(total_fee >= mempool.pending.len() as u64 * MIN_TRANSFER_FEE);

    let senders = mempool
        .pending
        .iter()
        .map(|entry| entry.transfer.unsigned.from.as_str())
        .collect::<HashSet<_>>();
    for sender in senders {
        let ledger_sequence = ledger
            .account(sender)
            .map(|account| account.sequence)
            .unwrap_or_default();
        let mut sequences = mempool
            .pending
            .iter()
            .filter(|entry| entry.transfer.unsigned.from == sender)
            .map(|entry| entry.transfer.unsigned.sequence)
            .collect::<Vec<_>>();
        sequences.sort_unstable();
        for (offset, sequence) in sequences.into_iter().enumerate() {
            report.assert_invariant(sequence == ledger_sequence + offset as u64 + 1);
        }
    }
}

fn fuzz_ordering_reference_invariants(
    iterations: usize,
) -> Result<FuzzTargetReport, Box<dyn Error>> {
    let mut report = FuzzTargetReport::new("ordering-reference-invariants", iterations, iterations);

    for i in 0..iterations {
        let references = fuzz_ordering_references(i);
        let mut reversed = references.clone();
        reversed.reverse();
        let mut duplicated = references.clone();
        duplicated.extend(references.iter().take((i % references.len()) + 1).cloned());

        let ordered = order_references(references.clone());
        let ordered_reversed = order_references(reversed);
        let ordered_duplicated = order_references(duplicated);
        report.record_parse(true);

        report.assert_invariant(ordered == ordered_reversed);
        report.assert_invariant(ordered == ordered_duplicated);
        report.assert_invariant(
            next_reference(references.clone())
                == ordered.first().map(|entry| entry.reference.clone()),
        );

        let unique_references = references
            .iter()
            .map(|reference| {
                (
                    reference.batch_id.as_str(),
                    reference.payload_hash.as_str(),
                    reference.transaction_count,
                )
            })
            .collect::<HashSet<_>>();
        report.assert_invariant(ordered.len() == unique_references.len());

        for (index, ordered_reference) in ordered.iter().enumerate() {
            report.assert_invariant(ordered_reference.height == index as u64 + 1);
            report.assert_invariant(unique_references.contains(&(
                ordered_reference.reference.batch_id.as_str(),
                ordered_reference.reference.payload_hash.as_str(),
                ordered_reference.reference.transaction_count,
            )));
            report.assert_invariant(ordered_reference.reference.transaction_count > 0);
            report.assert_invariant(is_lower_hex_96(&ordered_reference.reference.batch_id));
            report.assert_invariant(is_lower_hex_96(&ordered_reference.reference.payload_hash));
        }

        for window in ordered.windows(2) {
            let left = &window[0].reference;
            let right = &window[1].reference;
            report.assert_invariant(ordering_reference_key(left) <= ordering_reference_key(right));
        }

        let same_batch_payloads = ordered
            .iter()
            .filter(|entry| entry.reference.batch_id == references[0].batch_id.as_str())
            .map(|entry| entry.reference.payload_hash.as_str())
            .collect::<HashSet<_>>();
        report.assert_invariant(same_batch_payloads.len() >= 2);
    }

    Ok(report)
}

fn fuzz_ordering_references(iteration: usize) -> Vec<BatchReference> {
    let anchor_batch_id = fuzz_reference_hash("postfiat.fuzz.ordering.batch", iteration % 3);
    let mut references = vec![
        BatchReference {
            batch_id: anchor_batch_id.clone(),
            payload_hash: fuzz_reference_hash("postfiat.fuzz.ordering.payload", iteration),
            transaction_count: 1,
        },
        BatchReference {
            batch_id: anchor_batch_id,
            payload_hash: fuzz_reference_hash("postfiat.fuzz.ordering.payload", iteration + 10_000),
            transaction_count: 1 + (iteration % 3) as u64,
        },
    ];

    for offset in 0..(3 + iteration % 5) {
        let batch_slot = (iteration + offset * 7) % 6;
        let payload_slot = (iteration * 11 + offset * 5) % 9;
        references.push(BatchReference {
            batch_id: fuzz_reference_hash("postfiat.fuzz.ordering.batch", batch_slot),
            payload_hash: fuzz_reference_hash("postfiat.fuzz.ordering.payload", payload_slot),
            transaction_count: 1 + ((iteration + offset) % 7) as u64,
        });
        if offset % 2 == 0 {
            let duplicate = references.last().expect("reference just pushed").clone();
            references.push(duplicate);
        }
    }

    references
}

fn fuzz_reference_hash(domain: &str, value: usize) -> String {
    bytes_to_hex(&hash_bytes(domain, value.to_string().as_bytes()))
}

fn ordering_reference_key(reference: &BatchReference) -> (&str, &str, u64) {
    (
        reference.batch_id.as_str(),
        reference.payload_hash.as_str(),
        reference.transaction_count,
    )
}

fn fuzz_network_frame(iterations: usize) -> Result<FuzzTargetReport, Box<dyn Error>> {
    let domain = NetworkDomain {
        chain_id: "postfiat-fuzz".to_string(),
        genesis_hash: repeated_hex('f'),
        protocol_version: 1,
    };
    let payload = br#"{"batch":"fuzz"}"#;
    let valid = frame_message(
        &domain,
        "validator-0",
        Some("validator-1".to_string()),
        "batch_reference",
        payload,
    )?;
    let seed = serde_json::to_vec(&valid)?;
    let mut report = FuzzTargetReport::new("network-frame", iterations, 1);
    report.assert_invariant(verify_message_payload(&domain, &valid, payload));
    report.assert_invariant(!verify_message_payload(&domain, &valid, b"tampered"));

    for input in mutated_inputs(&seed, iterations) {
        match serde_json::from_slice::<FramedMessage>(&input) {
            Ok(message) => {
                report.record_parse(true);
                let payload_matches = verify_message_payload(&domain, &message, payload);
                if message == valid {
                    report.assert_invariant(payload_matches);
                }
            }
            Err(_) => report.record_parse(false),
        }
    }

    Ok(report)
}

fn fuzz_network_faults(iterations: usize) -> Result<FuzzTargetReport, Box<dyn Error>> {
    let domain = NetworkDomain {
        chain_id: "postfiat-fuzz".to_string(),
        genesis_hash: repeated_hex('f'),
        protocol_version: 1,
    };
    let messages: Vec<FramedMessage> = (0..4)
        .map(|index| {
            frame_message(
                &domain,
                format!("validator-{index}"),
                Some(format!("validator-{}", (index + 1) % 4)),
                "batch_reference",
                format!("payload-{index}").as_bytes(),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let valid = FaultPlan {
        drop_message_ids: vec![messages[1].message_id.clone()],
        duplicate_message_ids: vec![messages[2].message_id.clone()],
        delay_message_ids: vec![messages[3].message_id.clone()],
    };
    let seed = serde_json::to_vec(&valid)?;
    let mut report = FuzzTargetReport::new("network-faults", iterations, 1);

    let delivery = apply_fault_plan(&messages, &valid)?;
    report.assert_invariant(delivery.delivered.len() == 4);
    report.assert_invariant(
        delivery
            .delivered
            .iter()
            .filter(|message| message.message_id == messages[1].message_id)
            .count()
            == 0,
    );
    report.assert_invariant(
        delivery
            .delivered
            .iter()
            .filter(|message| message.message_id == messages[2].message_id)
            .count()
            == 2,
    );
    report.assert_invariant(
        delivery
            .delivered
            .last()
            .map(|message| message.message_id.as_str())
            == Some(messages[3].message_id.as_str()),
    );

    for input in mutated_inputs(&seed, iterations) {
        match serde_json::from_slice::<FaultPlan>(&input) {
            Ok(candidate) => {
                report.record_parse(true);
                match apply_fault_plan(&messages, &candidate) {
                    Ok(delivery) => {
                        let all_delivered_known = delivery.delivered.iter().all(|message| {
                            messages
                                .iter()
                                .any(|known| known.message_id == message.message_id)
                        });
                        report.assert_invariant(all_delivered_known);
                        if candidate == valid {
                            report.assert_invariant(
                                delivery.dropped_message_ids == valid.drop_message_ids,
                            );
                            report.assert_invariant(
                                delivery.duplicated_message_ids == valid.duplicate_message_ids,
                            );
                            report.assert_invariant(
                                delivery.delayed_message_ids == valid.delay_message_ids,
                            );
                        }
                    }
                    Err(_) => report.assert_invariant(candidate != valid),
                }
            }
            Err(_) => report.record_parse(false),
        }
    }

    Ok(report)
}

fn fuzz_bridge_attestation(iterations: usize) -> Result<FuzzTargetReport, Box<dyn Error>> {
    let replay_state = bridge_state();
    let valid = attested_bridge_request(
        &replay_state,
        BridgeTransferRequest {
            domain_id: "xrpl-fuzz".to_string(),
            direction: BRIDGE_DIRECTION_INBOUND.to_string(),
            from: "rFuzzSource".to_string(),
            to: "pfbridgefuzz000000000000000000000000".to_string(),
            asset_id: DEFAULT_SHIELDED_ASSET_ID.to_string(),
            amount: 10,
            witness_id: "witness-fuzz-0".to_string(),
            witness_epoch: 1,
            witness_attestation: None,
        },
    )?;
    let seed = serde_json::to_vec(&valid)?;
    let mut report = FuzzTargetReport::new("bridge-attestation", iterations, 1);

    let mut replay_state = replay_state;
    let accepted = apply_simulated_transfer(&mut replay_state, valid.clone())?;
    report.assert_invariant(accepted.sequence == 1);
    report.assert_invariant(apply_simulated_transfer(&mut replay_state, valid.clone()).is_err());

    for input in mutated_inputs(&seed, iterations) {
        match serde_json::from_slice::<BridgeTransferRequest>(&input) {
            Ok(request) => {
                report.record_parse(true);
                let mut state = bridge_state();
                let result = apply_simulated_transfer(&mut state, request);
                if let Ok(transfer) = result {
                    report.assert_invariant(transfer.amount > 0);
                    report.assert_invariant(state.replay_cache.len() == state.transfers.len());
                    report.assert_invariant(state.transfers.len() == 1);
                }
            }
            Err(_) => report.record_parse(false),
        }
    }

    Ok(report)
}

fn fuzz_bridge_supply_invariants(iterations: usize) -> Result<FuzzTargetReport, Box<dyn Error>> {
    let mut state = BridgeState::empty();
    upsert_domain(&mut state, "xrpl-supply", "XRPL Supply", 100, 80)?;
    let mut report = FuzzTargetReport::new("bridge-supply-invariants", iterations, iterations);
    let mut expected_inbound_used = 0_u64;
    let mut expected_outbound_used = 0_u64;
    let mut accepted_witnesses = Vec::<(String, u32)>::new();

    assert_bridge_supply_invariants(
        &mut report,
        &state,
        expected_inbound_used,
        expected_outbound_used,
    );

    for i in 0..iterations {
        let direction = if i % 2 == 0 {
            BRIDGE_DIRECTION_INBOUND
        } else {
            BRIDGE_DIRECTION_OUTBOUND
        };
        let amount = match i % 10 {
            0 => 0,
            1 | 2 => 1 + (i % 5) as u64,
            3 | 4 => 25,
            5 => 90,
            6 => 7,
            _ => 3,
        };
        let (witness_id, witness_epoch) = if i % 11 == 10 && !accepted_witnesses.is_empty() {
            accepted_witnesses[0].clone()
        } else {
            (format!("supply-witness-{i}"), 1 + (i % 3) as u32)
        };
        let request = BridgeTransferRequest {
            domain_id: "xrpl-supply".to_string(),
            direction: direction.to_string(),
            from: format!("source-{i}"),
            to: format!("target-{i}"),
            asset_id: DEFAULT_SHIELDED_ASSET_ID.to_string(),
            amount,
            witness_id: witness_id.clone(),
            witness_epoch,
            witness_attestation: None,
        };
        let request = if amount == 0 {
            request
        } else {
            attested_bridge_request(&state, request)?
        };

        let domain = state
            .domain("xrpl-supply")
            .ok_or("bridge supply domain missing")?;
        let next_inbound_used = if direction == BRIDGE_DIRECTION_INBOUND {
            domain.inbound_used.checked_add(amount)
        } else {
            Some(domain.inbound_used)
        };
        let next_outbound_used = if direction == BRIDGE_DIRECTION_OUTBOUND {
            domain.outbound_used.checked_add(amount)
        } else {
            Some(domain.outbound_used)
        };
        let replay_key = format!("{}:{}:{}", request.domain_id, witness_epoch, witness_id);
        let cap_exceeded = next_inbound_used
            .map(|used| used > domain.inbound_cap)
            .unwrap_or(true)
            || next_outbound_used
                .map(|used| used > domain.outbound_cap)
                .unwrap_or(true);
        let should_accept = amount > 0 && !state.has_witness(&replay_key) && !cap_exceeded;
        let before = state.clone();

        match apply_simulated_transfer(&mut state, request) {
            Ok(transfer) => {
                report.record_parse(true);
                report.assert_invariant(should_accept);
                report.assert_invariant(transfer.sequence == state.transfers.len() as u64);
                match direction {
                    BRIDGE_DIRECTION_INBOUND => {
                        expected_inbound_used = expected_inbound_used
                            .checked_add(amount)
                            .ok_or("expected inbound overflow")?;
                    }
                    BRIDGE_DIRECTION_OUTBOUND => {
                        expected_outbound_used = expected_outbound_used
                            .checked_add(amount)
                            .ok_or("expected outbound overflow")?;
                    }
                    _ => unreachable!("direction selected from constants"),
                }
                accepted_witnesses.push((witness_id, witness_epoch));
            }
            Err(_) => {
                report.record_parse(false);
                report.assert_invariant(!should_accept);
                report.assert_invariant(state == before);
            }
        }

        assert_bridge_supply_invariants(
            &mut report,
            &state,
            expected_inbound_used,
            expected_outbound_used,
        );
    }

    Ok(report)
}

fn assert_bridge_supply_invariants(
    report: &mut FuzzTargetReport,
    state: &BridgeState,
    expected_inbound_used: u64,
    expected_outbound_used: u64,
) {
    let Some(domain) = state.domain("xrpl-supply") else {
        report.assert_invariant(false);
        return;
    };
    report.assert_invariant(domain.inbound_used == expected_inbound_used);
    report.assert_invariant(domain.outbound_used == expected_outbound_used);
    report.assert_invariant(domain.inbound_used <= domain.inbound_cap);
    report.assert_invariant(domain.outbound_used <= domain.outbound_cap);
    report.assert_invariant(state.transfers.len() == state.replay_cache.len());

    let replay_keys = state.replay_cache.iter().collect::<HashSet<_>>();
    report.assert_invariant(replay_keys.len() == state.replay_cache.len());

    let inbound_sum = state
        .transfers
        .iter()
        .filter(|transfer| transfer.direction == BRIDGE_DIRECTION_INBOUND)
        .map(|transfer| transfer.amount)
        .sum::<u64>();
    let outbound_sum = state
        .transfers
        .iter()
        .filter(|transfer| transfer.direction == BRIDGE_DIRECTION_OUTBOUND)
        .map(|transfer| transfer.amount)
        .sum::<u64>();
    report.assert_invariant(inbound_sum == expected_inbound_used);
    report.assert_invariant(outbound_sum == expected_outbound_used);

    for (index, transfer) in state.transfers.iter().enumerate() {
        report.assert_invariant(transfer.sequence == (index as u64 + 1));
        report.assert_invariant(transfer.amount > 0);
        report.assert_invariant(
            transfer.direction == BRIDGE_DIRECTION_INBOUND
                || transfer.direction == BRIDGE_DIRECTION_OUTBOUND,
        );
    }
}

fn fuzz_shielded_nullifier_invariants(
    iterations: usize,
) -> Result<FuzzTargetReport, Box<dyn Error>> {
    let mut state = ShieldedState::empty();
    let mut report = FuzzTargetReport::new("shielded-nullifier-invariants", iterations, iterations);
    let mut minted_total = 0_u64;
    let mut candidate_note_ids = Vec::<String>::new();

    for i in 0..iterations {
        if i % 5 == 0 || candidate_note_ids.is_empty() {
            let value = 10 + (i % 7) as u64;
            let note = mint_debug_note(
                &mut state,
                format!("owner-{}", i % 3),
                DEFAULT_SHIELDED_ASSET_ID,
                value,
                format!("mint-{i}"),
            )?;
            minted_total = minted_total
                .checked_add(value)
                .ok_or("minted total overflow")?;
            candidate_note_ids.push(note.note_id);
            report.record_parse(true);
        } else {
            let note_id = candidate_note_ids[i % candidate_note_ids.len()].clone();
            let before = state.clone();
            let source_value = state.note(&note_id).map(|note| note.value).unwrap_or(0);
            let amount = match i % 7 {
                0 => 0,
                1 => source_value.saturating_add(1),
                _ => 1 + (i as u64 % source_value.max(1)),
            };
            let expected_nullifier = debug_nullifier(&note_id);
            let should_accept =
                amount > 0 && amount <= source_value && !before.is_nullified(&expected_nullifier);

            match spend_debug_note(
                &mut state,
                &note_id,
                format!("recipient-{}", i % 4),
                amount,
                format!("spend-{i}"),
            ) {
                Ok(spend) => {
                    report.record_parse(true);
                    report.assert_invariant(should_accept);
                    report.assert_invariant(spend.spent_note_id == note_id);
                    report.assert_invariant(spend.nullifier == expected_nullifier);
                    report.assert_invariant(state.is_nullified(&spend.nullifier));
                    report.assert_invariant(
                        spend.outputs.iter().map(|note| note.value).sum::<u64>() == source_value,
                    );
                    candidate_note_ids.extend(spend.outputs.into_iter().map(|note| note.note_id));
                }
                Err(_) => {
                    report.record_parse(false);
                    report.assert_invariant(!should_accept);
                    report.assert_invariant(state == before);
                }
            }
        }

        assert_shielded_nullifier_invariants(&mut report, &state, minted_total);
    }

    Ok(report)
}

fn assert_shielded_nullifier_invariants(
    report: &mut FuzzTargetReport,
    state: &ShieldedState,
    minted_total: u64,
) {
    let nullifiers = state.nullifiers.iter().collect::<HashSet<_>>();
    report.assert_invariant(nullifiers.len() == state.nullifiers.len());

    let mut live_value = 0_u64;
    for note in &state.notes {
        report.assert_invariant(note.value > 0);
        report.assert_invariant((note.position as usize) < state.notes.len());
        if let Some(position_note) = state.notes.get(note.position as usize) {
            report.assert_invariant(position_note.note_id == note.note_id);
        }

        let nullifier = debug_nullifier(&note.note_id);
        if !state.is_nullified(&nullifier) {
            live_value = live_value.saturating_add(note.value);
        }
    }
    report.assert_invariant(live_value == minted_total);

    for nullifier in &state.nullifiers {
        report.assert_invariant(
            state
                .notes
                .iter()
                .any(|note| debug_nullifier(&note.note_id) == *nullifier),
        );
    }

    let owners = state
        .notes
        .iter()
        .map(|note| note.owner.as_str())
        .collect::<HashSet<_>>();
    for owner in owners {
        for note in scan_owner(state, owner) {
            report.assert_invariant(note.owner == owner);
            report.assert_invariant(!state.is_nullified(&debug_nullifier(&note.note_id)));
        }
    }

    let turnstile = turnstile_summary(state);
    report.assert_invariant(turnstile.event_count == state.turnstile_events.len() as u64);
    report.assert_invariant(turnstile.bootstrap_deposit_total == minted_total);
    report.assert_invariant(turnstile.migration_total <= minted_total);
}

fn fuzz_orchard_parser(iterations: usize) -> Result<FuzzTargetReport, Box<dyn Error>> {
    let valid = valid_orchard_action()?;
    let seed = serde_json::to_vec(&valid)?;
    let valid_json = String::from_utf8(seed.clone())?;
    let mut report = FuzzTargetReport::new("orchard-parser", iterations, 4);

    report.assert_invariant(valid.validate().is_ok());
    report.assert_invariant(valid.flags.to_orchard().is_ok());
    report.assert_invariant(valid.anchor.to_orchard().is_ok());
    for nullifier in &valid.nullifiers {
        report.assert_invariant(nullifier.to_orchard().is_ok());
    }
    for key in &valid.randomized_verification_keys {
        report.assert_invariant(key.to_orchard().is_ok());
    }
    for commitment in &valid.value_commitments {
        report.assert_invariant(commitment.to_orchard().is_ok());
    }
    for commitment in &valid.output_commitments {
        report.assert_invariant(commitment.to_orchard().is_ok());
    }

    let invalid_anchor = valid_json.replacen(
        valid.anchor.as_hex(),
        &bytes_to_hex(&[0xff; ORCHARD_ANCHOR_BYTES]),
        1,
    );
    let invalid_nullifier = valid_json.replacen(
        valid.nullifiers[0].as_hex(),
        &bytes_to_hex(&[0xff; ORCHARD_NULLIFIER_BYTES]),
        1,
    );
    let invalid_commitment = valid_json.replacen(
        valid.output_commitments[0].as_hex(),
        &bytes_to_hex(&[0xff; ORCHARD_COMMITMENT_BYTES]),
        1,
    );

    for invalid in [invalid_anchor, invalid_nullifier, invalid_commitment] {
        report.record_parse(false);
        report.assert_invariant(serde_json::from_str::<OrchardShieldedAction>(&invalid).is_err());
    }

    for input in mutated_inputs(&seed, iterations) {
        match serde_json::from_slice::<OrchardShieldedAction>(&input) {
            Ok(action) => {
                report.record_parse(true);
                let valid_shape = action.validate().is_ok();
                if action == valid {
                    report.assert_invariant(valid_shape);
                }
                if valid_shape {
                    report.assert_invariant(action.flags.to_orchard().is_ok());
                    report.assert_invariant(action.anchor.to_orchard().is_ok());
                    report.assert_invariant(
                        action
                            .nullifiers
                            .iter()
                            .all(|nullifier| nullifier.to_orchard().is_ok()),
                    );
                    report.assert_invariant(
                        action
                            .randomized_verification_keys
                            .iter()
                            .all(|key| key.to_orchard().is_ok()),
                    );
                    report.assert_invariant(
                        action
                            .value_commitments
                            .iter()
                            .all(|commitment| commitment.to_orchard().is_ok()),
                    );
                    report.assert_invariant(
                        action
                            .output_commitments
                            .iter()
                            .all(|commitment| commitment.to_orchard().is_ok()),
                    );
                }
            }
            Err(_) => report.record_parse(false),
        }
    }

    Ok(report)
}

fn valid_orchard_action() -> Result<OrchardShieldedAction, Box<dyn Error>> {
    let spending_key = SpendingKey::from_bytes([7u8; 32]).unwrap();
    let recipient = FullViewingKey::from(&spending_key).address_at(0u32, Scope::External);
    let mut builder = Builder::new(BundleType::DEFAULT, Anchor::from_bytes([0u8; 32]).unwrap());
    builder.add_output(None, recipient, NoteValue::from_raw(10), [0u8; 512])?;
    let (bundle, _) = builder
        .build::<i64>(StdRng::from_seed([13u8; 32]))?
        .expect("bundle should be present");

    let mut nullifiers = Vec::new();
    let mut randomized_verification_keys = Vec::new();
    let mut value_commitments = Vec::new();
    let mut output_commitments = Vec::new();
    let mut encrypted_outputs = Vec::new();
    let mut spend_authorization_signatures = Vec::new();

    for action in bundle.actions() {
        let commitment = OrchardOutputCommitment::from_orchard(*action.cmx());
        let encrypted_note = action.encrypted_note();
        encrypted_outputs.push(EncryptedShieldedOutput::from_bytes(
            commitment.clone(),
            &encrypted_note.epk_bytes,
            &encrypted_note.enc_ciphertext,
            &encrypted_note.out_ciphertext,
            None,
        )?);
        nullifiers.push(OrchardNullifier::from_orchard(*action.nullifier()));
        randomized_verification_keys
            .push(OrchardRandomizedVerificationKey::from_orchard(action.rk()));
        value_commitments.push(OrchardValueCommitment::from_orchard(action.cv_net()));
        output_commitments.push(commitment);
        spend_authorization_signatures.push(OrchardSpendAuthSignature::parse_hex(bytes_to_hex(
            &[9u8; ORCHARD_REDPALLAS_SIGNATURE_BYTES],
        ))?);
    }

    Ok(OrchardShieldedAction {
        pool_id: "orchard-v1".to_string(),
        proof_system_id: OrchardProofSystemId::production_v2(),
        circuit_id: OrchardCircuitId::action_v2(),
        flags: OrchardFlags::from_orchard(*bundle.flags()),
        anchor: OrchardAnchor::from_orchard(*bundle.anchor()),
        nullifiers,
        randomized_verification_keys,
        value_commitments,
        output_commitments,
        encrypted_outputs,
        value_balance: *bundle.value_balance(),
        external_binding_hash: None,
        fee: 1,
        proof: OrchardProofBytes::from_bytes(&[8u8; 128])?,
        spend_authorization_signatures,
        binding_signature: OrchardBindingSignature::parse_hex(bytes_to_hex(
            &[10u8; ORCHARD_REDPALLAS_SIGNATURE_BYTES],
        ))?,
    })
}

fn fuzz_governance_amendment_invariants(
    iterations: usize,
) -> Result<FuzzTargetReport, Box<dyn Error>> {
    let domain = CobaltDomain {
        chain_id: "postfiat-fuzz".to_string(),
        genesis_hash: repeated_hex('d'),
        protocol_version: 1,
    };
    let validators = (0..4)
        .map(|index| format!("validator-{index}"))
        .collect::<Vec<_>>();
    let config = EssentialSubsetConfig::all_of(validators);
    let initial_active_validator_count = config.validators.len() as u32;
    let initial_state = GovernanceState::new(initial_active_validator_count);
    let initial_crypto_policy_version = initial_state.crypto_policy_version;
    let initial_bridge_witness_epoch = initial_state.bridge_witness_epoch;
    let mut state = initial_state;
    let mut report =
        FuzzTargetReport::new("governance-amendment-invariants", iterations, iterations);
    let governance_kinds = [
        GOVERNANCE_KIND_VALIDATOR_SET,
        GOVERNANCE_KIND_CRYPTO_POLICY,
        GOVERNANCE_KIND_BRIDGE_WITNESS_EPOCH,
    ];

    for i in 0..iterations {
        let before = state.clone();
        let scenario = i % 8;
        let kind = governance_kinds[i % governance_kinds.len()];
        let value = if scenario == 7 { 0 } else { 2 + (i % 9) as u32 };
        let support = governance_support_for_scenario(&config.validators, scenario);

        match ratify_governance_amendment(&domain, &config, kind, value, support) {
            Ok(mut amendment) => {
                if scenario == 3 && !state.amendments.is_empty() {
                    amendment = state.amendments[i % state.amendments.len()].clone();
                } else if scenario == 5 {
                    if let Some(vote) = amendment.votes.first_mut() {
                        vote.vote_id.push('0');
                    }
                } else if scenario == 6 {
                    amendment.genesis_hash = repeated_hex('e');
                }

                let verifies = verify_governance_amendment(&domain, &amendment).is_ok();
                let duplicate = state
                    .amendments
                    .iter()
                    .any(|existing| existing.amendment_id == amendment.amendment_id);
                let should_apply = verifies && !duplicate;

                match apply_fuzz_governance_amendment(&mut state, &domain, amendment) {
                    Ok(()) => {
                        report.record_parse(true);
                        report.assert_invariant(should_apply);
                        report.assert_invariant(state != before);
                    }
                    Err(_) => {
                        report.record_parse(false);
                        report.assert_invariant(!should_apply);
                        report.assert_invariant(state == before);
                    }
                }
            }
            Err(_) => {
                report.record_parse(false);
                report.assert_invariant(state == before);
            }
        }

        assert_governance_amendment_invariants(
            &mut report,
            &state,
            &domain,
            initial_active_validator_count,
            initial_crypto_policy_version,
            initial_bridge_witness_epoch,
        );
    }

    Ok(report)
}

fn governance_support_for_scenario(validators: &[String], scenario: usize) -> Vec<String> {
    match scenario {
        2 => {
            let mut support = validators.iter().rev().cloned().collect::<Vec<_>>();
            if let Some(first) = validators.first() {
                support.push(first.clone());
            }
            support.push("external-validator".to_string());
            support
        }
        4 => validators
            .first()
            .map(|validator| vec![validator.clone()])
            .unwrap_or_default(),
        _ => validators.to_vec(),
    }
}

fn apply_fuzz_governance_amendment(
    state: &mut GovernanceState,
    domain: &CobaltDomain,
    amendment: GovernanceAmendment,
) -> Result<(), String> {
    verify_governance_amendment(domain, &amendment)?;
    if state
        .amendments
        .iter()
        .any(|existing| existing.amendment_id == amendment.amendment_id)
    {
        return Err("duplicate governance amendment".to_string());
    }
    state.apply(amendment);
    Ok(())
}

fn assert_governance_amendment_invariants(
    report: &mut FuzzTargetReport,
    state: &GovernanceState,
    domain: &CobaltDomain,
    initial_active_validator_count: u32,
    initial_crypto_policy_version: u32,
    initial_bridge_witness_epoch: u32,
) {
    let amendment_ids = state
        .amendments
        .iter()
        .map(|amendment| &amendment.amendment_id)
        .collect::<HashSet<_>>();
    report.assert_invariant(amendment_ids.len() == state.amendments.len());

    let mut active_validator_count = initial_active_validator_count;
    let mut crypto_policy_version = initial_crypto_policy_version;
    let mut bridge_witness_epoch = initial_bridge_witness_epoch;
    for amendment in &state.amendments {
        report.assert_invariant(verify_governance_amendment(domain, amendment).is_ok());
        report.assert_invariant(is_lower_hex_96(&amendment.amendment_id));
        report.assert_invariant(is_lower_hex_96(&amendment.instance_id));
        report.assert_invariant(is_lower_hex_96(&amendment.proposal_id));
        report.assert_invariant(is_lower_hex_96(&amendment.certificate_id));
        report.assert_invariant(amendment.value > 0);
        report.assert_invariant(amendment.quorum > 0);
        report.assert_invariant(amendment.support.len() >= amendment.quorum);
        report.assert_invariant(amendment.votes.len() == amendment.support.len());

        let support = amendment.support.iter().collect::<HashSet<_>>();
        report.assert_invariant(support.len() == amendment.support.len());
        let vote_validators = amendment
            .votes
            .iter()
            .map(|vote| &vote.validator)
            .collect::<Vec<_>>();
        report.assert_invariant(vote_validators.iter().copied().eq(amendment.support.iter()));
        report.assert_invariant(
            amendment
                .votes
                .iter()
                .all(|vote| vote.accept && is_lower_hex_96(&vote.vote_id)),
        );

        match amendment.kind.as_str() {
            GOVERNANCE_KIND_VALIDATOR_SET => active_validator_count = amendment.value,
            GOVERNANCE_KIND_CRYPTO_POLICY => crypto_policy_version = amendment.value,
            GOVERNANCE_KIND_BRIDGE_WITNESS_EPOCH => bridge_witness_epoch = amendment.value,
            _ => report.assert_invariant(false),
        }
    }

    report.assert_invariant(state.active_validator_count == active_validator_count);
    report.assert_invariant(state.crypto_policy_version == crypto_policy_version);
    report.assert_invariant(state.bridge_witness_epoch == bridge_witness_epoch);
}

fn attested_bridge_request(
    state: &BridgeState,
    mut request: BridgeTransferRequest,
) -> Result<BridgeTransferRequest, Box<dyn Error>> {
    let domain = state
        .domain(&request.domain_id)
        .ok_or("bridge domain missing for attestation")?;
    let key_pair = ml_dsa_65_keygen()?;
    let signer = "validator-0";
    let public_key_hex = bytes_to_hex(&key_pair.public_key);
    let genesis = Genesis::new("postfiat-fuzz-bridge");
    let genesis_hash_hex = genesis_hash(&genesis);
    let chain_domain = BridgeWitnessChainDomain {
        chain_id: &genesis.chain_id,
        genesis_hash: &genesis_hash_hex,
        protocol_version: genesis.protocol_version,
    };
    let message = bridge_witness_attestation_message(
        chain_domain,
        domain,
        &request,
        signer,
        ML_DSA_65_ALGORITHM,
        &public_key_hex,
    )?;
    let signature_seed = bridge_witness_signature_seed(&message);
    let signature = ml_dsa_65_sign_with_context_seed(
        &key_pair.private_key,
        &message,
        BRIDGE_WITNESS_SIGNATURE_CONTEXT,
        &signature_seed,
    )?;
    let attestation_id = bridge_witness_attestation_id(
        chain_domain,
        domain,
        &request,
        signer,
        ML_DSA_65_ALGORITHM,
        &public_key_hex,
    )?;
    request.witness_attestation = Some(BridgeWitnessAttestation {
        attestation_id,
        chain_id: genesis.chain_id,
        genesis_hash: genesis_hash_hex,
        protocol_version: genesis.protocol_version,
        signer: signer.to_string(),
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        public_key_hex,
        signature_hex: bytes_to_hex(&signature),
    });
    Ok(request)
}

fn bridge_witness_signature_seed(message: &[u8]) -> [u8; 32] {
    let digest = hash_bytes("postfiat.bridge_witness.signature_seed.v1", message);
    digest[..32].try_into().expect("seed length")
}

fn bridge_state() -> BridgeState {
    let mut state = BridgeState::empty();
    upsert_domain(&mut state, "xrpl-fuzz", "XRPL Fuzz", 1_000, 1_000).expect("fuzz bridge domain");
    state
}

fn fuzz_proof_adapter(iterations: usize) -> Result<FuzzTargetReport, Box<dyn Error>> {
    let system = DebugProofSystem::for_controlled_testnet_debug()?;
    let note_id = repeated_hex('a');
    let nullifier = repeated_hex('b');
    let spend_id = repeated_hex('c');
    let statement = ProofStatement::new(
        DEBUG_SHIELDED_SPEND_CIRCUIT_ID,
        vec![
            PublicInput::new("note_id", note_id.clone()),
            PublicInput::new("nullifier", nullifier.clone()),
            PublicInput::new("to", "fuzz-recipient"),
            PublicInput::new("amount", "10"),
            PublicInput::new("spend_id", spend_id.clone()),
        ],
    );
    let valid = system.prove(&statement)?;
    let seed = serde_json::to_vec(&valid)?;
    let malformed_statements = vec![
        ProofStatement::new("", vec![PublicInput::new("nullifier", "fuzz-nullifier")]),
        ProofStatement::new(DEBUG_SHIELDED_SPEND_CIRCUIT_ID, Vec::new()),
        ProofStatement::new(
            DEBUG_SHIELDED_SPEND_CIRCUIT_ID,
            vec![PublicInput::new("", "fuzz-nullifier")],
        ),
        ProofStatement::new(
            DEBUG_SHIELDED_SPEND_CIRCUIT_ID,
            vec![
                PublicInput::new("nullifier", "fuzz-nullifier"),
                PublicInput::new("nullifier", "duplicate"),
            ],
        ),
        ProofStatement::new(
            DEBUG_SHIELDED_SPEND_CIRCUIT_ID,
            vec![
                PublicInput::new("note_id", note_id.clone()),
                PublicInput::new("nullifier", nullifier.clone()),
            ],
        ),
        ProofStatement::new(
            DEBUG_SHIELDED_SPEND_CIRCUIT_ID,
            vec![
                PublicInput::new("note_id", "not-hex"),
                PublicInput::new("nullifier", nullifier.clone()),
                PublicInput::new("to", "fuzz-recipient"),
                PublicInput::new("amount", "10"),
                PublicInput::new("spend_id", spend_id.clone()),
            ],
        ),
        ProofStatement::new(
            DEBUG_SHIELDED_SPEND_CIRCUIT_ID,
            vec![
                PublicInput::new("note_id", note_id.clone()),
                PublicInput::new("nullifier", nullifier.clone()),
                PublicInput::new("to", "fuzz-recipient"),
                PublicInput::new("amount", "0"),
                PublicInput::new("spend_id", spend_id),
            ],
        ),
        ProofStatement::new(
            "unknown_debug_circuit",
            vec![PublicInput::new("input", "value")],
        ),
    ];
    let mut report =
        FuzzTargetReport::new("proof-adapter", iterations, 1 + malformed_statements.len());
    report.assert_invariant(system.verify(&statement, &valid).is_ok());
    for malformed in malformed_statements {
        report.assert_invariant(system.prove(&malformed).is_err());
        report.assert_invariant(system.verify(&malformed, &valid).is_err());
    }

    for input in mutated_inputs(&seed, iterations) {
        match serde_json::from_slice::<ProofArtifact>(&input) {
            Ok(artifact) => {
                report.record_parse(true);
                let verified = system.verify(&statement, &artifact).is_ok();
                if artifact == valid {
                    report.assert_invariant(verified);
                }
            }
            Err(_) => report.record_parse(false),
        }
    }

    Ok(report)
}

fn repeated_hex(ch: char) -> String {
    std::iter::repeat_n(ch, 96).collect()
}

fn is_lower_hex_96(value: &str) -> bool {
    value.len() == 96
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn mutated_inputs(seed: &[u8], iterations: usize) -> Vec<Vec<u8>> {
    let mut inputs = Vec::with_capacity(iterations + 1);
    inputs.push(seed.to_vec());
    for i in 0..iterations {
        let mut data = seed.to_vec();
        if data.is_empty() {
            data.push(i as u8);
            inputs.push(data);
            continue;
        }
        let index = (i.wrapping_mul(37).wrapping_add(13)) % data.len();
        match i % 6 {
            0 => data[index] ^= 0x01,
            1 => data[index] = data[index].wrapping_add((i as u8).wrapping_add(17)),
            2 => {
                data.insert(index, b'X');
            }
            3 => {
                data.remove(index);
            }
            4 => {
                data.truncate(index.max(1));
            }
            _ => {
                data.extend_from_slice(format!("\"fuzz{i}\"").as_bytes());
            }
        }
        inputs.push(data);
    }
    inputs
}

fn flag_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|window| window[0] == flag)
        .map(|window| window[1].as_str())
}
