// The private payment circuit and its builder. Included into `asset_orchard_circuit.rs`, so it
// shares that file's note-commitment, nullifier, spend-authority and Merkle gadgets unchanged.
//
// Statement, for two input notes and two output notes:
//   1. every note opens to its commitment, and all four carry the same nonzero asset tag;
//   2. in0 + in1 = out0 + out1 + exit_amount, each note value below 2^64;
//   3. each input's nullifier and randomized key are the published ones, and its owner key
//      derives its address;
//   4. each input with a nonzero value is in the tree under the published anchor. A zero-value
//      input is a filler and needs no membership;
//   5. each output's rho is the published hash of this action's inputs;
//   6. if exit_amount is nonzero, the hidden asset tag is the published exit tag.
// Nothing about the asset or any amount is public unless exit_amount is nonzero.

pub const ASSET_ORCHARD_TRANSFER_V1_K: u32 = ASSET_ORCHARD_SWAP_V1_K;
/// Fingerprint of the verifying key every validator must derive for `asset_orchard.transfer.v1`.
/// A node whose build produces a different key refuses to verify transfers.
pub const ASSET_ORCHARD_TRANSFER_V1_RUNTIME_PINNED_VK_FINGERPRINT: &str =
    "0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";

static ASSET_ORCHARD_TRANSFER_VERIFYING_KEY: OnceLock<
    Result<AssetOrchardTransferVerifyingKey, AssetOrchardError>,
> = OnceLock::new();
static ASSET_ORCHARD_TRANSFER_PROVING_KEY: OnceLock<
    Result<AssetOrchardTransferProvingKey, AssetOrchardError>,
> = OnceLock::new();

use crate::asset_orchard::{
    asset_orchard_note_for_address, asset_orchard_transfer_exit_binding_hash,
    transfer_action_binding_hash, transfer_h_action_poseidon_inputs_from_public_instance,
    AssetOrchardShieldedAddress, AssetOrchardShieldedKeys, AssetOrchardTransferAction,
    AssetOrchardTransferExit, AssetOrchardTransferPublicFields,
    ASSET_ORCHARD_TRANSFER_ACTION_SCHEMA_V1, ASSET_ORCHARD_TRANSFER_CIRCUIT_ID_V1,
    ASSET_ORCHARD_TRANSFER_PUBLIC_INSTANCE_LEN,
};

#[derive(Clone, Debug)]
pub struct AssetOrchardTransferConfig {
    base: AssetOrchardSwapConservationConfig,
    q_transfer_tags: Selector,
    q_transfer_value: Selector,
    q_transfer_exit: Selector,
    q_transfer_anchor: Selector,
}

#[derive(Clone, Debug)]
pub struct AssetOrchardTransferCircuit {
    input_notes: [Option<AssetOrchardSwapNoteWitness>; ASSET_ORCHARD_LEG_COUNT],
    output_notes: [Option<AssetOrchardSwapNoteWitness>; ASSET_ORCHARD_LEG_COUNT],
    public_instance: Option<[pallas::Base; ASSET_ORCHARD_TRANSFER_PUBLIC_INSTANCE_LEN]>,
}

fn transfer_note_witness(
    pool_domain: pallas::Base,
    note: AssetNoteOpening,
    nk: pallas::Base,
) -> Result<AssetOrchardSwapNoteWitness, AssetOrchardError> {
    let cmx = note.cmx_allowing_zero_value(pool_domain)?;
    Ok(AssetOrchardSwapNoteWitness {
        note,
        nk: nk.into(),
        cmx,
        merkle_witness: None,
        spend_authority: None,
    })
}

impl AssetOrchardTransferCircuit {
    pub fn new(
        inputs: [AssetOrchardSwapNoteWitness; ASSET_ORCHARD_LEG_COUNT],
        outputs: [AssetOrchardSwapNoteWitness; ASSET_ORCHARD_LEG_COUNT],
        public_fields: &AssetOrchardTransferPublicFields,
    ) -> Result<Self, AssetOrchardError> {
        let public_instance = public_fields.public_instance()?;
        let tag = inputs[0].note.asset_tag;
        for note in inputs.iter().chain(outputs.iter()) {
            note.note.validate_allowing_zero_value()?;
            if note.note.cmx_allowing_zero_value(public_fields.pool_domain)? != note.cmx {
                return Err(AssetOrchardError::new(
                    "asset_orchard_note_cmx_mismatch",
                    "asset-orchard note witness cmx does not match note opening",
                ));
            }
            if note.note.asset_tag != tag {
                return Err(AssetOrchardError::new(
                    "asset_orchard_transfer_mixed_assets",
                    "every note of a transfer must carry the same asset",
                ));
            }
        }
        for input in &inputs {
            if input.merkle_witness.is_none() || input.spend_authority.is_none() {
                return Err(AssetOrchardError::new(
                    "asset_orchard_transfer_incomplete_input_witness",
                    "a transfer input needs its Merkle and spend-authority witnesses",
                ));
            }
        }
        for (index, output) in outputs.iter().enumerate() {
            if output.cmx != public_fields.output_commitments[index] {
                return Err(AssetOrchardError::new(
                    "asset_orchard_output_cmx_mismatch",
                    "asset-orchard output note witness cmx does not match public output commitment",
                ));
            }
        }
        let total_in = u128::from(inputs[0].note.value) + u128::from(inputs[1].note.value);
        let total_out = u128::from(outputs[0].note.value)
            + u128::from(outputs[1].note.value)
            + u128::from(public_fields.exit_amount);
        if total_in != total_out {
            return Err(AssetOrchardError::new(
                "asset_orchard_transfer_not_balanced",
                "transfer inputs do not equal outputs plus the exit amount",
            ));
        }
        if public_fields.exit_amount != 0
            && (tag.lo != public_fields.exit_asset_tag_lo || tag.hi != public_fields.exit_asset_tag_hi)
        {
            return Err(AssetOrchardError::new(
                "asset_orchard_transfer_exit_asset_mismatch",
                "the public exit asset is not the asset of the notes",
            ));
        }
        Ok(Self {
            input_notes: inputs.map(Some),
            output_notes: outputs.map(Some),
            public_instance: Some(public_instance),
        })
    }

