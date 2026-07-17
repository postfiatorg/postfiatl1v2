use super::*;

pub fn verify_blocks(options: NodeOptions) -> io::Result<BlockVerificationReport> {
    let store = NodeStore::new(&options.data_dir);
    recover_ordered_commit_journal(&store)?;
    let genesis = store.read_genesis()?;
    let blocks = store.read_blocks()?;
    let ordered_batches = store.read_ordered_batches()?;
    let archive = store.read_batch_archive()?;
    let receipts = store.read_receipts()?;
    let governance = store.read_governance()?;
    let history_checkpoint = read_history_checkpoint_state_optional(&store)?;
    let history_base_height = history_checkpoint
        .as_ref()
        .map(|checkpoint| checkpoint.pruned_up_to_height)
        .unwrap_or(0);
    let history_base_ordered_batches = history_checkpoint
        .as_ref()
        .map(|checkpoint| checkpoint.ordered_batches.len())
        .unwrap_or(0);
    let live_validator_registry =
        read_validator_registry_file(&store.data_dir().join(VALIDATOR_REGISTRY_FILE))?;
    validate_validator_registry_for_count(
        &live_validator_registry,
        governance.active_validator_count,
    )?;
    let certificate_validator_registry = read_validator_registry_replay_base(&store)?;
    validate_validator_registry_for_count(
        &certificate_validator_registry,
        genesis.validator_count,
    )?;
    let mut persisted_receipt_counts = HashMap::<String, usize>::new();
    for receipt in &receipts {
        *persisted_receipt_counts
            .entry(receipt.tx_id.clone())
            .or_default() += 1;
    }
    let mut block_receipt_counts = HashMap::<String, usize>::new();
    let current_state_root = current_replicated_state_root(&store, &genesis)?;
    let mut parent_hash = history_checkpoint
        .as_ref()
        .map(|checkpoint| checkpoint.checkpoint_block_hash.clone())
        .unwrap_or_else(|| "genesis".to_string());
    let mut certificate_governance = history_checkpoint
        .as_ref()
        .map(|checkpoint| checkpoint.governance.clone())
        .unwrap_or_else(|| GovernanceState::new(genesis.validator_count));
    let mut certificate_validator_registry = history_checkpoint
        .as_ref()
        .map(|checkpoint| checkpoint.validator_registry.clone())
        .unwrap_or(certificate_validator_registry);
    let mut certificate_registry_update_ids = certificate_governance
        .validator_registry_updates
        .iter()
        .filter(|update| update.activation_height <= history_base_height)
        .map(|update| update.update_id.clone())
        .collect::<HashSet<_>>();

    if ordered_batches.len() != history_base_ordered_batches + blocks.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "ordered batch journal length {} does not match checkpoint prefix {} plus block count {}",
                ordered_batches.len(),
                history_base_ordered_batches,
                blocks.len()
            ),
        ));
    }

    let mut archive_keys = HashSet::new();
    for entry in &archive.batches {
        let key = (entry.batch_kind.clone(), entry.batch_id.clone());
        if !archive_keys.insert(key) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "duplicate archived batch {}:{}",
                    entry.batch_kind, entry.batch_id
                ),
            ));
        }
    }

    let mut block_archive_keys = HashSet::new();
    for (index, block) in blocks.blocks.iter().enumerate() {
        block_archive_keys.insert((
            block.header.batch_kind.clone(),
            block.header.batch_id.clone(),
        ));
        let expected_height = history_base_height
            .checked_add(index as u64)
            .and_then(|height| height.checked_add(1))
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "block height overflow"))?;
        if block.header.height != expected_height {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "block height mismatch at index {index}: expected {expected_height}, got {}",
                    block.header.height
                ),
            ));
        }
        let ordered_index =
            usize::try_from(block.header.height.saturating_sub(1)).map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidData, "block ordered index overflow")
            })?;
        if ordered_batches
            .get(ordered_index)
            .map(|batch_id| batch_id.as_str())
            != Some(block.header.batch_id.as_str())
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("block {} ordered batch id mismatch", block.header.height),
            ));
        }
        if block.header.parent_hash != parent_hash {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("block {} parent hash mismatch", block.header.height),
            ));
        }
        if block.header.receipt_count != block.receipt_ids.len() as u64 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("block {} receipt count mismatch", block.header.height),
            ));
        }
        for receipt_id in &block.receipt_ids {
            let referenced_count = block_receipt_counts
                .entry(receipt_id.clone())
                .and_modify(|count| *count += 1)
                .or_insert(1);
            let persisted_count = persisted_receipt_counts
                .get(receipt_id)
                .copied()
                .unwrap_or_default();
            if *referenced_count > persisted_count {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "block {} references missing receipt `{receipt_id}`",
                        block.header.height
                    ),
                ));
            }
        }
        activate_validator_registry_updates_for_height(
            &genesis,
            &mut certificate_validator_registry,
            &mut certificate_governance,
            &mut certificate_registry_update_ids,
            block.header.height,
        )?;
        let certificate_validators = active_validator_ids(&certificate_governance)?;
        backfill_legacy_validator_registry_records(
            &mut certificate_validator_registry,
            &live_validator_registry,
            &certificate_validators,
            &format!("block {} certificate replay", block.header.height),
        )?;
        verify_block_certificate_evidence(
            &genesis,
            block,
            &certificate_validator_registry,
            &certificate_validators,
        )?;
        let Some(archive_entry) = archive.find(&block.header.batch_kind, &block.header.batch_id)
        else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "block {} batch payload is not archived",
                    block.header.height
                ),
            ));
        };
        let payload_hash = batch_archive_payload_hash(
            &genesis,
            &archive_entry.batch_kind,
            &archive_entry.batch_id,
            &archive_entry.payload_json,
        )?;
        if archive_entry.payload_hash != payload_hash {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "block {} archived payload hash mismatch",
                    block.header.height
                ),
            ));
        }
        let block_evidence = BlockEvidence::from_block(block);
        let expected_hash = if consensus_v2_active_at(&genesis, block.header.height) {
            let commit = block.header.consensus_v2_commit.as_ref().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "block {} is missing activated consensus v2 commit",
                        block.header.height
                    ),
                )
            })?;
            let validators = certificate_validators
                .iter()
                .map(|validator_id| {
                    let record =
                        validator_registry_record(&certificate_validator_registry, validator_id)?;
                    Ok(postfiat_ordering_fast::ConsensusV2Validator {
                        validator_id: validator_id.clone(),
                        public_key_hex: record.public_key_hex.clone(),
                    })
                })
                .collect::<io::Result<Vec<_>>>()?;
            let validators = postfiat_ordering_fast::ConsensusV2ValidatorSet::try_new(validators)
                .map_err(invalid_data)?;
            let committee_epoch = 1u64
                .checked_add(
                    certificate_governance
                        .validator_registry_updates
                        .iter()
                        // A registry update recorded for height H is committed by
                        // the committee that was active at the start of H.  The
                        // replacement committee signs from H + 1 onward.  This
                        // must match `activate_validator_registry_updates_for_height`,
                        // which deliberately applies only updates strictly older
                        // than the block being replayed.
                        .filter(|update| update.activation_height < block.header.height)
                        .count() as u64,
                )
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "consensus v2 replay committee epoch overflow",
                    )
                })?;
            let domain = postfiat_ordering_fast::consensus_v2_domain(
                genesis.chain_id.clone(),
                genesis_hash(&genesis),
                genesis.protocol_version,
                committee_epoch,
                &validators,
            );
            let committed = postfiat_ordering_fast::verify_consensus_v2_commit(
                &domain,
                &validators,
                commit,
                &postfiat_ordering_fast::ConsensusV2QcGraph::default(),
            )
            .map_err(invalid_data)?;
            let expected_parent =
                if block.header.height == 1 && block.header.parent_hash == "genesis" {
                    consensus_v2_genesis_parent_id(&domain).map_err(invalid_data)?
                } else {
                    block.header.parent_hash.clone()
                };
            if committed.height != block.header.height
                || committed.parent_block_id != expected_parent
                || committed.payload_hash != payload_hash
                || committed.state_root != block.header.state_root
                || commit.proposal.proposer != block.header.proposer
                || commit.proposal.round.view != block.header.view
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "block {} consensus v2 commit does not match replayed block",
                        block.header.height
                    ),
                ));
            }
            committed.block_id
        } else {
            if block.header.consensus_v2_commit.is_some() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "block {} carries consensus v2 commit before activation",
                        block.header.height
                    ),
                ));
            }
            block_hash(&genesis, &block_evidence, &block.header.certificate_id)?
        };
        if block.header.block_hash != expected_hash {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("block {} hash mismatch", block.header.height),
            ));
        }
        verify_archived_payload(&genesis, block, archive_entry)?;
        update_governance_for_certificate_replay(
            &mut certificate_governance,
            block,
            archive_entry,
        )?;
        parent_hash = block.header.block_hash.clone();
    }

    for (batch_kind, batch_id) in archive_keys {
        if !block_archive_keys.contains(&(batch_kind.clone(), batch_id.clone())) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("archived batch {batch_kind}:{batch_id} has no matching block"),
            ));
        }
    }

    for (receipt_id, persisted_count) in persisted_receipt_counts {
        let block_count = block_receipt_counts
            .get(&receipt_id)
            .copied()
            .unwrap_or_default();
        if block_count != persisted_count {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "receipt `{receipt_id}` appears {persisted_count} time(s) in receipts but {block_count} time(s) in blocks"
                ),
            ));
        }
    }

    let replay_state_root = verify_replayed_blocks(&store, &genesis, &blocks, &archive, &receipts)?;

    if let Some(tip) = blocks.blocks.last() {
        if tip.header.state_root != current_state_root && replay_state_root != current_state_root {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "block tip state root {} does not match current state root {} or replay state root {}",
                    tip.header.state_root, current_state_root, replay_state_root
                ),
            ));
        }
    } else if replay_state_root != current_state_root {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "genesis state root {replay_state_root} does not match current state root {}",
                current_state_root
            ),
        ));
    }

    Ok(BlockVerificationReport {
        verified: true,
        block_count: blocks.len(),
        tip_hash: parent_hash,
        state_root: current_state_root,
    })
}

pub(super) fn verify_archived_payload(
    genesis: &Genesis,
    block: &BlockRecord,
    archive_entry: &BatchArchiveEntry,
) -> io::Result<()> {
    if archive_entry.batch_kind != block.header.batch_kind
        || archive_entry.batch_id != block.header.batch_id
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "block {} archived payload key does not match block header",
                block.header.height
            ),
        ));
    }
    match block.header.batch_kind.as_str() {
        "transparent" => {
            let batch: TransactionBatch = parse_archived_payload(block, archive_entry)?;
            verify_archived_transparent_batch_id(genesis, block, &batch)?;
            verify_archived_payload_receipt_count(block, batch.transaction_count() as u64)
        }
        "governance" => {
            let batch: GovernanceActionBatch = parse_archived_payload(block, archive_entry)?;
            verify_archived_governance_action_batch_id(genesis, &batch)?;
            verify_archived_payload_id(block, &batch.batch_id)?;
            verify_archived_payload_receipt_count(
                block,
                (batch.amendments.len()
                    + batch.validator_registry_updates.len()
                    + batch.governance_agent_dry_runs.len()
                    + batch.fastswap_bootstraps.len()
                    + batch.fastpay_recovery_bootstraps.len()
                    + batch.vault_bridge_route_profile_activations.len()) as u64,
            )
        }
        "shielded" => {
            let batch: ShieldedActionBatch = parse_archived_payload(block, archive_entry)?;
            verify_shielded_action_batch_id(genesis, &batch)?;
            verify_archived_payload_id(block, &batch.batch_id)?;
            verify_archived_payload_receipt_count(block, batch.actions.len() as u64)
        }
        "bridge" => {
            let batch: BridgeActionBatch = parse_archived_payload(block, archive_entry)?;
            verify_bridge_action_batch_id(genesis, &batch)?;
            verify_archived_payload_id(block, &batch.batch_id)?;
            verify_archived_payload_receipt_count(block, batch.actions.len() as u64)
        }
        other => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "block {} has unknown batch kind `{other}`",
                block.header.height
            ),
        )),
    }
}

