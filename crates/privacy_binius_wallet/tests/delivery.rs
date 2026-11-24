use pft_binius_shielded::{payment::seal_stealth_envelope, shake256::coin_commit};
use postfiat_privacy_binius_wallet::keys::{derive, Network};
use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;

#[test]
fn independent_delivery_cases() {
    let v: serde_json::Value =
        serde_json::from_str(include_str!("../vectors/derivation.json")).unwrap();
    let n = Network {
        chain_id: v["chain_id"].as_str().unwrap().into(),
        genesis_hash: v["genesis_hash"].as_str().unwrap().into(),
        protocol_version: 1,
    };
    let seed = hex::decode(v["seed"].as_str().unwrap())
        .unwrap()
        .try_into()
        .unwrap();
    let wallet = derive(
        &seed,
        &n,
        v["account"].as_u64().unwrap() as u32,
        v["index"].as_u64().unwrap() as u32,
    )
    .unwrap();
    let wrong = derive(&seed, &n, 0, 1).unwrap();
    let coin = coin_commit(&wallet.addr, 123, &[7; 78]);
    let mut rng = ChaCha20Rng::from_seed([18; 32]);
    let valid = seal_stealth_envelope(&wallet.ek_bytes, &coin, 123, &[7; 78], &mut rng).unwrap();
    let false_opening =
        seal_stealth_envelope(&wallet.ek_bytes, &coin, 124, &[7; 78], &mut rng).unwrap();
    let mut corrupt = valid;
    *corrupt.last_mut().unwrap() ^= 1;
    assert!(wallet.try_scan_output(&coin, &valid) == Some((123, [7; 78])));
    assert!(wallet.try_scan_output(&coin, &false_opening).is_none());
    assert!(wallet.try_scan_output(&coin, &corrupt).is_none());
    assert!(wrong.try_scan_output(&coin, &valid).is_none());
    let bytes = [coin.as_slice(), &valid, &false_opening].concat();
    assert!(include_bytes!("../vectors/delivery.bin").as_slice() == bytes);
}

#[test]
fn scan_a_page_of_nonmatching_outputs_on_one_key() {
    use pft_binius_shielded::payment::ShieldedWallet;
    let mut rng = ChaCha20Rng::from_seed([71; 32]);
    let other = ShieldedWallet::from_seed(&mut rng);
    let scanning = ShieldedWallet::from_seed(&mut rng);
    let mut page = Vec::new();
    for value in 1..=256 {
        let coin = coin_commit(&other.addr, value, &[3; 78]);
        let env = seal_stealth_envelope(&other.ek_bytes, &coin, value, &[3; 78], &mut rng).unwrap();
        page.push((coin, env));
    }
    let start = std::time::Instant::now();
    for _ in 0..16 {
        for (coin, env) in &page {
            assert!(scanning.try_scan_output(coin, env).is_none());
        }
    }
    let time = start.elapsed().as_secs_f64();
    println!(
        "4096 nonmatching output trials, 1 key, {time:.6} s, {:.0} outputs/s",
        4096.0 / time
    );
}
