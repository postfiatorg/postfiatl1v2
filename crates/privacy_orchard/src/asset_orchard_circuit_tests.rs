use super::*;
use crate::asset_orchard::{
    asset_derive_nullifier, asset_orchard_accounting_record, asset_output_rho,
    encrypted_output_hash, hash_to_pallas_base, hash_to_pallas_scalar_nonzero, orchard_commit_ivk,
    orchard_psi, orchard_rcm, swap_binding_hash, AssetNoteOpening, AssetOrchardActionPublicFields,
    AssetOrchardBoundedBytes, AssetOrchardFieldElement, AssetOrchardPoint,
    AssetOrchardPricingClaim, AssetOrchardPricingPublicFields, AssetOrchardProofBytes,
    AssetOrchardSpendAuthSignature, AssetOrchardSwapAccountingRecord, AssetOrchardSwapAction,
    AssetOrchardSwapBindingHash, RandomizedVerificationKeyFields, ASSET_ORCHARD_ACTION_SCHEMA_V1,
    ASSET_ORCHARD_ACTION_VERSION_V1, ASSET_ORCHARD_CIRCUIT_ID_V1, ASSET_ORCHARD_DIVERSIFIER_BYTES,
    ASSET_ORCHARD_ENCRYPTED_OUTPUT_MAX_BYTES, ASSET_ORCHARD_POOL_ID_V1,
    ASSET_ORCHARD_PROOF_SYSTEM_ID_V1, ASSET_ORCHARD_RSEED_BYTES,
};
use crate::asset_orchard_sinsemilla::asset_spend_auth_g;
use ff::PrimeField;
use halo2_proofs::dev::MockProver;
use incrementalmerkletree::{Hashable, Level};
use orchard::{
    note::ExtractedNoteCommitment,
    primitives::redpallas::{SigningKey, SpendAuth, VerificationKey},
    tree::MerkleHashOrchard,
};
use pasta_curves::{arithmetic::CurveExt, group::Curve};
use rand::rngs::OsRng;

const ASSET_ORCHARD_NOTE_COMMITMENT_TEST_K: u32 = 16;
#[cfg(not(feature = "asset-orchard-vk-dev-env"))]
static PRIVATE_EGRESS_VK_ENV_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
static SWAP_VK_ENV_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn sample_point(seed: &[u8]) -> pallas::Affine {
    pallas::Point::hash_to_curve("postfiat.asset_orchard.circuit.test_point")(seed).to_affine()
}

fn public_fields() -> AssetOrchardActionPublicFields {
    AssetOrchardActionPublicFields {
        pool_domain: hash_to_pallas_base("test", b"pool").expect("pool"),
        anchor: hash_to_pallas_base("test", b"anchor").expect("anchor"),
        nullifiers: [
            hash_to_pallas_base("test", b"nf0").expect("nf0"),
            hash_to_pallas_base("test", b"nf1").expect("nf1"),
        ],
        randomized_verification_keys: [
            RandomizedVerificationKeyFields::from_affine(sample_point(b"rk0")).expect("rk0"),
            RandomizedVerificationKeyFields::from_affine(sample_point(b"rk1")).expect("rk1"),
        ],
        output_commitments: [
            hash_to_pallas_base("test", b"cmx0").expect("cmx0"),
            hash_to_pallas_base("test", b"cmx1").expect("cmx1"),
        ],
        encrypted_output_hashes: [
            encrypted_output_hash(0, b"eo0").expect("eo0"),
            encrypted_output_hash(1, b"eo1").expect("eo1"),
        ],
        pricing: test_pricing_public_fields(),
        fee: 0,
    }
}

fn test_pricing_claim() -> AssetOrchardPricingClaim {
    let base = AssetTag::derive("a651").expect("base tag");
    let quote = AssetTag::derive("pfUSDC").expect("quote tag");
    AssetOrchardPricingClaim {
        nav_epoch: 59,
        reserve_packet_hash: "ab".repeat(48),
        ratio_numerator: 9,
        ratio_denominator: 5,
        mode: "at_nav_with_band".to_string(),
        band_bps: 0,
        base_asset_tag_lo: base.lo,
        base_asset_tag_hi: base.hi,
        quote_asset_tag_lo: quote.lo,
        quote_asset_tag_hi: quote.hi,
    }
}

fn test_pricing_public_fields() -> AssetOrchardPricingPublicFields {
    let claim = test_pricing_claim();
    AssetOrchardPricingPublicFields {
        base_asset_tag: AssetTag {
            lo: claim.base_asset_tag_lo,
            hi: claim.base_asset_tag_hi,
        },
        quote_asset_tag: AssetTag {
            lo: claim.quote_asset_tag_lo,
            hi: claim.quote_asset_tag_hi,
        },
        ratio_numerator: claim.ratio_numerator,
        ratio_denominator: claim.ratio_denominator,
        commitment: claim.commitment_fields().expect("pricing commitment"),
    }
}

fn leg(asset_id: &str, value: u64) -> AssetOrchardSwapPrivateLeg {
    AssetOrchardSwapPrivateLeg {
        asset_tag: AssetTag::derive(asset_id).expect("asset tag"),
        value,
    }
}

fn note_witness(
    pool_domain: pallas::Base,
    asset_id: &str,
    value: u64,
    seed: u8,
) -> AssetOrchardSwapNoteWitness {
    let rho = hash_to_pallas_base("test", &[b"rho".as_slice(), &[seed]].concat()).expect("rho");
    note_witness_with_rho(pool_domain, asset_id, value, seed, rho)
}

fn note_witness_with_rho(
    pool_domain: pallas::Base,
    asset_id: &str,
    value: u64,
    seed: u8,
    rho: pallas::Base,
) -> AssetOrchardSwapNoteWitness {
    let mut diversifier = [0u8; ASSET_ORCHARD_DIVERSIFIER_BYTES];
    diversifier.fill(seed);
    let rseed = [seed; ASSET_ORCHARD_RSEED_BYTES];
    let nk = hash_to_pallas_base("test", &[b"nk".as_slice(), &[seed]].concat()).expect("nk");
    let ak = sample_point(&[b"ak".as_slice(), &[seed]].concat());
    let alpha = pallas::Scalar::from(u64::from(seed) + 10_000);
    let rivk = pallas::Scalar::from(u64::from(seed) + 20_000);
    let g_d = sample_point(&[b"g_d".as_slice(), &[seed]].concat());
    let ivk = orchard_commit_ivk(ak, nk, rivk).expect("ivk");
    let ivk_scalar = pallas::Scalar::from_repr(ivk.to_repr()).expect("ivk scalar");
    let pk_d = (pallas::Point::from(g_d) * ivk_scalar).to_affine();
    let note = AssetNoteOpening {
        diversifier,
        g_d,
        pk_d,
        asset_tag: AssetTag::derive(asset_id).expect("asset tag"),
        value,
        rho,
        psi: orchard_psi(&rseed, rho).expect("psi"),
        rcm: orchard_rcm(&rseed, rho).expect("rcm"),
    };
    AssetOrchardSwapNoteWitness::from_note_with_nk(pool_domain, note, nk)
        .expect("note witness")
        .with_spend_authority(AssetOrchardSpendAuthorityWitness { ak, alpha, rivk })
}

fn spend_auth_signing_key(seed: u64) -> SigningKey<SpendAuth> {
    SigningKey::try_from(pallas::Scalar::from(seed).to_repr()).expect("spend auth key")
}

fn verification_key_affine(key: &VerificationKey<SpendAuth>) -> pallas::Affine {
    let bytes = <[u8; 32]>::from(key);
    Option::<pallas::Affine>::from(pallas::Affine::from_bytes(&bytes)).expect("valid key")
}

fn note_witness_with_signing_key(
    pool_domain: pallas::Base,
    asset_id: &str,
    value: u64,
    seed: u8,
    rho: pallas::Base,
    signing_key: &SigningKey<SpendAuth>,
) -> AssetOrchardSwapNoteWitness {
    let mut diversifier = [0u8; ASSET_ORCHARD_DIVERSIFIER_BYTES];
    diversifier.fill(seed);
    let rseed = [seed; ASSET_ORCHARD_RSEED_BYTES];
    let nk = hash_to_pallas_base("test", &[b"nk".as_slice(), &[seed]].concat()).expect("nk");
    let alpha = pallas::Scalar::from(u64::from(seed) + 10_000);
    let rivk = pallas::Scalar::from(u64::from(seed) + 20_000);
    let ak = verification_key_affine(&VerificationKey::from(signing_key));
    let g_d = sample_point(&[b"g_d".as_slice(), &[seed]].concat());
    let ivk = orchard_commit_ivk(ak, nk, rivk).expect("ivk");
    let ivk_scalar = pallas::Scalar::from_repr(ivk.to_repr()).expect("ivk scalar");
    let pk_d = (pallas::Point::from(g_d) * ivk_scalar).to_affine();
    let note = AssetNoteOpening {
        diversifier,
        g_d,
        pk_d,
        asset_tag: AssetTag::derive(asset_id).expect("asset tag"),
        value,
        rho,
        psi: orchard_psi(&rseed, rho).expect("psi"),
        rcm: orchard_rcm(&rseed, rho).expect("rcm"),
    };
    AssetOrchardSwapNoteWitness::from_note_with_nk(pool_domain, note, nk)
        .expect("note witness")
        .with_spend_authority(AssetOrchardSpendAuthorityWitness { ak, alpha, rivk })
}

#[test]
fn swap_builder_alpha_randomizes_rk_for_repeated_public_commitment() {
    let chain_id = "postfiat-wan-devnet";
    let genesis_hash = [7u8; 32];
    let protocol_version = 2;
    let note = crate::asset_orchard::build_asset_orchard_wallet_note(
        chain_id,
        genesis_hash,
        protocol_version,
        "a651",
        5,
        &"11".repeat(32),
    )
    .expect("wallet note");
    let signing_key = asset_orchard_spend_signing_key(&note).expect("signing key");

    let first = spend_authority_from_wallet_note(&note, &signing_key).expect("first alpha");
    let second = spend_authority_from_wallet_note(&note, &signing_key).expect("second alpha");
    assert_eq!(first.ak, second.ak);
    assert_ne!(first.alpha, second.alpha);

    let first_rk = (pallas::Point::from(first.ak) + asset_spend_auth_g() * first.alpha).to_affine();
    let second_rk =
        (pallas::Point::from(second.ak) + asset_spend_auth_g() * second.alpha).to_affine();
    assert_ne!(first_rk, second_rk);

    let old_public_alpha = hash_to_pallas_scalar_nonzero(
        "postfiat.asset_orchard.swap_builder.alpha.v1",
        note.output_commitment.as_hex().as_bytes(),
    )
    .expect("old deterministic alpha");
    let old_public_rk =
        (pallas::Point::from(first.ak) + asset_spend_auth_g() * old_public_alpha).to_affine();
    assert_ne!(first_rk, old_public_rk);
    assert_ne!(second_rk, old_public_rk);
}

#[test]
fn private_egress_alpha_sampler_uses_fresh_randomness() {
    let first = private_egress_spend_randomizer();
    let second = private_egress_spend_randomizer();
    assert_ne!(first, second);
}

fn note_swap_witnesses(
    pool_domain: pallas::Base,
) -> (
    [AssetOrchardSwapNoteWitness; 2],
    [AssetOrchardSwapNoteWitness; 2],
    pallas::Base,
) {
    let input0 = note_witness(pool_domain, "a651", 5, 11);
    let input1 = note_witness(pool_domain, "pfUSDC", 9, 12);
    let (anchor, witnesses) = asset_merkle_witnesses([input0.cmx, input1.cmx]);
    let inputs_for_context = [
        input0.clone().with_merkle_witness(witnesses[0].clone()),
        input1.clone().with_merkle_witness(witnesses[1].clone()),
    ];
    let nullifiers = note_swap_nullifiers(pool_domain, &inputs_for_context);
    let rks = note_swap_rks(&inputs_for_context);
    let output0_rho =
        asset_output_rho(pool_domain, anchor, nullifiers, rks, 0).expect("output0 rho");
    let output1_rho =
        asset_output_rho(pool_domain, anchor, nullifiers, rks, 1).expect("output1 rho");
    (
        inputs_for_context,
        [
            note_witness_with_rho(pool_domain, "pfUSDC", 9, 21, output0_rho),
            note_witness_with_rho(pool_domain, "a651", 5, 22, output1_rho),
        ],
        anchor,
    )
}

