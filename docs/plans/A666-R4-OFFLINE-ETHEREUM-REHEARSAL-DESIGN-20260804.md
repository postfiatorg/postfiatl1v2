# A666 R4 offline Ethereum browser rehearsal design

**Status:** design only. No code, build, service, Anvil, deployment, transaction, relay, or credential access was performed.

## Decision and hard boundary

Create an ephemeral loopback-only Anvil-class rehearsal that drives the rendered NAVCoin export/return controls, existing browser relay clients, existing proxy durable job routes, real source-derived A666 contracts, and native PFTL receipt-finality validation. It is a candidate-changing local integration rehearsal, never a mainnet or trustless-mint qualification.

Current browser export creates PftlUniswapMintPacketV2. Select PFTLUniswapPrimaryMarketV2, not the legacy handoff-controller packet ABI. The isolated PFTL rehearsal route must be created after deployment and pin the local controller/token code hashes and identities. The preserved checkpoint route cannot be reused unchanged because it pins different controller identity/code and the lifecycle only constructs Ethereum proofs.

## Reusable implementation inventory

| Component | Evidence anchors | Decision |
|---|---|---|
| Foundry | crates/ethereum-contracts/foundry.toml:1-9 | Reuse compiler profile. No tracked Anvil config or compiled ABI/bytecode artifact was found. |
| Deployment | crates/ethereum-contracts/script/DeployA666PrimaryMarket.s.sol:27-112,135-183 | Reuse deployment pattern for wrapped token, receipt verifier, controller, emitted identity. Existing script inputs are not runtime configuration. |
| Real V2 controller | crates/ethereum-contracts/src/PFTLUniswapPrimaryMarketV2.sol:4-55,73-105,120-210,218-250 | Reuse real PacketConsumed and ReturnBurned events, replay maps, mint accounting, and return burn ABI. |
| Real receipt verifier | crates/ethereum-contracts/src/PFTLReceiptFinalityVerifierV1.sol:8-30,170-215,246-260 | Reuse only with actual locally produced proof/public values. Test mock at crates/ethereum-contracts/test/PFTLReceiptFinalityVerifierV1.t.sol:13-20 is forbidden. |
| Receipt proof | crates/node/src/ethereum_receipt_proof_builder.rs:42-262; CLI crates/node/src/main_parts/cli_dispatch_parts/group_03.rs:1933-1955 | Reuse actual Anvil receipt trie reconstruction and proof artifact. |
| BFT checkpoint | crates/node/src/ethereum_checkpoint_signing.rs:68-202,205-273,337-358; CLI group_03.rs:1907-2008 | Reuse observe, vote, and certificate assembly. |
| Native verification | crates/execution/src/pftl_uniswap_ethereum_verification.rs:162-232,280-379 | Reuse exact event/receipt-root/certificate binding for native destination and return. |
| Mainnet work | scripts/a666-mainnet-prove-wallet-export.sh; scripts/a666-mainnet-accept-and-mint.py:146-291; scripts/a666-mainnet-record-destination-consume.sh:124-255; scripts/a666-mainnet-return-import.sh:70-198 | Reuse ordering/artifact semantics only. They have production remote assumptions and are not rehearsal executables. |
| Browser path | wallet-web/src/components/NavcoinPrimaryMarket.jsx:67-96,334-418,425-525; wallet-web/src/lib/navcoin-export-relay.js:36-121; wallet-web/src/lib/navcoin-return-relay.js:68-130 | Reuse controls, durable requests, polling, and return recovery. Provider guard needs local-only staging support. |
| Proxy path | wallet-proxy/server.js:1191-1192; wallet-proxy/navswap-persistence-http.js:3-7,1584-1630; wallet-proxy/navcoin-export-jobs.js:80-125,285-375; wallet-proxy/navcoin-return-jobs.js:79-125,280-405 | Reuse generic durable jobs and routes unchanged. Existing production drivers require live/remote infrastructure. |
| R4 local PFTL | scripts/a666-r4-rehearsal-setup:217-366; crates/node/tests/atomic_swap_local_six.rs:1691-1853 | Reuse six-validator relaunch/convergence only. |
| Current gap | docs/evidence/a666-public-reserve-product-20260803/browser/r4-pass1/journey-fire-control-preflight.json:101-118,195-206,228-250 | Confirms no candidate Ethereum backend, relay, or browser runner exists. |

