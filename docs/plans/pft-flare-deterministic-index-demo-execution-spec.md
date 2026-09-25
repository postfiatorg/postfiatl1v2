# Post Fiat / Flare Deterministic Index Gate Demo Execution Spec

Date: 2026-08-24

Status: implementation-ready demo specification; no live-value authorization

Primary implementation repo: new standalone repo `pft-flare-index-gate`

Supporting repos: `pft_indexing`, `navstrategies`, `fce-extension-scaffold`

Settlement repo: `postfiatl1v2`, documentation only in v0

Target network: Flare Coston2, chain ID `114`

Funding: faucet C2FLR only; no live FLR and no real assets

TEE mode: simulated locally for the required demo; real GCP Confidential Space
attestation is an optional follow-on gate

Change control: any change to the epoch identity formula, replay-receipt signing
preimage, replay threshold, accepted-result signing preimage, Solidity contract,
Go extension, or fixture hashes produces a new demo version and a new evidence
bundle. Existing successful evidence bundles are immutable.

## 1. Objective

Build an end-to-end public testnet demonstration in which:

1. Post Fiat supplies a content-addressed deterministic financial-index epoch.
2. Two explicitly identified demo replay operators sign byte-matching replay
   receipts for that epoch.
3. A permissionless caller submits the epoch and receipts through a Solidity
   instruction contract on Coston2.
4. Flare Confidential Compute relays the instruction to a Go extension.
5. The extension validates the epoch commitments, replay quorum, freshness, and
   domain bindings and rejects any disagreement.
6. The TEE signs a bounded `AcceptedIndexEpochV0` authorization.
7. Anyone can relay that authorization back to the Coston2 contract.
8. The contract finalizes the `(series_id, epoch)` at most once and emits a
   permanent accepted-index event.
9. A verification report links the Coston2 transactions and FCC identities to
   the Post Fiat index snapshot and replay artifacts.

The required simulated demo proves the relay, validation, signature, and
one-request/one-finalization wiring under the tested program. It does not prove
that Coston2 measured the local container. The optional hardware-attested gate
is what proves that a measured acceptance program admitted the epoch. Neither
mode trades, custodies, mints, redeems, bridges, or rebalances assets.

### 1.1 One-sentence product statement

> Post Fiat proves the qualitative index decision is independently replayable;
> the Coston2 demo proves the Flare acceptance path, and a hardware-attested run
> proves the measured acceptance program signed only the matching, one-time
> index epoch.

### 1.2 Definition of done

The v0 demo is complete when a clean machine can reproduce the code and fixture
hashes, deploy the contract with faucet C2FLR, run the simulated FCC stack,
submit one valid epoch, finalize it exactly once, and produce a redacted report
containing:

- the Post Fiat epoch and snapshot digests;
- both replay-operator addresses and signed receipt digests;
- the Flare instruction and finalization transaction hashes;
- the FCC extension ID, TEE ID, code hash, platform, and version;
- the accepted-result signature and recovered TEE address;
- passing negative tests for altered weights, mismatched replay output,
  duplicate operators, insufficient quorum, expiration, wrong domain, and
  replayed finalization.

## 2. Product and Trust Boundary

### 2.1 What Post Fiat proves

The Post Fiat deterministic-index pipeline freezes and content-addresses the
semantic and mechanical index inputs:

- evidence or company-input manifest;
- complete prompt manifest;
- model weights/revision and tokenizer manifest;
- SGLang/container/GPU inference profile;
- raw model response and score-vector hashes;
- parser and transformation-code hash;
- final constituent and weight hash;
- `pft.index.snapshot.v1` identity and content hash.

Independent byte-exact replay is the evidence that the semantic classification
was applied consistently. Flare must not replace or weaken this evidence.

### 2.2 What Flare proves

Flare supplies the public instruction path, measured extension identity, TEE
signature, and bounded execution gate. The extension signs only when the replay
receipts agree with the exact Post Fiat epoch supplied in the instruction.

Flare does **not** prove that:

- the model's qualitative judgment is financially correct;
- the theme, prompt, or universe was wisely selected;
- the replay operators are organizationally independent merely because they use
  different keys;
- the index has investment merit;
- reserves or a custodian's off-chain statements are complete;
- any later NAVCoin is legally outside adviser, fund, securities, money-service,
  sanctions, KYC, or AML rules.

### 2.3 What the no-manager claim means

The precise Gate 4 simulated-demo claim is:

> The tested contract and handler reject a different result for the configured
> series and epoch, and the Coston2 state machine cannot be rerolled or finalized
> twice. Simulated attestation does not prove which container code ran.

After Gate 5 hardware attestation passes, the stronger claim is:

