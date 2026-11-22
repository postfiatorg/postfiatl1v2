use super::*;
use crate::asset_orchard::{build_asset_orchard_wallet_note, hash_to_pallas_base};
use halo2_proofs::dev::MockProver;

const CHAIN: &str = "transfer-test-chain";
const GENESIS: [u8; 32] = [7u8; 32];
const PROTOCOL: u32 = 1;
const USDC: &str = "pfusdc-test-asset";
const SENDER_SEED: &str = "1111111111111111111111111111111111111111111111111111111111111111";
const RECIPIENT_SEED: &str = "2222222222222222222222222222222222222222222222222222222222222222";
const OTHER_SEED: &str = "3333333333333333333333333333333333333333333333333333333333333333";
const PUBLIC_ACCOUNT: &str = "pf00000000000000000000000000000000000000aa";

fn pool_domain() -> pallas::Base {
    AssetOrchardTransferAction::expected_pool_domain(CHAIN, GENESIS, PROTOCOL).expect("domain")
}

/// A transfer-domain note owned by `seed`, as a wallet would hold it after receiving it.
fn shielded(seed: &str, asset: &str, value: u64) -> AssetOrchardWalletNote {
    let keys = AssetOrchardShieldedKeys::from_seed(seed).expect("keys");
    let mut label = format!("{seed}:{asset}:{value}").into_bytes();
    let rseed: [u8; 32] = {
        use sha3::{Digest, Sha3_256};
        Sha3_256::digest(&label).into()
    };
    label.extend_from_slice(b":rho");
    let rho = hash_to_pallas_base("postfiat.asset_orchard.transfer.test_rho", &label).expect("rho");
    let note = asset_orchard_note_for_address(
        &keys.address,
        AssetTag::derive(asset).expect("tag"),
        value,
        rho,
        &rseed,
    )
    .expect("note");
    keys.wallet_note(pool_domain(), asset, &note, &rseed).expect("wallet note")
}

/// A pool holding some unrelated notes around the given ones.
fn pool(notes: &[&AssetOrchardWalletNote]) -> Vec<String> {
    let mut commitments = vec![
        shielded(OTHER_SEED, USDC, 5).output_commitment.as_hex().to_string(),
        shielded(OTHER_SEED, "another-asset", 9)
            .output_commitment
            .as_hex()
            .to_string(),
    ];
    for note in notes {
        commitments.push(note.output_commitment.as_hex().to_string());
    }
    commitments.push(shielded(OTHER_SEED, USDC, 6).output_commitment.as_hex().to_string());
    commitments
}

fn entry(amount: u64) -> AssetOrchardTransferEntry {
    AssetOrchardTransferEntry {
        from: PUBLIC_ACCOUNT.to_string(),
        asset_id: USDC.to_string(),
        amount,
        burn_transaction_id: "ab".repeat(48),
    }
}

fn request<'a>(
    inputs: &'a [AssetOrchardWalletNote],
    payment: Option<u64>,
    exit: Option<u64>,
    commitments: &'a [String],
) -> AssetOrchardTransferRequest<'a> {
    AssetOrchardTransferRequest {
        chain_id: CHAIN,
        genesis_hash: GENESIS,
        protocol_version: PROTOCOL,
        input_notes: inputs,
        entry: None,
        change_seed_hex: SENDER_SEED,
        payment: payment.map(|amount| {
            (
                AssetOrchardShieldedKeys::from_seed(RECIPIENT_SEED)
                    .expect("keys")
                    .address,
                amount,
            )
        }),
        exit: exit.map(|amount| AssetOrchardTransferExit {
            to: PUBLIC_ACCOUNT.to_string(),
            asset_id: USDC.to_string(),
            amount,
        }),
        pool_output_commitments: commitments,
    }
}

