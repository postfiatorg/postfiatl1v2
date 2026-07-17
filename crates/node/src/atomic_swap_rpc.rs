use super::*;

pub const ATOMIC_SWAP_FEE_QUOTE_SCHEMA: &str = "postfiat-atomic-swap-fee-quote-v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtomicSwapFeeQuoteErrorDetail {
    code: String,
    message: String,
}

impl AtomicSwapFeeQuoteErrorDetail {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn code(&self) -> &str {
        &self.code
    }
}

impl std::fmt::Display for AtomicSwapFeeQuoteErrorDetail {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for AtomicSwapFeeQuoteErrorDetail {}

pub fn atomic_swap_fee_quote_typed_error_code(error: &io::Error) -> Option<&str> {
    error
        .get_ref()?
        .downcast_ref::<AtomicSwapFeeQuoteErrorDetail>()
        .map(AtomicSwapFeeQuoteErrorDetail::code)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtomicSwapQuoteLegInput {
    pub owner: String,
    pub recipient: String,
    pub issuer: String,
    pub asset_id: String,
    pub amount: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtomicSwapFeeQuoteOptions {
    pub data_dir: PathBuf,
    pub rfq_hash: String,
    pub market_envelope_hash: String,
    pub nav_epoch: u64,
    pub expires_at_height: u64,
    pub swap_nonce: String,
    pub leg_0: AtomicSwapQuoteLegInput,
    pub leg_1: AtomicSwapQuoteLegInput,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtomicSwapLegFeeQuoteReport {
    pub owner: String,
    pub sender_balance: u64,
    pub sender_sequence: u64,
    pub sequence: u64,
    pub mempool_pending_for_owner: u64,
    pub base_atomic_swap_fee: u64,
    pub state_expansion_fee: u64,
    pub minimum_fee: u64,
    pub sender_balance_after_fee: Option<u64>,
    pub sender_meets_reserve_after_fee: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtomicSwapFeeQuoteReport {
    pub schema: String,
    pub transaction_kind: String,
    pub parent_height: u64,
    pub parent_hash: String,
    pub parent_state_root: String,
    pub quote_height: u64,
    pub account_reserve: u64,
    pub transfer_fee_byte_quantum: u64,
    pub transfer_fee_per_quantum: u64,
    pub atomic_swap_weight_bytes: u64,
    pub leg_0: AtomicSwapLegFeeQuoteReport,
    pub leg_1: AtomicSwapLegFeeQuoteReport,
    pub unsigned_transaction: UnsignedAtomicSwapTransaction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedAtomicSwapTransactionJsonSubmitOptions {
    pub data_dir: PathBuf,
    pub signed_atomic_swap_transaction_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtomicSwapTargetBatchOptions {
    pub data_dir: PathBuf,
    pub batch_file: PathBuf,
    pub tx_id: String,
}

fn atomic_swap_quote_placeholder(
    unsigned: UnsignedAtomicSwapTransaction,
) -> SignedAtomicSwapTransaction {
    let placeholder_public_key = "00".repeat(ML_DSA_65_PUBLIC_KEY_BYTES);
    let placeholder_signature = "00".repeat(ML_DSA_65_SIGNATURE_BYTES);
    SignedAtomicSwapTransaction {
        authorization_0: AtomicSwapAuthorization {
            owner: unsigned.leg_0.owner.clone(),
            algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            public_key_hex: placeholder_public_key.clone(),
            signature_hex: placeholder_signature.clone(),
        },
        authorization_1: AtomicSwapAuthorization {
            owner: unsigned.leg_1.owner.clone(),
            algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            public_key_hex: placeholder_public_key,
            signature_hex: placeholder_signature,
        },
        unsigned,
    }
}

fn validate_atomic_swap_quote_asset(
    ledger: &LedgerState,
    leg_name: &str,
    leg: &AtomicSwapQuoteLegInput,
) -> io::Result<()> {
    let asset = ledger.asset_definition(&leg.asset_id).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("{leg_name} asset `{}` not found", leg.asset_id),
        )
    })?;
    if asset.issuer != leg.issuer {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{leg_name} issuer does not match the asset definition"),
        ));
    }
    Ok(())
}

pub fn atomic_swap_fee_quote(
    options: AtomicSwapFeeQuoteOptions,
) -> io::Result<AtomicSwapFeeQuoteReport> {
    let store = NodeStore::new(&options.data_dir);
    let mut coherent_snapshot = None;
    for _ in 0..3 {
        let before = status(NodeOptions {
            data_dir: options.data_dir.clone(),
        })?;
        let genesis = store.read_genesis()?;
        let ledger = store.read_ledger()?;
        let mempool = store.read_mempool()?;
        let mempool_after = store.read_mempool()?;
        let after = status(NodeOptions {
            data_dir: options.data_dir.clone(),
        })?;
        if before.block_height == after.block_height
            && before.block_tip_hash == after.block_tip_hash
            && before.state_root == after.state_root
            && mempool == mempool_after
        {
            coherent_snapshot = Some((before, genesis, ledger, mempool));
            break;
        }
    }
    let (parent, genesis, ledger, mempool) = coherent_snapshot.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::WouldBlock,
            "atomic swap fee quote could not obtain a coherent ledger/mempool parent snapshot",
        )
    })?;
    let quote_height = parent.block_height.checked_add(1).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "atomic swap quote height overflow",
        )
    })?;
    let compatibility = asset_execution_compatibility_from_store(&store)?;
    if compatibility.atomic_swap_paused {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "atomic_swap_paused: atomic swap fee quotes are disabled by governance",
        ));
    }
    if !compatibility.atomic_swap_active(quote_height) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("atomic_swap_not_active: atomic swap is not active at height {quote_height}"),
        ));
    }
    if options.expires_at_height < quote_height {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "swap_expired: atomic swap expired at height {}, quote height is {quote_height}",
                options.expires_at_height
            ),
        ));
    }

    validate_atomic_swap_quote_asset(&ledger, "leg_0", &options.leg_0)?;
    validate_atomic_swap_quote_asset(&ledger, "leg_1", &options.leg_1)?;

    let owner_0 = ledger.account(&options.leg_0.owner).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("leg_0 owner account `{}` not found", options.leg_0.owner),
        )
    })?;
    let owner_1 = ledger.account(&options.leg_1.owner).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("leg_1 owner account `{}` not found", options.leg_1.owner),
        )
    })?;
    let owner_0_balance = owner_0.balance;
    let owner_0_sequence = owner_0.sequence;
    let owner_1_balance = owner_1.balance;
    let owner_1_sequence = owner_1.sequence;
    let pending_0 = mempool_pending_count_for_sender(&mempool, &options.leg_0.owner);
    let pending_1 = mempool_pending_count_for_sender(&mempool, &options.leg_1.owner);
    if pending_0 != 0 || pending_1 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "atomic swap quote requires both owners to have no pending transactions; leg_0={pending_0}, leg_1={pending_1}"
            ),
        ));
    }
    let sequence_0 = owner_0_sequence.checked_add(1).ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "leg_0 owner sequence overflow")
    })?;
    let sequence_1 = owner_1_sequence.checked_add(1).ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "leg_1 owner sequence overflow")
    })?;

    let unsigned = UnsignedAtomicSwapTransaction {
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        address_namespace: ADDRESS_NAMESPACE.to_string(),
        signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        rfq_hash: options.rfq_hash,
        market_envelope_hash: options.market_envelope_hash,
        nav_epoch: options.nav_epoch,
        expires_at_height: options.expires_at_height,
        swap_nonce: options.swap_nonce,
        leg_0: AtomicSwapLeg {
            owner: options.leg_0.owner,
            recipient: options.leg_0.recipient,
            issuer: options.leg_0.issuer,
            asset_id: options.leg_0.asset_id,
            amount: options.leg_0.amount,
            sequence: sequence_0,
            fee: MIN_TRANSFER_FEE,
        },
        leg_1: AtomicSwapLeg {
            owner: options.leg_1.owner,
            recipient: options.leg_1.recipient,
            issuer: options.leg_1.issuer,
            asset_id: options.leg_1.asset_id,
            amount: options.leg_1.amount,
            sequence: sequence_1,
            fee: MIN_TRANSFER_FEE,
        },
    };
    unsigned
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;

    let mut quote_transaction = atomic_swap_quote_placeholder(unsigned);
    validate_atomic_swap_market_binding(&ledger, &quote_transaction).map_err(
        |(code, message)| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                AtomicSwapFeeQuoteErrorDetail::new(code, message),
            )
        },
    )?;
    let mut stabilized = false;
    for _ in 0..32 {
        let base_fee = minimum_atomic_swap_fee(&quote_transaction);
        let minimum_0 = base_fee
            .checked_add(atomic_swap_leg_state_expansion_fee(
                &ledger,
                &quote_transaction.unsigned.leg_0,
            ))
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "leg_0 fee overflow"))?;
        let minimum_1 = base_fee
            .checked_add(atomic_swap_leg_state_expansion_fee(
                &ledger,
                &quote_transaction.unsigned.leg_1,
            ))
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "leg_1 fee overflow"))?;
        if quote_transaction.unsigned.leg_0.fee == minimum_0
            && quote_transaction.unsigned.leg_1.fee == minimum_1
        {
            stabilized = true;
            break;
        }
        quote_transaction.unsigned.leg_0.fee = minimum_0;
        quote_transaction.unsigned.leg_1.fee = minimum_1;
    }
    if !stabilized {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "atomic swap fee quote did not converge",
        ));
    }

    let base_fee = minimum_atomic_swap_fee(&quote_transaction);
    let state_expansion_0 =
        atomic_swap_leg_state_expansion_fee(&ledger, &quote_transaction.unsigned.leg_0);
    let state_expansion_1 =
        atomic_swap_leg_state_expansion_fee(&ledger, &quote_transaction.unsigned.leg_1);
    let minimum_0 = quote_transaction.unsigned.leg_0.fee;
    let minimum_1 = quote_transaction.unsigned.leg_1.fee;
    let balance_after_0 = owner_0_balance.checked_sub(minimum_0);
    let balance_after_1 = owner_1_balance.checked_sub(minimum_1);
    let reserve_0 =
        postfiat_execution::required_account_reserve(&quote_transaction.unsigned.leg_0.owner);
    let reserve_1 =
        postfiat_execution::required_account_reserve(&quote_transaction.unsigned.leg_1.owner);
    let weight = atomic_swap_transaction_weight_bytes(&quote_transaction);

    Ok(AtomicSwapFeeQuoteReport {
        schema: ATOMIC_SWAP_FEE_QUOTE_SCHEMA.to_string(),
        transaction_kind: postfiat_types::ATOMIC_SWAP_TRANSACTION_KIND.to_string(),
        parent_height: parent.block_height,
        parent_hash: parent.block_tip_hash,
        parent_state_root: parent.state_root,
        quote_height,
        account_reserve: ACCOUNT_RESERVE,
        transfer_fee_byte_quantum: TRANSFER_FEE_BYTE_QUANTUM as u64,
        transfer_fee_per_quantum: TRANSFER_FEE_PER_QUANTUM,
        atomic_swap_weight_bytes: u64::try_from(weight).unwrap_or(u64::MAX),
        leg_0: AtomicSwapLegFeeQuoteReport {
            owner: quote_transaction.unsigned.leg_0.owner.clone(),
            sender_balance: owner_0_balance,
            sender_sequence: owner_0_sequence,
            sequence: sequence_0,
            mempool_pending_for_owner: pending_0,
            base_atomic_swap_fee: base_fee,
            state_expansion_fee: state_expansion_0,
            minimum_fee: minimum_0,
            sender_balance_after_fee: balance_after_0,
            sender_meets_reserve_after_fee: balance_after_0
                .is_some_and(|balance| balance >= reserve_0),
        },
        leg_1: AtomicSwapLegFeeQuoteReport {
            owner: quote_transaction.unsigned.leg_1.owner.clone(),
            sender_balance: owner_1_balance,
            sender_sequence: owner_1_sequence,
            sequence: sequence_1,
            mempool_pending_for_owner: pending_1,
            base_atomic_swap_fee: base_fee,
            state_expansion_fee: state_expansion_1,
            minimum_fee: minimum_1,
            sender_balance_after_fee: balance_after_1,
            sender_meets_reserve_after_fee: balance_after_1
                .is_some_and(|balance| balance >= reserve_1),
        },
        unsigned_transaction: quote_transaction.unsigned,
    })
}

