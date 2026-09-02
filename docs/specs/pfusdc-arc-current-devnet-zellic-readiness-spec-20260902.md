# pfUSDC Arc Current-Devnet Qualification and Zellic Review-Readiness Specification

**Date:** 2026-09-02

**Status:** Execution specification; no live deployment or grant submission is
authorized merely by this document.

**Priority:** P0

**Target:** qualify the existing pfUSDC-on-Arc implementation against the
currently deployed six-validator PFTL devnet, prove the protocol primitives
work on current source, and publish a complete packet that can be sent to
Zellic for technical review and possible co-sponsorship of the Arc grant.

## 1. Governing documents

This specification is a qualification overlay. It does not replace or
reinterpret:

- `docs/specs/pfusdc-arc-tier4-spec-20260828.md`;
- `docs/specs/pfusdc-arc-mvp-testnet-spec-20260828.md`; or
- `docs/business/pfusdc-arc-grant-proposal-20260828-v3.md`.

The MVP definition remains one exact proof-verified round trip:

```text
1.000000 Arc testnet USDC
    -> Arc vault deposit
    -> Arc finality + receipt proof
    -> pfUSDC mint on current PFTL devnet
    -> accepted burn on current PFTL devnet
    -> PFTL finality proof
    -> Arc vault release
    -> exact 1.000000 Arc testnet USDC returned
```

Both directions MUST use the configured proof verifiers. An observer,
attestation committee, threshold withdrawal signer, mock verifier, or downgrade
path does not satisfy this specification.

## 2. Required outcomes

This work has three outcomes, all required.

### O1 — Current-devnet qualification

The Arc branch is built from current `main`, qualified on an exact six-node
clone of the currently deployed devnet state, then—under a separate explicit
operator authorization—deployed to the current six-validator devnet and used
for one fresh Arc testnet round trip.

Success requires accepted receipts and exact balance/state changes. Compiling,
unit tests, block convergence, transaction inclusion, or an archived proof
alone is not success.

### O2 — Zellic review-ready grant packet

Produce a public, hash-manifested packet that lets Zellic:

1. reproduce the current build;
2. identify every security boundary and privileged action;
3. verify the testnet deployments and current-devnet receipts;
4. inspect positive and adversarial proof evidence;
5. see all unresolved findings without marketing language hiding them;
6. scope a contract and zkVM audit; and
7. decide independently whether to review or co-sponsor the Arc proposal.

“Review-ready” does not mean “Zellic-approved.” No document may claim Zellic
review, audit, endorsement, or co-sponsorship until Zellic provides it.

### O3 — Primitive-level proof

Each primitive in Section 7 has its own positive and negative acceptance
evidence. A successful end-to-end transaction cannot conceal a broken or
unproven component.

## 3. Current baseline

### 3.1 PFTL devnet

The operational authority is `docs/status/chain-state-current.md`. Its latest
recorded deployment is the six-validator controlled devnet with transactional
storage active at height 931. Before qualification, the fleet MUST be freshly
probed; the dated height-931 record is not a claim about the live state at
execution time.

The qualification packet MUST record:

- chain ID and genesis hash;
- height, tip, state root, registry root, and trust root;
- release identifier, source revision, and node binary SHA-256 on all six
  validators;
- active protocol and storage versions;
- validator/RPC service health and mempool count; and
- the observation interval.

### 3.2 Current integration branch

The implementation is carried by PR #37,
`integrate/arc-tier4-current-v2-20260901`, based on current `main`.
`docs/evidence/arc-mvp-20260828/current-main-integration-20260901.md`
records the initial code-level merge qualification. It explicitly does not
qualify a live deployment.

At specification time:

- Arc conformance tests pass;
- current Arc guest tests pass;
- types, execution, focused node, Clippy, and public-tree gates have passed;
- the Arc Solidity job passes in GitHub;
- some exhaustive PR jobs are still running; and
- no current-v2 Arc witness has produced a current-v2 Groth16 proof.

