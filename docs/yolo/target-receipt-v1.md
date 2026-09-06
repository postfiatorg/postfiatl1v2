# YOLO target receipt v1

A target receipt records a proved portfolio calculation. It cannot execute orders,
move portfolio capital, establish reserves, or authorize issuance. Existing
transaction fees still apply. This implements Phase 7 of NAVStrategies'
`docs/pre_production/yolo_e2e_tee_proof_runbook.md`.

## Operations and authority

`yolo_target_register_v1` signs immutable run metadata. All fields are required:
registrant, permitted submitter, program SHA-256 and SP1 verification key,
methodology and parameter commitments, collection manifest, series/underlier,
epoch, expected prior state, replay identifier and future activation height.
The registration ID is SHA3-384 over the versioned domain and length-prefixed
fixed-order JSON body. It binds every field.

Registrations are namespaced by their signer. Consumers must independently pin
the reviewed registration ID, registrant, program/key and chain identity. A
registration does not declare a provider or portfolio to be network-approved.
The registrant cannot register the same series/epoch or replay ID twice.

`yolo_target_submit_v1` carries the registration ID, submitter/replay identity,
complete redundant target fields, proof calldata and exactly 408 public bytes.
Consensus compares every redundant field, every registered expectation and the
permitted submitter, then verifies Groth16 with the registered key. The proof
bound is 4,096 bytes; unknown ABI versions, malformed lengths, wrong keys,
modified public values, early submissions, duplicates and conflicts reject.
Only one receipt may consume a registered run.

The expected prior state is supplied explicitly. Targets do not trade, so a
later actual position/cash state is never inferred from an earlier target.
The proof commits to the collection manifest, with no separate chain ID in the
locked ABI. Transaction signatures bind the destination chain; do not claim
that the same mathematical proof is globally unusable on another chain.

## Activation and persistence

The feature defaults to disabled. Existing signed Cobalt governance schedules
`yolo_target_activation_height` strictly after the amendment block. Once active,
run registrations must themselves name a future activation height. No production
height, principal, key, source descriptor or devnet has been selected here.

The ledger stores separate `yolo_target_registrations` and
`yolo_target_receipts`. Empty fields are omitted from serialization and state
commitment, preserving legacy empty-state roots. Populated records are sorted by
registration ID and fully committed. The issued-supply inventories explicitly
exclude them because they contain no asset balances.

A receipt binds transaction ID and inclusion height. Canonical transaction
finality evidence supplies the containing block/certificate and current tip;
execution does not assert future finality or include a circular self block hash.

## CLI and public interface

The Python entrypoint prepares unsigned operations and queries public receipts.
Every output is create-only. It accepts public proof calldata, never a private
witness. The existing certified asset-operation tooling signs reviewed JSON.

```bash
PYTHONPATH=python python3 -m postfiat_rpc.yolo_target --help
PYTHONPATH=python python3 -m postfiat_rpc.yolo_target prepare-register \
  --config /absolute/reviewed-run.json --output /absolute/new-register.json
PYTHONPATH=python python3 -m postfiat_rpc.yolo_target prepare-submit \
  --config /absolute/reviewed-run.json --public-values /absolute/public-values.bin \
  --proof-calldata /absolute/proof-calldata.bin --output /absolute/new-submit.json
```

Node RPC method `yolo_target_receipt` accepts `registration_id`. Its public
response includes chain/genesis identity, registration, receipt and canonical
transaction finality; absent receipts have no claimed finality. The method is
available through the existing read-only RPC interface.

```bash
PYTHONPATH=python python3 -m postfiat_rpc.yolo_target query --help
PYTHONPATH=python python3 -m postfiat_rpc.yolo_target html \
  --receipt /absolute/queried-receipt.json --output /absolute/new-receipt.html
```

Queries require independent expected chain/genesis, program/key and registration
identity. The standalone HTML viewer escapes node text, uses no scripts or
network dependencies, and labels the response as node-reported evidence.

## Qualification

The synthetic proof is the real Phase 3 Groth16 artifact, not a verification
stub. Tests exercise the transaction signature, consensus verifier and storage
boundaries. Four local validator stores verify four signed votes per block,
apply the same five certified rounds, reject the early/duplicate submissions,
confirm the valid receipt and replay to the same state root after reopening.
This qualifies local certificate/state processing; it does not claim a live
network deployment or AWS/Nitro hardware run.

```bash
cargo test --locked -p postfiat-execution yolo_
cargo test --locked -p postfiat-node --lib yolo_
PYTHONPATH=python python3 -m unittest python/tests/test_yolo_target.py
```