> After deployment of one immutable gate contract and one registered extension
> version, no caller can finalize a different result for the configured series
> and epoch unless the configured replay threshold signs the different result
> and the TEE acceptance code accepts it.

Neither claim eliminates genesis discretion. Humans still select the
series, prompt, universe, model, replay policy, and deployment version. Every
such choice must be visible in the manifest and evidence report.

### 2.4 Demo-only trust class

The required Coston2 run uses `SIMULATED_TEE=true`. Its UI and report trust
class MUST be:

```text
DEMO_SIMULATED_TEE
```

It must never display `production`, `hardware-attested`, or unqualified
`trustless`. A later real Confidential Space run may use:

```text
TESTNET_HARDWARE_ATTESTED_TEE
```

only after the attestation token and non-test code hash are independently
verified.

## 3. Scope

### 3.1 Required v0 scope

- One standalone Go FCC extension.
- One non-upgradeable Solidity gate contract.
- One fixed demonstration index series.
- One fixed epoch fixture derived from a published Post Fiat deterministic
  financial-index evidence set.
- Exactly two required replay signatures from two distinct configured addresses.
- One CLI that builds and validates the fixture.
- One CLI that submits, polls, finalizes, and verifies the Coston2 flow.
- One static/read-only result page or self-contained HTML report.
- Unit, integration, mutation, and Coston2 end-to-end tests.
- One machine-readable evidence bundle.

### 3.2 Explicit non-goals

- Running Qwen/SGLang inference inside the FCC TEE.
- Generating new themes during the demo.
- Live FLR, USDC, equities, NAVCoin, or other assets.
- TEE-held portfolio or custody keys.
- PFTL consensus changes.
- NAVCoin registration, minting, reserve submission, redemption, or settlement.
- FTSO price use or FDC/Web2Json attestation.
- Mainnet deployment.
- Upgradeable contracts.
- A generalized arbitrary-manifest oracle.
- Claims that deterministic replay proves analytical or economic validity.

## 4. Repository Ownership and File Map

### 4.1 New repo

Create `/home/postfiat/repos/pft-flare-index-gate` from the current
`fce-extension-scaffold` Go path. Do not modify the cloned upstream scaffold.

The target tree is:

```text
pft-flare-index-gate/
├── README.md
├── SECURITY.md
├── REPRODUCIBILITY.md
├── go.mod
├── go.sum
├── Dockerfile
├── docker-compose.yaml
├── contracts/
│   ├── PftIndexGate.sol
│   ├── interfaces/
│   │   ├── ITeeExtensionRegistry.sol
│   │   └── ITeeMachineRegistry.sol
│   └── test/
│       └── PftIndexGate.t.sol
├── internal/
│   ├── config/config.go
│   ├── extension/extension.go
│   ├── extension/attest_index.go
│   ├── extension/attest_index_test.go
│   ├── receipt/hash.go
│   ├── receipt/verify.go
│   └── receipt/verify_test.go
├── pkg/types/
│   ├── types.go
│   └── register.go
├── cmd/
│   ├── fixture/main.go
│   ├── run-demo/main.go
│   └── verify-report/main.go
├── fixtures/v0/
│   ├── epoch-basis.json
│   ├── epoch-manifest.json
│   ├── index-snapshot.json
│   ├── replay-policy.json
│   ├── replay-receipt-test-vectors.json
│   ├── expected-hashes.json
│   └── source/
│       └── qwen38-financial-index-determinism-20260815-summary.json
├── schemas/
│   ├── pft-index-epoch-manifest-v0.schema.json
│   ├── pft-index-replay-receipt-v0.schema.json
│   └── pft-flare-index-demo-report-v0.schema.json
├── scripts/
│   ├── preflight.sh
│   ├── build-fixture.sh
│   ├── run-local-tests.sh
│   ├── run-coston2-demo.sh
│   └── redact-and-verify-report.sh
├── config/
│   ├── coston2/deployed-addresses.json
│   └── proxy/extension_proxy.coston2.docker.toml.example
├── demo/
│   ├── index.html
│   └── render-report.js
└── reports/.gitkeep
```

Retain the scaffold's deployment, registration, proxy, Docker, reproducibility,
and version-checking machinery unless a test demonstrates it is unnecessary.

### 4.2 Existing repos

`pft_indexing` owns Post Fiat snapshot validation and content hashing. Additions
are not required to start v0. If a helper is needed, add only:

```text
src/pft_indexing/autonomy_receipt.py
schemas/pft-index-autonomy-receipt-v0.schema.json
tests/test_autonomy_receipt.py
```

The fixture builder must call the existing `pft_indexing` canonicalization and
snapshot validation rather than reimplementing it in Go.

