//! Host tool for the Solana ingress proof program.
//!
//!   synth   <changed_accounts> <out>   write a synthetic witness of a given size
//!   execute <elf> <deposit-witness>    run the guest without proving and report its cost.
//!                                      Block witnesses come from tools/pfusdc-sol-ingress-difftest.
//!
//! `synth` exists to size the proof before real mainnet blocks can be captured. Its blocks
//! are shaped like mainnet (mostly 165-byte token accounts, a few large accounts) but they
//! are not mainnet blocks.
#[cfg(feature = "cuda")]
mod service;

use pfusdc_sol_ingress_program::{account_lt_hash, bank_hash, verify_deposit_witness, verify_state_witness, AccountState, BankHashPreimage, LtHash, LT_ELEMENTS, RECEIPT_MAGIC};
use serde_json::{json, Value};
use std::time::Instant;
use sha2::{Digest, Sha256};
use sp1_sdk::{blocking::{Prover, ProverClient}, HashableKey, ProvingKey, SP1Stdin};

fn address(index: u32) -> [u8; 32] { Sha256::digest(index.to_le_bytes()).into() }

fn synth(count: u32) -> Vec<u8> {
    let vault = [7u8; 32];
    let route = [9u8; 32];
    let token_program = [6u8; 32];
    let mut before = LtHash([0u16; LT_ELEMENTS]);
    let mut after = LtHash([0u16; LT_ELEMENTS]);
    let mix = |sum: &mut LtHash, bytes: &[u8]| { for (lane, pair) in sum.0.iter_mut().zip(bytes.chunks_exact(2)) { *lane = lane.wrapping_add(u16::from_le_bytes([pair[0], pair[1]])); } };
    let mut body = Vec::new();
    for index in 0..count {
        let key = address(index);
        // One in fifty accounts is large (program state, order books); the rest are token accounts.
        let len = if index % 50 == 49 { 10_000 + (index as usize % 7) * 20_000 } else { 165 };
        let (old, new): (Vec<u8>, Vec<u8>);
        let (old_lamports, new_lamports, owner);
        if index == 0 {
            let mut data = RECEIPT_MAGIC.to_vec();
            data.extend_from_slice(&route); data.extend_from_slice(&[0x33; 20]); data.extend_from_slice(&5_000_000u64.to_le_bytes());
            data.extend_from_slice(&[1; 32]); data.extend_from_slice(&[0x44; 32]);
            old = vec![]; new = data; old_lamports = 0; new_lamports = 1_809_600; owner = vault;
        } else {
            old = vec![(index % 251) as u8; len]; let mut changed = old.clone(); changed[len / 2] ^= 1;
            new = changed; old_lamports = 2_039_280; new_lamports = 2_039_280; owner = token_program;
        }
        let old_state = AccountState { lamports: old_lamports, executable: false, owner, data: &old };
        let new_state = AccountState { lamports: new_lamports, executable: false, owner, data: &new };
        if let Some(hash) = account_lt_hash(&key, &old_state) { mix(&mut before, &hash); mix(&mut after, &hash); }
        if let Some(hash) = account_lt_hash(&key, &old_state) { for (lane, pair) in after.0.iter_mut().zip(hash.chunks_exact(2)) { *lane = lane.wrapping_sub(u16::from_le_bytes([pair[0], pair[1]])); } }
        if let Some(hash) = account_lt_hash(&key, &new_state) { mix(&mut after, &hash); }
        body.extend_from_slice(&key);
        for (lamports, data) in [(old_lamports, &old), (new_lamports, &new)] {
            body.extend_from_slice(&lamports.to_le_bytes()); body.push(0); body.extend_from_slice(&owner);
            body.extend_from_slice(&(data.len() as u32).to_le_bytes()); body.extend_from_slice(data);
        }
    }
    let parent_preimage = BankHashPreimage { parent_bank_hash: [0x11; 32], signature_count: 500, last_blockhash: [0x22; 32] };
    let parent = bank_hash(&parent_preimage, &before);
    let preimage = BankHashPreimage { parent_bank_hash: parent, signature_count: 633, last_blockhash: [0x23; 32] };
    let mut out = Vec::new();
    out.extend_from_slice(&bank_hash(&preimage, &after)); out.extend_from_slice(&vault); out.extend_from_slice(&route);
    out.extend_from_slice(&parent); out.extend_from_slice(&633u64.to_le_bytes()); out.extend_from_slice(&[0x23; 32]); out.extend_from_slice(&after.to_bytes());
    out.extend_from_slice(&[0x11; 32]); out.extend_from_slice(&500u64.to_le_bytes()); out.extend_from_slice(&[0x22; 32]); out.extend_from_slice(&before.to_bytes());
    out.extend_from_slice(&count.to_le_bytes()); out.extend_from_slice(&body);
    out
}

