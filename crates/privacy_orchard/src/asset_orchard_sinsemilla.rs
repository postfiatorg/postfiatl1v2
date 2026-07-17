use ff::{Field, PrimeField};
use group::Curve;
use halo2_gadgets::{
    ecc::{
        chip::{
            find_zs_and_us, BaseFieldElem, EccChip, EccConfig, FixedPoint as FixedPointTrait,
            FullScalar, ShortScalar, H, NUM_WINDOWS, NUM_WINDOWS_SHORT,
        },
        FixedPoints, ScalarFixed, X,
    },
    sinsemilla::{
        chip::{SinsemillaChip, SinsemillaConfig},
        CommitDomain, CommitDomains, HashDomains, Message, MessagePiece,
    },
    utilities::{lookup_range_check::PallasLookupRangeCheckConfig, RangeConstrained},
};
use halo2_proofs::{
    circuit::{AssignedCell, Layouter, Value},
    plonk::{Advice, Column, ConstraintSystem, Error, Fixed, Selector},
    poly::Rotation,
};
use pasta_curves::{arithmetic::CurveExt, pallas};
use std::sync::OnceLock;

use crate::asset_orchard::ASSET_ORCHARD_NOTE_COMMIT_DOMAIN_V1;

const ORCHARD_COMMIT_IVK_DOMAIN: &str = "z.cash:Orchard-CommitIvk";

