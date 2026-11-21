use super::*;
use crate::asset_orchard::build_asset_orchard_wallet_note;
use halo2_proofs::dev::MockProver;

const CHAIN: &str = "transfer-test-chain";
const GENESIS: [u8; 32] = [7u8; 32];
const PROTOCOL: u32 = 1;
const USDC: &str = "pfusdc-test-asset";
const SENDER_SEED: &str = "1111111111111111111111111111111111111111111111111111111111111111";
const RECIPIENT_SEED: &str = "2222222222222222222222222222222222222222222222222222222222222222";
const OTHER_SEED: &str = "3333333333333333333333333333333333333333333333333333333333333333";

fn shielded(seed: &str, asset: &str, value: u64) -> AssetOrchardWalletNote {
    build_asset_orchard_wallet_note(CHAIN, GENESIS, PROTOCOL, asset, value, seed).expect("note")
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
            to: "pf00000000000000000000000000000000000000aa".to_string(),
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
    // The address of a seed is the address existing wallet notes of that seed already use.
    let note = shielded(RECIPIENT_SEED, USDC, 1);
    assert_eq!(note.note.pk_d.to_affine().expect("pk_d"), address.pk_d);
    assert_eq!(note.note.g_d.to_affine().expect("g_d"), address.g_d);
}

#[test]
fn one_note_pays_with_change_and_both_sides_find_their_notes() {
    let inputs = [shielded(SENDER_SEED, USDC, 1_000_000)];
    let commitments = pool(&[&inputs[0]]);
    let request = request(&inputs, Some(250_000), None, &commitments);
    let draft = draft_asset_orchard_transfer(&request).expect("draft");
    mock(&draft.circuit).expect("a payment with change satisfies the circuit");
    assert_eq!(draft.public_fields.exit_amount, 0);
    assert_eq!(draft.public_fields.exit_asset_tag_lo, 0);
    assert_eq!(draft.change_note.as_ref().expect("change").value, 750_000);

    // Signatures and scanning do not need the proof; an empty one stands in.
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

    // The note the recipient scanned is one they can spend: it drafts a valid second transfer.
    let mut next_pool = commitments.clone();
    for commitment in &built.action.output_commitments {
        next_pool.push(commitment.as_hex().to_string());
    }
    let received = [received];
    let mut onward = request_from(&received, &next_pool);
    onward.change_seed_hex = RECIPIENT_SEED;
    let draft = draft_asset_orchard_transfer(&onward).expect("the recipient can spend");
    mock(&draft.circuit).expect("spending a received note satisfies the circuit");
    assert_eq!(draft.public_fields.exit_amount, 100_000);
    assert_ne!(draft.public_fields.exit_asset_tag_lo, 0);
}