    pub fn public_instance(
        &self,
    ) -> Option<[pallas::Base; ASSET_ORCHARD_TRANSFER_PUBLIC_INSTANCE_LEN]> {
        self.public_instance
    }

    pub fn full_shape() -> Self {
        Self {
            input_notes: [(); ASSET_ORCHARD_LEG_COUNT]
                .map(|_| Some(AssetOrchardSwapNoteWitness::dummy_for_shape())),
            output_notes: [(); ASSET_ORCHARD_LEG_COUNT]
                .map(|_| Some(AssetOrchardSwapNoteWitness::dummy_for_shape())),
            public_instance: None,
        }
    }
}

type TransferCell = AssignedCell<pallas::Base, pallas::Base>;

impl Circuit<pallas::Base> for AssetOrchardTransferCircuit {
    type Config = AssetOrchardTransferConfig;
    type FloorPlanner = SimpleFloorPlanner;

    fn without_witnesses(&self) -> Self {
        Self {
            input_notes: [(); ASSET_ORCHARD_LEG_COUNT]
                .map(|_| Some(AssetOrchardSwapNoteWitness::dummy_for_shape())),
            output_notes: [(); ASSET_ORCHARD_LEG_COUNT]
                .map(|_| Some(AssetOrchardSwapNoteWitness::dummy_for_shape())),
            public_instance: self.public_instance,
        }
    }

