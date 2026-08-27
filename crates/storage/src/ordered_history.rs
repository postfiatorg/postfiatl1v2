use super::*;

const SUMMARY_SCHEMA: &str = "postfiat-ordered-history-summary-v2";
const BITMAP_SCHEMA: &str = "postfiat-ordered-history-bitmap-v1";
const SLOT_SCHEMA: &str = "postfiat-ordered-history-slot-v1";
const INDEX_LOCK_FILE: &str = ".ordered-history-index.mutation.lock";
const SLOT_COUNT: u64 = 1 << 22;
const PROBE_LIMIT: u64 = 64;
const BITMAP_BYTES: usize = (SLOT_COUNT as usize) / 8;
const MAX_BATCH_ID_BYTES: usize = 1024;

pub const ORDERED_HISTORY_COMMITMENT_SCHEMA: &str = "postfiat-ordered-history-v2";
pub const ORDERED_HISTORY_SUMMARY_FILE: &str = "ordered_history_summary.json";
pub const ORDERED_HISTORY_BITMAP_FILE: &str = "bitmap.json";
pub const ORDERED_HISTORY_INDEX_DIR_PREFIX: &str = "ordered_history_index_v1_";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderedHistoryCommitment {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub count: u64,
    pub accumulator: String,
}

impl OrderedHistoryCommitment {
    pub fn genesis(chain_id: &str, genesis_hash: &str, protocol_version: u32) -> io::Result<Self> {
        let payload = serde_json::to_vec(&(
            ORDERED_HISTORY_COMMITMENT_SCHEMA,
            chain_id,
            genesis_hash,
            protocol_version,
        ))
        .map_err(invalid_data)?;
        Ok(Self {
            schema: ORDERED_HISTORY_COMMITMENT_SCHEMA.to_owned(),
            chain_id: chain_id.to_owned(),
            genesis_hash: genesis_hash.to_owned(),
            protocol_version,
            count: 0,
            accumulator: to_hex(&legacy_checksum(
                b"postfiat.ordered-history.genesis.v2",
                &payload,
            )),
        })
    }

