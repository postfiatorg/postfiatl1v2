use crate::witness::{self, State};
use anyhow::{Context, Result, bail, ensure};
use solana_account::{Account, AccountSharedData, ReadableAccount};
use solana_commitment_config::CommitmentConfig;
use solana_core::validator::ValidatorConfig;
use solana_entry::block_component::{
    BlockFinalizationCert, VersionedBlockFooter, VersionedBlockMarker,
};
use solana_hash::Hash;
use solana_instruction::{AccountMeta, Instruction};
use solana_keypair::Keypair;
use solana_ledger::{blockstore::Blockstore, blockstore_meta::BlockLocation};
use solana_local_cluster::{
    cluster::Cluster,
    integration_tests::{DEFAULT_NODE_STAKE, ValidatorKeys},
    local_cluster::{ClusterConfig, LocalCluster},
    validator_configs::make_identical_validator_configs,
};
use solana_net_utils::SocketAddrSpace;
use solana_program_pack::Pack;
use solana_pubkey::Pubkey;
use solana_rent::Rent;
use solana_runtime::bank::Bank;
use solana_signature::Signature;
use solana_signer::Signer;
use solana_transaction::Transaction;
use spl_token_interface::{
    instruction::TokenInstruction,
    state::{Account as TokenAccount, AccountState, Mint},
};
use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::Path,
    thread,
    time::{Duration, Instant},
};

const VALIDATORS: usize = 2;
const SUPPLY: u64 = 1_000_000;

struct Transfer {
    authority: Keypair,
    source: Pubkey,
    destination: Pubkey,
    deposit: Option<crate::relay_deposit::Deposit>,
}

pub(crate) fn token_account<T: Pack>(value: T) -> Result<AccountSharedData> {
    let mut data = vec![0; T::LEN];
    T::pack(value, &mut data)?;
    Ok(Account {
        lamports: Rent::default().minimum_balance(T::LEN),
        data,
        owner: spl_token_interface::id(),
        executable: false,
        rent_epoch: u64::MAX,
    }
    .into())
}

impl Transfer {
    fn genesis(config: &mut ClusterConfig, vault_elf: Option<&Path>) -> Result<Self> {
        config
            .additional_accounts
            .extend(solana_program_binaries::spl_programs(&Rent::default()));
        let mint = Pubkey::new_unique();
        let authority = Keypair::new();
        let deposit = vault_elf
            .map(|elf| {
                crate::relay_deposit::Deposit::genesis(config, mint, authority.pubkey(), elf)
            })
            .transpose()?;
        let supply = if deposit.is_some() {
            crate::relay_deposit::SUPPLY
        } else {
            SUPPLY
        };
        config.additional_accounts.push((
            mint,
            token_account(Mint {
                supply,
                decimals: if deposit.is_some() { 6 } else { 0 },
                is_initialized: true,
                ..Mint::default()
            })?,
        ));
        let transfer = Self {
            authority,
            source: Pubkey::new_unique(),
            destination: deposit
                .as_ref()
                .map(|d| d.custody)
                .unwrap_or_else(Pubkey::new_unique),
            deposit,
        };
        for (address, amount) in [(transfer.source, supply), (transfer.destination, 0)] {
            if transfer.deposit.is_some() && address == transfer.destination {
                continue;
            }
            config.additional_accounts.push((
                address,
                token_account(TokenAccount {
                    mint,
                    owner: transfer
                        .deposit
                        .as_ref()
                        .filter(|_| address == transfer.destination)
                        .map(|d| d.config)
                        .unwrap_or(transfer.authority.pubkey()),
                    amount,
                    state: AccountState::Initialized,
                    ..TokenAccount::default()
                })?,
            ));
        }
        Ok(transfer)
    }