`navstrategies` is read-only for v0. Copy a frozen, already-published evidence
summary into the new demo fixture with its source path, public URL, and digest.
Do not change a theme, prompt, universe, score, weight, or backtest.

`postfiatl1v2` receives no runtime or consensus changes in v0. This spec is the
only required PFTL change. NAVCoin integration starts only after the Coston2
evidence gate passes.

## 5. Canonical Post Fiat Epoch

### 5.1 Hash conventions

Use two named hash domains and never silently convert between them:

| Domain | Algorithm | Encoding | Purpose |
| --- | --- | --- | --- |
| Post Fiat content | SHA-256 | existing `pft_indexing.canonical_json_bytes` | manifests, snapshots, epoch identity |
| Flare/EVM | Keccak-256 | Solidity `abi.encode`, never `abi.encodePacked` for structured values | request, receipt-signing, accepted-result signing |

All JSON hexadecimal digests use lowercase, 64-character hex without `0x`.
All ABI `bytes32` values use `0x`-prefixed 32-byte hex.

### 5.2 Epoch basis

The v0 fixture is fixed now; implementation must not choose a different result:

- series ID:
  `pft.demo.deterministic-financial-index.nvda-sbux-long-short.v0`;
- epoch: `1`;
- source artifact:
  `https://postfiat.org/benchmarks/qwen38-financial-index-determinism-20260815-summary.json`;
- observed source artifact SHA-256 on 2026-08-24:
  `0b1cc03a27b91e72419abed4acb2410f7dabe55427c8ebfa94a3c0ebd92a0149`;
- model: `Qwen/Qwen3.8-27B` at revision
  `1d4bf0f2ff6012fd82039f2fa52739d0dd7c60c0`;
- source scores: NVDA/SBUX `85/35` for dystopia, `98/12` for hostile AGI,
  and `62/68` for positive ESG/utopia;
- transformation: population z-score each two-company lens, calculate
  `mean(z_dystopia, z_hostile, -z_esg_utopia)`, then normalize positive and
  negative sides separately to `+0.5/-0.5` gross-balanced weights;
- expected target weights: NVDA `0.5`, SBUX `-0.5`, net exposure `0`, gross
  exposure `1`.

This is intentionally a two-company, non-investable demonstration derivative
of the article's published six-case diagnostic, not the article's full
top-1,000 corpus and not a production index. The fixture must preserve that
label. Its purpose is to exercise exact semantic-score commitments and a
mechanical transformation with a small inspectable artifact.

Vendor the exact source bytes under `fixtures/v0/source/` so building and
verification never depend on the public URL remaining mutable or reachable.
The public URL is provenance; the vendored bytes and recorded SHA-256 are the
input.

`epoch-basis.json` has exactly these fields:

```json
{
  "schema": "pft.index.epoch-basis.v0",
  "series_id": "pft.index.series.example.v0",
  "epoch": 1,
  "evidence_manifest_hash": "<sha256>",
  "prompt_manifest_hash": "<sha256>",
  "model_runtime_manifest_hash": "<sha256>",
  "raw_score_vector_hash": "<sha256>",
  "transformation_code_hash": "<sha256>",
  "final_weights_hash": "<sha256>",
  "snapshot_digest": "<sha256>"
}
```

The identity is:

```text
index_epoch_id = SHA256(
  UTF8("pft.index.epoch.v0\n") ||
  pft_indexing.canonical_json_bytes(epoch_basis)
)
```

The fixed-width EVM series identifier is:

```text
series_id_hash = SHA256(
  UTF8("pft.index.series.v0\n") || UTF8(series_id)
)
```

The fixture builder must fail if a listed artifact is missing, its observed
hash differs, the snapshot fails `pft-index validate`, or the recomputed epoch
ID differs from `expected-hashes.json`.

### 5.3 Epoch manifest

`epoch-manifest.json` adds human and source metadata without changing the epoch
identity:

- `index_epoch_id`;
- public article URL;
- source repository and commit;
- artifact paths or content-addressed URLs;
- published model repository and revision;
- tokenizer/model-file manifest digest;
- SGLang image digest and version;
- hardware/runtime profile;
- deterministic launch flags;
- parser and missing-score policy;
- replay coverage statement;
- explicit limitations;
- creation timestamp.

The manifest itself receives `manifest_digest = SHA256(canonical manifest
without manifest_digest)`. The EVM instruction binds both `index_epoch_id` and
`manifest_digest`.

## 6. Replay Policy and Receipts

### 6.1 Replay policy

`replay-policy.json` is compiled into the Go image and copied into the evidence
bundle:

```json
{
  "schema": "pft.index.replay-policy.v0",
  "policy_id": "pft.demo.replay-policy.v0",
  "threshold": 2,
  "operators": [
    {"address": "0x...", "label": "demo-replay-a"},
    {"address": "0x...", "label": "demo-replay-b"}
  ],
  "max_receipts": 8
}
```