No existing address, account, RPC URL, private path, or compiled bytecode is a reusable identity. The deployment manifest records only non-secret endpoint class, chain id, genesis fingerprint, addresses, runtime hashes, route id/digest, and artifact hashes.

## End-to-end staging design

### Isolated local resources

- Bind Anvil and all proxy/PFTL endpoints to loopback. Reject any non-loopback URI.
- Use chain id 1 only because current governed route/browser behavior requires it. Also require the run's Anvil genesis fingerprint and process identity, so chain id cannot be confused with mainnet.
- Generate EVM, threshold-certificate, and local PFTL rehearsal credentials into a per-run mode-0700 directory. Pass paths only; never log values, derived secret addresses, mnemonics, or private paths. Teardown removes all credentials, Anvil state, browser profile, jobs, and child processes.
- Import the signed checkpoint into a disposable root, then create a separate ephemeral route with deployment-derived addresses/code hashes. Do not mutate the preserved route.

### Contract and export path

1. Compile from source and deploy WrappedVenueNAVCoin, PFTLReceiptFinalityVerifierV1, and PFTLUniswapPrimaryMarketV2.
2. Bind the ephemeral PFTL route to deployment addresses, runtime hashes, chain id, route digest, and V2 packet schema.
3. Browser drives Deliver to MetaMask then Mint and export through NavcoinPrimaryMarket.
4. Existing browser client creates the existing durable export job.
5. New local export driver reads the actual PFTL export packet/source finality, produces proof/public values, calls verifier verifyAndAccept, then controller consumeMintOnly.
6. It requires actual receipt, PacketConsumed identity, consumed packet state, wrapped balance delta, and controller accounting delta.
7. It uses existing node Ethereum checkpoint observe, receipt proof build, six local votes, certificate assembly, and certified PftlUniswapDestinationConsume submission.
8. Export is accepted only after event-bound values and all six native states agree.

**Hard gate:** PFTLReceiptFinalityVerifierV1 must accept a local proof/public-values pair bound to the real local PFTL export. A mock SP1 verifier, synthetic acceptance, copied output, or setAccepted substitute is forbidden. Local proof feasibility is the first implementation spike; its failure is STOP RED.

### Return path

1. Browser drives From MetaMask and Return and redeem; it sends controller burnForPftlReturn and persists the existing recovery record.
2. Existing return client creates/polls the existing durable job.
3. New return driver validates actual ReturnBurned receipt/event, then uses existing node checkpoint/receipt-proof/vote/certificate flows and certified PftlUniswapReturnImport.
4. It accepts only after all six native states have the exact import, wrapped balance and controller outstanding amount fall by the exact amount, native balance returns by the exact amount, and conservation equality holds.
5. A completed burn/import is idempotent: no re-submit and stable receipt identities after proxy restart/browser reload.

## Exact planned changes

### Reused without modification

wallet-web/src/lib/navcoin-export-relay.js; wallet-web/src/lib/navcoin-return-relay.js; wallet-proxy/server.js; wallet-proxy/navswap-persistence-http.js; wallet-proxy/navcoin-export-jobs.js; wallet-proxy/navcoin-return-jobs.js; crates/node/src/ethereum_receipt_proof_builder.rs; crates/node/src/ethereum_checkpoint_signing.rs; crates/execution/src/pftl_uniswap_ethereum_verification.rs.

### wallet-proxy