    fn configure(meta: &mut ConstraintSystem<pallas::Base>) -> Self::Config {
        let base = AssetOrchardSwapConservationCircuit::configure(meta);
        let advice = base.advice;
        let q_transfer_tags = meta.selector();
        let q_transfer_value = meta.selector();
        let q_transfer_exit = meta.selector();
        let q_transfer_anchor = meta.selector();

        // One row per tag limb: the four notes agree on it.
        meta.create_gate("asset-orchard transfer one asset", |meta| {
            let q = meta.query_selector(q_transfer_tags);
            let in0 = meta.query_advice(advice[1], Rotation::cur());
            let in1 = meta.query_advice(advice[2], Rotation::cur());
            let out0 = meta.query_advice(advice[3], Rotation::cur());
            let out1 = meta.query_advice(advice[4], Rotation::cur());
            vec![
                q.clone() * (in1 - in0.clone()),
                q.clone() * (out0 - in0.clone()),
                q * (out1 - in0),
            ]
        });

        meta.create_gate("asset-orchard transfer value balance", |meta| {
            let q = meta.query_selector(q_transfer_value);
            let in0 = meta.query_advice(advice[1], Rotation::cur());
            let in1 = meta.query_advice(advice[2], Rotation::cur());
            let out0 = meta.query_advice(advice[3], Rotation::cur());
            let out1 = meta.query_advice(advice[4], Rotation::cur());
            let exit_amount = meta.query_advice(advice[5], Rotation::cur());
            vec![q * (in0 + in1 - out0 - out1 - exit_amount)]
        });

        // A nonzero public exit amount reveals the asset: the hidden tag must be the public one.
        meta.create_gate("asset-orchard transfer exit asset", |meta| {
            let q = meta.query_selector(q_transfer_exit);
            let tag_lo = meta.query_advice(advice[0], Rotation::cur());
            let tag_hi = meta.query_advice(advice[1], Rotation::cur());
            let exit_tag_lo = meta.query_advice(advice[2], Rotation::cur());
            let exit_tag_hi = meta.query_advice(advice[3], Rotation::cur());
            let exit_amount = meta.query_advice(advice[4], Rotation::cur());
            vec![
                q.clone() * exit_amount.clone() * (tag_lo - exit_tag_lo),
                q * exit_amount * (tag_hi - exit_tag_hi),
            ]
        });

        // A zero-value input is a filler and needs no place in the tree.
        meta.create_gate("asset-orchard transfer conditional anchor", |meta| {
            let q = meta.query_selector(q_transfer_anchor);
            let root = meta.query_advice(advice[0], Rotation::cur());
            let anchor = meta.query_advice(advice[1], Rotation::cur());
            let value = meta.query_advice(advice[2], Rotation::cur());
            vec![q * value * (root - anchor)]
        });

        AssetOrchardTransferConfig {
            base,
            q_transfer_tags,
            q_transfer_value,
            q_transfer_exit,
            q_transfer_anchor,
        }
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<pallas::Base>,
    ) -> Result<(), Error> {
        let base = &config.base;
        AssetOrchardSinsemillaChip::load(base.sinsemilla.clone(), &mut layouter)?;
        let public_values = self
            .public_instance
            .unwrap_or([pallas::Base::ZERO; ASSET_ORCHARD_TRANSFER_PUBLIC_INSTANCE_LEN]);

        let public_cells = layouter.assign_region(
            || "asset-orchard transfer public instance",
            |mut region| {
                let mut cells = Vec::with_capacity(public_values.len());
                for (row, value) in public_values.iter().copied().enumerate() {
                    cells.push(region.assign_advice(
                        || "transfer public instance",
                        base.advice[0],
                        row,
                        || Value::known(value),
                    )?);
                }
                // The two nullifiers differ, and so do the two output commitments.
                for row in [2usize, 8usize] {
                    base.q_public_distinct.enable(&mut region, row)?;
                    region.assign_advice(
                        || "public distinct inverse",
                        base.advice[1],
                        row,
                        || {
                            Value::known(invert_or_zero(
                                public_values[row] - public_values[row + 1],
                            ))
                        },
                    )?;
                }
                Ok(cells)
            },
        )?;
        for (row, cell) in public_cells.iter().enumerate() {
            layouter.constrain_instance(cell.cell(), base.instance, row)?;
        }
        for row in 10..=17 {
            synthesize_range_check(
                &mut layouter,
                base,
                public_cells[row].clone(),
                Value::known(public_values[row]),
                ASSET_ORCHARD_ASSET_TAG_BITS,
            )?;
        }
        synthesize_range_check(
            &mut layouter,
            base,
            public_cells[18].clone(),
            Value::known(public_values[18]),
            ASSET_ORCHARD_VALUE_BITS,
        )?;

        let leg = |note: &Option<AssetOrchardSwapNoteWitness>| -> [Value<pallas::Base>; 3] {
            match note {
                Some(note) => [
                    Value::known(pallas::Base::from_u128(note.note.asset_tag.lo)),
                    Value::known(pallas::Base::from_u128(note.note.asset_tag.hi)),
                    Value::known(pallas::Base::from(note.note.value)),
                ],
                None => [Value::unknown(), Value::unknown(), Value::unknown()],
            }
        };
        let legs = [
            leg(&self.input_notes[0]),
            leg(&self.input_notes[1]),
            leg(&self.output_notes[0]),
            leg(&self.output_notes[1]),
        ];
        // Rows 0 and 1 hold the tag limbs, row 2 the values. Columns 1 to 4 are the four notes,
        // in the layout the shared nonzero-tag gate reads.
        let leg_cells: [Vec<TransferCell>; 4] = layouter.assign_region(
            || "asset-orchard transfer legs",
            |mut region| {
                let mut cells: [Vec<TransferCell>; 4] = Default::default();
                for row in 0..3 {
                    for (index, leg) in legs.iter().enumerate() {
                        cells[index].push(region.assign_advice(
                            || "transfer leg",
                            base.advice[1 + index],
                            row,
                            || leg[row],
                        )?);
                    }
                }
                base.q_asset_tag_nonzero.enable(&mut region, 0)?;
                for row in 0..2 {
                    config.q_transfer_tags.enable(&mut region, row)?;
                    for (index, leg) in legs.iter().enumerate() {
                        region.assign_advice(
                            || "transfer tag limb inverse",
                            base.advice[5 + index],
                            row,
                            || leg[row].map(invert_or_zero),
                        )?;
                    }
                }
                config.q_transfer_value.enable(&mut region, 2)?;
                let exit_amount = region.assign_advice(
                    || "transfer exit amount",
                    base.advice[5],
                    2,
                    || Value::known(public_values[18]),
                )?;
                region.constrain_equal(exit_amount.cell(), public_cells[18].cell())?;
                Ok(cells)
            },
        )?;

        layouter.assign_region(
            || "asset-orchard transfer exit asset",
            |mut region| {
                config.q_transfer_exit.enable(&mut region, 0)?;
                leg_cells[0][0].copy_advice(|| "hidden tag lo", &mut region, base.advice[0], 0)?;
                leg_cells[0][1].copy_advice(|| "hidden tag hi", &mut region, base.advice[1], 0)?;
                public_cells[16].copy_advice(|| "exit tag lo", &mut region, base.advice[2], 0)?;
                public_cells[17].copy_advice(|| "exit tag hi", &mut region, base.advice[3], 0)?;
                public_cells[18].copy_advice(|| "exit amount", &mut region, base.advice[4], 0)?;
                Ok(())
            },
        )?;

        let inputs = transfer_h_action_poseidon_inputs_from_public_instance(&public_values)
            .map_err(|_| Error::Synthesis)?;
        let sources: Vec<HActionInputSource> = inputs
            .iter()
            .copied()
            .enumerate()
            .map(|(index, value)| {
                if index < 7 {
                    HActionInputSource::Constant(value)
                } else {
                    HActionInputSource::Public {
                        value,
                        row: index - 7,
                    }
                }
            })
            .collect();
        let h_action_cells = synthesize_poseidon_sponge(
            &mut layouter,
            base,
            "asset-orchard transfer h_action poseidon binding",
            &sources,
        )?;
        for (cell, instance_row) in h_action_cells.public_inputs {
            layouter.constrain_instance(cell.cell(), base.instance, instance_row)?;
        }
        layouter.constrain_instance(h_action_cells.action_context[0].cell(), base.instance, 21)?;
        layouter.constrain_instance(h_action_cells.action_context[1].cell(), base.instance, 22)?;

        let ecc_chip =
            AssetOrchardEccChip::construct(base.ecc.clone(), CircuitVersion::AnchoredBase);
        let sinsemilla_chip = AssetOrchardSinsemillaChip::construct(base.sinsemilla.clone());
        for (index, note) in self.input_notes.iter().enumerate() {
            let note = note.as_ref().ok_or(Error::Synthesis)?;
            synthesize_transfer_note(
                &mut layouter,
                &config,
                sinsemilla_chip.clone(),
                ecc_chip.clone(),
                format!("transfer input note {index}"),
                note,
                &public_cells,
                &public_values,
                &leg_cells[index],
                TransferNoteRole::Input { index },
            )?;
        }
        for (index, note) in self.output_notes.iter().enumerate() {
            let note = note.as_ref().ok_or(Error::Synthesis)?;
            synthesize_transfer_note(
                &mut layouter,
                &config,
                sinsemilla_chip.clone(),
                ecc_chip.clone(),
                format!("transfer output note {index}"),
                note,
                &public_cells,
                &public_values,
                &leg_cells[2 + index],
                TransferNoteRole::Output { index },
            )?;
        }
        Ok(())
    }
}