Operator addresses must be strictly ascending. Duplicate or zero addresses are
invalid. The build must emit and test `replay_policy_hash_sha256`.

The first demo may control both keys, but the report must say
`organizational_independence: false`. Different addresses alone are not proof
of independent operators.

The content fixture contains public-key/signature test vectors bound to a fixed
local test contract. Coston2 replay receipts are generated only after the gate
contract address exists because the signing domain includes that address. Store
those deployment-bound receipts under the new run's report directory, not by
rewriting the immutable content fixture.

### 6.2 Replay statement

Each operator signs the same fixed statement:

```solidity
keccak256(abi.encode(
    bytes32("PFT_REPLAY_RECEIPT_V0"),
    uint256(114),
    address(indexGate),
    bytes32(seriesIdHash),
    uint64(epoch),
    bytes32(indexEpochId),
    bytes32(manifestDigest),
    bytes32(rawScoreVectorHash),
    bytes32(finalWeightsHash),
    bytes32(snapshotDigest),
    uint64(replayedAt),
    uint64(validUntil)
))
```

The signature is the standard Ethereum signed-message signature over that
32-byte digest. The implementation must use one shared Go helper and a matching
Solidity test vector. No handwritten alternative serializer is allowed.

### 6.3 Receipt validation

The extension must:

1. reject more than `max_receipts`;
2. recover every signer;
3. reject zero, unauthorized, or duplicate signers;
4. reject a receipt with a different chain, contract, series, epoch, epoch ID,
   manifest, score vector, final weights, or snapshot;
5. reject a receipt whose `validUntil <= replayedAt`;
6. reject the entire request if any supplied authorized receipt conflicts;
7. require at least the configured threshold of valid distinct signers;
8. sort valid receipts by recovered signer before computing
   `replay_receipts_root`;
9. return a stable error code rather than a free-form-only failure.

The receipt root is:

```text
receipt_leaf = Keccak256(abi.encode(replay statement fields, signer, signature))
replay_receipts_root = Keccak256(abi.encode(sorted receipt_leaf[]))
```

The replay policy hash placed in EVM fields is the 32-byte SHA-256 digest of the
canonical replay-policy JSON. The field name must remain
`replayPolicyHashSha256` in Go and Solidity types so reviewers cannot mistake it
for Keccak-256.

## 7. Coston2 Contract

### 7.1 Contract properties

`PftIndexGate.sol` must be non-upgradeable, hold no funds other than incidental
instruction fees, and expose no arbitrary call or token-transfer capability.

Constants:

```solidity
OP_TYPE_INDEX = bytes32("PFT_INDEX")
OP_COMMAND_ATTEST = bytes32("ATTEST_INDEX")
ACCEPTANCE_DOMAIN = bytes32("PFT_INDEX_ACCEPTANCE_V0")
```

Immutable or one-time state:

- Flare `TeeExtensionRegistry` address;
- Flare `TeeMachineRegistry` address;
- `seriesIdHash`;
- `replayPolicyHashSha256`;
- deployer/initial owner used only for one-time setup;
- extension ID, set once using the scaffold pattern;
- accepted TEE address, set once after registration evidence is checked.

The contract must not contain an upgrade hook, delegatecall, arbitrary signer
rotation, holdings override, result override, or reset function.

### 7.2 Epoch state machine

```text
UNSEEN -> REQUESTED -> ACCEPTED
                   \-> REJECTED is represented by a signed/status-0 FCC result,
                       but the contract remains REQUESTED and cannot accept a
                       different request hash.
```

`requestIndexAttestation` is permissionless. It must:

- require the configured series;
- require an unseen epoch;
- require `validUntil > block.timestamp` and within a bounded maximum window;
- ABI-encode the typed request and receipts;
- compute and store `requestHash` before the external registry call;
- set the epoch to `REQUESTED` before the external call;
- send exactly one FCC instruction;
- emit `IndexAttestationRequested`;
- revert the whole transaction if instruction submission fails.

Once an epoch is `REQUESTED`, no second request hash is accepted. A failed or
unavailable TEE therefore fails closed. Recovery uses a new contract/version,
not mutation of the existing epoch.

The contract and Go extension compute the instruction request hash as:

```solidity
encodedReceiptsHash = keccak256(abi.encode(receipts));

requestHash = keccak256(abi.encode(
    bytes32("PFT_INDEX_REQUEST_V0"),
    block.chainid,
    address(this),
    seriesIdHash,
    epoch,
    indexEpochId,
    manifestDigest,
    rawScoreVectorHash,
    finalWeightsHash,
    snapshotDigest,
    replayPolicyHashSha256,
    encodedReceiptsHash,
    uint16(receipts.length),
    validUntil
));
```