fn signed_note_swap_witnesses(
    pool_domain: pallas::Base,
    signing_keys: [&SigningKey<SpendAuth>; ASSET_ORCHARD_LEG_COUNT],
) -> (
    [AssetOrchardSwapNoteWitness; 2],
    [AssetOrchardSwapNoteWitness; 2],
    pallas::Base,
) {
    let input0 = note_witness_with_signing_key(
        pool_domain,
        "a651",
        5,
        31,
        hash_to_pallas_base("test", b"signed-rho0").expect("rho0"),
        signing_keys[0],
    );
    let input1 = note_witness_with_signing_key(
        pool_domain,
        "pfUSDC",
        9,
        32,
        hash_to_pallas_base("test", b"signed-rho1").expect("rho1"),
        signing_keys[1],
    );
    let (anchor, witnesses) = asset_merkle_witnesses([input0.cmx, input1.cmx]);
    let inputs_for_context = [
        input0.clone().with_merkle_witness(witnesses[0].clone()),
        input1.clone().with_merkle_witness(witnesses[1].clone()),
    ];
    let nullifiers = note_swap_nullifiers(pool_domain, &inputs_for_context);
    let rks = note_swap_rks(&inputs_for_context);
    let output0_rho =
        asset_output_rho(pool_domain, anchor, nullifiers, rks, 0).expect("output0 rho");
    let output1_rho =
        asset_output_rho(pool_domain, anchor, nullifiers, rks, 1).expect("output1 rho");
    (
        inputs_for_context,
        [
            note_witness_with_rho(pool_domain, "pfUSDC", 9, 41, output0_rho),
            note_witness_with_rho(pool_domain, "a651", 5, 42, output1_rho),
        ],
        anchor,
    )
}

fn asset_merkle_witnesses(
    leaves: [pallas::Base; ASSET_ORCHARD_LEG_COUNT],
) -> (
    pallas::Base,
    [AssetOrchardMerkleWitness; ASSET_ORCHARD_LEG_COUNT],
) {
    let leaves = leaves
        .iter()
        .copied()
        .map(merkle_hash_from_cmx)
        .collect::<Vec<_>>();
    let root = merkle_root_from_nodes(leaves.clone());
    (
        base_from_merkle_hash(&root),
        [
            merkle_witness_from_nodes(leaves.clone(), 0),
            merkle_witness_from_nodes(leaves, 1),
        ],
    )
}

fn merkle_hash_from_cmx(cmx: pallas::Base) -> MerkleHashOrchard {
    let bytes = cmx.to_repr();
    let extracted = ExtractedNoteCommitment::from_bytes(&bytes).expect("canonical cmx");
    MerkleHashOrchard::from_cmx(&extracted)
}

fn base_from_merkle_hash(hash: &MerkleHashOrchard) -> pallas::Base {
    pallas::Base::from_repr(hash.to_bytes()).expect("canonical merkle hash")
}

fn merkle_root_from_nodes(mut level_nodes: Vec<MerkleHashOrchard>) -> MerkleHashOrchard {
    for level in 0..ASSET_ORCHARD_MERKLE_DEPTH {
        let level = Level::from(level as u8);
        let empty = MerkleHashOrchard::empty_root(level);
        let mut next_level = Vec::with_capacity(level_nodes.len().div_ceil(2));
        for chunk in level_nodes.chunks(2) {
            let left = &chunk[0];
            let right = chunk.get(1).unwrap_or(&empty);
            next_level.push(MerkleHashOrchard::combine(level, left, right));
        }
        level_nodes = next_level;
    }
    level_nodes[0]
}

fn merkle_witness_from_nodes(
    mut level_nodes: Vec<MerkleHashOrchard>,
    position: usize,
) -> AssetOrchardMerkleWitness {
    let mut current_index = position;
    let mut auth_path = Vec::with_capacity(ASSET_ORCHARD_MERKLE_DEPTH);
    for level in 0..ASSET_ORCHARD_MERKLE_DEPTH {
        let level = Level::from(level as u8);
        let empty = MerkleHashOrchard::empty_root(level);
        let sibling_index = if current_index % 2 == 0 {
            current_index + 1
        } else {
            current_index - 1
        };
        let sibling = level_nodes.get(sibling_index).unwrap_or(&empty);
        auth_path.push(base_from_merkle_hash(sibling));

        let mut next_level = Vec::with_capacity(level_nodes.len().div_ceil(2));
        for chunk in level_nodes.chunks(2) {
            let left = &chunk[0];
            let right = chunk.get(1).unwrap_or(&empty);
            next_level.push(MerkleHashOrchard::combine(level, left, right));
        }
        level_nodes = next_level;
        current_index /= 2;
    }
    AssetOrchardMerkleWitness {
        position: position as u32,
        auth_path: auth_path.try_into().expect("fixed-depth auth path"),
    }
}

fn note_swap_nullifiers(
    pool_domain: pallas::Base,
    inputs: &[AssetOrchardSwapNoteWitness; 2],
) -> [pallas::Base; 2] {
    [
        asset_derive_nullifier(
            pool_domain,
            inputs[0].nk,
            inputs[0].note.rho,
            inputs[0].note.psi,
            inputs[0].cmx,
        )
        .expect("nf0"),
        asset_derive_nullifier(
            pool_domain,
            inputs[1].nk,
            inputs[1].note.rho,
            inputs[1].note.psi,
            inputs[1].cmx,
        )
        .expect("nf1"),
    ]
}

fn note_swap_rks(
    inputs: &[AssetOrchardSwapNoteWitness; ASSET_ORCHARD_LEG_COUNT],
) -> [RandomizedVerificationKeyFields; ASSET_ORCHARD_LEG_COUNT] {
    inputs.each_ref().map(|input| {
        let authority = input.spend_authority.as_ref().expect("spend authority");
        let rk = (pallas::Point::from(authority.ak) + asset_spend_auth_g() * authority.alpha)
            .to_affine();
        RandomizedVerificationKeyFields::from_affine(rk).expect("rk")
    })
}

fn note_swap_rk_points(
    inputs: &[AssetOrchardSwapNoteWitness; ASSET_ORCHARD_LEG_COUNT],
) -> [pallas::Affine; ASSET_ORCHARD_LEG_COUNT] {
    inputs.each_ref().map(|input| {
        let authority = input.spend_authority.as_ref().expect("spend authority");
        (pallas::Point::from(authority.ak) + asset_spend_auth_g() * authority.alpha).to_affine()
    })
}

fn asset_swap_test_domain() -> crate::OrchardAuthorizingDomain {
    crate::OrchardAuthorizingDomain::new(
        "postfiat-test",
        "a".repeat(96),
        1,
        ASSET_ORCHARD_POOL_ID_V1,
    )
    .expect("asset-orchard domain")
}

fn asset_orchard_pool_domain_for_domain(domain: &crate::OrchardAuthorizingDomain) -> pallas::Base {
    crate::asset_orchard::AssetOrchardSwapAction::expected_pool_domain(
        &domain.chain_id,
        crate::asset_orchard_domain_genesis_hash(&domain.genesis_hash).expect("genesis32"),
        domain.protocol_version,
    )
    .expect("pool domain")
}

fn asset_orchard_public_fields_from_witness(
    domain: &crate::OrchardAuthorizingDomain,
    inputs: &[AssetOrchardSwapNoteWitness; ASSET_ORCHARD_LEG_COUNT],
    outputs: &[AssetOrchardSwapNoteWitness; ASSET_ORCHARD_LEG_COUNT],
    anchor: pallas::Base,
    encrypted_outputs: &[AssetOrchardBoundedBytes; ASSET_ORCHARD_LEG_COUNT],
) -> AssetOrchardActionPublicFields {
    let pool_domain = asset_orchard_pool_domain_for_domain(domain);
    AssetOrchardActionPublicFields {
        pool_domain,
        anchor,
        nullifiers: note_swap_nullifiers(pool_domain, inputs),
        randomized_verification_keys: note_swap_rks(inputs),
        output_commitments: [outputs[0].cmx, outputs[1].cmx],
        encrypted_output_hashes: [
            encrypted_output_hash(0, &encrypted_outputs[0].to_bytes().expect("eo0 bytes"))
                .expect("eo0 hash"),
            encrypted_output_hash(1, &encrypted_outputs[1].to_bytes().expect("eo1 bytes"))
                .expect("eo1 hash"),
        ],
        pricing: test_pricing_public_fields(),
        fee: 0,
    }
}

fn asset_orchard_test_encrypted_outputs() -> [AssetOrchardBoundedBytes; ASSET_ORCHARD_LEG_COUNT] {
    [
        AssetOrchardBoundedBytes::from_bytes(
            b"asset-orchard-eo0",
            ASSET_ORCHARD_ENCRYPTED_OUTPUT_MAX_BYTES,
        )
        .expect("eo0"),
        AssetOrchardBoundedBytes::from_bytes(
            b"asset-orchard-eo1",
            ASSET_ORCHARD_ENCRYPTED_OUTPUT_MAX_BYTES,
        )
        .expect("eo1"),
    ]
}

fn asset_orchard_test_accounting_record(
    output_commitment: pallas::Base,
    asset_id: &str,
    amount: u64,
    blinding_seed: &[u8],
) -> AssetOrchardSwapAccountingRecord {
    let tag = AssetTag::derive(asset_id).expect("asset tag");
    let output_commitment = AssetOrchardFieldElement::from_field(output_commitment);
    let blinding = crate::hash_to_pallas_scalar_nonzero(
        "postfiat.asset_orchard.circuit.test.accounting_blinding",
        blinding_seed,
    )
    .expect("accounting blinding");
    asset_orchard_accounting_record(&output_commitment, tag, amount, blinding)
        .expect("accounting record")
}

fn asset_orchard_test_accounting_inputs(
    inputs: &[AssetOrchardSwapNoteWitness; ASSET_ORCHARD_LEG_COUNT],
) -> Vec<AssetOrchardSwapAccountingRecord> {
    vec![
        asset_orchard_test_accounting_record(inputs[0].cmx, "a651", 5, b"a651"),
        asset_orchard_test_accounting_record(inputs[1].cmx, "pfUSDC", 9, b"pfUSDC"),
    ]
}

fn asset_orchard_test_accounting_outputs(
    output_commitments: [pallas::Base; ASSET_ORCHARD_LEG_COUNT],
) -> Vec<AssetOrchardSwapAccountingRecord> {
    vec![
        asset_orchard_test_accounting_record(output_commitments[0], "pfUSDC", 9, b"pfUSDC"),
        asset_orchard_test_accounting_record(output_commitments[1], "a651", 5, b"a651"),
    ]
}