### 3.3 Historical evidence

The August evidence proves that the design was implemented and exercised:

- G0 Arc conformance passed;
- live Arc testnet USDC, precompile, receipt, certificate, and deployment
  observations exist;
- the historical Arc ingress guest executed in 2,376,633 instructions;
- its Groth16 proof was generated in 54.420955902 A100 GPU-seconds and verified;
- the PFTL execution verifier accepted the real proof and rejected mutations;
- local six-validator transport and replay tests passed; and
- the archived v1 proof was reproduced on an H100 as a toolchain/hardware
  control.

Those artifacts remain valid for their frozen historical source, ELF, vkey,
route, contracts, and witness. They do not qualify current-v2 source or the
current live devnet.

### 3.4 Known current-v2 blocker

The current-v2 Arc guest requires an authenticated post-block validator
registry proof on every ingress, including a no-change block. This closes the
unsafe possibility that a relayer asserts the next validator set.

The public Arc testnet RPC currently returns `-32601 method not supported`
for `eth_getProof`. The archived v1 deposit witness therefore cannot satisfy
current-v2 bounds, and a proof captured at another block cannot be substituted
because the state roots differ.

This is the first live primitive gate. It MUST be solved or the current-v2
qualification stops. Acceptable sources are:

1. an Arc testnet archive RPC that returns standard EIP-1186 proofs at the exact
   deposit block;
2. a locally operated proof-capable Arc testnet archive node; or
3. a narrowly scoped Arc-node proof-generation implementation used by the
   capture tool.

The selected source is infrastructure, not a new bridge trust authority. The
guest MUST independently verify every account and storage proof against the
finalized header's exact `stateRoot`.

Unacceptable workarounds:

- revert to witness v1;
- accept an RPC `eth_call` result as authenticated state;
- let the relayer assert the output validator commitment;
- reuse the no-change proof from a different block;
- pin the current set forever; or
- remove the registry proof requirement to meet the grant deadline.

## 4. Artifact freeze

Create a run root:

```text
docs/evidence/arc-current-devnet-qualification-<UTC>/
```

Generated proofs, private node data, keys, raw signed transactions, and
unredacted logs remain outside Git. The public packet contains only
redaction-safe evidence and hashes.

Before any live transaction, freeze:

| Artifact | Required identity |
| --- | --- |
| PFTL source | exact commit |
| Arc-node source | exact commit |
| SP1 SDK/circuit | exact versions |
| Rust/Foundry/Solidity | exact versions and optimizer settings |
| Arc ingress ELF | SHA-256 |
| Arc ingress vkey | bytes32 |
| PFTL egress ELF | SHA-256 |
| PFTL egress vkey | bytes32 |
| SP1 verifier/gateway | runtime code hashes and route selector |
| Arc vault/anchor/finality verifier | addresses, constructor bindings, runtime code hashes |
| PFTL route | canonical profile bytes, profile hash, route binding, epoch, activation height |
| Arc checkpoint | height, block hash, validator-set commitment |
| PFTL checkpoint | height, block ID, state root, committee root |
| Test identities | public addresses only |
| Input amount | exactly 1,000,000 atoms |

The final current-v2 Arc ingress identity expected from the existing candidate
is:

- ELF SHA-256
  `830634e6bf67333315bda7874ed1155dfc45c7e3b1ebc5e97a1fd34f9af7f130`;
- vkey
  `0x00d8e761fd2e0034388813ad8febd38beb3b271a83575a05834168450ea814c5`.

These values are candidates until rebuilt from the frozen qualification commit.
A mismatch stops the run and requires all route/proof bindings to be regenerated.

## 5. Phase A — Current-source qualification

### A1. Reconcile source and CI

- [ ] Rebase or merge the final approved `main` into PR #37.
- [ ] Require every mandatory GitHub check to finish green.
- [ ] Resolve any main/branch disagreement without changing protocol semantics.
- [ ] Record the PR URL, head commit, merge base, complete check list, and check
      completion times.
