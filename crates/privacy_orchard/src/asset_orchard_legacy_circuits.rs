// Replay-only Halo2 circuit-shape markers.
//
// Provenance: the constraint-system configuration is copied from
// commit 3218ec53^ (the last custom-Poseidon AssetOrchard circuits). These
// types exist only so historical pinned assemblies can reconstruct their VKs.
// They intentionally expose no proving or witness-synthesis surface.

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub(super) struct LegacyAssetOrchardCircuitConfig {
    advice: [Column<Advice>; 9],
    fixed: [Column<Fixed>; 5],
    instance: Column<Instance>,
    ecc: AssetOrchardEccConfig,
    sinsemilla: AssetOrchardSinsemillaConfig,
    message_piece: AssetOrchardMessagePieceConstraintConfig,
    merkle_1: AssetOrchardMerkleConfig,
    merkle_2: AssetOrchardMerkleConfig,
    q_conservation: Selector,
    q_value_nonzero: Selector,
    q_asset_tag_nonzero: Selector,
    q_range: Selector,
    q_absorb: Selector,
    q_input0_constant: Selector,
    q_input1_constant: Selector,
    q_public_distinct: Selector,
    q_pricing_binding: Selector,
    q_poseidon_full_round: Selector,
    q_poseidon_partial_round: Selector,
}

#[derive(Clone, Debug, Default)]
pub(super) struct LegacyAssetOrchardSwapV3Circuit;

#[derive(Clone, Debug, Default)]
pub(super) struct LegacyAssetOrchardPrivateEgressV1Circuit;

macro_rules! impl_legacy_asset_orchard_replay_circuit {
    ($circuit:ty) => {
        impl Circuit<pallas::Base> for $circuit {
            type Config = LegacyAssetOrchardCircuitConfig;
            type FloorPlanner = SimpleFloorPlanner;

            fn without_witnesses(&self) -> Self {
                Self
            }

            fn configure(meta: &mut ConstraintSystem<pallas::Base>) -> Self::Config {
                configure_legacy_asset_orchard_circuit(meta)
            }

            fn synthesize(
                &self,
                _config: Self::Config,
                _layouter: impl Layouter<pallas::Base>,
            ) -> Result<(), Error> {
                Err(Error::Synthesis)
            }
        }
    };
}

impl_legacy_asset_orchard_replay_circuit!(LegacyAssetOrchardSwapV3Circuit);
impl_legacy_asset_orchard_replay_circuit!(LegacyAssetOrchardPrivateEgressV1Circuit);