fn signed_asset_orchard_action_from_witness(
    domain: &crate::OrchardAuthorizingDomain,
    inputs: [AssetOrchardSwapNoteWitness; ASSET_ORCHARD_LEG_COUNT],
    outputs: [AssetOrchardSwapNoteWitness; ASSET_ORCHARD_LEG_COUNT],
    anchor: pallas::Base,
    signing_keys: [&SigningKey<SpendAuth>; ASSET_ORCHARD_LEG_COUNT],
    proof: Vec<u8>,
) -> AssetOrchardSwapAction {
    let encrypted_outputs = asset_orchard_test_encrypted_outputs();
    let rk_points = note_swap_rk_points(&inputs);
    let fields = asset_orchard_public_fields_from_witness(
        domain,
        &inputs,
        &outputs,
        anchor,
        &encrypted_outputs,
    );
    let binding =
        AssetOrchardSwapBindingHash::from_bytes(&swap_binding_hash(&fields).expect("binding"));
    let placeholder = AssetOrchardSpendAuthSignature::from_orchard(
        &signing_keys[0].sign(OsRng, b"asset-orchard-placeholder"),
    );
    let mut action = AssetOrchardSwapAction {
        version: ASSET_ORCHARD_ACTION_VERSION_V1,
        schema: ASSET_ORCHARD_ACTION_SCHEMA_V1.to_string(),
        pool_id: ASSET_ORCHARD_POOL_ID_V1.to_string(),
        proof_system_id: ASSET_ORCHARD_PROOF_SYSTEM_ID_V1.to_string(),
        circuit_id: ASSET_ORCHARD_CIRCUIT_ID_V1.to_string(),
        pool_domain: AssetOrchardFieldElement::from_field(fields.pool_domain),
        anchor: AssetOrchardFieldElement::from_field(fields.anchor),
        nullifiers: fields
            .nullifiers
            .into_iter()
            .map(AssetOrchardFieldElement::from_field)
            .collect(),
        randomized_verification_keys: rk_points
            .into_iter()
            .map(|rk| AssetOrchardPoint::from_affine(rk).expect("rk point"))
            .collect(),
        output_commitments: fields
            .output_commitments
            .into_iter()
            .map(AssetOrchardFieldElement::from_field)
            .collect(),
        encrypted_outputs: encrypted_outputs.to_vec(),
        accounting_inputs: asset_orchard_test_accounting_inputs(&inputs),
        accounting_outputs: asset_orchard_test_accounting_outputs(fields.output_commitments),
        pricing_claim: test_pricing_claim(),
        swap_binding_hash: binding,
        fee: 0,
        proof: AssetOrchardProofBytes::from_bytes(&proof).expect("proof bytes"),
        spend_authorization_signatures: vec![placeholder.clone(), placeholder],
    };
    let genesis_hash =
        crate::asset_orchard_domain_genesis_hash(&domain.genesis_hash).expect("genesis32");
    let sighash = action
        .sighash(&domain.chain_id, genesis_hash, domain.protocol_version)
        .expect("sighash");
    action.spend_authorization_signatures = signing_keys
        .into_iter()
        .zip(inputs.iter())
        .map(|(key, input)| {
            let alpha = input
                .spend_authority
                .as_ref()
                .expect("spend authority")
                .alpha;
            AssetOrchardSpendAuthSignature::from_orchard(
                &key.randomize(&alpha).sign(OsRng, &sighash),
            )
        })
        .collect();
    action.validate().expect("signed action validates");
    action
}

#[test]
fn conservation_core_accepts_pairwise_private_swap() {
    let fields = public_fields();
    let circuit = AssetOrchardSwapConservationCircuit::new(
        [leg("a651", 5), leg("pfUSDC", 9)],
        [leg("pfUSDC", 9), leg("a651", 5)],
        true,
        &fields,
    )
    .expect("circuit");
    let instance = circuit.public_instance().expect("instance");
    let prover = MockProver::run(
        ASSET_ORCHARD_CONSERVATION_CORE_K,
        &circuit,
        vec![instance.to_vec()],
    )
    .expect("mock prover");

    prover.assert_satisfied();
}

#[test]
fn pow5_chip_matches_reference_poseidon_for_randomized_action_vectors() {
    for vector in 0u8..8 {
        let mut fields = public_fields();
        let field = |label: &str, suffix: u8| {
            hash_to_pallas_base(label, &[vector, suffix]).expect("randomized field")
        };
        fields.pool_domain = field("p10.pool", 0);
        fields.anchor = field("p10.anchor", 0);
        fields.nullifiers = [field("p10.nullifier", 0), field("p10.nullifier", 1)];
        fields.output_commitments = [field("p10.output", 0), field("p10.output", 1)];
        fields.encrypted_output_hashes = [
            encrypted_output_hash(0, &[vector, 0]).expect("encrypted output hash 0"),
            encrypted_output_hash(1, &[vector, 1]).expect("encrypted output hash 1"),
        ];
        fields.randomized_verification_keys = [
            RandomizedVerificationKeyFields::from_affine(sample_point(&[vector, 0]))
                .expect("randomized key 0"),
            RandomizedVerificationKeyFields::from_affine(sample_point(&[vector, 1]))
                .expect("randomized key 1"),
        ];

        let circuit = AssetOrchardSwapConservationCircuit::new(
            [leg("a651", 5), leg("pfUSDC", 9)],
            [leg("pfUSDC", 9), leg("a651", 5)],
            true,
            &fields,
        )
        .expect("differential circuit");
        let instance = circuit
            .public_instance()
            .expect("reference public instance");
        let prover = MockProver::run(
            ASSET_ORCHARD_CONSERVATION_CORE_K,
            &circuit,
            vec![instance.to_vec()],
        )
        .expect("Pow5 differential prover");
        prover.assert_satisfied();
    }
}

#[test]
fn pricing_binding_rejects_forged_ratio_for_private_amounts() {
    let mut fields = public_fields();
    fields.pricing.ratio_numerator = 10;
    let circuit = AssetOrchardSwapConservationCircuit::new(
        [leg("a651", 5), leg("pfUSDC", 9)],
        [leg("pfUSDC", 9), leg("a651", 5)],
        true,
        &fields,
    )
    .expect("circuit");
    let instance = circuit.public_instance().expect("instance");
    let prover = MockProver::run(
        ASSET_ORCHARD_CONSERVATION_CORE_K,
        &circuit,
        vec![instance.to_vec()],
    )
    .expect("mock prover");
    assert!(prover.verify().is_err(), "forged ratio must not satisfy");
}

#[test]
fn pricing_binding_accepts_deterministic_floor_rounding() {
    let mut fields = public_fields();
    fields.pricing.ratio_numerator = 820_102_177;
    fields.pricing.ratio_denominator = 100_000_000;
    let circuit = AssetOrchardSwapConservationCircuit::new(
        [leg("a651", 1), leg("pfUSDC", 8)],
        [leg("pfUSDC", 8), leg("a651", 1)],
        true,
        &fields,
    )
    .expect("circuit");
    let instance = circuit.public_instance().expect("instance");
    let prover = MockProver::run(
        ASSET_ORCHARD_CONSERVATION_CORE_K,
        &circuit,
        vec![instance.to_vec()],
    )
    .expect("mock prover");
    prover.assert_satisfied();
}

#[test]
fn pricing_binding_rejects_non_floor_rounded_private_value() {
    let mut fields = public_fields();
    fields.pricing.ratio_numerator = 820_102_177;
    fields.pricing.ratio_denominator = 100_000_000;
    let circuit = AssetOrchardSwapConservationCircuit::new(
        [leg("a651", 1), leg("pfUSDC", 9)],
        [leg("pfUSDC", 9), leg("a651", 1)],
        true,
        &fields,
    )
    .expect("circuit");
    let instance = circuit.public_instance().expect("instance");
    let prover = MockProver::run(
        ASSET_ORCHARD_CONSERVATION_CORE_K,
        &circuit,
        vec![instance.to_vec()],
    )
    .expect("mock prover");
    assert!(
        prover.verify().is_err(),
        "non-floor rounded quote value must not satisfy"
    );
}

#[test]
fn pricing_binding_rejects_private_asset_order_under_constraint() {
    let fields = public_fields();
    let circuit = AssetOrchardSwapConservationCircuit::new(
        [leg("pfUSDC", 5), leg("a651", 9)],
        [leg("a651", 9), leg("pfUSDC", 5)],
        true,
        &fields,
    )
    .expect("circuit");
    let instance = circuit.public_instance().expect("instance");
    let prover = MockProver::run(
        ASSET_ORCHARD_CONSERVATION_CORE_K,
        &circuit,
        vec![instance.to_vec()],
    )
    .expect("mock prover");
    assert!(
        prover.verify().is_err(),
        "asset-order substitution must not satisfy"
    );
}

#[test]
fn pricing_claim_commitment_changes_on_epoch_or_packet_mismatch() {
    let original = test_pricing_claim();
    let mut wrong_epoch = original.clone();
    wrong_epoch.nav_epoch += 1;
    let mut wrong_packet = original.clone();
    wrong_packet.reserve_packet_hash = "cd".repeat(48);
    assert_ne!(
        original.commitment_fields().unwrap(),
        wrong_epoch.commitment_fields().unwrap()
    );
    assert_ne!(
        original.commitment_fields().unwrap(),
        wrong_packet.commitment_fields().unwrap()
    );
}

#[test]
fn conservation_core_accepts_uniform_private_swap_selectors() {
    for (selector_rows, outputs) in [
        ([false, false, false], [leg("a651", 5), leg("pfUSDC", 9)]),
        ([true, true, true], [leg("pfUSDC", 9), leg("a651", 5)]),
    ] {
        let fields = public_fields();
        let circuit = AssetOrchardSwapConservationCircuit {
            inputs: [Some(leg("a651", 5)), Some(leg("pfUSDC", 9))],
            outputs: outputs.map(Some),
            input_notes: [None, None],
            output_notes: [None, None],
            permutation_swap: None,
            permutation_swap_rows: Some(selector_rows),
            public_instance: Some(fields.public_instance().expect("instance")),
        };
        let instance = circuit.public_instance().expect("instance");
        let prover = MockProver::run(
            ASSET_ORCHARD_CONSERVATION_CORE_K,
            &circuit,
            vec![instance.to_vec()],
        )
        .expect("mock prover");

        prover.assert_satisfied();
    }
}

#[test]
fn conservation_core_rejects_split_private_swap_selectors() {
    for (selector_rows, outputs) in [
        ([true, true, false], [leg("pfUSDC", 5), leg("a651", 9)]),
        ([false, false, true], [leg("a651", 9), leg("pfUSDC", 5)]),
    ] {
        let fields = public_fields();
        let circuit = AssetOrchardSwapConservationCircuit {
            inputs: [Some(leg("a651", 5)), Some(leg("pfUSDC", 9))],
            outputs: outputs.map(Some),
            input_notes: [None, None],
            output_notes: [None, None],
            permutation_swap: None,
            permutation_swap_rows: Some(selector_rows),
            public_instance: Some(fields.public_instance().expect("instance")),
        };
        let instance = circuit.public_instance().expect("instance");
        let prover = MockProver::run(
            ASSET_ORCHARD_CONSERVATION_CORE_K,
            &circuit,
            vec![instance.to_vec()],
        )
        .expect("mock prover");

        assert!(
            prover.verify().is_err(),
            "MockProver accepted split selector rows {selector_rows:?}"
        );
    }
}

#[test]
fn conservation_core_rejects_forged_nonconserving_swap() {
    let fields = public_fields();
    let circuit = AssetOrchardSwapConservationCircuit::new(
        [leg("a651", 5), leg("pfUSDC", 9)],
        [leg("pfUSDC", 10), leg("a651", 5)],
        true,
        &fields,
    )
    .expect("circuit");
    let instance = circuit.public_instance().expect("instance");
    let prover = MockProver::run(
        ASSET_ORCHARD_CONSERVATION_CORE_K,
        &circuit,
        vec![instance.to_vec()],
    )
    .expect("mock prover");

    assert!(prover.verify().is_err());
}

#[test]
fn conservation_core_rejects_nonzero_fee_public_instance() {
    let mut fields = public_fields();
    fields.fee = 1;
    assert!(fields.public_instance().is_err());

    let fields = public_fields();
    let mut circuit = AssetOrchardSwapConservationCircuit::new(
        [leg("a651", 5), leg("pfUSDC", 9)],
        [leg("a651", 5), leg("pfUSDC", 9)],
        false,
        &fields,
    )
    .expect("circuit");
    let mut instance = circuit.public_instance().expect("instance");
    instance[16] = pallas::Base::ONE;
    circuit.public_instance = Some(instance);
    let prover = MockProver::run(
        ASSET_ORCHARD_CONSERVATION_CORE_K,
        &circuit,
        vec![instance.to_vec()],
    )
    .expect("mock prover");

    assert!(prover.verify().is_err());
}

#[test]
fn conservation_core_rejects_forged_action_context() {
    let fields = public_fields();
    let mut circuit = AssetOrchardSwapConservationCircuit::new(
        [leg("a651", 5), leg("pfUSDC", 9)],
        [leg("a651", 5), leg("pfUSDC", 9)],
        false,
        &fields,
    )
    .expect("circuit");
    let mut instance = circuit.public_instance().expect("instance");
    instance[17] += pallas::Base::ONE;
    circuit.public_instance = Some(instance);
    let prover = MockProver::run(
        ASSET_ORCHARD_CONSERVATION_CORE_K,
        &circuit,
        vec![instance.to_vec()],
    )
    .expect("mock prover");

    assert!(prover.verify().is_err());
}

