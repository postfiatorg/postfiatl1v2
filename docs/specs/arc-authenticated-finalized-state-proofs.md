# Arc Authenticated Finalized State Proofs

**Status:** Grant implementation specification

**Date:** 2026-09-02

**Proposed upstream:** `circlefin/arc-node`

**Reference integration:** PostFiat Arc/pfUSDC current-v2 branch

**License:** Apache-2.0-compatible upstream contribution

## 1. The fix

Add authenticated historical state proofs to Arc's public RPC path.

Concretely, an Arc archive RPC node MUST:

1. serve standard EIP-1186 `eth_getProof` responses at an exact finalized block;
2. accept a block-hash selector so a client cannot silently receive a proof for
   a different height or fork;
3. expose the proof-retention window and request limits;
4. fail explicitly when the requested state has been pruned; and
5. return the block header, Arc commit certificate, and EIP-1186 proofs in one
   Arc-specific finalized proof bundle.

This is the missing Arc capability. It is useful to any bridge, light client,
indexer, or ZK verifier that must authenticate Arc contract state. It is not an
AI/reputation project, a PostFiat consensus change, or a request to weaken proof
verification.

## 2. Why Arc should fund it

Arc already exposes two pieces that external verifiers need:

- Ethereum-compatible block and receipt RPCs; and
- `arc_getCertificate(height)`, which returns Arc's BFT commit certificate.

The public testnet endpoint does not currently expose `eth_getProof`. A commit
certificate authenticates a finalized block hash, but it does not by itself
prove the value returned by an arbitrary `eth_call` or
`eth_getStorageAt`. Consequently, a trust-minimized client can prove that a
deposit receipt is in an Arc block but cannot prove contract state, including
the validator registry, under that block's `stateRoot`.

The result is a specific ecosystem gap: applications must trust an RPC
operator's asserted state precisely where a standard Merkle-Patricia proof
could remove that trust. Funding this work gives Arc a reusable, documented
proof interface instead of a PostFiat-only workaround.

## 3. Existing evidence and exact blocker

The existing Arc/pfUSDC integration already verifies:

- canonical Arc header RLP and block hash;
- an Arc commit certificate and its quorum;
- receipt-trie inclusion of an Arc vault deposit;
- the vault, token, route, amount, recipient, nonce, and deposit identifier; and
- the input and output Arc validator-set commitments.

The current-v2 guest additionally requires an authenticated proof of the
post-block validator registry. That requirement is intentional. Without it, a
relayer could invent the next validator set and eventually choose the keys that
the light client trusts.

The current capture path requests:

```text
eth_getProof(
  ValidatorRegistryProxy,
  [
    ERC1967_IMPLEMENTATION_SLOT,
    activeValidatorArrayLengthSlot,
    activeValidatorArrayEntries...,
    validatorRegistrationStructSlots...,
    validatorPublicKeyDynamicDataSlots...
  ],
  exactBlock
)

eth_getProof(
  ValidatorRegistryImplementation,
  [],
  exactBlock
)
```

The tracked Arc testnet observation returned JSON-RPC `-32601 method not
supported`. The archived v1 ingress witness therefore cannot be upgraded into
a valid v2 witness: it has no authenticated registry proof. Reusing a registry
proof from another block is invalid because its state root differs.

Relevant evidence:

- `docs/evidence/arc-mvp-20260828/transplant-report.md`
- `docs/evidence/arc-mvp-20260828/arc-validator-transition-event.json`
- `docs/evidence/arc-mvp-20260828/arc-validator-state-59374844.json`
- `docs/evidence/arc-mvp-20260828/current-main-integration-20260901.md`
- `tools/pfusdc-tier4-prover/src/arc_ingress_capture.rs`
- `programs/pfusdc-arc-ingress/src/lib.rs`

## 4. Goals

### 4.1 Required

1. Provide standard, independently verifiable EIP-1186 account and storage
   proofs for finalized Arc blocks.