/// The CUDA SDK exposes one cumulative proving request for the selected mode.
/// Record only measured intervals, including the duration of a failed request.
fn timed<T>(report: &mut Value, stage: &str, f: impl FnOnce() -> T) -> T {
    let start = Instant::now();
    let result = f();
    report["seconds"][stage] = json!(start.elapsed().as_secs_f64());
    result
}

async fn timed_request<T>(
    report: &mut Value,
    stage: &str,
    request: impl std::future::IntoFuture<Output = T>,
) -> T {
    let start = Instant::now();
    let result = request.await;
    report["seconds"][stage] = json!(start.elapsed().as_secs_f64());
    result
}

fn public_values(kind: &str, witness: &[u8]) -> Vec<u8> {
    use pfusdc_sol_ingress_program::{verify_key_table_witness, verify_handoff_witness};
    match (kind, witness.split_first()) {
        ("deposit", _) => verify_deposit_witness(witness).expect("invalid deposit").encode(),
        ("key-table", _) => verify_key_table_witness(witness).expect("invalid keys").encode(),
        ("epoch-handoff", _) => verify_handoff_witness(witness).expect("invalid handoff").encode(),
        ("keys", Some((0, body))) => verify_key_table_witness(body).expect("invalid keys").encode(),
        ("handoff", Some((1, body))) => {
            // Preserve in-flight v1 jobs for the combined recorder ELF.
            let mut values = verify_handoff_witness(body).expect("invalid handoff").encode();
            values[..8].copy_from_slice(b"PFSOLHP1");
            values.drain(139..147);
            values
        }
        _ => panic!("invalid recorder witness kind"),
    }
}