fn mock(circuit: &AssetOrchardTransferCircuit) -> Result<(), String> {
    let instance = circuit.public_instance.expect("instance").to_vec();
    MockProver::run(ASSET_ORCHARD_TRANSFER_V1_K, circuit, vec![instance])
        .map_err(|error| format!("{error:?}"))?
        .verify()
        .map_err(|failures| format!("{failures:?}"))
}

/// The circuit must fail, and the named constraint must be among the reasons, so a forgery
/// that happens to trip something unrelated does not pass for a tested rule.
fn rejected_by(circuit: &AssetOrchardTransferCircuit, constraint: &str, what: &str) {
    let failures = mock(circuit).expect_err(what);
    assert!(
        failures.contains(constraint),
        "{what}: expected `{constraint}` among the failures, got {}",
        &failures[..failures.len().min(600)]
    );
}

fn rebind(circuit: &mut AssetOrchardTransferCircuit, fields: &AssetOrchardTransferPublicFields) {
    circuit.public_instance = Some(fields.public_instance().expect("instance"));
}

fn recommit(note: &mut AssetOrchardSwapNoteWitness) -> pallas::Base {
    note.cmx = note.note.cmx_allowing_zero_value(pool_domain()).expect("cmx");
    note.cmx
}

#[test]
fn shielded_address_round_trips_and_rejects_damage() {
    let address = AssetOrchardShieldedKeys::from_seed(RECIPIENT_SEED)
        .expect("keys")
        .address;
    let text = address.encode().expect("encode");
    assert!(text.starts_with("pfs1"));
    assert_eq!(AssetOrchardShieldedAddress::parse(&text).expect("parse"), address);
    let mut damaged = text.clone().into_bytes();
    let last = damaged.len() - 1;
    damaged[last] = if damaged[last] == b'0' { b'1' } else { b'0' };
    assert!(AssetOrchardShieldedAddress::parse(&String::from_utf8(damaged).expect("utf8")).is_err());
    assert!(AssetOrchardShieldedAddress::parse(&text.to_uppercase()).is_err());
}

#[test]
fn transfer_notes_live_in_their_own_domain() {
    // A note made for the older actions has the same keys but a different commitment, so it
    // can never be opened as a transfer note.
    let legacy = build_asset_orchard_wallet_note(CHAIN, GENESIS, PROTOCOL, USDC, 7, SENDER_SEED)
        .expect("legacy note");
    let legacy_domain =
        AssetOrchardSwapAction::expected_pool_domain(CHAIN, GENESIS, PROTOCOL).expect("domain");
    assert_ne!(legacy_domain, pool_domain());
    let opening = legacy.note.to_note_opening().expect("opening");
    assert_ne!(
        opening.cmx(legacy_domain).expect("legacy cmx"),
        opening.cmx(pool_domain()).expect("transfer cmx")
    );
    let commitments = pool(&[]);
    let mut with_legacy = commitments.clone();
    with_legacy.push(legacy.output_commitment.as_hex().to_string());
    let legacy_inputs = [legacy];
    assert!(
        draft_asset_orchard_transfer(&request(&legacy_inputs, Some(1), None, &with_legacy)).is_err(),
        "a note from the older actions was accepted as a transfer input"
    );
}