    pub fn append(&self, batch_id: &str) -> io::Result<Self> {
        validate_batch_id(batch_id)?;
        let count = self.count.checked_add(1).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "ordered history overflow")
        })?;
        let payload = serde_json::to_vec(&(
            ORDERED_HISTORY_COMMITMENT_SCHEMA,
            self.chain_id.as_str(),
            self.genesis_hash.as_str(),
            self.protocol_version,
            count,
            self.accumulator.as_str(),
            batch_id,
        ))
        .map_err(invalid_data)?;
        Ok(Self {
            schema: ORDERED_HISTORY_COMMITMENT_SCHEMA.to_owned(),
            chain_id: self.chain_id.clone(),
            genesis_hash: self.genesis_hash.clone(),
            protocol_version: self.protocol_version,
            count,
            accumulator: to_hex(&legacy_checksum(
                b"postfiat.ordered-history.append.v2",
                &payload,
            )),
        })
    }

    pub fn validate_domain(
        &self,
        chain_id: &str,
        genesis_hash: &str,
        protocol_version: u32,
    ) -> io::Result<()> {
        if self.schema != ORDERED_HISTORY_COMMITMENT_SCHEMA
            || self.chain_id != chain_id
            || self.genesis_hash != genesis_hash
            || self.protocol_version != protocol_version
            || from_hex(&self.accumulator).is_none_or(|bytes| bytes.len() != MAC_BYTES)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "ordered-history commitment domain is invalid",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderedHistoryIndexReport {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub finalized_height: u64,
    pub block_hash: String,
    pub state_root: String,
    pub record_count: u64,
    pub accumulator: String,
    pub generation: String,
    pub slot_count: u64,
    pub probe_limit: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Summary {
    schema: String,
    commitment: OrderedHistoryCommitment,
    finalized_height: u64,
    block_hash: String,
    state_root: String,
    generation: String,
    slot_count: u64,
    probe_limit: u64,
    last_batch_id: String,
    last_slot: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Bitmap {
    schema: String,
    chain_id: String,
    genesis_hash: String,
    protocol_version: u32,
    finalized_height: u64,
    block_hash: String,
    state_root: String,
    generation: String,
    slot_count: u64,
    probe_limit: u64,
    occupied_slots: u64,
    last_batch_id: String,
    last_slot: Option<u64>,
    previous_accumulator: String,
    accumulator: String,
    bitmap_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Slot {
    schema: String,
    chain_id: String,
    genesis_hash: String,
    protocol_version: u32,
    generation: String,
    slot: u64,
    batch_id: String,
    ordinal: u64,
    previous_accumulator: String,
    accumulator: String,
}

enum Lookup {
    Present(Slot),
    Absent(u64),
}

impl NodeStore {
    pub fn rebuild_ordered_history_index(&self) -> io::Result<OrderedHistoryIndexReport> {
        let _lock = acquire_mutation_lock(&self.data_dir, INDEX_LOCK_FILE)?;
        let batches = self.read_ordered_batches()?;
        self.rebuild_ordered_history_index_from_batches(&batches)
    }

    fn rebuild_ordered_history_index_from_batches(
        &self,
        batches: &[String],
    ) -> io::Result<OrderedHistoryIndexReport> {
        let context = self.jsonl_checkpoint_context()?;
        if context.chain_id.is_empty() || context.genesis_hash.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "ordered-history index requires initialized genesis",
            ));
        }
        if context.finalized_height != batches.len() as u64 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "ordered-history batch count {} does not match finalized height {}",
                    batches.len(),
                    context.finalized_height
                ),
            ));
        }
        let generation = generation(
            &context.chain_id,
            &context.genesis_hash,
            context.protocol_version,
        )?;
        let target_dir = self.data_dir.join(&generation);
        let build_dir = self.data_dir.join(format!(
            ".{generation}.build.{}.{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or_default()
        ));
        fs::create_dir_all(&build_dir)?;
        let mut bits = vec![0_u8; BITMAP_BYTES];
        let mut commitment = initial_commitment(
            &context.chain_id,
            &context.genesis_hash,
            context.protocol_version,
        )?;
        let mut last_batch_id = String::new();
        let mut last_slot = None;
        let mut previous_accumulator = commitment.accumulator.clone();

        let result = (|| -> io::Result<()> {
            for batch_id in batches {
                validate_batch_id(batch_id)?;
                let slot_number =
                    find_build_slot(self, &build_dir, &bits, &generation, &commitment, batch_id)?;
                previous_accumulator = commitment.accumulator.clone();
                let next = commitment.append(batch_id)?;
                let slot = Slot {
                    schema: SLOT_SCHEMA.to_owned(),
                    chain_id: commitment.chain_id.clone(),
                    genesis_hash: commitment.genesis_hash.clone(),
                    protocol_version: commitment.protocol_version,
                    generation: generation.clone(),
                    slot: slot_number,
                    batch_id: batch_id.clone(),
                    ordinal: next.count,
                    previous_accumulator: previous_accumulator.clone(),
                    accumulator: next.accumulator.clone(),
                };
                write_slot(self, &build_dir, &slot)?;
                set_bit(&mut bits, slot_number)?;
                commitment = next;
                last_batch_id = batch_id.clone();
                last_slot = Some(slot_number);
            }
            let bitmap = Bitmap {
                schema: BITMAP_SCHEMA.to_owned(),
                chain_id: context.chain_id.clone(),
                genesis_hash: context.genesis_hash.clone(),
                protocol_version: context.protocol_version,
                finalized_height: context.finalized_height,
                block_hash: context.block_hash.clone(),
                state_root: context.state_root.clone(),
                generation: generation.clone(),
                slot_count: SLOT_COUNT,
                probe_limit: PROBE_LIMIT,
                occupied_slots: commitment.count,
                last_batch_id: last_batch_id.clone(),
                last_slot,
                previous_accumulator,
                accumulator: commitment.accumulator.clone(),
                bitmap_hex: to_hex(&bits),
            };
            write_bitmap(self, build_dir.join(ORDERED_HISTORY_BITMAP_FILE), &bitmap)
        })();
        if let Err(error) = result {
            let _ = fs::remove_dir_all(&build_dir);
            return Err(error);
        }

        if target_dir.exists() {
            fs::remove_dir_all(&target_dir)?;
        }
        fs::rename(&build_dir, &target_dir)?;
        sync_parent_dir(&target_dir)?;
        let summary = Summary {
            schema: SUMMARY_SCHEMA.to_owned(),
            commitment,
            finalized_height: context.finalized_height,
            block_hash: context.block_hash,
            state_root: context.state_root,
            generation,
            slot_count: SLOT_COUNT,
            probe_limit: PROBE_LIMIT,
            last_batch_id,
            last_slot,
        };
        self.write_json(self.data_dir.join(ORDERED_HISTORY_SUMMARY_FILE), &summary)?;
        Ok(report(&summary))
    }

    pub fn ordered_history_commitment(&self) -> io::Result<OrderedHistoryCommitment> {
        let (summary, _, _) = self.load_ordered_history_index()?;
        Ok(summary.commitment)
    }

    pub fn ordered_batch_contains_indexed(&self, batch_id: &str) -> io::Result<bool> {
        validate_batch_id(batch_id)?;
        let (summary, bitmap, bits) = self.load_ordered_history_index()?;
        Ok(matches!(
            lookup(self, &summary, &bitmap, &bits, batch_id, false)?,
            Lookup::Present(_)
        ))
    }

    pub fn next_ordered_history_commitment(
        &self,
        batch_id: &str,
    ) -> io::Result<OrderedHistoryCommitment> {
        validate_batch_id(batch_id)?;
        let (summary, bitmap, bits) = self.load_ordered_history_index()?;
        match lookup(self, &summary, &bitmap, &bits, batch_id, false)? {
            Lookup::Present(_) => Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("ordered batch '{batch_id}' already exists"),
            )),
            Lookup::Absent(_) => summary.commitment.append(batch_id),
        }
    }

    pub fn append_ordered_history_index_record(
        &self,
        batch_id: &str,
    ) -> io::Result<OrderedHistoryCommitment> {
        validate_batch_id(batch_id)?;
        let _lock = acquire_mutation_lock(&self.data_dir, INDEX_LOCK_FILE)?;
        let mut summary = self.read_summary()?;
        let mut bitmap = self.read_bitmap(&summary.generation)?;
        validate_summary_bitmap(self, &summary, &bitmap, true)?;
        let mut bits = decode_bitmap(&bitmap)?;

        if bitmap.occupied_slots == summary.commitment.count.saturating_add(1) {
            if bitmap.last_batch_id != batch_id
                || bitmap.previous_accumulator != summary.commitment.accumulator
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "ordered-history bitmap has an unrelated pending append",
                ));
            }
            summary.commitment = OrderedHistoryCommitment {
                schema: ORDERED_HISTORY_COMMITMENT_SCHEMA.to_owned(),
                chain_id: summary.commitment.chain_id.clone(),
                genesis_hash: summary.commitment.genesis_hash.clone(),
                protocol_version: summary.commitment.protocol_version,
                count: bitmap.occupied_slots,
                accumulator: bitmap.accumulator.clone(),
            };
            summary.last_batch_id = bitmap.last_batch_id.clone();
            summary.last_slot = bitmap.last_slot;
            self.write_json(self.data_dir.join(ORDERED_HISTORY_SUMMARY_FILE), &summary)?;
            return Ok(summary.commitment);
        }
        if bitmap.occupied_slots != summary.commitment.count {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "ordered-history summary and bitmap counts disagree",
            ));
        }

        match lookup(self, &summary, &bitmap, &bits, batch_id, false)? {
            Lookup::Present(slot) => {
                if summary.last_batch_id == batch_id
                    && summary.last_slot == Some(slot.slot)
                    && slot.ordinal == summary.commitment.count
                    && slot.accumulator == summary.commitment.accumulator
                {
                    return Ok(summary.commitment);
                }
                Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!("ordered batch '{batch_id}' already exists out of order"),
                ))
            }
            Lookup::Absent(slot_number) => {
                let previous_accumulator = summary.commitment.accumulator.clone();
                let next = summary.commitment.append(batch_id)?;
                let slot = Slot {
                    schema: SLOT_SCHEMA.to_owned(),
                    chain_id: next.chain_id.clone(),
                    genesis_hash: next.genesis_hash.clone(),
                    protocol_version: next.protocol_version,
                    generation: summary.generation.clone(),
                    slot: slot_number,
                    batch_id: batch_id.to_owned(),
                    ordinal: next.count,
                    previous_accumulator: previous_accumulator.clone(),
                    accumulator: next.accumulator.clone(),
                };
                let generation_dir = self.data_dir.join(&summary.generation);
                let path = slot_path(&generation_dir, slot_number);
                if path.exists() {
                    let existing: Slot = self.read_json(path)?;
                    if existing != slot {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "ordered-history orphan slot conflicts with pending append",
                        ));
                    }
                } else {
                    write_slot(self, &generation_dir, &slot)?;
                }
                set_bit(&mut bits, slot_number)?;
                bitmap.occupied_slots = next.count;
                bitmap.last_batch_id = batch_id.to_owned();
                bitmap.last_slot = Some(slot_number);
                bitmap.previous_accumulator = previous_accumulator;
                bitmap.accumulator = next.accumulator.clone();
                bitmap.bitmap_hex = to_hex(&bits);
                write_bitmap(
                    self,
                    generation_dir.join(ORDERED_HISTORY_BITMAP_FILE),
                    &bitmap,
                )?;
                summary.commitment = next.clone();
                summary.last_batch_id = batch_id.to_owned();
                summary.last_slot = Some(slot_number);
                self.write_json(self.data_dir.join(ORDERED_HISTORY_SUMMARY_FILE), &summary)?;
                Ok(next)
            }
        }
    }

    pub fn bind_ordered_history_index_to_chain_tip(
        &self,
        tip: &ChainTipState,
        previous_block_hash: &str,
    ) -> io::Result<()> {
        let summary_path = self.data_dir.join(ORDERED_HISTORY_SUMMARY_FILE);
        if !summary_path.exists() {
            return Ok(());
        }
        let _lock = acquire_mutation_lock(&self.data_dir, INDEX_LOCK_FILE)?;
        let mut summary = self.read_summary_without_context()?;
        let mut bitmap = self.read_bitmap(&summary.generation)?;
        validate_static_domain(self, &summary, &bitmap, true)?;
        let context = self.jsonl_checkpoint_context()?;
        if context.finalized_height != tip.height
            || context.block_hash != tip.block_hash
            || context.state_root != tip.state_root
            || summary.commitment.count != tip.ordered_batch_count
            || bitmap.occupied_slots != tip.ordered_batch_count
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "cannot bind ordered-history index before matching chain tip",
            ));
        }
        let summary_ok = context_is_current(
            summary.finalized_height,
            &summary.block_hash,
            &summary.state_root,
            &context,
        ) || context_is_previous(
            summary.finalized_height,
            &summary.block_hash,
            &context,
            previous_block_hash,
        );
        let bitmap_ok = context_is_current(
            bitmap.finalized_height,
            &bitmap.block_hash,
            &bitmap.state_root,
            &context,
        ) || context_is_previous(
            bitmap.finalized_height,
            &bitmap.block_hash,
            &context,
            previous_block_hash,
        );
        if !summary_ok || !bitmap_ok {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "ordered-history index cannot be rebound from unrelated tip",
            ));
        }
        summary.finalized_height = context.finalized_height;
        summary.block_hash = context.block_hash.clone();
        summary.state_root = context.state_root.clone();
        bitmap.finalized_height = context.finalized_height;
        bitmap.block_hash = context.block_hash;
        bitmap.state_root = context.state_root;
        write_bitmap(
            self,
            self.data_dir
                .join(&summary.generation)
                .join(ORDERED_HISTORY_BITMAP_FILE),
            &bitmap,
        )?;
        self.write_json(summary_path, &summary)
    }

    fn load_ordered_history_index(&self) -> io::Result<(Summary, Bitmap, Vec<u8>)> {
        let summary = self.read_summary()?;
        let bitmap = self.read_bitmap(&summary.generation)?;
        validate_summary_bitmap(self, &summary, &bitmap, false)?;
        let bits = decode_bitmap(&bitmap)?;
        if count_bits(&bits) != summary.commitment.count {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "ordered-history bitmap cardinality does not match summary",
            ));
        }
        Ok((summary, bitmap, bits))
    }

    fn read_summary(&self) -> io::Result<Summary> {
        let summary = self.read_summary_without_context()?;
        let context = self.jsonl_checkpoint_context()?;
        if !context_is_current(
            summary.finalized_height,
            &summary.block_hash,
            &summary.state_root,
            &context,
        ) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "ordered-history summary does not match current chain tip",
            ));
        }
        Ok(summary)
    }

    fn read_summary_without_context(&self) -> io::Result<Summary> {
        let summary: Summary = self.read_json(self.data_dir.join(ORDERED_HISTORY_SUMMARY_FILE))?;
        validate_summary_static(self, &summary)?;
        Ok(summary)
    }

    fn read_bitmap(&self, generation: &str) -> io::Result<Bitmap> {
        if !valid_generation(generation) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "ordered-history generation is invalid",
            ));
        }
        let path = self
            .data_dir
            .join(generation)
            .join(ORDERED_HISTORY_BITMAP_FILE);
        let file_bytes = fs::metadata(&path)?.len();
        let bitmap: Bitmap = self.read_json(path)?;
        self.work_counters
            .ordered_index_bitmap_bytes_read
            .fetch_add(file_bytes, Ordering::Relaxed);
        if bitmap.schema != BITMAP_SCHEMA {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "ordered-history bitmap schema is invalid",
            ));
        }
        Ok(bitmap)
    }
}