#[test]
fn conservation_core_rejects_duplicate_public_nullifiers_and_outputs() {
    let mut duplicate_nullifier_fields = public_fields();
    duplicate_nullifier_fields.nullifiers[1] = duplicate_nullifier_fields.nullifiers[0];
    let duplicate_nullifier_circuit = AssetOrchardSwapConservationCircuit::new(
        [leg("a651", 5), leg("pfUSDC", 9)],
        [leg("pfUSDC", 9), leg("a651", 5)],
        true,
        &duplicate_nullifier_fields,
    )
    .expect("duplicate nullifier circuit");
    let duplicate_nullifier_instance = duplicate_nullifier_circuit
        .public_instance()
        .expect("duplicate nullifier instance");
    let duplicate_nullifier_prover = MockProver::run(
        ASSET_ORCHARD_CONSERVATION_CORE_K,
        &duplicate_nullifier_circuit,
        vec![duplicate_nullifier_instance.to_vec()],
    )
    .expect("duplicate nullifier prover");
    assert!(duplicate_nullifier_prover.verify().is_err());

    let mut duplicate_output_fields = public_fields();
    duplicate_output_fields.output_commitments[1] = duplicate_output_fields.output_commitments[0];
    let duplicate_output_circuit = AssetOrchardSwapConservationCircuit::new(
        [leg("a651", 5), leg("pfUSDC", 9)],
        [leg("pfUSDC", 9), leg("a651", 5)],
        true,
        &duplicate_output_fields,
    )
    .expect("duplicate output circuit");
    let duplicate_output_instance = duplicate_output_circuit
        .public_instance()
        .expect("duplicate output instance");
    let duplicate_output_prover = MockProver::run(
        ASSET_ORCHARD_CONSERVATION_CORE_K,
        &duplicate_output_circuit,
        vec![duplicate_output_instance.to_vec()],
    )
    .expect("duplicate output prover");
    assert!(duplicate_output_prover.verify().is_err());
}

#[test]
fn conservation_core_rejects_zero_asset_tag_witness() {
    let fields = public_fields();
    let zero_tag = AssetOrchardSwapPrivateLeg {
        asset_tag: AssetTag { lo: 0, hi: 0 },
        value: 5,
    };
    let pfusdc = leg("pfUSDC", 9);
    let circuit = AssetOrchardSwapConservationCircuit {
        inputs: [Some(zero_tag), Some(pfusdc)],
        outputs: [Some(zero_tag), Some(pfusdc)],
        input_notes: [None, None],
        output_notes: [None, None],
        permutation_swap: Some(false),
        permutation_swap_rows: None,
        public_instance: Some(fields.public_instance().expect("instance")),
    };
    let instance = circuit.public_instance().expect("instance");
    let prover = MockProver::run(
        ASSET_ORCHARD_CONSERVATION_CORE_K,
        &circuit,
        vec![instance.to_vec()],
    )
    .expect("mock prover");

    assert!(prover.verify().is_err());
}

#[test]
fn conservation_core_accepts_single_nonzero_asset_tag_limb() {
    let mut fields = public_fields();
    let hi_only = AssetOrchardSwapPrivateLeg {
        asset_tag: AssetTag { lo: 0, hi: 1 },
        value: 5,
    };
    let lo_only = AssetOrchardSwapPrivateLeg {
        asset_tag: AssetTag { lo: 2, hi: 0 },
        value: 9,
    };
    fields.pricing.base_asset_tag = hi_only.asset_tag;
    fields.pricing.quote_asset_tag = lo_only.asset_tag;
    let circuit = AssetOrchardSwapConservationCircuit {
        inputs: [Some(hi_only), Some(lo_only)],
        outputs: [Some(lo_only), Some(hi_only)],
        input_notes: [None, None],
        output_notes: [None, None],
        permutation_swap: Some(true),
        permutation_swap_rows: None,
        public_instance: Some(fields.public_instance().expect("instance")),
    };
    let instance = circuit.public_instance().expect("instance");
    let prover = MockProver::run(
        ASSET_ORCHARD_CONSERVATION_CORE_K,
        &circuit,
        vec![instance.to_vec()],
    )
    .expect("mock prover");

    prover.assert_satisfied();
}

#[test]
fn conservation_core_rejects_zero_value_witness() {
    let fields = public_fields();
    let zero_value = AssetOrchardSwapPrivateLeg {
        asset_tag: AssetTag::derive("a651").expect("asset tag"),
        value: 0,
    };
    let pfusdc = leg("pfUSDC", 9);
    let circuit = AssetOrchardSwapConservationCircuit {
        inputs: [Some(zero_value), Some(pfusdc)],
        outputs: [Some(zero_value), Some(pfusdc)],
        input_notes: [None, None],
        output_notes: [None, None],
        permutation_swap: Some(false),
        permutation_swap_rows: None,
        public_instance: Some(fields.public_instance().expect("instance")),
    };
    let instance = circuit.public_instance().expect("instance");
    let prover = MockProver::run(
        ASSET_ORCHARD_CONSERVATION_CORE_K,
        &circuit,
        vec![instance.to_vec()],
    )
    .expect("mock prover");

    assert!(prover.verify().is_err());
}

#[test]
fn swap_vk_release_pin_hashes_static_attestation_not_debug_format() {
    assert_eq!(
        asset_orchard_swap_vk_attestation_hash(),
        ASSET_ORCHARD_SWAP_V1_VK_HASH
    );
    assert_ne!(
        ASSET_ORCHARD_SWAP_V1_VK_HASH,
        ASSET_ORCHARD_SWAP_V1_RUNTIME_PINNED_VK_FINGERPRINT
    );
    let attestation =
        std::str::from_utf8(asset_orchard_swap_vk_attestation_bytes()).expect("attestation utf8");
    assert!(attestation.starts_with("postfiat.asset_orchard.swap_vk_attestation.v1\n"));
    assert!(attestation.contains("runtime_pinned_vk_fingerprint="));
    assert!(!attestation.contains("PinnedVerificationKey {"));
}

#[test]
fn private_egress_vk_release_pin_hashes_static_attestation() {
    assert_eq!(
        asset_orchard_private_egress_vk_attestation_hash(),
        ASSET_ORCHARD_PRIVATE_EGRESS_V1_VK_HASH
    );
}

#[test]
#[ignore = "full AssetOrchard note-commitment swap circuit is release-only benchmark-scale"]
fn swap_circuit_recomputes_input_and_output_note_commitments() {
    let mut fields = public_fields();
    let (inputs, outputs, anchor) = note_swap_witnesses(fields.pool_domain);
    fields.anchor = anchor;
    fields.nullifiers = note_swap_nullifiers(fields.pool_domain, &inputs);
    fields.randomized_verification_keys = note_swap_rks(&inputs);
    fields.output_commitments = [outputs[0].cmx, outputs[1].cmx];
    let circuit = AssetOrchardSwapConservationCircuit::new_with_note_witnesses(
        inputs, outputs, true, &fields,
    )
    .expect("circuit");
    let instance = circuit.public_instance().expect("instance");
    let prover = MockProver::run(
        ASSET_ORCHARD_NOTE_COMMITMENT_TEST_K,
        &circuit,
        vec![instance.to_vec()],
    )
    .expect("mock prover");

    prover.assert_satisfied();
}

#[test]
#[ignore = "full AssetOrchard note-commitment swap circuit is release-only benchmark-scale"]
fn swap_circuit_rejects_tampered_output_note_commitment() {
    let mut fields = public_fields();
    let (inputs, outputs, anchor) = note_swap_witnesses(fields.pool_domain);
    fields.anchor = anchor;
    fields.nullifiers = note_swap_nullifiers(fields.pool_domain, &inputs);
    fields.randomized_verification_keys = note_swap_rks(&inputs);
    fields.output_commitments = [outputs[0].cmx, outputs[1].cmx];
    let mut bad_fields = fields;
    bad_fields.output_commitments[0] += pallas::Base::ONE;
    let circuit = AssetOrchardSwapConservationCircuit {
        inputs: inputs
            .each_ref()
            .map(AssetOrchardSwapNoteWitness::leg)
            .map(Some),
        outputs: outputs
            .each_ref()
            .map(AssetOrchardSwapNoteWitness::leg)
            .map(Some),
        input_notes: inputs.map(Some),
        output_notes: outputs.map(Some),
        permutation_swap: Some(true),
        permutation_swap_rows: None,
        public_instance: Some(bad_fields.public_instance().expect("bad instance")),
    };
    let instance = circuit.public_instance().expect("instance");
    let prover = MockProver::run(
        ASSET_ORCHARD_NOTE_COMMITMENT_TEST_K,
        &circuit,
        vec![instance.to_vec()],
    )
    .expect("mock prover");

    assert!(prover.verify().is_err());
}

#[test]
#[ignore = "full AssetOrchard note-commitment swap circuit is release-only benchmark-scale"]
fn swap_circuit_rejects_tampered_input_nullifier() {
    let mut fields = public_fields();
    let (inputs, outputs, anchor) = note_swap_witnesses(fields.pool_domain);
    fields.anchor = anchor;
    fields.nullifiers = note_swap_nullifiers(fields.pool_domain, &inputs);
    fields.randomized_verification_keys = note_swap_rks(&inputs);
    fields.output_commitments = [outputs[0].cmx, outputs[1].cmx];
    let mut bad_fields = fields;
    bad_fields.nullifiers[0] += pallas::Base::ONE;
    let circuit = AssetOrchardSwapConservationCircuit {
        inputs: inputs
            .each_ref()
            .map(AssetOrchardSwapNoteWitness::leg)
            .map(Some),
        outputs: outputs
            .each_ref()
            .map(AssetOrchardSwapNoteWitness::leg)
            .map(Some),
        input_notes: inputs.map(Some),
        output_notes: outputs.map(Some),
        permutation_swap: Some(true),
        permutation_swap_rows: None,
        public_instance: Some(bad_fields.public_instance().expect("bad instance")),
    };
    let instance = circuit.public_instance().expect("instance");
    let prover = MockProver::run(
        ASSET_ORCHARD_NOTE_COMMITMENT_TEST_K,
        &circuit,
        vec![instance.to_vec()],
    )
    .expect("mock prover");

    assert!(prover.verify().is_err());
}

#[test]
#[ignore = "full AssetOrchard note-commitment swap circuit is release-only benchmark-scale"]
fn swap_circuit_rejects_wrong_merkle_anchor() {
    let mut fields = public_fields();
    let (inputs, outputs, anchor) = note_swap_witnesses(fields.pool_domain);
    fields.anchor = anchor + pallas::Base::ONE;
    fields.nullifiers = note_swap_nullifiers(fields.pool_domain, &inputs);
    fields.randomized_verification_keys = note_swap_rks(&inputs);
    fields.output_commitments = [outputs[0].cmx, outputs[1].cmx];
    let circuit = AssetOrchardSwapConservationCircuit {
        inputs: inputs
            .each_ref()
            .map(AssetOrchardSwapNoteWitness::leg)
            .map(Some),
        outputs: outputs
            .each_ref()
            .map(AssetOrchardSwapNoteWitness::leg)
            .map(Some),
        input_notes: inputs.map(Some),
        output_notes: outputs.map(Some),
        permutation_swap: Some(true),
        permutation_swap_rows: None,
        public_instance: Some(fields.public_instance().expect("bad instance")),
    };
    let instance = circuit.public_instance().expect("instance");
    let prover = MockProver::run(
        ASSET_ORCHARD_NOTE_COMMITMENT_TEST_K,
        &circuit,
        vec![instance.to_vec()],
    )
    .expect("mock prover");

    assert!(prover.verify().is_err());
}