#[test]
fn shielding_proves_the_note_holds_the_public_amount() {
    let commitments = pool(&[]);
    let mut shield = request(&[], None, None, &commitments);
    shield.entry = Some(entry(1_000_000));
    let draft = draft_asset_orchard_transfer(&shield).expect("draft");
    mock(&draft.circuit).expect("an honest shield satisfies the circuit");
    assert_eq!(draft.public_fields.entry_amount, 1_000_000);
    assert_eq!(draft.change_note.as_ref().expect("shielded note").value, 1_000_000);

    // Seal a note worth more than was burned.
    let mut circuit = draft.circuit.clone();
    let mut fields = draft.public_fields;
    {
        let output = circuit.output_notes[1].as_mut().expect("note");
        output.note.value += 1;
        fields.output_commitments[1] = recommit(output);
    }
    rebind(&mut circuit, &fields);
    rejected_by(&circuit, "transfer value balance", "a note worth more than the burn must not verify");

    // Burn one asset, seal a note of another.
    let mut circuit = draft.circuit.clone();
    let mut fields = draft.public_fields;
    let other = AssetTag::derive("another-asset").expect("tag");
    for index in 0..2 {
        circuit.input_notes[index].as_mut().expect("filler").note.asset_tag = other;
        recommit(circuit.input_notes[index].as_mut().expect("filler"));
        let output = circuit.output_notes[index].as_mut().expect("note");
        output.note.asset_tag = other;
        fields.output_commitments[index] = recommit(output);
    }
    for index in 0..2 {
        let filler = circuit.input_notes[index].as_ref().expect("filler");
        fields.nullifiers[index] = asset_derive_nullifier(
            pool_domain(),
            *filler.nk,
            *filler.note.rho,
            *filler.note.psi,
            filler.cmx,
        )
        .expect("nullifier");
    }
    for index in 0..2 {
        let rho = asset_output_rho(
            pool_domain(),
            fields.anchor,
            fields.nullifiers,
            fields.randomized_verification_keys,
            index as u8,
        )
        .expect("rho");
        let output = circuit.output_notes[index].as_mut().expect("note");
        output.note.rho = rho.into();
        fields.output_commitments[index] = recommit(output);
    }
    rebind(&mut circuit, &fields);
    rejected_by(&circuit, "transfer public asset", "a note of another asset than the burn must not verify");
}

#[test]
fn one_note_pays_with_change_and_both_sides_find_their_notes() {
    let inputs = [shielded(SENDER_SEED, USDC, 1_000_000)];
    let commitments = pool(&[&inputs[0]]);
    let request = request(&inputs, Some(250_000), None, &commitments);
    let draft = draft_asset_orchard_transfer(&request).expect("draft");
    mock(&draft.circuit).expect("a payment with change satisfies the circuit");
    assert_eq!(draft.public_fields.exit_amount, 0);
    assert_eq!(draft.public_fields.entry_amount, 0);
    assert_eq!(draft.public_fields.public_asset_tag_lo, 0);
    assert_eq!(draft.change_note.as_ref().expect("change").value, 750_000);

    // Signatures and scanning do not need the proof; a placeholder stands in.
    let built = finish_asset_orchard_transfer(&request, draft, vec![1u8; 32]).expect("finish");
    built
        .action
        .verify_spend_authorizations(CHAIN, GENESIS, PROTOCOL)
        .expect("both inputs are authorized");
    for output in &built.action.encrypted_outputs {
        assert_eq!(output.byte_len(), crate::ASSET_ORCHARD_TRANSFER_OUTPUT_CIPHERTEXT_BYTES);
    }
    let recipient = AssetOrchardShieldedKeys::from_seed(RECIPIENT_SEED).expect("keys");
    let sender = AssetOrchardShieldedKeys::from_seed(SENDER_SEED).expect("keys");
    let other = AssetOrchardShieldedKeys::from_seed(OTHER_SEED).expect("keys");
    let scan = |keys: &AssetOrchardShieldedKeys, index: usize| {
        crate::decrypt_asset_orchard_transfer_output(
            CHAIN,
            GENESIS,
            PROTOCOL,
            keys,
            built.action.output_commitments[index].as_hex(),
            &built.action.encrypted_outputs[index].to_bytes().expect("bytes"),
        )
        .expect("scan")
    };
    let received = scan(&recipient, 0).expect("the recipient finds the payment");
    assert_eq!((received.value, received.asset_id.as_str()), (250_000, USDC));
    assert_eq!(received.output_commitment, built.action.output_commitments[0]);
    assert_eq!(scan(&sender, 1), built.change_note);
    assert!(scan(&recipient, 1).is_none() && scan(&sender, 0).is_none());
    assert!(scan(&other, 0).is_none() && scan(&other, 1).is_none());

    // The note the recipient scanned is one they can spend: it drafts a valid exit.
    let mut next_pool = commitments.clone();
    for commitment in &built.action.output_commitments {
        next_pool.push(commitment.as_hex().to_string());
    }
    let received = [received];
    let mut onward = self::request(&received, None, Some(100_000), &next_pool);
    onward.change_seed_hex = RECIPIENT_SEED;
    let draft = draft_asset_orchard_transfer(&onward).expect("the recipient can spend");
    mock(&draft.circuit).expect("spending a received note satisfies the circuit");
    assert_eq!(draft.public_fields.exit_amount, 100_000);
    assert_ne!(draft.public_fields.public_asset_tag_lo, 0);
}

