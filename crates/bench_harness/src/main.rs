use std::env;
use std::error::Error;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use postfiat_bridge::{
    apply_simulated_transfer, bridge_witness_attestation_id, bridge_witness_attestation_message,
    upsert_domain, BridgeTransferRequest, BridgeWitnessChainDomain,
};
use postfiat_crypto_provider::{
    address_from_public_key, bytes_to_hex, hash_bytes, hash_hex, ml_dsa_65_keygen, ml_dsa_65_sign,
    ml_dsa_65_sign_with_context_seed, ml_dsa_65_verify, BRIDGE_WITNESS_SIGNATURE_CONTEXT,
    ML_DSA_65_ALGORITHM, ML_DSA_65_PUBLIC_KEY_BYTES, ML_DSA_65_SIGNATURE_BYTES,
};
use postfiat_execution::{
    execute_transfer, genesis_hash, minimum_transfer_fee, ACCOUNT_RESERVE, MIN_TRANSFER_FEE,
    TRANSFER_ACCOUNT_CREATION_FEE,
};
use postfiat_mempool_dag::{build_transaction_batch, MempoolBatchDomain};
use postfiat_ordering_fast::{bft_quorum_threshold, order_references};
use postfiat_privacy::{mint_debug_note, scan_owner};
use postfiat_proofs::{
    DebugProofSystem, ProofStatement, ProofSystem, PublicInput, DEBUG_SHIELDED_SPEND_CIRCUIT_ID,
};
use postfiat_storage::NodeStore;
use postfiat_types::{
    Account, BlockCertificate, BlockCertificateVote, BridgeState, BridgeWitnessAttestation,
    Genesis, LedgerState, NodeState, SignedTransfer, UnsignedTransfer, ADDRESS_NAMESPACE,
    BRIDGE_DIRECTION_INBOUND, DEFAULT_SHIELDED_ASSET_ID, TRANSFER_TRANSACTION_KIND,
};
use serde::Serialize;
use serde_json::json;

const DEFAULT_ITERATIONS: usize = 16;

#[derive(Debug, Serialize)]
struct BenchSuiteReport {
    schema: &'static str,
    iterations: usize,
    benchmarks: Vec<BenchReport>,
}

#[derive(Debug, Serialize)]
struct BenchReport {
    name: &'static str,
    operations: u64,
    elapsed_ms: u128,
    elapsed_ns: u128,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<serde_json::Value>,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let iterations = flag_value(&args, "--iterations")
        .map(str::parse::<usize>)
        .transpose()?
        .unwrap_or(DEFAULT_ITERATIONS);
    if iterations == 0 {
        return Err("--iterations must be positive".into());
    }

