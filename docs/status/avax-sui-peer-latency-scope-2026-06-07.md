# AVAX and Sui Peer Latency Evidence Scope

Date: 2026-06-07 UTC
Status: scope for goal execution
Owner: benchmark worker / tmux injector
Related blog: `postfiatorg.github.io/content/blog/postfiat-l1v2-private-xrpl-latency-benchmark.md`
Primary Post Fiat evidence: `docs/status/real-transaction-latency-benchmark-plan-2026-06-07.md`

## Goal Directive

When this document is used as `/goal` work, the worker should keep going until
the acceptance criteria are met, a real blocker is documented, or the operator
explicitly stops the run. Do not stop after writing scripts, running one smoke
test, or producing partial numbers. The expected output is a reproducible peer
latency evidence packet that can inform the current public latency article.

The immediate objective is not a marketing claim. The objective is to answer:

```text
Where does Post Fiat L1 v2's current real-transfer latency sit against two
mature low-latency peer designs when each is measured with clearly labeled local
or public-network transaction-finality semantics?
```

## Current State

The live latency article currently discusses a private XRPL control and an older
Post Fiat packet:

```text
postfiatorg.github.io/content/blog/postfiat-l1v2-private-xrpl-latency-benchmark.md
```

That article's headline Post Fiat number is the older 1000-transfer packet:

```text
submit_to_finality p50 ~= 182.951 ms
```

The newer real signed-transfer benchmark in this repo is cleaner and should be
treated as the current local Post Fiat baseline:

```text
full vote, 6 validators, 1000 real signed transfers:
  wallet_to_finality_ms p50 = 89.061525
  wallet_to_finality_ms p95 = 105.776512
  wallet_to_finality_ms p99 = 117.092231

quorum-fast, 6 validators, 1000 real signed transfers:
  wallet_to_finality_ms p50 = 84.277622
  wallet_to_finality_ms p95 = 100.484681
  wallet_to_finality_ms p99 = 105.198309
```

The evidence packet is:

```text
reports/testnet-real-transaction-latency-benchmark/evidence-packet-20260607/manifest.json
SHA-256: d72ad8a6b4a4ddc4d0643ec8b5bf9124514082aa2a408041615c274cf0cc5fe5
```

The safety companion is:

```text
reports/testnet-finality-chaos-gate/real-tx-latency-20260607/testnet-finality-chaos-gate.json
```

That gate passed 9/9 adversarial finality cases with `residual_work=[]`.

This new peer-latency work should extend the article's benchmark context. It
should not silently replace protocol concepts with false equivalences.

## Why AVAX and Sui

The XRPL control is historically relevant because Post Fiat has XRPL lineage.
It is not the cleanest low-latency architectural peer.

AVAX and Sui are useful for different reasons:

| Peer | Why It Matters | Main Trap |
|---|---|---|
| Avalanche C-Chain / Avalanche L1 | Mature subsecond EVM-style settlement surface with local and public RPC tooling. Easy to submit signed value transfers and wait for receipts. | Public RPC latency is not protocol latency; local C-Chain/L1 config is not mainnet. |
| Sui | Mature object-based architecture with explicit owned-object fast path and shared-object consensus path. Useful for understanding whether Post Fiat should add a non-conflicting transfer fast path. | A simple SUI transfer is not the same as shared-object consensus. Owned-object and shared-object lanes must be reported separately. |

This is most useful as an engineering design study:

```text
AVAX tells us what an EVM-style low-latency peer looks like.
Sui tells us how much latency can be removed when non-conflicting transfers avoid full consensus.
Post Fiat tells us where our account/balance certified-finality path currently sits.
```

## Claim Boundaries

Allowed claims after successful completion:

- local Post Fiat real signed-transfer latency compared with local AVAX and
  local Sui controls;
- public endpoint smoke-test latency for AVAX and Sui if run and labeled as
  public endpoint observations;
- owned-object Sui latency vs shared-object Sui latency as separate lanes;
- engineering implications for Post Fiat, especially whether a non-conflicting
  transfer fast path is worth designing.

Disallowed claims:

- "Post Fiat beats Sui";
- "Post Fiat beats Avalanche";
- "Post Fiat mainnet will be faster than AVAX/Sui mainnet";
- public RPC latency presented as validator consensus latency;
- Sui owned-object fast-path transfer presented as equivalent to shared-object
  consensus;
