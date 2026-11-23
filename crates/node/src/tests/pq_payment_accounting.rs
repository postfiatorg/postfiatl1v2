//! Closed, pre-funded local settlement fixture. Every accepted edge goes through
//! apply_shielded_batch, commits a certified block, and is read back from disk.
use super::vault_bridge_governed_route::{activate_route, amendment, route, route_ledger};
use super::*;
use pft_binius_shielded::{
    payment::{create_profiled_tx, ShieldedWallet, SpendInput},
    shake256::{nullifier, MerkleTree24},
};
use postfiat_crypto_provider::{ml_dsa_65_keygen_from_seed, MlDsa65KeyPair};
use postfiat_privacy_binius::{sign_l1_shield, ShieldOutputIntent};
use postfiat_types::{
    pq_payment::{self as pq, Chain, Mode, PqPayment, Terms},
    AssetDefinition, AssetOrchardEgressPolicyRecordV1, TrustLine, VaultBridgeBucketState,
};
use rand::{rngs::StdRng, SeedableRng};

struct Fixture {
    dir: PathBuf,
    genesis: Genesis,
    family: String,
    child: String,
    sibling: String,
    alice: MlDsa65KeyPair,
    sponsor: MlDsa65KeyPair,
    fresh: String,
    starting_supply: u64,
}

fn line(asset: &AssetDefinition, holder: &str, balance: u64, limit: u64) -> TrustLine {
    let mut line = TrustLine::new(holder, &asset.issuer, &asset.asset_id, limit, 0).unwrap();
    line.balance = balance;
    line.authorized = true;
    line
}

impl Fixture {
    fn new() -> Self {
        let dir = unique_test_dir("pq-accounting");
        init_consensus_v2(InitConsensusV2Options {
            data_dir: dir.clone(),
            node_id: "validator-0".into(),
            chain_id: pq_payment_fixture::CHAIN_ID.into(),
            validator_count: 6,
            activation_height: 1000,
            storage_activation_height: None,
        })
        .unwrap();
        let store = NodeStore::new(&dir);
        let genesis = store.read_genesis().unwrap();
        assert_eq!(genesis_hash(&genesis), pq_payment_fixture::GENESIS_HASH);
        let mut ledger = store.read_ledger().unwrap();
        let mut governance = store.read_governance().unwrap();
        let issuer = address_from_public_key(&ml_dsa_65_keygen_from_seed(&[7; 32]).public_key);
        let alice = ml_dsa_65_keygen_from_seed(&[41; 32]);
        let sponsor = ml_dsa_65_keygen_from_seed(&[42; 32]);
        let depositor = address_from_public_key(&alice.public_key);
        let relayer = address_from_public_key(&sponsor.public_key);
        let fresh = address_from_public_key(&ml_dsa_65_keygen_from_seed(&[43; 32]).public_key);
        let starting_supply = ledger.accounts.iter().map(|a| a.balance).sum();
        ledger.accounts[0].balance -= 3000;
        for holder in [&issuer, &depositor, &relayer] {
            ledger.ensure_account(holder).balance = 1000;
        }
        let mut family = AssetDefinition::new(&genesis.chain_id, &issuer, "pfUSDC", 1, 6).unwrap();
        family.requires_authorization = true;
        family.freeze_enabled = true;
        family.max_supply = Some(110);
        let mut profile = route(1, 1, "22");
        profile.asset_id = family.asset_id.clone();
        let mut nav = route_ledger(&profile);
        nav.nav_assets[0].issuer = issuer;
        ledger.nav_assets = nav.nav_assets;
        ledger.nav_proof_profiles = nav.nav_proof_profiles;
        for kind in [
            postfiat_types::GOVERNANCE_KIND_VAULT_BRIDGE_ROUTE_AUTHORITY_ACTIVATION_HEIGHT,
            postfiat_types::GOVERNANCE_KIND_PFUSDC_SOURCE_SERIES_ACTIVATION_HEIGHT,
            postfiat_types::GOVERNANCE_KIND_SHIELDED_ATOMIC_BATCH_ACTIVATION_HEIGHT,
            postfiat_types::GOVERNANCE_KIND_REQUIRE_MEMO_V2_ACTIVATION_HEIGHT,
        ] {
            governance.apply(amendment(kind, 1, 0));
        }
        let mut activation = amendment(postfiat_types::PQ_REHEARSAL_V1, 1, 0);
        activation.chain_id = genesis.chain_id.clone();
        activation.genesis_hash = genesis_hash(&genesis);
        activation.protocol_version = genesis.protocol_version;
        governance.apply(activation);
        let receipt = activate_route(&mut governance, &mut ledger, &profile);
        assert!(receipt.accepted, "{}", receipt.message);
        let backed_series = |profile: &postfiat_types::VaultBridgeRouteProfileV1, amount, name| {
            let mut bucket = VaultBridgeBucketState::new(
                &family.asset_id,
                profile.source_domain(),
                profile.profile_hash().unwrap(),
                1,
            )
            .unwrap();
            bucket.counted_value_atoms = amount;
            bucket.gross_receipt_atoms = amount;
            bucket.outstanding_vault_bridge_atoms = amount;
            bucket.validate().unwrap();
            let asset = AssetDefinition::new_source_series(
                &family,
                profile
                    .bucket_source_series_id(&genesis.chain_id, &bucket)
                    .unwrap(),
                &bucket.bucket_id,
                name,
            )
            .unwrap();
            (bucket, asset)
        };
        let (bucket, child) = backed_series(&profile, 100, "local pfUSDC");
        let mut sibling_profile = profile.clone();
        sibling_profile.route_epoch = 2;
        let (sibling_bucket, sibling) = backed_series(&sibling_profile, 10, "sibling issue");
        ledger.trustlines.extend([
            line(&child, &depositor, 100, 100),
            line(&child, &relayer, 0, 100),
            line(&sibling, &depositor, 10, 100),
        ]);
        ledger
            .vault_bridge_bucket_states
            .extend([bucket, sibling_bucket]);
        ledger
            .asset_definitions
            .extend([family.clone(), child.clone(), sibling.clone()]);
        store.write_ledger(&ledger).unwrap();
        store.write_governance(&governance).unwrap();
        Self {
            dir,
            genesis,
            family: family.asset_id,
            child: child.asset_id,
            sibling: sibling.asset_id,
            alice,
            sponsor,
            fresh,
            starting_supply,
        }
    }