- [ ] Keep the PR draft until Sections 5–9 pass.

### A2. Rebuild proof identities

From the exact frozen source:

- [ ] rebuild `programs/pfusdc-arc-ingress`;
- [ ] rebuild `programs/pfusdc-egress`;
- [ ] derive both vkeys from those exact ELFs;
- [ ] byte-compare two clean builds;
- [ ] record ELF, vkey, lockfile, compiler, SP1 circuit, and container hashes;
- [ ] reject any checked route or deployment that pins another vkey.

No proof generated for an earlier ELF may qualify the current route.

### A3. Required code gates

Run and preserve machine-readable results for:

```bash
cargo test -p arc-conformance --locked
cargo test --manifest-path programs/pfusdc-arc-ingress/Cargo.toml --locked
cargo test -p postfiat-types --locked
cargo test -p postfiat-execution --locked
cargo check -p postfiat-node --all-targets --locked
cargo clippy -p arc-conformance -p postfiat-types -p postfiat-execution   -p postfiat-node --all-targets --locked -- -D warnings
cargo clippy --manifest-path tools/pfusdc-tier4-prover/Cargo.toml   --no-default-features --locked -- -D warnings
cargo fmt --all -- --check
forge test --root crates/arc-sp1-contracts -vv
forge test --root crates/ethereum-contracts   --match-contract PFUSDCTier4Test -vv
scripts/test-public-secret-scan
scripts/public-secret-scan
```

Also run the complete workspace and non-Rust release gates required by
`CONTRIBUTING.md`. A documented unrelated baseline failure may remain only if
it is reproduced on `main`, absent from the Arc diff, and accepted explicitly
by the release reviewer. No Arc, proof, bridge, execution, storage, replay, or
contract failure may be waived.

**Gate A:** exact current source builds reproducibly, all owning tests pass, and
the program/contract/route identities agree.

## 6. Phase B — Exact current-devnet rehearsal and deployment

### B1. Fresh fleet observation

Probe all six validators and write `fleet-before.json`. Stop if:

- validators disagree on height, tip, state, registry, or trust root;
- a service is inactive;
- a mempool is unexpectedly non-empty;
- the source snapshot cannot be exported and verified;
- signer isolation cannot be demonstrated; or
- the deployed release differs across validators.

### B2. Deployment-exact six-node clone

Using one authenticated snapshot of the current devnet:

- [ ] construct six isolated validator clones;
- [ ] give each clone only its own signing key;
- [ ] start the exact signed service topology, including transport and RPC
      processes against the intended storage ownership model;
- [ ] deploy the Arc candidate binaries one node at a time;
- [ ] require rejoin and identical tip/root after each node;
- [ ] activate a candidate Arc route with the rebuilt vkeys;
- [ ] submit a valid proof-backed ingress lifecycle;
- [ ] verify accepted propose, finalize, and claim receipts;
- [ ] verify the exact pfUSDC balance delta;
- [ ] replay from the pre-activation snapshot to the identical terminal root;
- [ ] restart all six nodes and re-verify state, route, finality checkpoint,
      proof nullifier, and deposit replay protection; and
- [ ] roll back the clone to the original binary and data using the actual
      operator procedure.

The clone MUST reproduce the live multi-process storage topology. A transport-
only clone is not deployment-exact.

### B3. Live deployment authorization boundary

Live current-devnet mutation requires a separate written operator
authorization naming:

- candidate commit and binary hashes;
- six target validators;
- route profile and activation height;
- Arc and PFTL checkpoints;
- exact test amount;
- rollback release and snapshots;
- stop conditions; and
- the authorized execution window.

Without that authorization, stop after B2 and deliver a review packet labeled
“clone-qualified; live current-devnet run pending.”

### B4. Live rolling deployment

When authorized:

