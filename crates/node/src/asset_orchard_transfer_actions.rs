// Wallet-side operations for `asset_orchard_transfer_v1`: show a shielded address, build a
// private payment or exit from the wallet's notes, and find the wallet's notes in the pool.
// Included into the crate root next to the other pool operations.

pub const ASSET_ORCHARD_TRANSFER_FILE_SCHEMA: &str = "postfiat-asset-orchard-transfer-file-v1";
pub const ASSET_ORCHARD_TRANSFER_REPORT_SCHEMA: &str = "postfiat-asset-orchard-transfer-report-v1";
pub const ASSET_ORCHARD_TRANSFER_SCAN_REPORT_SCHEMA: &str = "postfiat-asset-orchard-transfer-scan-report-v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetOrchardTransferFile {
    pub schema: String,
    pub payload: AssetOrchardTransferActionPayload,
}

#[derive(Debug, Clone)]
pub struct AssetOrchardTransferCreateOptions {
    pub data_dir: PathBuf,
    /// Up to two notes of the same asset that the wallet owns. Empty when only shielding.
    pub note_files: Vec<PathBuf>,
    /// Shield a public amount: the account key that burns it, the asset, the amount, and the
    /// burn fee (0 picks the minimum).
    pub shield: Option<AssetOrchardTransferShieldOptions>,
    /// The wallet's shielded seed. Change returns to its address.
    pub change_seed_hex: String,
    /// A shielded address (`pfs1...`) and the amount it receives.
    pub pay: Option<(String, u64)>,
    /// A public account and the amount it receives. The asset is the notes' asset.
    pub exit: Option<(String, u64)>,
    pub transfer_file: PathBuf,
    /// Written only when the change is more than zero.
    pub change_note_file: PathBuf,
    pub overwrite: bool,
}

