//! Local relay gate: load the vault at genesis, then initialize it and execute a real
//! deposit instruction. Both the token transfer and receipt creation execute in SBF.
use anyhow::Result;
use sha2::{Digest, Sha256};
use solana_account::Account;
use solana_instruction::{AccountMeta, Instruction};
use solana_keypair::Keypair;
use solana_local_cluster::local_cluster::ClusterConfig;
use solana_pubkey::Pubkey;
use solana_rent::Rent;
use solana_signature::Signature;
use solana_signer::Signer;
use std::{fs, path::Path};

pub const AMOUNT: u64 = 5_000_000;
pub const ROUTE_ID: &str = "solana-local-usdc-v1";
pub const CHAIN_ID: &str = "postfiat-wan-devnet-2";
pub const GENESIS: &str = "ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad25521aff3ed334da07e150a7233a3e90a9";
pub const SUPPLY: u64 = 1000 * AMOUNT;
const RECIPIENT: [u8; 20] = [0x33; 20];

pub struct Deposit {
    pub program: Pubkey,
    pub deployment: Keypair,
    pub config: Pubkey,
    pub custody: Pubkey,
    mint: Pubkey,
}

impl Deposit {
    pub fn genesis(
        cluster: &mut ClusterConfig,
        mint: Pubkey,
        owner: Pubkey,
        elf: &Path,
    ) -> Result<Self> {
        let deployment = Keypair::new();
        let program = deployment.pubkey();
        let pda = |seeds: &[&[u8]]| Pubkey::find_program_address(seeds, &program).0;
        let config = pda(&[b"config"]);
        let rent = Rent::default();
        let (id, executable) =
            solana_program_binaries::bpf_loader_program_account(&program, &fs::read(elf)?, &rent);
        cluster.additional_accounts.push((id, executable.into()));
        cluster.additional_accounts.push((
            owner,
            Account {
                lamports: 1_000_000_000,
                ..Account::default()
            }
            .into(),
        ));
        Ok(Self {
            deployment,
            program,
            config,
            custody: pda(&[b"custody"]),
            mint,
        })
    }

    pub fn binding(&self, genesis: &[u8; 32]) -> Result<[u8; 32]> {
        Ok(Sha256::new()
            .chain_update(b"postfiat.pfusdc.solana.route_binding.v1")
            .chain_update(CHAIN_ID)
            .chain_update(hex::decode(GENESIS)?)
            .chain_update(ROUTE_ID)
            .chain_update(genesis)
            .chain_update(self.program.as_ref())
            .finalize()
            .into())
    }

    pub fn initialize(&self, payer: Pubkey, binding: [u8; 32]) -> Vec<Instruction> {
        let mut data = vec![0];
        data.extend(binding);
        data.extend(AMOUNT.to_le_bytes());
        data.extend(SUPPLY.to_le_bytes());
        vec![
            Instruction {
                program_id: self.program,
                data,
                accounts: vec![
                    AccountMeta::new(payer, true),
                    AccountMeta::new_readonly(self.program, true),
                    AccountMeta::new(self.config, false),
                    AccountMeta::new(self.custody, false),
                    AccountMeta::new_readonly(self.mint, false),
                    AccountMeta::new_readonly(spl_token_interface::id(), false),
                    AccountMeta::new_readonly(Pubkey::default(), false),
                ],
            },
            solana_system_interface::instruction::transfer(&payer, &self.config, 1),
            solana_system_interface::instruction::transfer(&payer, &self.custody, 1),
        ]
    }

    fn receipt(&self, nonce: &[u8; 32]) -> Pubkey {
        Pubkey::find_program_address(&[b"receipt", nonce], &self.program).0
    }

    pub fn instruction(&self, owner: Pubkey, source: Pubkey, nonce: [u8; 32]) -> Instruction {
        let mut data = vec![1];
        data.extend(AMOUNT.to_le_bytes());
        data.extend(RECIPIENT);
        data.extend(nonce);
        Instruction {
            program_id: self.program,
            data,
            accounts: vec![
                AccountMeta::new(owner, true),
                AccountMeta::new(source, false),
                AccountMeta::new(self.config, false),
                AccountMeta::new(self.custody, false),
                AccountMeta::new_readonly(self.mint, false),
                AccountMeta::new(self.receipt(&nonce), false),
                AccountMeta::new_readonly(spl_token_interface::id(), false),
                AccountMeta::new_readonly(Pubkey::default(), false),
            ],
        }
    }

    pub fn index(
        &self,
        archive: &Path,
        signature: &Signature,
        nonce: [u8; 32],
        slot: u64,
        block: [u8; 32],
        owner: Pubkey,
        genesis: [u8; 32],
    ) -> Result<()> {
        let root = archive
            .parent()
            .ok_or_else(|| anyhow::anyhow!("missing capture root"))?;
        let directory = root.join(format!("deposit-{signature}"));
        let record = serde_json::json!({"kind":"deposit", "source":"agave-local-vault", "slot":slot,
            "signature":signature.to_string(),"block_id":hex::encode(block),
            "route_id":ROUTE_ID, "chain_id":CHAIN_ID, "genesis_hash":GENESIS, "solana_genesis_hash":hex::encode(genesis),
            "route_binding":hex::encode(self.binding(&genesis)?),
            "receipt_address":hex::encode(self.receipt(&nonce).to_bytes()),"pftl_recipient":format!("pf{}",hex::encode(RECIPIENT)),
            "program_id":hex::encode(self.program.to_bytes()),"depositor":hex::encode(owner.to_bytes()),
            "nonce":hex::encode(nonce),"amount_atoms":AMOUNT.to_string()});
        fs::create_dir_all(&directory)?;
        fs::write(directory.join("source.json"), serde_json::to_vec(&record)?)?;
        fs::write(root.join("signature"), signature.to_string())?;
        Ok(())
    }
}
