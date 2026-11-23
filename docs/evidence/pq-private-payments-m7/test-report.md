# M7 test report (worker report, re-checked by the coordinator)

```text
DONE
M7 browser private payments for task_39dda44c502d0746e8b7bb72ca7f1305 on feat/peel-binius-shielded-pq.

e9b2976f9 Record the M7 validator receipts and browser evidence for both assets
9031d7c04 Check that the PQ gateway submits a new payment once to each validator
8d84794e2 Test two browser wallets against independent validators and recover both migration steps
48f9b0d0e Prove and review private payments locally and resume Orchard migration after restart
396a61e77 Read both private pools through separate gateway routes and review PQ payment bytes in the wallet

Open the owner-local wallet with the pq-wallet browser command in crates/privacy_binius_wallet/README.md. The Python bridge checks origin and session. WASM encodes/checks addresses and reviews public payment bytes. The native daemon scans, proves, checks intended outputs and signs with the local key path. Only public pages, signed actions and receipt lookups traversed the inspected gateway. The browser compared each result with its trusted validator.

Browser merge on the disposable six-key rehearsal chain, using two independent validator data directories. The harness certified each reserved batch locally and the peer applied its certificate. No production activation. Task Node verification belongs to the coordinator.

Commands below ran in this worktree with source target/m7/env (CARGO_TARGET_DIR=$WORK/target-m7). For wallet-web, Node 22.23.1 came first in PATH. Full local command output is under target/m7.

RAYON_NUM_THREADS=6 node wallet-web/scripts/test-pq-payments.mjs
Orchard-only finalized snapshot: one encrypted output recovered before the first PQ deposit.
Before migration: public 31, PQ 60, Orchard 9 = 100 source-series atoms
After Orchard exit: public 40, PQ 60, Orchard 0 = 100 source-series atoms
After fresh PQ shield: public 31, PQ 69, Orchard 0 = 100 source-series atoms
Historical Orchard-only snapshot returned the original ciphertext after migration.
Public gateway frames verified: {"snapshot":62,"page":14,"submit":10,"receipt":13}
Two isolated browser sessions, changed signer terms, partial sync, disconnected daemon and restart passed. 15 PNGs.
Final child issue: public 31, PQ 69, Orchard 0. Family supply 110.
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 749 filtered out; finished in 429.24s

Committed PNG files in docs/evidence/pq-private-payments-m7/ and their validator receipt IDs
15 PNG files decoded in Chromium; 2161524 bytes total.
thin-pool.png — Measured thin-pool warning for the selected PFT route.
pft-01-alice-shield.png — Alice shields in PFT. block 2 receipt 7959c97714eb1c3ddf7cd5a2e58228f67cca5cd4ee8d5239ff9f7c450bd7e5cc1f974e3e01863578edd25bd522e459d3
pft-02-alice-pay.png — Alice pays Bob in PFT. block 3 receipt 68a14cecf4453fd641437d9b3c2a98933e95092aefc564bb7d437b37130371be729e9b8e292793d596866297bbb6ffd3
pft-03-bob-second-spend.png — Bob spends a received note in PFT. block 4 receipt b939dd9625fa6484a694c2d2fa686c8affe0664def03269722e5da2818d5eb27dc92fc0216bad439132c298dce1297d5
pft-04-bob-exit.png — Bob withdraws the remaining note to his public account in PFT. block 5 receipt 355a9fadbb1844e21b6402d16e05d6ad503e274248c42360cd1038b5f84c96ff94788a5e3e77f60fc962c07dd1f01b0e
pfusdc-01-alice-shield.png — Alice shields in pfUSDC. block 6 receipt eee2944c4544cd206d2e7742e343c12d5fa8bbb7f5c5e4e549fdefe6bd508a37ff4f0380321a59339cbb7a109f0b1b7c
pfusdc-02-alice-pay.png — Alice pays Bob in pfUSDC. block 7 receipt 3faebfa20c7d346e07d44c4dd756aafc76feabe93338afc07edaae60c93ad561feab4524ee04c41712825ecf67a31ded
pfusdc-03-bob-second-spend.png — Bob spends a received note in pfUSDC. block 8 receipt 4527821c45be4d1ad7c22c8c9ede8cfc0b902383395ef81c5ac15882196569c8f2712ec2a6aba3834d2ad61208b76e92
pfusdc-04-bob-exit.png — Bob withdraws the remaining note to his public account in pfUSDC. block 9 receipt 7902d8095a24625386726b3cbea22f237c119f5371dee51f6597147c9d57d88d32543a0929f2cdd315576608fc0f3448
migration-01-review-exit.png — Orchard exit review with public amount, destination and disclosure.
migration-02-await-finality.png — Orchard exit submitted. New shield stays unavailable pending finality.
migration-03-final-exit-after-restart.png — Confirmed Orchard exit after stopping and restarting the bridge and daemon. block 10 receipt 91eee24475cfac7ab526da8b2a1241eab62aed9b6f816055ff40eb0181af7bee811e40192ecf1d3fd3205a03a1a5c915
migration-04-review-shield.png — Separate approval of the fresh PQ shield and its PFT fee.
migration-04b-await-shield-finality.png — Previously submitted PQ shield awaiting finality after another restart; no resubmission.
migration-05-final-shield.png — Final PQ shield and restored private balance. block 11 receipt 54813b802d91f7ab967adad89cbcf278c2266723c769a6a9c50163e3e5bc7de2c54ecda656a7e2f0528be03c11b575e5

PYTHONPATH=python python3 -m unittest discover -s python/tests -p test_pq_wallet_cli.py
8 finalized payments; identical retry; cancellation and restore passed; 89 RPC requests, signed bodies, CLI and daemon output checked for secret leakage
8 pinned proofs: median 27.118 s, max 32.339 s, peak worker RSS 584592 KiB
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 747 filtered out; finished in 230.01s
Ran 1 test in 858.120s
OK
RAYON_NUM_THREADS=4 PYTHONPATH=python python3 target/m6/live-wallet.py (M6_FIXTURE is the prior disposable genesis. M6_LIVE_PAYER and M6_ISSUE are its public funded account and source-series ID.)
shipping rpc-serve: both assets recovered and withdrawn, Bob finalized balance empty

node wallet-proxy/test_private_actions.js; node wallet-proxy/test_legacy_shielded_certification.js; POSTFIAT_NODE_BIN="$CARGO_TARGET_DIR/release/postfiat-node" node wallet-proxy/test_legacy_shielded_certification_node_binary.js
8 shared PQ payment vectors: WASM, gateway and Python agree
private action routes regression passed
legacy shielded certification switch regression passed
legacy shielded certification node-binary test passed (../target-m7/release/postfiat-node; validator-0: shielded_atomic_batch_not_active, validator-1: shielded_atomic_batch_not_active)
npm --prefix wallet-web test; npm --prefix wallet-web run test:custody-browser; npm --prefix wallet-web run test:public-browser; (cd wallet-web && node --test src/lib/shielded-navswap.test.js)
web-final.log  # pass 275 | # fail 0 | # skipped 0
custody-final.log  # pass 1 | # fail 0 | # skipped 0
public-browser.log  # pass 2 | # fail 0 | # skipped 0
orchard-web.log  # pass 13 | # fail 0 | # skipped 0

bash target/m7/run-prior.sh (runs target/m7/regress.sh, prior-tests.sh, node-regressions.sh and additional.sh using the exact named filters in those scripts). Final result lines by suite:
types  test result: ok. 260 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
privacy  test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s
privacy  test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.39s
privacy-portable  test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.35s
privacy-portable  test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.10s
storage  test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 101 filtered out; finished in 0.12s
storage-recovery  test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 94 filtered out; finished in 0.12s
execution  test result: ok. 291 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.01s
rpc  test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 200 filtered out; finished in 0.00s
admission-final  test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 731 filtered out; finished in 33.70s
gateway-final  test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 3.88s
pool-merge  test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 743 filtered out; finished in 0.37s
fleet  test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 298 filtered out; finished in 46.66s
node-regressions  test result: ok. 66 passed; 0 failed; 0 ignored; 0 measured; 681 filtered out; finished in 13.90s
transport  test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 299 filtered out; finished in 6.26s
m1-extra  test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 743 filtered out; finished in 0.30s
m2-extra  test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 301 filtered out; finished in 3.19s
gateway-all  test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.79s
accounting  test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 746 filtered out; finished in 68.52s
affected  test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 743 filtered out; finished in 369.09s
pages  test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 744 filtered out; finished in 0.50s
orchard  test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 746 filtered out; finished in 0.53s
cobalt  test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 69 filtered out; finished in 0.02s
wallet  test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
wallet  test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.52s
wallet  test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
block 8: public PFT 999999974 + live PQ 0 + burned 26 = 1000000000; pfUSDC family 110; wallet notes [0, 0]

Node release lib filters: private_pool_page private_pool_snapshot pq_pool_state pq_snapshot_rebuild pq_payment_activation --test-threads=2
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 738 filtered out; finished in 0.66s
cargo test --release --locked --offline -p postfiat-node --lib --features pq-wallet pq_wallet_pages_pin_real_disk_history_after_another_commit
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 748 filtered out; finished in 0.40s
Node release lib filters: orchard_action_gate_verifies_applies_and_rejects_duplicate_nullifiers orchard_deposit_batch_locks_transparent_value_and_mints_spendable_note
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 745 filtered out; finished in 96.05s
Node release lib filter: wan_devnet_invalid_asset_orchard_swap_proof_is_rejected_and_valid_swap_still_applies
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 746 filtered out; finished in 391.78s
PQ_VALIDATOR_BENCH=1 RAYON_NUM_THREADS=1 taskset -c "$core" "$lib" tests::pq_payment_accounting:: --nocapture --test-threads=1 (core from idle_cpu.py, lib from target/m7/build.log)
Pinned CPU 11
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 746 filtered out; finished in 271.17s
peak RSS 618832 KiB, elapsed 271.17 s

In tools/pft-binius-shielded-pq/go-verifier: "$GO_BIN" test -run TestL1Profile -count=1 -v . with default and -tags purego; "$GO_BIN" test -count=1 ./... with default and -tags purego.
go.log  ok  	postfiat/research/binius-shielded-verifier	31.821s
go-portable.log  ok  	postfiat/research/binius-shielded-verifier	35.743s
go-all.log  ok  	postfiat/research/binius-shielded-verifier	35.380s | ok  	postfiat/research/binius-shielded-verifier/cmd/delivery	0.004s
go-all-portable.log  ok  	postfiat/research/binius-shielded-verifier	32.284s | ok  	postfiat/research/binius-shielded-verifier/cmd/delivery	0.004s
cargo run --release --locked -p postfiat-privacy-binius --example generate_l1_vectors -- target/m7/l1-vectors; likewise generate_rollover_vectors to target/m7/rollover-vectors. diff -qr against each canonical vector directory.
All l1 vectors match byte for byte.
All rollover vectors match byte for byte.
rustc --edition=2021 --test -O --cfg 'feature="portable"' tools/pft-binius-shielded-pq/rust-prover/src/field.rs -o target/m7/portable-field && target/m7/portable-field
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s

(cd tools/pft-binius-shielded-pq && BENCH_CPU=$("$PY_BIN" idle_cpu.py) ./verify.sh). First pinned run:
Replay CPU 21
RUST FULL TREE PASS: leaves=16777216 input=16777215 epoch=0->1 wire=138364 B build_and_fill=18065.92 ms prove=25065.33 ms parse_verify_apply=20.18 ms
GO FULL TREE PASS: leaves=16777216 epoch=1 wire=138364 B build_and_fill=21021.84 ms parse_verify=30.45 ms
=== ALL 7/7 INDEPENDENT CHECKS PASSED ===

(cd target/m7/replay && bash ../../../target/m5/check-meter.sh archive). Compared every archived height, block hash and state root with R64.
1734 target/m3/m5-final.rows
1734 910c744852de4203274bd606859edaccdc27e25181223a1dd07cf2f62da3de048ffef8e427dcf229a00cbb8225121e40 a7b1185e94bf0e224a3944be4aaba896618594823ba2225399dad2148f6611fe527ce6a60a7852f94c8bc154a90dfe56
c28eb87280050e96e996ad5c9e8c2eae12f2fb9774438f3a7b5f1295ef155910  target/m3/m5-baseline.rows
  "verified": true,
  "block_count": 1734,
  "tip_hash": "910c744852de4203274bd606859edaccdc27e25181223a1dd07cf2f62da3de048ffef8e427dcf229a00cbb8225121e40",
  "state_root": "a7b1185e94bf0e224a3944be4aaba896618594823ba2225399dad2148f6611fe527ce6a60a7852f94c8bc154a90dfe56"
Replayed executable equals the browser merge executable.

mkdocs build --site-dir "$PWD/target/m7/docs-site" and local HTML link check:
Verified the generated wallet guide and its link from the documentation index.
mkdocs build --strict --site-dir "$PWD/target/m7/docs-site": Aborted with 1 warnings in strict mode!
WARNING -  Doc file 'quantum/private-payments.md' contains a link '../../tools/pft-binius-shielded-pq/README.md', but the target '../tools/pft-binius-shielded-pq/README.md' is not found among documentation files.
This relative link is also present at base 7a4405924. The user excluded docs/quantum from edits.

An intermediate failed pairing assertion printed its short-lived loopback session token. The owning process exited, the stored diagnostic was redacted, and the updated assertion waits for the fragment to clear without logging it. Screenshots are taken after authentication and checked before publication.
During integration, duplicate forwarding to the primary exhausted its admission limit. The gateway now retains its first acknowledgment. The CLI briefly saw a busy worker during teardown and now retries only that explicit pre-dispatch refusal. A concurrent wallet bundle rebuild interrupted an intermediate browser reload. The merge test now pins its own copy of the built UI.

Delivery test binary from target/m7/wallet.log, pinned with taskset -c from idle_cpu.py:
Delivery scan CPU 11
test scan_a_page_of_nonmatching_outputs_on_one_key ... 4096 nonmatching output trials, 1 key, 0.466158 s, 8787 outputs/s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.52s

Supplemental M5 portable validator measurement under shared-host load. The Rust test passed. Some samples exceeded the 50 ms per-payment target.
bash target/m7/portable-meter.sh
Portable CPU 13
PQ benchmark asset=PFT actions=1 valid=false cold=true n=21 history=1024 median=64.23 p95=73.18 max=73.30 ms
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 746 filtered out; finished in 281.96s
peak RSS 619596 KiB, elapsed 281.97 s
bash target/m7/portable-meter-rerun.sh (fresh idle_cpu.py selection, same executable and settings)
Portable rerun CPU 30
PQ benchmark asset=PFT actions=1 valid=true cold=false n=21 history=1024 median=37.12 p95=54.28 max=54.71 ms
PQ benchmark asset=pfUSDC actions=1 valid=true cold=false n=21 history=1024 median=37.72 p95=38.40 max=45.90 ms
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 746 filtered out; finished in 281.16s
peak RSS 619084 KiB, elapsed 281.17 s
The complete cold/warm, accepted/rejected, one/four-payment samples are in the two logs. The standalone seven checks passed on their first run.
```