    fn chain(&self) -> Chain<'_> {
        Chain {
            id: &self.genesis.chain_id,
            genesis: hex_to_bytes(pq_payment_fixture::GENESIS_HASH)
                .unwrap()
                .try_into()
                .unwrap(),
            protocol: 1,
        }
    }

    fn apply(&self, payments: &[PqPayment]) -> io::Result<Vec<Receipt>> {
        let batch = build_atomic_shielded_action_batch(
            &self.genesis,
            payments
                .iter()
                .cloned()
                .map(ShieldedAction::PqPaymentV1)
                .collect(),
        )
        .unwrap();
        let path = self.dir.join("pq-batch.json");
        write_shielded_action_batch_file(&path, &batch)?;
        apply_shielded_batch(ApplyBatchOptions {
            data_dir: self.dir.clone(),
            batch_file: path,
            certificate_file: None,
        })
    }

    fn history(&self, address: &str) -> AccountTxReport {
        account_tx(AccountTxQueryOptions {
            data_dir: self.dir.clone(),
            address: address.into(),
            from_height: None,
            to_height: None,
            limit: Some(100),
        })
        .unwrap()
    }

    fn check_cap(&self) {
        let disk = NodeStore::new(&self.dir);
        let ledger = disk.read_ledger().unwrap();
        let key = ml_dsa_65_keygen_from_seed(&[7; 32]);
        let issuer = address_from_public_key(&key.public_key);
        let tx = signed_asset_transaction_for_test(
            &self.genesis,
            &ledger,
            &issuer,
            &bytes_to_hex(&key.public_key),
            &bytes_to_hex(&key.private_key),
            postfiat_types::ISSUED_PAYMENT_TRANSACTION_KIND,
            1,
            AssetTransactionOperation::IssuedPayment(postfiat_types::IssuedPaymentOperation {
                from: issuer.clone(),
                issuer: issuer.clone(),
                to: address_from_public_key(&self.alice.public_key),
                asset_id: self.sibling.clone(),
                amount: 1,
            }),
        );
        let before = disk.read_mempool().unwrap();
        let error = submit_signed_asset_transaction_json_to_mempool(
            SignedAssetTransactionJsonSubmitOptions {
                data_dir: self.dir.clone(),
                signed_asset_transaction_json: serde_json::to_string(&tx).unwrap(),
            },
        )
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("global supply 111 exceeds max_supply 110"),
            "{error}"
        );
        assert_eq!(disk.read_mempool().unwrap(), before);
        assert_eq!(disk.read_ledger().unwrap(), ledger);
    }
}

