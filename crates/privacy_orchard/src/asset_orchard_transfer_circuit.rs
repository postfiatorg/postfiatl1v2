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
    "6764003f26308e55114b10c4b338d6e3bb477be2177618e95d1ec59db148a2a19e1a4bdbf57a3d90e199bf687c7410fa";

/// The verifying key's pinned assembly, built once from the circuit by
/// `write_the_transfer_verifying_key_artifact` and shipped in the binary, so a validator loads
/// the key at start without rebuilding it. A wrong or damaged artifact
/// reconstructs a key with another fingerprint and is refused.
const ASSET_ORCHARD_TRANSFER_VK_EMBEDDED_ARTIFACT: &[u8] =
    include_bytes!("../artifacts/asset_orchard_transfer_vk_pinned_assembly.v1.bin");

static ASSET_ORCHARD_TRANSFER_VERIFYING_KEY: OnceLock<
    Result<AssetOrchardTransferVerifyingKey, AssetOrchardError>,
> = OnceLock::new();
static ASSET_ORCHARD_TRANSFER_PROVING_KEY: OnceLock<
    Result<AssetOrchardTransferProvingKey, AssetOrchardError>,
> = OnceLock::new();

use crate::asset_orchard::{
    asset_orchard_note_for_address, asset_orchard_transfer_entry_binding_hash,
    asset_orchard_transfer_exit_binding_hash, AssetOrchardTransferEntry,
    ASSET_ORCHARD_NOTE_MESSAGE_PADDED_BITS,
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
    q_transfer_public_asset: Selector,
    q_transfer_anchor: Selector,
    /// The chained accumulator every decomposition and packing in this circuit uses.
    q_chain: Selector,
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
        let total_in = u128::from(inputs[0].note.value)
            + u128::from(inputs[1].note.value)
            + u128::from(public_fields.entry_amount);
        let total_out = u128::from(outputs[0].note.value)
            + u128::from(outputs[1].note.value)
            + u128::from(public_fields.exit_amount);
        if total_in != total_out {
            return Err(AssetOrchardError::new(
                "asset_orchard_transfer_not_balanced",
                "transfer inputs plus the entry do not equal outputs plus the exit",
            ));
        }
        if total_in == 0 {
            return Err(AssetOrchardError::new(
                "asset_orchard_transfer_moves_nothing",
                "a transfer must spend or shield a positive amount",
            ));
        }
        if (public_fields.exit_amount != 0 || public_fields.entry_amount != 0)
            && (tag.lo != public_fields.public_asset_tag_lo
                || tag.hi != public_fields.public_asset_tag_hi)
        {
            return Err(AssetOrchardError::new(
                "asset_orchard_transfer_public_asset_mismatch",
                "the public entry or exit asset is not the asset of the notes",
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

// Public instance rows. Rows 0 to 23 feed the action hash in rows 24 and 25.
const ROW_POOL_DOMAIN: usize = 0;
const ROW_ANCHOR: usize = 1;
const ROW_NULLIFIER_0: usize = 2;
const ROW_RK_0: usize = 4;
const ROW_OUTPUT_0: usize = 8;
const ROW_PUBLIC_TAG_LO: usize = 16;
const ROW_PUBLIC_TAG_HI: usize = 17;
const ROW_EXIT_AMOUNT: usize = 18;
const ROW_ENTRY_AMOUNT: usize = 19;
const ROW_ACTION_HASH: usize = 24;

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
        // The swap configuration supplies the columns and the ECC, Sinsemilla, Merkle and
        // Poseidon chips. Of its own gates this circuit enables only the nonzero-tag gate and
        // the distinct-public-rows gate; its bit accumulator gate is never enabled here.
        let base = AssetOrchardSwapConservationCircuit::configure(meta);
        let advice = base.advice;
        let q_transfer_tags = meta.selector();
        let q_transfer_value = meta.selector();
        let q_transfer_public_asset = meta.selector();
        let q_transfer_anchor = meta.selector();
        let q_chain = meta.selector();

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

        // in0 + in1 + entry = out0 + out1 + exit, and the left side is not zero, so every
        // accepted transfer spends or shields something.
        meta.create_gate("asset-orchard transfer value balance", |meta| {
            let q = meta.query_selector(q_transfer_value);
            let in0 = meta.query_advice(advice[1], Rotation::cur());
            let in1 = meta.query_advice(advice[2], Rotation::cur());
            let out0 = meta.query_advice(advice[3], Rotation::cur());
            let out1 = meta.query_advice(advice[4], Rotation::cur());
            let exit_amount = meta.query_advice(advice[5], Rotation::cur());
            let entry_amount = meta.query_advice(advice[6], Rotation::cur());
            let inverse = meta.query_advice(advice[7], Rotation::cur());
            let total_in = in0 + in1 + entry_amount;
            vec![
                q.clone() * (total_in.clone() - out0 - out1 - exit_amount),
                q * (total_in * inverse - Expression::Constant(pallas::Base::ONE)),
            ]
        });

        // A nonzero public entry or exit names the asset: the hidden tag must be the public
        // one. Both amounts are below 2^64, so their sum is zero only when both are.
        meta.create_gate("asset-orchard transfer public asset", |meta| {
            let q = meta.query_selector(q_transfer_public_asset);
            let tag_lo = meta.query_advice(advice[0], Rotation::cur());
            let tag_hi = meta.query_advice(advice[1], Rotation::cur());
            let public_tag_lo = meta.query_advice(advice[2], Rotation::cur());
            let public_tag_hi = meta.query_advice(advice[3], Rotation::cur());
            let exit_amount = meta.query_advice(advice[4], Rotation::cur());
            let entry_amount = meta.query_advice(advice[5], Rotation::cur());
            let moved = exit_amount + entry_amount;
            vec![
                q.clone() * moved.clone() * (tag_lo - public_tag_lo),
                q * moved * (tag_hi - public_tag_hi),
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

        // Each row adds one weighted bit to a running sum held in ONE column, so row i's
        // result is by construction row i+1's start. Row 0's start is copied from the
        // constant zero by the callers.
        meta.create_gate("asset-orchard transfer chained bit sum", |meta| {
            let q = meta.query_selector(q_chain);
            let bit = meta.query_advice(advice[0], Rotation::cur());
            let sum = meta.query_advice(advice[1], Rotation::cur());
            let next_sum = meta.query_advice(advice[1], Rotation::next());
            let weight = meta.query_fixed(base.fixed[1]);
            vec![
                q.clone() * bit.clone() * (bit.clone() - Expression::Constant(pallas::Base::ONE)),
                q * (next_sum - sum - bit * weight),
            ]
        });

        AssetOrchardTransferConfig {
            base,
            q_transfer_tags,
            q_transfer_value,
            q_transfer_public_asset,
            q_transfer_anchor,
            q_chain,
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
                for row in [ROW_NULLIFIER_0, ROW_OUTPUT_0] {
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
        // The public amounts are below 2^64, which the value-balance gate relies on.
        for row in [ROW_EXIT_AMOUNT, ROW_ENTRY_AMOUNT] {
            transfer_bits(
                &mut layouter,
                &config,
                "transfer public amount bits",
                &public_cells[row],
                ASSET_ORCHARD_VALUE_BITS,
            )?;
        }
        // The pool domain is the same for all four notes; decompose it once.
        let pool_domain_bits = transfer_bits(
            &mut layouter,
            &config,
            "transfer pool domain bits",
            &public_cells[ROW_POOL_DOMAIN],
            255,
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
                public_cells[ROW_EXIT_AMOUNT].copy_advice(
                    || "transfer exit amount",
                    &mut region,
                    base.advice[5],
                    2,
                )?;
                public_cells[ROW_ENTRY_AMOUNT].copy_advice(
                    || "transfer entry amount",
                    &mut region,
                    base.advice[6],
                    2,
                )?;
                let total_in = legs[0][2]
                    .zip(legs[1][2])
                    .map(|(a, b)| a + b + public_values[ROW_ENTRY_AMOUNT]);
                region.assign_advice(
                    || "transfer total-in inverse",
                    base.advice[7],
                    2,
                    || total_in.map(invert_or_zero),
                )?;
                Ok(cells)
            },
        )?;

        layouter.assign_region(
            || "asset-orchard transfer public asset",
            |mut region| {
                config.q_transfer_public_asset.enable(&mut region, 0)?;
                leg_cells[0][0].copy_advice(|| "hidden tag lo", &mut region, base.advice[0], 0)?;
                leg_cells[0][1].copy_advice(|| "hidden tag hi", &mut region, base.advice[1], 0)?;
                public_cells[ROW_PUBLIC_TAG_LO].copy_advice(
                    || "public tag lo",
                    &mut region,
                    base.advice[2],
                    0,
                )?;
                public_cells[ROW_PUBLIC_TAG_HI].copy_advice(
                    || "public tag hi",
                    &mut region,
                    base.advice[3],
                    0,
                )?;
                public_cells[ROW_EXIT_AMOUNT].copy_advice(
                    || "exit amount",
                    &mut region,
                    base.advice[4],
                    0,
                )?;
                public_cells[ROW_ENTRY_AMOUNT].copy_advice(
                    || "entry amount",
                    &mut region,
                    base.advice[5],
                    0,
                )?;
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
        for (offset, cell) in h_action_cells.action_context.iter().enumerate() {
            layouter.constrain_instance(cell.cell(), base.instance, ROW_ACTION_HASH + offset)?;
        }

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
                &pool_domain_bits,
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
                &pool_domain_bits,
                &leg_cells[2 + index],
                TransferNoteRole::Output { index },
            )?;
        }
        Ok(())
    }
}

/// Decompose `target` into `bit_len` bits, least significant first. The bits are Boolean and
/// their weighted sum, accumulated in one column from a constant zero, equals `target`. This
/// bounds `target` below 2^bit_len when bit_len is under the field size. For 255 bits it fixes
/// the bits only up to adding the field modulus; see the note at the callers.
fn transfer_bits(
    layouter: &mut impl Layouter<pallas::Base>,
    config: &AssetOrchardTransferConfig,
    label: &str,
    target: &TransferCell,
    bit_len: usize,
) -> Result<Vec<TransferCell>, Error> {
    let base = &config.base;
    layouter.assign_region(
        || label.to_string(),
        |mut region| {
            let mut sum = region.assign_advice_from_constant(
                || "bit sum start",
                base.advice[1],
                0,
                pallas::Base::ZERO,
            )?;
            let mut bits = Vec::with_capacity(bit_len);
            for index in 0..bit_len {
                config.q_chain.enable(&mut region, index)?;
                let weight = two_pow(index);
                region.assign_fixed(
                    || "bit weight",
                    base.fixed[1],
                    index,
                    || Value::known(weight),
                )?;
                let bit_value = bit_from_field_value(target.value().copied(), index);
                let bit =
                    region.assign_advice(|| "bit", base.advice[0], index, || bit_value)?;
                let next_value = sum
                    .value()
                    .copied()
                    .zip(bit_value)
                    .map(|(sum, bit)| sum + bit * weight);
                sum = region.assign_advice(|| "bit sum", base.advice[1], index + 1, || next_value)?;
                bits.push(bit);
            }
            let copy =
                target.copy_advice(|| "decomposed cell", &mut region, base.advice[2], bit_len)?;
            region.constrain_equal(sum.cell(), copy.cell())?;
            Ok(bits)
        },
    )
}

/// The weighted sum of already-constrained bits, least significant first, as a cell that the
/// chained gate ties to exactly those bits.
fn transfer_pack(
    layouter: &mut impl Layouter<pallas::Base>,
    config: &AssetOrchardTransferConfig,
    label: &str,
    bits: &[TransferCell],
) -> Result<TransferCell, Error> {
    if bits.is_empty() || bits.len() > 250 {
        return Err(Error::Synthesis);
    }
    let base = &config.base;
    layouter.assign_region(
        || label.to_string(),
        |mut region| {
            let mut sum = region.assign_advice_from_constant(
                || "packed sum start",
                base.advice[1],
                0,
                pallas::Base::ZERO,
            )?;
            for (index, source) in bits.iter().enumerate() {
                config.q_chain.enable(&mut region, index)?;
                let weight = two_pow(index);
                region.assign_fixed(
                    || "packed bit weight",
                    base.fixed[1],
                    index,
                    || Value::known(weight),
                )?;
                let bit = source.copy_advice(|| "packed bit", &mut region, base.advice[0], index)?;
                let next_value = sum
                    .value()
                    .copied()
                    .zip(bit.value().copied())
                    .map(|(sum, bit)| sum + bit * weight);
                sum = region.assign_advice(
                    || "packed sum",
                    base.advice[1],
                    index + 1,
                    || next_value,
                )?;
            }
            Ok(sum)
        },
    )
}

/// A Sinsemilla commitment over `bits`, split into 250-bit pieces. Each piece the hash
/// consumes is constrained equal to the packed sum of its own bits.
#[allow(clippy::too_many_arguments)]
fn transfer_commit(
    layouter: &mut impl Layouter<pallas::Base>,
    config: &AssetOrchardTransferConfig,
    sinsemilla_chip: AssetOrchardSinsemillaChip,
    ecc_chip: AssetOrchardEccChip,
    domain: AssetOrchardCommitDomain,
    label: &str,
    bits: &[TransferCell],
    blinding: Value<pallas::Scalar>,
) -> Result<halo2_gadgets::ecc::X<pallas::Affine, AssetOrchardEccChip>, Error> {
    let base = &config.base;
    let mut pieces = Vec::new();
    for (index, chunk) in bits.chunks(ASSET_ORCHARD_NOTE_MESSAGE_PIECE_BITS).enumerate() {
        let packed = transfer_pack(layouter, config, &format!("{label} piece {index} sum"), chunk)?;
        let piece = witness_asset_orchard_message_piece(
            layouter,
            sinsemilla_chip.clone(),
            &format!("{label} piece {index}"),
            packed.value(),
            chunk.len(),
        )?;
        let piece_cell = piece.inner().cell_value();
        layouter.assign_region(
            || format!("{label} piece {index} binding"),
            |mut region| {
                let left = packed.copy_advice(|| "packed bits", &mut region, base.advice[0], 0)?;
                let right =
                    piece_cell.copy_advice(|| "hashed piece", &mut region, base.advice[1], 0)?;
                region.constrain_equal(left.cell(), right.cell())
            },
        )?;
        pieces.push(piece);
    }
    synthesize_sinsemilla_commitment_from_bound_pieces(
        layouter,
        sinsemilla_chip,
        ecc_chip,
        domain,
        label,
        pieces,
        blinding,
    )
}

#[derive(Copy, Clone)]
enum TransferNoteRole {
    Input { index: usize },
    Output { index: usize },
}

/// Witness a point, tie its x coordinate to `x_cell` and its y parity to `sign_cell`.
fn transfer_encoded_point(
    layouter: &mut impl Layouter<pallas::Base>,
    config: &AssetOrchardTransferConfig,
    ecc_chip: AssetOrchardEccChip,
    label: &str,
    point: pallas::Affine,
    x_cell: &TransferCell,
    sign_cell: &TransferCell,
) -> Result<NonIdentityPoint<pallas::Affine, AssetOrchardEccChip>, Error> {
    let base = &config.base;
    let point = NonIdentityPoint::new(
        ecc_chip,
        layouter.namespace(|| format!("{label} point")),
        Value::known(point),
    )?;
    // The parity is that of the 255-bit decomposition. A second decomposition of the same y
    // exists when y + p fits in 255 bits and has the opposite parity. Choosing it only makes
    // the commitment name the negated point, an address nobody holds keys for.
    let y_bits = transfer_bits(
        layouter,
        config,
        &format!("{label} y bits"),
        &point.inner().y(),
        255,
    )?;
    layouter.assign_region(
        || format!("{label} encoding binding"),
        |mut region| {
            let point_x = point.inner().x().copy_advice(|| "point x", &mut region, base.advice[0], 0)?;
            let encoded_x = x_cell.copy_advice(|| "encoded x", &mut region, base.advice[1], 0)?;
            region.constrain_equal(point_x.cell(), encoded_x.cell())?;
            let parity = y_bits[0].copy_advice(|| "y parity", &mut region, base.advice[2], 0)?;
            let sign = sign_cell.copy_advice(|| "encoded sign", &mut region, base.advice[3], 0)?;
            region.constrain_equal(parity.cell(), sign.cell())
        },
    )?;
    Ok(point)
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
    pool_domain_bits: &[TransferCell],
    leg_cells: &[TransferCell],
    role: TransferNoteRole,
) -> Result<(), Error> {
    let base = &config.base;
    let pool_domain = public_values[ROW_POOL_DOMAIN];
    let pool_domain_cell = public_cells[ROW_POOL_DOMAIN].clone();
    let (gd_x, gd_y_sign) = point_encoding_fields(*note.note.g_d)?;
    let (pkd_x, pkd_y_sign) = point_encoding_fields(*note.note.pk_d)?;
    // The note's private fields. Every later use copies from these cells.
    let [gd_x_cell, gd_sign_cell, pkd_x_cell, pkd_sign_cell, rho_cell, psi_cell, nk_cell] =
        layouter.assign_region(
            || format!("{label} private fields"),
            |mut region| {
                let values = [
                    gd_x,
                    gd_y_sign,
                    pkd_x,
                    pkd_y_sign,
                    *note.note.rho,
                    *note.note.psi,
                    *note.nk,
                ];
                let mut cells = Vec::with_capacity(values.len());
                for (column, value) in values.into_iter().enumerate() {
                    cells.push(region.assign_advice(
                        || "note field",
                        base.advice[column],
                        0,
                        || Value::known(value),
                    )?);
                }
                cells.try_into().map_err(|_| Error::Synthesis)
            },
        )?;
    let zero_padding = layouter.assign_region(
        || format!("{label} padding"),
        |mut region| {
            (0..3)
                .map(|row| {
                    region.assign_advice_from_constant(
                        || "zero padding bit",
                        base.advice[0],
                        row,
                        pallas::Base::ZERO,
                    )
                })
                .collect::<Result<Vec<_>, Error>>()
        },
    )?;
    let mut bits = |name: &str, cell: &TransferCell, bit_len: usize| {
        transfer_bits(layouter, config, &format!("{label} {name} bits"), cell, bit_len)
    };
    let tag_lo_bits = bits("asset tag lo", &leg_cells[0], ASSET_ORCHARD_ASSET_TAG_BITS)?;
    let tag_hi_bits = bits("asset tag hi", &leg_cells[1], ASSET_ORCHARD_ASSET_TAG_BITS)?;
    let value_bits = bits("value", &leg_cells[2], ASSET_ORCHARD_VALUE_BITS)?;
    let gd_x_bits = bits("g_d x", &gd_x_cell, 255)?;
    let gd_sign_bits = bits("g_d sign", &gd_sign_cell, 1)?;
    let pkd_x_bits = bits("pk_d x", &pkd_x_cell, 255)?;
    let pkd_sign_bits = bits("pk_d sign", &pkd_sign_cell, 1)?;
    let rho_bits = bits("rho", &rho_cell, 255)?;
    let psi_bits = bits("psi", &psi_cell, 255)?;

    let mut message = Vec::with_capacity(ASSET_ORCHARD_NOTE_MESSAGE_PADDED_BITS);
    for segment in asset_note_message_segments() {
        let source: &[TransferCell] = match segment.source {
            AssetNoteMessageSource::PoolDomain => pool_domain_bits,
            AssetNoteMessageSource::AssetTagLo => &tag_lo_bits,
            AssetNoteMessageSource::AssetTagHi => &tag_hi_bits,
            AssetNoteMessageSource::GdX => &gd_x_bits,
            AssetNoteMessageSource::GdYSign => &gd_sign_bits,
            AssetNoteMessageSource::PkdX => &pkd_x_bits,
            AssetNoteMessageSource::PkdYSign => &pkd_sign_bits,
            AssetNoteMessageSource::Value => &value_bits,
            AssetNoteMessageSource::Rho => &rho_bits,
            AssetNoteMessageSource::Psi => &psi_bits,
            AssetNoteMessageSource::Padding => &zero_padding,
        };
        message.extend_from_slice(
            &source[segment.source_bit_offset..segment.source_bit_offset + segment.bit_len],
        );
    }
    if message.len() != ASSET_ORCHARD_NOTE_MESSAGE_PADDED_BITS {
        return Err(Error::Synthesis);
    }
    let cmx = transfer_commit(
        layouter,
        config,
        sinsemilla_chip.clone(),
        ecc_chip.clone(),
        AssetOrchardCommitDomain::NoteCommit,
        &format!("{label} commitment"),
        &message,
        Value::known(*note.note.rcm),
    )?;

    match role {
        TransferNoteRole::Output { index } => {
            layouter.constrain_instance(cmx.inner().cell(), base.instance, ROW_OUTPUT_0 + index)?;
            synthesize_transfer_output_rho(
                layouter,
                base,
                &format!("{label} output rho"),
                public_cells,
                public_values,
                index,
                rho_cell,
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
                nk_cell.clone(),
                rho_cell,
                psi_cell,
                cmx.inner().clone(),
            )?;
            layouter.constrain_instance(nullifier.cell(), base.instance, ROW_NULLIFIER_0 + index)?;

            // Spend authority: rk = ak + [alpha]G is public, and the note's address belongs to
            // the key the same ak and nk derive: pk_d = [CommitIvk(ak, nk)] g_d.
            let authority = note.spend_authority.as_ref().ok_or(Error::Synthesis)?;
            let ak = NonIdentityPoint::new(
                ecc_chip.clone(),
                layouter.namespace(|| format!("{label} ak")),
                Value::known(*authority.ak),
            )?;
            let alpha = ScalarFixed::new(
                ecc_chip.clone(),
                layouter.namespace(|| format!("{label} alpha")),
                Value::known(*authority.alpha),
            )?;
            let spend_auth_g =
                FixedPoint::from_inner(ecc_chip.clone(), AssetOrchardFullScalarBase::SpendAuthG);
            let (alpha_commitment, _) =
                spend_auth_g.mul(layouter.namespace(|| format!("{label} [alpha]G")), alpha)?;
            let rk = alpha_commitment.add(layouter.namespace(|| format!("{label} rk")), &ak)?;
            layouter.constrain_instance(
                rk.inner().x().cell(),
                base.instance,
                ROW_RK_0 + index * 2,
            )?;
            layouter.constrain_instance(
                rk.inner().y().cell(),
                base.instance,
                ROW_RK_0 + index * 2 + 1,
            )?;
            let g_d = transfer_encoded_point(
                layouter,
                config,
                ecc_chip.clone(),
                &format!("{label} g_d"),
                *note.note.g_d,
                &gd_x_cell,
                &gd_sign_cell,
            )?;
            let pk_d = transfer_encoded_point(
                layouter,
                config,
                ecc_chip.clone(),
                &format!("{label} pk_d"),
                *note.note.pk_d,
                &pkd_x_cell,
                &pkd_sign_cell,
            )?;
            let mut ivk_message = transfer_bits(
                layouter,
                config,
                &format!("{label} ak bits"),
                &ak.inner().x(),
                255,
            )?;
            ivk_message.extend(transfer_bits(
                layouter,
                config,
                &format!("{label} nk bits"),
                &nk_cell,
                255,
            )?);
            let ivk = transfer_commit(
                layouter,
                config,
                sinsemilla_chip,
                ecc_chip.clone(),
                AssetOrchardCommitDomain::CommitIvk,
                &format!("{label} commit ivk"),
                &ivk_message,
                Value::known(*authority.rivk),
            )?;
            let ivk = ScalarVar::from_base(
                ecc_chip,
                layouter.namespace(|| format!("{label} ivk scalar")),
                ivk.inner(),
            )?;
            let (derived_pk_d, _) =
                g_d.mul(layouter.namespace(|| format!("{label} [ivk]g_d")), ivk)?;
            derived_pk_d.constrain_equal(
                layouter.namespace(|| format!("{label} pk_d equality")),
                &pk_d,
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
                    public_cells[ROW_ANCHOR].copy_advice(
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
    /// Load the shipped key and refuse it unless it is the pinned one.
    pub fn build() -> Result<Self, AssetOrchardError> {
        let params = asset_orchard_k15_params()?.clone();
        let mut cursor = Cursor::new(ASSET_ORCHARD_TRANSFER_VK_EMBEDDED_ARTIFACT);
        let assembly = VerifyingKeyPinnedAssembly::read_with_limits(
            &mut cursor,
            VerifyingKeyPinnedAssemblyLimits {
                max_fixed_commitments: 8192,
                max_permutation_commitments: 8192,
                max_selectors: 8192,
                max_selector_rows: 1usize << ASSET_ORCHARD_TRANSFER_V1_K,
            },
        )
        .map_err(|error| {
            AssetOrchardError::new("asset_orchard_transfer_vk_artifact_malformed", error.to_string())
        })?;
        if cursor.position() != ASSET_ORCHARD_TRANSFER_VK_EMBEDDED_ARTIFACT.len() as u64 {
            return Err(AssetOrchardError::new(
                "asset_orchard_transfer_vk_artifact_malformed",
                "the transfer verifying-key artifact has trailing bytes",
            ));
        }
        let vk = keygen_vk_from_pinned_assembly::<vesta::Affine, AssetOrchardTransferCircuit>(
            &params, assembly,
        )
        .map_err(|error| {
            AssetOrchardError::new(
                "asset_orchard_transfer_vk_artifact_reconstruct_failed",
                error.to_string(),
            )
        })?;
        let fingerprint = asset_orchard_transfer_runtime_pinned_vk_fingerprint(&vk);
        if fingerprint != ASSET_ORCHARD_TRANSFER_V1_RUNTIME_PINNED_VK_FINGERPRINT {
            return Err(AssetOrchardError::new(
                "asset_orchard_transfer_vk_pin_mismatch",
                format!(
                    "transfer verifying key fingerprint {fingerprint} is not the pinned {}",
                    ASSET_ORCHARD_TRANSFER_V1_RUNTIME_PINNED_VK_FINGERPRINT
                ),
            ));
        }
        Ok(Self {
            params,
            vk,
            fingerprint,
        })
    }

    /// Build the key from the circuit itself. Slow; used to make and to check the artifact.
    pub fn build_unpinned() -> Result<Self, AssetOrchardError> {
        Ok(Self::build_from_circuit()?.0)
    }

    fn build_from_circuit() -> Result<(Self, Vec<u8>), AssetOrchardError> {
        let params = asset_orchard_k15_params()?.clone();
        let (vk, assembly) =
            keygen_vk_pinned_assembly(&params, &AssetOrchardTransferCircuit::full_shape())
                .map_err(|error| {
                    AssetOrchardError::new(
                        "asset_orchard_transfer_vk_build_failed",
                        error.to_string(),
                    )
                })?;
        let mut artifact = Vec::new();
        assembly.write(&mut artifact).map_err(|error| {
            AssetOrchardError::new("asset_orchard_transfer_vk_artifact_write_failed", error.to_string())
        })?;
        let fingerprint = asset_orchard_transfer_runtime_pinned_vk_fingerprint(&vk);
        Ok((
            Self {
                params,
                vk,
                fingerprint,
            },
            artifact,
        ))
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
    /// Up to two notes of the same asset, owned by the caller. None when only shielding.
    pub input_notes: &'a [AssetOrchardWalletNote],
    /// A public amount to bring into the pool. Its asset is the asset of the transfer.
    pub entry: Option<AssetOrchardTransferEntry>,
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
    entry: Option<AssetOrchardTransferEntry>,
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
        entry,
        change_seed_hex,
        payment,
        exit,
        pool_output_commitments,
    } = request;
    let (chain_id, genesis_hash, protocol_version) = (*chain_id, *genesis_hash, *protocol_version);
    if input_notes.len() > ASSET_ORCHARD_LEG_COUNT {
        return Err(AssetOrchardError::new(
            "asset_orchard_transfer_input_count",
            "a transfer spends at most two notes",
        ));
    }
    if input_notes.is_empty() && entry.is_none() {
        return Err(AssetOrchardError::new(
            "asset_orchard_transfer_input_count",
            "a transfer spends a note, shields an amount, or both",
        ));
    }
    if entry.is_some() && exit.is_some() {
        return Err(AssetOrchardError::new(
            "asset_orchard_transfer_entry_and_exit",
            "one transfer either brings value in or takes value out, not both",
        ));
    }
    if payment.is_none() && exit.is_none() && entry.is_none() {
        return Err(AssetOrchardError::new(
            "asset_orchard_transfer_nothing_to_send",
            "a transfer needs a shielded payment, a public exit, or an amount to shield",
        ));
    }
    let pool_domain =
        AssetOrchardTransferAction::expected_pool_domain(chain_id, genesis_hash, protocol_version)?;
    let asset_id = match (input_notes.first(), entry) {
        (Some(note), _) => note.asset_id.clone(),
        (None, Some(entry)) => entry.asset_id.clone(),
        (None, None) => unreachable!("checked above"),
    };
    let entry_amount = match entry {
        Some(entry) => {
            entry.validate()?;
            if entry.asset_id != asset_id {
                return Err(AssetOrchardError::new(
                    "asset_orchard_transfer_public_asset_mismatch",
                    "the shielded asset is not the asset of the notes",
                ));
            }
            entry.amount
        }
        None => 0,
    };
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
    let total_in: u128 = input_notes
        .iter()
        .map(|note| u128::from(note.value))
        .sum::<u128>()
        + u128::from(entry_amount);
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

    // Inputs. Every transfer publishes two nullifiers, so unused input slots get zero-value
    // fillers with fresh keys and every transfer looks the same.
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
    let anchor = match anchor {
        Some(anchor) => anchor,
        // Shielding with no note to spend still names the pool's current root.
        None => asset_orchard_transfer_pool_anchor(pool_output_commitments)?,
    };
    while input_witnesses.len() < ASSET_ORCHARD_LEG_COUNT {
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
        public_asset_tag_lo: if exit_amount == 0 && entry_amount == 0 { 0 } else { asset_tag.lo },
        public_asset_tag_hi: if exit_amount == 0 && entry_amount == 0 { 0 } else { asset_tag.hi },
        exit_amount,
        entry_amount,
        exit_binding_hash,
        entry_binding_hash: match entry {
            Some(entry) => asset_orchard_transfer_entry_binding_hash(
                chain_id,
                genesis_hash,
                protocol_version,
                pool_domain,
                entry,
            )?,
            None => [0u8; crate::asset_orchard::ASSET_ORCHARD_SWAP_BINDING_HASH_BYTES],
        },
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
        entry: entry.clone(),
        change_note,
        anchor,
    })
}

/// The root a transfer names when it spends no existing note.
pub fn asset_orchard_transfer_pool_anchor(
    pool_output_commitments: &[String],
) -> Result<AssetOrchardFieldElement, AssetOrchardError> {
    let leaves = pool_output_commitments
        .iter()
        .map(|commitment| {
            AssetOrchardFieldElement::parse_hex(commitment.clone())
                .and_then(|cmx| merkle_hash_from_cmx(cmx.to_field()?))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let root = merkle_root_from_nodes(leaves)?;
    Ok(AssetOrchardFieldElement::from_field(base_from_merkle_hash(&root)?))
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
        entry,
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
        entry,
        entry_binding_hash: AssetOrchardSwapBindingHash::from_bytes(
            &public_fields.entry_binding_hash,
        ),
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
