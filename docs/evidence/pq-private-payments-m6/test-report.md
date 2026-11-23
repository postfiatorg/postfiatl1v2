# M6 test report (worker report, re-checked by the coordinator)

**Historical evidence.** The original milestone measurements and artifact inventory tested L1 profile 1, including the public PFT payer for issued spends. Issued payments and exits now use profile 2.

```text
DONE
M6 implements local PFSH02 proving and scanning, encrypted recovery, finalized public pages, and the Python payment CLI.
Local branch feat/peel-binius-shielded-pq. Task Node evidence and reward are handled by the coordinator.
140673693 Record the private wallet recovery and rollover test results
751e56f3c Recover and spend private PFT and pfUSDC from the Python wallet CLI
192a9d538 Serve finalized PQ payments and epoch frontiers in bounded public pages
f2746a431 Derive network-bound wallet addresses and check recovery in Rust, Go and Python
Checkpoint identity comes from an operator-trusted validator, optionally separate from the archival page server. Public page coverage and epoch roots are recomputed in the wallet.
CARGO_TARGET_DIR=$WORK/target-m6 throughout. Commands below ran from the worktree with source target/m6/env.
Pinned measurements are from this Linux AMD EPYC host. A laptop measurement remains outstanding.
The merge chain begins with a checked depth-24 prefix four leaves short of capacity. Its synchronous test transport uses node execution, production pair reads, and six locally held validator keys. Each post-birthday block certificate was independently verified. The separate disk-page test reopens normal finalized state.

PYTHONPATH=python python3 -m unittest discover -s python/tests -p test_pq_wallet_cli.py
wallet benchmark CPU 5
shield PFT 100: accepted; prove 26.666 s, worker peak 584332 KiB
pay PFT 30: accepted; prove 25.161 s, worker peak 584332 KiB
shield bbf2fb96 80: accepted; prove 25.342 s, worker peak 584336 KiB
pay bbf2fb96 25: accepted; prove 28.222 s, worker peak 584340 KiB
restored at height 4: 30 PFT, 25 pfUSDC; 1 page, 8 outputs x 2 keys in 0.002635 s (6073 trials/s), full sync with retry 0.316 s
certified block 5, 000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000 read epoch 0 output epoch 1, PFT supply 1000000000, pfUSDC family 110
pay PFT 10: accepted; prove 25.089 s, worker peak 584340 KiB
unshield PFT 18: accepted; prove 25.508 s, worker peak 584336 KiB
pay bbf2fb96 5: accepted; prove 27.574 s, worker peak 584608 KiB
certified block 8, bbf2fb96437f2e87ff3b470f19aa2455ae226ab810b06b64ab327b0daa71a2d1ad14638a107b7bbd3b0cf3a6f4d56269 read epoch 0 output epoch 0, PFT supply 1000000000, pfUSDC family 110
unshield bbf2fb96 20: accepted; prove 26.059 s, worker peak 584344 KiB
8 finalized payments; identical retry; cancellation and restore passed; 89 RPC requests, signed bodies, CLI and daemon output checked for secret leakage
8 pinned proofs: median 25.783 s, max 28.222 s, peak worker RSS 584608 KiB
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 748 filtered out; finished in 219.00s
OK

Additional end-to-end run: RAYON_NUM_THREADS=4 PYTHONPATH=python python3 target/m6/live-wallet.py with the disposable fixture, public issue and payer in its M6 environment. The shipping rpc-serve admitted each CLI action. apply-shield-batch finalized the reserved bytes, and the CLI read the receipt and synced from disk.
shipping rpc-serve: PFT shield 90 queued, archived and accepted
shipping rpc-serve: PFT pay 24 queued, archived and accepted
shipping rpc-serve: Bob recovered 24 PFT atoms from finalized disk pages
shipping rpc-serve: bbf2fb96 shield 80 queued, archived and accepted
shipping rpc-serve: bbf2fb96 pay 25 queued, archived and accepted
shipping rpc-serve: PFT unshield 23 queued, archived and accepted
shipping rpc-serve: bbf2fb96 unshield 25 queued, archived and accepted
shipping rpc-serve: both assets recovered and withdrawn, Bob finalized balance empty

cargo test --release --locked --offline -p postfiat-privacy-binius-wallet (vectors and local witnesses). Also ran the delivery test executable with taskset -c from idle_cpu.py.
wallet scan CPU 27
test scan_a_page_of_nonmatching_outputs_on_one_key ... 4096 nonmatching output trials, 1 key, 0.612928 s, 6683 outputs/s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.66s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

cargo test --release --offline --locked -p postfiat-privacy-binius-wallet --lib
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

cargo test --release --locked --offline -p postfiat-node --lib --features pq-wallet pq_wallet_pages_pin_real_disk_history_after_another_commit
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 748 filtered out; finished in 0.31s

Release node lib test executable with private_pool_page private_pool_snapshot pq_pool_state pq_snapshot_rebuild pq_payment_activation --test-threads=2
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 738 filtered out; finished in 1.14s

Release node lib test executable with pq_payment_accounting --nocapture --test-threads=1. Each of the eight blocks reconciled public PFT, live PQ PFT, burned fees and both pfUSDC issues.
block 8: public PFT 999999974 + live PQ 0 + burned 26 = 1000000000; pfUSDC family 110; wallet notes [0, 0]
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 746 filtered out; finished in 76.75s

cargo test --release --offline --locked -p postfiat-node --bin postfiat-node pq_payment_local_chain_tests -- --test-threads=1
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 298 filtered out; finished in 28.41s

cargo test --offline --locked --release -p postfiat-types --lib
test result: ok. 260 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s

cargo test --offline --locked --release -p postfiat-privacy-binius
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.12s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.40s

cargo test --offline --locked --release -p postfiat-privacy-binius --features portable
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.25s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.88s

Go 1.25.1 at $GO_BIN in tools/pft-binius-shielded-pq/go-verifier

test -run TestL1Profile -count=1 -v .
ok  	postfiat/research/binius-shielded-verifier	30.621s

test -tags purego -run TestL1Profile -count=1 -v .
ok  	postfiat/research/binius-shielded-verifier	30.169s

test -count=1 -v ./cmd/delivery
ok  	postfiat/research/binius-shielded-verifier/cmd/delivery	0.004s

cd wallet-proxy && node test_private_actions.js && node test_legacy_shielded_certification.js
private action routes regression passed
legacy shielded certification switch regression passed

cd tools/pft-binius-shielded-pq && BENCH_CPU=$("$PY_BIN" idle_cpu.py) ./verify.sh. First run missed the unchanged <50 ms Go gate on the shared host.
Replay CPU 15
GO VERIFY PASS (pclmul=true): payments/tx_mint.bin (143337 B <= 149130 B budget) in 52.37 ms

Same command, newly sampled core
Replay CPU 12
GO VERIFY PASS (pclmul=true): payments/tx_mint.bin (143337 B <= 149130 B budget) in 36.41 ms
RUST FULL TREE PASS: leaves=16777216 input=16777215 epoch=0->1 wire=138364 B build_and_fill=18721.52 ms prove=32760.55 ms parse_verify_apply=20.74 ms
GO FULL TREE PASS: leaves=16777216 epoch=1 wire=138364 B build_and_fill=25007.50 ms parse_verify=30.41 ms
=== ALL 7/7 INDEPENDENT CHECKS PASSED ===

cargo build --release --locked --offline -p postfiat-node --bin postfiat-node; (cd target/m6/replay && bash ../../../target/m5/check-meter.sh archive). cmp succeeded for every height/hash/root row against R64.
1734 target/m3/m5-final.rows
1734 910c744852de4203274bd606859edaccdc27e25181223a1dd07cf2f62da3de048ffef8e427dcf229a00cbb8225121e40 a7b1185e94bf0e224a3944be4aaba896618594823ba2225399dad2148f6611fe527ce6a60a7852f94c8bc154a90dfe56
c28eb87280050e96e996ad5c9e8c2eae12f2fb9774438f3a7b5f1295ef155910  target/m3/m5-baseline.rows
c28eb87280050e96e996ad5c9e8c2eae12f2fb9774438f3a7b5f1295ef155910  target/m3/m5-final.rows
  "verified": true,
  "block_count": 1734,
  "tip_hash": "910c744852de4203274bd606859edaccdc27e25181223a1dd07cf2f62da3de048ffef8e427dcf229a00cbb8225121e40",
  "state_root": "a7b1185e94bf0e224a3944be4aaba896618594823ba2225399dad2148f6611fe527ce6a60a7852f94c8bc154a90dfe56"

Earlier 0bb4483f5 results reused as requested. I checked the following saved command output in target/m6; these timings are from the prior baseline.

bash target/m6/regress.sh
execution: test result: ok. 291 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.24s
storage: test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 101 filtered out; finished in 0.13s
storage-recovery: test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 94 filtered out; finished in 0.11s
rpc: test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 200 filtered out; finished in 0.00s

bash target/m6/prior-tests.sh and bash target/m6/additional.sh
admission-final: test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 731 filtered out; finished in 29.92s
gateway-all: test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 12.16s
node-regressions: test result: ok. 66 passed; 0 failed; 0 ignored; 0 measured; 681 filtered out; finished in 12.99s
transport: test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 299 filtered out; finished in 6.65s
m1-extra: test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 743 filtered out; finished in 0.25s
m2-extra: test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 301 filtered out; finished in 3.08s
affected: test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 743 filtered out; finished in 362.64s

cargo test --offline --locked --release -p postfiat-consensus-cobalt --lib -- governance_ ratifies_
cobalt: test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 69 filtered out; finished in 0.03s

Saved baseline portable field test (rustc --test -O)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s

Saved baseline Go test -tags purego -count=1 ./...
ok  	postfiat/research/binius-shielded-verifier	32.462s

Saved baseline generate_l1_vectors and generate_rollover_vectors; diff -qr with checked-in vectors
All L1 vector bytes match.
All rollover vector bytes match.

During development one prover job ended without a reply during a rebuild, and a separate mixed-revision run failed on a missing action hash. The daemon now spawns workers from its running executable and the merge test keeps a private binary copy. Final results above use the matched revisions.
```