pub(super) fn verify_archived_transparent_batch_id(
    genesis: &Genesis,
    block: &BlockRecord,
    batch: &TransactionBatch,
) -> io::Result<()> {
    verify_archived_payload_id(block, &batch.batch_id)?;
    let batch_domain = mempool_batch_domain(genesis);
    let reference = match reference_for_batch(&batch_domain, batch) {
        Ok(reference) => reference,
        Err(error)
            if archived_transparent_legacy_batch_id_allowed(genesis, block, batch, &error) =>
        {
            return Ok(());
        }
        Err(error) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "block {} archived transparent payload invalid: {error}",
                    block.header.height
                ),
            ));
        }
    };
    verify_batch_payload(&batch_domain, batch, &reference).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "block {} archived transparent payload unavailable: {error}",
                block.header.height
            ),
        )
    })?;
    verify_archived_payload_id(block, &reference.batch_id)
}

pub(super) fn archived_transparent_legacy_batch_id_allowed(
    genesis: &Genesis,
    block: &BlockRecord,
    batch: &TransactionBatch,
    error: &postfiat_mempool_dag::AvailabilityError,
) -> bool {
    // WAN devnet block 9 was produced before the transparent-batch reference
    // algorithm was tightened. The archive remains safe only if the old
    // payload self-id matches the certified block id; receipt/state replay
    // still has to reproduce the recorded chain state.
    genesis.chain_id == "postfiat-wan-devnet"
        && block.header.height <= 9
        && batch.batch_id == block.header.batch_id
        && error.to_string().contains("batch id mismatch")
}

pub(super) const WAN_DEVNET_LEGACY_NAV_PROFILE_ID_SCHEMA_MAX_HEIGHT: u64 = 8;
pub(super) const WAN_DEVNET_LEGACY_NAV_REPLAY_MAX_HEIGHT: u64 = 119;
pub(super) const WAN_DEVNET_LEGACY_CASH_OMITTED_SP1_NAV_MAX_HEIGHT: u64 = 338;
pub(super) const WAN_DEVNET_LEGACY_STRICT_DOMAIN_VALIDATION_HEIGHT: u64 = 73;
pub(super) const WAN_DEVNET_LEGACY_DOMAINLESS_WITHDRAWAL_BATCHES: &[(u64, &str)] = &[
    (
        46,
        "bae0de84b72e1b627304f08ae2338a2e09797f5336b560f68bd4a1be21702f9f2650a1b3ac4982e3c82c9bf5eee87561",
    ),
    (
        60,
        "f9eb69b4fd622076a58e3d1a4b9c896f189df47dcc584608e74fbd6f3f7d29ebc9fcaf93a1becb0390e8b51399164046",
    ),
];
// Immutable, certificate-bound pre-pricing Asset-Orchard swaps in the existing
// WAN devnet-2 archive.  Current live swaps never enter this compatibility
// path: both height and the canonical 48-byte batch digest must match.
pub(super) const WAN_DEVNET2_PRE_PRICING_SWAP_BATCHES: &[(u64, &str)] = &[
    (17, "ace64594dd02afedcab12a380d16a4b3e754ec14463f0daf971b9962c0bd9093c9b73f74bd81f4c735b328fee4d9c620"),
    (27, "533aa560bdbc65251d0ab43daa60f51a93349b8f60f566adfa3a809356e2a18bd2edaa8930324904e481b353bf2fccc7"),
    (53, "7008f642f7e72064837b2bb906826740769ac6f1342509e293d2d46f7dfe0109b05a5864e77234b8abbe993654585ff2"),
    (60, "405b4b1e3ae6c3b50b5700d1bb7a0f780f73573ccc8389515d3c4ccbaeff89982acbd41f3cc17d71c3b05e688d66e60d"),
    (67, "f079e4234442cda2ca735fb47682d758536ba0183eeb1850f1029e4053450338b8cc889db681bb54cc0a4f2b71a847b4"),
    (68, "ec063c3f220b343c608e0284a710bd2229819e3941645c32d23067213aaf8b1ab2144b014d9110d4a027ed38b1bedb77"),
    (81, "92ca66f9323b8ae676c3417acbb88945494328f24d12fa1e30c627b564d7840ee38dcd224f80ad318fb29e853a28f595"),
    (85, "6183df1fe3d751c70e20b02059f2d553984e31d44c9f76ff6538895d729d72687e54b34da7130540f92c51d80149e39a"),
    (120, "5ae44ddaf3830a375f1b1d0cf6e1ce607c37ba6517804b9207a4e21b9b0070c3997a284ea8be0ebf8f9c1d33f91fe8ad"),
    (124, "ccb966d2c47b9513ca8ad55d8af93faf355ed597c63b8530f3ad46e9fb2374c7792059da8a0ef43faad219c66b4a23c1"),
    (130, "81e31487e7513ea3297ccef7c2e24022b94e29f3f8766313f9699ca443dd6772c5b2214ca9793627aba1fcf193eec6e1"),
    (151, "0266ed5d6b216fb26793b22a535ffb283a2d063c0a675023107bac303567f8b76f0704fadaa039c1c54996585e237ecb"),
    (176, "f1a7293e4274832511266e6d0a7b5e8053223f86184828ec53cdb96decdce920cb5bdbdfb9d098f6a32bafdff125494b"),
    (194, "d1831688265bbd96410d8b86930ebe9bef0ed5811392708417721f994c112c717b038c3b6a34dd3c0464027922c746ca"),
    (222, "bb5960a9f5785b9ab2ccdbc36c7773708614545608a17a28a2bf862f986c64a3ec17c76b8940c0db8d2a624333ee6c6d"),
    (346, "f70f32f040c3178c9dd8676eff7d6722543604680c74c70af30af11baef8082f6566ebd8c214f8971984c394e76e080d"),
    (365, "300ba8d714b69d00f46b85b49056be71682a4508880119e0b5c64aac5c0696d43af3ac5757721ffab92273f5d5054b84"),
    (381, "1844558c740548c3b9114d18c98db48c719edaa4153d1150f7c842115d7ea1a753110902f0d1b0016f1f8438e42de8e8"),
    (397, "affb91f5409247273b070a90a2b448e7c8b5e62f153cbc22496f5bfdf0e3ecf63d2a059dba7774b02f92f8f4a294d3ff"),
    (411, "6513538cd737df18b98c78091292257f48364e58a84ed6ccf13bc96811fb729c1393e562ca9b01d1712dec7ad1e4c938"),
    (430, "b8052093d42d9bfbb567c068fc80dc782bbe81e4579f336ca1a44d925ddd7f018c2535c48f9b822189f309741d155bc5"),
    (446, "e007ab886278eccf00d78a85946b1b346226a7173d0e0a4f6b096918139af5bd46bab6665bb99f97671a7a496f71830c"),
    (462, "f62dc842d51f340621cbda9788949e034da5c02e84b54b55da62d00f608035f19dca94c990e98e1658a195d054712587"),
    (481, "52a05829a88b4ddc1f62ccf91bee694730814e6b1a92527e911c79199a75ec0c90b71857a58597abd9cd56d36e775596"),
    (497, "e55f2f94f4933f83e5be29d52f97aa048ae69d96510be04ae96df4f3024c0f24e9c45be68386f51426bf597241a920cb"),
    (557, "3069a1c7bdff661979a84eb651e75cf1c4671674676b3965f0eed1b45810aa63919388279d006b4ebf5496bb5d68a982"),
    (571, "95b8b10d9ba9ceae2427211cce62ae64aa9d677e515ff8e6a54541fccaa7b61bbaab4c0d297d4b9af03bf3569818ad8e"),
    (584, "f167225cf97a5ec929943750e622a39b8886a35991e1af856d813469443c1e9b18e766a739ed08a062cea233ceb742e4"),
    (588, "b8f27a67fd1967c81b358eb3b6e6fb1021f522d034577e87a0c1a6c8a910c027bb1783fa806aa9259508ae11a2edae88"),
    (592, "9708a26414ea3348e7ddcd71f369cd6e09bfc8f5d0b6219b4ed9e358233a184ac36bab5744f0b7be084376a1566bce7e"),
    (596, "b7ef49cab92da1bb59acf0becda2403cb2f2ea1a33688d293425aa08e6e0b2991a6ebc63686f0eb62bd6df5c2afa7eba"),
];