2. Support historical proofs on a dedicated archive RPC node without imposing
   archive storage on consensus validators.
3. Bind every response to the caller's exact block hash or block number.
4. Make retention and limits machine-readable.
5. Ship upstream code, integration tests, operator documentation, test vectors,
   and an independent verifier.
6. Demonstrate one current Arc testnet deposit proof through the current-v2 SP1
   guest on an H100.
7. Demonstrate validator-set continuity for both a no-change block and a
   controlled validator transition.

### 4.2 Non-goals

- Changing Arc consensus, certificate rules, or the validator registry.
- Making Arc validators run archive nodes.
- Replacing Arc finality with an oracle, multisig, or PostFiat assertion.
- Trusting `eth_call` as proof of state.
- Weakening the current-v2 guest to accept a relayer-asserted validator set.
- Deploying a production asset bridge or representing the result as a mainnet
  security audit.
- Any AI scoring or reputation work.

## 5. Architecture

### 5.1 Deployment boundary

Run the proof service on a non-validating Arc RPC node:

```text
Arc consensus network
        |
        v
sync-only consensus layer (--no-consensus)
        |
        +---- arc_getCertificate
        |
archive execution layer (no --full or --minimal pruning preset)
        |
        +---- eth_getBlockByHash / eth_getBlockReceipts
        +---- eth_getProof
        +---- arc_getProofCapabilities
        +---- arc_getFinalizedProofBundle
```

The execution node retains historical trie data. The colocated sync-only
consensus layer supplies certificates. The public load balancer applies rate,
request-size, response-size, and concurrency limits. Consensus validators do
not serve proof-generation traffic.

### 5.2 Trust model

The RPC response is untrusted data. A client independently:

1. hashes the canonical header and matches the requested block hash;
2. verifies the Arc certificate against the validator set authorized for that
   height;
3. verifies receipt inclusion against `receiptsRoot`;
4. verifies account and storage proof nodes against `stateRoot`;
5. verifies the registry proxy code hash and ERC-1967 implementation pointer;
6. verifies the implementation account code hash;
7. decodes the proven registry slots into the next active validator set; and
8. rejects any missing, malformed, inconsistent, non-canonical, or oversized
   field.

The server need not be trusted for correctness. It remains trusted for
availability.

## 6. RPC contract

### 6.1 Standard method: `eth_getProof`

Arc MUST expose EIP-1186 `eth_getProof` in the existing `eth` namespace.

Request:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "eth_getProof",
  "params": [
    "0x3600000000000000000000000000000000000002",
    ["0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc"],
    {
      "blockHash": "0xBLOCK_HASH",
      "requireCanonical": true
    }
  ]
}
```

The method MUST also accept a canonical hex block quantity for compatibility.
Security-sensitive clients SHOULD use the EIP-1898 block-hash form.

Response fields MUST follow EIP-1186:

```json
{
  "address": "0x...",
  "accountProof": ["0x..."],
  "balance": "0x0",
  "codeHash": "0x...",
  "nonce": "0x...",
  "storageHash": "0x...",
  "storageProof": [
    {
      "key": "0x...",
      "value": "0x...",
      "proof": ["0x..."]
    }
  ]
}
```

Required behavior:

- the proof MUST verify against the selected header's exact `stateRoot`;
- output storage proofs MUST correspond one-for-one to the deduplicated request
  keys;
- an absent account or zero-valued storage slot MUST return a valid
  non-inclusion proof, not an omitted proof;
- `latest` MAY be supported for compatibility but MUST NOT be used by the
  finalized bundle;
- `pending` MUST be rejected by the finalized proof path;
- the server MUST NOT substitute a newer block when old state is unavailable;
- a non-canonical block hash, unknown block, pruned state, excessive key count,
  and excessive generated response MUST be distinguishable failures; and
- HTTP and WebSocket behavior MUST be identical.

### 6.2 Capability method: `arc_getProofCapabilities`

This small method lets clients determine whether a node can satisfy a request
before constructing a large proof query.

Request:

```json
{"jsonrpc":"2.0","id":1,"method":"arc_getProofCapabilities","params":[]}
```

Response schema:

```json
{
  "version": "arc-proof-capabilities-v1",
  "enabled": true,
  "historical": true,
  "blockHashSelector": true,
  "earliestAvailableBlock": "0x...",
  "latestFinalizedBlock": "0x...",
  "maxAccounts": 4,
  "maxStorageKeysPerAccount": 2048,
  "maxResponseBytes": 16777216
}
```

`earliestAvailableBlock` is an availability claim, not a consensus claim. It
MUST be computed from the execution node's actual proof-capable state, updated
after pruning or restore, and confirmed by a probe before publication.

### 6.3 Atomic method: `arc_getFinalizedProofBundle`

This method is an Arc convenience API. It removes client races and round trips;
it does not introduce a new proof format.

Request:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "arc_getFinalizedProofBundle",
  "params": [{
    "block": {
      "blockHash": "0xBLOCK_HASH",
      "requireCanonical": true
    },
    "accounts": [
      {
        "address": "0x3600000000000000000000000000000000000002",
        "storageKeys": ["0x..."]
      },
      {
        "address": "0xIMPLEMENTATION",
        "storageKeys": []
      }
    ]
  }]
}
```