fn initial_commitment(
    chain_id: &str,
    genesis_hash: &str,
    protocol_version: u32,
) -> io::Result<OrderedHistoryCommitment> {
    OrderedHistoryCommitment::genesis(chain_id, genesis_hash, protocol_version)
}

fn generation(chain_id: &str, genesis_hash: &str, protocol_version: u32) -> io::Result<String> {
    let payload =
        serde_json::to_vec(&(chain_id, genesis_hash, protocol_version)).map_err(invalid_data)?;
    Ok(format!(
        "{ORDERED_HISTORY_INDEX_DIR_PREFIX}{}",
        to_hex(&legacy_checksum(
            b"postfiat.ordered-history.index-generation.v1",
            &payload,
        ))
    ))
}

fn validate_summary_static(store: &NodeStore, summary: &Summary) -> io::Result<()> {
    let context = store.jsonl_checkpoint_context()?;
    summary.commitment.validate_domain(
        &context.chain_id,
        &context.genesis_hash,
        context.protocol_version,
    )?;
    let expected = generation(
        &context.chain_id,
        &context.genesis_hash,
        context.protocol_version,
    )?;
    if summary.schema != SUMMARY_SCHEMA
        || summary.generation != expected
        || summary.slot_count != SLOT_COUNT
        || summary.probe_limit != PROBE_LIMIT
        || !valid_generation(&summary.generation)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "ordered-history summary format is invalid",
        ));
    }
    Ok(())
}