    fn transaction(&self, payer: &Keypair, hash: Hash) -> Transaction {
        if let Some(deposit) = &self.deposit {
            let ix = deposit.instruction(self.authority.pubkey(), self.source, hash.to_bytes());
            // Agave LocalCluster hardcodes Rent::free(). Keep the receipt funded by
            // transferring one lamport after the vault has created it in the same transaction.
            let keep_alive = solana_system_interface::instruction::transfer(
                &payer.pubkey(),
                &ix.accounts[5].pubkey,
                1,
            );
            return Transaction::new_signed_with_payer(
                &[ix, keep_alive],
                Some(&payer.pubkey()),
                &[payer, &self.authority],
                hash,
            );
        }
        let ix = Instruction {
            program_id: spl_token_interface::id(),
            accounts: vec![
                AccountMeta::new(self.source, false),
                AccountMeta::new(self.destination, false),
                AccountMeta::new_readonly(self.authority.pubkey(), true),
            ],
            data: TokenInstruction::Transfer { amount: 1 }.pack(),
        };
        Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[payer, &self.authority],
            hash,
        )
    }

    fn balances(&self, bank: &Bank) -> Result<(u64, u64)> {
        let balance = |key| -> Result<u64> {
            Ok(TokenAccount::unpack(
                bank.get_account(key)
                    .context("missing token account")?
                    .data(),
            )?
            .amount)
        };
        Ok((balance(&self.source)?, balance(&self.destination)?))
    }
}

pub(crate) fn footer_cert(store: &Blockstore, slot: u64) -> Result<Option<BlockFinalizationCert>> {
    let (components, _, full) = store.get_slot_components_with_shred_info(slot, 0, false)?;
    ensure!(full, "incomplete rooted slot {slot}");
    Ok(components.iter().find_map(|component| {
        let VersionedBlockMarker::V1(marker) = component.as_marker()?;
        let VersionedBlockFooter::V1(footer) = marker.as_block_footer()?;
        footer.block_final_cert.clone()
    }))
}