#[derive(Copy, Clone)]
enum TransferNoteRole {
    Input { index: usize },
    Output { index: usize },
}

#[allow(clippy::too_many_arguments)]
fn synthesize_transfer_note(
    layouter: &mut impl Layouter<pallas::Base>,
    config: &AssetOrchardTransferConfig,
    sinsemilla_chip: AssetOrchardSinsemillaChip,
    ecc_chip: AssetOrchardEccChip,
    label: String,
    note: &AssetOrchardSwapNoteWitness,
    public_cells: &[TransferCell],
    public_values: &[pallas::Base; ASSET_ORCHARD_TRANSFER_PUBLIC_INSTANCE_LEN],
    leg_cells: &[TransferCell],
    role: TransferNoteRole,
) -> Result<(), Error> {
    let base = &config.base;
    let pool_domain = public_values[0];
    let pool_domain_cell = public_cells[0].clone();
    let source_bits = synthesize_note_source_bits(
        layouter,
        base,
        &label,
        note,
        pool_domain_cell.clone(),
        pool_domain,
        leg_cells,
    )?;
    let mut piece_bits = vec![Vec::<TransferCell>::new(); ASSET_ORCHARD_NOTE_MESSAGE_PIECE_COUNT];
    for segment in asset_note_message_segments() {
        let source = source_bits.source(segment.source);
        piece_bits[segment.piece_index].extend_from_slice(
            &source[segment.source_bit_offset..segment.source_bit_offset + segment.bit_len],
        );
    }
    let mut piece_subpieces = Vec::with_capacity(piece_bits.len());
    for (piece_index, bits) in piece_bits.iter().enumerate() {
        let piece = synthesize_packed_bit_subpiece(
            layouter,
            base,
            &format!("{label} message piece {piece_index}"),
            bits,
        )?;
        piece_subpieces.push(vec![piece]);
    }
    let cmx = synthesize_asset_note_commitment_from_assigned_subpieces(
        layouter,
        &base.message_piece,
        sinsemilla_chip.clone(),
        ecc_chip.clone(),
        &piece_subpieces,
        Value::known(*note.note.rcm),
    )?;
    match role {
        TransferNoteRole::Output { index } => {
            layouter.constrain_instance(cmx.inner().cell(), base.instance, 8 + index)?;
            synthesize_transfer_output_rho(
                layouter,
                base,
                &format!("{label} output rho"),
                public_cells,
                public_values,
                index,
                source_bits.rho_cell.clone(),
            )
        }
        TransferNoteRole::Input { index } => {
            let nullifier = synthesize_asset_nullifier(
                layouter,
                base,
                &format!("{label} nullifier"),
                pool_domain,
                note,
                pool_domain_cell,
                source_bits.nk_cell.clone(),
                source_bits.rho_cell.clone(),
                source_bits.psi_cell.clone(),
                cmx.inner().clone(),
            )?;
            layouter.constrain_instance(nullifier.cell(), base.instance, 2 + index)?;
            synthesize_input_spend_authority(
                layouter,
                base,
                ecc_chip,
                sinsemilla_chip,
                &format!("{label} spend authority"),
                note,
                &source_bits,
                4 + index * 2,
                5 + index * 2,
            )?;
            let witness = note.merkle_witness.as_ref().ok_or(Error::Synthesis)?;
            let path = AssetOrchardMerklePath::construct(
                [
                    AssetOrchardMerkleChip::construct(base.merkle_1.clone()),
                    AssetOrchardMerkleChip::construct(base.merkle_2.clone()),
                ],
                AssetOrchardMerkleHashDomain,
                Value::known(witness.position),
                Value::known(witness.auth_path),
            );
            let root = path.calculate_root(
                layouter.namespace(|| format!("{label} merkle root")),
                cmx.inner().clone(),
            )?;
            layouter.assign_region(
                || format!("{label} conditional anchor"),
                |mut region| {
                    config.q_transfer_anchor.enable(&mut region, 0)?;
                    root.copy_advice(|| "computed merkle root", &mut region, base.advice[0], 0)?;
                    public_cells[1].copy_advice(
                        || "public anchor",
                        &mut region,
                        base.advice[1],
                        0,
                    )?;
                    leg_cells[2].copy_advice(|| "input value", &mut region, base.advice[2], 0)?;
                    Ok(())
                },
            )
        }
    }
}