fn authorization(key: &MlDsa65KeyPair, sequence: u64) -> pq::Account {
    let dest = pq::destination(&address_from_public_key(&key.public_key)).unwrap();
    pq::Account {
        id: dest[12..].try_into().unwrap(),
        sequence,
        public_key: key.public_key.as_slice().try_into().map(Box::new).unwrap(),
    }
}

fn altered(payment: &PqPayment, at: usize) -> PqPayment {
    let mut bytes = payment.bytes().to_vec();
    bytes[at] ^= 1;
    PqPayment::from_bytes(bytes).unwrap()
}

fn rejection(
    f: &Fixture,
    payment: &PqPayment,
    ledger: &LedgerState,
    pool: &ShieldedState,
    gov: &GovernanceState,
    height: u64,
    code: &str,
) {
    let mut l = ledger.clone();
    let mut s = pool.clone();
    let r = crate::pq_payment_actions::apply(
        &f.genesis,
        pq_payment_fixture::GENESIS_HASH,
        &mut l,
        &mut s,
        payment,
        height,
        asset_execution_compatibility_for_genesis_and_governance(&f.genesis, gov),
        gov,
        &gov.asset_orchard_egress_policy_registry(),
    );
    assert_eq!(
        r.err(),
        Some(code),
        "expected PQ refusal {code} at height {height}"
    );
    assert_eq!(&l, ledger);
    assert_eq!(&s, pool);
}

// Replay every authenticated output through both KEM keys. Only live notes
// contribute to the balances; the next prover input comes from decryption.
fn recover(
    f: &Fixture,
    pool: &pq::PqPool,
    asset: &str,
    wallets: &[ShieldedWallet; 2],
) -> ([u64; 2], SpendInput) {
    let mut balance = [0; 2];
    let mut spend = SpendInput::Dummy;
    for index in 0..pool.head.assets[asset].writable.count {
        let id = pool.indexes.page_for_output(asset, 0, index).unwrap();
        let page = postfiat_storage::pq_pool::read_page(&f.dir, id).unwrap();
        let output = &page.outputs[(index - page.first_index) as usize];
        let coin = hex_to_bytes(&output.commitment)
            .unwrap()
            .try_into()
            .unwrap();
        let envelope = output.envelope().unwrap().try_into().unwrap();
        for (w, wallet) in wallets.iter().enumerate() {
            if let Some((value, r_blind)) = wallet.try_scan_output(&coin, &envelope) {
                let serial = bytes_to_hex(&nullifier(&wallet.spend_secret, &coin));
                if value > 0 && !pool.indexes.is_spent(&serial) {
                    balance[w] += value;
                    spend = SpendInput::Real {
                        leaf_index: index as usize,
                        spend_secret: wallet.spend_secret,
                        value,
                        r_blind,
                    };
                }
            }
        }
    }
    (balance, spend)
}