#[test]
#[ignore = "full AssetOrchard note-commitment swap circuit is release-only benchmark-scale"]
fn swap_circuit_rejects_wrong_merkle_path() {
    let mut fields = public_fields();
    let (mut inputs, outputs, anchor) = note_swap_witnesses(fields.pool_domain);
    fields.anchor = anchor;
    fields.nullifiers = note_swap_nullifiers(fields.pool_domain, &inputs);
    fields.randomized_verification_keys = note_swap_rks(&inputs);
    fields.output_commitments = [outputs[0].cmx, outputs[1].cmx];
    inputs[0]
        .merkle_witness
        .as_mut()
        .expect("merkle witness")
        .auth_path[0] += pallas::Base::ONE;
    let circuit = AssetOrchardSwapConservationCircuit {
        inputs: inputs
            .each_ref()
            .map(AssetOrchardSwapNoteWitness::leg)
            .map(Some),
        outputs: outputs
            .each_ref()
            .map(AssetOrchardSwapNoteWitness::leg)
            .map(Some),
        input_notes: inputs.map(Some),
        output_notes: outputs.map(Some),
        permutation_swap: Some(true),
        permutation_swap_rows: None,
        public_instance: Some(fields.public_instance().expect("instance")),
    };
    let instance = circuit.public_instance().expect("instance");
    let prover = MockProver::run(
        ASSET_ORCHARD_NOTE_COMMITMENT_TEST_K,
        &circuit,
        vec![instance.to_vec()],
    )
    .expect("mock prover");

    assert!(prover.verify().is_err());
}

#[test]
#[ignore = "full AssetOrchard note-commitment swap circuit is release-only benchmark-scale"]
fn swap_circuit_rejects_tampered_randomized_verification_key() {
    let mut fields = public_fields();
    let (inputs, outputs, anchor) = note_swap_witnesses(fields.pool_domain);
    fields.anchor = anchor;
    fields.nullifiers = note_swap_nullifiers(fields.pool_domain, &inputs);
    fields.randomized_verification_keys = note_swap_rks(&inputs);
    fields.randomized_verification_keys[0].x += pallas::Base::ONE;
    fields.output_commitments = [outputs[0].cmx, outputs[1].cmx];
    let circuit = AssetOrchardSwapConservationCircuit {
        inputs: inputs
            .each_ref()
            .map(AssetOrchardSwapNoteWitness::leg)
            .map(Some),
        outputs: outputs
            .each_ref()
            .map(AssetOrchardSwapNoteWitness::leg)
            .map(Some),
        input_notes: inputs.map(Some),
        output_notes: outputs.map(Some),
        permutation_swap: Some(true),
        permutation_swap_rows: None,
        public_instance: Some(fields.public_instance().expect("bad instance")),
    };
    let instance = circuit.public_instance().expect("instance");
    let prover = MockProver::run(
        ASSET_ORCHARD_NOTE_COMMITMENT_TEST_K,
        &circuit,
        vec![instance.to_vec()],
    )
    .expect("mock prover");

    assert!(prover.verify().is_err());
}

#[test]
#[ignore = "full AssetOrchard note-commitment swap circuit is release-only benchmark-scale"]
fn swap_circuit_rejects_wrong_spend_authority_relation() {
    let mut fields = public_fields();
    let (mut inputs, outputs, anchor) = note_swap_witnesses(fields.pool_domain);
    fields.anchor = anchor;
    fields.nullifiers = note_swap_nullifiers(fields.pool_domain, &inputs);
    fields.randomized_verification_keys = note_swap_rks(&inputs);
    fields.output_commitments = [outputs[0].cmx, outputs[1].cmx];
    inputs[0]
        .spend_authority
        .as_mut()
        .expect("spend authority")
        .rivk += pallas::Scalar::ONE;
    let circuit = AssetOrchardSwapConservationCircuit {
        inputs: inputs
            .each_ref()
            .map(AssetOrchardSwapNoteWitness::leg)
            .map(Some),
        outputs: outputs
            .each_ref()
            .map(AssetOrchardSwapNoteWitness::leg)
            .map(Some),
        input_notes: inputs.map(Some),
        output_notes: outputs.map(Some),
        permutation_swap: Some(true),
        permutation_swap_rows: None,
        public_instance: Some(fields.public_instance().expect("instance")),
    };
    let instance = circuit.public_instance().expect("instance");
    let prover = MockProver::run(
        ASSET_ORCHARD_NOTE_COMMITMENT_TEST_K,
        &circuit,
        vec![instance.to_vec()],
    )
    .expect("mock prover");

    assert!(prover.verify().is_err());
}

#[test]
fn conservation_core_real_halo2_proof_verifies_and_binds_instance() {
    let fields = public_fields();
    let circuit = AssetOrchardSwapConservationCircuit::new(
        [leg("a651", 5), leg("pfUSDC", 9)],
        [leg("pfUSDC", 9), leg("a651", 5)],
        true,
        &fields,
    )
    .expect("circuit");
    let instance = circuit.public_instance().expect("instance");
    let proving_key = AssetOrchardConservationProvingKey::build().expect("pk");
    let verifying_key = AssetOrchardConservationVerifyingKey::build().expect("vk");
    let proof = proving_key.create_proof(&circuit, OsRng).expect("proof");

    verifying_key
        .verify_proof(&proof, &instance)
        .expect("proof verifies");

    let mut mutated_instance = instance;
    mutated_instance[8] += pallas::Base::ONE;
    assert!(verifying_key
        .verify_proof(&proof, &mutated_instance)
        .is_err());

    let mut tampered_proof = proof;
    let last = tampered_proof.last_mut().expect("proof byte");
    *last ^= 0x01;
    assert!(verifying_key
        .verify_proof(&tampered_proof, &instance)
        .is_err());
}

#[test]
#[ignore = "private egress note/Merkle/spend-authority MockProver is release-scale"]
fn private_egress_mock_prover_binds_note_to_public_exit_instance() {
    let domain = asset_swap_test_domain();
    let pool_domain = asset_orchard_pool_domain_for_domain(&domain);
    let input = note_witness(pool_domain, "a651", 5, 81);
    let dummy = note_witness(pool_domain, "dummy", 7, 82);
    let (anchor, witnesses) = asset_merkle_witnesses([input.cmx, dummy.cmx]);
    let input = input.with_merkle_witness(witnesses[0].clone());
    let nullifier = asset_derive_nullifier(
        pool_domain,
        input.nk,
        input.note.rho,
        input.note.psi,
        input.cmx,
    )
    .expect("nullifier");
    let authority = input.spend_authority.as_ref().expect("spend authority");
    let rk_point =
        (pallas::Point::from(authority.ak) + asset_spend_auth_g() * authority.alpha).to_affine();
    let exit_binding_hash = asset_orchard_private_egress_exit_binding_hash(
        &AssetOrchardPrivateEgressExitBindingPreimage {
            chain_id: &domain.chain_id,
            genesis_hash: crate::asset_orchard_domain_genesis_hash(&domain.genesis_hash)
                .expect("genesis32"),
            protocol_version: domain.protocol_version,
            pool_id: ASSET_ORCHARD_POOL_ID_V1,
            circuit_id: ASSET_ORCHARD_PRIVATE_EGRESS_CIRCUIT_ID_V1,
            pool_domain,
            to: "alice",
            asset_id: "a651",
            amount: 5,
            fee: 0,
            policy_id: "postfiat.asset_orchard.private_egress.test",
            disclosure_hash: "test-disclosure",
        },
    )
    .expect("exit binding");
    let fields = AssetOrchardPrivateEgressPublicFields {
        pool_domain,
        anchor,
        nullifier,
        randomized_verification_key: RandomizedVerificationKeyFields::from_affine(rk_point)
            .expect("rk"),
        asset_tag: AssetTag::derive("a651").expect("asset tag"),
        amount: 5,
        fee: 0,
        exit_binding_hash,
    };
    let circuit = AssetOrchardPrivateEgressCircuit::new_with_note_witness(input, &fields)
        .expect("private egress circuit");
    let instance = circuit.public_instance().expect("public instance");
    let prover = MockProver::run(
        ASSET_ORCHARD_NOTE_COMMITMENT_TEST_K,
        &circuit,
        vec![instance.to_vec()],
    )
    .expect("mock prover");
    prover.assert_satisfied();

    let mut tampered_instance = instance;
    tampered_instance[7] += pallas::Base::ONE;
    let tampered = MockProver::run(
        ASSET_ORCHARD_NOTE_COMMITMENT_TEST_K,
        &circuit,
        vec![tampered_instance.to_vec()],
    )
    .expect("tampered mock prover");
    assert!(tampered.verify().is_err());
}

#[test]
#[ignore = "full AssetOrchard swap proof verification is release-only benchmark-scale"]
fn swap_full_vk_rejects_missing_or_partial_note_witnesses() {
    let proving_key = AssetOrchardSwapProvingKey::build().expect("swap proving key");

    let fields = public_fields();
    let no_note_circuit = AssetOrchardSwapConservationCircuit::new(
        [leg("a651", 5), leg("pfUSDC", 9)],
        [leg("pfUSDC", 9), leg("a651", 5)],
        true,
        &fields,
    )
    .expect("no-note circuit");
    assert_eq!(
        proving_key
            .create_proof(&no_note_circuit, OsRng)
            .expect_err("full swap proof must require note witnesses")
            .code(),
        "asset_orchard_swap_missing_note_witness"
    );

    let mut fields = public_fields();
    let (inputs, outputs, anchor) = note_swap_witnesses(fields.pool_domain);
    fields.anchor = anchor;
    fields.nullifiers = note_swap_nullifiers(fields.pool_domain, &inputs);
    fields.randomized_verification_keys = note_swap_rks(&inputs);
    fields.output_commitments = [outputs[0].cmx, outputs[1].cmx];
    let partial_note_circuit = AssetOrchardSwapConservationCircuit {
        inputs: inputs
            .each_ref()
            .map(AssetOrchardSwapNoteWitness::leg)
            .map(Some),
        outputs: outputs
            .each_ref()
            .map(AssetOrchardSwapNoteWitness::leg)
            .map(Some),
        input_notes: [Some(inputs[0].clone()), None],
        output_notes: outputs.map(Some),
        permutation_swap: Some(true),
        permutation_swap_rows: None,
        public_instance: Some(fields.public_instance().expect("instance")),
    };
    assert_eq!(
        proving_key
            .create_proof(&partial_note_circuit, OsRng)
            .expect_err("full swap proof must reject partial note witnesses")
            .code(),
        "asset_orchard_swap_missing_note_witness"
    );
}

#[test]
#[ignore = "full AssetOrchard swap proof verification is release-only benchmark-scale"]
fn zk_prover_baseline_benchmark() {
    let available_parallelism = std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1);
    let domain = asset_swap_test_domain();
    let signing_key0 = spend_auth_signing_key(61);
    let signing_key1 = spend_auth_signing_key(62);
    let pool_domain = asset_orchard_pool_domain_for_domain(&domain);
    let (inputs, outputs, anchor) =
        signed_note_swap_witnesses(pool_domain, [&signing_key0, &signing_key1]);
    let encrypted_outputs = asset_orchard_test_encrypted_outputs();
    let fields = asset_orchard_public_fields_from_witness(
        &domain,
        &inputs,
        &outputs,
        anchor,
        &encrypted_outputs,
    );
    let circuit = AssetOrchardSwapConservationCircuit::new_with_note_witnesses(
        inputs, outputs, true, &fields,
    )
    .expect("valid swap circuit");
    let instance = circuit.public_instance().expect("public instance");

    let pk_start = std::time::Instant::now();
    let proving_key = AssetOrchardSwapProvingKey::build().expect("swap proving key");
    let pk_build_ms = pk_start.elapsed().as_millis();

    let prove_start = std::time::Instant::now();
    let proof = proving_key
        .create_proof(&circuit, OsRng)
        .expect("valid proof");
    let baseline_prove_ms = prove_start.elapsed().as_millis();

    let vk_start = std::time::Instant::now();
    let verifying_key = AssetOrchardSwapVerifyingKey::build().expect("swap verifying key");
    let vk_build_ms = vk_start.elapsed().as_millis();

    let verify_start = std::time::Instant::now();
    verifying_key
        .verify_proof(&proof, &instance)
        .expect("proof verifies");
    let baseline_verify_ms = verify_start.elapsed().as_millis();

    println!(
        "zk_prover_baseline_benchmark pk_build_ms={pk_build_ms} baseline_prove_ms={baseline_prove_ms} vk_build_ms={vk_build_ms} baseline_verify_ms={baseline_verify_ms} proof_bytes={} K={} available_parallelism={available_parallelism}",
        proof.len(),
        ASSET_ORCHARD_SWAP_V1_K,
    );
}