- local AVAX/Sui devnet results presented as production network results.

## Source Anchors

Use official sources first. Record retrieval date in the final packet.

Avalanche:

- Primary Network and C-Chain RPC shape:
  `https://docs.avax.network/docs`
- Avalanche CLI command reference:
  `https://docs.avax.network/docs/tooling/avalanche-cli/cli-commands`
- Local Avalanche L1 deployment:
  `https://docs.avax.network/docs/tooling/avalanche-cli/create-deploy-avalanche-l1s/deploy-locally`
- API endpoint shape:
  `https://docs.avax.network/docs/rpcs/other/guides/issuing-api-calls`

Sui:

- Sui documentation:
  `https://docs.sui.io/`
- Sui CLI cheat sheet:
  `https://www.docs.sui.io/doc/sui-cli-cheatsheet.pdf`
- Sui architecture / owned vs shared object protocol:
  `https://docs.sui.io/doc/sui.pdf`
- Sui Lutris paper:
  `https://docs.sui.io/paper/sui-lutris.pdf`
- Mysticeti paper:
  `https://docs.sui.io/paper/mysticeti.pdf`

## Evidence Packet Target

Create:

```text
reports/peer-l1-latency-benchmark/avax-sui-YYYYMMDDTHHMMSSZ/
```

If promoted publicly later, mirror the sanitized packet to:

```text
postfiatorg.github.io/static/benchmarks/postfiat-peer-l1-latency-avax-sui-YYYYMMDDTHHMMSSZ/
```

Required packet contents:

| Artifact | Purpose |
|---|---|
| `README.md` | human-readable summary, claim boundary, headline table |
| `manifest.json` | run matrix, versions, host metadata, commands, source links |
| `commands.sh` | exact command log |
| `raw/postfiat/` | copied Post Fiat baseline reports or fresh reruns |
| `raw/avax/` | unedited AVAX raw run reports |
| `raw/sui/` | unedited Sui raw run reports |
| `aggregate.json` | normalized machine-readable statistics |
| `aggregate.md` | readable comparison tables |
| `session-summary.csv` | per-session summaries |
| `methodology.md` | endpoint/finality semantics and exclusions |
| `lab-book.md` | chronological run log, blockers, decisions |
| `SHA256SUMS.txt` | hash manifest for all public artifacts |

Do not include private keys, mnemonic phrases, generated wallet seeds, local node
databases, faucet tokens, or transient validator material.

## Benchmark Matrix

Run the following lanes.

| Lane | Network | Count | Required? | Notes |
|---|---|---:|---|---|
| `postfiat_full_local` | local controlled Post Fiat, 6 validators, full vote | 1000 | yes | Use current report if no code changed; rerun if benchmark code changed. |
| `postfiat_quorum_fast_local` | local controlled Post Fiat, 6 validators, quorum-fast | 1000 | yes | Use current report if no code changed; rerun if benchmark code changed. |
| `avax_local_c_chain_transfer` | local Avalanche C-Chain or local Avalanche L1 EVM | 1000 | yes | Signed native AVAX-style EVM transfers, wait for receipt/finality surface. |
| `avax_public_fuji_transfer` | Fuji public RPC | 100-1000 | optional | Public endpoint smoke only; faucet availability may limit count. |
| `sui_local_owned_transfer` | local Sui network | 1000 | yes | Owned-object/coin transfer fast path. |
| `sui_local_shared_object_tx` | local Sui network | 1000 | yes | Shared-object Move call that forces consensus path. |
| `sui_public_testnet_owned_transfer` | Sui testnet fullnode | 100-1000 | optional | Public endpoint smoke only; faucet availability may limit count. |

Minimum article-grade local packet:

```text
1000 successful local transactions per required lane.
3 independent sessions per required peer lane.
```

Preferred packet:

```text
5 independent sessions per required peer lane.
1000 successful local transactions per session.
```

If time is constrained, do not reduce the Post Fiat baseline. Reduce optional
public endpoint smoke tests first.

## Normalized Metrics

Every lane must emit the same normalized metric names where possible:

| Metric | Starts | Stops | Applies To |
|---|---|---|---|
| `client_submit_to_finality_ms` | client sends signed transaction to local/public RPC | client observes final transaction result/effects/receipt | all lanes |
| `signed_tx_to_finality_ms` | signed transaction bytes are ready | client observes final transaction result/effects/receipt | all lanes where signing is controlled |
| `admitted_to_finality_ms` | local node accepts transaction | final result/effects/receipt is available | Post Fiat; peer lanes only if observable |
| `consensus_path_ms` | consensus-relevant sequencing begins | final consensus result is known | Post Fiat and Sui shared-object lane if observable |

Report per lane:

- count;
- failures;
- min;
- max;
- mean;
- standard deviation;
- p50;
- p95;
- p99;
- first tenth p50;
- final tenth p50;
- final/first p50 ratio;
- raw transaction IDs/digests/hashes;
- finality or receipt object used for confirmation.

## Finality Semantics Table

The packet must include a table like this and fill in exact observed mechanics:

| System | Measured endpoint | What "final" means in this packet | Caveat |
|---|---|---|---|
| Post Fiat L1 v2 | `wallet_to_finality_ms` from `tx-latency-benchmark` | native transfer appears in certified and locally applied batch with transaction-specific finality receipt | local controlled-testnet, not WAN |
| AVAX local C-Chain/L1 | signed EVM tx submit to receipt / accepted final block status | native EVM transfer has a receipt on the local Avalanche chain | not mainnet; RPC receipt semantics must be documented |
| AVAX Fuji/public | public RPC submit to receipt | public endpoint observed a confirmed transaction receipt | includes faucet/public RPC/network noise |
| Sui local owned | `sui_executeTransactionBlock` or CLI equivalent to final effects | owned-object transfer has signed effects/finality result | this is the Sui fast path, not shared-object consensus |
| Sui local shared | shared-object Move call to final effects | shared-object transaction has final effects after consensus sequencing | requires a small benchmark Move package |
| Sui public/testnet | public fullnode submit to final effects | public endpoint observed final effects | includes faucet/public RPC/network noise |

## AVAX Workstream

### AVAX Setup

1. Check whether `avalanche`, `avalanchego`, `foundry`, `cast`, `node`, and
   `npm` are installed.
2. If not installed, install tooling in a local, documented way. Record versions.
3. Create or reuse a local Avalanche C-Chain/L1 deployment with fresh state.
4. Record:
   - Avalanche CLI version;
   - AvalancheGo version;
   - VM/coreth/subnet-evm version if applicable;
   - chain ID;
   - RPC URL;
   - block/consensus timing config;
   - funding key policy.

### AVAX Benchmark Runner

Implement one of:

```text
scripts/peer-latency-avax-local
scripts/peer-latency-avax-public
```

Preferred implementation:

- persistent process;
- no shell subprocess inside the timed loop;
- sign transactions with a local key;
- submit raw signed transactions over JSON-RPC;
- poll or subscribe until receipt/finality condition is met;
- write JSONL per iteration;
- emit a normalized summary JSON.

Acceptable first implementation:

- Node/TypeScript using `ethers`;
- Rust if existing dependencies make it easy;
- shell only for provisioning and final artifact assembly, not per-round timing.

### AVAX Acceptance

The AVAX local lane is accepted only if:

- 1000 successful signed transfers complete;
- every transfer has a transaction hash and receipt/finality observation;
- final sender/recipient balances match expected deltas within fee accounting;
- no private key is copied into the public packet;
- report includes p50/p95/p99 and raw JSONL;
- command log and version metadata are captured.

## Sui Workstream

### Sui Setup

1. Check whether `sui` and `sui-faucet` binaries are installed.
2. If not installed, build or install Sui tooling in a documented way.
3. Start a local Sui network using official local-network tooling.
4. Use local genesis/faucet gas. No real SUI is required for the local benchmark.
5. Record:
   - Sui CLI version;
   - git tag/commit if built from source;
   - local network config;
   - RPC URL;
   - validator count if visible;
   - protocol version;
   - gas price and budget assumptions.

### Sui Owned-Object Lane

Measure ordinary owned-object/coin transfer finality.

The runner should:

- generate or use local funded addresses;
- build/sign an owned-object transfer;
- submit it to the local fullnode;
- wait for final effects;
- record digest, gas object, effects status, timestamp boundaries, and latency.

This lane answers:

```text
How fast can a mature object-based chain finalize non-conflicting value transfer?
```

It does not answer:

```text
How fast is Sui shared-object consensus?
```

### Sui Shared-Object Lane