pub(super) const WAN_DEVNET2_PRE_REPIN_PRIVATE_EGRESS_BATCHES: &[(u64, &str)] = &[
    (82, "95cfdfc8d5d8f523709431209de3722be2daaa826180068d2eac5974e58355a21216e490a216bf681652bc6a77dacd07"),
    (86, "59738b56be1a970b37e297905e34870080fe8b2ac155117cf79ae2b52a3b5a24d494900c8d6d3a06869c564e15547227"),
    (121, "cf575779f9e6cd67b6fc24b2d98ed5f0ce0b154bad6fc2cd150e38e6672d69dbb522ff01e54bf638ed4f38bb3969febc"),
    (125, "6fa1b7a06990d7e245c99483cf0e33ac06f67d2f2ebf395fcba01ae045883a94f580b2519dd4d24546b818af033f73d3"),
    (131, "d6eab968fe3e3aa338869cc757f3673a073df35d92d1432d8e355422ddefea502cd204e4c07bd9331aad84552daf2787"),
    (152, "4820ebf8a4dd27f0592f612ecf97e737fae1fb4efc5d23367565fbbf404494936ce4d0f3f5037c3d94b62b36058221cf"),
    (177, "1334ac6fa81376aa9936c0fcdfd0f189c281607220d01e8747b36a34e2a8749c7ac1afb613857b7d14d4a391546ed934"),
    (195, "f9f02af0e085b0a769802e26baecf9eeed4960deb22cd772b6878216c86b6266217e61313daeeaaf2826514412af84a1"),
    (223, "0e7630f9caf4071b1d51af12dd13c29b7a3632d3d76f77e577e21c3e43ebb58bfc159e7e0c85986dbebbe151a9989645"),
    (347, "48dd0525256e586a2ee8344c0718ae14e6b7731004b21643d9eb621c218a6f7c64d0513d51fa8b8615ee6639d859db5b"),
    (366, "7e68e92cfeb6129cf299d28462af538ef2dc9252b47e6f42ad3407f14ad186db897ad6093ec70e4730dac5ce4c3395a0"),
    (382, "5b79be128dd9cf43cbd5f388e64ba889f54360d5f0a5e77e2e50b990afe935ae86d6119334369f542b26fa454c0cf8c0"),
    (398, "26308ad7148166724fc0ec43de4947dbf49522b2dd0d5e2f2462689e01dcb1d135be3428baa552a5e4c7245936a4e566"),
    (412, "d53958010ec8f893de952f3c2dcfc80a3cf1e04d65557a5976fd3eda3ecf8c6259feb500fe2e234fc8fce93f150f1720"),
    (431, "9c6b4355ecc64c6f7c819f2b3ab2e88ba54e9ea0cf9ff6e88a0cc2a4aac09cd94cc95ff85f2e14f65417451574159c3b"),
    (447, "32c46fa79bb9f1d843170009c88afaaa981ce97358d46cedb7bc4b8cfbc30210f0b0b31c4efbd764d8977c8736eb9438"),
    (463, "82e3eeaba40beedee749761c3031805d6e1ac04e35d30da071988c11f25ea07c33ee21ccc49882493e464fb6db15240e"),
    (482, "1f6bd89492bb4f4aec46b3de917bf54979147ebc1280c20517204117f22c795a4fb0e17572750d9424cad4272ef6e5ae"),
    (498, "24791386a52b8101a05e659496887e08f374761e6769351766ec0ac5420caf85f427e16c4cf5eab85967b1047a683862"),
    (542, "bc6daa09e53311d71bffe07be0041c15f80d38e491da840953188db92ff7b3ca65a0407aba66ea2d44f3b7fbcedf3122"),
    (543, "4136a4045193862c70c2cec65d32afcb034c317037ab0eea8dc4c78734e14e84ea45a267508fc12ac3d3aab198e76a1f"),
    (544, "24224bd3e97cf94c9f2063d6dc5cdfb5c4a0e355deeb554dce41fd03c1859aea19017762ebc7876ebad6f48c7313c173"),
    (558, "f1495066f01bf70d5511910cdced5a15ed01eba76a607281117a897d26d25e8d841917c7fa98bd2973f954afb902c052"),
    (572, "f0c16df5910caa969ee4aaf034bca57a6ea5c2a38239abf57091fccf2aa995d791f2bbfade6006024da05d0a69095040"),
    (585, "05448929ca506772612ff7b537de70aa716e7c0946a7f4e2dbecf12458374e83f561d01ba3d9739caa7b9b3b2cee5c2c"),
    (589, "e6d29f2ad2e1b16ec73d4d9faf40da3a6a71088b1d42fccb2d9d3bf33c8dbb677659340c7389982bf77e63389f07e6ef"),
    (593, "fb8a14773cd843138be13858ee3e0b0a7d040bd2d33ea7e856d97e0ba8c69bfb9381e55949baded1f2dc8921dae5fbdd"),
    (597, "6515ab3ccf4255246278f623719b8e3d8bd1bb75153a4ff0fb010fe452da0acaebd7530efcde59abd6899ae65daef50f"),
];

pub(super) fn archived_wan_devnet2_pre_pricing_swap_allowed(
    genesis: &Genesis,
    height: u64,
    batch_id: &str,
) -> bool {
    genesis.chain_id == "postfiat-wan-devnet-2"
        && WAN_DEVNET2_PRE_PRICING_SWAP_BATCHES
            .iter()
            .any(|(allowed_height, allowed_batch_id)| {
                height == *allowed_height && batch_id == *allowed_batch_id
            })
}

pub(super) fn archived_wan_devnet2_pre_repin_private_egress_allowed(
    genesis: &Genesis,
    height: u64,
    batch_id: &str,
) -> bool {
    genesis.chain_id == "postfiat-wan-devnet-2"
        && WAN_DEVNET2_PRE_REPIN_PRIVATE_EGRESS_BATCHES.iter().any(
            |(allowed_height, allowed_batch_id)| {
                height == *allowed_height && batch_id == *allowed_batch_id
            },
        )
}

pub(super) fn archived_wan_devnet_legacy_nav_profile_id_allowed(
    genesis: &Genesis,
    block: &BlockRecord,
) -> bool {
    genesis.chain_id == "postfiat-wan-devnet"
        && block.header.height <= WAN_DEVNET_LEGACY_NAV_REPLAY_MAX_HEIGHT
}

pub(super) fn archived_wan_devnet2_legacy_receipt_id_drift_allowed(
    genesis: &Genesis,
    block: &BlockRecord,
) -> bool {
    archived_wan_devnet2_pre_pricing_swap_allowed(
        genesis,
        block.header.height,
        &block.header.batch_id,
    ) || archived_wan_devnet2_pre_repin_private_egress_allowed(
        genesis,
        block.header.height,
        &block.header.batch_id,
    )
}

pub(super) fn archived_wan_devnet_legacy_nav_profile_id_schema_allowed(
    genesis: &Genesis,
    block: &BlockRecord,
) -> bool {
    genesis.chain_id == "postfiat-wan-devnet"
        && block.header.height <= WAN_DEVNET_LEGACY_NAV_PROFILE_ID_SCHEMA_MAX_HEIGHT
}

pub(super) fn archived_wan_devnet_legacy_strict_domain_validation_allowed(
    genesis: &Genesis,
    block: &BlockRecord,
) -> bool {
    genesis.chain_id == "postfiat-wan-devnet"
        && block.header.height == WAN_DEVNET_LEGACY_STRICT_DOMAIN_VALIDATION_HEIGHT
}

pub(super) fn archived_wan_devnet_legacy_cash_omitted_sp1_nav_allowed(
    genesis: &Genesis,
    block: &BlockRecord,
    transaction: &SignedAssetTransaction,
) -> bool {
    genesis.chain_id == "postfiat-wan-devnet"
        && block.header.height <= WAN_DEVNET_LEGACY_CASH_OMITTED_SP1_NAV_MAX_HEIGHT
        && matches!(
            &transaction.unsigned.operation,
            AssetTransactionOperation::NavReserveSubmit(operation)
                if !operation.sp1_public_values.is_empty()
        )
}

pub(super) fn archived_wan_devnet_legacy_domainless_withdrawal_packet_emit_allowed(
    genesis: &Genesis,
    block: &BlockRecord,
    transaction: &SignedAssetTransaction,
) -> bool {
    genesis.chain_id == "postfiat-wan-devnet"
        && matches!(
            &transaction.unsigned.operation,
            AssetTransactionOperation::VaultBridgeBurnToRedeem(_)
        )
        && WAN_DEVNET_LEGACY_DOMAINLESS_WITHDRAWAL_BATCHES
            .iter()
            .any(|(height, batch_id)| {
                block.header.height == *height && block.header.batch_id == *batch_id
            })
}

pub(super) fn legacy_nav_profile_id_without_empty_sp1_fields(
    operation: &NavProfileRegisterOperation,
) -> String {
    let mut preimage = format!(
        "verifier_kind={}\nsource_class={}\nmax_snapshot_age_blocks={}\nchallenge_window_blocks={}\nmax_epoch_gap_blocks={}\nsettle_deadline_blocks={}\nmin_challenge_bond={}\nmin_attestations={}\ntolerance_bp={}\nvaluation_policy_hash={}\n",
        operation.verifier_kind,
        operation.effective_source_class(),
        operation.max_snapshot_age_blocks,
        operation.challenge_window_blocks,
        operation.max_epoch_gap_blocks,
        operation.settle_deadline_blocks,
        operation.min_challenge_bond,
        operation.min_attestations,
        operation.tolerance_bp,
        operation.valuation_policy_hash,
    );
    if !operation.sp1_program_vkey.is_empty() {
        preimage.push_str(&format!(
            "sp1_program_vkey={}\n",
            operation.sp1_program_vkey
        ));
    }
    if !operation.sp1_proof_encoding.is_empty() {
        preimage.push_str(&format!(
            "sp1_proof_encoding={}\n",
            operation.sp1_proof_encoding
        ));
    }
    if operation.max_proof_bytes != 0 {
        preimage.push_str(&format!("max_proof_bytes={}\n", operation.max_proof_bytes));
    }
    if operation.max_public_values_bytes != 0 {
        preimage.push_str(&format!(
            "max_public_values_bytes={}\n",
            operation.max_public_values_bytes
        ));
    }

    let mut hasher = sha3::Sha3_384::new();
    hasher.update(NAV_PROFILE_ID_DOMAIN.as_bytes());
    hasher.update([0u8]);
    hasher.update(preimage.as_bytes());
    bytes_to_hex(&hasher.finalize())
}

pub(super) fn legacy_nav_profile_register_signing_byte_candidates(
    transaction: &SignedAssetTransaction,
) -> Vec<Vec<u8>> {
    let AssetTransactionOperation::NavProfileRegister(operation) = &transaction.unsigned.operation
    else {
        return Vec::new();
    };
    let has_sp1_fields = !operation.sp1_program_vkey.is_empty()
        || !operation.sp1_proof_encoding.is_empty()
        || operation.max_proof_bytes != 0
        || operation.max_public_values_bytes != 0;
    if !has_sp1_fields {
        return vec![legacy_nav_profile_register_signing_bytes(
            transaction,
            true,
            false,
            false,
        )];
    }

    vec![
        legacy_nav_profile_register_signing_bytes(transaction, false, false, false),
        legacy_nav_profile_register_signing_bytes(transaction, true, false, false),
        legacy_nav_profile_register_signing_bytes(transaction, false, true, true),
        legacy_nav_profile_register_signing_bytes(transaction, true, true, true),
    ]
}

pub(super) fn legacy_nav_profile_register_signing_bytes(
    transaction: &SignedAssetTransaction,
    include_operation_tag: bool,
    include_sp1_fields: bool,
    include_zero_sp1_limits: bool,
) -> Vec<u8> {
    let AssetTransactionOperation::NavProfileRegister(operation) = &transaction.unsigned.operation
    else {
        return Vec::new();
    };
    let mut bytes = legacy_asset_transaction_unsigned_prefix(&transaction.unsigned);
    if include_operation_tag {
        bytes.extend_from_slice(
            format!(
                "operation={}\n",
                transaction.unsigned.operation.transaction_kind()
            )
            .as_bytes(),
        );
    }
    bytes.extend_from_slice(
        format!(
            "registrant={}\nverifier_kind={}\nsource_class={}\nmax_snapshot_age_blocks={}\nchallenge_window_blocks={}\nmax_epoch_gap_blocks={}\nsettle_deadline_blocks={}\nmin_challenge_bond={}\nmin_attestations={}\ntolerance_bp={}\nvaluation_policy_hash={}\n",
            operation.registrant,
            operation.verifier_kind,
            operation.source_class,
            operation.max_snapshot_age_blocks,
            operation.challenge_window_blocks,
            operation.max_epoch_gap_blocks,
            operation.settle_deadline_blocks,
            operation.min_challenge_bond,
            operation.min_attestations,
            operation.tolerance_bp,
            operation.valuation_policy_hash,
        )
        .as_bytes(),
    );
    if include_sp1_fields {
        if !operation.sp1_program_vkey.is_empty() {
            bytes.extend_from_slice(
                format!("sp1_program_vkey={}\n", operation.sp1_program_vkey).as_bytes(),
            );
        }
        if !operation.sp1_proof_encoding.is_empty() {
            bytes.extend_from_slice(
                format!("sp1_proof_encoding={}\n", operation.sp1_proof_encoding).as_bytes(),
            );
        }
        if include_zero_sp1_limits || operation.max_proof_bytes != 0 {
            bytes.extend_from_slice(
                format!("max_proof_bytes={}\n", operation.max_proof_bytes).as_bytes(),
            );
        }
        if include_zero_sp1_limits || operation.max_public_values_bytes != 0 {
            bytes.extend_from_slice(
                format!(
                    "max_public_values_bytes={}\n",
                    operation.max_public_values_bytes
                )
                .as_bytes(),
            );
        }
    }
    bytes
}