Response schema:

```json
{
  "version": "arc-finalized-proof-bundle-v1",
  "chainId": "0x...",
  "blockNumber": "0x...",
  "blockHash": "0x...",
  "stateRoot": "0x...",
  "receiptsRoot": "0x...",
  "headerRlp": "0x...",
  "certificate": {},
  "accountProofs": []
}
```

`certificate` MUST be the same canonical data returned by
`arc_getCertificate(blockNumber)`. Each item in `accountProofs` MUST be a
standard EIP-1186 response. Before replying, the server MUST verify internally
that:

- the header hashes to `blockHash`;
- the block height matches the certificate request;
- the certificate's committed block matches `blockHash`;
- every proof was generated for `stateRoot`; and
- the block remains canonical and finalized.

Clients MUST still verify all of those conditions independently.

### 6.4 Stable failure classes

The Arc-specific methods MUST return stable, documented error codes:

| Code | Meaning |
| --- | --- |
| `-32602` | Invalid parameters or malformed block selector |
| `-32061` | Block is known but not finalized |
| `-32062` | Historical state is unavailable or pruned |
| `-32063` | Account, key, computation, or response limit exceeded |
| `-32064` | Block is unknown or no longer canonical |
| `-32065` | Certificate unavailable for the selected finalized block |
| `-32603` | Internal proof generation or consistency failure |

Errors MUST NOT include internal paths, peer addresses, credentials, or raw
database diagnostics.

## 7. Bounds and abuse resistance

Default public limits:

| Resource | Default |
| --- | ---: |
| Accounts per bundle | 4 |
| Storage keys per account | 2,048 |
| Total storage keys per bundle | 2,048 |
| Generated response | 16 MiB |
| Concurrent proof jobs per process | 4 |
| Request timeout | 30 seconds |
| JSON-RPC batch entries | Existing Arc default, currently 100 |

The PostFiat validator-registry witness uses at most
`2 + 5 * validator_count` storage keys. Its guest supports at most 256
validators, or 1,282 registry keys, so the 2,048-key server default covers the
bounded use case without creating an unbounded endpoint.

Implementation requirements:

- validate account and key counts before database work;
- deduplicate keys deterministically;
- charge rate limits by requested keys, not only request count;
- bound proof nodes, node bytes, aggregate response bytes, wall time, and
  concurrent jobs;
- stop work when the client disconnects or the deadline expires;
- expose latency, failure-class, response-size, and active-job metrics without
  logging proof contents; and
- preserve Arc's existing public-RPC restrictions.

## 8. Retention and operator behavior

The grant reference deployment MUST use an archive execution profile. Arc's
documented `--full` profile retains only a bounded history; it is not assumed
to provide the historical trie data required by this interface.