fn request_from<'a>(
    inputs: &'a [AssetOrchardWalletNote],
    commitments: &'a [String],
) -> AssetOrchardTransferRequest<'a> {
    let mut onward = request(inputs, None, Some(100_000), commitments);
    onward.payment = None;
    onward
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
    let pool_domain = honest.public_fields.pool_domain;

    // 1. Print money: an output worth more than the inputs allow.
    let mut circuit = honest.circuit.clone();
    let mut fields = honest.public_fields;
    {
        let output = circuit.output_notes[1].as_mut().expect("change");
        output.note.value += 1;
        output.cmx = output.note.cmx_allowing_zero_value(pool_domain).expect("cmx");
        fields.output_commitments[1] = output.cmx;
    }
    rebind(&mut circuit, &fields);
    rejected_by(&circuit, "transfer value balance", "an inflated output must not verify");

    // 2. Swap assets: an output in a different asset than the inputs.
    let mut circuit = honest.circuit.clone();
    let mut fields = honest.public_fields;
    {
        let output = circuit.output_notes[0].as_mut().expect("payment");
        output.note.asset_tag = AssetTag::derive("another-asset").expect("tag");
        output.cmx = output.note.cmx_allowing_zero_value(pool_domain).expect("cmx");
        fields.output_commitments[0] = output.cmx;
    }
    rebind(&mut circuit, &fields);
    rejected_by(&circuit, "transfer one asset", "an output in another asset must not verify");

    // 3. Spend a note that is not in the tree: give the zero-value filler a value, and shrink
    //    the real input so the amounts still balance.
    let mut circuit = honest.circuit.clone();
    let mut fields = honest.public_fields;
    {
        let filler = circuit.input_notes[1].as_mut().expect("filler");
        filler.note.value = 10;
        filler.cmx = filler.note.cmx_allowing_zero_value(pool_domain).expect("cmx");
        fields.nullifiers[1] = asset_derive_nullifier(
            pool_domain,
            *filler.nk,
            *filler.note.rho,
            *filler.note.psi,
            filler.cmx,
        )
        .expect("nullifier");
        let change = circuit.output_notes[1].as_mut().expect("change");
        change.note.value += 10;
    }
    // The outputs' rho depends on the nullifiers, so rebuild both outputs honestly.
    for index in 0..2 {
        let rho = asset_output_rho(
            pool_domain,
            fields.anchor,
            fields.nullifiers,
            fields.randomized_verification_keys,
            index as u8,
        )
        .expect("rho");
        let output = circuit.output_notes[index].as_mut().expect("output");
        output.note.rho = rho.into();
        output.cmx = output.note.cmx_allowing_zero_value(pool_domain).expect("cmx");
        fields.output_commitments[index] = output.cmx;
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
    fields.exit_asset_tag_lo = other.lo;
    fields.exit_asset_tag_hi = other.hi;
    rebind(&mut circuit, &fields);
    rejected_by(&circuit, "transfer exit asset", "an exit under another asset's name must not verify");

    // 5. Take more out in public than the notes hold.
    let mut circuit = exiting.circuit.clone();
    let mut fields = exiting.public_fields;
    fields.exit_amount += 1;
    rebind(&mut circuit, &fields);
    rejected_by(&circuit, "transfer value balance", "a larger public exit must not verify");

    // 6. Reuse someone else's action hash.
    let mut circuit = honest.circuit.clone();
    let mut instance = circuit.public_instance.expect("instance");
    instance[21] += pallas::Base::ONE;
    circuit.public_instance = Some(instance);
    rejected_by(&circuit, "Permutation", "a forged action hash must not verify");
}

#[test]
#[ignore = "builds the transfer proving key; release-scale"]
fn a_real_proof_verifies_and_a_tampered_action_does_not() {
    let started = std::time::Instant::now();
    let key = AssetOrchardTransferVerifyingKey::build_unpinned().expect("verifying key");
    println!("transfer vk fingerprint {} ({:.1} s)", key.fingerprint(), started.elapsed().as_secs_f64());
    assert_eq!(key.fingerprint(), ASSET_ORCHARD_TRANSFER_V1_RUNTIME_PINNED_VK_FINGERPRINT);

    let inputs = [shielded(SENDER_SEED, USDC, 1_000_000)];
    let commitments = pool(&[&inputs[0]]);
    let started = std::time::Instant::now();
    let first = build_asset_orchard_transfer_action(&request(&inputs, Some(250_000), None, &commitments))
        .expect("build");
    println!("first transfer, key build included: {:.1} s", started.elapsed().as_secs_f64());
    println!("proof bytes {}", first.action.proof.byte_len());
    verify_asset_orchard_transfer_action(CHAIN, GENESIS, PROTOCOL, &first.action).expect("verify");

    let started = std::time::Instant::now();
    let second =
        build_asset_orchard_transfer_action(&request(&inputs, None, Some(400_000), &commitments))
            .expect("build");
    println!("second transfer: {:.1} s", started.elapsed().as_secs_f64());
    let started = std::time::Instant::now();
    verify_asset_orchard_transfer_action(CHAIN, GENESIS, PROTOCOL, &second.action).expect("verify");
    println!("verify: {:.3} s", started.elapsed().as_secs_f64());

    // Redirecting the public exit breaks the binding the proof and the signatures cover.
    let mut redirected = second.action.clone();
    redirected.exit.as_mut().expect("exit").to = "pf00000000000000000000000000000000000000bb".to_string();
    assert!(verify_asset_orchard_transfer_action(CHAIN, GENESIS, PROTOCOL, &redirected).is_err());
    // Swapping in another action's output is caught too.
    let mut spliced = first.action.clone();
    spliced.output_commitments[0] = second.action.output_commitments[0].clone();
    assert!(verify_asset_orchard_transfer_action(CHAIN, GENESIS, PROTOCOL, &spliced).is_err());
    // A wrong chain is refused.
    assert!(verify_asset_orchard_transfer_action("other-chain", GENESIS, PROTOCOL, &first.action).is_err());
}