fn refusal_cases(f: &Fixture, payment: &PqPayment, issued: bool, step: usize, height: u64) {
    let disk = NodeStore::new(&f.dir);
    let ledger = disk.read_ledger().unwrap();
    let shielded = disk.read_shielded().unwrap();
    let gov = disk.read_governance().unwrap();
    let alice = address_from_public_key(&f.alice.public_key);
    let sponsor = address_from_public_key(&f.sponsor.public_key);
    let (terms, sig, inner) = payment.parts().unwrap();
    let start = payment.bytes().len() - inner.len();
    let reject =
        |p: &PqPayment, l: &LedgerState, s: &ShieldedState, g: &GovernanceState, code: &str| {
            rejection(f, p, l, s, g, height, code)
        };
    if step == 0 {
        for offset in [208, 256, 304, 304 + pq::ENVELOPE_BYTES] {
            reject(
                &altered(payment, start + offset),
                &ledger,
                &shielded,
                &gov,
                "pq_invalid_proof",
            );
        }
        assert!(PqPayment::encode(&terms, None, inner).is_err());
        let mut full = shielded.clone();
        let pool = full.pq_pool_v1.get_or_insert_with(Default::default);
        pool.head
            .assets
            .entry(bytes_to_hex(&terms.asset))
            .or_default()
            .reserve = u64::MAX;
        reject(payment, &ledger, &full, &gov, "pq_reserve");
        let mut unfunded = ledger.clone();
        unfunded.account_mut(&alice).unwrap().balance = ACCOUNT_RESERVE;
        reject(payment, &unfunded, &shielded, &gov, "pq_unfunded_source");
        let mut stale = ledger.clone();
        stale.account_mut(&alice).unwrap().sequence += 1;
        reject(payment, &stale, &shielded, &gov, "pq_source_authorization");
        let mut pinned = ledger.clone();
        pinned.account_mut(&alice).unwrap().public_key_hex =
            Some(bytes_to_hex(&f.sponsor.public_key));
        reject(payment, &pinned, &shielded, &gov, "pq_source_authorization");
        // Try the exact shield signature on an otherwise correctly formed account burn.
        let mut burn = signed_asset_transaction_for_test(
            &f.genesis,
            &ledger,
            &alice,
            &bytes_to_hex(&f.alice.public_key),
            &bytes_to_hex(&f.alice.private_key),
            postfiat_types::ASSET_BURN_TRANSACTION_KIND,
            terms.account.unwrap().sequence,
            AssetTransactionOperation::AssetBurn(postfiat_types::AssetBurnOperation {
                owner: alice.clone(),
                issuer: ledger.asset_definition(&f.child).unwrap().issuer.clone(),
                asset_id: f.child.clone(),
                amount: 100,
            }),
        );
        let burn_sig = hex_to_bytes(&burn.signature_hex)
            .unwrap()
            .try_into()
            .unwrap();
        let reverse =
            PqPayment::encode(&payment.parts().unwrap().0, Some(&burn_sig), inner).unwrap();
        reject(&reverse, &ledger, &shielded, &gov, "pq_invalid_proof");
        burn.signature_hex = bytes_to_hex(sig.unwrap());
        let mut trial = ledger.clone();
        assert_eq!(
            postfiat_execution::execute_asset_transaction(&f.genesis, &mut trial, &burn, height)
                .code,
            "bad_signature"
        );
        assert_eq!(trial, ledger);
        let ledger_before = disk.read_ledger().unwrap();
        let receipts = disk.read_receipts().unwrap();
        let blocks = disk.read_blocks().unwrap();
        let history = f.history(&alice).rows;
        let error = f.apply(&[payment.clone(), payment.clone()]).unwrap_err();
        assert!(error.to_string().contains("pq_spent_serial"), "{error}");
        assert_eq!(disk.read_ledger().unwrap(), ledger_before);
        assert_eq!(disk.read_shielded().unwrap(), shielded);
        assert_eq!(disk.read_receipts().unwrap(), receipts);
        assert_eq!(disk.read_blocks().unwrap(), blocks);
        assert_eq!(f.history(&alice).rows, history);
    }
    if issued && (step == 0 || step == 2) {
        let holder = if step == 0 { &alice } else { &sponsor };
        let pos = ledger
            .trustlines
            .iter()
            .position(|l| l.account == *holder && l.asset_id == f.child)
            .unwrap();
        for (which, code) in [
            (0, "frozen_trustline"),
            (1, "missing_issuer_authorization"),
            (2, "pq_missing_trustline"),
        ] {
            let mut l = ledger.clone();
            match which {
                0 => l.trustlines[pos].frozen = true,
                1 => l.trustlines[pos].authorized = false,
                _ => l.trustlines[pos].asset_id = f.sibling.clone(),
            }
            reject(payment, &l, &shielded, &gov, code);
        }
        let mut bad = ledger.clone();
        if step == 0 {
            bad.trustlines[pos].balance = 99;
            reject(
                payment,
                &bad,
                &shielded,
                &gov,
                "pq_insufficient_issued_balance",
            );
        } else {
            bad.trustlines[pos].limit = 19;
            reject(payment, &bad, &shielded, &gov, "pq_trustline_limit");
            bad.trustlines[pos].balance = u64::MAX;
            reject(payment, &bad, &shielded, &gov, "issued_balance_overflow");
            bad = ledger.clone();
            bad.account_mut(&sponsor).unwrap().flags = postfiat_types::ACCOUNT_FLAG_REQUIRE_MEMO;
            reject(
                payment,
                &bad,
                &shielded,
                &gov,
                postfiat_types::RECEIPT_CODE_DESTINATION_MEMO_REQUIRED,
            );
        }
        let mut disclosure = gov.clone();
        let record =
            AssetOrchardEgressPolicyRecordV1::new(&f.child, "disclose", "aa".repeat(48), height)
                .unwrap();
        disclosure.apply(amendment(&record.governance_kind(), height as u32, 0));
        reject(
            payment,
            &ledger,
            &shielded,
            &disclosure,
            "pq_unsupported_disclosure",
        );
        let mut paused = ledger.clone();
        paused.vault_bridge_bucket_states[0].status =
            postfiat_types::VAULT_BRIDGE_BUCKET_STATUS_PAUSED.into();
        reject(payment, &paused, &shielded, &gov, "pq_issue_backing");
    }
    if step == 2 {
        reject(
            &altered(payment, start + 103),
            &ledger,
            &shielded,
            &gov,
            "pq_invalid_proof",
        );
        let mut short = shielded.clone();
        let asset = bytes_to_hex(&payment.parts().unwrap().0.asset);
        short
            .pq_pool_v1
            .as_mut()
            .unwrap()
            .head
            .assets
            .get_mut(&asset)
            .unwrap()
            .reserve = 1;
        reject(payment, &ledger, &short, &gov, "pq_reserve");
        if !issued {
            let mut overflow = ledger.clone();
            overflow.ensure_account(&f.fresh).balance = u64::MAX;
            reject(payment, &overflow, &shielded, &gov, "pq_recipient_overflow");
        }
    }
    if !issued && step == 3 {
        let mut no_recipient = ledger.clone();
        no_recipient.accounts.retain(|a| a.address != f.fresh);
        reject(payment, &no_recipient, &shielded, &gov, "pq_minimum_fee");
        assert_eq!(
            postfiat_execution::prepare_pq_native_credit(
                &no_recipient,
                &f.fresh,
                ACCOUNT_RESERVE - 1
            )
            .unwrap_err(),
            "pq_recipient_reserve"
        );
    }
    if issued && step == 1 {
        let mut exhausted = ledger.clone();
        exhausted.account_mut(&sponsor).unwrap().balance = ACCOUNT_RESERVE;
        reject(payment, &exhausted, &shielded, &gov, "pq_unfunded_source");
    }
}