fn configure_legacy_asset_orchard_circuit(
    meta: &mut ConstraintSystem<pallas::Base>,
) -> LegacyAssetOrchardCircuitConfig {
    let advice = [
        meta.advice_column(),
        meta.advice_column(),
        meta.advice_column(),
        meta.advice_column(),
        meta.advice_column(),
        meta.advice_column(),
        meta.advice_column(),
        meta.advice_column(),
        meta.advice_column(),
    ];
    let fixed = [
        meta.fixed_column(),
        meta.fixed_column(),
        meta.fixed_column(),
        meta.fixed_column(),
        meta.fixed_column(),
    ];
    let instance = meta.instance_column();
    let _constants = meta.fixed_column();
    meta.enable_constant(_constants);
    let sinsemilla_advices: [Column<Advice>; 10] = std::array::from_fn(|_| meta.advice_column());
    let table_idx = meta.lookup_table_column();
    let lagrange_coeffs = std::array::from_fn(|_| meta.fixed_column());
    let lookup = (
        table_idx,
        meta.lookup_table_column(),
        meta.lookup_table_column(),
    );
    let message_piece_weight = meta.fixed_column();
    let sinsemilla_range_check =
        PallasLookupRangeCheckConfig::configure(meta, sinsemilla_advices[9], table_idx);
    let ecc = AssetOrchardEccChip::configure(
        meta,
        sinsemilla_advices,
        lagrange_coeffs,
        sinsemilla_range_check,
    );
    let sinsemilla = AssetOrchardSinsemillaChip::configure(
        meta,
        sinsemilla_advices[..5]
            .try_into()
            .expect("five advice columns"),
        sinsemilla_advices[2],
        lagrange_coeffs[0],
        lookup,
        sinsemilla_range_check,
        false,
    );
    let merkle_sinsemilla_1 = halo2_gadgets::sinsemilla::chip::SinsemillaChip::<
        AssetOrchardMerkleHashDomain,
        AssetOrchardMerkleCommitDomain,
        AssetOrchardFixedBases,
    >::configure(
        meta,
        sinsemilla_advices[..5]
            .try_into()
            .expect("five advice columns"),
        sinsemilla_advices[2],
        lagrange_coeffs[1],
        lookup,
        sinsemilla_range_check,
        false,
    );
    let merkle_1 = AssetOrchardMerkleChip::configure(meta, merkle_sinsemilla_1.clone());
    let merkle_sinsemilla_2 = halo2_gadgets::sinsemilla::chip::SinsemillaChip::<
        AssetOrchardMerkleHashDomain,
        AssetOrchardMerkleCommitDomain,
        AssetOrchardFixedBases,
    >::configure(
        meta,
        sinsemilla_advices[5..]
            .try_into()
            .expect("five advice columns"),
        sinsemilla_advices[7],
        lagrange_coeffs[2],
        lookup,
        sinsemilla_range_check,
        false,
    );
    let merkle_2 = AssetOrchardMerkleChip::configure(meta, merkle_sinsemilla_2.clone());
    let message_piece = AssetOrchardMessagePieceConstraintConfig::configure(
        meta,
        sinsemilla_advices[5],
        sinsemilla_advices[6],
        sinsemilla_advices[7],
        sinsemilla_advices[8],
        message_piece_weight,
    );
    for column in &advice {
        meta.enable_equality(*column);
    }
    meta.enable_equality(instance);

    let q_conservation = meta.selector();
    let q_value_nonzero = meta.selector();
    let q_asset_tag_nonzero = meta.selector();
    let q_range = meta.selector();
    let q_absorb = meta.selector();
    let q_input0_constant = meta.selector();
    let q_input1_constant = meta.selector();
    let q_public_distinct = meta.selector();
    let q_pricing_binding = meta.selector();
    let q_poseidon_full_round = meta.selector();
    let q_poseidon_partial_round = meta.selector();

    meta.create_gate("asset-orchard private pair conservation", |meta| {
        let q = meta.query_selector(q_conservation);
        let s = meta.query_advice(advice[0], Rotation::cur());
        let in0 = meta.query_advice(advice[1], Rotation::cur());
        let in1 = meta.query_advice(advice[2], Rotation::cur());
        let out0 = meta.query_advice(advice[3], Rotation::cur());
        let out1 = meta.query_advice(advice[4], Rotation::cur());
        let one = Expression::Constant(pallas::Base::ONE);

        vec![
            q.clone() * s.clone() * (s.clone() - one),
            q.clone() * (out0 - (in0.clone() + s.clone() * (in1.clone() - in0.clone()))),
            q * (out1 - (in1.clone() + s * (in0 - in1))),
        ]
    });

    meta.create_gate("asset-orchard nonzero private values", |meta| {
        let q = meta.query_selector(q_value_nonzero);
        let in0 = meta.query_advice(advice[1], Rotation::cur());
        let in1 = meta.query_advice(advice[2], Rotation::cur());
        let out0 = meta.query_advice(advice[3], Rotation::cur());
        let out1 = meta.query_advice(advice[4], Rotation::cur());
        let inv_in0 = meta.query_advice(advice[5], Rotation::cur());
        let inv_in1 = meta.query_advice(advice[6], Rotation::cur());
        let inv_out0 = meta.query_advice(advice[7], Rotation::cur());
        let inv_out1 = meta.query_advice(advice[8], Rotation::cur());
        let one = Expression::Constant(pallas::Base::ONE);

        vec![
            q.clone() * (in0 * inv_in0 - one.clone()),
            q.clone() * (in1 * inv_in1 - one.clone()),
            q.clone() * (out0 * inv_out0 - one.clone()),
            q * (out1 * inv_out1 - one),
        ]
    });

    meta.create_gate("asset-orchard nonzero private asset tags", |meta| {
        let q = meta.query_selector(q_asset_tag_nonzero);
        let in0_lo = meta.query_advice(advice[1], Rotation::cur());
        let in0_hi = meta.query_advice(advice[1], Rotation::next());
        let in1_lo = meta.query_advice(advice[2], Rotation::cur());
        let in1_hi = meta.query_advice(advice[2], Rotation::next());
        let out0_lo = meta.query_advice(advice[3], Rotation::cur());
        let out0_hi = meta.query_advice(advice[3], Rotation::next());
        let out1_lo = meta.query_advice(advice[4], Rotation::cur());
        let out1_hi = meta.query_advice(advice[4], Rotation::next());
        let inv_in0_lo = meta.query_advice(advice[5], Rotation::cur());
        let inv_in0_hi = meta.query_advice(advice[5], Rotation::next());
        let inv_in1_lo = meta.query_advice(advice[6], Rotation::cur());
        let inv_in1_hi = meta.query_advice(advice[6], Rotation::next());
        let inv_out0_lo = meta.query_advice(advice[7], Rotation::cur());
        let inv_out0_hi = meta.query_advice(advice[7], Rotation::next());
        let inv_out1_lo = meta.query_advice(advice[8], Rotation::cur());
        let inv_out1_hi = meta.query_advice(advice[8], Rotation::next());
        let one = Expression::Constant(pallas::Base::ONE);

        let mut constraints = Vec::new();
        for (lo, hi, inv_lo, inv_hi) in [
            (in0_lo, in0_hi, inv_in0_lo, inv_in0_hi),
            (in1_lo, in1_hi, inv_in1_lo, inv_in1_hi),
            (out0_lo, out0_hi, inv_out0_lo, inv_out0_hi),
            (out1_lo, out1_hi, inv_out1_lo, inv_out1_hi),
        ] {
            let lo_is_nonzero = lo * inv_lo;
            let hi_is_nonzero = hi * inv_hi;
            constraints
                .push(q.clone() * lo_is_nonzero.clone() * (lo_is_nonzero.clone() - one.clone()));
            constraints
                .push(q.clone() * hi_is_nonzero.clone() * (hi_is_nonzero.clone() - one.clone()));
            constraints
                .push(q.clone() * (one.clone() - lo_is_nonzero) * (one.clone() - hi_is_nonzero));
        }
        constraints
    });

    meta.create_gate("asset-orchard bit range accumulator", |meta| {
        let q = meta.query_selector(q_range);
        let bit = meta.query_advice(advice[0], Rotation::cur());
        let acc = meta.query_advice(advice[1], Rotation::cur());
        let next_acc = meta.query_advice(advice[2], Rotation::cur());
        let weight = meta.query_fixed(fixed[0]);
        let one = Expression::Constant(pallas::Base::ONE);

        vec![
            q.clone() * bit.clone() * (bit.clone() - one),
            q * (next_acc - acc - bit * weight),
        ]
    });

    meta.create_gate("asset-orchard poseidon absorb inputs", |meta| {
        let q = meta.query_selector(q_absorb);
        let state0 = meta.query_advice(advice[0], Rotation::cur());
        let state1 = meta.query_advice(advice[1], Rotation::cur());
        let state2 = meta.query_advice(advice[2], Rotation::cur());
        let input0 = meta.query_advice(advice[3], Rotation::cur());
        let input1 = meta.query_advice(advice[4], Rotation::cur());
        let prev0 = meta.query_advice(advice[5], Rotation::cur());
        let prev1 = meta.query_advice(advice[6], Rotation::cur());
        let prev2 = meta.query_advice(advice[7], Rotation::cur());

        vec![
            q.clone() * (state0 - prev0 - input0),
            q.clone() * (state1 - prev1 - input1),
            q * (state2 - prev2),
        ]
    });

    meta.create_gate("asset-orchard h_action constant inputs", |meta| {
        let q0 = meta.query_selector(q_input0_constant);
        let q1 = meta.query_selector(q_input1_constant);
        let input0 = meta.query_advice(advice[3], Rotation::cur());
        let input1 = meta.query_advice(advice[4], Rotation::cur());
        let fixed0 = meta.query_fixed(fixed[3]);
        let fixed1 = meta.query_fixed(fixed[4]);

        vec![q0 * (input0 - fixed0), q1 * (input1 - fixed1)]
    });

    meta.create_gate("asset-orchard distinct public state fields", |meta| {
        let q = meta.query_selector(q_public_distinct);
        let left = meta.query_advice(advice[0], Rotation::cur());
        let right = meta.query_advice(advice[0], Rotation::next());
        let inverse = meta.query_advice(advice[1], Rotation::cur());
        let one = Expression::Constant(pallas::Base::ONE);

        vec![q * ((left - right) * inverse - one)]
    });

    meta.create_gate("asset-orchard private pricing claim binding", |meta| {
        let q = meta.query_selector(q_pricing_binding);
        let base_tag_lo = meta.query_advice(advice[0], Rotation::cur());
        let base_tag_hi = meta.query_advice(advice[1], Rotation::cur());
        let quote_tag_lo = meta.query_advice(advice[2], Rotation::cur());
        let quote_tag_hi = meta.query_advice(advice[3], Rotation::cur());
        let base_value = meta.query_advice(advice[4], Rotation::cur());
        let quote_value = meta.query_advice(advice[5], Rotation::cur());
        let numerator = meta.query_advice(advice[6], Rotation::cur());
        let denominator = meta.query_advice(advice[7], Rotation::cur());
        let input_base_lo = meta.query_advice(advice[0], Rotation::next());
        let input_base_hi = meta.query_advice(advice[1], Rotation::next());
        let input_quote_lo = meta.query_advice(advice[2], Rotation::next());
        let input_quote_hi = meta.query_advice(advice[3], Rotation::next());
        let rounding_remainder = meta.query_advice(advice[4], Rotation::next());
        let rounding_slack = meta.query_advice(advice[5], Rotation::next());
        let one = Expression::Constant(pallas::Base::ONE);
        vec![
            q.clone() * (base_tag_lo - input_base_lo),
            q.clone() * (base_tag_hi - input_base_hi),
            q.clone() * (quote_tag_lo - input_quote_lo),
            q.clone() * (quote_tag_hi - input_quote_hi),
            q.clone()
                * (base_value * numerator
                    - quote_value * denominator.clone()
                    - rounding_remainder.clone()),
            q * (rounding_remainder + rounding_slack + one - denominator),
        ]
    });

    let (_, mds, _) = P128Pow5T3::constants();
    meta.create_gate("asset-orchard poseidon full round", |meta| {
        let q = meta.query_selector(q_poseidon_full_round);
        let current = [
            meta.query_advice(advice[0], Rotation::cur()),
            meta.query_advice(advice[1], Rotation::cur()),
            meta.query_advice(advice[2], Rotation::cur()),
        ];
        let next = [
            meta.query_advice(advice[0], Rotation::next()),
            meta.query_advice(advice[1], Rotation::next()),
            meta.query_advice(advice[2], Rotation::next()),
        ];
        let rc = [
            meta.query_fixed(fixed[0]),
            meta.query_fixed(fixed[1]),
            meta.query_fixed(fixed[2]),
        ];
        let sboxed = [
            legacy_expr_pow5(current[0].clone() + rc[0].clone()),
            legacy_expr_pow5(current[1].clone() + rc[1].clone()),
            legacy_expr_pow5(current[2].clone() + rc[2].clone()),
        ];
        legacy_poseidon_mds_constraints(q, next, sboxed, &mds)
    });

    meta.create_gate("asset-orchard poseidon partial round", |meta| {
        let q = meta.query_selector(q_poseidon_partial_round);
        let current = [
            meta.query_advice(advice[0], Rotation::cur()),
            meta.query_advice(advice[1], Rotation::cur()),
            meta.query_advice(advice[2], Rotation::cur()),
        ];
        let next = [
            meta.query_advice(advice[0], Rotation::next()),
            meta.query_advice(advice[1], Rotation::next()),
            meta.query_advice(advice[2], Rotation::next()),
        ];
        let rc = [
            meta.query_fixed(fixed[0]),
            meta.query_fixed(fixed[1]),
            meta.query_fixed(fixed[2]),
        ];
        let sboxed = [
            legacy_expr_pow5(current[0].clone() + rc[0].clone()),
            current[1].clone() + rc[1].clone(),
            current[2].clone() + rc[2].clone(),
        ];
        legacy_poseidon_mds_constraints(q, next, sboxed, &mds)
    });

    LegacyAssetOrchardCircuitConfig {
        advice,
        fixed,
        instance,
        ecc,
        sinsemilla,
        message_piece,
        merkle_1,
        merkle_2,
        q_conservation,
        q_value_nonzero,
        q_asset_tag_nonzero,
        q_range,
        q_absorb,
        q_input0_constant,
        q_input1_constant,
        q_public_distinct,
        q_pricing_binding,
        q_poseidon_full_round,
        q_poseidon_partial_round,
    }
}

fn legacy_expr_pow5(value: Expression<pallas::Base>) -> Expression<pallas::Base> {
    let square = value.clone() * value.clone();
    square.clone() * square * value
}

fn legacy_poseidon_mds_constraints(
    q: Expression<pallas::Base>,
    next: [Expression<pallas::Base>; 3],
    sboxed: [Expression<pallas::Base>; 3],
    mds: &halo2_poseidon::Mds<pallas::Base, 3>,
) -> Vec<Expression<pallas::Base>> {
    let mut constraints = Vec::with_capacity(3);
    for row in 0..3 {
        let mut expected = Expression::Constant(pallas::Base::ZERO);
        for col in 0..3 {
            expected = expected + Expression::Constant(mds[row][col]) * sboxed[col].clone();
        }
        constraints.push(q.clone() * (next[row].clone() - expected));
    }
    constraints
}