Create a minimal benchmark Move package with one shared object and one function
that mutates it. The function should be intentionally simple, for example:

```text
shared counter:
  increment(counter: &mut Counter, ctx: &mut TxContext)
```

The runner should:

- publish the package on the local Sui network;
- create and share the counter object;
- submit 1000 signed calls that mutate the same shared object;
- wait for final effects for each call;
- record digest, effects status, object version, and latency.

This lane answers:

```text
What does Sui's consensus path look like for a contended shared object?
```

### Sui Acceptance

The Sui local lanes are accepted only if:

- owned-object lane completes 1000 successful transfers;
- shared-object lane completes 1000 successful shared-object mutations;
- every transaction has final effects and success status;
- final balances/object versions match expected deltas;
- owned and shared lanes are labeled separately in all output;
- no mnemonic/private key is copied into the public packet.

## Post Fiat Baseline Workstream

Use the current clean local benchmark unless code changed after the final report:

```text
reports/testnet-real-transaction-latency-benchmark/full-1000/real-transaction-latency-full-1000.json
reports/testnet-real-transaction-latency-benchmark/quorum-1000/real-transaction-latency-quorum-fast-1000.json
```

If any relevant code changed, rerun:

```bash
VALIDATORS=6 ROUNDS=1000 VOTE_POLICY=full \
  BASE_DIR=reports/testnet-real-transaction-latency-benchmark/full-1000-rerun/nodes \
  LOG_DIR=reports/testnet-real-transaction-latency-benchmark/full-1000-rerun/logs \
  PRIVATE_DIR=reports/testnet-real-transaction-latency-benchmark/full-1000-rerun/private \
  REPORT=reports/testnet-real-transaction-latency-benchmark/full-1000-rerun/real-transaction-latency-full-1000.json \
  ITERATIONS_FILE=reports/testnet-real-transaction-latency-benchmark/full-1000-rerun/logs/iterations.jsonl \
  CARGO_BUILD_MODE=release \
  scripts/testnet-real-transaction-latency-benchmark --rounds 1000 --validators 6 --vote-policy full \
  --report reports/testnet-real-transaction-latency-benchmark/full-1000-rerun/real-transaction-latency-full-1000.json
```

and the corresponding `quorum-fast` command with separate ports/directories.

Always rerun the adversarial finality gate if Post Fiat code changed.

## Aggregation Rules

Create `aggregate.json` with this shape:

```json
{
  "schema": "postfiat-peer-l1-latency-benchmark-v1",
  "generated_utc": "YYYY-MM-DDTHH:MM:SSZ",
  "claim_boundary": "local controlled benchmark unless lane name says public",
  "postfiat_baseline_manifest_sha256": "...",
  "lanes": {
    "postfiat_full_local": {},
    "postfiat_quorum_fast_local": {},
    "avax_local_c_chain_transfer": {},
    "sui_local_owned_transfer": {},
    "sui_local_shared_object_tx": {}
  },
  "comparisons": {
    "postfiat_vs_avax_local": {},
    "postfiat_vs_sui_owned_local": {},
    "postfiat_vs_sui_shared_local": {}
  },
  "warnings": []
}
```

Warnings must include any semantic mismatch, public endpoint noise, faucet
constraint, missing repeated sessions, or lower-than-target transaction count.

## Lab Book Requirements

Maintain:

```text
reports/peer-l1-latency-benchmark/avax-sui-YYYYMMDDTHHMMSSZ/lab-book.md
```

Append an entry for every material step:

```text
## YYYY-MM-DDTHH:MM:SSZ

Action:
Result:
Artifacts:
Blockers:
Next:
```

The worker should update the lab book before any long install/build, after every
benchmark lane, after every failure, and before stopping.

## Burn Down

### Phase 0: Preflight

- [ ] Record repo HEADs and dirty status for `postfiatl1v2` and
  `postfiatorg.github.io`.
- [ ] Confirm the current Post Fiat real-transfer evidence packet is present.
- [ ] Create the peer benchmark report directory.
- [ ] Create `lab-book.md`.
- [ ] Record host metadata: `uname -a`, CPU, memory, disk, `date -u`.
- [ ] Check installed tools: `avalanche`, `avalanchego`, `cast`, `node`, `npm`,
  `sui`, `sui-faucet`, `jq`, `sha256sum`.

### Phase 1: AVAX Local