#[derive(Debug, Clone)]
pub struct AssetOrchardTransferShieldOptions {
    pub key_file: PathBuf,
    pub asset_id: String,
    pub amount: u64,
    pub fee: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetOrchardTransferReport {
    pub schema: String,
    pub transfer_file: String,
    pub change_note_file: Option<String>,
    pub change_amount: u64,
    pub anchor: String,
    pub nullifiers: Vec<String>,
    pub output_commitments: Vec<String>,
    pub proof_bytes: usize,
    pub build_seconds: f64,
    pub verified: bool,
    /// What an observer of the chain learns from this action.
    pub public: String,
}

/// The shielded address of a wallet seed, as text to hand to a payer.
pub fn asset_orchard_shielded_address_for_seed(seed_hex: &str) -> io::Result<String> {
    AssetOrchardShieldedKeys::from_seed(seed_hex)
        .and_then(|keys| keys.address.encode())
        .map_err(invalid_data)
}

pub fn create_asset_orchard_transfer(
    options: AssetOrchardTransferCreateOptions,
) -> io::Result<AssetOrchardTransferReport> {
    let started = std::time::Instant::now();
    ensure_output_can_be_written(
        &options.transfer_file,
        options.overwrite,
        "AssetOrchard transfer file",
    )?;
    ensure_output_can_be_written(
        &options.change_note_file,
        options.overwrite,
        "AssetOrchard change note file",
    )?;
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let shielded = store.read_shielded()?;
    let pool_output_commitments = match shielded
        .orchard
        .as_ref()
        .filter(|pool| pool.pool_id == ASSET_ORCHARD_POOL_ID_V1)
    {
        Some(pool) => pool.output_commitments.clone(),
        None if options.shield.is_some() => Vec::new(),
        None => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "the asset pool is empty; shield an amount before sending privately",
            ))
        }
    };
    let (entry, burn_transaction) = match &options.shield {
        Some(shield) => {
            let burn = asset_orchard_transfer_shield_burn(&store, &genesis, shield)?;
            let entry = AssetOrchardTransferEntry {
                from: burn.unsigned.source.clone(),
                asset_id: shield.asset_id.clone(),
                amount: shield.amount,
                burn_transaction_id: asset_transaction_tx_id(&burn),
            };
            (Some(entry), Some(burn))
        }
        None => (None, None),
    };
    let notes = options
        .note_files
        .iter()
        .map(|file| read_json_file::<AssetOrchardWalletNote>(file, "AssetOrchard transfer note"))
        .collect::<io::Result<Vec<_>>>()?;
    let genesis_hash_32 =
        asset_orchard_domain_genesis_hash(&genesis_hash(&genesis)).map_err(invalid_data)?;
    let payment = match &options.pay {
        Some((address, amount)) => Some((
            AssetOrchardShieldedAddress::parse(address).map_err(invalid_data)?,
            *amount,
        )),
        None => None,
    };
    let exit = match (&options.exit, notes.first()) {
        (Some((to, amount)), Some(note)) => Some(AssetOrchardTransferExit {
            to: to.clone(),
            asset_id: note.asset_id.clone(),
            amount: *amount,
        }),
        (Some(_), None) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "an exit spends a note; name the note to take the amount from",
            ))
        }
        (None, _) => None,
    };
    let built = build_asset_orchard_transfer_action(&AssetOrchardTransferRequest {
        chain_id: &genesis.chain_id,
        genesis_hash: genesis_hash_32,
        protocol_version: genesis.protocol_version,
        input_notes: &notes,
        entry,
        change_seed_hex: &options.change_seed_hex,
        payment,
        exit,
        pool_output_commitments: &pool_output_commitments,
    })
    .map_err(invalid_data)?;
    verify_asset_orchard_transfer_action(
        &genesis.chain_id,
        genesis_hash_32,
        genesis.protocol_version,
        &built.action,
    )
    .map_err(invalid_data)?;

    let file = AssetOrchardTransferFile {
        schema: ASSET_ORCHARD_TRANSFER_FILE_SCHEMA.to_string(),
        payload: AssetOrchardTransferActionPayload {
            transfer_json: serde_json::to_string(&built.action).map_err(invalid_data)?,
            burn_transaction,
        },
    };
    let json = serde_json::to_string_pretty(&file).map_err(invalid_data)?;
    atomic_write(&options.transfer_file, format!("{json}\n"))?;
    let change_amount = built.change_note.as_ref().map_or(0, |note| note.value);
    if let Some(note) = &built.change_note {
        write_asset_orchard_wallet_note_file(&options.change_note_file, note)?;
    }
    Ok(AssetOrchardTransferReport {
        schema: ASSET_ORCHARD_TRANSFER_REPORT_SCHEMA.to_string(),
        transfer_file: options.transfer_file.display().to_string(),
        change_note_file: built
            .change_note
            .as_ref()
            .map(|_| options.change_note_file.display().to_string()),
        change_amount,
        anchor: built.action.anchor.as_hex().to_string(),
        nullifiers: built
            .action
            .nullifiers
            .iter()
            .map(|nullifier| nullifier.as_hex().to_string())
            .collect(),
        output_commitments: built
            .action
            .output_commitments
            .iter()
            .map(|commitment| commitment.as_hex().to_string())
            .collect(),
        proof_bytes: built.action.proof.byte_len(),
        build_seconds: started.elapsed().as_secs_f64(),
        verified: true,
        public: match (&built.action.entry, &built.action.exit) {
            (Some(entry), _) => format!(
                "that {} shielded {} of asset {}; where it goes next is hidden",
                entry.from, entry.amount, entry.asset_id
            ),
            (_, Some(exit)) => format!(
                "an exit of {} of asset {} to {}; the notes spent and the change are hidden",
                exit.amount, exit.asset_id, exit.to
            ),
            _ => "that a transfer happened; sender, recipient, asset and amount are hidden"
                .to_string(),
        },
    })
}

/// Sign the burn of the public asset that funds a shield, at the account's next sequence.
fn asset_orchard_transfer_shield_burn(
    store: &NodeStore,
    genesis: &Genesis,
    shield: &AssetOrchardTransferShieldOptions,
) -> io::Result<SignedAssetTransaction> {
    if shield.amount == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "the amount to shield must be more than zero",
        ));
    }
    let ledger = store.read_ledger()?;
    let mempool = store.read_mempool()?;
    let key_file = read_key_file(&shield.key_file)?;
    let source = key_file.address.clone();
    let source_account = ledger.account(&source).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("source account `{source}` not found"),
        )
    })?;
    let sequence = next_pending_sender_sequence(&mempool, &source, source_account.sequence)?;
    let block_height = next_block_height_from_chain_tip(store, genesis)?;
    let asset_execution_compatibility = asset_execution_compatibility_from_store(store)?;
    let quote_ledger = ledger_after_executable_mempool(
        genesis,
        ledger,
        &mempool,
        block_height,
        asset_execution_compatibility,
    )?;
    let asset = quote_ledger
        .asset_definition(&shield.asset_id)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("asset `{}` not found", shield.asset_id),
            )
        })?;
    let operation = AssetTransactionOperation::AssetBurn(AssetBurnOperation {
        owner: source.clone(),
        issuer: asset.issuer.clone(),
        asset_id: shield.asset_id.clone(),
        amount: shield.amount,
    });
    let mut fee = shield.fee;
    loop {
        let quoted =
            quote_signed_asset_transaction(genesis, source.clone(), fee, sequence, operation.clone())?;
        let minimum_fee = minimum_asset_transaction_fee(&quoted)
            .saturating_add(asset_transaction_state_expansion_fee(&quote_ledger, &quoted));
        if fee >= minimum_fee {
            break;
        }
        if shield.fee != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("the minimum burn fee to shield is {minimum_fee}"),
            ));
        }
        fee = minimum_fee;
    }
    wallet_sign_asset_transaction(WalletSignAssetTransactionOptions {
        key_file: shield.key_file.clone(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(genesis),
        protocol_version: genesis.protocol_version,
        fee,
        sequence,
        expected_source: Some(source),
        operation,
    })
}