fn validate_static_domain(
    store: &NodeStore,
    summary: &Summary,
    bitmap: &Bitmap,
    allow_pending: bool,
) -> io::Result<()> {
    validate_summary_static(store, summary)?;
    let same = bitmap.chain_id == summary.commitment.chain_id
        && bitmap.genesis_hash == summary.commitment.genesis_hash
        && bitmap.protocol_version == summary.commitment.protocol_version
        && bitmap.generation == summary.generation
        && bitmap.slot_count == summary.slot_count
        && bitmap.probe_limit == summary.probe_limit;
    let count_ok = bitmap.occupied_slots == summary.commitment.count
        || (allow_pending && bitmap.occupied_slots == summary.commitment.count.saturating_add(1));
    if !same || !count_ok {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "ordered-history summary and bitmap domains disagree",
        ));
    }
    if bitmap.occupied_slots == summary.commitment.count
        && (bitmap.accumulator != summary.commitment.accumulator
            || bitmap.last_batch_id != summary.last_batch_id
            || bitmap.last_slot != summary.last_slot)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "ordered-history summary and bitmap tails disagree",
        ));
    }
    Ok(())
}

fn validate_summary_bitmap(
    store: &NodeStore,
    summary: &Summary,
    bitmap: &Bitmap,
    allow_pending: bool,
) -> io::Result<()> {
    validate_static_domain(store, summary, bitmap, allow_pending)?;
    let context = store.jsonl_checkpoint_context()?;
    if !context_is_current(
        summary.finalized_height,
        &summary.block_hash,
        &summary.state_root,
        &context,
    ) || !context_is_current(
        bitmap.finalized_height,
        &bitmap.block_hash,
        &bitmap.state_root,
        &context,
    ) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "ordered-history index does not match current chain tip",
        ));
    }
    Ok(())
}