- [ ] Install or locate Avalanche tooling.
- [ ] Start local Avalanche C-Chain/L1.
- [ ] Fund benchmark sender.
- [ ] Implement persistent AVAX runner.
- [ ] Run 10-transaction smoke.
- [ ] Run 1000-transaction local lane.
- [ ] Verify balances and receipts.
- [ ] Write raw JSONL, summary JSON, and lab-book entry.

### Phase 2: Sui Local Owned

- [ ] Install or locate Sui tooling.
- [ ] Start local Sui network.
- [ ] Fund benchmark addresses with local faucet/genesis gas.
- [ ] Implement persistent owned-object runner.
- [ ] Run 10-transaction smoke.
- [ ] Run 1000-transaction owned-object lane.
- [ ] Verify final effects and balances.
- [ ] Write raw JSONL, summary JSON, and lab-book entry.

### Phase 3: Sui Local Shared

- [ ] Create minimal Move benchmark package.
- [ ] Publish package locally.
- [ ] Create/share counter object.
- [ ] Implement persistent shared-object runner.
- [ ] Run 10-transaction smoke.
- [ ] Run 1000-transaction shared-object lane.
- [ ] Verify object version/effects.
- [ ] Write raw JSONL, summary JSON, and lab-book entry.

### Phase 4: Optional Public Smoke

- [ ] Run AVAX Fuji public endpoint smoke if faucet is available.
- [ ] Run Sui public testnet smoke if faucet is available.
- [ ] Clearly label public endpoint results as public endpoint observations.

### Phase 5: Aggregate and Validate

- [ ] Build `aggregate.json`.
- [ ] Build `aggregate.md`.
- [ ] Build `methodology.md`.
- [ ] Build `README.md`.
- [ ] Build `session-summary.csv`.
- [ ] Build `commands.sh`.
- [ ] Build `SHA256SUMS.txt`.
- [ ] Validate all JSON with `jq`.
- [ ] Validate all hashes with `sha256sum -c`.
- [ ] Confirm no local benchmark services are still running.

### Phase 6: Article Decision

- [ ] Decide whether the current article should be updated.
- [ ] If yes, draft a targeted article patch that:
  - replaces the stale `182.951 ms` Post Fiat headline with the current
    `84-89 ms` clean real-transfer benchmark where appropriate;
  - keeps the XRPL control claim separate;
  - adds AVAX/Sui only as architectural context unless the evidence packet is
    strong enough for a peer-results table;
  - states that Sui owned-object and shared-object results are different lanes.
- [ ] Do not promote article changes unless the scoring harness and operator
  approval say to promote.

## Stop Conditions

Stop only for real blockers:

- official toolchain cannot be installed or built after documented attempts;
- local AVAX or Sui network cannot be started after documented attempts;
- required benchmark cannot produce valid transactions;
- finality semantics cannot be verified;
- safety or balance/effects checks fail;
- machine resource exhaustion prevents completion;
- operator explicitly stops the work.

Do not stop for:

- one failed smoke run;
- missing public faucet funds;
- lack of mainnet coins;
- need to write a small runner;
- need to adjust local ports;
- partial completion of only AVAX or only Sui.

If public faucet funds are unavailable, skip public smoke and complete the local
packet.

## Done Definition

The goal is done only when all are true:

- required local AVAX lane completed or has a documented blocker;
- required local Sui owned lane completed or has a documented blocker;
- required local Sui shared lane completed or has a documented blocker;
- Post Fiat current baseline is copied or rerun and hash-bound;
- aggregate tables exist;
- methodology explains every finality semantic mismatch;
- lab book is complete;
- hash manifest validates;
- no private keys or local node databases are in the public packet;
- no benchmark services remain running;
- final answer states what was actually learned and what remains unproven.

## Expected Article Use

If the results are clean, the current latency article can say something like:

```text
The XRPL control remains the lineage-specific comparison. We also ran local
peer controls against Avalanche and Sui to calibrate the number against mature
low-latency architectures. These controls are not public-mainnet claims. They
show where Post Fiat's current account/balance certified-finality path sits
relative to an EVM-style AVAX local chain and Sui's separate owned-object and
shared-object paths.
```

Do not use the peer packet to imply mainnet superiority. Use it to decide
whether Post Fiat should pursue a Sui-like non-conflicting transfer fast path or
whether the current certified account/balance path is already good enough for
the controlled testnet phase.