#[test]
#[ignore = "full AssetOrchard swap proof verification is release-only benchmark-scale"]
fn zk_prover_cached_key_benchmark() {
    let available_parallelism = std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1);
    let domain = asset_swap_test_domain();
    let signing_key0 = spend_auth_signing_key(71);
    let signing_key1 = spend_auth_signing_key(72);
    let pool_domain = asset_orchard_pool_domain_for_domain(&domain);
    let (inputs, outputs, anchor) =
        signed_note_swap_witnesses(pool_domain, [&signing_key0, &signing_key1]);
    let encrypted_outputs = asset_orchard_test_encrypted_outputs();
    let fields = asset_orchard_public_fields_from_witness(
        &domain,
        &inputs,
        &outputs,
        anchor,
        &encrypted_outputs,
    );
    let circuit = AssetOrchardSwapConservationCircuit::new_with_note_witnesses(
        inputs, outputs, true, &fields,
    )
    .expect("valid swap circuit");
    let instance = circuit.public_instance().expect("public instance");

    let cold_pk_start = std::time::Instant::now();
    let proving_key = AssetOrchardSwapProvingKey::cached().expect("cached proving key");
    let cold_pk_lookup_ms = cold_pk_start.elapsed().as_millis();

    let first_prove_start = std::time::Instant::now();
    let first_proof = proving_key
        .create_proof(&circuit, OsRng)
        .expect("first proof");
    let first_prove_ms = first_prove_start.elapsed().as_millis();

    let cold_vk_start = std::time::Instant::now();
    let verifying_key = AssetOrchardSwapVerifyingKey::cached().expect("cached verifying key");
    let cold_vk_lookup_ms = cold_vk_start.elapsed().as_millis();

    let first_verify_start = std::time::Instant::now();
    verifying_key
        .verify_proof(&first_proof, &instance)
        .expect("first proof verifies");
    let first_verify_ms = first_verify_start.elapsed().as_millis();

    let hot_pk_start = std::time::Instant::now();
    let hot_proving_key = AssetOrchardSwapProvingKey::cached().expect("hot proving key");
    let hot_pk_lookup_ms = hot_pk_start.elapsed().as_millis();
    assert!(std::ptr::eq(proving_key, hot_proving_key));

    let second_prove_start = std::time::Instant::now();
    let second_proof = hot_proving_key
        .create_proof(&circuit, OsRng)
        .expect("second proof");
    let second_prove_ms = second_prove_start.elapsed().as_millis();

    let hot_vk_start = std::time::Instant::now();
    let hot_verifying_key = AssetOrchardSwapVerifyingKey::cached().expect("hot verifying key");
    let hot_vk_lookup_ms = hot_vk_start.elapsed().as_millis();
    assert!(std::ptr::eq(verifying_key, hot_verifying_key));

    let second_verify_start = std::time::Instant::now();
    hot_verifying_key
        .verify_proof(&second_proof, &instance)
        .expect("second proof verifies");
    let second_verify_ms = second_verify_start.elapsed().as_millis();

    println!(
        "zk_prover_cached_key_benchmark cold_pk_lookup_ms={cold_pk_lookup_ms} first_prove_ms={first_prove_ms} cold_vk_lookup_ms={cold_vk_lookup_ms} first_verify_ms={first_verify_ms} hot_pk_lookup_ms={hot_pk_lookup_ms} second_prove_ms={second_prove_ms} hot_vk_lookup_ms={hot_vk_lookup_ms} second_verify_ms={second_verify_ms} proof_bytes={} K={} available_parallelism={available_parallelism}",
        second_proof.len(),
        ASSET_ORCHARD_SWAP_V1_K,
    );
}

#[test]
#[ignore = "full AssetOrchard swap proof verification is release-only benchmark-scale"]
fn swap_consensus_verifier_accepts_real_proof_and_rejects_forged_nonconservation() {
    let domain = asset_swap_test_domain();
    let signing_key0 = spend_auth_signing_key(51);
    let signing_key1 = spend_auth_signing_key(52);
    let pool_domain = asset_orchard_pool_domain_for_domain(&domain);
    let (inputs, outputs, anchor) =
        signed_note_swap_witnesses(pool_domain, [&signing_key0, &signing_key1]);
    let encrypted_outputs = asset_orchard_test_encrypted_outputs();
    let fields = asset_orchard_public_fields_from_witness(
        &domain,
        &inputs,
        &outputs,
        anchor,
        &encrypted_outputs,
    );
    let circuit = AssetOrchardSwapConservationCircuit::new_with_note_witnesses(
        inputs.clone(),
        outputs.clone(),
        true,
        &fields,
    )
    .expect("valid swap circuit");
    let proving_key = AssetOrchardSwapProvingKey::build().expect("swap proving key");
    let proof = proving_key
        .create_proof(&circuit, OsRng)
        .expect("valid proof");
    let action = signed_asset_orchard_action_from_witness(
        &domain,
        inputs.clone(),
        outputs.clone(),
        anchor,
        [&signing_key0, &signing_key1],
        proof,
    );

    crate::reset_asset_orchard_swap_timings();
    crate::verify_serialized_asset_orchard_swap_action(&action, &domain)
        .expect("real asset-orchard proof verifies at consensus boundary");
    let valid_timings = crate::take_asset_orchard_swap_timings();
    assert_eq!(valid_timings.proof_verifications.len(), 1);
    assert!(valid_timings.proof_verifications[0].halo2_verify_proof_ms > 0.0);
    assert_eq!(valid_timings.proof_verifications[0].result, "ok");

    let mut tampered = action.clone();
    tampered.output_commitments[0] =
        AssetOrchardFieldElement::from_field(outputs[0].cmx + pallas::Base::ONE);
    tampered.accounting_outputs[0].output_commitment =
        tampered.output_commitments[0].as_hex().to_string();
    let mut tampered_fields = fields;
    tampered_fields.output_commitments[0] = outputs[0].cmx + pallas::Base::ONE;
    tampered.swap_binding_hash = AssetOrchardSwapBindingHash::from_bytes(
        &swap_binding_hash(&tampered_fields).expect("tampered binding"),
    );
    let genesis_hash =
        crate::asset_orchard_domain_genesis_hash(&domain.genesis_hash).expect("genesis32");
    let sighash = tampered
        .sighash(&domain.chain_id, genesis_hash, domain.protocol_version)
        .expect("tampered sighash");
    tampered.spend_authorization_signatures = [&signing_key0, &signing_key1]
        .into_iter()
        .zip(inputs.iter())
        .map(|(key, input)| {
            let alpha = input
                .spend_authority
                .as_ref()
                .expect("spend authority")
                .alpha;
            AssetOrchardSpendAuthSignature::from_orchard(
                &key.randomize(&alpha).sign(OsRng, &sighash),
            )
        })
        .collect();
    crate::reset_asset_orchard_swap_timings();
    assert_eq!(
        crate::verify_serialized_asset_orchard_swap_action(&tampered, &domain)
            .expect_err("proof must not replay across a changed public instance")
            .code(),
        "asset_orchard_swap_proof_verification_failed"
    );
    let rejected_timings = crate::take_asset_orchard_swap_timings();
    assert_eq!(rejected_timings.proof_verifications.len(), 1);
    assert!(rejected_timings.proof_verifications[0].halo2_verify_proof_ms > 0.0);
    assert_eq!(rejected_timings.proof_verifications[0].result, "error");

    let mut forged_outputs = outputs.clone();
    forged_outputs[0] = note_witness_with_rho(pool_domain, "pfUSDC", 10, 43, outputs[0].note.rho);
    let forged_fields = asset_orchard_public_fields_from_witness(
        &domain,
        &inputs,
        &forged_outputs,
        anchor,
        &encrypted_outputs,
    );
    let forged_circuit = AssetOrchardSwapConservationCircuit::new_with_note_witnesses(
        inputs.clone(),
        forged_outputs.clone(),
        true,
        &forged_fields,
    )
    .expect("forged circuit object");
    if let Ok(forged_proof) = proving_key.create_proof(&forged_circuit, OsRng) {
        let forged_action = signed_asset_orchard_action_from_witness(
            &domain,
            inputs,
            forged_outputs,
            anchor,
            [&signing_key0, &signing_key1],
            forged_proof,
        );
        assert_eq!(
            crate::verify_serialized_asset_orchard_swap_action(&forged_action, &domain)
                .expect_err("non-conserving swap proof must be rejected")
                .code(),
            "asset_orchard_swap_proof_verification_failed"
        );
    }
}

#[test]
#[ignore = "full AssetOrchard swap keygen is release-only benchmark-scale"]
fn swap_full_shape_key_metadata_is_pinned_and_consistent() {
    let params = Params::new(ASSET_ORCHARD_SWAP_V1_K);
    let full_shape = AssetOrchardSwapConservationCircuit::full_shape();
    let (vk, pinned_assembly) = keygen_vk_pinned_assembly(&params, &full_shape).expect("swap vk");
    let metadata = AssetOrchardSwapPinnedMetadata::from_vk(&vk, ASSET_ORCHARD_SWAP_V1_K)
        .expect("swap metadata");
    eprintln!("swap_pinned_metadata={metadata:#?}");
    metadata.validate_release_pin().expect("release pin");
    assert_eq!(
        metadata.circuit_id,
        crate::asset_orchard::ASSET_ORCHARD_CIRCUIT_ID_V1
    );
    assert_eq!(metadata.k, ASSET_ORCHARD_SWAP_V1_K);
    assert_eq!(
        metadata.public_instance_len,
        ASSET_ORCHARD_PUBLIC_INSTANCE_LEN
    );
    assert_eq!(metadata.merkle_tree_depth, ASSET_ORCHARD_MERKLE_DEPTH);
    assert_eq!(metadata.vk_hash.len(), 96);
    assert_eq!(
        metadata.public_instance_layout_hash,
        ASSET_ORCHARD_SWAP_V1_PUBLIC_INSTANCE_LAYOUT_HASH
    );
    assert_eq!(metadata.params_hash, ASSET_ORCHARD_SWAP_V1_PARAMS_HASH);
    assert_eq!(metadata.vk_hash, ASSET_ORCHARD_SWAP_V1_VK_HASH);
    assert_eq!(
        metadata.poseidon_parameter_hash,
        ASSET_ORCHARD_SWAP_V1_POSEIDON_PARAMETER_HASH
    );
    assert_eq!(
        metadata.note_message_layout_hash,
        ASSET_ORCHARD_SWAP_V1_NOTE_MESSAGE_LAYOUT_HASH
    );
    assert_eq!(
        metadata.merkle_parameter_hash,
        ASSET_ORCHARD_SWAP_V1_MERKLE_PARAMETER_HASH
    );
    assert_eq!(
        metadata.runtime_pinned_vk_fingerprint,
        ASSET_ORCHARD_SWAP_V1_RUNTIME_PINNED_VK_FINGERPRINT
    );

    if let Some(path) = swap_vk_artifact_write_path() {
        write_swap_vk_artifact(&path, &pinned_assembly, &metadata)
            .expect("write swap pinned vk artifact");
        eprintln!("wrote_swap_vk_artifact={}", path.display());
    }
}

#[test]
fn swap_embedded_vk_artifact_loads_and_matches_release_pin() {
    crate::timing::reset_asset_orchard_swap_timings();
    let verifying_key = AssetOrchardSwapVerifyingKey::build().expect("embedded swap vk");
    verifying_key
        .metadata()
        .validate_release_pin()
        .expect("release pin");
    assert_eq!(
        verifying_key.metadata().runtime_pinned_vk_fingerprint,
        ASSET_ORCHARD_SWAP_V1_RUNTIME_PINNED_VK_FINGERPRINT
    );

    let timings = crate::timing::take_asset_orchard_swap_timings();
    assert_eq!(timings.vk_builds.len(), 1);
    let build = &timings.vk_builds[0];
    assert_eq!(build.artifact_mode, "embedded");
    assert_eq!(build.keygen_vk_ms, 0.0);
    assert!(build.artifact_decode_ms > 0.0);
    assert!(build.artifact_vk_reconstruct_ms > 0.0);
    assert_eq!(build.result, "ok");
}