pub const ASSET_ORCHARD_SINSEMILLA_K: usize = ::sinsemilla::K;
pub const ASSET_ORCHARD_NOTE_COMMIT_MAX_WORDS: usize = 200;
const ASSET_ORCHARD_SINSEMILLA_PIECE_WORDS: usize = 25;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum AssetOrchardHashDomain {
    NoteCommit,
    CommitIvk,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum AssetOrchardCommitDomain {
    NoteCommit,
    CommitIvk,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct AssetOrchardFixedBases;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct AssetOrchardCommitR;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum AssetOrchardFullScalarBase {
    NoteCommitR,
    CommitIvkR,
    SpendAuthG,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct AssetOrchardCommitRBase;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct AssetOrchardCommitRShort;

pub type AssetOrchardEccChip = EccChip<AssetOrchardFixedBases>;
pub type AssetOrchardEccConfig = EccConfig<AssetOrchardFixedBases>;
pub type AssetOrchardSinsemillaChip =
    SinsemillaChip<AssetOrchardHashDomain, AssetOrchardCommitDomain, AssetOrchardFixedBases>;
pub type AssetOrchardSinsemillaConfig = SinsemillaConfig<
    AssetOrchardHashDomain,
    AssetOrchardCommitDomain,
    AssetOrchardFixedBases,
    PallasLookupRangeCheckConfig,
>;
pub type AssetOrchardAssignedSubpiece =
    RangeConstrained<pallas::Base, AssignedCell<pallas::Base, pallas::Base>>;
pub type AssetOrchardMessagePiece = MessagePiece<
    pallas::Affine,
    AssetOrchardSinsemillaChip,
    { ::sinsemilla::K },
    { ::sinsemilla::C },
>;

#[derive(Clone, Debug)]
pub struct AssetOrchardMessagePieceConstraintConfig {
    q_accumulate: Selector,
    piece: Column<Advice>,
    subpiece: Column<Advice>,
    accumulator: Column<Advice>,
    next_accumulator: Column<Advice>,
    weight: Column<Fixed>,
}

impl AssetOrchardMessagePieceConstraintConfig {
    pub fn configure(
        meta: &mut ConstraintSystem<pallas::Base>,
        piece: Column<Advice>,
        subpiece: Column<Advice>,
        accumulator: Column<Advice>,
        next_accumulator: Column<Advice>,
        weight: Column<Fixed>,
    ) -> Self {
        for column in [piece, subpiece, accumulator, next_accumulator] {
            meta.enable_equality(column);
        }
        let q_accumulate = meta.selector();
        meta.create_gate(
            "asset-orchard message piece from assigned subpieces",
            |meta| {
                let q = meta.query_selector(q_accumulate);
                let subpiece = meta.query_advice(subpiece, Rotation::cur());
                let accumulator = meta.query_advice(accumulator, Rotation::cur());
                let next_accumulator = meta.query_advice(next_accumulator, Rotation::cur());
                let weight = meta.query_fixed(weight);

                vec![q * (next_accumulator - accumulator - subpiece * weight)]
            },
        );

        Self {
            q_accumulate,
            piece,
            subpiece,
            accumulator,
            next_accumulator,
            weight,
        }
    }
}

pub fn asset_note_commit_q() -> pallas::Affine {
    *ASSET_NOTE_COMMIT_Q.get_or_init(|| {
        let domain = format!("{ASSET_ORCHARD_NOTE_COMMIT_DOMAIN_V1}-M");
        pallas::Point::hash_to_curve(::sinsemilla::Q_PERSONALIZATION)(domain.as_bytes()).to_affine()
    })
}

pub fn asset_note_commit_r() -> pallas::Affine {
    *ASSET_NOTE_COMMIT_R.get_or_init(|| {
        let domain = format!("{ASSET_ORCHARD_NOTE_COMMIT_DOMAIN_V1}-r");
        let point = pallas::Point::hash_to_curve(&domain)(&[]).to_affine();
        point
    })
}

pub fn orchard_commit_ivk_q() -> pallas::Affine {
    *ORCHARD_COMMIT_IVK_Q.get_or_init(|| {
        let domain = format!("{ORCHARD_COMMIT_IVK_DOMAIN}-M");
        pallas::Point::hash_to_curve(::sinsemilla::Q_PERSONALIZATION)(domain.as_bytes()).to_affine()
    })
}

pub fn orchard_commit_ivk_r() -> pallas::Affine {
    *ORCHARD_COMMIT_IVK_R.get_or_init(|| {
        let domain = format!("{ORCHARD_COMMIT_IVK_DOMAIN}-r");
        let point = pallas::Point::hash_to_curve(&domain)(&[]).to_affine();
        point
    })
}

pub fn asset_spend_auth_g() -> pallas::Affine {
    *ASSET_SPEND_AUTH_G
        .get_or_init(|| pallas::Point::hash_to_curve("z.cash:Orchard")(b"G").to_affine())
}

static ASSET_NOTE_COMMIT_Q: OnceLock<pallas::Affine> = OnceLock::new();
static ASSET_NOTE_COMMIT_R: OnceLock<pallas::Affine> = OnceLock::new();
static ORCHARD_COMMIT_IVK_Q: OnceLock<pallas::Affine> = OnceLock::new();
static ORCHARD_COMMIT_IVK_R: OnceLock<pallas::Affine> = OnceLock::new();
static ASSET_SPEND_AUTH_G: OnceLock<pallas::Affine> = OnceLock::new();
static ASSET_NOTE_COMMIT_R_ZS_US: OnceLock<Vec<(u64, [pallas::Base; H])>> = OnceLock::new();
static ASSET_NOTE_COMMIT_R_ZS_US_SHORT: OnceLock<Vec<(u64, [pallas::Base; H])>> = OnceLock::new();
static ORCHARD_COMMIT_IVK_R_ZS_US: OnceLock<Vec<(u64, [pallas::Base; H])>> = OnceLock::new();
static ASSET_SPEND_AUTH_G_ZS_US: OnceLock<Vec<(u64, [pallas::Base; H])>> = OnceLock::new();

pub fn sinsemilla_piece_values_from_bits(bits: &[bool]) -> Vec<(pallas::Base, usize)> {
    assert!(bits.len() <= ASSET_ORCHARD_SINSEMILLA_K * ASSET_ORCHARD_NOTE_COMMIT_MAX_WORDS);
    let mut padded = bits.to_vec();
    let remainder = padded.len() % ASSET_ORCHARD_SINSEMILLA_K;
    if remainder != 0 {
        padded.extend(std::iter::repeat_n(
            false,
            ASSET_ORCHARD_SINSEMILLA_K - remainder,
        ));
    }

    padded
        .chunks(ASSET_ORCHARD_SINSEMILLA_PIECE_WORDS * ASSET_ORCHARD_SINSEMILLA_K)
        .map(|chunk| {
            let value = bits_to_base_field(chunk);
            let words = chunk.len() / ASSET_ORCHARD_SINSEMILLA_K;
            (value, words)
        })
        .collect()
}

pub fn synthesize_asset_note_commitment_from_pieces(
    layouter: &mut impl Layouter<pallas::Base>,
    sinsemilla_chip: AssetOrchardSinsemillaChip,
    ecc_chip: AssetOrchardEccChip,
    pieces: &[(pallas::Base, usize)],
    rcm: Value<pallas::Scalar>,
) -> Result<X<pallas::Affine, AssetOrchardEccChip>, Error> {
    let message_pieces = pieces
        .iter()
        .enumerate()
        .map(|(index, (value, words))| {
            MessagePiece::from_field_elem(
                sinsemilla_chip.clone(),
                layouter.namespace(|| format!("asset note message piece {index}")),
                Value::known(*value),
                *words,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let message = Message::from_pieces(sinsemilla_chip.clone(), message_pieces);
    let domain = CommitDomain::new(
        sinsemilla_chip,
        ecc_chip.clone(),
        &AssetOrchardCommitDomain::NoteCommit,
    );
    let rcm = ScalarFixed::new(ecc_chip, layouter.namespace(|| "asset note rcm"), rcm)?;
    let (cmx, _) =
        domain.short_commit(layouter.namespace(|| "asset note commitment"), message, rcm)?;
    Ok(cmx)
}

pub fn synthesize_asset_note_commitment_from_assigned_subpieces(
    layouter: &mut impl Layouter<pallas::Base>,
    message_piece_config: &AssetOrchardMessagePieceConstraintConfig,
    sinsemilla_chip: AssetOrchardSinsemillaChip,
    ecc_chip: AssetOrchardEccChip,
    piece_subpieces: &[Vec<AssetOrchardAssignedSubpiece>],
    rcm: Value<pallas::Scalar>,
) -> Result<X<pallas::Affine, AssetOrchardEccChip>, Error> {
    let mut message_pieces = Vec::with_capacity(piece_subpieces.len());
    for (index, subpieces) in piece_subpieces.iter().enumerate() {
        message_pieces.push(synthesize_message_piece_from_assigned_subpieces(
            layouter,
            message_piece_config,
            sinsemilla_chip.clone(),
            &format!("asset note assigned message piece {index}"),
            subpieces,
        )?);
    }
    let message = Message::from_pieces(sinsemilla_chip.clone(), message_pieces);
    synthesize_sinsemilla_commitment_from_message(
        layouter,
        sinsemilla_chip,
        ecc_chip,
        AssetOrchardCommitDomain::NoteCommit,
        "asset note commitment",
        message,
        rcm,
    )
}

pub fn synthesize_sinsemilla_commitment_from_assigned_subpieces(
    layouter: &mut impl Layouter<pallas::Base>,
    message_piece_config: &AssetOrchardMessagePieceConstraintConfig,
    sinsemilla_chip: AssetOrchardSinsemillaChip,
    ecc_chip: AssetOrchardEccChip,
    domain: AssetOrchardCommitDomain,
    label: &str,
    piece_subpieces: &[Vec<AssetOrchardAssignedSubpiece>],
    rcm: Value<pallas::Scalar>,
) -> Result<X<pallas::Affine, AssetOrchardEccChip>, Error> {
    let mut message_pieces = Vec::with_capacity(piece_subpieces.len());
    for (index, subpieces) in piece_subpieces.iter().enumerate() {
        message_pieces.push(synthesize_message_piece_from_assigned_subpieces(
            layouter,
            message_piece_config,
            sinsemilla_chip.clone(),
            &format!("{label} assigned message piece {index}"),
            subpieces,
        )?);
    }
    let message = Message::from_pieces(sinsemilla_chip.clone(), message_pieces);
    synthesize_sinsemilla_commitment_from_message(
        layouter,
        sinsemilla_chip,
        ecc_chip,
        domain,
        label,
        message,
        rcm,
    )
}

fn synthesize_sinsemilla_commitment_from_message(
    layouter: &mut impl Layouter<pallas::Base>,
    sinsemilla_chip: AssetOrchardSinsemillaChip,
    ecc_chip: AssetOrchardEccChip,
    domain: AssetOrchardCommitDomain,
    label: &str,
    message: Message<
        pallas::Affine,
        AssetOrchardSinsemillaChip,
        { ::sinsemilla::K },
        { ::sinsemilla::C },
    >,
    rcm: Value<pallas::Scalar>,
) -> Result<X<pallas::Affine, AssetOrchardEccChip>, Error> {
    let domain = CommitDomain::new(sinsemilla_chip, ecc_chip.clone(), &domain);
    let rcm = ScalarFixed::new(ecc_chip, layouter.namespace(|| "asset note rcm"), rcm)?;
    let (cmx, _) = domain.short_commit(layouter.namespace(|| label.to_string()), message, rcm)?;
    Ok(cmx)
}

fn synthesize_message_piece_from_assigned_subpieces(
    layouter: &mut impl Layouter<pallas::Base>,
    config: &AssetOrchardMessagePieceConstraintConfig,
    sinsemilla_chip: AssetOrchardSinsemillaChip,
    label: &str,
    subpieces: &[AssetOrchardAssignedSubpiece],
) -> Result<AssetOrchardMessagePiece, Error> {
    let mut total_bits = 0usize;
    let subpiece_values = subpieces
        .iter()
        .map(|subpiece| {
            if total_bits >= 64 {
                return Err(Error::Synthesis);
            }
            total_bits += subpiece.num_bits();
            Ok(subpiece.value())
        })
        .collect::<Result<Vec<_>, Error>>()?;
    if total_bits == 0 || total_bits % ASSET_ORCHARD_SINSEMILLA_K != 0 {
        return Err(Error::Synthesis);
    }

    let piece = MessagePiece::from_subpieces(
        sinsemilla_chip,
        layouter.namespace(|| format!("{label} value")),
        subpiece_values,
    )?;
    constrain_message_piece_to_assigned_subpieces(layouter, config, label, &piece, subpieces)?;
    Ok(piece)
}

fn constrain_message_piece_to_assigned_subpieces(
    layouter: &mut impl Layouter<pallas::Base>,
    config: &AssetOrchardMessagePieceConstraintConfig,
    label: &str,
    piece: &AssetOrchardMessagePiece,
    subpieces: &[AssetOrchardAssignedSubpiece],
) -> Result<(), Error> {
    let piece_cell = piece.inner().cell_value();
    constrain_assigned_message_piece_to_subpieces(layouter, config, label, &piece_cell, subpieces)
}

fn constrain_assigned_message_piece_to_subpieces(
    layouter: &mut impl Layouter<pallas::Base>,
    config: &AssetOrchardMessagePieceConstraintConfig,
    label: &str,
    piece_cell: &AssignedCell<pallas::Base, pallas::Base>,
    subpieces: &[AssetOrchardAssignedSubpiece],
) -> Result<(), Error> {
    layouter.assign_region(
        || format!("{label} assigned subpiece binding"),
        |mut region| {
            let mut bit_offset = 0usize;
            let mut accumulator_value = Value::known(pallas::Base::ZERO);
            let mut final_accumulator = None;
            for (row, subpiece) in subpieces.iter().enumerate() {
                config.q_accumulate.enable(&mut region, row)?;
                let weight = two_pow_base(bit_offset);
                let subpiece_cell = subpiece.inner().copy_advice(
                    || "message subpiece",
                    &mut region,
                    config.subpiece,
                    row,
                )?;
                let next_accumulator_value = accumulator_value
                    .zip(subpiece_cell.value().copied())
                    .map(|(accumulator, subpiece)| accumulator + subpiece * weight);
                region.assign_fixed(
                    || "message subpiece weight",
                    config.weight,
                    row,
                    || Value::known(weight),
                )?;
                region.assign_advice(
                    || "message piece accumulator",
                    config.accumulator,
                    row,
                    || accumulator_value,
                )?;
                let next_accumulator = region.assign_advice(
                    || "message piece next accumulator",
                    config.next_accumulator,
                    row,
                    || next_accumulator_value,
                )?;
                bit_offset += subpiece.num_bits();
                accumulator_value = next_accumulator_value;
                final_accumulator = Some(next_accumulator);
            }

            let final_accumulator = final_accumulator.ok_or(Error::Synthesis)?;
            let piece_cell = piece_cell.copy_advice(
                || "message piece",
                &mut region,
                config.piece,
                subpieces.len(),
            )?;
            region.constrain_equal(final_accumulator.cell(), piece_cell.cell())?;
            Ok(())
        },
    )
}

fn bits_to_base_field(bits: &[bool]) -> pallas::Base {
    bits.iter().rev().fold(pallas::Base::ZERO, |acc, bit| {
        if *bit {
            acc.double() + pallas::Base::ONE
        } else {
            acc.double()
        }
    })
}

fn two_pow_base(bit_index: usize) -> pallas::Base {
    let mut value = pallas::Base::ONE;
    for _ in 0..bit_index {
        value = value.double();
    }
    value
}

impl HashDomains<pallas::Affine> for AssetOrchardHashDomain {
    fn Q(&self) -> pallas::Affine {
        match self {
            Self::NoteCommit => asset_note_commit_q(),
            Self::CommitIvk => orchard_commit_ivk_q(),
        }
    }
}

impl CommitDomains<pallas::Affine, AssetOrchardFixedBases, AssetOrchardHashDomain>
    for AssetOrchardCommitDomain
{
    fn r(&self) -> AssetOrchardFullScalarBase {
        match self {
            Self::NoteCommit => AssetOrchardFullScalarBase::NoteCommitR,
            Self::CommitIvk => AssetOrchardFullScalarBase::CommitIvkR,
        }
    }

    fn hash_domain(&self) -> AssetOrchardHashDomain {
        match self {
            Self::NoteCommit => AssetOrchardHashDomain::NoteCommit,
            Self::CommitIvk => AssetOrchardHashDomain::CommitIvk,
        }
    }
}

impl FixedPoints<pallas::Affine> for AssetOrchardFixedBases {
    type FullScalar = AssetOrchardFullScalarBase;
    type ShortScalar = AssetOrchardCommitRShort;
    type Base = AssetOrchardCommitRBase;
}

impl FixedPointTrait<pallas::Affine> for AssetOrchardFullScalarBase {
    type FixedScalarKind = FullScalar;

    fn generator(&self) -> pallas::Affine {
        match self {
            Self::NoteCommitR => asset_note_commit_r(),
            Self::CommitIvkR => orchard_commit_ivk_r(),
            Self::SpendAuthG => asset_spend_auth_g(),
        }
    }

    fn u(&self) -> Vec<[[u8; 32]; H]> {
        fixed_u(self.generator(), NUM_WINDOWS)
    }

    fn z(&self) -> Vec<u64> {
        fixed_z(self.generator(), NUM_WINDOWS)
    }
}

impl FixedPointTrait<pallas::Affine> for AssetOrchardCommitRBase {
    type FixedScalarKind = BaseFieldElem;

    fn generator(&self) -> pallas::Affine {
        asset_note_commit_r()
    }

    fn u(&self) -> Vec<[[u8; 32]; H]> {
        fixed_u(self.generator(), NUM_WINDOWS)
    }

    fn z(&self) -> Vec<u64> {
        fixed_z(self.generator(), NUM_WINDOWS)
    }
}

impl FixedPointTrait<pallas::Affine> for AssetOrchardCommitRShort {
    type FixedScalarKind = ShortScalar;

    fn generator(&self) -> pallas::Affine {
        asset_note_commit_r()
    }

    fn u(&self) -> Vec<[[u8; 32]; H]> {
        fixed_u(self.generator(), NUM_WINDOWS_SHORT)
    }

    fn z(&self) -> Vec<u64> {
        fixed_z(self.generator(), NUM_WINDOWS_SHORT)
    }
}

fn fixed_u(generator: pallas::Affine, num_windows: usize) -> Vec<[[u8; 32]; H]> {
    fixed_zs_and_us(generator, num_windows)
        .iter()
        .map(|(_, us)| {
            [
                us[0].to_repr(),
                us[1].to_repr(),
                us[2].to_repr(),
                us[3].to_repr(),
                us[4].to_repr(),
                us[5].to_repr(),
                us[6].to_repr(),
                us[7].to_repr(),
            ]
        })
        .collect()
}

fn fixed_z(generator: pallas::Affine, num_windows: usize) -> Vec<u64> {
    fixed_zs_and_us(generator, num_windows)
        .iter()
        .map(|(z, _)| *z)
        .collect()
}

fn fixed_zs_and_us(
    generator: pallas::Affine,
    num_windows: usize,
) -> &'static [(u64, [pallas::Base; H])] {
    if generator == asset_note_commit_r() && num_windows == NUM_WINDOWS {
        ASSET_NOTE_COMMIT_R_ZS_US
            .get_or_init(|| {
                find_zs_and_us(generator, NUM_WINDOWS)
                    .expect("asset-orchard note-commitment fixed base must have z/u table")
            })
            .as_slice()
    } else if generator == asset_note_commit_r() && num_windows == NUM_WINDOWS_SHORT {
        ASSET_NOTE_COMMIT_R_ZS_US_SHORT
            .get_or_init(|| {
                find_zs_and_us(generator, NUM_WINDOWS_SHORT)
                    .expect("asset-orchard short fixed base must have z/u table")
            })
            .as_slice()
    } else if generator == orchard_commit_ivk_r() && num_windows == NUM_WINDOWS {
        ORCHARD_COMMIT_IVK_R_ZS_US
            .get_or_init(|| {
                find_zs_and_us(generator, NUM_WINDOWS)
                    .expect("asset-orchard CommitIvkR fixed base must have z/u table")
            })
            .as_slice()
    } else if generator == asset_spend_auth_g() && num_windows == NUM_WINDOWS {
        ASSET_SPEND_AUTH_G_ZS_US
            .get_or_init(|| {
                find_zs_and_us(generator, NUM_WINDOWS)
                    .expect("asset-orchard SpendAuthG fixed base must have z/u table")
            })
            .as_slice()
    } else {
        panic!("unsupported asset-orchard fixed-base generator/window combination");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset_orchard::{
        asset_note_message_bits, hash_to_pallas_base, orchard_psi, orchard_rcm, AssetNoteOpening,
        AssetTag, ASSET_ORCHARD_DIVERSIFIER_BYTES, ASSET_ORCHARD_RSEED_BYTES,
    };
    use group::{prime::PrimeCurveAffine, GroupEncoding};
    use halo2_gadgets::{
        ecc::{CircuitVersion, X},
        utilities::lookup_range_check::LookupRangeCheck,
    };
    use halo2_proofs::{
        circuit::{Layouter, SimpleFloorPlanner, Value},
        dev::MockProver,
        plonk::{Advice, Circuit, Column, ConstraintSystem, Error, Instance},
    };
    use pasta_curves::arithmetic::{CurveAffine, CurveExt};

    #[derive(Clone, Debug)]
    struct AssetNoteCommitmentTestCircuit {
        pieces: Vec<(pallas::Base, usize)>,
        rcm: pallas::Scalar,
    }

    #[derive(Clone, Debug)]
    struct AssetNoteCommitmentTestConfig {
        instance: Column<Instance>,
        ecc: AssetOrchardEccConfig,
        sinsemilla: AssetOrchardSinsemillaConfig,
        message_piece: AssetOrchardMessagePieceConstraintConfig,
    }

    impl Circuit<pallas::Base> for AssetNoteCommitmentTestCircuit {
        type Config = AssetNoteCommitmentTestConfig;
        type FloorPlanner = SimpleFloorPlanner;

        fn without_witnesses(&self) -> Self {
            Self {
                pieces: self
                    .pieces
                    .iter()
                    .map(|(_, words)| (pallas::Base::ZERO, *words))
                    .collect(),
                rcm: pallas::Scalar::ZERO,
            }
        }

        fn configure(meta: &mut ConstraintSystem<pallas::Base>) -> Self::Config {
            let instance = meta.instance_column();
            meta.enable_equality(instance);
            let constants = meta.fixed_column();
            meta.enable_constant(constants);

            let advices: [Column<Advice>; 10] = std::array::from_fn(|_| meta.advice_column());
            let table_idx = meta.lookup_table_column();
            let lagrange_coeffs = std::array::from_fn(|_| meta.fixed_column());
            let lookup = (
                table_idx,
                meta.lookup_table_column(),
                meta.lookup_table_column(),
            );
            let message_piece_weight = meta.fixed_column();
            let range_check = PallasLookupRangeCheckConfig::configure(meta, advices[9], table_idx);
            let ecc = AssetOrchardEccChip::configure(meta, advices, lagrange_coeffs, range_check);
            let sinsemilla = AssetOrchardSinsemillaChip::configure(
                meta,
                advices[..5].try_into().expect("five advice columns"),
                advices[2],
                lagrange_coeffs[0],
                lookup,
                range_check,
                false,
            );
            let message_piece = AssetOrchardMessagePieceConstraintConfig::configure(
                meta,
                advices[5],
                advices[6],
                advices[7],
                advices[8],
                message_piece_weight,
            );

            AssetNoteCommitmentTestConfig {
                instance,
                ecc,
                sinsemilla,
                message_piece,
            }
        }

        fn synthesize(
            &self,
            config: Self::Config,
            mut layouter: impl Layouter<pallas::Base>,
        ) -> Result<(), Error> {
            let ecc_chip =
                AssetOrchardEccChip::construct(config.ecc.clone(), CircuitVersion::AnchoredBase);
            AssetOrchardSinsemillaChip::load(config.sinsemilla.clone(), &mut layouter)?;
            let sinsemilla_chip = AssetOrchardSinsemillaChip::construct(config.sinsemilla.clone());
            let cmx = synthesize_asset_note_commitment_from_pieces(
                &mut layouter,
                sinsemilla_chip,
                ecc_chip,
                &self.pieces,
                Value::known(self.rcm),
            )?;
            constrain_x_to_instance(&mut layouter, &config, cmx, 0)
        }
    }

    #[derive(Clone, Debug)]
    struct AssignedSubpieceCommitmentTestCircuit {
        subpieces: Vec<(pallas::Base, usize)>,
        rcm: pallas::Scalar,
    }

    impl Circuit<pallas::Base> for AssignedSubpieceCommitmentTestCircuit {
        type Config = AssetNoteCommitmentTestConfig;
        type FloorPlanner = SimpleFloorPlanner;

        fn without_witnesses(&self) -> Self {
            Self {
                subpieces: self
                    .subpieces
                    .iter()
                    .map(|(_, bits)| (pallas::Base::ZERO, *bits))
                    .collect(),
                rcm: pallas::Scalar::ZERO,
            }
        }

        fn configure(meta: &mut ConstraintSystem<pallas::Base>) -> Self::Config {
            <AssetNoteCommitmentTestCircuit as Circuit<pallas::Base>>::configure(meta)
        }

        fn synthesize(
            &self,
            config: Self::Config,
            mut layouter: impl Layouter<pallas::Base>,
        ) -> Result<(), Error> {
            let ecc_chip =
                AssetOrchardEccChip::construct(config.ecc.clone(), CircuitVersion::AnchoredBase);
            AssetOrchardSinsemillaChip::load(config.sinsemilla.clone(), &mut layouter)?;
            let sinsemilla_chip = AssetOrchardSinsemillaChip::construct(config.sinsemilla.clone());
            let assigned_subpieces = layouter.assign_region(
                || "assigned asset-note subpieces",
                |mut region| {
                    self.subpieces
                        .iter()
                        .enumerate()
                        .map(|(row, (value, bits))| {
                            region
                                .assign_advice(
                                    || "assigned subpiece",
                                    config.message_piece.subpiece,
                                    row,
                                    || Value::known(*value),
                                )
                                .map(|cell| RangeConstrained::unsound_unchecked(cell, *bits))
                        })
                        .collect::<Result<Vec<_>, _>>()
                },
            )?;
            let piece_subpieces = vec![assigned_subpieces];
            let cmx = synthesize_asset_note_commitment_from_assigned_subpieces(
                &mut layouter,
                &config.message_piece,
                sinsemilla_chip,
                ecc_chip,
                &piece_subpieces,
                Value::known(self.rcm),
            )?;
            constrain_x_to_instance(&mut layouter, &config, cmx, 0)
        }
    }

    #[derive(Clone, Debug)]
    struct MessagePieceAccumulatorTestCircuit {
        subpieces: Vec<(pallas::Base, usize)>,
        piece: pallas::Base,
    }

    #[derive(Clone, Debug)]
    struct MessagePieceAccumulatorTestConfig {
        message_piece: AssetOrchardMessagePieceConstraintConfig,
    }

    impl Circuit<pallas::Base> for MessagePieceAccumulatorTestCircuit {
        type Config = MessagePieceAccumulatorTestConfig;
        type FloorPlanner = SimpleFloorPlanner;

        fn without_witnesses(&self) -> Self {
            Self {
                subpieces: self
                    .subpieces
                    .iter()
                    .map(|(_, bits)| (pallas::Base::ZERO, *bits))
                    .collect(),
                piece: pallas::Base::ZERO,
            }
        }

        fn configure(meta: &mut ConstraintSystem<pallas::Base>) -> Self::Config {
            let columns: [Column<Advice>; 4] = std::array::from_fn(|_| meta.advice_column());
            let weight = meta.fixed_column();
            let message_piece = AssetOrchardMessagePieceConstraintConfig::configure(
                meta, columns[0], columns[1], columns[2], columns[3], weight,
            );

            MessagePieceAccumulatorTestConfig { message_piece }
        }

        fn synthesize(
            &self,
            config: Self::Config,
            mut layouter: impl Layouter<pallas::Base>,
        ) -> Result<(), Error> {
            let (piece_cell, assigned_subpieces) = layouter.assign_region(
                || "message accumulator witnesses",
                |mut region| {
                    let piece_cell = region.assign_advice(
                        || "message piece",
                        config.message_piece.piece,
                        0,
                        || Value::known(self.piece),
                    )?;
                    let subpieces = self
                        .subpieces
                        .iter()
                        .enumerate()
                        .map(|(row, (value, bits))| {
                            region
                                .assign_advice(
                                    || "assigned subpiece",
                                    config.message_piece.subpiece,
                                    row,
                                    || Value::known(*value),
                                )
                                .map(|cell| RangeConstrained::unsound_unchecked(cell, *bits))
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    Ok((piece_cell, subpieces))
                },
            )?;
            constrain_assigned_message_piece_to_subpieces(
                &mut layouter,
                &config.message_piece,
                "message accumulator test",
                &piece_cell,
                &assigned_subpieces,
            )
        }
    }

    fn constrain_x_to_instance(
        layouter: &mut impl Layouter<pallas::Base>,
        config: &AssetNoteCommitmentTestConfig,
        cmx: X<pallas::Affine, AssetOrchardEccChip>,
        instance_row: usize,
    ) -> Result<(), Error> {
        layouter.constrain_instance(cmx.inner().cell(), config.instance, instance_row)
    }

    fn sample_point(seed: &[u8]) -> pallas::Affine {
        pallas::Point::hash_to_curve("postfiat.asset_orchard.sinsemilla.test_point")(seed)
            .to_affine()
    }

    fn sample_note(rho: pallas::Base) -> AssetNoteOpening {
        let rseed = [11u8; ASSET_ORCHARD_RSEED_BYTES];
        AssetNoteOpening {
            diversifier: [3u8; ASSET_ORCHARD_DIVERSIFIER_BYTES],
            g_d: sample_point(b"g_d"),
            pk_d: sample_point(b"pk_d"),
            asset_tag: AssetTag::derive("a651").expect("asset tag"),
            value: 123,
            rho,
            psi: orchard_psi(&rseed, rho).expect("psi"),
            rcm: orchard_rcm(&rseed, rho).expect("rcm"),
        }
    }

    fn host_asset_note_commitment_cmx(bits: Vec<bool>, rcm: pallas::Scalar) -> pallas::Base {
        let domain = ::sinsemilla::CommitDomain::new(ASSET_ORCHARD_NOTE_COMMIT_DOMAIN_V1);
        let point = Option::<pallas::Point>::from(domain.commit(bits.into_iter(), &rcm))
            .expect("host commitment point");
        let coordinates: pasta_curves::arithmetic::Coordinates<pallas::Affine> =
            Option::from(point.to_affine().coordinates()).expect("commitment coordinates");
        *coordinates.x()
    }

    fn subpiece_bits(subpieces: &[(pallas::Base, usize)]) -> Vec<bool> {
        subpieces
            .iter()
            .flat_map(|(value, bits)| {
                let encoded = value.to_repr();
                (0..*bits).map(move |bit| ((encoded[bit / 8] >> (bit % 8)) & 1) == 1)
            })
            .collect()
    }

    fn packed_subpiece_value(subpieces: &[(pallas::Base, usize)]) -> pallas::Base {
        let mut offset = 0usize;
        let mut packed = pallas::Base::ZERO;
        for (value, bits) in subpieces {
            packed += *value * two_pow_base(offset);
            offset += *bits;
        }
        packed
    }

    #[test]
    fn asset_note_message_piece_packing_is_deterministic() {
        let pool_domain = hash_to_pallas_base("test", b"asset-orchard-pool").expect("pool");
        let rho = hash_to_pallas_base("test", b"asset-orchard-rho").expect("rho");
        let note = sample_note(rho);
        let bits = asset_note_message_bits(pool_domain, &note).expect("message bits");
        let pieces = sinsemilla_piece_values_from_bits(&bits);
        let pieces_again = sinsemilla_piece_values_from_bits(&bits);

        assert_eq!(bits.len(), 1597);
        assert_eq!(pieces, pieces_again);
        assert_eq!(pieces.iter().map(|(_, words)| *words).sum::<usize>(), 160);
        assert_eq!(pieces.len(), 7);
    }

    #[test]
    fn asset_note_commitment_domain_generators_are_non_identity() {
        assert_ne!(
            asset_note_commit_q().to_bytes(),
            pallas::Affine::identity().to_bytes()
        );
        assert_ne!(
            asset_note_commit_r().to_bytes(),
            pallas::Affine::identity().to_bytes()
        );
        assert_ne!(
            asset_note_commit_q().to_bytes(),
            asset_note_commit_r().to_bytes()
        );
    }

    #[test]
    fn assigned_subpiece_accumulator_binds_piece_value() {
        let subpieces = vec![
            (pallas::Base::from(0b1010), 4),
            (pallas::Base::ONE, 1),
            (pallas::Base::from(0b10010), 5),
        ];
        let circuit = MessagePieceAccumulatorTestCircuit {
            piece: packed_subpiece_value(&subpieces),
            subpieces,
        };
        let prover = MockProver::run(4, &circuit, vec![]).expect("prover");

        prover.assert_satisfied();
    }

    #[test]
    fn assigned_subpiece_accumulator_rejects_wrong_piece_value() {
        let subpieces = vec![
            (pallas::Base::from(0b1010), 4),
            (pallas::Base::ONE, 1),
            (pallas::Base::from(0b10010), 5),
        ];
        let circuit = MessagePieceAccumulatorTestCircuit {
            piece: packed_subpiece_value(&subpieces) + pallas::Base::ONE,
            subpieces,
        };
        let prover = MockProver::run(4, &circuit, vec![]).expect("prover");

        assert!(prover.verify().is_err());
    }

    #[test]
    #[ignore = "assigned-subpiece Sinsemilla MockProver is release-only benchmark-scale"]
    fn assigned_subpiece_commitment_gadget_matches_host_commitment() {
        let rcm =
            orchard_rcm(&[9u8; ASSET_ORCHARD_RSEED_BYTES], pallas::Base::from(17)).expect("rcm");
        let subpieces = vec![
            (pallas::Base::from(0b1010), 4),
            (pallas::Base::ONE, 1),
            (pallas::Base::from(0b10010), 5),
        ];
        let expected_cmx = host_asset_note_commitment_cmx(subpiece_bits(&subpieces), rcm);
        let circuit = AssignedSubpieceCommitmentTestCircuit { subpieces, rcm };
        let prover = MockProver::run(11, &circuit, vec![vec![expected_cmx]]).expect("prover");

        prover.assert_satisfied();
    }

    #[test]
    #[ignore = "full asset-note Sinsemilla MockProver is release-only benchmark-scale"]
    fn asset_note_commitment_gadget_matches_host_commitment() {
        let pool_domain = hash_to_pallas_base("test", b"asset-orchard-pool").expect("pool");
        let rho = hash_to_pallas_base("test", b"asset-orchard-rho").expect("rho");
        let note = sample_note(rho);
        let bits = asset_note_message_bits(pool_domain, &note).expect("message bits");
        let pieces = sinsemilla_piece_values_from_bits(&bits);
        let expected_cmx = note.cmx(pool_domain).expect("host cmx");
        let circuit = AssetNoteCommitmentTestCircuit {
            pieces,
            rcm: note.rcm,
        };
        let prover = MockProver::run(11, &circuit, vec![vec![expected_cmx]]).expect("prover");

        prover.assert_satisfied();
    }
}