async fn prove(args: &[String], report: &mut Value) -> Result<(), String> {
    use sp1_sdk::{ProveRequest, Prover};
    let mode = args.get(4).map(String::as_str).unwrap_or("compressed");
    report["mode"] = json!(mode);
    if !(4..=7).contains(&args.len()) || !matches!(mode, "core" | "compressed" | "groth16") {
        return Err("expected prove <elf> <witness> [core|compressed|groth16]".into());
    }
    let (elf, witness) = timed(report, "read_inputs", || {
        Ok::<_, std::io::Error>((std::fs::read(&args[2])?, std::fs::read(&args[3])?))
    }).map_err(|e| e.to_string())?;
    report["elf_sha256"] = json!(format!("{:x}", Sha256::digest(&elf)));
    report["witness_sha256"] = json!(format!("{:x}", Sha256::digest(&witness)));
    report["witness_bytes"] = json!(witness.len());
    let expected = timed(report, "native_verify", || public_values(args.get(6).map(String::as_str).unwrap_or("deposit"), &witness));
    report["public_values_sha256"] = json!(format!("{:x}", Sha256::digest(&expected)));
    let mut stdin = SP1Stdin::new();
    stdin.write_vec(witness);
    let client = timed_request(report, "client_init", sp1_sdk::ProverClient::from_env()).await;
    let key = timed_request(report, "setup", client.setup(elf.into())).await
        .map_err(|e| format!("setup: {e}"))?;
    report["program_vkey"] = json!(key.verifying_key().bytes32());
    let request = client.prove(&key, stdin);
    let request = match mode {
        "core" => request.core(),
        "compressed" => request.compressed(),
        "groth16" => request.groth16(),
        _ => unreachable!(),
    };
    let proof = timed_request(report, "prove", request).await.map_err(|e| format!("prove: {e}"))?;
    let matches = proof.public_values.as_slice() == expected.as_slice();
    report["public_values_match"] = json!(matches);
    if !matches { return Err("proof and native public values differ".into()); }
    timed(report, "verify", || client.verify(&proof, key.verifying_key(), None))
        .map_err(|e| format!("proof verification: {e}"))?;
    if let Some(directory) = args.get(5) {
        if mode != "groth16" { return Err("relay output requires Groth16".into()); }
        let directory = std::path::Path::new(directory);
        std::fs::write(directory.join("proof.bin"), proof.bytes()).map_err(|e| e.to_string())?;
        std::fs::write(directory.join("public.bin"), &expected).map_err(|e| e.to_string())?;
        std::fs::write(directory.join("vkey"), key.verifying_key().bytes32()).map_err(|e| e.to_string())?;
    }
    report["proof_verified"] = json!(true);
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        #[cfg(feature = "cuda")]
        Some("serve") => {
            if let Err(e) = tokio::runtime::Runtime::new().unwrap().block_on(service::serve(&args[2..])) {
                eprintln!("serve: {e:#}");
                std::process::exit(1);
            }
        }
        Some("synth") => {
            let count: u32 = args[2].parse().expect("changed account count");
            let witness = synth(count);
            let values = verify_state_witness(&witness).expect("synthetic witness must verify natively");
            std::fs::write(&args[3], &witness).expect("write witness");
            println!("accounts={count} witness_bytes={} receipts={}", witness.len(), values.receipts.len());
        }
        Some("execute") => {
            let elf = std::fs::read(&args[2]).expect("read elf");
            let witness = std::fs::read(&args[3]).expect("read witness");
            // The guest runs the complete deposit witness (steps 1 to 6).
            let expected = verify_deposit_witness(&witness).expect("deposit witness must verify natively").encode();
            let mut stdin = SP1Stdin::new();
            stdin.write_vec(witness.clone());
            let client = ProverClient::builder().cpu().build();
            let started = std::time::Instant::now();
            let (public_values, report) = client.execute(elf.into(), stdin).run().expect("guest execution");
            assert_eq!(public_values.as_slice(), expected.as_slice(), "guest and native public values differ");
            println!("witness_bytes={} cycles={} execute_seconds={:.1} public_values_match=true", witness.len(), report.total_instruction_count(), started.elapsed().as_secs_f64());
        }
        // Cost of each stage of the parallel deposit proof, without proving.
        Some("profile") => {
            use pfusdc_sol_ingress_program::shards;
            let dir = std::path::Path::new(&args[2]);
            let witness = std::fs::read(&args[3]).expect("read witness");
            let workers: usize = args.get(4).map_or(4, |n| n.parse().expect("workers"));
            let client = ProverClient::builder().cpu().build();
            let run = |name: &str, stdin: SP1Stdin| {
                let elf = std::fs::read(dir.join(name)).expect("read elf");
                let started = Instant::now();
                let (_, report) = client.execute(elf.into(), stdin).deferred_proof_verification(false).run().expect("guest execution");
                let calls: Vec<_> = report.syscall_counts.iter().filter(|(_, n)| **n > 0).map(|(c, n)| format!("{c:?}={n}")).collect();
                println!("{name} cycles={} gas={:?} seconds={:.1} syscalls=[{}]", report.total_instruction_count(), report.gas(), started.elapsed().as_secs_f64(), calls.join(" "));
            };
            let witness = shards::canonical(&witness).expect("canonical");
            let expected = verify_deposit_witness(&witness).expect("native").encode();
            let (header, leaves) = shards::inputs(&witness, workers).expect("inputs");
            let mut frames = Vec::new();
            for input in leaves {
                println!("leaf bytes={}", input.len());
                let journal = shards::leaf(&input).expect("leaf");
                frames.extend_from_slice(&(journal.len() as u32).to_le_bytes());
                frames.extend_from_slice(&journal);
                run("pfusdc-sol-delta-shard", SP1Stdin::from(&input));
            }
            assert_eq!(shards::aggregate(header, &frames).expect("aggregate").encode(), expected, "aggregate and native values differ");
            println!("header bytes={} frames bytes={} native aggregate matches", header.len(), frames.len());
            let mut stdin = SP1Stdin::new();
            stdin.write_vec(frames);
            stdin.write_vec(header.to_vec());
            run("pfusdc-sol-deposit-aggregate", stdin);
        }
        Some("inspect") => {
            let witness = std::fs::read(&args[3]).expect("read witness");
            std::fs::write(&args[4], public_values(&args[2], &witness)).expect("write native journal");
        }
        Some("verify-groth16") => {
            let proof = std::fs::read(&args[3]).expect("read proof");
            let journal = std::fs::read(&args[4]).expect("read public values");
            sp1_verifier::Groth16Verifier::verify(&proof, &journal, &args[2], &sp1_verifier::GROTH16_VK_BYTES)
                .expect("invalid Groth16 proof or program key");
        }
        Some("vkey") => {
            let elf = std::fs::read(&args[2]).expect("read elf");
            let client = ProverClient::builder().cpu().build();
            println!("{}", client.setup(elf.into()).expect("setup").verifying_key().bytes32());
        }
        // The aggregate guest pins its leaf program by this form of the key.
        Some("vkey-u32") => {
            let elf = std::fs::read(&args[2]).expect("read elf");
            let client = ProverClient::builder().cpu().build();
            println!("{:?}", client.setup(elf.into()).expect("setup").verifying_key().hash_u32());
        }
        Some("prove") => {
            let started = Instant::now();
            let mut report = json!({
                "schema_version": 1,
                "prover": std::env::var("SP1_PROVER").unwrap_or_else(|_| "cpu".into()),
                "seconds": {},
                "proof_verified": false,
            });
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let runtime = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
                runtime.block_on(prove(&args, &mut report))
            })).unwrap_or_else(|_| Err("prover panicked (see stderr)".into()));
            report["seconds"]["total"] = json!(started.elapsed().as_secs_f64());
            if let Err(error) = &result { report["error"] = json!(error); }
            println!("{report}");
            std::process::exit(i32::from(result.is_err()));
        }
        _ => eprintln!("usage: pfusdc-sol-ingress-host synth <accounts> <out> | execute <elf> <witness> | prove <elf> <witness> [core|compressed|groth16]"),
    }
}
