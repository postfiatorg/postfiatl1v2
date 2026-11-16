//! Copy-only signed burn and synthetic consensus fixture. Never opens key files.
use std::{fs,path::Path};
use anyhow::{ensure,Result};
use serde_json::{json,Value};
use postfiat_crypto_provider::*;
use postfiat_ordering_fast::*;
use postfiat_types::*;
include!("fixture_consensus.rs");
const FAMILY:&str="02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b";
const SERIES:&str="541e451e6ae96b6680988655571a0feec59c7f1d073c9b5d29547254cab38a3be540f8767446d25baebc7081ce97aef7";
const BUCKET:&str="a1dbe54059cd948e25b6695a9ea32f95d8236b1616a6cff47c2ca20992c04b991bb03d3b6a805990e0dd5cd67b2af090";
const RECIPIENT:&str="0x00000000000000000000000000000000000a11ce";
fn save(dir:&Path,name:&str,v:&Value)->Result<()>{fs::write(dir.join(name),serde_json::to_vec_pretty(v)?)?;Ok(())}
fn main()->Result<()>{
 let a:Vec<String>=std::env::args().collect();ensure!(a.len()==4,"usage: rehearsal burn|witness SNAPSHOT OUTPUT");let mut s:Value=serde_json::from_slice(&fs::read(&a[2])?)?;let out=Path::new(&a[3]);fs::create_dir_all(out)?;
 let height=s["tip"]["height"].as_u64().unwrap()+1;
 if a[1]=="burn" {
   let genesis:Genesis=serde_json::from_value(s["genesis"].clone())?;
   // Public deterministic TEST key stays in memory. Shift 0.5 of existing S
   // to its account on this local copy; do not invent USDC backing or supply.
   let key=ml_dsa_65_keygen_from_seed(&[0x72;32]);let owner=address_from_public_key(&key.public_key);
   let account=Account::new(owner.clone(),100_000_000,Some(bytes_to_hex(&key.public_key)));
   s["ledger"]["accounts"].as_array_mut().unwrap().push(serde_json::to_value(account)?);
   let lines=s["ledger"]["trustlines"].as_array_mut().unwrap();
   let i=lines.iter().position(|x|x["asset_id"]==SERIES&&x["account"]=="pfab9b9228942e5c529633a13aa271d5297bec6353").unwrap();
   let before=lines[i]["balance"].as_u64().unwrap();ensure!(before>=500_000,"copy source insufficient");
   let mut line=lines[i].clone();lines[i]["balance"]=json!(before-500_000);line["account"]=json!(owner);line["balance"]=json!(500_000);line["limit"]=json!(1_000_000);line["trustline_id"]=json!(trustline_id(&owner,line["issuer"].as_str().unwrap(),SERIES).map_err(anyhow::Error::msg)?);lines.push(line);
   let nav=s["ledger"]["nav_assets"].as_array().unwrap().iter().find(|x|x["asset_id"]==FAMILY).unwrap();
   let operation:AssetTransactionOperation=serde_json::from_value(json!({"operation":"vault_bridge_burn_to_redeem","owner":owner,"issuer":nav["issuer"],"asset_id":FAMILY,"bucket_id":BUCKET,"amount_atoms":500_000,"epoch":nav["finalized_epoch"],"reserve_packet_hash":nav["finalized_reserve_packet_hash"],"destination_ref":format!("evm-erc20:1:{RECIPIENT}")}))?;
   let unsigned=UnsignedAssetTransaction{chain_id:genesis.chain_id.clone(),genesis_hash:s["tip"]["genesis_hash"].as_str().unwrap().into(),protocol_version:genesis.protocol_version,address_namespace:ADDRESS_NAMESPACE.into(),transaction_kind:VAULT_BRIDGE_BURN_TO_REDEEM_TRANSACTION_KIND.into(),signature_algorithm_id:ML_DSA_65_ALGORITHM.into(),source:owner.clone(),fee:1000,sequence:1,operation};
   let tx=SignedAssetTransaction{signature_hex:bytes_to_hex(&ml_dsa_65_sign(&key.private_key,&unsigned.signing_bytes()).map_err(anyhow::Error::msg)?),unsigned,algorithm_id:ML_DSA_65_ALGORITHM.into(),public_key_hex:bytes_to_hex(&key.public_key)};
   save(out,"fixture-snapshot.json",&s)?;
   save(out,"steps.json",&json!([{"label":"epoch11-source-series-burn","height":height,"transaction":tx,"kind":"asset","expect_accepted":true}]))?;
   save(out,"fixture-disclosure.json",&json!({"copy_only":true,"test_owner":owner,"recipient":RECIPIENT,"source_series_atoms_reassigned_from_alice":500000,"source_series_supply_added":0,"backing_added":0,"test_native_fee_atoms_added":100000000,"live_keys_read":false,"signature_generated_in_memory":true,"not_live_holder_authorization":true}))?;
 } else {
   ensure!(a[1]=="witness","bad mode");ensure!(s["canary_pass"]==true,"burn did not pass");
   let receipt:Receipt=serde_json::from_value(s["canary_receipts"][0]["receipt"].clone())?;ensure!(receipt.accepted,"burn rejected");
   let redemption:VaultBridgeRedemption=serde_json::from_value(s["ledger"]["vault_bridge_redemptions"].as_array().unwrap().iter().find(|r|r["created_at_height"]==height&&r["withdrawal_packet"]["burn_tx_id"]==receipt.tx_id).unwrap().clone())?;
   let route:VaultBridgeRouteProfileRecordV1=serde_json::from_value(s["governance"]["vault_bridge_route_profiles"].as_array().unwrap().iter().find(|r|r["profile"]["route_epoch"]==11&&r["profile"]["asset_id"]==FAMILY).unwrap().clone())?;
   let leaf=BridgeExitLeafV1::from_withdrawal_packet(11,receipt.tx_id.clone(),receipt.code.clone(),&redemption.withdrawal_packet,redemption.withdrawal_packet_hash.clone(),redemption.withdrawal_packet_evm_digest.clone()).map_err(anyhow::Error::msg)?;
   let root=bridge_exit_merkle_root_v1(&[leaf.clone()]).map_err(anyhow::Error::msg)?;
   let merkle=bridge_exit_merkle_proof_v1(&[leaf],0).map_err(anyhow::Error::msg)?;
   let (validators,keys)=committee();let epoch=1;
   let chain=s["tip"]["chain_id"].as_str().unwrap();let genesis=s["tip"]["genesis_hash"].as_str().unwrap();let protocol=s["tip"]["protocol_version"].as_u64().unwrap() as u32;
   let domain=consensus_v2_domain(chain,genesis,protocol,epoch,&validators);
   let parent=s["tip"]["block_hash"].as_str().unwrap().to_string();
   // This state digest is only a fixture header commitment. The actual
   // release-4 executor's entire resulting ledger is retained separately.
   let state=hash_hex("withdraw-fixture-state",&serde_json::to_vec(&s["ledger"])?);
   let block=signed_block_record(&domain,&validators,&keys,height,parent.clone(),hash_hex("withdraw-fixture-payload",receipt.tx_id.as_bytes()),state,Some(root),"transactions",hash_hex("withdraw-fixture-batch",b"withdraw-local-batch"),vec![receipt.tx_id.clone()]);
   let committee=validators.validators.iter().map(|v|ValidatorRegistryEntry{node_id:v.validator_id.clone(),algorithm_id:ML_DSA_65_ALGORITHM.into(),public_key_hex:v.public_key_hex.clone(),active:true}).collect();
   let w=PfUsdcEgressProofWitnessV1{schema:PFUSDC_EGRESS_PROOF_WITNESS_SCHEMA_V1.into(),chain_id:chain.into(),genesis_hash:genesis.into(),protocol_version:protocol,bridge_exit_root_activation_height:1,prior_checkpoint_block_id:parent,finality_ancestry:vec![],route_profile:route,block,receipt,merkle_proof:merkle,withdrawal_packet:redemption.withdrawal_packet,withdrawal_packet_hash:redemption.withdrawal_packet_hash,withdrawal_packet_evm_digest:redemption.withdrawal_packet_evm_digest,committee_epoch:epoch,committee};
   let pv=postfiat_pfusdc_proofs::verify_egress_witness_v1(&w).map_err(anyhow::Error::msg)?;
   save(out,"original-witness.json",&serde_json::to_value(&w)?)?;save(out,"original-public-values.json",&serde_json::to_value(&pv)?)?;
   fs::write(out.join("original-public-values.bin"),pv.canonical_bytes_without_commitment().map_err(anyhow::Error::msg)?)?;
   save(out,"consensus-fixture-disclosure.json",&json!({"synthetic_committee":true,"real_live_finality":false,"test_keys_in_memory":true,"live_key_files_read":false,"leaf_epoch":11,"release4_burn_accepted":true,"source_snapshot_height":height-1,"committee_root":validators.committee_root,"state_root_is_fixture_digest":true,"fork_requires_explicit_local_committee_and_checkpoint_storage_override":true}))?;
 }
 Ok(())
}
