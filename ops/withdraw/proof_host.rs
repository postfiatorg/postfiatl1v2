//! Frozen mainnet USDC egress host. No signing keys and no chain writes.
use std::{fs,path::PathBuf,time::Instant};
use anyhow::{ensure,Result};
use sha2::{Digest,Sha256};
use sp1_sdk::{Elf,HashableKey,ProveRequest,Prover,ProverClient,ProvingKey,SP1Stdin};
use postfiat_types::{PfUsdcEgressProgramInputV1,PfUsdcEgressProofWitnessV1,PfUsdcCheckpointProofWitnessV1};
const ELF_HASH:&str="4d5f84493c9b02b0d2a082c446229e30ce6645210a00c271dfb125b2761c67e0";
const VKEY:&str="0x0015b046ba4b80c0ca7e2d9429a1f5fd88bc6d1d328cca6acec29ffdf48a9d87";
fn main()->Result<()> {tokio::runtime::Runtime::new()?.block_on(run())}
async fn run()->Result<()> {
 let a:Vec<String>=std::env::args().collect();ensure!(a.len()==6,"usage: proof-host withdrawal|checkpoint ELF WITNESS OUTPUT native|execute|prove");
 let bytes=fs::read(&a[2])?;ensure!(hex::encode(Sha256::digest(&bytes))==ELF_HASH,"frozen ELF mismatch");
 let (input,expected)=if a[1]=="withdrawal" {
   let w:PfUsdcEgressProofWitnessV1=serde_json::from_slice(&fs::read(&a[3])?)?;
   let v=postfiat_pfusdc_proofs::verify_egress_witness_v1(&w).map_err(anyhow::Error::msg)?;
   (PfUsdcEgressProgramInputV1::Withdrawal(w),v.canonical_bytes_without_commitment().map_err(anyhow::Error::msg)?)
 } else {ensure!(a[1]=="checkpoint","invalid mode");let w:PfUsdcCheckpointProofWitnessV1=serde_json::from_slice(&fs::read(&a[3])?)?;
   let v=postfiat_pfusdc_proofs::verify_checkpoint_witness_v1(&w).map_err(anyhow::Error::msg)?;
   (PfUsdcEgressProgramInputV1::Checkpoint(w),v.canonical_bytes_without_commitment().map_err(anyhow::Error::msg)?) };
 let out=PathBuf::from(&a[4]);fs::create_dir_all(&out)?;fs::write(out.join("native-public-values.bin"),&expected)?;
 if a[5]=="native" {println!("native verified {} bytes",expected.len());return Ok(())}
 ensure!(a[5]=="execute"||a[5]=="prove","invalid action");
 let mut stdin=SP1Stdin::new();stdin.write_vec(serde_cbor::to_vec(&input)?);
 let client=ProverClient::from_env().await;let elf=Elf::from(bytes);let start=Instant::now();
 let (values,report)=client.execute(elf.clone(),stdin.clone()).await?;
 ensure!(values.to_vec()==expected,"frozen guest differs from native verifier");
 fs::write(out.join("public-values.bin"),&expected)?;
 fs::write(out.join("execute-report.json"),serde_json::to_vec_pretty(&serde_json::json!({"elf_sha256":ELF_HASH,"cycles":report.total_instruction_count(),"elapsed_ms":start.elapsed().as_millis(),"native_guest_equal":true,"public_values_bytes":expected.len()}))?)?;
 println!("frozen guest executed: {} cycles in {} ms",report.total_instruction_count(),start.elapsed().as_millis());
 if a[5]=="prove" {
   let pk=client.setup(elf).await?;ensure!(pk.verifying_key().bytes32()==VKEY,"deployed vkey mismatch");
   let start=Instant::now();let proof=client.prove(&pk,stdin).groth16().await?;client.verify(&proof,pk.verifying_key(),None)?;
   ensure!(proof.public_values.to_vec()==expected,"proved values mismatch");
   fs::write(out.join("proof.bin"),bincode::serialize(&proof)?)?;fs::write(out.join("proof-calldata.bin"),proof.bytes())?;
   fs::write(out.join("proof-report.json"),serde_json::to_vec_pretty(&serde_json::json!({"program_vkey":VKEY,"elf_sha256":ELF_HASH,"proof_mode":"groth16","prover_backend":std::env::var("SP1_PROVER").unwrap_or_default(),"proof_bytes":proof.bytes().len(),"elapsed_ms":start.elapsed().as_millis(),"verified":true}))?)?;
   println!("verified real Groth16 proof in {} ms",start.elapsed().as_millis());
 }
 Ok(())
}