#[test]
fn two_notes_merge_into_a_payment_an_exit_and_change() {
    let inputs = [
        shielded(SENDER_SEED, USDC, 600_000),
        shielded(OTHER_SEED, USDC, 400_000),
    ];
    let commitments = pool(&[&inputs[0], &inputs[1]]);
    let draft = draft_asset_orchard_transfer(&request(
        &inputs,
        Some(700_000),
        Some(200_000),
        &commitments,
    ))
    .expect("draft");
    mock(&draft.circuit).expect("merge, pay, exit and change satisfy the circuit");
    assert_eq!(draft.change_note.expect("change").value, 100_000);
}

#[test]
fn the_builder_refuses_what_the_circuit_would_reject() {
    let inputs = [shielded(SENDER_SEED, USDC, 1_000)];
    let commitments = pool(&[&inputs[0]]);
    let code = |payment, exit| {
        draft_asset_orchard_transfer(&request(&inputs, payment, exit, &commitments))
            .err()
            .map(|error| error.code().to_string())
    };
    assert_eq!(code(Some(1_001), None).as_deref(), Some("asset_orchard_transfer_insufficient_notes"));
    assert_eq!(code(Some(600), Some(600)).as_deref(), Some("asset_orchard_transfer_insufficient_notes"));
    assert_eq!(code(None, None).as_deref(), Some("asset_orchard_transfer_nothing_to_send"));
    assert_eq!(code(Some(0), None).as_deref(), Some("zero_asset_orchard_transfer_payment"));
    let mut both = request(&inputs, None, Some(5), &commitments);
    both.entry = Some(entry(5));
    assert_eq!(
        draft_asset_orchard_transfer(&both).err().map(|error| error.code().to_string()).as_deref(),
        Some("asset_orchard_transfer_entry_and_exit")
    );
    let mixed = [
        shielded(SENDER_SEED, USDC, 1_000),
        shielded(SENDER_SEED, "another-asset", 1_000),
    ];
    let mixed_pool = pool(&[&mixed[0], &mixed[1]]);
    assert_eq!(
        draft_asset_orchard_transfer(&request(&mixed, Some(5), None, &mixed_pool))
            .err()
            .map(|error| error.code().to_string())
            .as_deref(),
        Some("asset_orchard_transfer_mixed_assets")
    );
}