Operators MUST:

1. restore an archive-compatible snapshot or sync archive state;
2. start the execution layer without `--full` or `--minimal`;
3. run the consensus layer in sync-only mode with `--no-consensus`;
4. enable the Arc RPC extension and proof RPC;
5. bind the consensus-layer upstream to loopback or a private socket;
6. expose only `eth,net,web3,rpc` plus Arc's automatically registered methods;
7. apply the bounds in Section 7;
8. probe the first and last advertised proof-capable blocks; and
9. monitor disk growth, proof latency, failures, and certificate availability.

A startup check MUST fail closed if proof RPC is enabled with a pruning profile
that cannot meet the configured retention promise. Changing pruning profiles
MUST invalidate and recompute the advertised window.

## 9. Implementation plan

### Work package 1 — Execution proof support

Likely owning surfaces:

- Arc's Reth-based execution-node construction under
  `external/arc-node/crates/evm-node`;
- RPC registration/configuration under `external/arc-node/crates/node`; and
- upstream Reth proof/provider traits where Arc's customized node does not
  currently satisfy them.

Deliverables:

- identify why `eth_getProof` is absent from the Arc `eth` module;
- implement or enable account/storage proof generation against exact historical
  state;
- support block-number and EIP-1898 block-hash selectors;
- add CLI configuration and public-safe defaults; and
- add standard EIP-1186 conformance tests.

### Work package 2 — Arc finality binding

Owning surfaces:

- `external/arc-node/crates/evm-node/src/rpc/arc.rs`;
- `external/arc-node/crates/evm-node/src/rpc/get_certificate.rs`; and
- the existing execution-to-consensus RPC boundary.

Deliverables:

- `arc_getProofCapabilities`;
- `arc_getFinalizedProofBundle`;
- exact header/certificate/proof consistency checks;
- stable errors and response bounds; and
- cache policy: immutable finalized bundles MAY be cached by block hash.

### Work package 3 — Independent verification and fixtures

Deliverables:

- an independent verifier that consumes only the RPC response;
- canonical positive fixtures for inclusion, non-inclusion, multiple storage
  keys, proxy implementation lookup, and validator-registry decoding;
- negative fixtures for every mutated root, node, key, value, address, code
  hash, header, block hash, certificate, and response-bound violation; and
- fixture hashes and reproduction commands.

The independent verifier MUST NOT call `eth_call` to decide whether a proof is
valid.

### Work package 4 — Public testnet and ZK demonstration

Deliverables:

- a sync-only archive RPC node on Arc testnet;
- a fresh Arc vault deposit;
- a v2 witness captured at the deposit's exact finalized block;
- native verification of the complete witness;
- an SP1 Groth16 proof generated on an H100;
- local proof verification and byte-exact public-value comparison;
- a controlled validator-transition proof plus a no-change proof; and
- a redaction-safe result packet with source revisions, commands, hashes,
  timings, and GPU termination receipt.

The demo MUST use the current-v2 guest. A successful historical-v1 proof is a
hardware/toolchain control, not completion of this grant.

## 10. Acceptance tests

### 10.1 Standard RPC

- [ ] `rpc_modules` advertises `eth` and the Arc extension.
- [ ] `eth_getProof` returns a valid account proof with zero storage keys.
- [ ] It returns valid proofs for one, many, duplicate, zero-valued, and absent
      storage keys.
- [ ] Proofs requested by number and canonical block hash are byte-equivalent
      after canonical response normalization.
- [ ] An independent verifier reconstructs the selected header's exact
      `stateRoot`.
- [ ] Unknown, non-canonical, unfinalized, and pruned blocks fail distinctly.
- [ ] No failure silently falls back to `latest`.
- [ ] HTTP and WebSocket results agree.
- [ ] Restart and archive-snapshot restore preserve proof correctness.

### 10.2 Finalized bundle