1. take and verify pre-deployment snapshots;
2. stage the old and new signed releases;
3. deploy one validator at a time;
4. after every node, require six-node convergence and service health;
5. activate the exact versioned Arc route through the governed path;
6. verify every node reports identical route bytes, vkeys, checkpoints, and
   activation height; and
7. write a signed deployment receipt.

**Gate B:** the exact candidate is deployed on all six current-devnet validators
with an accepted governed route and a proven rollback path.

## 7. Phase C — Primitive qualification matrix

Every row requires positive evidence, at least one targeted negative, and an
owner code/test reference.

| ID | Primitive | Positive acceptance | Required negative |
| --- | --- | --- | --- |
| P1 | Arc network and USDC | chain 5042002; token code/decimals; exact approve/pull; gas measured | wrong chain/token and non-exact delta reject |
| P2 | Arc cryptographic precompiles | SHA-256 and BN254 add/mul/pairing match vectors | mutated vectors fail |
| P3 | Arc certificate | canonical commit preimage, Ed25519 signatures, distinct signers, >2/3 power | forged, duplicate, unknown, and sub-quorum signers fail |
| P4 | Certificate/header binding | certificate authenticates exact EVM block hash/header | changed hash, height, round, or header fails |
| P5 | Receipt MPT | typed receipt opens under exact `receiptsRoot`; status is 1 | changed receipt, index, node, root, type, or failed status rejects |
| P6 | Deposit semantics | exact vault log reproduces route, deposit ID, recipient hash, nonce, token, and amount | every field mutation rejects |
| P7 | Registry state proof | proxy account, implementation account, ERC-1967 slot, registry slots, code hashes, and output set verify under exact `stateRoot` | wrong root/node/slot/code/implementation/set rejects |
| P8 | Validator transition | H-1 signing set and H post-state set are proven; no-change and changed-set fixtures work; zero-power entries excluded | asserted, stale, reordered, duplicate, zero/overflow, or unproven rotation rejects |
| P9 | Current-v2 SP1 ingress | native and zkVM execution agree; H100 Groth16 proof verifies; public values byte-match | proof, witness, vkey, public-value, and route mutations fail |
| P10 | PFTL proof admission | current execution verifies exact proof/vkey/profile and accepted receipt | wrong proof/vkey/profile/route and oversized inputs reject without mutation |
| P11 | Arc checkpoint continuity | ingress advances the stored height/hash/set commitment monotonically | replay, stale height, wrong prior set, and conflicting block reject |
| P12 | Deposit lifecycle | propose/finalize/claim all accepted and credit exactly 1,000,000 pfUSDC atoms | duplicate deposit/evidence/proof cannot credit twice |
| P13 | PFTL exit commitment | accepted burn creates the exact exit leaf/root and accepted receipt binding | rejected receipt or changed packet cannot create/prove an exit |
| P14 | PFTL finality proof | current egress guest verifies proposal/prepare/precommit, quorum, ancestry, exit path, and accepted receipt | insufficient/duplicate signers, wrong ancestry/root/path/receipt fail |
| P15 | Arc egress verification | deployed SP1 route and `PFTLFinalityVerifierV1` accept current proof | wrong selector, vkey, public values, checkpoint, vault, amount, or recipient fail |
| P16 | Vault release | exact recipient receives exactly 1,000,000 atoms and both withdrawal/nullifier replay guards consume atomically | replay, redirect, reentrancy, pause, and malformed token behavior fail |
| P17 | Conservation | `issued == Σcounted − Σredeemed` and vault/counting balances reconcile before, during, and after | any one-atom mismatch fails the gate |
| P18 | Durability | six nodes restart, replay, snapshot/export/import, and converge to one terminal root | tampered snapshot/history/proof rejects without durable mutation |

The matrix is not complete if a row points only to historical v1 evidence.
Historical evidence may be attached as lineage; current-v2 evidence is required
for P7–P18.

**Gate C:** P1–P18 pass with current source and independently checkable evidence.