pub fn submit_signed_atomic_swap_transaction_json_to_mempool(
    options: SignedAtomicSwapTransactionJsonSubmitOptions,
) -> io::Result<MempoolAtomicSwapEntry> {
    if options.signed_atomic_swap_transaction_json.len() > MAX_SIGNED_TRANSFER_JSON_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "signed atomic swap transaction JSON exceeds {MAX_SIGNED_TRANSFER_JSON_BYTES} bytes"
            ),
        ));
    }
    let signed: SignedAtomicSwapTransaction =
        serde_json::from_str(&options.signed_atomic_swap_transaction_json).map_err(invalid_data)?;
    admit_signed_atomic_swap_to_mempool(&options.data_dir, signed)
}

pub fn create_atomic_swap_mempool_batch_for_tx_id(
    options: AtomicSwapTargetBatchOptions,
) -> io::Result<TransactionBatch> {
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let ledger = store.read_ledger()?;
    let mut mempool = store.read_mempool()?;
    enforce_mempool_state_limits(&mempool)?;
    let index = mempool
        .pending_atomic_swaps
        .iter()
        .position(|entry| entry.tx_id == options.tx_id)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("atomic swap `{}` is not pending", options.tx_id),
            )
        })?;
    let entry = mempool.pending_atomic_swaps[index].clone();
    let expected_tx_id = atomic_swap_transaction_tx_id(&entry.transaction);
    if expected_tx_id != entry.tx_id {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "pending atomic swap tx_id mismatch: stored `{}`, computed `{expected_tx_id}`",
                entry.tx_id
            ),
        ));
    }
    let block_height = next_block_height_from_chain_tip(&store, &genesis)?;
    let compatibility = asset_execution_compatibility_from_store(&store)?;
    let mut dry_run_ledger = ledger;
    let receipt = execute_atomic_swap_transaction_with_compatibility(
        &genesis,
        &mut dry_run_ledger,
        &entry.transaction,
        block_height,
        compatibility,
    );
    if !receipt.accepted {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "target atomic swap `{}` is not executable: {}: {}",
                entry.tx_id, receipt.code, receipt.message
            ),
        ));
    }

    let batch = build_mixed_transaction_batch_with_atomic_swaps(
        &mempool_batch_domain(&genesis),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![entry.transaction],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .map_err(invalid_data)?
    .batch;
    write_batch_file(&options.batch_file, &batch)?;
    mempool.pending_atomic_swaps.remove(index);
    store.write_mempool(&mempool)?;
    Ok(batch)
}