- [ ] Header RLP hashes to the returned block hash.
- [ ] The Arc certificate authenticates that same block and height.
- [ ] Every EIP-1186 proof verifies against the returned state root.
- [ ] A mutation to any bound field is rejected.
- [ ] Pending-block requests are rejected.
- [ ] Account/key/byte/concurrency/time limits fail before unbounded work.
- [ ] Capability bounds match enforced bounds.
- [ ] Load tests do not starve block sync or certificate service.

### 10.3 Validator registry

- [ ] The proxy account proof verifies against `stateRoot`.
- [ ] The ERC-1967 implementation slot is proven.
- [ ] The implementation account and pinned code hash are proven.
- [ ] Active-array length, registration IDs, addresses, public keys, and voting
      powers are all derived from proven slots.
- [ ] Zero-power entries are excluded.
- [ ] The signing set for height H is read with Arc's H-1 semantics.
- [ ] The post-state set at H becomes the commitment used for the next block.
- [ ] A controlled real set change verifies; a relayer-invented change fails.
- [ ] The no-change case still requires and verifies a state proof.

### 10.4 Current-v2 H100 gate

- [ ] A fresh deposit and registry proof share the exact block hash and roots.
- [ ] Native execution succeeds before paid proving begins.
- [ ] The checked ELF and verifying-key hashes are recorded.
- [ ] The H100 prover reports the selected GPU and completes Groth16 proving.
- [ ] Local SP1 verification succeeds.
- [ ] Public values are byte-identical to native execution.
- [ ] Tampered witness/proof/public-value cases fail.
- [ ] The rented GPU is destroyed after evidence retrieval.

## 11. Performance targets

These are grant acceptance targets for the reference testnet archive node, not
mainnet SLAs:

| Operation | Target |
| --- | ---: |
| One account, no storage keys, warm p95 | <= 1 second |
| One account, 128 storage keys, warm p95 | <= 3 seconds |
| One account, 1,282 storage keys, warm p95 | <= 15 seconds |
| Finalized bundle, 128 storage keys, warm p95 | <= 5 seconds |
| Error on a request rejected by static bounds | <= 100 ms |
| Proof correctness under 4 concurrent jobs | 100% |
| Sync progress under bounded proof load | No sustained stall |

Benchmarks MUST report machine type, Arc revision, database size/profile, block
age, cache condition, sample count, p50/p95/max, response sizes, and error count.

## 12. Milestones and grant tranches

| Milestone | Duration | Payment | Objective evidence |
| --- | ---: | ---: | --- |
| M1: design and local proof path | 2 weeks | 20% | Root-cause note, accepted API review, local EIP-1186 conformance fixture |
| M2: upstream implementation | 3 weeks | 30% | Arc-node PR, block-hash historical proofs, bounds, unit/integration/adversarial tests |
| M3: reference deployment | 2 weeks | 25% | Testnet archive endpoint, capability probe, operator runbook, independent verifier and fixtures |
| M4: ZK completion | 1 week | 25% | Fresh v2 witness, controlled transition evidence, H100 Groth16 proof, reproducible final report |

Total planned duration: **8 weeks**.

Each tranche is earned by the listed, reproducible evidence—not by prose,
research time, an archived v1 result, or an unmerged draft.

## 13. Definition of done

The project is complete only when all of the following are true:

1. an upstreamable Arc-node implementation exists;
2. a public Arc testnet archive RPC returns bounded historical EIP-1186 proofs;
3. an independent verifier validates those proofs against finalized Arc
   headers;
4. the finality bundle binds the header, certificate, and state proofs to one
   block;
5. positive and adversarial tests pass;
6. a fresh Arc deposit produces a complete current-v2 witness;
7. that witness generates and locally verifies an H100 SP1 Groth16 proof;
8. exact source, program, key, fixture, public-value, and proof hashes are
   published; and
9. documentation clearly states that this is testnet proof infrastructure, not
   a production bridge audit or mainnet-readiness claim.

The grant's concrete outcome is: **an external application can cryptographically
verify finalized Arc contract state without trusting the Arc RPC operator.**