pub(super) fn legacy_nav_reserve_submit_signing_bytes_without_sp1_evidence_fields(
    transaction: &SignedAssetTransaction,
) -> Option<Vec<u8>> {
    let AssetTransactionOperation::NavReserveSubmit(operation) = &transaction.unsigned.operation
    else {
        return None;
    };
    if !operation.sp1_proof_bytes.is_empty() || !operation.sp1_public_values.is_empty() {
        return None;
    }
    let unsigned = &transaction.unsigned;
    let mut bytes = legacy_asset_transaction_unsigned_prefix(unsigned);
    bytes.extend_from_slice(
        format!(
            "operation={}\nissuer={}\nsubmitter={}\nasset_id={}\nepoch={}\nnav_per_unit={}\ncirculating_supply={}\nverified_net_assets={}\nproof_profile={}\nsource_root={}\nattestor_root={}\nreserve_packet_hash={}\nreserve_accounts={}\n",
            transaction.unsigned.operation.transaction_kind(),
            operation.issuer,
            operation.submitter,
            operation.asset_id,
            operation.epoch,
            operation.nav_per_unit,
            operation.circulating_supply,
            operation.verified_net_assets,
            operation.proof_profile,
            operation.source_root,
            operation.attestor_root,
            operation.reserve_packet_hash,
            operation.reserve_accounts.join(","),
        )
        .as_bytes(),
    );
    Some(bytes)
}

pub(super) fn legacy_asset_transaction_unsigned_prefix(
    unsigned: &UnsignedAssetTransaction,
) -> Vec<u8> {
    format!(
        "postfiat.asset_transaction.v1\nchain_id={}\ngenesis_hash={}\nprotocol_version={}\naddress_namespace={}\ntransaction_kind={}\nsignature_algorithm_id={}\nsource={}\nfee={}\nsequence={}\n",
        unsigned.chain_id,
        unsigned.genesis_hash,
        unsigned.protocol_version,
        unsigned.address_namespace,
        unsigned.transaction_kind,
        unsigned.signature_algorithm_id,
        unsigned.source,
        unsigned.fee,
        unsigned.sequence,
    )
    .into_bytes()
}

pub(super) fn verified_legacy_wan_asset_transaction_signing_bytes(
    transaction: &SignedAssetTransaction,
) -> Option<Vec<u8>> {
    let public_key = hex_to_bytes(&transaction.public_key_hex).ok()?;
    let signature = hex_to_bytes(&transaction.signature_hex).ok()?;
    let mut candidates = legacy_nav_profile_register_signing_byte_candidates(transaction);
    if let Some(reserve_submit) =
        legacy_nav_reserve_submit_signing_bytes_without_sp1_evidence_fields(transaction)
    {
        candidates.push(reserve_submit);
    }
    candidates
        .into_iter()
        .find(|signing_bytes| ml_dsa_65_verify(&public_key, signing_bytes, &signature))
}

pub(super) fn replay_legacy_wan_devnet_nav_profile_ids(
    genesis: &Genesis,
    block: &BlockRecord,
    ledger: &mut LedgerState,
    batch: &TransactionBatch,
) -> io::Result<()> {
    if !archived_wan_devnet_legacy_nav_profile_id_schema_allowed(genesis, block) {
        return Ok(());
    }
    for transaction in &batch.asset_transactions {
        let AssetTransactionOperation::NavProfileRegister(operation) =
            &transaction.unsigned.operation
        else {
            continue;
        };
        if !operation.sp1_program_vkey.is_empty()
            || !operation.sp1_proof_encoding.is_empty()
            || operation.max_proof_bytes != 0
            || operation.max_public_values_bytes != 0
        {
            continue;
        }
        let current_profile_id = nav_proof_profile_id(
            &operation.verifier_kind,
            operation.effective_source_class(),
            operation.max_snapshot_age_blocks,
            operation.challenge_window_blocks,
            operation.max_epoch_gap_blocks,
            operation.settle_deadline_blocks,
            operation.min_challenge_bond,
            operation.min_attestations,
            operation.tolerance_bp,
            &operation.valuation_policy_hash,
            &operation.sp1_program_vkey,
            &operation.sp1_proof_encoding,
            operation.max_proof_bytes,
            operation.max_public_values_bytes,
        )
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        let legacy_profile_id = legacy_nav_profile_id_without_empty_sp1_fields(operation);
        if legacy_profile_id == current_profile_id {
            continue;
        }
        if ledger.nav_proof_profile(&legacy_profile_id).is_some() {
            continue;
        }
        let Some(profile) = ledger
            .nav_proof_profiles
            .iter_mut()
            .find(|profile| profile.profile_id == current_profile_id)
        else {
            let present_profile_ids = ledger
                .nav_proof_profiles
                .iter()
                .map(|profile| profile.profile_id.as_str())
                .collect::<Vec<_>>();
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "block {} legacy WAN profile replay could not find current profile id {current_profile_id}; present profile ids: {:?}",
                    block.header.height
                    , present_profile_ids
                ),
            ));
        };
        profile.profile_id = legacy_profile_id;
    }
    Ok(())
}

pub(super) fn parse_archived_payload<T: DeserializeOwned>(
    block: &BlockRecord,
    archive_entry: &BatchArchiveEntry,
) -> io::Result<T> {
    serde_json::from_str(&archive_entry.payload_json).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "block {} archived {} payload parse failed: {error}",
                block.header.height, block.header.batch_kind
            ),
        )
    })
}

pub(super) fn update_governance_for_certificate_replay(
    governance: &mut GovernanceState,
    block: &BlockRecord,
    archive_entry: &BatchArchiveEntry,
) -> io::Result<()> {
    if block.header.batch_kind != "governance" {
        return Ok(());
    }
    let batch: GovernanceActionBatch = parse_archived_payload(block, archive_entry)?;
    let _ = execute_governance_batch(governance, None, &batch, block.header.height);
    Ok(())
}

pub(super) fn activate_validator_registry_updates_for_height(
    genesis: &Genesis,
    registry: &mut ValidatorRegistry,
    governance: &mut GovernanceState,
    applied_update_ids: &mut HashSet<String>,
    block_height: u64,
) -> io::Result<()> {
    let due_updates = governance
        .validator_registry_updates
        .iter()
        .filter(|update| {
            update.activation_height < block_height
                && !applied_update_ids.contains(&update.update_id)
        })
        .cloned()
        .collect::<Vec<_>>();
    for update in due_updates {
        let context = format!(
            "block {} replay update `{}`",
            block_height, update.update_id
        );
        let new_validators = apply_historical_validator_registry_update_to_registry(
            genesis,
            registry,
            &update,
            block_height,
            &context,
        )?;
        if update.operation != VALIDATOR_REGISTRY_OP_ROTATE_KEY {
            set_active_validator_ids(governance, new_validators)?;
        }
        applied_update_ids.insert(update.update_id);
    }
    Ok(())
}

pub(super) fn backfill_legacy_validator_registry_records(
    registry: &mut ValidatorRegistry,
    live_registry: &ValidatorRegistry,
    validators: &[String],
    context: &str,
) -> io::Result<()> {
    for node_id in validators {
        if registry
            .validators
            .iter()
            .any(|record| record.node_id == *node_id)
        {
            continue;
        }
        let record = validator_registry_record(live_registry, node_id).map_err(|error| {
            io::Error::new(
                error.kind(),
                format!("{context} missing legacy validator registry key `{node_id}`"),
            )
        })?;
        registry.validators.push(record.clone());
    }
    sort_validator_registry_records(&mut registry.validators);
    validate_validator_registry(registry)
}

pub(super) fn live_validator_registry_after_due_updates(
    store: &NodeStore,
    genesis: &Genesis,
    governance: &GovernanceState,
    block_height: u64,
) -> io::Result<Option<ValidatorRegistry>> {
    let mut registry =
        read_validator_registry_file(&store.data_dir().join(VALIDATOR_REGISTRY_FILE))?;
    let mut changed = false;
    for update in &governance.validator_registry_updates {
        if update.activation_height > block_height {
            continue;
        }
        let new_validators = validator_registry_update_new_validators(update);
        if !validator_registry_update_can_live_apply(update, governance)? {
            continue;
        }
        if validator_registry_root(&registry, &new_validators)
            .map(|current_new_root| current_new_root == update.new_registry_root)
            .unwrap_or(false)
        {
            continue;
        }
        apply_verified_validator_registry_update_to_registry(
            genesis,
            &mut registry,
            update,
            block_height,
            "live validator registry activation",
        )?;
        changed = true;
    }
    Ok(changed.then_some(registry))
}

pub(super) struct DueValidatorRegistryActivations {
    pub(super) registry: Option<ValidatorRegistry>,
    pub(super) governance_changed: bool,
}

pub(super) fn governance_with_due_validator_registry_activations(
    store: &NodeStore,
    genesis: &Genesis,
    block_height: u64,
) -> io::Result<GovernanceState> {
    let mut governance = store.read_governance()?;
    let _ = activate_due_validator_registry_updates_for_commit(
        store,
        genesis,
        &mut governance,
        block_height,
    )?;
    Ok(governance)
}

pub(super) fn activate_due_validator_registry_updates_for_commit(
    store: &NodeStore,
    genesis: &Genesis,
    governance: &mut GovernanceState,
    block_height: u64,
) -> io::Result<DueValidatorRegistryActivations> {
    let mut registry =
        read_validator_registry_file(&store.data_dir().join(VALIDATOR_REGISTRY_FILE))?;
    let mut registry_changed = false;
    let mut governance_changed = false;
    for update in &governance.validator_registry_updates.clone() {
        if update.activation_height >= block_height
            || update.operation == VALIDATOR_REGISTRY_OP_ROTATE_KEY
        {
            continue;
        }
        let new_validators = validator_registry_update_new_validators(update);
        let already_live = validator_registry_root(&registry, &new_validators)
            .map(|current_new_root| current_new_root == update.new_registry_root)
            .unwrap_or(false);
        if !already_live {
            apply_verified_validator_registry_update_to_registry(
                genesis,
                &mut registry,
                update,
                block_height,
                "ordered validator registry activation",
            )?;
            registry_changed = true;
        }
        let previous = active_validator_ids(governance)?;
        set_active_validator_ids(governance, new_validators)?;
        if active_validator_ids(governance)? != previous {
            governance_changed = true;
        }
    }
    Ok(DueValidatorRegistryActivations {
        registry: registry_changed.then_some(registry),
        governance_changed,
    })
}