#[derive(Debug, Clone)]
pub struct AssetOrchardTransferBatchOptions {
    pub data_dir: PathBuf,
    pub transfer_file: PathBuf,
    pub batch_file: PathBuf,
}

pub fn create_asset_orchard_transfer_batch(
    options: AssetOrchardTransferBatchOptions,
) -> io::Result<ShieldedActionBatch> {
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let file: AssetOrchardTransferFile =
        read_json_file(&options.transfer_file, "AssetOrchard transfer file")?;
    if file.schema != ASSET_ORCHARD_TRANSFER_FILE_SCHEMA {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unsupported AssetOrchard transfer file schema `{}`", file.schema),
        ));
    }
    let batch = build_shielded_action_batch(
        &genesis,
        vec![ShieldedAction::AssetOrchardTransferV1(file.payload)],
    )?;
    write_shielded_action_batch_file(&options.batch_file, &batch)?;
    Ok(batch)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetOrchardScannedNote {
    pub output_commitment: String,
    pub asset_id: String,
    pub value: u64,
    pub spent: bool,
}

#[derive(Debug, Clone)]
pub struct AssetOrchardTransferScanReport {
    pub schema: String,
    pub address: String,
    pub pool_outputs_scanned: usize,
    /// Every transfer output addressed to this wallet, with whether it has been spent.
    pub notes: Vec<AssetOrchardScannedNote>,
    /// The spendable notes themselves. They hold the wallet's keys; keep them private.
    pub unspent: Vec<AssetOrchardWalletNote>,
}

/// Find the transfer outputs in the pool that belong to a wallet seed. This reads only what
/// the chain publishes for every output: its commitment and its ciphertext.
pub fn scan_asset_orchard_transfer_notes(
    data_dir: &Path,
    seed_hex: &str,
) -> io::Result<AssetOrchardTransferScanReport> {
    let store = NodeStore::new(data_dir);
    let genesis = store.read_genesis()?;
    let shielded = store.read_shielded()?;
    let keys = AssetOrchardShieldedKeys::from_seed(seed_hex).map_err(invalid_data)?;
    let address = keys.address.encode().map_err(invalid_data)?;
    let genesis_hash_32 =
        asset_orchard_domain_genesis_hash(&genesis_hash(&genesis)).map_err(invalid_data)?;
    let mut report = AssetOrchardTransferScanReport {
        schema: ASSET_ORCHARD_TRANSFER_SCAN_REPORT_SCHEMA.to_string(),
        address,
        pool_outputs_scanned: 0,
        notes: Vec::new(),
        unspent: Vec::new(),
    };
    let Some(pool) = shielded
        .orchard
        .as_ref()
        .filter(|pool| pool.pool_id == ASSET_ORCHARD_POOL_ID_V1)
    else {
        return Ok(report);
    };
    for output in &pool.asset_orchard_outputs {
        report.pool_outputs_scanned += 1;
        let Ok(ciphertext) = hex_to_bytes(&output.encrypted_output) else {
            continue;
        };
        let Some(note) = decrypt_asset_orchard_transfer_output(
            &genesis.chain_id,
            genesis_hash_32,
            genesis.protocol_version,
            &keys,
            &output.output_commitment,
            &ciphertext,
        )
        .map_err(invalid_data)?
        else {
            continue;
        };
        let nullifier = asset_orchard_transfer_note_nullifier(
            &genesis.chain_id,
            genesis_hash_32,
            genesis.protocol_version,
            &note,
        )
        .map_err(invalid_data)?;
        let spent = pool.is_nullified(nullifier.as_hex());
        report.notes.push(AssetOrchardScannedNote {
            output_commitment: output.output_commitment.clone(),
            asset_id: note.asset_id.clone(),
            value: note.value,
            spent,
        });
        if !spent {
            report.unspent.push(note);
        }
    }
    Ok(report)
}