fn lookup(
    store: &NodeStore,
    summary: &Summary,
    bitmap: &Bitmap,
    bits: &[u8],
    batch_id: &str,
    allow_pending: bool,
) -> io::Result<Lookup> {
    let start = start_slot(batch_id);
    let directory = store.data_dir.join(&summary.generation);
    for probe in 0..PROBE_LIMIT {
        let number = (start + probe) % SLOT_COUNT;
        if !bit_is_set(bits, number)? {
            return Ok(Lookup::Absent(number));
        }
        let slot: Slot = store.read_json(slot_path(&directory, number))?;
        store
            .work_counters
            .ordered_index_slots_read
            .fetch_add(1, Ordering::Relaxed);
        validate_slot(summary, bitmap, &slot, allow_pending)?;
        if slot.batch_id == batch_id {
            return Ok(Lookup::Present(slot));
        }
    }
    Err(io::Error::other(
        "ordered-history index probe limit exhausted",
    ))
}

fn find_build_slot(
    store: &NodeStore,
    directory: &Path,
    bits: &[u8],
    generation: &str,
    commitment: &OrderedHistoryCommitment,
    batch_id: &str,
) -> io::Result<u64> {
    let start = start_slot(batch_id);
    for probe in 0..PROBE_LIMIT {
        let number = (start + probe) % SLOT_COUNT;
        if !bit_is_set(bits, number)? {
            return Ok(number);
        }
        let existing: Slot = store.read_json(slot_path(directory, number))?;
        if existing.schema != SLOT_SCHEMA
            || existing.generation != generation
            || existing.chain_id != commitment.chain_id
            || existing.genesis_hash != commitment.genesis_hash
            || existing.protocol_version != commitment.protocol_version
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "ordered-history build found invalid occupied slot",
            ));
        }
        if existing.batch_id == batch_id {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("duplicate ordered batch '{batch_id}' while rebuilding"),
            ));
        }
    }
    Err(io::Error::other(
        "ordered-history probe limit exhausted during rebuild",
    ))
}