fn synthesize_transfer_output_rho(
    layouter: &mut impl Layouter<pallas::Base>,
    base: &AssetOrchardSwapConservationConfig,
    label: &str,
    public_cells: &[TransferCell],
    public_values: &[pallas::Base; ASSET_ORCHARD_TRANSFER_PUBLIC_INSTANCE_LEN],
    output_index: usize,
    rho_cell: TransferCell,
) -> Result<(), Error> {
    let output_index_u8 = u8::try_from(output_index).map_err(|_| Error::Synthesis)?;
    let inputs = asset_output_rho_poseidon_inputs(
        public_values[0],
        public_values[1],
        [public_values[2], public_values[3]],
        [
            RandomizedVerificationKeyFields {
                x: public_values[4],
                y: public_values[5],
            },
            RandomizedVerificationKeyFields {
                x: public_values[6],
                y: public_values[7],
            },
        ],
        output_index_u8,
    )
    .map_err(|_| Error::Synthesis)?;
    let mut sources = vec![
        PoseidonAssignedSource::Constant(inputs[0]),
        PoseidonAssignedSource::Constant(inputs[1]),
    ];
    for row in 0..8 {
        sources.push(PoseidonAssignedSource::Cell {
            value: inputs[2 + row],
            cell: public_cells[row].clone(),
        });
    }
    sources.push(PoseidonAssignedSource::Constant(inputs[10]));
    let computed = synthesize_poseidon_hash1_from_sources(layouter, base, label, &sources)?;
    layouter.assign_region(
        || format!("{label} binding"),
        |mut region| {
            let computed =
                computed.copy_advice(|| "computed output rho", &mut region, base.advice[0], 0)?;
            let rho = rho_cell.copy_advice(|| "note output rho", &mut region, base.advice[1], 0)?;
            region.constrain_equal(computed.cell(), rho.cell())?;
            Ok(())
        },
    )
}

fn asset_orchard_transfer_runtime_pinned_vk_fingerprint(
    vk: &halo2_proofs::plonk::VerifyingKey<vesta::Affine>,
) -> String {
    let pinned = format!("{:?}", vk.pinned());
    hash_bytes("asset_orchard_transfer_vk", pinned.as_bytes())
}

#[derive(Debug)]
pub struct AssetOrchardTransferVerifyingKey {
    params: Params<vesta::Affine>,
    vk: halo2_proofs::plonk::VerifyingKey<vesta::Affine>,
    fingerprint: String,
}

impl AssetOrchardTransferVerifyingKey {
    /// Build the key from the circuit and refuse it unless it is the pinned one.
    pub fn build() -> Result<Self, AssetOrchardError> {
        let key = Self::build_unpinned()?;
        if key.fingerprint != ASSET_ORCHARD_TRANSFER_V1_RUNTIME_PINNED_VK_FINGERPRINT {
            return Err(AssetOrchardError::new(
                "asset_orchard_transfer_vk_pin_mismatch",
                format!(
                    "transfer verifying key fingerprint {} is not the pinned {}",
                    key.fingerprint, ASSET_ORCHARD_TRANSFER_V1_RUNTIME_PINNED_VK_FINGERPRINT
                ),
            ));
        }
        Ok(key)
    }

    pub fn build_unpinned() -> Result<Self, AssetOrchardError> {
        let params = asset_orchard_k15_params()?.clone();
        let (vk, _) = keygen_vk_pinned_assembly(&params, &AssetOrchardTransferCircuit::full_shape())
            .map_err(|error| {
                AssetOrchardError::new("asset_orchard_transfer_vk_build_failed", error.to_string())
            })?;
        let fingerprint = asset_orchard_transfer_runtime_pinned_vk_fingerprint(&vk);
        Ok(Self {
            params,
            vk,
            fingerprint,
        })
    }

    pub fn cached() -> Result<&'static Self, AssetOrchardError> {
        match ASSET_ORCHARD_TRANSFER_VERIFYING_KEY.get_or_init(Self::build) {
            Ok(key) => Ok(key),
            Err(error) => Err(error.clone()),
        }
    }

    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }

    pub fn verify_proof(
        &self,
        proof: &[u8],
        public_instance: &[pallas::Base; ASSET_ORCHARD_TRANSFER_PUBLIC_INSTANCE_LEN],
    ) -> Result<(), AssetOrchardError> {
        let instance_column = [&public_instance[..]];
        let instances = [&instance_column[..]];
        let strategy = SingleVerifier::new(&self.params);
        let mut transcript = Blake2bRead::<_, vesta::Affine, Challenge255<_>>::init(proof);
        verify_proof(
            &self.params,
            &self.vk,
            strategy,
            &instances,
            &mut transcript,
        )
        .map_err(|error| {
            AssetOrchardError::new(
                "asset_orchard_transfer_proof_verification_failed",
                error.to_string(),
            )
        })
    }
}

#[derive(Debug)]
pub struct AssetOrchardTransferProvingKey {
    params: Params<vesta::Affine>,
    pk: halo2_proofs::plonk::ProvingKey<vesta::Affine>,
}

impl AssetOrchardTransferProvingKey {
    pub fn build() -> Result<Self, AssetOrchardError> {
        let verifying_key = AssetOrchardTransferVerifyingKey::cached()?;
        let params = verifying_key.params.clone();
        let pk = keygen_pk(
            &params,
            verifying_key.vk.clone(),
            &AssetOrchardTransferCircuit::full_shape(),
        )
        .map_err(|error| {
            AssetOrchardError::new("asset_orchard_transfer_pk_build_failed", error.to_string())
        })?;
        Ok(Self { params, pk })
    }

    pub fn cached() -> Result<&'static Self, AssetOrchardError> {
        match ASSET_ORCHARD_TRANSFER_PROVING_KEY.get_or_init(Self::build) {
            Ok(key) => Ok(key),
            Err(error) => Err(error.clone()),
        }
    }

    pub fn create_proof(
        &self,
        circuit: &AssetOrchardTransferCircuit,
        mut rng: impl RngCore + CryptoRng,
    ) -> Result<Vec<u8>, AssetOrchardError> {
        let public_instance = circuit.public_instance.ok_or_else(|| {
            AssetOrchardError::new(
                "missing_public_instance",
                "asset-orchard transfer proof requires a public instance",
            )
        })?;
        let instance_column = [&public_instance[..]];
        let instances = [&instance_column[..]];
        let mut transcript = Blake2bWrite::<_, vesta::Affine, Challenge255<_>>::init(vec![]);
        create_proof(
            &self.params,
            &self.pk,
            std::slice::from_ref(circuit),
            &instances,
            &mut rng,
            &mut transcript,
        )
        .map_err(|error| {
            AssetOrchardError::new(
                "asset_orchard_transfer_proof_create_failed",
                error.to_string(),
            )
        })?;
        Ok(transcript.finalize())
    }
}