The exact typed request and receipts are included in the instruction message.
The contract does not treat a caller-supplied request hash as authoritative.
The Go extension rejects a supplied or decoded hash that differs from its own
recomputation.

### 7.3 Accepted result

The extension returns and signs:

```solidity
struct AcceptedIndexEpochV0 {
    bytes32 requestHash;
    bytes32 seriesIdHash;
    uint64 epoch;
    bytes32 indexEpochId;
    bytes32 manifestDigest;
    bytes32 rawScoreVectorHash;
    bytes32 finalWeightsHash;
    bytes32 snapshotDigest;
    bytes32 replayPolicyHashSha256;
    bytes32 replayReceiptsRoot;
    uint16 receiptCount;
    bytes32 instructionId;
    uint64 validUntil;
}
```

The TEE sign-server message is `abi.encode(ACCEPTANCE_DOMAIN, blockChainId,
indexGateAddress, acceptedResult)`. The sign server applies its documented
Keccak/Ethereum-message signing wrapper. Solidity must recover against the exact
same preimage.

`finalizeIndexAttestation` is permissionless. It must:

- require epoch state `REQUESTED`;
- require the stored request hash;
- require all configured series/policy fields;
- require `block.timestamp <= validUntil`;
- recover the one-time configured TEE signer;
- set state to `ACCEPTED` before emitting;
- store only compact fields needed for verification;
- emit the complete accepted result and signer;
- reject a second finalization.

### 7.4 TEE binding limitation

The scaffold's minimal Solidity interface exposes random TEE selection but not
a complete on-chain production-machine predicate. For v0, the deployer binds the
TEE address once after the deployment CLI verifies `/info`, extension ID,
registered machine state, and availability. This is an observable genesis
trust step and must be recorded in the report.

Do not copy the orderbook example's TEE setter without adding:

- one-time-only enforcement;
- extension-ID equality evidence;
- code-hash evidence;
- platform/trust-class evidence;
- recovered-address equality test;
- explicit report fields.

## 8. FCC Go Extension

### 8.1 Handler

Add one handler:

```text
OP_TYPE=PFT_INDEX
OP_COMMAND=ATTEST_INDEX
```

Processing order is consensus-like and must not mutate state:

1. Decode the FCC `DataFixed` envelope.
2. Require the exact operation type and command.
3. ABI-decode the typed request and bounded receipt array.
4. Recompute `seriesIdHash`, `requestHash`, and receipt roots.
5. Check chain ID, target contract, series, policy, epoch bounds, and expiry.
6. Verify every replay receipt and threshold.
7. Build `AcceptedIndexEpochV0` from recomputed values only.
8. Ask the local TEE sign server to sign the acceptance preimage.
9. Return ABI-encoded `(AcceptedIndexEpochV0, bytes teeSignature)` in the FCC
   action result.

The handler must never fetch a URL, call an LLM, use wall-clock data as a
portfolio input, accept a caller-supplied policy, or silently discard a
conflicting authorized receipt.

For expiry, the extension requires `validUntil` to be later than every receipt's
`replayedAt` and later than the latest signed FCC instruction timestamp when one
is present. The Coston2 contract remains the definitive clock and rechecks
`block.timestamp <= validUntil` during finalization.

### 8.2 State and concurrency

The extension must be stateless. The Coston2 contract owns epoch uniqueness.
No security property may depend on in-memory maps because an FCC restart creates
a new process and potentially a new TEE ID.

The handler must be safe under concurrent requests. Use immutable configuration
and local variables only. Run unit tests with Go's race detector.

### 8.3 Stable errors

Status `0` results use machine-readable prefixes:

```text
E_SCHEMA
E_DOMAIN
E_SERIES
E_POLICY
E_RECEIPT_LIMIT
E_RECEIPT_SIGNER
E_RECEIPT_DUPLICATE
E_RECEIPT_MISMATCH
E_QUORUM
E_EXPIRED
E_SIGN
```

The human-readable suffix may add detail but must not contain private keys,
credentials, complete environment variables, or arbitrary upstream response
bodies.

## 9. Demo CLI and Evidence Report

### 9.1 `fixture`

`cmd/fixture` must:

- invoke `pft-index validate` for the snapshot;
- recompute every SHA-256 content digest;
- recompute `index_epoch_id`;
- verify the fixed local signature vectors, or create deployment-bound receipts
  in a newly named run directory after a contract address is supplied;
- emit EVM test vectors;
- refuse to overwrite an existing evidence fixture unless `--out` names a new
  directory;
- print paths and public addresses, never private-key contents.