## 8. Phase D — Fresh Arc testnet/current-devnet round trip

Use a dedicated testnet-only EOA funded with Arc native gas and testnet USDC.
The private key remains outside the repository and logs. The address is recorded
in the run receipt, not hardcoded into protocol source.

### D1. Preflight

- [ ] Arc RPC reports chain 5042002.
- [ ] EOA has at least 2.000000 Arc testnet USDC plus bounded gas.
- [ ] vault, token, anchor, finality verifier, SP1 gateway, and verifier runtime
      hashes match the frozen manifest.
- [ ] Arc and PFTL checkpoints are fresh and match the active route.
- [ ] a proof-capable exact-block registry-state source passes P7.
- [ ] current devnet is six-of-six healthy with empty mempools.
- [ ] ingress and egress H100 proving environments are pinned and budget-bounded.

### D2. Ingress

1. approve exactly the bounded test allowance;
2. deposit exactly 1,000,000 USDC atoms for a fresh PFTL recipient and nonce;
3. require Arc transaction receipt status 1;
4. capture one v2 witness at that exact block;
5. run native verification and the complete negative audit before renting/proving;
6. generate the current-v2 Groth16 proof on H100;
7. verify it locally and byte-compare public values;
8. submit proof-backed propose/finalize/claim on current devnet;
9. require accepted receipts from the finalized chain; and
10. verify all six nodes report the exact 1,000,000-atom pfUSDC credit and
    advanced Arc checkpoint.

### D3. Egress

1. burn exactly the minted 1,000,000 pfUSDC atoms to the originating Arc EOA;
2. require an accepted PFTL receipt and exact bridge exit commitment;
3. capture the current PFTL finality witness from multiple nodes and byte-compare
   immutable fields;
4. run native verification and adversarial audit;
5. generate and locally verify the current egress proof;
6. preflight the proof against the deployed Arc verifier with no broadcast;
7. submit the withdrawal proof;
8. require Arc receipt status 1;
9. verify the exact 1,000,000-atom release; and
10. prove withdrawal ID and proof nullifier replay both fail.

### D4. Terminal audit

All six PFTL nodes and two independent Arc RPC observations MUST agree on:

- transaction and block identifiers;
- proof and public-value hashes;
- route and checkpoint state;
- deposit, burn, withdrawal, and nullifier state;
- wallet, vault, counted, issued, and redeemed balances; and
- exact conservation.

Terminate all rented GPUs after artifact retrieval and record
provider-confirmed termination.

**Gate D:** one fresh 1.000000-USDC proof-verified round trip completes on Arc
testnet and the current six-validator PFTL devnet, exact to the atom.

## 9. Phase E — Zellic co-sponsor review packet

Create:

```text
docs/review/zellic/pfusdc-arc-<UTC>/
  00-cover-letter.md
  01-executive-technical-summary.md
  02-review-scope.md
  03-architecture-and-dataflow.md
  04-trust-and-privilege-model.md
  05-primitive-evidence-matrix.md
  06-deployment-and-round-trip.md
  07-adversarial-results.md
  08-reproducible-build.md
  09-known-findings-and-limitations.md
  10-grant-claim-matrix.md
  11-audit-scope-and-estimate-request.md
  MANIFEST.json
  SHA256SUMS
```

### E1. Review scope

The packet MUST identify exact review surfaces:

- `programs/pfusdc-arc-ingress`;
- `programs/pfusdc-egress`;
- shared proof/MPT/canonical-encoding code reached by those guests;
- Arc witness capture and proof tooling;
- PFTL proof verification and route/checkpoint state transitions;
- `ArcPfUsdcDeploymentFactory.sol`;
- `ERC20BridgeVaultV2.sol`;
- `PfUsdcIngressAnchorV1.sol`;
- `PFTLFinalityVerifierV1.sol`;
- SP1 gateway/verifier integration;
- deployment scripts and immutable bindings; and
- replay, pause, upgrade, custody, and operational boundaries.