#[test]
fn swap_vk_artifact_tamper_fails_closed() {
    let mut artifact = ASSET_ORCHARD_SWAP_VK_EMBEDDED_ARTIFACT.to_vec();
    let last = artifact.len() - 1;
    artifact[last] ^= 1;
    let error = decode_swap_vk_artifact(&artifact).expect_err("tampered artifact must fail");
    assert_eq!(
        error.code(),
        "asset_orchard_swap_vk_artifact_payload_mismatch"
    );
}

#[test]
fn swap_vk_artifact_wrong_schema_and_metadata_fail_closed() {
    let mut wrong_schema = ASSET_ORCHARD_SWAP_VK_EMBEDDED_ARTIFACT.to_vec();
    wrong_schema[0] = b'X';
    let error = decode_swap_vk_artifact(&wrong_schema).expect_err("wrong schema must fail closed");
    assert_eq!(
        error.code(),
        "asset_orchard_swap_vk_artifact_schema_mismatch"
    );

    let mut wrong_metadata = ASSET_ORCHARD_SWAP_VK_EMBEDDED_ARTIFACT.to_vec();
    let needle = format!("circuit_id={ASSET_ORCHARD_CIRCUIT_ID_V1}");
    let offset = wrong_metadata
        .windows(needle.len())
        .position(|window| window == needle.as_bytes())
        .expect("embedded circuit id");
    wrong_metadata[offset + needle.len() - 1] ^= 1;
    let error =
        decode_swap_vk_artifact(&wrong_metadata).expect_err("wrong metadata must fail closed");
    assert_eq!(
        error.code(),
        "asset_orchard_swap_vk_artifact_metadata_mismatch"
    );
}

#[cfg(not(feature = "asset-orchard-vk-dev-env"))]
#[test]
fn swap_vk_env_overrides_are_ignored_without_dev_feature() {
    let _guard = SWAP_VK_ENV_TEST_LOCK.lock().expect("env test lock");
    let write_path = std::env::temp_dir().join(format!(
        "postfiat-swap-vk-env-ignored-{}-{}.bin",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time")
            .as_nanos()
    ));
    let _ = std::fs::remove_file(&write_path);
    let previous_load = std::env::var("POSTFIAT_ASSET_ORCHARD_SWAP_VK_ARTIFACT").ok();
    let previous_write = std::env::var("POSTFIAT_ASSET_ORCHARD_SWAP_VK_WRITE_ARTIFACT").ok();
    let previous_rebuild = std::env::var("POSTFIAT_ASSET_ORCHARD_SWAP_VK_REBUILD").ok();

    std::env::set_var(
        "POSTFIAT_ASSET_ORCHARD_SWAP_VK_ARTIFACT",
        "/definitely/missing/swap-vk.bin",
    );
    std::env::set_var("POSTFIAT_ASSET_ORCHARD_SWAP_VK_WRITE_ARTIFACT", &write_path);
    std::env::set_var("POSTFIAT_ASSET_ORCHARD_SWAP_VK_REBUILD", "true");

    crate::timing::reset_asset_orchard_swap_timings();
    let verifying_key = AssetOrchardSwapVerifyingKey::build()
        .expect("production build must ignore swap VK env overrides");
    verifying_key
        .metadata()
        .validate_release_pin()
        .expect("release pin");
    assert!(!write_path.exists());
    let timings = crate::timing::take_asset_orchard_swap_timings();
    assert_eq!(timings.vk_builds.len(), 1);
    assert_eq!(timings.vk_builds[0].artifact_mode, "embedded");
    assert_eq!(timings.vk_builds[0].keygen_vk_ms, 0.0);
    assert_eq!(timings.vk_builds[0].result, "ok");

    match previous_load {
        Some(value) => std::env::set_var("POSTFIAT_ASSET_ORCHARD_SWAP_VK_ARTIFACT", value),
        None => std::env::remove_var("POSTFIAT_ASSET_ORCHARD_SWAP_VK_ARTIFACT"),
    }
    match previous_write {
        Some(value) => std::env::set_var("POSTFIAT_ASSET_ORCHARD_SWAP_VK_WRITE_ARTIFACT", value),
        None => std::env::remove_var("POSTFIAT_ASSET_ORCHARD_SWAP_VK_WRITE_ARTIFACT"),
    }
    match previous_rebuild {
        Some(value) => std::env::set_var("POSTFIAT_ASSET_ORCHARD_SWAP_VK_REBUILD", value),
        None => std::env::remove_var("POSTFIAT_ASSET_ORCHARD_SWAP_VK_REBUILD"),
    }
    let _ = std::fs::remove_file(write_path);
}

#[test]
fn private_egress_embedded_vk_artifact_loads_and_matches_release_pin() {
    crate::timing::reset_asset_orchard_private_egress_timings();
    let verifying_key =
        AssetOrchardPrivateEgressVerifyingKey::build().expect("embedded private egress vk");
    verifying_key
        .metadata()
        .validate_release_pin()
        .expect("release pin");
    assert_eq!(
        verifying_key.metadata().runtime_pinned_vk_fingerprint,
        ASSET_ORCHARD_PRIVATE_EGRESS_V1_RUNTIME_PINNED_VK_FINGERPRINT
    );

    let timings = crate::timing::take_asset_orchard_private_egress_timings();
    assert_eq!(timings.vk_builds.len(), 1);
    let build = &timings.vk_builds[0];
    assert_eq!(build.artifact_mode, "embedded");
    assert_eq!(build.keygen_vk_ms, 0.0);
    assert!(build.artifact_decode_ms > 0.0);
    assert!(build.artifact_vk_reconstruct_ms > 0.0);
    assert_eq!(build.result, "ok");
}

#[test]
fn shared_circuit_embedded_vk_artifacts_close_together() {
    let swap = AssetOrchardSwapVerifyingKey::build()
        .expect("shared-circuit swap artifact must reconstruct");
    swap.metadata()
        .validate_release_pin()
        .expect("shared-circuit swap artifact must match its release pin");

    let private_egress = AssetOrchardPrivateEgressVerifyingKey::build()
        .expect("shared-circuit private-egress artifact must reconstruct");
    private_egress
        .metadata()
        .validate_release_pin()
        .expect("shared-circuit private-egress artifact must match its release pin");

    assert_eq!(
        swap.metadata().runtime_pinned_vk_fingerprint,
        ASSET_ORCHARD_SWAP_V1_RUNTIME_PINNED_VK_FINGERPRINT
    );
    assert_eq!(
        private_egress.metadata().runtime_pinned_vk_fingerprint,
        ASSET_ORCHARD_PRIVATE_EGRESS_V1_RUNTIME_PINNED_VK_FINGERPRINT
    );
}

#[test]
fn current_and_replay_vk_identities_are_distinct_and_exactly_pinned() {
    assert_ne!(
        crate::asset_orchard::ASSET_ORCHARD_CIRCUIT_ID_V4,
        crate::asset_orchard::ASSET_ORCHARD_CIRCUIT_ID_V3_REPLAY
    );
    assert_ne!(
        crate::asset_orchard::ASSET_ORCHARD_PRIVATE_EGRESS_CIRCUIT_ID_V2,
        crate::asset_orchard::ASSET_ORCHARD_PRIVATE_EGRESS_CIRCUIT_ID_V1_REPLAY
    );
    assert_eq!(
        asset_orchard_swap_vk_attestation_hash_for_circuit(
            crate::asset_orchard::ASSET_ORCHARD_CIRCUIT_ID_V4
        )
        .expect("current swap attestation"),
        ASSET_ORCHARD_SWAP_V1_VK_HASH
    );
    assert_eq!(
        asset_orchard_swap_vk_attestation_hash_for_circuit(
            crate::asset_orchard::ASSET_ORCHARD_CIRCUIT_ID_V3_REPLAY
        )
        .expect("replay swap attestation"),
        ASSET_ORCHARD_SWAP_V3_REPLAY_VK_HASH
    );
    assert_eq!(
        asset_orchard_private_egress_vk_attestation_hash_for_circuit(
            crate::asset_orchard::ASSET_ORCHARD_PRIVATE_EGRESS_CIRCUIT_ID_V2
        )
        .expect("current private-egress attestation"),
        ASSET_ORCHARD_PRIVATE_EGRESS_V1_VK_HASH
    );
    assert_eq!(
        asset_orchard_private_egress_vk_attestation_hash_for_circuit(
            crate::asset_orchard::ASSET_ORCHARD_PRIVATE_EGRESS_CIRCUIT_ID_V1_REPLAY
        )
        .expect("replay private-egress attestation"),
        ASSET_ORCHARD_PRIVATE_EGRESS_V1_REPLAY_VK_HASH
    );
    assert_ne!(
        ASSET_ORCHARD_SWAP_V1_RUNTIME_PINNED_VK_FINGERPRINT,
        ASSET_ORCHARD_SWAP_V3_REPLAY_RUNTIME_PINNED_VK_FINGERPRINT
    );
    assert_ne!(
        ASSET_ORCHARD_PRIVATE_EGRESS_V1_RUNTIME_PINNED_VK_FINGERPRINT,
        ASSET_ORCHARD_PRIVATE_EGRESS_V1_REPLAY_RUNTIME_PINNED_VK_FINGERPRINT
    );
    assert_eq!(
        AssetOrchardSwapVerifyingKey::cached_for_archive_replay("unknown.swap.circuit")
            .expect_err("unknown swap circuit must fail closed")
            .code(),
        "unsupported_asset_orchard_circuit"
    );
    assert_eq!(
        AssetOrchardPrivateEgressVerifyingKey::cached_for_archive_replay(
            "unknown.private_egress.circuit",
        )
        .expect_err("unknown private-egress circuit must fail closed")
        .code(),
        "unsupported_asset_orchard_private_egress_circuit"
    );
}

#[test]
fn replay_only_vk_artifacts_reconstruct_with_historical_circuit_shapes() {
    let swap = AssetOrchardSwapVerifyingKey::build_v3_replay()
        .expect("historical swap v3 VK must reconstruct");
    swap.metadata()
        .validate_release_pin()
        .expect("historical swap v3 pin");
    assert_eq!(
        swap.metadata().circuit_id,
        crate::asset_orchard::ASSET_ORCHARD_CIRCUIT_ID_V3_REPLAY
    );
    assert_eq!(
        swap.metadata().runtime_pinned_vk_fingerprint,
        ASSET_ORCHARD_SWAP_V3_REPLAY_RUNTIME_PINNED_VK_FINGERPRINT
    );

    let private_egress = AssetOrchardPrivateEgressVerifyingKey::build_v1_replay()
        .expect("historical private-egress v1 VK must reconstruct");
    private_egress
        .metadata()
        .validate_release_pin()
        .expect("historical private-egress v1 pin");
    assert_eq!(
        private_egress.metadata().circuit_id,
        crate::asset_orchard::ASSET_ORCHARD_PRIVATE_EGRESS_CIRCUIT_ID_V1_REPLAY
    );
    assert_eq!(
        private_egress.metadata().runtime_pinned_vk_fingerprint,
        ASSET_ORCHARD_PRIVATE_EGRESS_V1_REPLAY_RUNTIME_PINNED_VK_FINGERPRINT
    );
}