#[test]
fn two_wallets_settle_both_assets_and_reconcile_every_certified_block() {
    let f = Fixture::new();
    let disk = NodeStore::new(&f.dir);
    let mut rng = StdRng::seed_from_u64(904);
    let wallets = [
        ShieldedWallet::from_seed(&mut rng),
        ShieldedWallet::from_seed(&mut rng),
    ];
    let alice = address_from_public_key(&f.alice.public_key);
    let sponsor = address_from_public_key(&f.sponsor.public_key);
    let mut burned = 0;
    let mut height = 0;
    let mut alice_pft = 1000;
    let mut sponsor_pft = 1000;
    let mut fresh_pft = 0;
    let mut alice_seq = 0;
    let mut sponsor_seq = 0;
    let mut deposited = 0;
    let mut withdrawn = 0;
    for issued in [false, true] {
        let sample = std::env::var_os("PQ_VALIDATOR_BENCH").is_some().then(|| {
            (
                disk.read_ledger().unwrap(),
                disk.read_shielded().unwrap(),
                disk.read_ordered_batches().unwrap(),
                height + 1,
            )
        });
        let mut measured_payments = Vec::new();
        let asset = if issued {
            f.child.clone()
        } else {
            "00".repeat(48)
        };
        let mut tree = MerkleTree24::new();
        let mut input = SpendInput::Dummy;
        let mut value = 0;
        for (step, mode) in [Mode::Shield, Mode::Internal, Mode::Exit, Mode::Exit]
            .into_iter()
            .enumerate()
        {
            height += 1;
            let fee = if issued {
                0
            } else if step == 2 {
                1 + TRANSFER_ACCOUNT_CREATION_FEE
            } else {
                1
            };
            let mint = if step == 0 { 100 } else { 0 };
            let exit = match step {
                2 => 20,
                3 => value - fee,
                _ => 0,
            };
            value = value + mint - exit - fee;
            let receiver = if step == 0 { 0 } else { 1 };
            let dest = if exit == 0 {
                [0; 32]
            } else {
                pq::destination(if issued { &sponsor } else { &f.fresh }).unwrap()
            };
            let terms = Terms {
                mode,
                asset: hex_to_bytes(&asset).unwrap().try_into().unwrap(),
                epoch: 0,
                public_fee: if issued { 3 } else { 0 },
                account: if step == 0 {
                    alice_seq += 1;
                    Some(authorization(&f.alice, alice_seq))
                } else if issued {
                    sponsor_seq += 1;
                    Some(authorization(&f.sponsor, sponsor_seq))
                } else {
                    None
                },
            };
            let (tx, r0, r1) = create_profiled_tx(
                &tree,
                input,
                SpendInput::Dummy,
                &wallets[receiver].stealth_address(),
                value,
                &wallets[receiver].stealth_address(),
                0,
                mint,
                exit,
                dest,
                fee,
                None,
                Some(&|raw| terms.statement_context(f.chain(), raw)),
                &mut rng,
            )
            .unwrap();
            let inner = tx.to_bytes();
            let payment = if step == 0 {
                let output = |intended| {
                    [
                        ShieldOutputIntent {
                            owner: &wallets[receiver].addr,
                            value: intended,
                            blinder: &r0,
                            envelope: &tx.env0,
                        },
                        ShieldOutputIntent {
                            owner: &wallets[receiver].addr,
                            value: 0,
                            blinder: &r1,
                            envelope: &tx.env1,
                        },
                    ]
                };
                assert!(sign_l1_shield(
                    f.chain(),
                    &terms,
                    &inner,
                    mint,
                    fee,
                    output(value + 1),
                    &f.alice.private_key
                )
                .is_err());
                sign_l1_shield(
                    f.chain(),
                    &terms,
                    &inner,
                    mint,
                    fee,
                    output(value),
                    &f.alice.private_key,
                )
                .unwrap()
            } else {
                let signature =
                    terms
                        .signing_message(f.chain(), tx.statement().tx_ctx)
                        .map(|msg| {
                            ml_dsa_65_sign(&f.sponsor.private_key, &msg)
                                .unwrap()
                                .try_into()
                                .unwrap()
                        });
                PqPayment::encode(&terms, signature.as_ref(), &inner).unwrap()
            };
            refusal_cases(&f, &payment, issued, step, height);
            if issued && step == 0 {
                // Both the proof and signature commit to the exact child.
                let mut sibling = payment.bytes().to_vec();
                sibling[7..55].copy_from_slice(&hex_to_bytes(&f.sibling).unwrap());
                assert!(postfiat_privacy_binius::verify_l1(
                    &PqPayment::from_bytes(sibling).unwrap(),
                    f.chain()
                )
                .is_err());
                // An independently proved, signed deposit of units actually held
                // on the sibling line still lacks opt-in for that exact issue.
                let other = Terms {
                    asset: hex_to_bytes(&f.sibling).unwrap().try_into().unwrap(),
                    ..terms.clone()
                };
                let (tx, r0, r1) = create_profiled_tx(
                    &tree,
                    SpendInput::Dummy,
                    SpendInput::Dummy,
                    &wallets[0].stealth_address(),
                    10,
                    &wallets[0].stealth_address(),
                    0,
                    10,
                    0,
                    [0; 32],
                    0,
                    None,
                    Some(&|raw| other.statement_context(f.chain(), raw)),
                    &mut rng,
                )
                .unwrap();
                let other_payment = sign_l1_shield(
                    f.chain(),
                    &other,
                    &tx.to_bytes(),
                    10,
                    0,
                    [
                        ShieldOutputIntent {
                            owner: &wallets[0].addr,
                            value: 10,
                            blinder: &r0,
                            envelope: &tx.env0,
                        },
                        ShieldOutputIntent {
                            owner: &wallets[0].addr,
                            value: 0,
                            blinder: &r1,
                            envelope: &tx.env1,
                        },
                    ],
                    &f.alice.private_key,
                )
                .unwrap();
                assert!(f
                    .apply(&[other_payment])
                    .unwrap_err()
                    .to_string()
                    .contains("pq_issue_disabled"));
            }
            if sample.is_some() {
                measured_payments.push(payment.clone());
            }
            let receipt = f.apply(&[payment]).unwrap().pop().unwrap();
            let charged = if issued { 3 } else { fee };
            assert!(receipt.accepted);
            assert_eq!(
                (receipt.fee_charged, receipt.fee_burned),
                (charged, charged)
            );
            assert_eq!(
                receipt.state_expansion_fee,
                if !issued && step == 2 {
                    TRANSFER_ACCOUNT_CREATION_FEE
                } else {
                    0
                }
            );
            burned += charged;
            if !issued {
                alice_pft -= mint;
                fresh_pft += exit
            } else if step == 0 {
                alice_pft -= 3;
                deposited += mint
            } else {
                sponsor_pft -= 3;
                withdrawn += exit
            }
            let ledger = disk.read_ledger().unwrap();
            let shielded = disk.read_shielded().unwrap();
            let pool = shielded.pq_pool_v1.as_ref().unwrap();
            let block = disk.read_blocks().unwrap().blocks.pop().unwrap();
            assert_eq!(block.header.height, height);
            assert!(
                block
                    .header
                    .certificate
                    .votes
                    .iter()
                    .filter(|v| v.accept)
                    .count()
                    >= block.header.certificate.quorum
            );
            assert_eq!(block.receipt_ids, vec![receipt.tx_id.clone()]);
            let (notes, next) = recover(&f, pool, &asset, &wallets);
            input = next;
            let mut expected = [0; 2];
            expected[receiver] = value;
            assert_eq!(notes, expected);
            assert_eq!(pool.head.assets[&asset].reserve, value);
            tree.append(tx.coin_out0);
            tree.append(tx.coin_out1);
            let public: u64 = ledger.accounts.iter().map(|a| a.balance).sum();
            let native = pool.head.assets[&"00".repeat(48)].reserve;
            let total_burned: u64 = disk
                .read_receipts()
                .unwrap()
                .iter()
                .map(|r| r.fee_burned)
                .sum();
            assert_eq!(total_burned, burned);
            let metrics = crate::metrics(NodeOptions {
                data_dir: f.dir.clone(),
            })
            .unwrap();
            assert_eq!(metrics.execution.burned_fee_total, burned);
            assert_eq!(metrics.assets.total_outstanding_supply, 110);
            assert_eq!(public + native + total_burned, f.starting_supply);
            let account = ledger.account(&alice).unwrap();
            assert_eq!((account.balance, account.sequence), (alice_pft, alice_seq));
            assert_eq!(
                account.public_key_hex,
                Some(bytes_to_hex(&f.alice.public_key))
            );
            let account = ledger.account(&sponsor).unwrap();
            assert_eq!(
                (account.balance, account.sequence),
                (sponsor_pft, sponsor_seq)
            );
            assert_eq!(
                account.public_key_hex,
                (sponsor_seq > 0).then(|| bytes_to_hex(&f.sponsor.public_key))
            );
            assert_eq!(ledger.account(&f.fresh).map_or(0, |a| a.balance), fresh_pft);
            assert_eq!(
                ledger
                    .trustline_for_account_asset(&alice, &f.child)
                    .unwrap()
                    .balance,
                100 - deposited
            );
            assert_eq!(
                ledger
                    .trustline_for_account_asset(&sponsor, &f.child)
                    .unwrap()
                    .balance,
                withdrawn
            );
            assert_eq!(
                global_issued_asset_supply(&ledger, &shielded, &f.child).unwrap(),
                100
            );
            assert_eq!(
                global_issued_asset_supply(&ledger, &shielded, &f.family).unwrap(),
                110
            );
            if issued && step == 0 {
                f.check_cap();
            }
            println!("block {height}: public PFT {public} + live PQ {native} + burned {burned} = {}; pfUSDC family 110; wallet notes {:?}",f.starting_supply,notes);
        }
        if let Some((ledger, shielded, ordered, height)) = sample {
            super::pq_payment_activation::benchmark(
                &f.genesis,
                &disk.read_governance().unwrap(),
                &ledger,
                shielded,
                &disk.read_bridge().unwrap(),
                &ordered,
                height,
                &measured_payments,
                if issued {
                    pq::NATIVE_ASSET_HEX
                } else {
                    &f.child
                },
                if issued { &sponsor } else { &f.fresh },
            );
        }
    }
    let issue = Some(f.child.as_str());
    for (address, expected) in [
        (
            &alice,
            vec![
                ("shielded_deposit", 100, 1, 1, None),
                ("shielded_deposit", 100, 3, 2, issue),
            ],
        ),
        (
            &sponsor,
            vec![
                ("shielded_fee", 0, 3, 1, None),
                ("shielded_fee", 0, 3, 2, None),
                ("shielded_withdrawal", 20, 0, 0, issue),
                ("shielded_fee", 0, 3, 3, None),
                ("shielded_withdrawal", 80, 0, 0, issue),
            ],
        ),
        (
            &f.fresh,
            vec![
                ("shielded_withdrawal", 20, 11, 0, None),
                ("shielded_withdrawal", 66, 1, 0, None),
            ],
        ),
    ] {
        let rows = f.history(address).rows;
        let observed = rows
            .iter()
            .map(|r| {
                (
                    r.tx_role.as_deref().unwrap(),
                    r.amount,
                    r.fee,
                    r.sequence,
                    r.asset_id.as_deref(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(observed, expected);
    }
    let before = f.history(&sponsor).rows;
    rebuild_account_tx_index(AccountTxIndexOptions {
        data_dir: f.dir.clone(),
    })
    .unwrap();
    let after = f.history(&sponsor);
    let rpc = postfiat_rpc_sdk::success_response("pq-history", &after, vec![]).unwrap();
    postfiat_rpc_sdk::validate_response_kind(&rpc, postfiat_rpc_sdk::RpcResponseKind::AccountTx)
        .unwrap();
    assert!(after.index_used);
    assert_eq!(before, after.rows);
    fs::remove_dir_all(&f.dir).unwrap();
}

#[cfg(feature = "pq-wallet")]
#[path = "pq_wallet_merge.rs"]
mod wallet_cli;

#[cfg(feature = "pq-wallet-browser-test")]
#[path = "pq_browser_merge.rs"]
mod browser;