Every surface is pinned to a commit and line inventory. Generated and vendored
code is labeled separately.

### E2. Threat model

At minimum cover:

- dishonest Arc RPC/proof provider;
- forged or sub-quorum Arc certificate;
- validator-set rotation and registry/proxy upgrade;
- malformed receipt/MPT/ABI data;
- malicious relayer;
- wrong ELF/vkey/verifier route;
- PFTL Byzantine minority and committee transition;
- rejected transaction inside a finalized block;
- replay across deposits, routes, epochs, chains, proofs, and withdrawals;
- malicious/reentrant/non-standard token;
- pause and privileged-key misuse;
- checkpoint races and stale proofs;
- resource exhaustion in host, guest, and RPC inputs;
- snapshot/restart/replay faults; and
- compromised prover availability without proof-forging capability.

### E3. Grant claim matrix

Every material sentence in
`docs/business/pfusdc-arc-grant-proposal-20260828-v3.md` is classified as:

- **verified technical fact** — linked to current evidence;
- **historical fact** — linked and dated;
- **commercial commitment** — requires founder signoff, not Zellic technical
  verification;
- **forecast/target** — explicitly labeled non-guaranteed;
- **third-party fact** — linked to its primary source and dated; or
- **unsupported** — remove or rewrite before submission.

In particular, the packet MUST distinguish:

- the already demonstrated historical Arbitrum round trip;
- the newly qualified Arc testnet/current-devnet round trip;
- planned audits from completed audits;
- testnet contracts from mainnet deployments;
- a requested Zellic review from Zellic endorsement;
- current locked testnet value from future TVL targets; and
- measured latency/cost from targets.

Claims such as “first,” “audited,” “live,” “production,” and “trustless” require
explicit evidence and scope. If that evidence is absent, remove the word.

### E4. Requested Zellic action

The cover letter asks Zellic to:

1. review technical feasibility and the primitive evidence matrix;
2. identify blocking security findings or missing audit surfaces;
3. confirm or revise the proposed contract/zkVM audit scope;
4. provide an audit estimate and schedule if interested; and
5. optionally act as a named technical co-sponsor for the Arc grant.

Co-sponsorship language MUST be supplied or approved by Zellic. Post Fiat does
not draft a quotation or endorsement on Zellic's behalf.

### E5. Packet validation

- [ ] all links resolve from a clean public clone;
- [ ] every command works from the pinned revision;
- [ ] manifests and SHA-256 sums verify;
- [ ] no secret, private endpoint, private host identity, raw signed
      transaction, or unredacted log is present;
- [ ] testnet/mainnet and historical/current labels are unambiguous;
- [ ] known blockers and failed attempts are included;
- [ ] the grant proposal's requested amount, milestones, commercial commitments,
      and submission authority receive founder signoff;
- [ ] an independent reviewer reproduces the packet;
- [ ] the docs site builds strictly and redaction/link checks pass; and
- [ ] the final archive is content-addressed.

**Gate E:** a clean-room reviewer can reproduce the technical claims and the
packet can be sent to Zellic without further engineering explanation.

## 10. Stop conditions

Stop the affected lane immediately if any of the following occurs:

- current-v2 registry state cannot be authenticated at the deposit block;
- rebuilt ELF/vkey differs from any active route or deployed verifier;
- Arc certificate does not bind the exact execution header;
- any proof verifies against altered public values;
- a finalized PFTL block contains a rejected bridge operation and tooling treats
  inclusion as success;
- any replay credits or releases value twice;
- any one-atom conservation mismatch appears;
- any validator diverges on height, state, route, checkpoint, or replay state;
- snapshot/restart/replay changes the terminal state root;
- Arc contract readbacks differ from the deployment manifest;
- an H100 proof fails local verification;
- a required negative case accepts;
- a private key or operational secret enters logs/evidence; or
- a commercial claim cannot be classified under E3.

A stop condition is a finding. It is recorded, not waived to preserve the grant
schedule.