/// Each case forges one thing a dishonest prover would want and re-derives the public instance
/// around it, so the only thing standing in the way is the constraint under test.
#[test]
fn the_circuit_rejects_forged_statements() {
    let inputs = [shielded(SENDER_SEED, USDC, 1_000_000)];
    let commitments = pool(&[&inputs[0]]);
    let honest = draft_asset_orchard_transfer(&request(&inputs, Some(250_000), None, &commitments))
        .expect("draft");

    // 1. Print money: an output worth more than the inputs allow.
    let mut circuit = honest.circuit.clone();
    let mut fields = honest.public_fields;
    {
        let output = circuit.output_notes[1].as_mut().expect("change");
        output.note.value += 1;
        fields.output_commitments[1] = recommit(output);
    }
    rebind(&mut circuit, &fields);
    rejected_by(&circuit, "transfer value balance", "an inflated output must not verify");

    // 2. Swap assets: an output in a different asset than the inputs.
    let mut circuit = honest.circuit.clone();
    let mut fields = honest.public_fields;
    {
        let output = circuit.output_notes[0].as_mut().expect("payment");
        output.note.asset_tag = AssetTag::derive("another-asset").expect("tag");
        fields.output_commitments[0] = recommit(output);
    }
    rebind(&mut circuit, &fields);
    rejected_by(&circuit, "transfer one asset", "an output in another asset must not verify");

    // 3. Spend a note that is not in the tree: give the zero-value filler a value.
    let mut circuit = honest.circuit.clone();
    let mut fields = honest.public_fields;
    {
        let filler = circuit.input_notes[1].as_mut().expect("filler");
        filler.note.value = 10;
        recommit(filler);
        fields.nullifiers[1] = asset_derive_nullifier(
            pool_domain(),
            *filler.nk,
            *filler.note.rho,
            *filler.note.psi,
            filler.cmx,
        )
        .expect("nullifier");
        circuit.output_notes[1].as_mut().expect("change").note.value += 10;
    }
    for index in 0..2 {
        let rho = asset_output_rho(
            pool_domain(),
            fields.anchor,
            fields.nullifiers,
            fields.randomized_verification_keys,
            index as u8,
        )
        .expect("rho");
        let output = circuit.output_notes[index].as_mut().expect("output");
        output.note.rho = rho.into();
        fields.output_commitments[index] = recommit(output);
    }
    rebind(&mut circuit, &fields);
    rejected_by(&circuit, "transfer conditional anchor", "a valued note outside the tree must not verify");

    // 4. Exit one asset while claiming another.
    let exiting = draft_asset_orchard_transfer(&request(&inputs, None, Some(300_000), &commitments))
        .expect("draft");
    mock(&exiting.circuit).expect("an honest exit satisfies the circuit");
    let mut circuit = exiting.circuit.clone();
    let mut fields = exiting.public_fields;
    let other = AssetTag::derive("another-asset").expect("tag");
    fields.public_asset_tag_lo = other.lo;
    fields.public_asset_tag_hi = other.hi;
    rebind(&mut circuit, &fields);
    rejected_by(&circuit, "transfer public asset", "an exit under another asset's name must not verify");

    // 5. Take more out in public than the notes hold.
    let mut circuit = exiting.circuit.clone();
    let mut fields = exiting.public_fields;
    fields.exit_amount += 1;
    rebind(&mut circuit, &fields);
    rejected_by(&circuit, "transfer value balance", "a larger public exit must not verify");

    // 6. Reuse someone else's action hash.
    let mut circuit = honest.circuit.clone();
    let mut instance = circuit.public_instance.expect("instance");
    instance[ROW_ACTION_HASH] += pallas::Base::ONE;
    circuit.public_instance = Some(instance);
    rejected_by(&circuit, "Permutation", "a forged action hash must not verify");

    // 7. Move nothing at all: two fillers in, two empty notes out.
    let mut circuit = honest.circuit.clone();
    let mut fields = honest.public_fields;
    circuit.input_notes[0] = circuit.input_notes[1].clone();
    {
        let first = circuit.input_notes[0].as_mut().expect("filler");
        first.note.rho = (*first.note.rho + pallas::Base::ONE).into();
        recommit(first);
        fields.nullifiers[0] = asset_derive_nullifier(
            pool_domain(),
            *first.nk,
            *first.note.rho,
            *first.note.psi,
            first.cmx,
        )
        .expect("nullifier");
        fields.randomized_verification_keys[0] = fields.randomized_verification_keys[1];
    }
    for index in 0..2 {
        let rho = asset_output_rho(
            pool_domain(),
            fields.anchor,
            fields.nullifiers,
            fields.randomized_verification_keys,
            index as u8,
        )
        .expect("rho");
        let output = circuit.output_notes[index].as_mut().expect("output");
        output.note.value = 0;
        output.note.rho = rho.into();
        fields.output_commitments[index] = recommit(output);
    }
    rebind(&mut circuit, &fields);
    rejected_by(&circuit, "transfer value balance", "a transfer that moves nothing must not verify");
}