The four-validator test can retain a public JSON report at an explicitly new
`YOLO_QUALIFICATION_REPORT` path. `YOLO_QUALIFICATION_KEEP_DIRECTORY=1` retains
its isolated synthetic node directories for a subsequent process-level CLI
check; these contain test keys and must not be published. Normal tests remove
them. The [qualification summary](evidence/target-receipt-v1-qualification-20260905.json)
and [compressed public validator report](evidence/target-receipt-v1-local-20260905.json.gz)
are durable acceptance evidence. Implementation source is `f8edad6b`; additional
rejection tests are `978612ac`. Six execution tests, three node tests, five
Python CLI tests and the existing asset conservation/replay test pass. The real
node binary also prepared, queried and rendered the finalized synthetic receipt
through the Python CLI. No retained test private keys are in these artifacts.

The earlier AWS-compatible guest qualification used source `459a6d3d`, ELF SHA-256
`9aa40d86cc478a01fd7c6b2a63308188fb7e671079bd3321b405e7ac1a275cff`, key
`0x0008ed5f9307fc00d9e60fbcd0e463e10d1bb3143ce3f73bcb2c807aa0670968`.
It adds bounded decoding for real AWS NSM CBOR documents. The original proof
fixture remains a regression case. A second real Groth16 proof of synthetic
inputs uses the new guest; it passed the same five certified rounds, four-store
finality/replay and Python CLI/HTML qualification. See the
[new summary](evidence/target-receipt-v1-aws-compatible-summary-20260905.json)
and [public validator report](evidence/target-receipt-v1-aws-compatible-20260905.json.gz).
Both fixtures contain only public proof bytes and commitments. Real AWS
attestation is qualified separately in the 44-case SP1 attestation corpus;
do not describe this synthetic proof as a live collection proof.

```bash
cargo test --locked -p postfiat-node --lib yolo_aws_compatible_guest_four_validator_finality_restart_and_replay
```

Synthetic AWS Nitro/KMS and native private-prover qualification are complete;
temporary resources are removed. NAV's Phase 5 and Phase 6 journals contain
exact hardware evidence. Owner-approved live inputs, external deployment,
real collection/proving, external finality and authorized live replay remain.

For a newly generated, independently verified public proof, the same local
four-validator qualification can run without replacing either regression
fixture. Supply a JSON fixture with `program.programVkey`, `publicValuesHex`
and `proofCalldataHex` (maximum 128 KiB). It must contain public artifacts only.

```bash
YOLO_QUALIFICATION_PROOF_FIXTURE=/absolute/new-public-proof.json \
YOLO_QUALIFICATION_REPORT=/absolute/new-validator-report.json \
YOLO_QUALIFICATION_KEEP_DIRECTORY=1 \
cargo test --locked -p postfiat-node --lib \
  yolo_supplied_public_proof_four_validator_finality_restart_and_replay -- --ignored
```

This opt-in test checks signed registration, early/duplicate rejection,
four-validator certification, receipt persistence and restart/replay using the
supplied proof. It does not activate an external network.

The supplied-proof harness at `aaad0c48` passed using the real non-debug Nitro
native proof on September 5, 2026. Its 122.64-second run certified five rounds
with four votes each, rejected early/duplicate submissions, confirmed the proof
receipt and replayed all four reopened stores. The actual Python CLI prepared
the same certified operation, queried the receipt with independent pins and
generated its standalone HTML viewer. See the
[Nitro receipt summary](evidence/target-receipt-v1-nitro-native-summary-20260905.json)
and [public validator report](evidence/target-receipt-v1-nitro-native-20260905.json.gz).
NAV retains the hardware proof, measurements, independent verification,
six tamper negatives, encrypted diagnostics, HTML and AWS cleanup in
`docs/pre_production/evidence/yolo_phase6_nitro_proof_20260905.json` and its linked
artifacts. The proof inputs and embedded collection attestations are synthetic;
the prover hardware and Groth16 proof are real. This is local finality
qualification, not an external network receipt or a live Schwab collection.

The September 5–6 real-data continuation uses the optimized guest at
`d3aeb780bff327bc2d9734226770aba7433ba5d9`, ELF
`7a348b7484a1f7933fa79f3b77a1855174885d09c76da1abb3bd715445503142`, key
`0x002e0440a99b9a755ff45d2837b2ad5f2779437f94c62db4142b306ba5e2846d`.
Thirty-nine Rust tests, fourteen guest vectors and thirteen malformed guest
witnesses pass. The same retained MU witness falls from 4.09 billion to 477.84
million instructions, preserving every public byte except program identity.
The actual MU replay passed independent Python/native checks, but its private
proof did not finish before the approved 02:00 UTC cutoff; NVDA had not started.
AWS is closed out. No real-data proof receipt is claimed. NAV retains the
incomplete checkpoint at
`docs/pre_production/evidence/yolo_closed_market_target_checkpoint_20260906.json`.
A rebuilt NAV package refreshes KMS sessions for a proposed longer run; that
package and resource window are not a completed hardware qualification.
