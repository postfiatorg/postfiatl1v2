# M8 test report (worker report, re-checked by the coordinator)

**Historical evidence.** The original milestone measurements and artifact inventory tested L1 profile 1, including the public PFT payer for issued spends. Issued payments and exits now use profile 2.

Coordinator addendum, 23 Nov: item 1 of UNMET below is resolved as expected fail-closed upgrade behaviour (see rehearsal-record.md, Upgrade compatibility; the test now asserts it and passes). Item 4's four failures also fail on main at 071b6dc46 and predate this work.

```text
BLOCKED

M8 rehearsal, regressions and review package

1208c826d89bcec486526b6fbf05458269ed1581 Connect the rendered rehearsal to its review package and pinned integration plan
1799899d12c775304f8064624e5d398c5669111b Record the release rehearsal, qualification failures and reviewer scope
e7fa39058489b7a237ca98c008392099eac378c2 Cold-start the original validator and the signed import independently
970a296b76486fc50925e735d0f169f7e3b5f426 Rehearse private payment activation, pause and recovery across six validators

The candidate replayed both chains. The old node fails on the fresh pre-H governance block. The signed-snapshot wallet recovered 99 PFT and 100 pfUSDC after the pause. No real activation height was scheduled.

The rehearsal record includes the complete per-asset supply tables, tip hashes, pool and state roots, certificate IDs and accepted receipt IDs for these exact runs.
$WORK/pq/docs/evidence/pq-private-payments-m8/rehearsal-record.md
$WORK/pq/docs/evidence/pq-private-payments-m8/external-review-package.md

CPU(s):                               32
Model name:                           AMD EPYC Processor
Thread(s) per core:                   2
Hypervisor vendor:                    KVM
L3 cache:                             32 MiB (1 instance)
Fresh rehearsal values
SUPPLY genesis=e9c735eecfc4550f152330b52f782de84eea2aa7d59dc7e9e317718ed2ba28473854f0cb7c6c972f7f2825ba944b72e8 PFT_live=999999413 receipt_fee_burn=587 fastpay_fee_burn=0 genesis_native=1000000000
FRESH H=6 pause=29 first_blocked=30 height=30 tip=dc963b2bef20e0f6541a7ee6ffc074ee9c080c159c1f2cde2c3fcc6f48f0df8e7de6033460aa93c234dc9479826192dd root=d26fd217c54d1ac47ec3a4694acf8eeb425eaef846e4a1f35fd485d423d107bae8702c339bf1094ee4d2e96cb46373ba
POOL asset=000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000 epoch=0 reserve=494 root=22481c24c248a8ec94f1070211bd8b695a9c5a0ddd4112d55be2ffcff05f171700d9d6458c935547c7a85b16f841108e
POOL asset=bbf2fb96437f2e87ff3b470f19aa2455ae226ab810b06b64ab327b0daa71a2d1ad14638a107b7bbd3b0cf3a6f4d56269 epoch=0 reserve=100 root=b62742dc5bb69ce28794992f720fb3834f66e3f80c82182ca06d06f671723839e9249ae13c128c561dc7ddc56e5f7034
ASSET A666 9f7a9e7b5fe8a4a7f26aec66c9fddfe1817c96cbc141f85a712b548149dda5b3b541736f14c8caa4c9d22b97e4e095a0 family 200
ASSET USDC 82c3308ffdaf0df2d30fcd8d2a508dc99719777c2822424986e194a6902af1fd3ebe698d87dca89f711a02be7bb1426b family 200
ASSET pfUSDC d48ccde2847cd40aa54d1a2a292c6f4dd714f2c72dc0d79584c7d308969977122fcb7d4426af3a5c195d635d752410c9 family 100
ASSET pfUSDC bbf2fb96437f2e87ff3b470f19aa2455ae226ab810b06b64ab327b0daa71a2d1ad14638a107b7bbd3b0cf3a6f4d56269 series 100
READBACK receipts=7 largest_body_bytes=144397 largest_charged_bytes=145140 worst_uncached_verify_l1_ms=32.652
READBACK receipts=7 largest_body_bytes=144397 largest_charged_bytes=145140 worst_uncached_verify_l1_ms=40.259
ordinary six-validator round 844.65 ms
four-proof six-validator round 979.44 ms
BLOCK h=9 proofs=4 charged=578304 votes=5 root=46808219ac24a5c0cc798867e413bbbfa484ce8ac1cfefcde8bdd2c90e1463b9f9812acc92702ba2f9e27b415b5b3591
ROLLOVER epoch=0 leaves=16777216 terminal=bc84ead5ae686c067739f7653f2a7959f7f61e1f23d765dda23b53b7c598d7d89e4602c22f154bcaba0e5a782f79e2b1
ROLLOVER epoch=1 leaves=16777216 terminal=fc071c3d095be6479dab94a7f55fdcd41377c55083dcdbf73cdbe0996aca71a261c81dad2afcab7a41c22fff8b6b8cbd
Exact archive values (family totals include child series)
SUPPLY genesis=ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad25521aff3ed334da07e150a7233a3e90a9 PFT_live=999954296 receipt_fee_burn=45671 fastpay_fee_burn=33 genesis_native=1000000000
PQ_PAIRS 0
ASSET PFUSDC 02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b family 5740471455
ASSET a651 42cb574dcf156f906ce424f8555eac83c7dde33e48c44f4ef6307df705edfe036f69b1d437e8b7c630dd325684965a0c family 0
ASSET a651 85ed5a81cd7a4c662fcf45d1e2e5746827b146dbf370f9421035a31493fc0741891d76d255bce17ddf0d7c6fc50c5859 family 0
ASSET a651 de8d3d78aca83f7000632cbd3dd06edd30d105085d16eef1696f9e1707908ec9209239080d21ca1c0cecc36ca6f966b6 family 45702464561
ASSET a666 300bf48a63a94770b6e67817f88cd1abf77e7f592a061e15682d7fd9973260af4c2e631e32df3c2c402b7d2fe272a293 family 200000
ASSET A666 521c6c630bb48d4a37ab4a7bd4900dd2caa2d9e99499e452da3c7ce75b3d74b62d20e18555642bec32174498cbee5e2c family 31519438511
ASSET PNOK 4b8dd69b1f1ee425ae84d17f2fc2fe3630d904c4c356b0ff2df0e00c20109e6346d765ca4c0407ddb08edcb6525d5cc1 family 500
ASSET PFUSDC c6d2a21646091dc721332d114f6288ede715d0a6325b3cf41ec7b4865f9f2e85014e79445fb66a7141747e4f42584062 series 14000000
ASSET PFUSDC 3923511d5be0557a61051e099b606d3decc11a5ba274c7d551168735accad8ed18d89c9200efc4bfbbbab6e85d4c173f series 0
ASSET PFETH 629f1ba8f36963662d6e36243aea85627787cd72d41a9e92c5f01b60e3e68c1b951af771d16780bf13c4036b8d2a4996 family 0
ASSET PFETH 418c0dd528239758d2b684cd9fd06e066aefc5f4cc231a796a279e8cc0a31ad9e3bd474eb1c22847059300a2cce2bae2 series 0
ASSET PFUSDC 2bae082a6703375b9405af44715e1e64623265392627767b040fa2c30abb100a09da403724a6f105317292d9c0073df7 series 75067140
ASSET BMNRC 43b9ba500a8cc04b13a10f35c3187ff09fdf52a3accfb507226144dcbe2ef14f4b2e1d555c62bbdf63e5cf3e7e8231cc family 640585238
ASSET PFDEP 1489e9672c5901432631094e5b2070291f7441820651f0cae3946131d62eeca3753eced417af6e37928c0554cafc76b6 family 0
ASSET B23 ca7845134c8e95819082fcd2f1612808e20a37dfabab98dab120f99fd2c54c18ffb50d98c0664480420c881773244627 family 1000000
ASSET PFUSDC 541e451e6ae96b6680988655571a0feec59c7f1d073c9b5d29547254cab38a3be540f8767446d25baebc7081ce97aef7 series 5354189330
ASSET B24 52285e4ded1d58113cfaa12bbc034b8b10213f699b3224c74160450229d6bf7ccfe505c78f2599d975ae2ecf98f7d8af family 425323380000
ASSET PFUSDC e08c9bc78cbff20f45d84c05120b99d39674237438d5af53d62387709fa57fe3a711e084773aa7eb6cb215278dadf48e series 7303
ASSET PFUSDC aecd46d679c89533794668dcc6ee3c28ca08313603643b2d488e1fb3d7e80e80df47d51ca1e80a7339a9eeb3624b5194 series 5303
ASSET PFUSDC c843a071e81cbb2e6dfaccd5e9d4a95d23ec36f3da7a8bb1af4199e5a24e22a524d0ff4a46c6a79c7136fc9d82e875f1 series 6104
ASSET PFUSDC 19d06de09e8d4926112ac93b7ee95136f5aebb4631f0aaf3cde6c9383d5a684e4b5f9f364c951ba5f830ed66ad0bf106 series 59745
ASSET PFUSDC f8b846d762c5ae2a6340271c273d37202de2be9f65f0330cb4d5b76f9409101918a54528ce0632e0a19ea78fb6237c8e series 353539
ASSET PFUSDC 1f2048bed45e80cee0e55e5b466bc4f64376560fdfc952a8604608f89713aea2abfad25774726b97f0f1e82cf310288a series 1031452
ASSET PFUSDC baffc58ea2a9e03f15a0f9416b2e5ac0b0d1d5d5d1ee0ff1992b1a1ccc9b84c8043fe5662a474c926ab18a0ab8d1e3dd series 123407
ASSET PFUSDC 6cb83042de6e869ca412d3ff7cc3fc6b48acc8a332e1cdbee651cbb78d769ea88f88a44b5098390c069df45dbb9ac8c3 series 505
ASSET PFUSDC fc22fc6ddffebddb5411133d7dd3c8c4b59b65f81e822ad8dd270a41ccc1c193dd10165920f5fe88a0a6c9c4069dff87 series 430000
ASSET PFUSDC 7039e36cf4964085f6a847ee4c7644df39db905d3ff06f7331abaa5c73d0f86a1cffba9ead914a1cf9b101bdfb75444b series 127003
ASSET PFUSDC 68c8a42305516e1d3229de72a26a68d1824118f9d63d58bc4c74e0151e49cd4a35c650a6f8bd03b73d9aba6b514df552 series 80021
ASSET PFUSDC 873b06ac4ab8a8e06f834a71b23d012b1ddc9d58f2ef19136bb69702639ef1fbbd1acad18b98739b10e39cc247974c02 series 20008
ASSET PFUSDC 8afde29570a3b7f4392860b7dffeb9a9605e5578322c8a1ff611ac933476dd5e7a1b57c88c7ae6ddc55c0ed1ae07112c series 50000
ASSET PFUSDC 541cb1bb58a792ef337fcf88351c7ff519a979fe49f5d2c194334d1141e5758b04c114a1efbf1b478e07fdc78c42a201 series 5180000
ASSET PFUSDC fd660c0c7bdc912396cd36fca37cc09a91cef46d53ffbd1446e3425a68c24cab9bc0c10ed2b279ff8df27fafc7c6de8b series 40000
R64 replayed 1734 blocks with both binaries. Tip 910c744852de4203274bd606859edaccdc27e25181223a1dd07cf2f62da3de048ffef8e427dcf229a00cbb8225121e40, root a7b1185e94bf0e224a3944be4aaba896618594823ba2225399dad2148f6611fe527ce6a60a7852f94c8bc154a90dfe56.

Release and regression commands

Bash, repository root. Source target/m8/env for every command below. The local working-directory prefix is $WORK/pq. CARGO_TARGET_DIR=$WORK/target-m8. GO_BIN=$HOME/go/pkg/mod/golang.org/toolchain@v0.0.1-go1.25.1.linux-amd64/bin/go. PY_BIN=$HOME/repos/pft-deep-ligero-pq/.venv/bin/python. Node 22.23.1 first on PATH. For the workspace and remaining Cargo regressions, CARGO_HOME=$PWD/target/m8/cargo-home contains dependencies staged within this worktree.

git -C target/m8/old-source rev-parse HEAD → b8c8a44031090f4965e9f735fa3348fe010907d1
(cd target/m8/old-source && cargo build --release --target x86_64-unknown-linux-gnu --locked --offline -j4 -p postfiat-node --bin postfiat-node) → Finished `release` profile [optimized] target(s) in 27m 11s
cargo build --release --locked --offline -p postfiat-node --features pq-wallet --bin postfiat-node --bin pftl-pq-walletd → Finished `release` profile [optimized] target(s) in 2m 43s
cargo test --release --locked --offline -p postfiat-node --bin postfiat-node --no-run → Executable unittests src/main.rs ($WORK/target-m8/release/deps/postfiat_node-a36ef28456781f15)
$bin pq_payment_rehearsal_tests --nocapture --test-threads=1 → SUPPLY genesis=e9c735eecfc4550f152330b52f782de84eea2aa7d59dc7e9e317718ed2ba28473854f0cb7c6c972f7f2825ba944b72e8 PFT_live=999999413 receipt_fee_burn=587 fastpay_fee_burn=0 genesis_native=1000000000 / PQ_PAIRS 7 / test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 303 filtered out; finished in 89.54s
PQ_PRE_H_NODE=$PWD/target/m8/bin/m4 PQ_POST_H_NODE=$PWD/target/m8/bin/post-h $bin pq_payment_rehearsal_tests --nocapture --test-threads=1 → SUPPLY genesis=e9c735eecfc4550f152330b52f782de84eea2aa7d59dc7e9e317718ed2ba28473854f0cb7c6c972f7f2825ba944b72e8 PFT_live=999999413 receipt_fee_burn=587 fastpay_fee_burn=0 genesis_native=1000000000 / PQ_PAIRS 7 / test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 303 filtered out; finished in 91.68s
First old-source build was made from git archive, which inherited the outer build revision label. An isolated detached Git checkout supplied the final M4 binary. Both attempts returned the same governance error.
tools/pft-binius-shielded-pq/replay-r64.sh target/m3/archive target/m8/bin/m4 target/m8/bin/post-h target/m8/archive-final → R64 replayed 1734 blocks with both binaries. Tip 910c744852de4203274bd606859edaccdc27e25181223a1dd07cf2f62da3de048ffef8e427dcf229a00cbb8225121e40, root a7b1185e94bf0e224a3944be4aaba896618594823ba2225399dad2148f6611fe527ce6a60a7852f94c8bc154a90dfe56.
(cd target/m8/replay && bash ../../../target/m5/check-meter.sh archive) → c28eb87280050e96e996ad5c9e8c2eae12f2fb9774438f3a7b5f1295ef155910  target/m3/m5-final.rows
R64 status on the scratch archive and git rev-parse its reported revision; sha256sum of the retained executable → 95d5b237c991f57a9ea122e110f6a2d0c017bf9ca0a85315b2359ecba3b017d1  $HOME/var/main-r64-20261121/bin/postfiat-node / R64 674dbb18 release / 674dbb18a29931d9ae749d28270e3c6cc508795a
python3 target/m8/prefix-check.py → m4 before-H rows 5 digest 154f98e1eb481c589d9771d8b65a01b1fd384e3a1ab1871f29bc391afc8cb6c1 / m4 exit 1 error: verify-blocks failed: unsupported governance amendment kind `pq_payment_rehearsal_v1` / post-h before-H rows 5 digest 154f98e1eb481c589d9771d8b65a01b1fd384e3a1ab1871f29bc391afc8cb6c1 / post-h exit 0 {"verified": true, "block_count": 5, "tip_hash": "688862da8597e811b9b2f515da09678c1694155b33468a2bc24a06c78f9756924b0dc3f88f7f3f1c8e4fc9b508b592d6", "state_root": "e61b4cfc991788d4e309535e2188443c94ed8de4406b4e52bf8e9a99cd909f814500dc04bd1c7a92d08000e252746777"}
Original R64 node: in the isolated replay layout, bash target/m3/archive-check.sh $HOME/var/main-r64-20261121/bin/postfiat-node m8-r64; sha256sum target/m3/m8-r64.rows → c28eb87280050e96e996ad5c9e8c2eae12f2fb9774438f3a7b5f1295ef155910  target/m3/m8-r64.rows
cargo build --release --locked --offline -p postfiat-node --example pq_rehearsal_inventory → Finished `release` profile [optimized] target(s) in 10m 47s
Docker with /var/lib/postfiat/validator-3 mapped to the isolated R64 copy: /opt/inventory /var/lib/postfiat/validator-3 → SUPPLY genesis=ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad25521aff3ed334da07e150a7233a3e90a9 PFT_live=999954296 receipt_fee_burn=45671 fastpay_fee_burn=33 genesis_native=1000000000
Same copy: /opt/node orchard-pool-report --data-dir /var/lib/postfiat/validator-3 → {"counters": {"accepted_anchor_count": 41, "accounted_pool_deposit_total": 0, "direct_deposit_total": 0, "fee_burn_total": 0, "latest_retained_root": "d55447e9ae66194069fe6bff4ff113f9ab83d0a1cafa756281bf801f17972935", "legacy_migration_total": 0, "nullifier_count": 165, "output_count": 177, "pool_initialized": true, "retained_root_count": 137, "turnstile_event_count": 0, "value_balance_total": 0, "withdraw_total": 0}, "genesis_hash": "ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad25521aff3ed334da07e150a7233a3e90a9", "passed": true}
(cd tools/pft-binius-shielded-pq && BENCH_CPU=$("$PY_BIN" idle_cpu.py) ./verify.sh) → === ALL 7/7 INDEPENDENT CHECKS PASSED ===
Standalone depth-24 reproduction from the same run: RUST FULL TREE PASS: leaves=16777216 input=16777215 epoch=0->1 wire=138364 B build_and_fill=19936.60 ms prove=24381.03 ms parse_verify_apply=20.47 ms / GO FULL TREE PASS: leaves=16777216 epoch=1 wire=138364 B build_and_fill=21877.39 ms parse_verify=31.02 ms
(cd tools/pft-binius-shielded-pq/go-verifier && "$GO_BIN" test -run TestL1Profile -count=1 -v .) → ok  	postfiat/research/binius-shielded-verifier	30.830s
(cd tools/pft-binius-shielded-pq/go-verifier && "$GO_BIN" test -tags purego -run TestL1Profile -count=1 -v .) → ok  	postfiat/research/binius-shielded-verifier	31.875s
(cd tools/pft-binius-shielded-pq/go-verifier && "$GO_BIN" test -count=1 -tags purego ./...) → ok  	postfiat/research/binius-shielded-verifier	31.541s / ok  	postfiat/research/binius-shielded-verifier/cmd/delivery	0.009s
Default Go unit and delivery tests via verify.sh: ok  	postfiat/research/binius-shielded-verifier	30.752s / ok  	postfiat/research/binius-shielded-verifier/cmd/delivery	0.004s
rustc --edition=2021 --test -O --cfg 'feature="portable"' tools/pft-binius-shielded-pq/rust-prover/src/field.rs -o target/m8/field && target/m8/field → test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s
mkdir -p target/m8/l1-vectors && cargo run --offline --release --locked -p postfiat-privacy-binius --example generate_l1_vectors -- target/m8/l1-vectors && diff -qr tools/pft-binius-shielded-pq/l1-vectors target/m8/l1-vectors → Canonical l1 vectors match byte for byte.
mkdir -p target/m8/rollover-vectors && cargo run --offline --release --locked -p postfiat-privacy-binius --example generate_rollover_vectors -- target/m8/rollover-vectors && diff -qr tools/pft-binius-shielded-pq/rollover-vectors target/m8/rollover-vectors → Canonical rollover vectors match byte for byte.
Regenerated vectors require pre-created output directories. The first attempt failed to open its output; the commands above used empty directories.
PQ_VALIDATOR_BENCH=1 RAYON_NUM_THREADS=1 /usr/bin/time -f "peak RSS %M KiB, elapsed %e s" taskset -c "$core" "$lib" tests::pq_payment_accounting:: --nocapture --test-threads=1 (idle_cpu.py sampled before each run). test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 746 filtered out; finished in 272.50s / peak RSS 619340 KiB, elapsed 272.50 s
bash target/m8/portable-meter.sh (same command using --features postfiat-privacy-binius/portable); bash target/m8/rerun-portable.sh. test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 746 filtered out; finished in 268.90s / peak RSS 619088 KiB, elapsed 268.91 s / test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 746 filtered out; finished in 262.86s / peak RSS 619344 KiB, elapsed 262.86 s

Pinned CPU 17
1x valid=true max of per-cell median/p95/max: 25.37 / 30.96 / 31.49 ms
1x valid=false max of per-cell median/p95/max: 24.35 / 36.58 / 36.60 ms
4x valid=true max of per-cell median/p95/max: 143.58 / 166.13 / 166.23 ms
4x valid=false max of per-cell median/p95/max: 95.31 / 130.45 / 138.67 ms
8 durable commits, median / p95 / max 48.02 / 67.73 / 67.73 ms
peak RSS 619340 KiB, elapsed 272.50 s
Portable CPU 13
1x valid=true max of per-cell median/p95/max: 38.96 / 46.51 / 50.23 ms
1x valid=false max of per-cell median/p95/max: 39.79 / 44.11 / 47.55 ms
4x valid=true max of per-cell median/p95/max: 154.56 / 162.63 / 186.54 ms
4x valid=false max of per-cell median/p95/max: 159.54 / 252.45 / 252.53 ms
8 durable commits, median / p95 / max 45.68 / 66.95 / 66.95 ms
peak RSS 619088 KiB, elapsed 268.91 s
Portable rerun CPU 11
1x valid=true max of per-cell median/p95/max: 39.16 / 48.26 / 54.50 ms
1x valid=false max of per-cell median/p95/max: 40.08 / 53.18 / 55.32 ms
4x valid=true max of per-cell median/p95/max: 200.88 / 234.42 / 235.86 ms
4x valid=false max of per-cell median/p95/max: 150.99 / 164.58 / 166.62 ms
8 durable commits, median / p95 / max 49.30 / 53.58 / 53.58 ms
peak RSS 619344 KiB, elapsed 262.86 s

Earlier milestone tests. $lib, $bin and $gateway are the respective release test executables emitted by Cargo --no-run. The original M7 scripts were copied with scratch paths redirected into target/m8. The report lists their executable filters below.

cargo test --release --locked -p postfiat-types --lib → test result: ok. 260 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
cargo test --release --locked -p postfiat-privacy-binius → test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.12s / test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.37s / test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
cargo test --release --locked -p postfiat-privacy-binius --features portable → test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.23s / test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.69s / test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
cargo test --release --locked -p postfiat-storage --lib -- pq_archive_ batch_archive_append_log_is_visible read_snapshot_remains_bound logical_scan_rejects_forged_receipt_archive retained_history_prune_is_atomic ordered_commit_journal → test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 101 filtered out; finished in 0.19s
cargo test --release --locked -p postfiat-storage --lib -- init_and_read_back ordered_commit_lock_serializes atomic_write_ tampered_state_file legacy_state_file_loads atomic_commit_preserves injected_disk_permission dropped_write_transaction durable_commit_survives dropped_transaction_at_every retained_checkpoint_initialization → test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 94 filtered out; finished in 0.14s
cargo test --release --locked -p postfiat-execution --lib → test result: ok. 291 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.24s
cargo test --release --locked -p postfiat-rpc-sdk --lib account_tx → test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 200 filtered out; finished in 0.00s
"$lib" tests::pq_payment_activation tests::pq_reproves_reserve tests::private_action_ --nocapture --test-threads=2 → test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 731 filtered out; finished in 29.88s
"$gateway" pq_gateway_and_legacy_routes_follow_validator_activation private_action_path_has_no_verification_switch --nocapture --test-threads=1 → test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 3.76s
"$lib" shielded_swap_action_batch_fails_closed_without_consensus_conservation_proof verify_blocks_replays_historical_registry_after_live_key_rotation verify_blocks_rejects_tampered_genesis_faucet_account verify_blocks_rejects_coordinated_genesis_native_supply_rewrite --test-threads=2 → test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 743 filtered out; finished in 0.34s
"$bin" a_shielded_batch_certified_while_a_peer_refused_is_reported_final_and_cached rpc_serve_exposes_private_actions_only_with_the_switch --test-threads=1 → test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 302 filtered out; finished in 3.34s
"$LIB" tests::pq_pool_state:: pq_snapshot_rebuild --nocapture --test-threads=1 → test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 743 filtered out; finished in 0.34s
"$BIN" pq_payment_local_chain_tests --nocapture --test-threads=1 → test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 299 filtered out; finished in 46.36s
"$LIB" account_tx_ global_issued_ archive_bridge_supply:: asset_orchard_egress_policy_ asset_info issuer_assets asset_metrics state_commitment:: replicated_state_root_commits_to_ orchard_frontier_cache_does_not_affect_replicated_state_root asset_orchard_outputs_validate_and_affect_replicated_state_root global_issued_supply_inventory_rejects_duplicate_unknown_and_unsupported_lanes native_bond_custody_is_conserved_and_duplicate_custody_rejected atomic_shielded_batch_rolls_back_earlier_accepted_action shielded_atomic_batch_builder_is_governance_gated_and_canonical legacy_cleartext_shielded_actions_are_historical_replay_only proposal_builder_rejects_duplicate_execution_receipt_ids signed_block_proposals_verify_before_votes tampered_parent_or_state_root_proposal_rejects_votes status_recovers_pending_ordered_commit historical_external_certificate_ signed_snapshot_roundtrip_rejects_tampering_and_preserves_signer_isolation finalized_checkpoint_snapshot_restores_transactional_storage_without_legacy_replay ordered_history_v2_state_root_is_domain_bound_and_activation_gated ordered_history_v2_active_commit_uses_one_database_transaction_without_jsonl active_storage_backends_apply_identical_commits_from_one_snapshot history_prune_writes_checkpoint history_prune_recover_completes_pending lifecycle_checkpoint_tests:: --test-threads=2 → test result: ok. 66 passed; 0 failed; 0 ignored; 0 measured; 681 filtered out; finished in 14.77s
"$BIN" six_validators_admit_round_and_receipt_user_proven_ingress certified_transport_payload_commits_batch_kind private_action_submit_caps_the_entire_json_request_before_params_decoding rpc_serve_rejects_oversized_request_lines_before_parse --test-threads=1 → test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 300 filtered out; finished in 7.99s
"$gateway" --test-threads=1 → test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 12.15s
"$lib" tests::pq_payment_accounting --nocapture --test-threads=1 → test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 746 filtered out; finished in 71.44s
"$lib" governance_pause_rejects_all_shielded_actions_before_mutation route_profile_may_commit_late_but_never_before_scheduled_activation shielded_swap_rejects_wrong_anchor_wrong_domain_tampered_binding_and_oversized_payload wan_devnet_invalid_asset_orchard_swap_proof_is_rejected_and_valid_swap_still_applies --test-threads=2 → test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 743 filtered out; finished in 362.28s
cargo test --release --locked -p postfiat-node --lib -- private_pool_page private_pool_snapshot → test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 744 filtered out; finished in 0.36s
cargo test --release --locked -p postfiat-node --lib -- asset_orchard_ingress_and_disclosed_egress_round_trip_issued_asset → test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 746 filtered out; finished in 0.60s
cargo test --release --locked -p postfiat-consensus-cobalt --lib -- governance_ ratifies_ → test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 69 filtered out; finished in 0.01s
cargo test --release --locked -p postfiat-privacy-binius-wallet → test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s / test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.01s / test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s / test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
The exact suite filters and local log destinations are retained in target/m8/{regress,prior-tests-ready,node-regressions-ready,extra-only,additional,remaining}.sh.
Initial shared-host six-validator Orchard ingress: test result: FAILED. 3 passed; 1 failed; 0 ignored; 0 measured; 300 filtered out; finished in 273.92s. The observed failure reported transport validator service schema unknown on validator-0. Identical four-test selection pinned to sampled CPU 26: test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 300 filtered out; finished in 7.99s
$lib private_pool_page private_pool_snapshot pq_pool_state pq_snapshot_rebuild pq_payment_activation --test-threads=2 → test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 738 filtered out; finished in 0.55s
$lib orchard_action_gate_verifies_applies_and_rejects_duplicate_nullifiers orchard_deposit_batch_locks_transparent_value_and_mints_spendable_note --test-threads=1 → test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 745 filtered out; finished in 82.97s
cargo test --release --locked --offline -p postfiat-node --lib --features pq-wallet pq_wallet_pages_pin_real_disk_history_after_another_commit → test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 748 filtered out; finished in 0.31s
PYTHONPATH=python python3 -m unittest discover -s python/tests -p test_pq_wallet_cli.py → test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 747 filtered out; finished in 210.46s / Ran 1 test in 1119.585s / OK
RAYON_NUM_THREADS=6 node wallet-web/scripts/test-pq-payments.mjs → test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 749 filtered out; finished in 416.45s
RAYON_NUM_THREADS=4 PYTHONPATH=python M6_FIXTURE=$PWD/target/m6/tmp/pq-accounting-181863-1795395520530092791 M6_LIVE_PAYER=pf5264488c726dd329e776239bd0ff4e6bd9206fe8 M6_ISSUE=bbf2fb96437f2e87ff3b470f19aa2455ae226ab810b06b64ab327b0daa71a2d1ad14638a107b7bbd3b0cf3a6f4d56269 python3 target/m6/live-wallet.py → shipping rpc-serve: both assets recovered and withdrawn, Bob finalized balance empty
npm --prefix wallet-web test → # pass 275 / # fail 0
npm --prefix wallet-web run test:custody-browser → # pass 1 / # fail 0
npm --prefix wallet-web run test:public-browser → # pass 2 / # fail 0
(cd wallet-web && node --test src/lib/shielded-navswap.test.js) → # pass 13 / # fail 0
node wallet-proxy/test_private_actions.js → private action routes regression passed
node wallet-proxy/test_legacy_shielded_certification.js → legacy shielded certification switch regression passed
POSTFIAT_NODE_BIN=$CARGO_TARGET_DIR/release/postfiat-node node wallet-proxy/test_legacy_shielded_certification_node_binary.js → legacy shielded certification node-binary test passed (../target-m8/release/postfiat-node; validator-0: shielded_atomic_batch_not_active, validator-1: shielded_atomic_batch_not_active)
cargo test --release --locked --offline -p postfiat-wallet-wasm --lib → test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
Verify reviewer inventory line counts against git show for every pinned blob → 70 source inventory entries match Git blobs at f0a44680e6435fb87a7392f75677a86d3ba5d5fe.
$workspace_lib / $m7_library_tests (copied from target/m7/build.log), same selection: external_lane_export_in_a_block_burns_the_fee_only_after_fee_activation old_committee_release_in_a_block_burns_the_fee_only_after_fee_activation release_above_the_cap_in_a_block_burns_the_fee_only_after_fee_activation --test-threads=1 → test result: FAILED. 0 passed; 3 failed; 0 ignored; 0 measured; 744 filtered out; finished in 0.93s / test result: FAILED. 0 passed; 3 failed; 0 ignored; 0 measured; 744 filtered out; finished in 0.98s
$workspace_lib and $m7_library_tests default_stage_writes_the_single_user_units_unchanged --test-threads=1 → test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 746 filtered out; finished in 0.09s / test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 746 filtered out; finished in 0.11s
mkdocs build --strict --site-dir "$PWD/target/m8/docs-site" → INFO    -  Documentation built in 7.84 seconds
First isolated cold read, candidate verify-blocks on the original replica and signed import → Two cold CLI processes, one original replica and one signed-snapshot import: {"block_count": 30, "state_root": "8b162b3d36fbcc7c708b92167e0e26790cbb4f0447e01200d73307ea9b95893ca218929fb9caec9f1a581c2c790fba95", "tip_hash": "0b48003388ccb344a4b1b040c12085f9c8024a1e7a960d06ee5bab64c3b0ab33c9741def06c41ea8b02c850ea0d34264", "verified": true}
Initial offline workspace and WASM builds lacked cached defmt and serde-wasm-bindgen. The workspace-local Cargo home fetched the locked crates. An initial verifier doctest reported E0463 while dependencies were rebuilding in the shared target. See the final results above.
CARGO_BUILD_JOBS=4 RUST_TEST_THREADS=4 RAYON_NUM_THREADS=4 cargo test --release --workspace --locked --no-fail-fast → error: 26 targets failed: / test result: FAILED. 734 passed; 4 failed; 9 ignored; 0 measured; 0 filtered out; finished in 499.78s
cargo test --release --workspace --locked --offline --doc --no-fail-fast (after the other builds completed) → 26 documentation test targets passed. test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
python3 target/m8/render-audit.py → Seven public pages rendered with working local M8 evidence links. Both links from the source milestone journal resolve.
Workspace outcome across completed harnesses → Workspace harness totals reported: 2451 passed, 4 failed, 53 ignored. / error: 26 targets failed: / Documentation targets with build errors: 25
bash -n tools/pft-binius-shielded-pq/replay-r64.sh, rustfmt --edition 2021 --check on new rehearsal Rust sources, and git diff --check → Shell syntax, new Rust formatting and git whitespace checks passed.
Rehearsal and review audits → Both 1,734-row archive digests, all displayed asset supplies, final hashes, roots and seven PQ receipt IDs match the recorded runs. / 70 source inventory entries match Git blobs at f0a44680e6435fb87a7392f75677a86d3ba5d5fe.

UNMET OR QUALIFIED

1. Old-node replay of the scheduled pre-H prefix failed. No production activation height or release qualification.
2. Independent cryptographic and implementation review remains outstanding. The review package is local. A.3 remains an unreviewed reduction and A.4 a conjecture.
3. Local branch only. Public PR #207 commits, Task Node submission, verification and reward belong to the coordinator.
4. Full workspace release gate remains red. Three external-lane fee assertions expected 1,000 and observed zero. The default signer RPC unit digest differed. Both the candidate and saved M7 test binaries reproduced these four failures. The first workspace run also reported 25 documentation targets with missing crate metadata during overlapping builds. See the isolated documentation rerun above.
5. Timing evidence covers one shared AMD EPYC VM. The portable cold pfUSDC run reached 50.23 ms on CPU 13; a fresh idle_cpu.py selection reached 55.32 ms on CPU 11. Both exceeded the 50 ms single-action target. The rollover boundary uses authenticated frontiers; standalone Rust and Go rebuilt full trees. The R64 snapshot has zero PQ payments and the fresh bridge source is locally attested.
```