/// What a wallet asks for: spend one or two of its notes, pay `payment` to a shielded address
/// and/or send `exit` to a public account, and keep the rest as change.
pub struct AssetOrchardTransferRequest<'a> {
    pub chain_id: &'a str,
    pub genesis_hash: [u8; 32],
    pub protocol_version: u32,
    /// One or two notes of the same asset, owned by the caller.
    pub input_notes: &'a [AssetOrchardWalletNote],
    /// The seed whose address receives the change.
    pub change_seed_hex: &'a str,
    pub payment: Option<(AssetOrchardShieldedAddress, u64)>,
    pub exit: Option<AssetOrchardTransferExit>,
    /// Every output commitment of the pool, in tree order.
    pub pool_output_commitments: &'a [String],
}

#[derive(Debug, Clone)]
pub struct AssetOrchardTransferBuildResult {
    pub action: AssetOrchardTransferAction,
    /// The sender's change, when it is more than zero.
    pub change_note: Option<AssetOrchardWalletNote>,
    pub anchor: AssetOrchardFieldElement,
}

fn random_rseed() -> [u8; crate::asset_orchard::ASSET_ORCHARD_RSEED_BYTES] {
    let mut rseed = [0u8; crate::asset_orchard::ASSET_ORCHARD_RSEED_BYTES];
    OsRng.fill_bytes(&mut rseed);
    rseed
}

fn signing_key_from_scalar(
    scalar: pallas::Scalar,
) -> Result<SigningKey<SpendAuth>, AssetOrchardError> {
    SigningKey::<SpendAuth>::try_from(crate::asset_orchard::scalar_enc(scalar)).map_err(|_| {
        AssetOrchardError::new(
            "invalid_asset_orchard_spend_auth_signing_key",
            "asset-orchard spend authorization key is invalid",
        )
    })
}

/// A transfer with everything but its proof and signatures.
struct AssetOrchardTransferDraft {
    circuit: AssetOrchardTransferCircuit,
    public_fields: AssetOrchardTransferPublicFields,
    input_witnesses: [AssetOrchardSwapNoteWitness; ASSET_ORCHARD_LEG_COUNT],
    signing_keys: Vec<SigningKey<SpendAuth>>,
    encrypted_outputs: Vec<AssetOrchardBoundedBytes>,
    exit: Option<AssetOrchardTransferExit>,
    change_note: Option<AssetOrchardWalletNote>,
    anchor: AssetOrchardFieldElement,
}

pub fn build_asset_orchard_transfer_action(
    request: &AssetOrchardTransferRequest<'_>,
) -> Result<AssetOrchardTransferBuildResult, AssetOrchardError> {
    let draft = draft_asset_orchard_transfer(request)?;
    let proof = AssetOrchardTransferProvingKey::cached()?.create_proof(&draft.circuit, OsRng)?;
    finish_asset_orchard_transfer(request, draft, proof)
}