pub(super) fn validator_registry_update_can_live_apply(
    update: &ValidatorRegistryUpdateRecord,
    governance: &GovernanceState,
) -> io::Result<bool> {
    let previous_validators = validator_registry_update_previous_validators(update);
    let new_validators = validator_registry_update_new_validators(update);
    let active_validators = active_validator_ids(governance)?;
    match update.operation.as_str() {
        VALIDATOR_REGISTRY_OP_ROTATE_KEY => {
            let rotated_count = u32::try_from(new_validators.len()).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "validator registry rotation validator count exceeds u32",
                )
            })?;
            Ok(previous_validators == new_validators
                && (new_validators == active_validators
                    || (new_validators == local_validator_ids(rotated_count)?
                        && new_validators.len() <= active_validators.len())))
        }
        VALIDATOR_REGISTRY_OP_ADMIT => Ok(new_validators.len() == previous_validators.len() + 1),
        VALIDATOR_REGISTRY_OP_REMOVE
        | VALIDATOR_REGISTRY_OP_SUSPEND
        | VALIDATOR_REGISTRY_OP_REACTIVATE => Ok(!new_validators.is_empty()),
        _ => Ok(false),
    }
}

pub(super) fn verify_archived_payload_id(block: &BlockRecord, batch_id: &str) -> io::Result<()> {
    if batch_id != block.header.batch_id {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "block {} archived payload batch id mismatch",
                block.header.height
            ),
        ));
    }
    Ok(())
}

pub(super) fn verify_archived_payload_receipt_count(
    block: &BlockRecord,
    action_count: u64,
) -> io::Result<()> {
    if action_count != block.header.receipt_count {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "block {} archived payload action count {} does not match receipt count {}",
                block.header.height, action_count, block.header.receipt_count
            ),
        ));
    }
    Ok(())
}

pub(super) fn verify_replayed_blocks(
    store: &NodeStore,
    genesis: &Genesis,
    blocks: &BlockLog,
    archive: &BatchArchive,
    persisted_receipts: &[Receipt],
) -> io::Result<String> {
    let mut persisted_receipts_by_id = BTreeMap::<&str, Vec<&Receipt>>::new();
    for receipt in persisted_receipts {
        persisted_receipts_by_id
            .entry(receipt.tx_id.as_str())
            .or_default()
            .push(receipt);
    }
    let mut consumed_persisted_receipts = HashMap::<&str, usize>::new();
    let live_validator_registry =
        read_validator_registry_file(&store.data_dir().join(VALIDATOR_REGISTRY_FILE))?;
    let checkpoint = read_history_checkpoint_state_optional(store)?;
    let has_history_checkpoint = checkpoint.is_some();
    let mut validator_registry;
    let mut governance;
    let mut registry_update_ids;
    let mut ledger;
    let mut ordered_batches;
    let mut shielded;
    let mut bridge;
    let live_ledger = store.read_ledger()?;
    let tip_fastpay_effects = fastpay_pre_state_effects_for_next_block(store, &live_ledger)?;
    let mut replayed_fastpay_locks = BTreeSet::new();
    if let Some(checkpoint) = checkpoint {
        validator_registry = checkpoint.validator_registry;
        governance = checkpoint.governance;
        registry_update_ids = governance
            .validator_registry_updates
            .iter()
            .filter(|update| update.activation_height <= checkpoint.pruned_up_to_height)
            .map(|update| update.update_id.clone())
            .collect::<HashSet<_>>();
        ledger = checkpoint.ledger;
        ordered_batches = checkpoint.ordered_batches;
        shielded = checkpoint.shielded;
        bridge = checkpoint.bridge;
    } else {
        let faucet_account = read_faucet_account_file(&store.data_dir().join(FAUCET_ACCOUNT_FILE))?;
        validator_registry = read_validator_registry_replay_base(store)?;
        validate_validator_registry_for_count(&validator_registry, genesis.validator_count)?;
        governance = GovernanceState::new(genesis.validator_count);
        registry_update_ids = HashSet::new();
        ledger = LedgerState::new(vec![faucet_account]);
        ordered_batches = Vec::<String>::new();
        shielded = ShieldedState::empty();
        bridge = BridgeState::empty();
    }
    let initial_native_supply = native_pft_live_total(&ledger, &shielded)?;
    if initial_native_supply > u128::from(genesis.expected_native_supply_atoms()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "replay base live native supply {initial_native_supply} exceeds genesis native supply {}",
                genesis.expected_native_supply_atoms()
            ),
        ));
    }
    if !has_history_checkpoint
        && initial_native_supply != u128::from(genesis.expected_native_supply_atoms())
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "height-zero live native supply {initial_native_supply} does not equal genesis native supply {}",
                genesis.expected_native_supply_atoms()
            ),
        ));
    }
    let mut replay_state_root;

    for block in &blocks.blocks {
        validate_fastpay_pre_state_effects(&block.fastpay_pre_state_effects)?;
        for effect in &block.fastpay_pre_state_effects {
            if !replayed_fastpay_locks.insert(effect.lock_id.clone()) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "FastPay pre-state effect is anchored by more than one block",
                ));
            }
            replay_confirmed_fastpay_fence(&mut ledger, &shielded, effect)?;
        }
        let native_supply_before = native_pft_live_total(&ledger, &shielded)?;
        if ordered_batches.contains(&block.header.batch_id) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "block {} replays duplicate ordered batch `{}`",
                    block.header.height, block.header.batch_id
                ),
            ));
        }
        let Some(archive_entry) = archive.find(&block.header.batch_kind, &block.header.batch_id)
        else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "block {} batch payload is not archived",
                    block.header.height
                ),
            ));
        };
        activate_validator_registry_updates_for_height(
            genesis,
            &mut validator_registry,
            &mut governance,
            &mut registry_update_ids,
            block.header.height,
        )?;
        let replay_validators = active_validator_ids(&governance)?;
        backfill_legacy_validator_registry_records(
            &mut validator_registry,
            &live_validator_registry,
            &replay_validators,
            &format!("block {} state replay", block.header.height),
        )?;
        let receipts = replay_archived_payload(
            genesis,
            block,
            archive_entry,
            ArchivedReplayState {
                governance: &mut governance,
                ledger: &mut ledger,
                shielded: &mut shielded,
                bridge: &mut bridge,
                validator_registry: &validator_registry,
            },
        )?;
        let native_supply_after = native_pft_live_total(&ledger, &shielded)?;
        verify_native_pft_transition(
            block.header.height,
            native_supply_before,
            native_supply_after,
            &receipts,
        )?;
        let replay_receipt_ids = receipts
            .iter()
            .map(|receipt| receipt.tx_id.clone())
            .collect::<Vec<_>>();
        let legacy_receipt_id_mismatch = replay_receipt_ids != block.receipt_ids
            && (archived_wan_devnet_legacy_nav_profile_id_allowed(genesis, block)
                || archived_wan_devnet2_legacy_receipt_id_drift_allowed(genesis, block));
        if replay_receipt_ids != block.receipt_ids && !legacy_receipt_id_mismatch {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "block {} replay receipt ids mismatch: replayed {:?}; expected {:?}",
                    block.header.height, replay_receipt_ids, block.receipt_ids
                ),
            ));
        }
        if legacy_receipt_id_mismatch && replay_receipt_ids.len() != block.receipt_ids.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "block {} legacy replay receipt id mismatch changed receipt count: replayed {:?}; expected {:?}",
                    block.header.height, replay_receipt_ids, block.receipt_ids
                ),
            ));
        }
        for (receipt_index, replayed_receipt) in receipts.iter().enumerate() {
            let expected_receipt_id = block.receipt_ids.get(receipt_index).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "block {} replay receipt index {receipt_index} has no block receipt id",
                        block.header.height
                    ),
                )
            })?;
            let consumed = consumed_persisted_receipts
                .entry(expected_receipt_id.as_str())
                .or_default();
            let persisted_receipt = persisted_receipts_by_id
                .get(expected_receipt_id.as_str())
                .and_then(|receipts| receipts.get(*consumed))
                .copied()
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "block {} replay receipt `{expected_receipt_id}` is missing from persisted receipts",
                            block.header.height
                        ),
                    )
                })?;
            *consumed += 1;
            if !replayed_receipt_matches_persisted(
                genesis,
                block,
                replayed_receipt,
                persisted_receipt,
            ) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "block {} replay receipt `{expected_receipt_id}` differs from persisted receipt: replayed {}; persisted {}",
                        block.header.height,
                        receipt_replay_summary(replayed_receipt),
                        receipt_replay_summary(persisted_receipt)
                    ),
                ));
            }
        }

        ordered_batches.push(block.header.batch_id.clone());
        replay_state_root = replicated_state_root(
            genesis,
            &governance,
            &ledger,
            &ordered_batches,
            &shielded,
            &bridge,
        )?;
        if replay_state_root != block.header.state_root {
            let legacy_nav_state_root = legacy_nav_incomplete_replicated_state_root(
                genesis,
                &governance,
                &ledger,
                &ordered_batches,
                &shielded,
                &bridge,
            )?;
            if legacy_nav_state_root == block.header.state_root {
                continue;
            }
            let legacy_nav_profile_sp1_uncommitted_state_root =
                legacy_nav_profile_sp1_uncommitted_replicated_state_root(
                    genesis,
                    &governance,
                    &ledger,
                    &ordered_batches,
                    &shielded,
                    &bridge,
                )?;
            if archived_wan_devnet_legacy_nav_profile_id_allowed(genesis, block)
                && legacy_nav_profile_sp1_uncommitted_state_root == block.header.state_root
            {
                continue;
            }
            let legacy_nav_asset_uncommitted_state_root =
                legacy_nav_asset_uncommitted_replicated_state_root(
                    genesis,
                    &governance,
                    &ledger,
                    &ordered_batches,
                    &shielded,
                    &bridge,
                )?;
            if archived_wan_devnet_legacy_nav_asset_commitment_allowed(genesis, block)
                && legacy_nav_asset_uncommitted_state_root == block.header.state_root
            {
                continue;
            }
            let legacy_vault_bridge_domainless_withdrawal_state_root =
                legacy_vault_bridge_domainless_withdrawal_replicated_state_root(
                    genesis,
                    &governance,
                    &ledger,
                    &ordered_batches,
                    &shielded,
                    &bridge,
                )?;
            if archived_wan_devnet_legacy_nav_profile_id_allowed(genesis, block)
                && legacy_vault_bridge_domainless_withdrawal_state_root == block.header.state_root
            {
                continue;
            }
            let legacy_vault_bridge_deposit_attestation_state_root =
                legacy_vault_bridge_deposit_attestation_replicated_state_root(
                    genesis,
                    &governance,
                    &ledger,
                    &ordered_batches,
                    &shielded,
                    &bridge,
                )?;
            if bridge_verification_legacy_replay_allowed(&governance, block.header.height)
                && legacy_vault_bridge_deposit_attestation_state_root == block.header.state_root
            {
                continue;
            }
            let legacy_state_root = legacy_json_replicated_state_root(
                genesis,
                &governance,
                &ledger,
                &ordered_batches,
                &shielded,
                &bridge,
            )?;
            if legacy_state_root == block.header.state_root {
                continue;
            }
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "block {} replay state root {}, legacy NAV-incomplete replay state root {}, legacy NAV-profile-SP1-uncommitted replay state root {}, legacy NAV-asset-uncommitted replay state root {}, legacy vault-bridge-domainless-withdrawal replay state root {}, legacy vault-bridge-deposit-attestation replay state root {}, and legacy JSON replay state root {} do not match header {}",
                    block.header.height,
                    replay_state_root,
                    legacy_nav_state_root,
                    legacy_nav_profile_sp1_uncommitted_state_root,
                    legacy_nav_asset_uncommitted_state_root,
                    legacy_vault_bridge_domainless_withdrawal_state_root,
                    legacy_vault_bridge_deposit_attestation_state_root,
                    legacy_state_root,
                    block.header.state_root
                ),
            ));
        }
    }

    validate_fastpay_pre_state_effects(&tip_fastpay_effects)?;
    for effect in &tip_fastpay_effects {
        if !replayed_fastpay_locks.insert(effect.lock_id.clone()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "FastPay tip effect was already anchored by a block",
            ));
        }
        replay_confirmed_fastpay_fence(&mut ledger, &shielded, effect)?;
    }
    replay_state_root = replicated_state_root(
        genesis,
        &governance,
        &ledger,
        &ordered_batches,
        &shielded,
        &bridge,
    )?;

    Ok(replay_state_root)
}