Private replay keys are development secrets. Generate them into a gitignored
directory or accept them through environment/file descriptors. Fixed test keys
may exist only in unit-test fixtures and must be unmistakably labeled unsafe.

### 9.2 `run-demo`

`cmd/run-demo` must be resumable and idempotent at each read-only step:

1. preflight RPC, balance, chain ID, contract bytecode, proxy `/info`, and
   fixture hashes;
2. submit `requestIndexAttestation`;
3. extract and record the Coston2 transaction and FCC instruction ID;
4. poll the extension proxy with a bounded timeout;
5. decode the result and verify its FCC action-result signature;
6. verify the custom accepted-result signature;
7. call `finalizeIndexAttestation`;
8. read accepted state back from Coston2;
9. write the report to a new timestamped directory;
10. run the report verifier before printing success.

The CLI must not retry `requestIndexAttestation` with a changed payload. A
timeout leaves the epoch visibly `REQUESTED` and exits nonzero.

### 9.3 Report schema

The JSON report must include:

- schema/version and run timestamps;
- trust class;
- Git commits and dirty-state flags;
- fixture path and every Post Fiat hash;
- replay policy, operator public addresses, signatures, and independence label;
- chain ID, RPC identity, contract address, extension ID;
- request transaction/hash, instruction ID, action status;
- proxy URL origin with credentials/query strings removed;
- TEE ID, recovered address, platform, code hash and extension version;
- finalization transaction/hash and accepted event;
- verification checks with pass/fail status;
- limitations and unproven claims.

Never include deployer keys, replay private keys, proxy/indexer credentials,
seed phrases, bearer tokens, raw environment dumps, or authenticated URLs.

## 10. Implementation Sequence

### Gate 0 — freeze fixture and protocol constants

Work:

- Materialize the fixed NVDA/SBUX demonstration fixture specified in section
  5.2 without changing its source scores or transformation.
- Record source URLs, local artifacts, hashes, and limitations.
- Generate two demo replay identities.
- Freeze the three domain tags, hash formulas, threshold, series ID, and epoch.
- Add cross-language hash/signature vectors.

Exit criteria:

- `pft-index validate` passes.
- Python and Go compute the same epoch ID.
- Go and Solidity compute the same replay preimage and recover both signers.
- The fixture states whether it is complete published evidence or a reduced demo
  derivative.

### Gate 1 — standalone repository and deterministic core

Work:

- Copy the current Go scaffold into the new repo.
- Rename module, commands, containers, and operation constants.
- Remove greeting state and handlers.
- Implement types, hash helpers, receipt recovery, bounds, and stable errors.
- Retain reproducible Go Docker controls.

Exit criteria:

- `go test ./...` passes.
- `go test -race ./...` passes.
- `go vet ./...` passes.
- Reordered receipts produce the same root after sorting.
- Mutating any bound field changes the expected digest or fails validation.

### Gate 2 — Solidity gate

Work:

- Implement the one-series, one-request-per-epoch state machine.
- Implement FCC instruction submission.
- Implement accepted-result signature recovery and finalization.
- Add complete events and getters.
- Add invariant/fuzz tests.

Exit criteria:

- `forge fmt --check` passes.
- `forge test -vvv` passes.
- `forge test --fuzz-runs 1000` passes.
- A second request for an epoch reverts.
- A second finalization reverts.
- Wrong chain, contract, series, policy, epoch, weights, snapshot, expiry, signer,
  or request hash reverts.
- No method can transfer assets or execute arbitrary calls.

### Gate 3 — local extension round trip

Work:

- Build the deterministic Go image.
- Run the handler and TEE-node path under Docker.
- Submit valid and invalid actions through the extension endpoint.
- Verify FCC and custom signatures.

Exit criteria:

- Valid fixture returns status `1` and the expected accepted result.
- Every mutation corpus case returns status `0` with the expected stable code.
- Rebuilding from the same commit and source epoch yields the documented image
  digest under the scaffold's reproducibility procedure.

### Gate 4 — Coston2 simulated-TEE demo

Prerequisites:

- fresh dev-only EVM key;
- faucet C2FLR balance;
- Go 1.25+;
- Docker/Compose, Foundry, `jq`, and `cloudflared`;
- Coston2 extension-indexer read credentials supplied through Flare support;
- public HTTPS tunnel to the local extension proxy.

Work:

1. Create `.env.coston2` from `.env.example` without committing it.
2. Create the proxy TOML from its example and insert credentials without
   committing it.
3. Confirm RPC chain ID `114` and faucet balance.
4. Run `pre-build.sh` to deploy/register the extension contract.
5. Start the Go extension, tee-node, proxy, Redis, and tunnel with
   `SIMULATED_TEE=true`.