/// The decomposition every note field goes through. A prover who assigns the bits of one
/// number and claims they sum to another must be refused by the chained gate.
#[derive(Clone)]
struct ForgedBitSum;

impl Circuit<pallas::Base> for ForgedBitSum {
    type Config = AssetOrchardTransferConfig;
    type FloorPlanner = SimpleFloorPlanner;
    fn without_witnesses(&self) -> Self {
        self.clone()
    }
    fn configure(meta: &mut ConstraintSystem<pallas::Base>) -> Self::Config {
        AssetOrchardTransferCircuit::configure(meta)
    }
    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<pallas::Base>,
    ) -> Result<(), Error> {
        let base = &config.base;
        AssetOrchardSinsemillaChip::load(base.sinsemilla.clone(), &mut layouter)?;
        let claimed = pallas::Base::from(1000u64);
        let bits_of = 5u64;
        let target = layouter.assign_region(
            || "target",
            |mut region| region.assign_advice(|| "target", base.advice[4], 0, || Value::known(claimed)),
        )?;
        layouter.assign_region(
            || "forged chain",
            |mut region| {
                let bit_len = 64usize;
                let mut sum = region.assign_advice_from_constant(
                    || "start",
                    base.advice[1],
                    0,
                    pallas::Base::ZERO,
                )?;
                let mut running = pallas::Base::ZERO;
                for index in 0..bit_len {
                    config.q_chain.enable(&mut region, index)?;
                    let bit = pallas::Base::from((bits_of >> index) & 1);
                    let weight = two_pow(index);
                    region.assign_fixed(|| "weight", base.fixed[1], index, || Value::known(weight))?;
                    region.assign_advice(|| "bit", base.advice[0], index, || Value::known(bit))?;
                    running += bit * weight;
                    // The only place left to lie is the last cell of the single sum column.
                    let shown = if index == bit_len - 1 { claimed } else { running };
                    sum = region.assign_advice(|| "sum", base.advice[1], index + 1, || Value::known(shown))?;
                }
                let copy = target.copy_advice(|| "target copy", &mut region, base.advice[2], bit_len)?;
                region.constrain_equal(sum.cell(), copy.cell())
            },
        )
    }
}

#[test]
fn the_chained_decomposition_binds_a_cell_to_its_bits() {
    let failures = MockProver::run(ASSET_ORCHARD_TRANSFER_V1_K, &ForgedBitSum, vec![vec![]])
        .expect("mock prover")
        .verify()
        .expect_err("the bits of 5 were accepted as a decomposition of 1000");
    let failures = format!("{failures:?}");
    assert!(
        failures.contains("transfer chained bit sum"),
        "refused for another reason: {}",
        &failures[..failures.len().min(400)]
    );
}

/// Rebuilds the verifying key from the circuit and writes the artifact the binary ships. Run
/// it after any change to the circuit, then update the pinned fingerprint it prints.
#[test]
#[ignore = "builds the transfer verifying key from the circuit; release-scale"]
fn write_the_transfer_verifying_key_artifact() {
    let (key, artifact) = AssetOrchardTransferVerifyingKey::build_from_circuit().expect("key");
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("artifacts/asset_orchard_transfer_vk_pinned_assembly.v1.bin");
    std::fs::write(&path, &artifact).expect("write artifact");
    println!("transfer vk fingerprint {}", key.fingerprint());
    println!("wrote {} bytes to {}", artifact.len(), path.display());
}