#[test]
fn vk_artifacts_reject_cross_identity_and_replay_tampering() {
    assert_eq!(
        decode_swap_vk_artifact_for_circuit(
            ASSET_ORCHARD_SWAP_VK_EMBEDDED_ARTIFACT,
            crate::asset_orchard::ASSET_ORCHARD_CIRCUIT_ID_V3_REPLAY,
        )
        .expect_err("current swap artifact must not alias replay v3")
        .code(),
        "asset_orchard_swap_vk_artifact_metadata_mismatch"
    );
    assert_eq!(
        decode_swap_vk_artifact_for_circuit(
            ASSET_ORCHARD_SWAP_V3_REPLAY_VK_EMBEDDED_ARTIFACT,
            crate::asset_orchard::ASSET_ORCHARD_CIRCUIT_ID_V4,
        )
        .expect_err("replay swap artifact must not alias current v4")
        .code(),
        "asset_orchard_swap_vk_artifact_metadata_mismatch"
    );
    assert_eq!(
        decode_private_egress_vk_artifact_for_circuit(
            ASSET_ORCHARD_PRIVATE_EGRESS_VK_EMBEDDED_ARTIFACT,
            crate::asset_orchard::ASSET_ORCHARD_PRIVATE_EGRESS_CIRCUIT_ID_V1_REPLAY,
        )
        .expect_err("current private-egress artifact must not alias replay v1")
        .code(),
        "asset_orchard_private_egress_vk_artifact_metadata_mismatch"
    );
    assert_eq!(
        decode_private_egress_vk_artifact_for_circuit(
            ASSET_ORCHARD_PRIVATE_EGRESS_V1_REPLAY_VK_EMBEDDED_ARTIFACT,
            crate::asset_orchard::ASSET_ORCHARD_PRIVATE_EGRESS_CIRCUIT_ID_V2,
        )
        .expect_err("replay private-egress artifact must not alias current v2")
        .code(),
        "asset_orchard_private_egress_vk_artifact_metadata_mismatch"
    );

    let mut swap_tampered = ASSET_ORCHARD_SWAP_V3_REPLAY_VK_EMBEDDED_ARTIFACT.to_vec();
    let last = swap_tampered.len() - 1;
    swap_tampered[last] ^= 1;
    assert_eq!(
        decode_swap_vk_artifact_for_circuit(
            &swap_tampered,
            crate::asset_orchard::ASSET_ORCHARD_CIRCUIT_ID_V3_REPLAY,
        )
        .expect_err("tampered replay swap artifact must fail")
        .code(),
        "asset_orchard_swap_vk_artifact_payload_mismatch"
    );

    let mut egress_tampered = ASSET_ORCHARD_PRIVATE_EGRESS_V1_REPLAY_VK_EMBEDDED_ARTIFACT.to_vec();
    let last = egress_tampered.len() - 1;
    egress_tampered[last] ^= 1;
    assert_eq!(
        decode_private_egress_vk_artifact_for_circuit(
            &egress_tampered,
            crate::asset_orchard::ASSET_ORCHARD_PRIVATE_EGRESS_CIRCUIT_ID_V1_REPLAY,
        )
        .expect_err("tampered replay private-egress artifact must fail")
        .code(),
        "asset_orchard_private_egress_vk_artifact_payload_mismatch"
    );
}

#[cfg(not(feature = "asset-orchard-vk-dev-env"))]
#[test]
fn private_egress_vk_env_overrides_are_ignored_without_dev_feature() {
    let _guard = PRIVATE_EGRESS_VK_ENV_TEST_LOCK
        .lock()
        .expect("env test lock");
    let write_path = std::env::temp_dir().join(format!(
        "postfiat-private-egress-vk-env-ignored-{}-{}.bin",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time")
            .as_nanos()
    ));
    let _ = std::fs::remove_file(&write_path);
    let previous_load = std::env::var("POSTFIAT_ASSET_ORCHARD_PRIVATE_EGRESS_VK_ARTIFACT").ok();
    let previous_write =
        std::env::var("POSTFIAT_ASSET_ORCHARD_PRIVATE_EGRESS_VK_WRITE_ARTIFACT").ok();
    let previous_rebuild = std::env::var("POSTFIAT_ASSET_ORCHARD_PRIVATE_EGRESS_VK_REBUILD").ok();

    std::env::set_var(
        "POSTFIAT_ASSET_ORCHARD_PRIVATE_EGRESS_VK_ARTIFACT",
        "/definitely/missing/private-egress-vk.bin",
    );
    std::env::set_var(
        "POSTFIAT_ASSET_ORCHARD_PRIVATE_EGRESS_VK_WRITE_ARTIFACT",
        &write_path,
    );
    std::env::set_var("POSTFIAT_ASSET_ORCHARD_PRIVATE_EGRESS_VK_REBUILD", "true");

    crate::timing::reset_asset_orchard_private_egress_timings();
    let verifying_key = AssetOrchardPrivateEgressVerifyingKey::build()
        .expect("default build must ignore private egress VK env overrides");
    verifying_key
        .metadata()
        .validate_release_pin()
        .expect("release pin");
    assert_eq!(
        verifying_key.metadata().runtime_pinned_vk_fingerprint,
        ASSET_ORCHARD_PRIVATE_EGRESS_V1_RUNTIME_PINNED_VK_FINGERPRINT
    );
    assert!(
        !write_path.exists(),
        "default build must ignore VK artifact write env var"
    );
    let timings = crate::timing::take_asset_orchard_private_egress_timings();
    assert_eq!(timings.vk_builds.len(), 1);
    assert_eq!(timings.vk_builds[0].artifact_mode, "embedded");
    assert_eq!(timings.vk_builds[0].result, "ok");

    match previous_load {
        Some(value) => {
            std::env::set_var("POSTFIAT_ASSET_ORCHARD_PRIVATE_EGRESS_VK_ARTIFACT", value)
        }
        None => std::env::remove_var("POSTFIAT_ASSET_ORCHARD_PRIVATE_EGRESS_VK_ARTIFACT"),
    }
    match previous_write {
        Some(value) => std::env::set_var(
            "POSTFIAT_ASSET_ORCHARD_PRIVATE_EGRESS_VK_WRITE_ARTIFACT",
            value,
        ),
        None => std::env::remove_var("POSTFIAT_ASSET_ORCHARD_PRIVATE_EGRESS_VK_WRITE_ARTIFACT"),
    }
    match previous_rebuild {
        Some(value) => std::env::set_var("POSTFIAT_ASSET_ORCHARD_PRIVATE_EGRESS_VK_REBUILD", value),
        None => std::env::remove_var("POSTFIAT_ASSET_ORCHARD_PRIVATE_EGRESS_VK_REBUILD"),
    }
    let _ = std::fs::remove_file(write_path);
}

#[test]
fn private_egress_vk_artifact_tamper_fails_closed() {
    let mut artifact = ASSET_ORCHARD_PRIVATE_EGRESS_VK_EMBEDDED_ARTIFACT.to_vec();
    let last = artifact.len() - 1;
    artifact[last] ^= 1;
    let error =
        decode_private_egress_vk_artifact(&artifact).expect_err("tampered artifact must fail");
    assert_eq!(
        error.code(),
        "asset_orchard_private_egress_vk_artifact_payload_mismatch"
    );
}

#[test]
fn embedded_k15_params_artifact_loads_exactly() {
    let params = decode_asset_orchard_k15_params(ASSET_ORCHARD_K15_PARAMS_EMBEDDED_ARTIFACT)
        .expect("embedded K=15 params");
    assert_eq!(params.k(), ASSET_ORCHARD_SWAP_V1_K);
    let mut encoded = Vec::new();
    params
        .write(&mut encoded)
        .expect("serialize decoded params");
    assert_eq!(encoded, ASSET_ORCHARD_K15_PARAMS_EMBEDDED_ARTIFACT);
}

#[test]
fn k15_params_artifact_rejects_wrong_length_k_and_hash() {
    let truncated = &ASSET_ORCHARD_K15_PARAMS_EMBEDDED_ARTIFACT
        [..ASSET_ORCHARD_K15_PARAMS_EMBEDDED_ARTIFACT.len() - 1];
    assert_eq!(
        decode_asset_orchard_k15_params(truncated)
            .expect_err("truncated params must fail")
            .code(),
        "asset_orchard_k15_params_artifact_length_mismatch"
    );

    let mut wrong_k = ASSET_ORCHARD_K15_PARAMS_EMBEDDED_ARTIFACT.to_vec();
    wrong_k[..4].copy_from_slice(&(ASSET_ORCHARD_SWAP_V1_K - 1).to_le_bytes());
    assert_eq!(
        decode_asset_orchard_k15_params(&wrong_k)
            .expect_err("wrong k must fail")
            .code(),
        "asset_orchard_k15_params_artifact_k_mismatch"
    );

    let mut tampered = ASSET_ORCHARD_K15_PARAMS_EMBEDDED_ARTIFACT.to_vec();
    tampered[4] ^= 1;
    assert_eq!(
        decode_asset_orchard_k15_params(&tampered)
            .expect_err("tampered params must fail")
            .code(),
        "asset_orchard_k15_params_artifact_hash_mismatch"
    );
}

#[test]
#[ignore = "full private-egress keygen is release-only benchmark-scale"]
fn private_egress_full_shape_key_metadata_is_pinned_and_consistent() {
    let params = Params::new(ASSET_ORCHARD_PRIVATE_EGRESS_V1_K);
    let full_shape = AssetOrchardPrivateEgressCircuit::full_shape();
    let (vk, pinned_assembly) =
        keygen_vk_pinned_assembly(&params, &full_shape).expect("private egress vk");
    let metadata =
        AssetOrchardPrivateEgressPinnedMetadata::from_vk(&vk, ASSET_ORCHARD_PRIVATE_EGRESS_V1_K)
            .expect("private egress metadata");
    eprintln!("private_egress_pinned_metadata={metadata:#?}");
    metadata.validate_release_pin().expect("release pin");

    let fingerprint = asset_orchard_private_egress_runtime_pinned_vk_fingerprint(&vk);
    let attestation = ASSET_ORCHARD_PRIVATE_EGRESS_V1_VK_ATTESTATION.replace(
        "TODO_PRIVATE_EGRESS_RUNTIME_PINNED_VK_FINGERPRINT",
        &fingerprint,
    );
    let vk_hash = hash_bytes(
        "asset_orchard_private_egress_vk_attestation",
        attestation.as_bytes(),
    );
    assert_eq!(
        fingerprint,
        ASSET_ORCHARD_PRIVATE_EGRESS_V1_RUNTIME_PINNED_VK_FINGERPRINT
    );
    assert_eq!(vk_hash, ASSET_ORCHARD_PRIVATE_EGRESS_V1_VK_HASH);
    assert_eq!(
        metadata.runtime_pinned_vk_fingerprint,
        ASSET_ORCHARD_PRIVATE_EGRESS_V1_RUNTIME_PINNED_VK_FINGERPRINT
    );
    assert_eq!(
        metadata.public_instance_layout_hash,
        ASSET_ORCHARD_PRIVATE_EGRESS_V1_PUBLIC_INSTANCE_LAYOUT_HASH
    );
    assert_eq!(
        metadata.params_hash,
        ASSET_ORCHARD_PRIVATE_EGRESS_V1_PARAMS_HASH
    );
    assert_eq!(metadata.vk_hash, ASSET_ORCHARD_PRIVATE_EGRESS_V1_VK_HASH);

    if let Some(path) = private_egress_vk_artifact_write_path() {
        write_private_egress_vk_artifact(&path, &pinned_assembly, &metadata)
            .expect("write private egress pinned vk artifact");
        eprintln!("wrote_private_egress_vk_artifact={}", path.display());
    }
}

#[test]
#[ignore = "K=15 parameter generation is release-only"]
fn write_asset_orchard_k15_params_release_artifact() {
    let path = std::env::var("POSTFIAT_ASSET_ORCHARD_K15_PARAMS_WRITE_ARTIFACT")
        .expect("set K=15 params artifact output path");
    let params = Params::<vesta::Affine>::new(ASSET_ORCHARD_SWAP_V1_K);
    let mut bytes = Vec::new();
    params.write(&mut bytes).expect("serialize K=15 params");
    assert_eq!(
        bytes, ASSET_ORCHARD_K15_PARAMS_EMBEDDED_ARTIFACT,
        "generated K=15 parameters must match the reviewed embedded artifact"
    );
    std::fs::write(&path, &bytes).expect("write K=15 params artifact");
    eprintln!("wrote_k15_params_artifact={path} bytes={}", bytes.len());
}