6. Run `post-build.sh` and verify `/info` reports the simulated platform and
   expected extension.
7. Bind the recovered TEE address once in `PftIndexGate`.
8. Run the valid fixture end to end.
9. Run negative cases that do not consume a second real epoch.
10. Generate and verify the report.

Exit criteria:

- One Coston2 epoch reaches `ACCEPTED`.
- The final event fields equal the Post Fiat fixture.
- A repeat finalization and altered acceptance both revert.
- The evidence report verifier passes from a clean process.
- UI/report visibly says `DEMO_SIMULATED_TEE`.

### Gate 5 — optional hardware-attested Coston2 run

This gate is not required for the hack demo.

Work:

- Build and publish the image by digest.
- Deploy it on GCP Confidential Space with `MODE=0`.
- Verify the real attestation token, platform, image-derived code hash,
  extension ID, owner, and governance hash.
- Register the real machine and deploy a fresh gate contract/series version.
- Repeat Gate 4 without reusing simulated evidence.

Exit criteria:

- `/info` does not report `TEST_PLATFORM` or the simulated code hash.
- An independent rebuild matches the registered Go image digest.
- The final report uses `TESTNET_HARDWARE_ATTESTED_TEE` and identifies the GCP,
  Flare-provider, replay-operator, and genesis-governance trust assumptions.

### Gate 6 — optional PFTL/NAVCoin integration

Do not start this gate until Gate 4 passes and the NAVCoin policy is explicitly
approved.

Target design:

- Add the Flare accepted-index receipt as evidence referenced from
  `pft.index.snapshot.v1.extensions`.
- Publish the snapshot through the existing IPFS and `pf.ptr:v4` PFTL path.
- Add a PFTL proof profile that recognizes the approved Flare extension/code
  hash and verifies the bound Post Fiat epoch fields.
- Permit a NAVCoin operation only through a bounded PFTL-native operation
  envelope.
- Keep reserves, valuation, supply, mint/redeem, challenges, and off-ledger
  settlement under the existing NAVCoin truth boundary.

Gate 6 requires a separate L1 specification and the Rust L1 engineering review
process. This document does not authorize those changes.

## 11. Test Matrix

| ID | Case | Expected result |
| --- | --- | --- |
| H01 | Published fixture, two valid distinct receipts | Accepted |
| H02 | Receipts supplied in reverse order | Same semantic receipt root; request hash differs because it binds the exact wire envelope |
| F01 | One receipt only | `E_QUORUM` |
| F02 | Same signer twice | `E_RECEIPT_DUPLICATE` |
| F03 | Unauthorized signer | `E_RECEIPT_SIGNER` |
| F04 | One signer signs different final weights | `E_RECEIPT_MISMATCH` |
| F05 | Altered raw score vector | `E_RECEIPT_MISMATCH` |
| F06 | Altered snapshot digest | `E_RECEIPT_MISMATCH` |
| F07 | Wrong chain ID | `E_DOMAIN` |
| F08 | Wrong contract address | `E_DOMAIN` |
| F09 | Wrong series or replay policy | `E_SERIES` or `E_POLICY` |
| F10 | Expired request | extension rejection or contract expiry revert |
| F11 | More than eight receipts | `E_RECEIPT_LIMIT` |
| F12 | Malformed ABI/unknown fields | `E_SCHEMA` |
| C01 | Second request for same epoch | Contract revert |
| C02 | Finalization without request | Contract revert |
| C03 | Finalization with wrong TEE signature | Contract revert |
| C04 | Finalization with mutated accepted result | Contract revert |
| C05 | Second finalization | Contract revert |
| C06 | Reuse acceptance on another contract | Contract revert |
| C07 | Reuse acceptance on another chain | Signature mismatch/revert |
| O01 | Proxy unavailable after request | Epoch remains `REQUESTED`; no fallback |
| O02 | TEE restart/new address | Old gate cannot silently rotate; new deployment required |
| O03 | Report contains a seeded secret marker | Redaction verifier fails |

## 12. Operational Runbook

### 12.1 Preflight

```bash
docker --version
docker compose version
go version
forge --version
cast --version
jq --version
cloudflared --version
```

Required checks:

- no dirty generated fixture;
- no `.env`, credential TOML, private key, or report secret staged in Git;
- Coston2 RPC returns chain ID `114`;
- deployer balance is nonzero C2FLR;
- tunnel hostname resolves and `/info` is reachable only while the demo runs;
- current deployed FCC addresses come from the scaffold's Coston2 config, not
  chat text or an old document.

### 12.2 Expected command sequence

The implementation should converge on:

```bash
./scripts/preflight.sh --chain coston2
./scripts/build-fixture.sh --fixture fixtures/v0
./scripts/run-local-tests.sh
./scripts/pre-build.sh --chain coston2
./scripts/start-services.sh --chain coston2 --tunnel
./scripts/post-build.sh --chain coston2
./scripts/run-coston2-demo.sh --fixture fixtures/v0
./scripts/redact-and-verify-report.sh reports/<run-id>/report.json
./scripts/stop-services.sh --chain coston2 --tunnel
```

No script may print private-key material or proxy/indexer passwords. Failure
after request submission must print the stored request hash and current epoch
state so the operator does not rerun with a different payload.

## 13. Evidence and Presentation

The read-only demo page should show three separate proof columns:

| Post Fiat replay | Flare acceptance | Not proven |
| --- | --- | --- |
| Epoch identity | Coston2 request transaction | Investment merit |
| Model/runtime manifest | Extension and TEE identity | Prompt neutrality |
| Two replay receipts | Code hash/platform | Independent organizations in v0 |
| Score/weight/snapshot hashes | TEE acceptance signature | Custody or reserve completeness |
| Replay coverage | Accepted event/final state | Regulatory classification |

The page must link to the public Post Fiat deterministic-index article, the
Coston2 explorer transactions, the public fixture/evidence bundle, and the
source commit used to build the FCC extension.

## 14. Known Blockers and Current Workstation State

As of 2026-08-24, the workstation has Docker, Docker Compose, Foundry, `cast`,
Node 20, `uv`, `jq`, and `cloudflared`. Go is missing. The cloned Flare scaffold
does not contain a populated Coston2 proxy-indexer TOML or `.env.coston2`.

Before Gate 1:

- install Go 1.25+ and record the version;
- do not create or fund a wallet until the contract is ready to deploy;
- request Coston2 read-only indexer credentials from Flare technical support;
- use a new dev-only wallet and the Coston2 faucet;
- never reuse a Post Fiat, StakeHub, validator, or production treasury key.

The missing credentials block the public FCC relay, not local code, fixtures,
contract tests, or handler tests.

## 15. Security Review Checklist

- [ ] Every signed payload is chain- and contract-bound.
- [ ] Structured EVM values use `abi.encode`, not ambiguous packed encoding.
- [ ] SHA-256 and Keccak-256 fields are named and never interchanged.
- [ ] Receipt signers are unique, authorized, sorted, and threshold checked.
- [ ] A conflicting authorized receipt rejects the whole request.
- [ ] Contract state is written before external calls.
- [ ] One request and one finalization are possible per epoch.
- [ ] No upgrade, arbitrary call, token transfer, result override, or signer
      rotation exists.
- [ ] TEE restart cannot mutate accepted state.
- [ ] Timeouts and missing data fail closed.
- [ ] Report generation is append-only and redacted.
- [ ] Simulated and hardware-attested trust classes cannot be confused.
- [ ] Tests verify the exact Go/Solidity/Python cross-language vectors.
- [ ] Demo operator keys are not described as independent without evidence.
- [ ] No statement confuses replay fidelity with analytical or economic
      validity.

## 16. Deferred Decisions

These choices do not block v0 and must not be improvised during implementation:

1. Whether future index themes are generated on a schedule or submitted
   permissionlessly.
2. How prompt genesis and backtest cherry-picking are governed.
3. Whether replay admission uses named operators, Flare providers, Cobalt-linked
   validators, or an open bonded registry.
4. Whether Flare remains only an acceptance gate or later holds an execution
   key.
5. Which PFTL proof profile recognizes Flare evidence.
6. Whether every accepted index can instantiate a NAVCoin permissionlessly.
7. What legal and disclosure controls apply to any capital-bearing instance.

Each decision changes the product or trust claim and requires its own locked
specification.

## 17. References

- Post Fiat, [Deterministic Financial Indices: A New Paradigm for Trustless
  Qualitative Analysis](https://postfiat.org/blog/deterministic-financial-indices/)
- Flare, [Build Your First Extension](https://dev.flare.network/fcc/guides/getting-started)
- Flare, [Developer FAQ](https://dev.flare.network/support/faqs)
- Local Post Fiat index implementation: `pft_indexing/README.md`
- Local Post Fiat snapshot schema:
  `pft_indexing/schemas/pft-index-snapshot-v1.schema.json`
- Local Flare reproducibility guidance:
  `fce-extension-scaffold/REPRODUCIBILITY.md`
- Local FCC action-result signing documentation: `tee-node/docs/actions.md` and
  `tee-node/docs/cryptography.md`
- Local NAVCoin lifecycle: `postfiatl1v2/docs/navcoins/reserve-primitives.md`
- Local Post Fiat commitment model:
  `postfiatl1v2/docs/governance/verifiable-constitution-plan.md`