fn validate_slot(
    summary: &Summary,
    bitmap: &Bitmap,
    slot: &Slot,
    allow_pending: bool,
) -> io::Result<()> {
    let max = if allow_pending {
        bitmap.occupied_slots
    } else {
        summary.commitment.count
    };
    if slot.schema != SLOT_SCHEMA
        || slot.chain_id != summary.commitment.chain_id
        || slot.genesis_hash != summary.commitment.genesis_hash
        || slot.protocol_version != summary.commitment.protocol_version
        || slot.generation != summary.generation
        || slot.slot >= SLOT_COUNT
        || slot.ordinal == 0
        || slot.ordinal > max
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "ordered-history slot domain is invalid",
        ));
    }
    Ok(())
}

fn write_bitmap(store: &NodeStore, path: PathBuf, bitmap: &Bitmap) -> io::Result<()> {
    store.write_json(path.clone(), bitmap)?;
    let file_bytes = fs::metadata(path)?.len();
    store
        .work_counters
        .ordered_index_bitmap_bytes_written
        .fetch_add(file_bytes, Ordering::Relaxed);
    Ok(())
}

fn write_slot(store: &NodeStore, directory: &Path, slot: &Slot) -> io::Result<()> {
    store.write_json(slot_path(directory, slot.slot), slot)?;
    store
        .work_counters
        .ordered_index_slots_written
        .fetch_add(1, Ordering::Relaxed);
    Ok(())
}

fn slot_path(directory: &Path, slot: u64) -> PathBuf {
    directory
        .join(format!("{:02x}", slot >> 16))
        .join(format!("{:04x}.json", slot & 0xffff))
}

fn start_slot(batch_id: &str) -> u64 {
    let digest = legacy_checksum(
        b"postfiat.ordered-history.index-key.v1",
        batch_id.as_bytes(),
    );
    u64::from_be_bytes(digest[..8].try_into().expect("digest prefix")) % SLOT_COUNT
}

fn validate_batch_id(batch_id: &str) -> io::Result<()> {
    if batch_id.is_empty()
        || batch_id.len() > MAX_BATCH_ID_BYTES
        || batch_id.chars().any(char::is_control)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "ordered-history batch id is invalid",
        ));
    }
    Ok(())
}

fn decode_bitmap(bitmap: &Bitmap) -> io::Result<Vec<u8>> {
    let bytes = from_hex(&bitmap.bitmap_hex)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "bitmap is not hex"))?;
    if bytes.len() != BITMAP_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "ordered-history bitmap length is invalid",
        ));
    }
    Ok(bytes)
}

fn set_bit(bits: &mut [u8], slot: u64) -> io::Result<()> {
    let index = usize::try_from(slot / 8)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "bitmap index overflow"))?;
    let bit = u8::try_from(slot % 8)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "bitmap bit overflow"))?;
    let byte = bits
        .get_mut(index)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "bitmap slot out of bounds"))?;
    *byte |= 1_u8 << bit;
    Ok(())
}

fn bit_is_set(bits: &[u8], slot: u64) -> io::Result<bool> {
    let index = usize::try_from(slot / 8)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "bitmap index overflow"))?;
    let bit = u8::try_from(slot % 8)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "bitmap bit overflow"))?;
    let byte = bits
        .get(index)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "bitmap slot out of bounds"))?;
    Ok(byte & (1_u8 << bit) != 0)
}