pub fn run(dir: &Path, requested: Option<u64>, vault_elf: Option<&Path>) -> Result<()> {
    fs::create_dir_all(dir)?;
    agave_logger::setup_with_default("error");
    let begin = Instant::now();
    let mut node = ValidatorConfig::default_for_test();
    node.wait_for_supermajority = Some(0);
    node.rpc_config.skip_preflight_health_check = true;
    let mut config = ClusterConfig {
        validator_configs: make_identical_validator_configs(&node, VALIDATORS),
        validator_keys: Some(
            (0..VALIDATORS)
                .map(|_| (ValidatorKeys::new(), true))
                .collect(),
        ),
        node_stakes: vec![DEFAULT_NODE_STAKE; VALIDATORS],
        slots_per_epoch: 2048,
        stakers_slot_offset: 2048,
        skip_warmup_slots: true,
        ..ClusterConfig::default()
    };
    let transfer = Transfer::genesis(&mut config, vault_elf)?;
    let cluster = LocalCluster::new(&mut config, SocketAddrSpace::Unspecified);
    let mut nodes = cluster.validators.iter();
    let (key, first) = nodes.next().context("capture node missing")?;
    let first = first.validator.as_ref().context("capture node offline")?;
    let peer = nodes
        .next()
        .context("peer missing")?
        .1
        .validator
        .as_ref()
        .context("peer offline")?;
    let rpc = cluster.build_rpc_client_with_commitment(key, CommitmentConfig::processed())?;
    let start = Instant::now();
    println!(
        "validators={VALIDATORS} boot_ms={}",
        begin.elapsed().as_millis()
    );
    // These two fields configure receipt discovery and are not consensus data.
    // Use the local token authority as the (unused) vault selector for a plain transfer.
    let vault = transfer
        .deposit
        .as_ref()
        .map(|d| d.program)
        .unwrap_or(transfer.authority.pubkey());
    let genesis = cluster.genesis_config.hash().to_bytes();
    let route = if let Some(deposit) = &transfer.deposit {
        let route = deposit.binding(&genesis)?;
        let tx = Transaction::new_signed_with_payer(
            &deposit.initialize(cluster.funding_keypair.pubkey(), route),
            Some(&cluster.funding_keypair.pubkey()),
            &[&cluster.funding_keypair, &deposit.deployment],
            rpc.get_latest_blockhash()?,
        );
        let sig = rpc.send_transaction(&tx)?;
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if let Some(result) = rpc.get_signature_status(&sig)? {
                result?;
                break;
            }
            ensure!(Instant::now() < deadline, "vault initialization timeout");
            thread::sleep(Duration::from_millis(100));
        }
        println!(
            "initialize={sig} route={} binding={} genesis={}",
            crate::relay_deposit::ROUTE_ID,
            hex::encode(route),
            hex::encode(genesis)
        );
        route
    } else {
        genesis
    };
    let mut submitted = HashMap::<Signature, Hash>::new();
    let mut candidates = BTreeMap::new();
    let mut prev_hash = None;
    let mut next_footer = first.bank_forks.read().unwrap().root().max(1);
    loop {
        ensure!(
            start.elapsed() < Duration::from_secs(150),
            "capture timed out; next footer={next_footer}, submitted={}, captured={}",
            submitted.len(),
            candidates.len()
        );
        let (hash, root) = {
            let forks = first.bank_forks.read().unwrap();
            for (slot, bank) in forks.frozen_banks() {
                let Some(block_id) = bank.block_id() else {
                    continue;
                };
                if requested.is_some_and(|wanted| wanted != slot)
                    || candidates.contains_key(&(slot, block_id))
                {
                    continue;
                }
                let Some(parent) = bank.parent() else {
                    continue;
                };
                if parent.block_id().is_none() {
                    continue;
                }
                let landed = submitted
                    .keys()
                    .find(|sig| bank.get_signature_status_slot(sig) == Some((slot, Ok(()))))
                    .copied();
                if landed.is_none() && requested.is_none() {
                    continue;
                }
                let before = transfer.balances(&parent)?;
                let after = transfer.balances(&bank)?;
                if landed.is_some() {
                    ensure!(
                        before.0 > after.0
                            && after.1.checked_sub(before.1) == Some(before.0 - after.0),
                        "transfer balance mismatch"
                    );
                }
                let captured = State::capture(&bank, &parent, &vault, &route)?;
                candidates.insert((slot, block_id), (captured, landed, before, after));
            }
            // RPC preflight may still be serving the parent of the newest working bank.
            (forks.root_bank().last_blockhash(), forks.root())
        };
        while next_footer <= root.min(peer.bank_forks.read().unwrap().root()) {
            let footer_slot = next_footer;
            next_footer += 1;
            if !first.blockstore.is_root(footer_slot) {
                continue;
            }
            let Some(cert) = footer_cert(&first.blockstore, footer_slot)? else {
                continue;
            };
            let Some((state, signature, before, after)) =
                candidates.get(&(cert.slot, cert.block_id))
            else {
                continue;
            };
            if !peer.blockstore.is_root(cert.slot) {
                continue;
            }
            let finalized = peer
                .blockstore
                .get_double_merkle_root(cert.slot, BlockLocation::Original)?
                .context("missing peer ID")?;
            if let Some(sig) = signature {
                let entries = peer.blockstore.get_slot_entries(cert.slot, 0)?;
                ensure!(
                    entries
                        .iter()
                        .flat_map(|entry| &entry.transactions)
                        .any(|tx| tx.signatures.first() == Some(sig)),
                    "transfer absent from rooted peer entries"
                );
            }
            witness::archive(dir, &first.blockstore, state, &cert, finalized)?;
            if let (Some(deposit), Some(sig)) = (&transfer.deposit, signature) {
                deposit.index(
                    dir,
                    sig,
                    submitted[sig].to_bytes(),
                    cert.slot,
                    finalized.to_bytes(),
                    transfer.authority.pubkey(),
                    genesis,
                )?;
            }
            if let Some(sig) = signature {
                println!(
                    "spl_transfer={sig} slot={} source={}->{} destination={}->{}",
                    cert.slot, before.0, after.0, before.1, after.1
                );
            }
            println!(
                "cert_footer_slot={footer_slot} peer_root={} capture_ms={}",
                peer.bank_forks.read().unwrap().root(),
                start.elapsed().as_millis()
            );
            return Ok(());
        }
        if let Some(slot) = requested
            && root > slot.saturating_add(64)
        {
            bail!("slot {slot} unavailable or has no matching footer certificate");
        }
        if prev_hash != Some(hash) {
            let tx = transfer.transaction(&cluster.funding_keypair, hash);
            match rpc.send_transaction(&tx) {
                Ok(sig) => {
                    submitted.insert(sig, hash);
                }
                Err(e) => eprintln!("RPC transfer submission: {e}"),
            }
            prev_hash = Some(hash);
        }
        thread::sleep(Duration::from_millis(10));
    }
}