| File | Change |
|---|---|
| wallet-proxy/a666-offline-ethereum-rehearsal-driver.js (new) | Config-validated readiness, inspect, and run-job entry points for generic export/return jobs. Enforce loopback/genesis/route/code-hash pins, actual event binding, idempotence, and no secret logging. |
| wallet-proxy/a666-offline-ethereum-receipt-adapter.js (new) | Orchestrate existing node proof/checkpoint commands and local certified finality; verify six-way convergence and write redacted artifacts. |
| wallet-proxy/test_a666-offline-ethereum-rehearsal-driver.js (new) | Configuration refusal, receipt/event mismatch, replay/idempotence, and egress refusal tests. |

Generic jobs/HTTP dispatch stay unchanged; their existing configuration selects the new driver.

### wallet-web

| File | Change |
|---|---|
| wallet-web/src/components/NavcoinPrimaryMarket.jsx | Replace unconditional provider guard with strict route-derived chain-id verification that allows only an explicit loopback rehearsal manifest; production remains chain id 1 only. Keep existing rendered controls and relay calls. |
| wallet-web/src/lib/a666-r4-offline-ethereum-rehearsal.e2e.js (new) | Durable Chromium test that injects only the local provider, drives rendered controls, and checks actual receipt/event/native states plus restart/reload. |
| wallet-web/package.json | Wire staging test into a required browser command. |
| wallet-web/src/components/NavcoinPrimaryMarket.test.jsx (new) | Production rejection of non-mainnet provider and explicit local-rehearsal acceptance. |

wallet-web/src/lib/navcoin-markets.js already propagates ethereumChainId from governed route data at lines 29-70; no change is planned.

### Required non-wallet support

- crates/ethereum-contracts/script/DeployA666OfflineRehearsal.s.sol (new): deterministic source-derived local V2 contract deployment and redacted manifest.
- scripts/a666-r4-offline-ethereum-rehearsal-setup (new): isolated roots, generated credentials, services, deploy, ephemeral route, proxy config, teardown/fire-control manifest.
- scripts/a666-r4-offline-ethereum-rehearsal-verify (new): recompute acceptance and reject unredacted evidence.
- crates/node/tests/atomic_swap_local_six.rs or a new candidate-scoped local-six fixture: create the isolated route before staging if the current harness cannot inject deployment-derived identities.

## Computed readiness and acceptance

ready_to_fire equals:
loopback endpoint AND matching Anvil genesis AND route-matching contract runtime hashes AND real verifier accepts actual local proof AND six RPC ready AND six terminal tuples equal AND both relays ready AND browser provider bound locally AND custody scan clean.

Final evidence must compute, never hand-set:

- PFTL native balances; wrapped balance; controller total minted, total returned, and outstanding amounts before/after.
- Export packet id; EVM receipt/log id; destination-consume native receipt id.
- Return burn id; EVM receipt/log id; return-import native receipt id.
- Conservation equality and six-validator height/tip/state-root equality.
- Candidate/artifact hashes, command, exits, timing, first failure, redacted capture/public receipt.

## Required negative tests

1. Non-loopback endpoint or chain-id match with wrong genesis.
2. Controller/token code-hash mismatch.
3. Mutated proof/public values rejected by the real verifier.
4. Wrong packet, recipient, amount, deadline, route digest, receipt, or log index.
5. Packet replay, return nonce replay, duplicate destination consume, and duplicate return import leave balances unchanged.
6. Under-quorum, duplicate, or altered checkpoint votes; altered receipt trie proof; premature finality.
7. Anvil loss or corrupted job artifact never reports success.
8. Actual proxy SIGTERM/restart plus Chromium reload preserves pending state, resumes only once, and preserves final identities.
9. Proxy/browser/storage/profile/evidence secret-boundary scan is clean.
10. Test fails if rendered controls, relay routes, or actual contract calls are bypassed.

## Candidate impact and requalification

Candidate product behavior changes. After implementation: regression-manifest qualification; all applicable 26 Rust validator regressions; wallet-web npm test, custody browser, public browser, build; exact journey-step-9 restart/reload; new Ethereum browser staging plus negatives; one ordered R4 fire-control run; independent review of deployment identity, verifier output, receipt proof, native certificate, conservation, and secret scans.