fn count_bits(bits: &[u8]) -> u64 {
    bits.iter().map(|byte| u64::from(byte.count_ones())).sum()
}

fn valid_generation(value: &str) -> bool {
    value
        .strip_prefix(ORDERED_HISTORY_INDEX_DIR_PREFIX)
        .is_some_and(|digest| {
            digest.len() == MAC_BYTES * 2 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
        })
}

fn context_is_current(
    height: u64,
    block_hash: &str,
    state_root: &str,
    context: &JsonlCheckpointContext,
) -> bool {
    height == context.finalized_height
        && block_hash == context.block_hash
        && state_root == context.state_root
}

fn context_is_previous(
    height: u64,
    block_hash: &str,
    context: &JsonlCheckpointContext,
    previous_block_hash: &str,
) -> bool {
    height.saturating_add(1) == context.finalized_height && block_hash == previous_block_hash
}

fn report(summary: &Summary) -> OrderedHistoryIndexReport {
    OrderedHistoryIndexReport {
        schema: "postfiat-ordered-history-index-report-v1".to_owned(),
        chain_id: summary.commitment.chain_id.clone(),
        genesis_hash: summary.commitment.genesis_hash.clone(),
        protocol_version: summary.commitment.protocol_version,
        finalized_height: summary.finalized_height,
        block_hash: summary.block_hash.clone(),
        state_root: summary.state_root.clone(),
        record_count: summary.commitment.count,
        accumulator: summary.commitment.accumulator.clone(),
        generation: summary.generation.clone(),
        slot_count: summary.slot_count,
        probe_limit: summary.probe_limit,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn initialized_store(name: &str, batches: &[String]) -> (PathBuf, NodeStore) {
        let dir = std::env::temp_dir().join(format!(
            "{name}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let store = NodeStore::new(&dir);
        let genesis = Genesis::new("postfiat-ordered-history-test");
        store
            .init(&genesis, &NodeState::initialized("validator-0"))
            .expect("init store");
        store
            .write_ordered_batches(batches)
            .expect("write ordered batches");
        let genesis_json = genesis.to_json().expect("genesis JSON");
        let genesis_hash = to_hex(&legacy_checksum(
            b"postfiat.genesis.v1",
            genesis_json.as_bytes(),
        ));
        let tip = ChainTipState {
            schema: "postfiat-chain-tip-v1".to_owned(),
            chain_id: genesis.chain_id,
            genesis_hash: genesis_hash.clone(),
            protocol_version: genesis.protocol_version,
            height: batches.len() as u64,
            block_hash: if batches.is_empty() {
                genesis_hash
            } else {
                format!("block-{}", batches.len())
            },
            state_root: format!("state-{}", batches.len()),
            ordered_batch_count: batches.len() as u64,
            receipt_count: 0,
            history_base_height: 0,
        };
        store.write_chain_tip(&tip).expect("write chain tip");
        (dir, store)
    }

    #[test]
    fn rebuild_is_deterministic_and_membership_is_exact() {
        let batches = (0..128)
            .map(|index| format!("batch-{index:04}"))
            .collect::<Vec<_>>();
        let (dir, store) = initialized_store("postfiat-ordered-history-index-test", &batches);
        let first = store.rebuild_ordered_history_index().expect("first build");
        assert_eq!(first.record_count, 128);
        for batch in &batches {
            assert!(store
                .ordered_batch_contains_indexed(batch)
                .expect("membership"));
        }
        assert!(!store
            .ordered_batch_contains_indexed("batch-absent")
            .expect("absence"));
        let commitment = store.ordered_history_commitment().expect("commitment");
        let second = store.rebuild_ordered_history_index().expect("second build");
        assert_eq!(first.accumulator, second.accumulator);
        assert_eq!(
            commitment,
            store
                .ordered_history_commitment()
                .expect("second commitment")
        );
        fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn append_is_idempotent_and_advances_once() {
        let (dir, store) = initialized_store(
            "postfiat-ordered-history-append-test",
            &["batch-0".to_owned()],
        );
        store.rebuild_ordered_history_index().expect("build");
        store.reset_work_counters();
        let next = store
            .append_ordered_history_index_record("batch-1")
            .expect("append");
        assert_eq!(next.count, 2);
        let work = store.work_counters();
        assert!(work.ordered_index_bitmap_bytes_read > 0);
        assert!(work.ordered_index_bitmap_bytes_read < 2 * 1024 * 1024);
        assert!(work.ordered_index_bitmap_bytes_written > 0);
        assert!(work.ordered_index_bitmap_bytes_written < 2 * 1024 * 1024);
        assert!(work.ordered_index_slots_read <= PROBE_LIMIT);
        assert_eq!(work.ordered_index_slots_written, 1);
        assert_eq!(work.legacy_prefix_bytes_read, 0);
        assert_eq!(work.legacy_prefix_records_verified, 0);
        assert_eq!(
            store
                .append_ordered_history_index_record("batch-1")
                .expect("idempotent"),
            next
        );
        assert!(store
            .ordered_batch_contains_indexed("batch-1")
            .expect("membership"));
        fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn missing_occupied_slot_fails_closed() {
        let (dir, store) = initialized_store(
            "postfiat-ordered-history-slot-test",
            &["batch-0".to_owned()],
        );
        store.rebuild_ordered_history_index().expect("build");
        let summary = store.read_summary().expect("summary");
        fs::remove_file(slot_path(
            &dir.join(&summary.generation),
            summary.last_slot.expect("slot"),
        ))
        .expect("remove slot");
        let error = store
            .ordered_batch_contains_indexed("batch-0")
            .expect_err("missing slot must fail");
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    #[ignore = "manual height-scaling evidence; creates 6,650 authenticated index slots"]
    fn indexed_work_is_bounded_through_height_5000() {
        for height in [50_usize, 100, 500, 1_000, 5_000] {
            let batches = (0..height)
                .map(|index| format!("batch-{index:04}"))
                .collect::<Vec<_>>();
            let (dir, store) = initialized_store(
                &format!("postfiat-ordered-history-height-{height}"),
                &batches,
            );
            let rebuild_start = std::time::Instant::now();
            store.rebuild_ordered_history_index().expect("rebuild");
            let rebuild_ms = rebuild_start.elapsed().as_secs_f64() * 1_000.0;
            store.reset_work_counters();
            let candidate = format!("batch-{height:04}");
            let proposal_start = std::time::Instant::now();
            let next = store
                .next_ordered_history_commitment(&candidate)
                .expect("next commitment");
            let proposal_ms = proposal_start.elapsed().as_secs_f64() * 1_000.0;
            assert_eq!(next.count, height as u64 + 1);
            let append_start = std::time::Instant::now();
            store
                .append_ordered_history_index_record(&candidate)
                .expect("append commitment");
            let append_ms = append_start.elapsed().as_secs_f64() * 1_000.0;
            let work = store.work_counters();
            assert_eq!(work.legacy_prefix_bytes_read, 0);
            assert_eq!(work.legacy_prefix_records_verified, 0);
            assert!(work.ordered_index_bitmap_bytes_read < 4 * 1024 * 1024);
            assert!(work.ordered_index_bitmap_bytes_written < 2 * 1024 * 1024);
            assert!(work.ordered_index_slots_read <= PROBE_LIMIT * 2);
            assert_eq!(work.ordered_index_slots_written, 1);
            println!(
                "{}",
                serde_json::json!({
                    "height": height,
                    "rebuild_ms": rebuild_ms,
                    "proposal_lookup_and_accumulator_ms": proposal_ms,
                    "index_append_ms": append_ms,
                    "work": work,
                })
            );
            fs::remove_dir_all(dir).expect("cleanup");
        }
    }

    #[test]
    fn stale_context_rejects_then_explicit_bind_recovers() {
        let (dir, store) = initialized_store(
            "postfiat-ordered-history-stale-test",
            &["batch-0".to_owned()],
        );
        store.rebuild_ordered_history_index().expect("build");
        let mut tip = store.read_chain_tip().expect("tip");
        store
            .append_ordered_history_index_record("batch-1")
            .expect("append pending record");
        tip.height += 1;
        tip.block_hash = "block-2".to_owned();
        tip.state_root = "state-2".to_owned();
        tip.ordered_batch_count += 1;
        store.write_chain_tip(&tip).expect("move tip");
        assert!(store.ordered_history_commitment().is_err());
        store
            .bind_ordered_history_index_to_chain_tip(&tip, "block-1")
            .expect("bind");
        assert_eq!(store.ordered_history_commitment().expect("bound").count, 2);
        fs::remove_dir_all(dir).expect("cleanup");
    }
}