fn draft_asset_orchard_transfer(
    request: &AssetOrchardTransferRequest<'_>,
) -> Result<AssetOrchardTransferDraft, AssetOrchardError> {
    let AssetOrchardTransferRequest {
        chain_id,
        genesis_hash,
        protocol_version,
        input_notes,
        change_seed_hex,
        payment,
        exit,
        pool_output_commitments,
    } = request;
    let (chain_id, genesis_hash, protocol_version) = (*chain_id, *genesis_hash, *protocol_version);
    if input_notes.is_empty() || input_notes.len() > ASSET_ORCHARD_LEG_COUNT {
        return Err(AssetOrchardError::new(
            "asset_orchard_transfer_input_count",
            "a transfer spends one or two notes",
        ));
    }
    if payment.is_none() && exit.is_none() {
        return Err(AssetOrchardError::new(
            "asset_orchard_transfer_nothing_to_send",
            "a transfer needs a shielded payment, a public exit, or both",
        ));
    }
    let pool_domain =
        AssetOrchardSwapAction::expected_pool_domain(chain_id, genesis_hash, protocol_version)?;
    let asset_id = input_notes[0].asset_id.clone();
    let asset_tag = AssetTag::derive(&asset_id)?;
    for note in input_notes.iter() {
        validate_wallet_note_for_swap(note, pool_domain)?;
        if note.asset_id != asset_id {
            return Err(AssetOrchardError::new(
                "asset_orchard_transfer_mixed_assets",
                "every note of a transfer must carry the same asset",
            ));
        }
    }
    if input_notes.len() == 2 && input_notes[0].output_commitment == input_notes[1].output_commitment
    {
        return Err(AssetOrchardError::new(
            "duplicate_asset_orchard_input_note",
            "asset-orchard transfer inputs must be distinct notes",
        ));
    }
    let payment_amount = payment.as_ref().map_or(0, |(_, amount)| *amount);
    if payment.is_some() && payment_amount == 0 {
        return Err(AssetOrchardError::new(
            "zero_asset_orchard_transfer_payment",
            "a shielded payment must be more than zero",
        ));
    }
    let exit_amount = match exit {
        Some(exit) => {
            exit.validate()?;
            if exit.asset_id != asset_id {
                return Err(AssetOrchardError::new(
                    "asset_orchard_transfer_exit_asset_mismatch",
                    "the public exit asset is not the asset of the notes",
                ));
            }
            exit.amount
        }
        None => 0,
    };
    let total_in: u128 = input_notes.iter().map(|note| u128::from(note.value)).sum();
    let spent = u128::from(payment_amount) + u128::from(exit_amount);
    if spent > total_in {
        return Err(AssetOrchardError::new(
            "asset_orchard_transfer_insufficient_notes",
            format!("the chosen notes hold {total_in}, less than the {spent} to send"),
        ));
    }
    let change = u64::try_from(total_in - spent).map_err(|_| {
        AssetOrchardError::new(
            "asset_orchard_transfer_change_overflow",
            "the change does not fit one note",
        )
    })?;

    // Inputs. A single-note payment gets a zero-value filler with fresh keys, so every transfer
    // publishes two nullifiers and looks the same.
    let mut input_witnesses = Vec::with_capacity(ASSET_ORCHARD_LEG_COUNT);
    let mut signing_keys = Vec::with_capacity(ASSET_ORCHARD_LEG_COUNT);
    let mut anchor = None;
    for note in input_notes.iter() {
        let (note_anchor, merkle_witness) = asset_orchard_merkle_witness_from_commitments(
            pool_output_commitments,
            note.output_commitment.as_hex(),
        )?;
        anchor = Some(note_anchor);
        let signing_key = asset_orchard_spend_signing_key(note)?;
        input_witnesses.push(wallet_note_to_swap_witness(
            pool_domain,
            note,
            Some(merkle_witness),
            Some(spend_authority_from_wallet_note(note, &signing_key)?),
        )?);
        signing_keys.push(signing_key);
    }
    let anchor = anchor.expect("at least one input note");
    if input_witnesses.len() == 1 {
        let mut filler_seed = [0u8; 32];
        OsRng.fill_bytes(&mut filler_seed);
        let filler_keys = AssetOrchardShieldedKeys::from_seed(&bytes_to_hex(&filler_seed))?;
        let filler_rho = crate::asset_orchard::hash_to_pallas_base(
            "postfiat.asset_orchard.transfer.filler_rho.v1",
            &filler_seed,
        )?;
        let filler_note = asset_orchard_note_for_address(
            &filler_keys.address,
            asset_tag,
            0,
            filler_rho,
            &random_rseed(),
        )?;
        let signing_key = signing_key_from_scalar(filler_keys.spend_scalar)?;
        let mut witness = transfer_note_witness(pool_domain, filler_note, filler_keys.nk)?;
        witness.merkle_witness = Some(AssetOrchardMerkleWitness {
            position: 0,
            auth_path: [pallas::Base::ZERO; ASSET_ORCHARD_MERKLE_DEPTH],
        });
        witness.spend_authority = Some(AssetOrchardSpendAuthorityWitness {
            ak: spend_auth_verification_key_affine(&VerificationKey::from(&signing_key))?.into(),
            alpha: random_pallas_scalar_nonzero().into(),
            rivk: filler_keys.rivk.into(),
        });
        input_witnesses.push(witness);
        signing_keys.push(signing_key);
    }
    let input_witnesses: [AssetOrchardSwapNoteWitness; ASSET_ORCHARD_LEG_COUNT] = input_witnesses
        .try_into()
        .map_err(|_| AssetOrchardError::new("asset_orchard_transfer_input_count", "two inputs"))?;

    let anchor_field = anchor.to_field()?;
    let nullifiers = asset_orchard_input_nullifiers(pool_domain, &input_witnesses)?;
    let randomized_keys = note_swap_randomized_verification_keys(&input_witnesses)?;

    // Outputs: the payment (or a zero-value filler to ourselves), then the change.
    let change_keys = AssetOrchardShieldedKeys::from_seed(change_seed_hex)?;
    let destinations = [
        match payment {
            Some((address, amount)) => (*address, *amount),
            None => (change_keys.address, 0),
        },
        (change_keys.address, change),
    ];
    let mut output_witnesses = Vec::with_capacity(ASSET_ORCHARD_LEG_COUNT);
    let mut encrypted_outputs = Vec::with_capacity(ASSET_ORCHARD_LEG_COUNT);
    let mut change_note = None;
    for (index, (address, value)) in destinations.iter().enumerate() {
        let rho = asset_output_rho(
            pool_domain,
            anchor_field,
            nullifiers,
            randomized_keys,
            index as u8,
        )?;
        let rseed = random_rseed();
        let note = asset_orchard_note_for_address(address, asset_tag, *value, rho, &rseed)?;
        let witness = transfer_note_witness(pool_domain, note.clone(), pallas::Base::ONE)?;
        let commitment = AssetOrchardFieldElement::from_field(witness.cmx);
        if pool_output_commitments
            .iter()
            .any(|existing| existing == commitment.as_hex())
        {
            return Err(AssetOrchardError::new(
                "asset_orchard_output_already_exists",
                "asset-orchard transfer output commitment already exists in the pool",
            ));
        }
        encrypted_outputs.push(crate::encrypt_asset_orchard_transfer_output(
            chain_id,
            genesis_hash,
            protocol_version,
            address,
            commitment.as_hex(),
            &asset_id,
            *value,
            &rseed,
            rho,
        )?);
        if index == 1 && *value != 0 {
            change_note = Some(change_keys.wallet_note(pool_domain, &asset_id, &note, &rseed)?);
        }
        output_witnesses.push(witness);
    }
    let output_witnesses: [AssetOrchardSwapNoteWitness; ASSET_ORCHARD_LEG_COUNT] =
        output_witnesses.try_into().map_err(|_| {
            AssetOrchardError::new("asset_orchard_transfer_output_count", "two outputs")
        })?;

    let exit_binding_hash = match exit {
        Some(exit) => asset_orchard_transfer_exit_binding_hash(
            chain_id,
            genesis_hash,
            protocol_version,
            pool_domain,
            exit,
        )?,
        None => [0u8; crate::asset_orchard::ASSET_ORCHARD_SWAP_BINDING_HASH_BYTES],
    };
    let public_fields = AssetOrchardTransferPublicFields {
        pool_domain,
        anchor: anchor_field,
        nullifiers,
        randomized_verification_keys: randomized_keys,
        output_commitments: [output_witnesses[0].cmx, output_witnesses[1].cmx],
        encrypted_output_hashes: [
            encrypted_output_hash(0, &encrypted_outputs[0].to_bytes()?)?,
            encrypted_output_hash(1, &encrypted_outputs[1].to_bytes()?)?,
        ],
        exit_asset_tag_lo: if exit_amount == 0 { 0 } else { asset_tag.lo },
        exit_asset_tag_hi: if exit_amount == 0 { 0 } else { asset_tag.hi },
        exit_amount,
        exit_binding_hash,
    };
    let circuit = AssetOrchardTransferCircuit::new(
        input_witnesses.clone(),
        output_witnesses,
        &public_fields,
    )?;
    Ok(AssetOrchardTransferDraft {
        circuit,
        public_fields,
        input_witnesses,
        signing_keys,
        encrypted_outputs,
        exit: exit.clone(),
        change_note,
        anchor,
    })
}