## Estimate

| Work | Estimate |
|---|---:|
| Real local proof feasibility spike | 1-2 engineer-days |
| Isolated route/generated test credential fixture | 1-2 engineer-days |
| Foundry/Anvil deployment identity | 1 day |
| Proxy driver and receipt adapter | 2-3 days |
| Browser guard and E2E | 1-1.5 days |
| Conservation/evidence/negative suite | 1.5-2 days |
| Planned rework cycle | 1.5 days |
| Review/qualification reserve | 1 day |

**Total:** 9-13 engineer-days. Critical path: real local proof -> isolated route binding -> proxy adapter -> browser E2E -> full requalification. Any need for mock acceptance, live dependency, or externally held credential is STOP RED.

## 2026-08-04 contract-stage update

Anvil adoption verified loopback-only chain 1 and a public deployer address. Contract staging is RED before deployment: the installed v6.1 SP1 verifier source imports ISP1Verifier.sol, but that dependency is absent from the installed circuit tree. No mock, product change, deployment, verifier acceptance, or business transaction was used. Proof acceptance remains RED at real_receipt_proof_artifact_unavailable_after_local_cpu_oom.

Future adapter contract remains unchanged: wallet-proxy durable export/return jobs stay the public boundary; a new content-addressed proof-slot adapter verifies proof, public-values, program-vkey, receipt, and route bindings asynchronously, polls at 5 seconds for a maximum 12-hour R4 rehearsal budget, never invokes a prover inline, and STOP-no-retry times out. Future product files remain wallet-proxy/a666-offline-ethereum-rehearsal-driver.js, wallet-proxy/a666-offline-ethereum-receipt-adapter.js, wallet-proxy/test_a666-offline-ethereum-rehearsal-driver.js, wallet-web/src/components/NavcoinPrimaryMarket.jsx, wallet-web/src/lib/a666-r4-offline-ethereum-rehearsal.e2e.js, wallet-web/src/components/NavcoinPrimaryMarket.test.jsx, and wallet-web/package.json; none changed in this stage.

## 2026-08-04 split-compiler interface increment

The installed real Groth16 verifier is v6.1.0 and imports both ISP1Verifier and ISP1VerifierWithHash. The required interface was vendored unchanged from official succinctlabs/sp1-contracts tag v6.1.0 commit 2ac5ecbbe473421a963d67e55f182e9a36576f7c, at scripts/a666-r4-ethereum-contract-vendor/sp1-contracts-v6.1.0/ISP1Verifier.sol. Its SHA-256 is 9e918032a5aa799c1319b14b013154d6a40ca6e5f2267c9f540560abd7fd7689; the installed verifier SHA-256 is 48e1db5baca3b102242ebd88280b3689a088076688146cd0d98876f5dacb76d0. The staging script compiles the real SP1 source with exact Solidity 0.8.20 and the unmodified product sources with 0.8.24.

The real SP1 verifier deployed to the adopted loopback Anvil and returned VERSION() = v6.1.0 plus the expected Groth16 verifier hash. The product staging script then stopped before deploying WrappedVenueNAVCoin, PFTLReceiptFinalityVerifierV1, or PFTLUniswapPrimaryMarketV2: this Foundry build rejected the generated Vm.computeCreateAddress selector 0x019a2f5e as unknown. No mock, product change, private credential, proof fabrication, verifyAndAccept call, or business-value transaction occurred. Proof acceptance remains RED at real_receipt_proof_artifact_unavailable_after_local_cpu_oom; the selector compatibility wall also blocks the remaining real product-stack deployment.

## 2026-08-04 external-create correction

The new fresh durable loopback Anvil uses the same chain ID but a distinct genesis fingerprint. The staging script removes every Vm and Forge-script dependency. It uses external forge create through the public unlocked Anvil account and uses external cast compute-address only once, for the approved cyclic receipt.controller binding. It records the exact nonce progression and rejects an actual primary address that differs from that external prediction.