    let report = BenchSuiteReport {
        schema: "postfiat-benchmark-suite-v1",
        iterations,
        benchmarks: vec![
            bench_signature(iterations)?,
            bench_certificate_size_model()?,
            bench_proof_adapter(iterations)?,
            bench_ordering(iterations)?,
            bench_execution(iterations)?,
            bench_storage(iterations)?,
            bench_wallet_scan(iterations)?,
            bench_bridge(iterations)?,
        ],
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn bench_signature(iterations: usize) -> Result<BenchReport, Box<dyn Error>> {
    let keygen_start = Instant::now();
    let key_pair = ml_dsa_65_keygen()?;
    let keygen_elapsed = keygen_start.elapsed();
    let messages = (0..iterations)
        .map(|index| {
            let mut message = b"postfiat benchmark signature".to_vec();
            message.extend_from_slice(&index.to_le_bytes());
            message
        })
        .collect::<Vec<_>>();

    let total_start = Instant::now();
    let sign_start = Instant::now();
    let signatures = messages
        .iter()
        .map(|message| ml_dsa_65_sign(&key_pair.private_key, message))
        .collect::<Result<Vec<_>, _>>()?;
    let sign_elapsed = sign_start.elapsed();

    let verify_start = Instant::now();
    for (message, signature) in messages.iter().zip(signatures.iter()) {
        if !ml_dsa_65_verify(&key_pair.public_key, message, signature) {
            return Err("signature verification failed during benchmark".into());
        }
    }
    let verify_elapsed = verify_start.elapsed();
    let total_elapsed = total_start.elapsed();
    let sample_message_bytes = messages.first().map(Vec::len).unwrap_or_default();
    let signature_bytes = signatures.first().map(Vec::len).unwrap_or_default();
    let details = json!({
        "algorithm_id": ML_DSA_65_ALGORITHM,
        "public_key_bytes": key_pair.public_key.len(),
        "private_key_bytes": key_pair.private_key.len(),
        "signature_bytes": signature_bytes,
        "sample_message_bytes": sample_message_bytes,
        "keygen_elapsed_ns": duration_ns(keygen_elapsed),
        "sign_elapsed_ns": duration_ns(sign_elapsed),
        "verify_elapsed_ns": duration_ns(verify_elapsed),
        "sign_ops_per_second": ops_per_second(iterations, sign_elapsed),
        "verify_ops_per_second": ops_per_second(iterations, verify_elapsed)
    });
    Ok(report_from_elapsed(
        "signature_sign_verify",
        iterations as u64,
        total_elapsed,
        Some(details),
    ))
}

fn bench_certificate_size_model() -> Result<BenchReport, Box<dyn Error>> {
    let validator_counts = [4usize, 5, 7, 10, 21, 33, 100];
    let start = Instant::now();
    let mut rows = Vec::with_capacity(validator_counts.len());
    for validator_count in validator_counts {
        let quorum = bft_quorum_threshold(validator_count)?;
        let compact_certificate_json_bytes =
            modeled_certificate_json_bytes(validator_count, quorum, false)?;
        let with_public_keys_certificate_json_bytes =
            modeled_certificate_json_bytes(validator_count, quorum, true)?;
        rows.push(json!({
            "validator_count": validator_count,
            "quorum": quorum,
            "signature_bytes_per_vote": ML_DSA_65_SIGNATURE_BYTES,
            "public_key_bytes_per_vote": ML_DSA_65_PUBLIC_KEY_BYTES,
            "compact_vote_crypto_bytes": quorum * ML_DSA_65_SIGNATURE_BYTES,
            "with_public_keys_vote_crypto_bytes": quorum * (ML_DSA_65_SIGNATURE_BYTES + ML_DSA_65_PUBLIC_KEY_BYTES),
            "compact_certificate_json_bytes": compact_certificate_json_bytes,
            "with_public_keys_certificate_json_bytes": with_public_keys_certificate_json_bytes,
            "registry_public_key_savings_bytes": with_public_keys_certificate_json_bytes.saturating_sub(compact_certificate_json_bytes)
        }));
    }
    let details = json!({
        "algorithm_id": ML_DSA_65_ALGORITHM,
        "public_key_bytes": ML_DSA_65_PUBLIC_KEY_BYTES,
        "signature_bytes": ML_DSA_65_SIGNATURE_BYTES,
        "model": "BlockCertificate JSON with registry-root-bound compact votes; public-key variant models legacy/self-contained votes.",
        "rows": rows
    });
    Ok(report_from_elapsed(
        "certificate_size_model",
        validator_counts.len() as u64,
        start.elapsed(),
        Some(details),
    ))
}

fn modeled_certificate_json_bytes(
    validator_count: usize,
    quorum: usize,
    include_public_keys: bool,
) -> Result<usize, Box<dyn Error>> {
    let validators = (0..validator_count)
        .map(|index| format!("validator-{index}"))
        .collect::<Vec<_>>();
    let registry_root = "c".repeat(96);
    let signature_hex = "a".repeat(ML_DSA_65_SIGNATURE_BYTES * 2);
    let public_key_hex = "b".repeat(ML_DSA_65_PUBLIC_KEY_BYTES * 2);
    let votes = validators
        .iter()
        .take(quorum)
        .map(|validator| BlockCertificateVote {
            vote_id: hash_hex("postfiat.bench.certificate.vote.v1", validator.as_bytes()),
            validator: validator.clone(),
            accept: true,
            algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            registry_root: registry_root.clone(),
            public_key_hex: if include_public_keys {
                public_key_hex.clone()
            } else {
                String::new()
            },
            signature_hex: signature_hex.clone(),
        })
        .collect::<Vec<_>>();
    let certificate = BlockCertificate {
        validators,
        quorum,
        registry_root,
        votes,
    };
    Ok(serde_json::to_vec(&certificate)?.len())
}

fn bench_proof_adapter(iterations: usize) -> Result<BenchReport, Box<dyn Error>> {
    let system = DebugProofSystem::for_controlled_testnet_debug()?;
    let start = Instant::now();
    for index in 0..iterations {
        let index_bytes = index.to_le_bytes();
        let statement = ProofStatement::new(
            DEBUG_SHIELDED_SPEND_CIRCUIT_ID,
            vec![
                PublicInput::new(
                    "note_id",
                    hash_hex("postfiat.bench.proof.note.v1", &index_bytes),
                ),
                PublicInput::new(
                    "nullifier",
                    hash_hex("postfiat.bench.proof.nullifier.v1", &index_bytes),
                ),
                PublicInput::new("to", "bench-recipient"),
                PublicInput::new("amount", "10"),
                PublicInput::new(
                    "spend_id",
                    hash_hex("postfiat.bench.proof.spend.v1", &index_bytes),
                ),
            ],
        );
        let artifact = system.prove(&statement)?;
        system.verify(&statement, &artifact)?;
    }
    Ok(report("proof_adapter_prove_verify", iterations, start))
}

fn bench_ordering(iterations: usize) -> Result<BenchReport, Box<dyn Error>> {
    let mut references = Vec::with_capacity(iterations);
    let genesis = Genesis::new("postfiat-bench-ordering");
    let genesis_hash_hex = genesis_hash(&genesis);
    let batch_domain = MempoolBatchDomain {
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash_hex.clone(),
        protocol_version: genesis.protocol_version,
    };
    let start = Instant::now();
    for index in 0..iterations {
        let batch = build_transaction_batch(
            &batch_domain,
            vec![dummy_transfer(
                &genesis,
                &genesis_hash_hex,
                index as u64 + 1,
            )],
        )?;
        references.push(batch.reference);
    }
    let ordered = order_references(references);
    if ordered.len() != iterations {
        return Err("ordering benchmark lost references".into());
    }
    Ok(report("ordering_reference_sort", iterations, start))
}

fn bench_execution(iterations: usize) -> Result<BenchReport, Box<dyn Error>> {
    let key_pair = ml_dsa_65_keygen()?;
    let genesis = Genesis::new("postfiat-bench");
    let from = address_from_public_key(&key_pair.public_key);
    let mut ledger = LedgerState::new(vec![Account::new(
        from.clone(),
        (iterations as u64 * 10_000) + ACCOUNT_RESERVE,
        Some(bytes_to_hex(&key_pair.public_key)),
    )]);

    let start = Instant::now();
    for index in 0..iterations {
        let transfer = signed_transfer(
            &genesis,
            &key_pair.private_key,
            &key_pair.public_key,
            &from,
            &format!("pfbenchdest{index:030}"),
            ACCOUNT_RESERVE,
            index as u64 + 1,
        )?;
        let receipt = execute_transfer(&genesis, &mut ledger, &transfer);
        if !receipt.accepted {
            return Err(format!("execution benchmark transfer rejected: {}", receipt.code).into());
        }
    }
    Ok(report("execution_transfer", iterations, start))
}

fn bench_storage(iterations: usize) -> Result<BenchReport, Box<dyn Error>> {
    let dir = temp_dir("postfiat-bench-storage");
    let store = NodeStore::new(&dir);
    let genesis = Genesis::new("postfiat-bench-storage");
    store.init(&genesis, &NodeState::initialized("bench-validator"))?;

    let start = Instant::now();
    for index in 0..iterations {
        let mut ledger = store.read_ledger()?;
        ledger.accounts.push(Account::new(
            format!("pfstorage{index:031}"),
            index as u64,
            None,
        ));
        store.write_ledger(&ledger)?;
        let _ = store.read_ledger()?;
    }
    let _ = std::fs::remove_dir_all(dir);
    Ok(report("storage_ledger_roundtrip", iterations, start))
}

fn bench_wallet_scan(iterations: usize) -> Result<BenchReport, Box<dyn Error>> {
    let mut state = postfiat_types::ShieldedState::empty();
    for index in 0..iterations {
        let owner = if index % 2 == 0 { "alice" } else { "bob" };
        mint_debug_note(
            &mut state,
            owner,
            DEFAULT_SHIELDED_ASSET_ID,
            index as u64 + 1,
            format!("bench-note-{index}"),
        )?;
    }

    let start = Instant::now();
    for _ in 0..iterations {
        let notes = scan_owner(&state, "alice");
        if notes.is_empty() {
            return Err("wallet scan benchmark found no alice notes".into());
        }
    }
    Ok(report("wallet_scan", iterations, start))
}

fn bench_bridge(iterations: usize) -> Result<BenchReport, Box<dyn Error>> {
    let mut state = BridgeState::empty();
    upsert_domain(
        &mut state,
        "bench-bridge",
        "Benchmark Bridge",
        iterations as u64 + 1,
        iterations as u64 + 1,
    )?;
    let requests = (0..iterations)
        .map(|index| {
            attested_bridge_request(
                &state,
                BridgeTransferRequest {
                    domain_id: "bench-bridge".to_string(),
                    direction: BRIDGE_DIRECTION_INBOUND.to_string(),
                    from: format!("source-{index}"),
                    to: format!("pfbridgedest{index:029}"),
                    asset_id: DEFAULT_SHIELDED_ASSET_ID.to_string(),
                    amount: 1,
                    witness_id: format!("bench-witness-{index}"),
                    witness_epoch: 1,
                    witness_attestation: None,
                },
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    let start = Instant::now();
    for (index, request) in requests.into_iter().enumerate() {
        let transfer = apply_simulated_transfer(&mut state, request)?;
        if transfer.sequence != index as u64 + 1 {
            return Err("bridge benchmark sequence mismatch".into());
        }
    }
    Ok(report("bridge_simulation", iterations, start))
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
    let genesis = Genesis::new("postfiat-bench-bridge");
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

fn dummy_transfer(genesis: &Genesis, genesis_hash_hex: &str, sequence: u64) -> SignedTransfer {
    SignedTransfer {
        unsigned: UnsignedTransfer {
            chain_id: genesis.chain_id.clone(),
            genesis_hash: genesis_hash_hex.to_string(),
            protocol_version: genesis.protocol_version,
            address_namespace: ADDRESS_NAMESPACE.to_string(),
            transaction_kind: TRANSFER_TRANSACTION_KIND.to_string(),
            signature_algorithm_id: "bench".to_string(),
            from: "pfbenchsource".to_string(),
            to: "pfbenchdest".to_string(),
            amount: 1,
            fee: MIN_TRANSFER_FEE,
            sequence,
        },
        algorithm_id: "bench".to_string(),
        public_key_hex: "00".to_string(),
        signature_hex: "11".to_string(),
    }
}

fn signed_transfer(
    genesis: &Genesis,
    private_key: &[u8],
    public_key: &[u8],
    from: &str,
    to: &str,
    amount: u64,
    sequence: u64,
) -> Result<SignedTransfer, Box<dyn Error>> {
    let mut fee = MIN_TRANSFER_FEE;
    for _ in 0..8 {
        let unsigned = UnsignedTransfer {
            chain_id: genesis.chain_id.clone(),
            genesis_hash: genesis_hash(genesis),
            protocol_version: genesis.protocol_version,
            address_namespace: ADDRESS_NAMESPACE.to_string(),
            transaction_kind: TRANSFER_TRANSACTION_KIND.to_string(),
            signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            from: from.to_string(),
            to: to.to_string(),
            amount,
            fee,
            sequence,
        };
        let signature = ml_dsa_65_sign(private_key, &unsigned.signing_bytes())?;
        let transfer = SignedTransfer {
            unsigned,
            algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            public_key_hex: bytes_to_hex(public_key),
            signature_hex: bytes_to_hex(&signature),
        };
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

fn report(name: &'static str, iterations: usize, start: Instant) -> BenchReport {
    report_from_elapsed(name, iterations as u64, start.elapsed(), None)
}

fn report_from_elapsed(
    name: &'static str,
    operations: u64,
    elapsed: std::time::Duration,
    details: Option<serde_json::Value>,
) -> BenchReport {
    BenchReport {
        name,
        operations,
        elapsed_ms: elapsed.as_millis(),
        elapsed_ns: elapsed.as_nanos(),
        details,
    }
}

fn duration_ns(duration: std::time::Duration) -> u64 {
    u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX)
}

fn ops_per_second(operations: usize, elapsed: std::time::Duration) -> f64 {
    let elapsed_ns = elapsed.as_nanos();
    if elapsed_ns == 0 {
        return 0.0;
    }
    (operations as f64 * 1_000_000_000.0) / elapsed_ns as f64
}

fn temp_dir(prefix: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    env::temp_dir().join(format!("{prefix}-{}-{nanos}", std::process::id()))
}

fn flag_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|window| window[0] == flag)
        .map(|window| window[1].as_str())
}