pub(super) fn replay_confirmed_fastpay_fence(
    ledger: &mut LedgerState,
    shielded: &ShieldedState,
    expected_fence: &postfiat_types::FastPayVersionFenceV1,
) -> io::Result<()> {
    expected_fence.validate_shape().map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("FastPay confirmed fence is invalid: {error}"),
        )
    })?;
    if let Some(existing) = ledger
        .fastpay_version_fences
        .iter()
        .find(|fence| fence.lock_id == expected_fence.lock_id)
    {
        if existing == expected_fence {
            return Ok(());
        }
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "FastPay replay fence conflicts with ordered state",
        ));
    }
    let certificate = expected_fence.certificate.as_ref().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "FastPay confirmed fence omitted its certificate",
        )
    })?;
    let policy = ledger.fastpay_recovery_policy.clone().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "FastPay confirmed fence predates its recovery policy",
        )
    })?;
    let committee = ledger
        .fastpay_recovery_committees
        .iter()
        .find(|committee| {
            committee.committee_epoch == expected_fence.committee_epoch
                && committee.registry_root == expected_fence.registry_root
        })
        .cloned()
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "FastPay confirmed fence predates its committee",
            )
        })?;
    committee.validate().map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("FastPay replay committee is invalid: {error}"),
        )
    })?;
    let validator_public_keys = committee.validator_public_keys();
    let domain = committee.certificate_domain();
    let context = postfiat_execution::FastPayRecoveryVerificationContext {
        validator_public_keys: &validator_public_keys,
        expected_domain: &domain,
        committee_epoch: committee.committee_epoch,
        policy: &policy,
        quorum: committee.quorum,
    };
    let supply_before = native_pft_live_total(ledger, shielded)?;
    let fee = match certificate {
        postfiat_types::FastPayCertificateV1::Transfer(certificate) => {
            postfiat_execution::apply_owned_transfer_certificate_v3(
                ledger,
                certificate,
                context,
                expected_fence.decided_at_height,
            )
            .map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("FastPay transfer replay failed: {error:?}"),
                )
            })?;
            certificate.order.fee
        }
        postfiat_types::FastPayCertificateV1::Unwrap(certificate) => {
            postfiat_execution::apply_owned_unwrap_certificate_v3(
                ledger,
                certificate,
                context,
                expected_fence.decided_at_height,
            )
            .map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("FastPay unwrap replay failed: {error:?}"),
                )
            })?;
            certificate.order.fee
        }
    };
    let supply_after = native_pft_live_total(ledger, shielded)?;
    if supply_after.checked_add(u128::from(fee)) != Some(supply_before)
        || ledger.fastpay_version_fences.last() != Some(expected_fence)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "FastPay replay effect does not match its durable fence or fee burn",
        ));
    }
    Ok(())
}

pub(super) fn fastpay_pre_state_effects_for_next_block(
    store: &NodeStore,
    ledger: &LedgerState,
) -> io::Result<Vec<postfiat_types::FastPayVersionFenceV1>> {
    let mut anchored = BTreeMap::<String, postfiat_types::FastPayVersionFenceV1>::new();
    if let Some(checkpoint) = read_history_checkpoint_state_optional(store)? {
        for effect in checkpoint
            .ledger
            .fastpay_version_fences
            .into_iter()
            .filter(|fence| {
                fence.origin == postfiat_types::FastPayFenceOriginV1::Consensusless
                    && matches!(
                        fence.decision,
                        postfiat_types::FastPayRecoveryDecisionV1::Confirmed { .. }
                    )
            })
        {
            if anchored.insert(effect.lock_id.clone(), effect).is_some() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "FastPay effect is duplicated in the history checkpoint",
                ));
            }
        }
    }
    for block in store.read_blocks()?.blocks {
        validate_fastpay_pre_state_effects(&block.fastpay_pre_state_effects)?;
        for effect in block.fastpay_pre_state_effects {
            if anchored.insert(effect.lock_id.clone(), effect).is_some() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "FastPay effect is anchored more than once across checkpoint and block history",
                ));
            }
        }
    }
    let mut effects = ledger
        .fastpay_version_fences
        .iter()
        .filter(|fence| {
            fence.origin == postfiat_types::FastPayFenceOriginV1::Consensusless
                && matches!(
                    fence.decision,
                    postfiat_types::FastPayRecoveryDecisionV1::Confirmed { .. }
                )
                && !anchored.contains_key(&fence.lock_id)
        })
        .cloned()
        .collect::<Vec<_>>();
    for (lock_id, effect) in &anchored {
        if !ledger
            .fastpay_version_fences
            .iter()
            .any(|fence| fence == effect)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("anchored FastPay effect `{lock_id}` does not match live ledger evidence"),
            ));
        }
    }
    effects.sort_by(|left, right| left.lock_id.cmp(&right.lock_id));
    if effects.len() > postfiat_types::MAX_FASTPAY_PRE_STATE_EFFECTS_PER_BLOCK {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{} unanchored FastPay effects exceed the per-block limit {}",
                effects.len(),
                postfiat_types::MAX_FASTPAY_PRE_STATE_EFFECTS_PER_BLOCK
            ),
        ));
    }
    Ok(effects)
}

pub(super) fn reconcile_fastpay_pre_state_effects(
    store: &NodeStore,
    ledger: &mut LedgerState,
    shielded: &ShieldedState,
    supplied: &[postfiat_types::FastPayVersionFenceV1],
) -> io::Result<Vec<postfiat_types::FastPayVersionFenceV1>> {
    validate_fastpay_pre_state_effects(supplied)?;

    let local = fastpay_pre_state_effects_for_next_block(store, ledger)?;
    for expected in &local {
        if !supplied.iter().any(|effect| effect == expected) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "block proposal omitted a locally durable unanchored FastPay effect",
            ));
        }
    }
    for effect in supplied {
        replay_confirmed_fastpay_fence(ledger, shielded, effect)?;
    }
    let reconciled = fastpay_pre_state_effects_for_next_block(store, ledger)?;
    if reconciled != supplied {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "FastPay pre-state effects do not exactly reconcile local state",
        ));
    }
    Ok(reconciled)
}

pub(super) fn reconcile_certified_fastpay_pre_state_effects(
    store: &NodeStore,
    ledger: &mut LedgerState,
    shielded: &ShieldedState,
    supplied: &[postfiat_types::FastPayVersionFenceV1],
) -> io::Result<Vec<postfiat_types::FastPayVersionFenceV1>> {
    validate_fastpay_pre_state_effects(supplied)?;
    let local = fastpay_pre_state_effects_for_next_block(store, ledger)?;
    if local
        .iter()
        .any(|expected| !supplied.iter().any(|effect| effect == expected))
    {
        // A valid block certificate can omit a direct effect only when fewer
        // than q validators applied it: q durable applies and q block voters
        // intersect in honest validators that reject omission. Such a minority
        // effect was never product-final, so roll back the entire unanchored
        // suffix in reverse order, retain its certificate for recovery, and
        // replay the certificate-bound effects selected by the canonical block.
        rollback_unanchored_fastpay_effects_for_certified_block(store, ledger, &local)?;
    }
    reconcile_fastpay_pre_state_effects(store, ledger, shielded, supplied)
}

pub(super) fn validate_fastpay_pre_state_effects(
    supplied: &[postfiat_types::FastPayVersionFenceV1],
) -> io::Result<()> {
    if supplied.len() > postfiat_types::MAX_FASTPAY_PRE_STATE_EFFECTS_PER_BLOCK {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "FastPay pre-state effect list exceeds its per-block limit",
        ));
    }
    let mut prior_lock_id: Option<&str> = None;
    for effect in supplied {
        effect.validate_shape().map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("FastPay pre-state effect is invalid: {error}"),
            )
        })?;
        if effect.origin != postfiat_types::FastPayFenceOriginV1::Consensusless
            || !matches!(
                effect.decision,
                postfiat_types::FastPayRecoveryDecisionV1::Confirmed { .. }
            )
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "block pre-state evidence may contain only confirmed consensusless FastPay effects",
            ));
        }
        let certificate = effect.certificate.as_ref().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "FastPay pre-state effect omitted its certificate",
            )
        })?;
        if effect.decided_at_height != certificate.recovery().valid_from_height {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "consensusless FastPay effect used a non-canonical decision reference height",
            ));
        }
        if prior_lock_id.is_some_and(|prior| prior >= effect.lock_id.as_str()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "FastPay pre-state effects are not strictly ordered by lock ID",
            ));
        }
        prior_lock_id = Some(&effect.lock_id);
    }

    Ok(())
}