This first external-create run stopped before any transaction because the generic deploy helper passed an empty --constructor-args flag to the zero-argument SP1 constructor. The error is a Foundry CLI invocation wall, not a Solidity, proof, route, or contract acceptance result. The fresh Anvil remains active at height zero; it has no staged contracts or business transactions. No retry occurred. Proof acceptance is RED pending real_receipt_proof_artifact_pending_docker_capable_refire; deploy correction remains separately RED until all four real contract receipts and wiring readbacks exist.

## 2026-08-04 empty-constructor closeout

The helper now omits --constructor-args for a zero-argument constructor. The required single stage run passed that CLI point and deployed the real SP1 v6.1.0 verifier with a successful receipt. It then stopped before any product deployment because cast call renders the VERSION string as quoted JSON text, while the shell assertion compares it to an unquoted value. This is an exact readback-format wall; no retry occurred. The durable Anvil now has one real SP1 deployment, block height one, and no WrappedVenueNAVCoin, receipt verifier, primary controller, proof call, or business-value transaction. Deployment remains RED; proof acceptance remains independently RED pending real_receipt_proof_artifact_pending_docker_capable_refire.

## Path B ordering and pre-prove binding gate

Path B has one mandatory order: deploy the real verifier/token/receipt/controller stack, generate the witness from that deployed state, run the pre-prove binding check, generate the proof only after that check is GREEN, then call the real verifier exactly once. Any changed deployment identity, witness, decoded field, or pre-prove result is STOP-no-retry; generate a new witness from the then-current deployment and repeat the gate before proving. A proof generated before the matching pre-prove result is ineligible for verifier submission.

The pre-prove artifact is r4-construction/pre-prove-binding-check.json, schema postfiat.a666.r4.path-b-pre-prove-binding-check.v1. Its mandatory identity fields are candidate revision 39f7fae3191aa34c376ae1657650a9ec2444f421, nonempty witness SHA-256, deployment evidence path, deployment commit, and decoder SHA-256 6fb2fb9ac693e5cea787eeb4a80701de562d3b12ce47d837d121d8dc361f6d4a. It must report decoded_field_count=35, conjunction_field_count=35, exact_mismatch_count=0, and all_match=true.

The 35 ordered fields are: (1) proofProgramVersion; (2) pftlChainIdHash; (3) pftlGenesisHashCommitment; (4) pftlProtocolVersion; (5) committeeRootCommitment; (6) committeeTransitionCommitment; (7) finalizedBlockCommitment; (8) finalizedStateRootCommitment; (9) routeEpoch; (10) policyHashCommitment; (11) routeIdCommitment; (12) routeTrustClass; (13) routeConfigDigestCommitment; (14) nativeNavAssetIdCommitment; (15) settlementAssetIdCommitment; (16) pricingNavEpoch; (17) pricingReservePacketHashCommitment; (18) sourceWalletCommitment; (19) sourceReceiptRootCommitment; (20) sourceReceiptHashCommitment; (21) acceptedReceiptCode; (22) packetDigest; (23) destinationChainId; (24) controller; (25) wrappedToken; (26) recipient; (27) mintAmountAtoms; (28) settlementValueAtoms; (29) packetNonce; (30) deadline; (31) sourceHeight; (32) priorCheckpointCommitment; (33) resultingCheckpointCommitment; (34) finalizedHeight; (35) proofNullifier.

The pre-prove conjunction must explicitly report true for prior_checkpoint_matches_initial, route_matches, controller_matches, wrapped_matches, chain_matches, genesis_matches, protocol_matches, and program_vkey_matches. Both prior_checkpoint_commitment and initial_checkpoint_commitment must be nonempty. It also requires live_chain=false, business_value=0, and stakehub=false. The fire-control aggregate consumes this evidence mechanically; an absent, malformed, stale, or mismatched record leaves pre_prove_all_35_bindings=false and blocks proving.
