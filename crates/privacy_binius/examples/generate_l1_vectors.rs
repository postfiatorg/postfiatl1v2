//! Generates only disposable test vectors. The seeds are public.
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
    let chain = Chain{id:"postfiat-pfsh02-m1-disposable",genesis:hex_to_bytes("e9c735eecfc4550f152330b52f782de84eea2aa7d59dc7e9e317718ed2ba28473854f0cb7c6c972f7f2825ba944b72e8").unwrap().try_into().unwrap(),protocol:1};
    let series = postfiat_types::pfusdc_source_series_id(
        chain.id,
        &"11".repeat(48),
        1,
        &format!("0x{}", "22".repeat(20)),
        &format!("0x{}", "33".repeat(20)),
        6,
        &"44".repeat(48),
    )
    .unwrap();
    let key = ml_dsa_65_keygen_from_seed(&[7; 32]);
    let dest = destination(&address_from_public_key(&key.public_key)).unwrap();
    let mut rng = ChaCha20Rng::from_seed([41; 32]);
    let wallet = ShieldedWallet::from_seed(&mut rng);
    for (name, asset) in [
        ("pft", [0; 48]),
        ("pfusdc", hex_to_bytes(&series).unwrap().try_into().unwrap()),
    ] {
        let issued = asset != [0; 48];
        let fee = if issued { 0 } else { 1 };
        let mut tree = MerkleTree24::from_owned_leaves(vec![]);
        let mut input = SpendInput::Dummy;
        let mut value = 0;
        for (step, mode, mint, exit) in [
            ("shield", Mode::Shield, 100, 0),
            ("pay", Mode::Internal, 0, 0),
            ("topup", Mode::Shield, 5, 0),
            ("exit", Mode::Exit, 0, 20),
        ] {
            let terms = Terms {
                mode,
                asset,
                epoch: 0,
                public_fee: if issued { 3 } else { 0 },
                account: (issued || mint > 0).then(|| Account {
                    id: dest[12..].try_into().unwrap(),
                    sequence: tree.len() as u64 + 1,
                    public_key: key.public_key.as_slice().try_into().map(Box::new).unwrap(),
                }),
            };
            value = value + mint - exit - fee;
            let (tx, blind, _) = create_profiled_tx(
                &tree,
                input,
                SpendInput::Dummy,
                &wallet.stealth_address(),
                value,
                &wallet.stealth_address(),
                0,
                mint,
                exit,
                if exit > 0 { dest } else { [0; 32] },
                fee,
                None,
                Some(&|raw| terms.statement_context(chain, raw)),
                &mut rng,
            )
            .unwrap();
            let signature = terms
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
            let payment = PqPayment::encode(&terms, signature.as_ref(), &tx.to_bytes()).unwrap();
            postfiat_privacy_binius::verify_l1(&payment, chain).unwrap();
            fs::write(dir.join(format!("{name}-{step}.bin")), payment.bytes()).unwrap();
            input = SpendInput::Real {
                leaf_index: tree.len(),
                spend_secret: wallet.spend_secret,
                value,
                r_blind: blind,
            };
            tree.append(tx.coin_out0);
            tree.append(tx.coin_out1);
        }
    }
}