pub fn native_pft_live_total(ledger: &LedgerState, shielded: &ShieldedState) -> io::Result<u128> {
    fn add(total: &mut u128, amount: u128, lane: &str) -> io::Result<()> {
        *total = total.checked_add(amount).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("native supply overflow while accounting {lane}"),
            )
        })?;
        Ok(())
    }

    // Deliberately exhaustive. A new ledger or shielded-state field fails the
    // build until it is classified as native custody or explicitly non-native.
    let LedgerState {
        accounts,
        asset_definitions: _,
        trustlines: _,
        escrows,
        nfts: _,
        offers,
        nav_assets: _,
        nav_reserve_packets: _,
        nav_redemptions: _,
        nav_proof_profiles: _,
        nav_attestors: _,
        market_ops_policies: _,
        market_ops_envelopes: _,
        vault_bridge_receipts: _,
        vault_bridge_bucket_states: _,
        vault_bridge_allocations: _,
        vault_bridge_redemptions: _,
        vault_bridge_deposits: _,
        pftl_uniswap_routes: _,
        pftl_uniswap_receipts: _,
        owned_objects,
        fastpay_recovery_policy: _,
        fastpay_recovery_committees: _,
        fastpay_recovery_reveals: _,
        fastpay_version_fences: _,
        fast_lane_reserves,
        fast_lane_deposit_receipts: _,
        redeemed_fast_lane_exit_claims: _,
        fast_lane_asset_rules: _,
        fast_lane_holder_permits: _,
        fastswap_policy_snapshots: _,
        fastswap_committees: _,
        fast_lane_prepare_fences: _,
        fast_lane_checkpoint_anchors: _,
        fastswap_activation_height: _,
        ethereum_arbitrum_finality_states: _,
    } = ledger;
    let ShieldedState {
        next_note_position: _,
        notes: _,
        nullifiers: _,
        turnstile_events: _,
        orchard,
    } = shielded;

    let duplicate = |lane: &str, key: &str| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("duplicate native custody {lane} key `{key}`"),
        )
    };
    let mut total = 0_u128;
    let mut account_ids = std::collections::BTreeSet::new();
    for account in accounts {
        if !account_ids.insert(account.address.as_str()) {
            return Err(duplicate("account", &account.address));
        }
        add(&mut total, u128::from(account.balance), "account balances")?;
    }
    let mut escrow_ids = std::collections::BTreeSet::new();
    for escrow in escrows {
        if !escrow_ids.insert(escrow.escrow_id.as_str()) {
            return Err(duplicate("escrow", &escrow.escrow_id));
        }
        if escrow.state == ESCROW_STATE_OPEN
            && escrow.asset_id == postfiat_execution::NATIVE_PFT_ESCROW_ASSET_ID
        {
            add(&mut total, u128::from(escrow.amount), "open escrows")?;
        }
    }
    let mut offer_ids = std::collections::BTreeSet::new();
    for offer in offers {
        if !offer_ids.insert(offer.offer_id.as_str()) {
            return Err(duplicate("offer", &offer.offer_id));
        }
        add(&mut total, u128::from(offer.reserve_paid), "offer reserves")?;
        if offer.state == OFFER_STATE_OPEN
            && offer.taker_gets_asset_id == postfiat_execution::NATIVE_PFT_ESCROW_ASSET_ID
        {
            add(
                &mut total,
                u128::from(offer.taker_gets_amount_remaining),
                "open offer native sell balances",
            )?;
        }
    }
    let mut object_ids = std::collections::BTreeSet::new();
    for object in owned_objects {
        if !object_ids.insert((object.id.as_str(), object.version)) {
            return Err(duplicate("owned object", &object.id));
        }
        if object.asset == postfiat_execution::OWNED_NATIVE_ASSET {
            add(&mut total, u128::from(object.value), "owned native objects")?;
        }
    }
    let mut reserve_assets = std::collections::BTreeSet::new();
    for reserve in fast_lane_reserves {
        if !reserve_assets.insert(reserve.asset_id.0) {
            return Err(duplicate(
                "FastLane reserve",
                &postfiat_crypto_provider::bytes_to_hex(&reserve.asset_id.0),
            ));
        }
        if reserve.asset_id == postfiat_types::FastAssetIdV1::native_pft() {
            add(&mut total, reserve.amount_atoms, "FastLane native reserves")?;
        }
    }
    if let Some(pool) = orchard.as_ref() {
        let orchard_live = pool
            .turnstile_deposit_total
            .checked_sub(pool.fee_burn_total)
            .and_then(|value| value.checked_sub(pool.withdraw_total))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Orchard native accounting exceeds turnstile deposits",
                )
            })?;
        add(
            &mut total,
            u128::from(orchard_live),
            "Orchard live native value",
        )?;
    }
    Ok(total)
}

pub(super) fn verify_native_pft_transition(
    block_height: u64,
    live_before: u128,
    live_after: u128,
    receipts: &[Receipt],
) -> io::Result<()> {
    let fee_burn = native_pft_fee_burn_total(block_height, receipts)?;
    let expected_after = live_before.checked_sub(fee_burn).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "block {block_height} burns {fee_burn} native atoms from live supply {live_before}"
            ),
        )
    })?;
    if live_after != expected_after {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "block {block_height} native supply conservation failed: live before {live_before}, receipt fee burn {fee_burn}, expected live after {expected_after}, observed {live_after}"
            ),
        ));
    }
    Ok(())
}

pub(super) fn native_pft_fee_burn_total(
    block_height: u64,
    receipts: &[Receipt],
) -> io::Result<u128> {
    receipts.iter().try_fold(0_u128, |total, receipt| {
        total
            .checked_add(u128::from(receipt.fee_burned))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("block {block_height} native fee burn total overflow"),
                )
            })
    })
}

pub(super) fn receipt_replay_summary(receipt: &Receipt) -> String {
    format!(
        "accepted={} code={} message={:?} fee_charged={} fee_burned={} minimum_fee={} account_reserve={} state_expansion_fee={} offer_id={:?} offer_fills={}",
        receipt.accepted,
        receipt.code,
        receipt.message,
        receipt.fee_charged,
        receipt.fee_burned,
        receipt.minimum_fee,
        receipt.account_reserve,
        receipt.state_expansion_fee,
        receipt.offer_id,
        receipt.offer_fills.len()
    )
}

pub(super) fn replayed_receipt_matches_persisted(
    genesis: &Genesis,
    block: &BlockRecord,
    replayed_receipt: &Receipt,
    persisted_receipt: &Receipt,
) -> bool {
    if replayed_receipt == persisted_receipt {
        return true;
    }
    if !archived_wan_devnet_legacy_nav_profile_id_allowed(genesis, block)
        && !archived_wan_devnet2_legacy_receipt_id_drift_allowed(genesis, block)
    {
        return false;
    }
    let mut replayed_with_persisted_id = replayed_receipt.clone();
    replayed_with_persisted_id.tx_id = persisted_receipt.tx_id.clone();
    replayed_with_persisted_id == *persisted_receipt
}

pub(super) struct ArchivedReplayState<'a> {
    pub(super) governance: &'a mut GovernanceState,
    pub(super) ledger: &'a mut LedgerState,
    pub(super) shielded: &'a mut ShieldedState,
    pub(super) bridge: &'a mut BridgeState,
    pub(super) validator_registry: &'a ValidatorRegistry,
}

pub(super) fn replay_archived_payload(
    genesis: &Genesis,
    block: &BlockRecord,
    archive_entry: &BatchArchiveEntry,
    state: ArchivedReplayState<'_>,
) -> io::Result<Vec<Receipt>> {
    match block.header.batch_kind.as_str() {
        "transparent" => {
            let batch: TransactionBatch = parse_archived_payload(block, archive_entry)?;
            verify_archived_transparent_batch_id(genesis, block, &batch)?;
            let receipts = execute_transparent_batch_for_archive_replay(
                genesis,
                state.ledger,
                &batch,
                block,
                state.governance,
            )?;
            replay_legacy_wan_devnet_nav_profile_ids(genesis, block, state.ledger, &batch)?;
            Ok(receipts)
        }
        "governance" => {
            let batch: GovernanceActionBatch = parse_archived_payload(block, archive_entry)?;
            let has_v2_authorization = batch
                .amendments
                .iter()
                .any(|amendment| !amendment.signed_authorizations.is_empty())
                || batch
                    .validator_registry_updates
                    .iter()
                    .any(|update| !update.signed_authorizations.is_empty())
                || batch
                    .fastswap_bootstraps
                    .iter()
                    .any(|bootstrap| !bootstrap.amendment.signed_authorizations.is_empty())
                || batch
                    .vault_bridge_route_profile_activations
                    .iter()
                    .any(|activation| !activation.amendment.signed_authorizations.is_empty());
            if has_v2_authorization {
                verify_live_signed_governance_batch(
                    genesis,
                    state.governance,
                    state.validator_registry,
                    &batch,
                    block.header.height,
                )?;
            }
            Ok(execute_governance_batch(
                state.governance,
                Some(state.ledger),
                &batch,
                block.header.height,
            ))
        }
        "shielded" => {
            let batch: ShieldedActionBatch = parse_archived_payload(block, archive_entry)?;
            let compatibility =
                if archived_wan_devnet_legacy_strict_domain_validation_allowed(genesis, block) {
                    AssetExecutionCompatibility::wan_devnet_legacy_strict_domain_validation()
                } else {
                    AssetExecutionCompatibility::strict()
                };
            let compatibility = asset_execution_compatibility_with_chain_activation(
                compatibility,
                genesis,
                state.governance,
            );
            Ok(execute_shielded_batch(
                genesis,
                state.ledger,
                state.shielded,
                &batch,
                block.header.height,
                compatibility,
                state.governance.orchard_pool_paused,
                true,
            ))
        }
        "bridge" => {
            let batch: BridgeActionBatch = parse_archived_payload(block, archive_entry)?;
            Ok(execute_bridge_batch(
                genesis,
                state.bridge,
                &batch,
                state.governance.bridge_witness_epoch,
                state.validator_registry,
            ))
        }
        other => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "block {} has unknown batch kind `{other}`",
                block.header.height
            ),
        )),
    }
}

pub fn faucet_key(options: NodeOptions) -> io::Result<DevKeyFile> {
    read_key_file(&options.data_dir.join(FAUCET_KEY_FILE))
}

pub fn wallet_keygen(options: WalletKeygenOptions) -> io::Result<WalletKeyReport> {
    if options.chain_id.trim().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "wallet chain id must not be empty",
        ));
    }
    ensure_output_can_be_written(&options.key_file, options.overwrite, "wallet key file")?;
    ensure_output_can_be_written(
        &options.backup_file,
        options.overwrite,
        "wallet backup file",
    )?;

    let backup = WalletBackupFile {
        schema: WALLET_BACKUP_FILE_SCHEMA.to_string(),
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        kdf: WALLET_DERIVATION_KDF.to_string(),
        derivation_domain: WALLET_DERIVATION_DOMAIN.to_string(),
        chain_id: options.chain_id,
        account_index: options.account_index,
        key_role: WALLET_KEY_ROLE_TRANSPARENT_SPEND.to_string(),
        master_seed_hex: normalized_wallet_master_seed_hex(&options.master_seed_hex)?,
    };
    validate_wallet_backup_file(&backup)?;
    let key_file = derive_wallet_dev_key_file(&backup)?;

    write_wallet_backup_file(&options.backup_file, &backup)?;
    write_key_file(&options.key_file, &key_file)?;
    Ok(wallet_key_report(
        "keygen",
        &backup,
        &key_file,
        &options.key_file,
        Some(&options.backup_file),
    ))
}

pub fn wallet_restore(options: WalletRestoreOptions) -> io::Result<WalletKeyReport> {
    ensure_output_can_be_written(&options.key_file, options.overwrite, "wallet key file")?;
    let backup = read_wallet_backup_file(&options.backup_file)?;
    let key_file = derive_wallet_dev_key_file(&backup)?;
    write_key_file(&options.key_file, &key_file)?;
    Ok(wallet_key_report(
        "restore",
        &backup,
        &key_file,
        &options.key_file,
        Some(&options.backup_file),
    ))
}