## 11. Evidence and decision records

The qualification root MUST contain:

```text
MANIFEST.json
ACCEPTANCE.json
source-and-build.json
fleet-before.json
fleet-after.json
deployment-receipt.json
route-activation-receipt.json
primitive-matrix.json
round-trip.json
conservation.json
negative-suite.json
gpu-receipts.json
known-findings.json
public-artifact-index.json
```

`ACCEPTANCE.json` contains booleans for Gates A–E and hashes every referenced
artifact. The terminal disposition is one of:

| Disposition | Meaning |
| --- | --- |
| `READY_FOR_ZELLIC_REVIEW` | A–E pass; packet may be sent for review/co-sponsor consideration |
| `REVIEWABLE_WITH_DISCLOSED_FINDING` | A and E pass; one or more B–D gates remain false and are plainly disclosed |
| `NOT_READY` | build, primitive, reproducibility, secret, or claim-integrity gate failed |

Only `READY_FOR_ZELLIC_REVIEW` supports the claim that the Arc proposal was
tested end to end on the current devnet. The second disposition may be shared
for early technical feedback but MUST NOT claim current-devnet qualification.

## 12. Definition of done

This specification is complete when:

1. the exact current source and proof identities are frozen and reproducible;
2. all required code and contract gates pass;
3. the current-v2 registry proof blocker is closed without weakening the guest;
4. P1–P18 pass;
5. the exact candidate is qualified and deployed on the current six-validator
   devnet under explicit authorization;
6. one fresh 1.000000-USDC Arc-testnet/current-devnet round trip succeeds;
7. both proof directions verify, both replay boundaries reject, and
   conservation is exact;
8. all six validators survive restart/replay and agree on the terminal root;
9. the grant claims are evidence-classified and founder-approved;
10. the public Zellic packet reproduces from a clean clone; and
11. `ACCEPTANCE.json` records `READY_FOR_ZELLIC_REVIEW`.

That state is sufficient to submit the Arc proposal to Zellic for independent
technical review and possible co-sponsorship. It is not an audit, a Zellic
endorsement, an Arc grant award, or mainnet authorization.

## 13. Fastest safe execution order

The submission deadline does not change any gate. Work proceeds in three
parallel lanes where dependencies permit:

### Lane 1 — Source and devnet

1. freeze the final branch head and finish CI;
2. run the current-source and contract gates;
3. capture the fresh fleet observation;
4. build the deployment-exact six-node clone;
5. obtain explicit live-deployment authorization; and
6. roll the qualified candidate to the current devnet.

### Lane 2 — Arc proof primitives

1. qualify an exact-block registry proof source first;
2. make a fresh 1.000000-USDC deposit only after that source passes preflight;
3. capture and natively verify the complete v2 witness;
4. run the negative suite;
5. rent the H100 only after native verification passes;
6. prove and locally verify ingress;
7. complete current-devnet admission and burn; and
8. prove and settle egress on Arc testnet.

### Lane 3 — Zellic packet

1. build the packet skeleton and source inventory immediately;
2. classify every v3 grant claim;
3. populate historical lineage without calling it current evidence;
4. add current evidence only after each gate closes;
5. run a clean-clone reproduction and public-link check; and
6. obtain founder signoff on business commitments and the final send.

Hard ordering constraints:

- no H100 spend before a complete witness passes native verification;
- no live current-devnet deployment before the exact clone and rollback pass;
- no “current-devnet tested” claim before Gate D;
- no “Zellic reviewed/co-sponsored” claim before Zellic says so; and
- no deadline-based waiver of a primitive, replay, conservation, or secret gate.

If Gate D cannot close before the initial Zellic contact, the packet may be sent
only as `REVIEWABLE_WITH_DISCLOSED_FINDING` for early technical feedback. The
fully qualified co-sponsor request follows when the disposition becomes
`READY_FOR_ZELLIC_REVIEW`.