fn finish_asset_orchard_transfer(
    request: &AssetOrchardTransferRequest<'_>,
    draft: AssetOrchardTransferDraft,
    proof: Vec<u8>,
) -> Result<AssetOrchardTransferBuildResult, AssetOrchardError> {
    let AssetOrchardTransferDraft {
        circuit: _,
        public_fields,
        input_witnesses,
        signing_keys,
        encrypted_outputs,
        exit,
        change_note,
        anchor,
    } = draft;
    let placeholder = AssetOrchardSpendAuthSignature::from_orchard(
        &signing_keys[0].sign(OsRng, b"asset-orchard-placeholder"),
    );
    let mut action = AssetOrchardTransferAction {
        version: ASSET_ORCHARD_ACTION_VERSION_V1,
        schema: ASSET_ORCHARD_TRANSFER_ACTION_SCHEMA_V1.to_string(),
        pool_id: ASSET_ORCHARD_POOL_ID_V1.to_string(),
        proof_system_id: ASSET_ORCHARD_PROOF_SYSTEM_ID_V1.to_string(),
        circuit_id: ASSET_ORCHARD_TRANSFER_CIRCUIT_ID_V1.to_string(),
        pool_domain: AssetOrchardFieldElement::from_field(public_fields.pool_domain),
        anchor: anchor.clone(),
        nullifiers: public_fields
            .nullifiers
            .into_iter()
            .map(AssetOrchardFieldElement::from_field)
            .collect(),
        randomized_verification_keys: input_witnesses
            .iter()
            .map(|input| {
                AssetOrchardPoint::from_affine(randomized_verification_key_point(input)?)
            })
            .collect::<Result<Vec<_>, _>>()?,
        output_commitments: public_fields
            .output_commitments
            .into_iter()
            .map(AssetOrchardFieldElement::from_field)
            .collect(),
        encrypted_outputs,
        exit,
        exit_binding_hash: AssetOrchardSwapBindingHash::from_bytes(
            &public_fields.exit_binding_hash,
        ),
        action_binding_hash: AssetOrchardSwapBindingHash::from_bytes(
            &transfer_action_binding_hash(&public_fields)?,
        ),
        proof: AssetOrchardProofBytes::from_bytes(&proof)?,
        spend_authorization_signatures: vec![placeholder.clone(), placeholder],
    };
    let sighash = action.sighash(
        request.chain_id,
        request.genesis_hash,
        request.protocol_version,
    )?;
    action.spend_authorization_signatures = signing_keys
        .iter()
        .zip(input_witnesses.iter())
        .map(|(key, input)| {
            let alpha = *input
                .spend_authority
                .as_ref()
                .ok_or_else(|| {
                    AssetOrchardError::new(
                        "missing_asset_orchard_spend_authority",
                        "asset-orchard input witness is missing spend authority",
                    )
                })?
                .alpha;
            Ok(AssetOrchardSpendAuthSignature::from_orchard(
                &key.randomize(&alpha).sign(OsRng, &sighash),
            ))
        })
        .collect::<Result<Vec<_>, AssetOrchardError>>()?;
    action.validate()?;
    Ok(AssetOrchardTransferBuildResult {
        action,
        change_note,
        anchor,
    })
}

/// Everything a verifier checks that does not need chain state: shape, bindings, signatures and
/// the proof. The caller still checks the anchor and the nullifiers against its ledger.
pub fn verify_asset_orchard_transfer_action(
    chain_id: &str,
    genesis_hash: [u8; 32],
    protocol_version: u32,
    action: &AssetOrchardTransferAction,
) -> Result<AssetOrchardTransferPublicFields, AssetOrchardError> {
    let fields = action.public_fields()?;
    action.verify_spend_authorizations(chain_id, genesis_hash, protocol_version)?;
    AssetOrchardTransferVerifyingKey::cached()?
        .verify_proof(&action.proof.to_bytes()?, &fields.public_instance()?)?;
    Ok(fields)
}

#[cfg(test)]
#[path = "asset_orchard_transfer_tests.rs"]
mod transfer_tests;