pub fn wallet_sign_transfer(options: WalletSignTransferOptions) -> io::Result<SignedTransfer> {
    if options.amount == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "wallet sign-transfer amount must be nonzero",
        ));
    }
    if options.fee == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "wallet sign-transfer fee must be nonzero",
        ));
    }
    if options.sequence == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "wallet sign-transfer sequence must be nonzero",
        ));
    }

    let key_file = read_key_file(&options.key_file)?;
    let private_key =
        Zeroizing::new(hex_to_bytes(&key_file.private_key_hex).map_err(invalid_data)?);
    let unsigned = UnsignedTransfer {
        chain_id: options.chain_id,
        genesis_hash: options.genesis_hash,
        protocol_version: options.protocol_version,
        address_namespace: postfiat_types::ADDRESS_NAMESPACE.to_string(),
        transaction_kind: postfiat_types::TRANSFER_TRANSACTION_KIND.to_string(),
        signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        from: key_file.address.clone(),
        to: options.to,
        amount: options.amount,
        fee: options.fee,
        sequence: options.sequence,
    };
    unsigned
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    let signature =
        ml_dsa_65_sign(&private_key, &unsigned.signing_bytes()).map_err(invalid_data)?;
    let signed = SignedTransfer {
        unsigned,
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        public_key_hex: key_file.public_key_hex,
        signature_hex: bytes_to_hex(&signature),
    };
    signed
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;

    let public_key = hex_to_bytes(&signed.public_key_hex).map_err(invalid_data)?;
    if !ml_dsa_65_verify(&public_key, &signed.unsigned.signing_bytes(), &signature) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "wallet sign-transfer signature verification failed",
        ));
    }
    Ok(signed)
}

pub fn wallet_sign_asset_transaction(
    options: WalletSignAssetTransactionOptions,
) -> io::Result<SignedAssetTransaction> {
    if options.fee == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "wallet sign-asset-transaction fee must be nonzero",
        ));
    }
    if options.sequence == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "wallet sign-asset-transaction sequence must be nonzero",
        ));
    }
    options
        .operation
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;

    let key_file = read_key_file(&options.key_file)?;
    if let Some(expected_source) = &options.expected_source {
        if expected_source != &key_file.address {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "wallet sign-asset-transaction quote source `{expected_source}` does not match key address `{}`",
                    key_file.address
                ),
            ));
        }
    }
    let private_key =
        Zeroizing::new(hex_to_bytes(&key_file.private_key_hex).map_err(invalid_data)?);
    let unsigned = UnsignedAssetTransaction {
        chain_id: options.chain_id,
        genesis_hash: options.genesis_hash,
        protocol_version: options.protocol_version,
        address_namespace: postfiat_types::ADDRESS_NAMESPACE.to_string(),
        transaction_kind: options.operation.transaction_kind().to_string(),
        signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        source: key_file.address.clone(),
        fee: options.fee,
        sequence: options.sequence,
        operation: options.operation,
    };
    unsigned
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    let signature =
        ml_dsa_65_sign(&private_key, &unsigned.signing_bytes()).map_err(invalid_data)?;
    let signed = SignedAssetTransaction {
        unsigned,
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        public_key_hex: key_file.public_key_hex,
        signature_hex: bytes_to_hex(&signature),
    };
    signed
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;

    let public_key = hex_to_bytes(&signed.public_key_hex).map_err(invalid_data)?;
    if !ml_dsa_65_verify(&public_key, &signed.unsigned.signing_bytes(), &signature) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "wallet sign-asset-transaction signature verification failed",
        ));
    }
    Ok(signed)
}

pub fn wallet_sign_escrow_transaction(
    options: WalletSignEscrowTransactionOptions,
) -> io::Result<SignedEscrowTransaction> {
    if options.fee == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "wallet sign-escrow-transaction fee must be nonzero",
        ));
    }
    if options.sequence == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "wallet sign-escrow-transaction sequence must be nonzero",
        ));
    }
    options
        .operation
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;

    let key_file = read_key_file(&options.key_file)?;
    if let Some(expected_source) = &options.expected_source {
        if expected_source != &key_file.address {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "wallet sign-escrow-transaction quote source `{expected_source}` does not match key address `{}`",
                    key_file.address
                ),
            ));
        }
    }
    let private_key =
        Zeroizing::new(hex_to_bytes(&key_file.private_key_hex).map_err(invalid_data)?);
    let unsigned = UnsignedEscrowTransaction {
        chain_id: options.chain_id,
        genesis_hash: options.genesis_hash,
        protocol_version: options.protocol_version,
        address_namespace: postfiat_types::ADDRESS_NAMESPACE.to_string(),
        transaction_kind: options.operation.transaction_kind().to_string(),
        signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        source: key_file.address.clone(),
        fee: options.fee,
        sequence: options.sequence,
        operation: options.operation,
    };
    unsigned
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    let signature =
        ml_dsa_65_sign(&private_key, &unsigned.signing_bytes()).map_err(invalid_data)?;
    let signed = SignedEscrowTransaction {
        unsigned,
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        public_key_hex: key_file.public_key_hex,
        signature_hex: bytes_to_hex(&signature),
    };
    signed
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;

    let public_key = hex_to_bytes(&signed.public_key_hex).map_err(invalid_data)?;
    if !ml_dsa_65_verify(&public_key, &signed.unsigned.signing_bytes(), &signature) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "wallet sign-escrow-transaction signature verification failed",
        ));
    }
    Ok(signed)
}

pub fn wallet_sign_offer_transaction(
    options: WalletSignOfferTransactionOptions,
) -> io::Result<SignedOfferTransaction> {
    if options.fee == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "wallet sign-offer-transaction fee must be nonzero",
        ));
    }
    if options.sequence == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "wallet sign-offer-transaction sequence must be nonzero",
        ));
    }
    options
        .operation
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;

    let key_file = read_key_file(&options.key_file)?;
    if let Some(expected_source) = &options.expected_source {
        if expected_source != &key_file.address {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "wallet sign-offer-transaction quote source `{expected_source}` does not match key address `{}`",
                    key_file.address
                ),
            ));
        }
    }
    let private_key =
        Zeroizing::new(hex_to_bytes(&key_file.private_key_hex).map_err(invalid_data)?);
    let unsigned = UnsignedOfferTransaction {
        chain_id: options.chain_id,
        genesis_hash: options.genesis_hash,
        protocol_version: options.protocol_version,
        address_namespace: postfiat_types::ADDRESS_NAMESPACE.to_string(),
        transaction_kind: options.operation.transaction_kind().to_string(),
        signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        source: key_file.address.clone(),
        fee: options.fee,
        sequence: options.sequence,
        operation: options.operation,
    };
    unsigned
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    let signature =
        ml_dsa_65_sign(&private_key, &unsigned.signing_bytes()).map_err(invalid_data)?;
    let signed = SignedOfferTransaction {
        unsigned,
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        public_key_hex: key_file.public_key_hex,
        signature_hex: bytes_to_hex(&signature),
    };
    signed
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;

    let public_key = hex_to_bytes(&signed.public_key_hex).map_err(invalid_data)?;
    if !ml_dsa_65_verify(&public_key, &signed.unsigned.signing_bytes(), &signature) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "wallet sign-offer-transaction signature verification failed",
        ));
    }
    Ok(signed)
}

pub fn wallet_test_vector(options: WalletTestVectorOptions) -> io::Result<WalletTestVectorReport> {
    if options.amount == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "wallet test-vector amount must be nonzero",
        ));
    }
    let genesis =
        Genesis::try_new_with_validator_count(options.chain_id.clone(), options.validator_count)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    let genesis_hash_hex = genesis_hash(&genesis);
    let backup = WalletBackupFile {
        schema: WALLET_BACKUP_FILE_SCHEMA.to_string(),
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        kdf: WALLET_DERIVATION_KDF.to_string(),
        derivation_domain: WALLET_DERIVATION_DOMAIN.to_string(),
        chain_id: genesis.chain_id.clone(),
        account_index: options.account_index,
        key_role: WALLET_KEY_ROLE_TRANSPARENT_SPEND.to_string(),
        master_seed_hex: normalized_wallet_master_seed_hex(&options.master_seed_hex)?,
    };
    validate_wallet_backup_file(&backup)?;
    let key_file = derive_wallet_dev_key_file(&backup)?;
    let private_key =
        Zeroizing::new(hex_to_bytes(&key_file.private_key_hex).map_err(invalid_data)?);
    let signature_seed = wallet_test_vector_signature_seed_bytes(&options.signature_seed_hex)?;

    let mut fee = MIN_TRANSFER_FEE;
    let mut signed_transfer = None;
    for _ in 0..8 {
        let unsigned = UnsignedTransfer {
            chain_id: genesis.chain_id.clone(),
            genesis_hash: genesis_hash_hex.clone(),
            protocol_version: genesis.protocol_version,
            address_namespace: postfiat_types::ADDRESS_NAMESPACE.to_string(),
            transaction_kind: postfiat_types::TRANSFER_TRANSACTION_KIND.to_string(),
            signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            from: key_file.address.clone(),
            to: options.to.clone(),
            amount: options.amount,
            fee,
            sequence: options.sequence,
        };
        unsigned
            .validate()
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
        let signature = ml_dsa_65_sign_with_context_seed(
            &private_key,
            &unsigned.signing_bytes(),
            TX_SIGNATURE_CONTEXT,
            &signature_seed,
        )
        .map_err(invalid_data)?;
        let signed = SignedTransfer {
            unsigned,
            algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            public_key_hex: key_file.public_key_hex.clone(),
            signature_hex: bytes_to_hex(&signature),
        };
        let state_expansion_fee = if signed.unsigned.to != signed.unsigned.from {
            TRANSFER_ACCOUNT_CREATION_FEE
        } else {
            0
        };
        let minimum_fee = minimum_transfer_fee(&signed).saturating_add(state_expansion_fee);
        if fee >= minimum_fee {
            signed_transfer = Some(signed);
            break;
        }
        fee = minimum_fee;
    }
    let signed_transfer = signed_transfer.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "wallet test-vector minimum transfer fee did not converge",
        )
    })?;

    let public_key = hex_to_bytes(&signed_transfer.public_key_hex).map_err(invalid_data)?;
    let signature = hex_to_bytes(&signed_transfer.signature_hex).map_err(invalid_data)?;
    let signing_bytes = signed_transfer.unsigned.signing_bytes();
    let signature_verified = ml_dsa_65_verify(&public_key, &signing_bytes, &signature);
    if !signature_verified {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "wallet test-vector signature verification failed",
        ));
    }
    let state_expansion_fee = if signed_transfer.unsigned.to != signed_transfer.unsigned.from {
        TRANSFER_ACCOUNT_CREATION_FEE
    } else {
        0
    };
    let minimum_fee = minimum_transfer_fee(&signed_transfer).saturating_add(state_expansion_fee);
    Ok(WalletTestVectorReport {
        schema: "postfiat-wallet-test-vector-v2".to_string(),
        fixture_warning: "public fixture only; do not fund or reuse this seed".to_string(),
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        kdf: WALLET_DERIVATION_KDF.to_string(),
        derivation_domain: WALLET_DERIVATION_DOMAIN.to_string(),
        chain_id: genesis.chain_id,
        validator_count: genesis.validator_count,
        genesis_hash: genesis_hash_hex,
        protocol_version: genesis.protocol_version,
        account_index: backup.account_index,
        key_role: backup.key_role,
        address: key_file.address,
        public_key_hex: key_file.public_key_hex,
        transfer_signing_bytes_hex: bytes_to_hex(&signing_bytes),
        transfer_signing_hash: hash_hex(
            "postfiat.wallet_test_vector.signing_bytes.v1",
            &signing_bytes,
        ),
        tx_id: transfer_tx_id(&signed_transfer),
        signed_transfer,
        minimum_fee,
        signature_verified,
        private_key_material_redacted: true,
    })
}