#[test]
fn the_shipped_verifying_key_loads_and_matches_the_pin() {
    let started = std::time::Instant::now();
    let key = AssetOrchardTransferVerifyingKey::build().expect("shipped verifying key");
    assert_eq!(key.fingerprint(), ASSET_ORCHARD_TRANSFER_V1_RUNTIME_PINNED_VK_FINGERPRINT);
    println!("shipped transfer verifying key loaded in {:.3} s", started.elapsed().as_secs_f64());
}

#[test]
#[ignore = "builds the transfer proving key; release-scale"]
fn a_real_proof_verifies_and_a_tampered_action_does_not() {
    // The key built from the circuit is the key the binary ships.
    let started = std::time::Instant::now();
    let key = AssetOrchardTransferVerifyingKey::build_unpinned().expect("verifying key");
    println!("transfer vk fingerprint {} ({:.1} s)", key.fingerprint(), started.elapsed().as_secs_f64());
    assert_eq!(key.fingerprint(), ASSET_ORCHARD_TRANSFER_V1_RUNTIME_PINNED_VK_FINGERPRINT);

    let commitments = pool(&[]);
    let mut shield = request(&[], None, None, &commitments);
    shield.entry = Some(entry(1_000_000));
    let started = std::time::Instant::now();
    let shielded_in = build_asset_orchard_transfer_action(&shield).expect("build shield");
    println!("shield, key build included: {:.1} s", started.elapsed().as_secs_f64());
    println!("proof bytes {}", shielded_in.action.proof.byte_len());
    verify_asset_orchard_transfer_action(CHAIN, GENESIS, PROTOCOL, &shielded_in.action).expect("verify shield");

    let mut after = commitments.clone();
    for commitment in &shielded_in.action.output_commitments {
        after.push(commitment.as_hex().to_string());
    }
    let inputs = [shielded_in.change_note.clone().expect("shielded note")];
    let started = std::time::Instant::now();
    let payment = build_asset_orchard_transfer_action(&request(&inputs, Some(250_000), None, &after))
        .expect("build payment");
    println!("payment: {:.1} s", started.elapsed().as_secs_f64());
    verify_asset_orchard_transfer_action(CHAIN, GENESIS, PROTOCOL, &payment.action).expect("verify payment");
    let started = std::time::Instant::now();
    let exit = build_asset_orchard_transfer_action(&request(&inputs, None, Some(400_000), &after))
        .expect("build exit");
    println!("exit: {:.1} s", started.elapsed().as_secs_f64());
    let started = std::time::Instant::now();
    verify_asset_orchard_transfer_action(CHAIN, GENESIS, PROTOCOL, &exit.action).expect("verify exit");
    println!("verify: {:.3} s", started.elapsed().as_secs_f64());

    // Redirecting the public exit breaks the binding the proof and the signatures cover.
    let mut redirected = exit.action.clone();
    redirected.exit.as_mut().expect("exit").to = "pf00000000000000000000000000000000000000bb".to_string();
    assert!(verify_asset_orchard_transfer_action(CHAIN, GENESIS, PROTOCOL, &redirected).is_err());
    // Claiming a larger burn than the one the proof covers is refused.
    let mut inflated = shielded_in.action.clone();
    inflated.entry.as_mut().expect("entry").amount += 1;
    assert!(verify_asset_orchard_transfer_action(CHAIN, GENESIS, PROTOCOL, &inflated).is_err());
    // Swapping in another action's output is caught too.
    let mut spliced = payment.action.clone();
    spliced.output_commitments[0] = exit.action.output_commitments[0].clone();
    assert!(verify_asset_orchard_transfer_action(CHAIN, GENESIS, PROTOCOL, &spliced).is_err());
    // A wrong chain is refused.
    assert!(verify_asset_orchard_transfer_action("other-chain", GENESIS, PROTOCOL, &payment.action).is_err());
}
