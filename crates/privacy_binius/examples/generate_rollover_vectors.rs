//! Rebuild the disposable L1 epoch fixtures with the standalone depth-24 tree.
use pft_binius_shielded::{
    payment::{create_profiled_tx, ShieldedWallet, SpendInput},
    shake256::MerkleTree24,
};
use postfiat_crypto_provider::*;
use postfiat_types::pq_payment::*;
use rand_chacha::{rand_core::SeedableRng, ChaCha20Rng};
use std::{fs, path::PathBuf};

fn main() {
    let dir = PathBuf::from(std::env::args_os().nth(1).expect("output directory"));
    fs::create_dir_all(&dir).unwrap();
    let chain = Chain { id:"postfiat-pfsh02-m1-disposable",genesis:hex_to_bytes("e9c735eecfc4550f152330b52f782de84eea2aa7d59dc7e9e317718ed2ba28473854f0cb7c6c972f7f2825ba944b72e8").unwrap().try_into().unwrap(),protocol:1 };
    let key = ml_dsa_65_keygen_from_seed(&[7; 32]);
    let dest = destination(&address_from_public_key(&key.public_key)).unwrap();
    let mut rng = ChaCha20Rng::from_seed([81; 32]);
    let wallet = ShieldedWallet::from_seed(&mut rng);
    let mut make = |name: &str,
                    tree: &MerkleTree24,
                    epoch: u64,
                    a: SpendInput,
                    b: SpendInput,
                    mint: u64,
                    value: u64| {
        let terms = Terms {
            mode: if mint > 0 {
                Mode::Shield
            } else {
                Mode::Internal
            },
            asset: [0; 48],
            epoch,
            public_fee: 0,
            account: (mint > 0).then(|| Account {
                id: dest[12..].try_into().unwrap(),
                sequence: epoch + 1,
                public_key: Box::new(key.public_key.as_slice().try_into().unwrap()),
            }),
        };
        let (tx, blind, _) = create_profiled_tx(
            tree,
            a,
            b,
            &wallet.stealth_address(),
            value,
            &wallet.stealth_address(),
            0,
            mint,
            0,
            [0; 32],
            1,
            None,
            Some(&|raw| terms.statement_context(chain, raw)),
            &mut rng,
        )
        .unwrap();
        let sig = terms
            .signing_message(chain, tx.statement().tx_ctx)
            .map(|msg| {
                ml_dsa_65_sign_with_context_seed(
                    &key.private_key,
                    &msg,
                    TX_SIGNATURE_CONTEXT,
                    &[21; 32],
                )
                .unwrap()
                .try_into()
                .unwrap()
            });
        let wire = PqPayment::encode(&terms, sig.as_ref(), &tx.to_bytes()).unwrap();
        postfiat_privacy_binius::verify_l1(&wire, chain).unwrap();
        fs::write(dir.join(format!("{name}.bin")), wire.bytes()).unwrap();
        ([tx.coin_out0, tx.coin_out1], blind)
    };
    let real = |index, value, blind| SpendInput::Real {
        leaf_index: index,
        spend_secret: wallet.spend_secret,
        value,
        r_blind: blind,
    };
    let cap = PQ_CAPACITY as usize;
    let mut notes = Vec::new();
    for epoch in 0..2 {
        let leaves = (0..cap - 2)
            .map(|i| {
                let mut coin = [0; 48];
                coin[..8].copy_from_slice(&(i as u64 + 1 + epoch * cap as u64).to_le_bytes());
                coin
            })
            .collect();
        let mut tree = MerkleTree24::from_owned_leaves(leaves);
        let opening = bytes_to_hex(&tree.root());
        let (outputs, blind) = make(
            &format!("boundary-{epoch}"),
            &tree,
            epoch,
            SpendInput::Dummy,
            SpendInput::Dummy,
            10,
            9,
        );
        for c in outputs {
            tree.append(c);
        }
        let (_, siblings) = tree.auth_path(cap - 2);
        let frontier: Vec<_> = siblings
            .iter()
            .enumerate()
            .map(|(level, n)| (level != 0).then(|| bytes_to_hex(n)))
            .collect();
        fs::write(dir.join(format!("frontier-{epoch}.json")),serde_json::to_vec_pretty(&serde_json::json!({
            "count":cap-2,"frontier":frontier,"anchors":[opening],"terminal":bytes_to_hex(&tree.root())})).unwrap()).unwrap();
        notes.push((outputs[0], blind));
        if epoch == 0 {
            let (moved, blind) = make(
                "old-spend",
                &tree,
                0,
                real(cap - 2, 9, blind),
                SpendInput::Dummy,
                0,
                8,
            );
            let new = MerkleTree24::from_owned_leaves(moved.to_vec());
            make(
                "successor-spend",
                &new,
                2,
                real(0, 8, blind),
                SpendInput::Dummy,
                0,
                7,
            );
        }
    }
    // Both openings are genuine but this combined tree never existed on chain.
    let mixed = MerkleTree24::from_owned_leaves(notes.iter().map(|n| n.0).collect());
    make(
        "mixed-inputs",
        &mixed,
        0,
        real(0, 9, notes[0].1),
        real(1, 9, notes[1].1),
        0,
        17,
    );
    println!("Rebuilt two depth-24 frontiers and five verified L1 proofs");
}
