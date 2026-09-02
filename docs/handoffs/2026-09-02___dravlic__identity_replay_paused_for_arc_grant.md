# Identity replay paused for Arc grant qualification

- **Operator:** Domagoj Ravlić (`dravlic`)
- **Date:** 2026-09-02 UTC

## BLUF

The validator-identity work reached two distinct completed evidence boundaries
before it was paused for the time-sensitive Arc mainnet launch-partner grant and
Zellic co-sponsor review path:

1. the earlier institution-reputation replay completed on two distinct-owner
   H200-class hosts with **192/192 byte-identical comparisons** across the
   frozen 35-validator XRPL and 20-validator PostFiat sets; and
2. the later identity-packet corpus completed **55/55 strict verification**,
   with one frozen Markdown identity packet, exact prompt, complete
   Corbanu/Codex JSONL session log, empty stderr capture, and run receipt for
   every validator.

The completed earlier replay did **not** consume the later Markdown identity
packets. The next intended step—an H200 replay whose exact inputs are the frozen
packet bytes and packet hashes—has not run. That step was stopped because the
Arc grant schedule became the P0 requirement. Do not rewrite either frozen
corpus while paused.

The active Arc work is PR
[#37](https://github.com/postfiatorg/postfiatl1v2/pull/37), branch
`integrate/arc-tier4-current-v2-20260901`, current head `ecb59984`.
The current execution authority is the
[Arc current-devnet and Zellic readiness specification](../specs/pfusdc-arc-current-devnet-zellic-readiness-spec-20260902.md),
TIH-scored **91.8/100** against exact document SHA-256
`02a044becc344602eb7940b6a5ef1531d9890760036944631ec47d971109c06f`.

## Current state

### 1. Earlier institution-reputation replay — complete

The completed direct institution/domain scoring replay lives at:

```text
benchmarks/ai-governance/institution-reputation-unl-20260901/
```

Human-readable results:

- [H200 institution-reputation results](../governance/institution-reputation-unl-h200-results-20260901.md)
- [Institution legitimacy scoring rule](../governance/institution-legitimacy-scoring.md)

Source commit:

- `85ab75dbf7c850f77a75f20a904411d6a5e43de3` —
  `Publish deterministic UNL institution reputation replay`

Frozen inputs and execution:

- 35 XRPL validators; the Ripple and XRPL Foundation publishers returned the
  same 35 keys;
- 20 validators from completed PostFiat scoring round 20;
- `Qwen/Qwen3.8-27B-FP8`, revision
  `017b9c7af6b5689d5dd426a76e0bc077eb5ca20a`;
- two distinct Vast owners: one H200 and one H200 NVL;
- two runs on each host;
- temperature 0, top-p 1, seed `438916795`, thinking disabled;
- fixed two-batch schedule of 32 requests;
- loopback-only SGLang; OpenRouter was not used.

Replay result:

- validator comparisons: **165/165 byte-identical**;
- fixed-padding comparisons: **27/27 byte-identical**;
- total: **192/192 byte-identical**, zero failures;
- aggregate response SHA-256 in all four runs:
  `a1875309748195422b6bdfd0ac951fda54930a4d0bd3c7090026d1250a7c45cf`;
- comparison SHA-256:
  `7b10b3ba83b79820b48a4355a2a8f12ee021f8f8c5a1634de42a6f2486f088f7`;
- rentals destroyed after evidence retrieval.

Important paths:

| Path | Purpose |
| --- | --- |
| `manifest.json` | Frozen model/runtime/source/request identities |
| `inputs/prompt.txt` | Exact five-point scoring bands |
| `inputs/validators.json` | Frozen normalized 55-validator set |
| `inputs/requests.json` | Exact direct-coordinate scoring requests |
| `sources/` | Frozen publisher, metadata, and PostFiat source responses |
| `outputs/primary-run1.json`, `primary-run2.json` | H200 NVL raw runs |
| `outputs/replay-run1.json`, `replay-run2.json` | H200 raw runs |
| `outputs/host_identity_primary.txt`, `host_identity_replay.txt` | Runtime host identities used to prove distinct hosts/owners |
| `outputs/comparison.json` | Raw UTF-8 byte comparisons |
| `outputs/scores.json` | Canonical scores and explanations |
| `outputs/rental-teardown.json` | Redaction-safe destruction receipt |
| `test_package.py`, `compare_runs.py` | Offline verification |

Offline verification:

```bash
cd benchmarks/ai-governance/institution-reputation-unl-20260901
python3 test_package.py
python3 compare_runs.py
```

Boundary: these requests used the frozen institution/domain coordinates that
existed at that time. They are not evidence that the later identity-packet
Markdown was replayed.

### 2. Broader H200 replay lineage — complete but separate

The earlier multi-lane reputation experiments live at:

```text
benchmarks/ai-governance/reputation-h200-20260901/
```

The v4 profile completed **864/864 byte-identical comparisons** and produced
the first measured prestige-lane lift. Its result commit is
`a1699ac255589f34cce2744190447ebb7cea7c9b`; the prior Dravlic handoff is
[H200 reputation v2/v3/v4](2026-09-01___dravlic__h200_reputation_v2_published_v3_in_flight.md).

Do not confuse that anchor-suite replay with the 55-validator institution
replay above or with the not-yet-run packet-input replay below.

### 3. Validator identity packets with complete Corbanu/Codex logs — complete

The authoritative full corpus lives at:

```text
benchmarks/ai-governance/validator-identity-packets-20260901/
```

Source commits:

- `579a6b22ff6f3c3b460cfe25e1cf171a9148a516` —
  `Publish validator identity packet corpus`;
- `2f8281d53a6ea9d74ae3565311efe75bfeb994a6` —
  `Document validator identity packet operations`.

Primary documentation:

- corpus [README](https://github.com/postfiatorg/postfiatl1v2/blob/main/benchmarks/ai-governance/validator-identity-packets-20260901/README.md);
- human [index](https://github.com/postfiatorg/postfiatl1v2/blob/main/benchmarks/ai-governance/validator-identity-packets-20260901/index.md);
- operator [runbook](../runbooks/validator-identity-packets.md);
- governance [identity-packet boundary](../governance/validator-identity-packets.md).

Frozen corpus:

- 35 XRPL mainnet validators;
- 20 PostFiat round-20 validators;
- 55 Markdown packets;
- 55 exact rendered prompts;
- 55 complete Corbanu/Codex JSONL exec logs;
- 55 empty stderr captures;
- 55 command/session receipts;
- strict finalizer result: **PASS, 55/55**.

Each validator received an independent `corbanu --search exec` session:

- Corbanu Terminal 0.1.36;
- configured model `gpt-5.6-sol`;
- OpenAI provider;
- live search enabled;
- read-only sandbox;
- approval policy `never`;
- one session per validator;
- Codex fallback not used;
- OpenRouter not used.

Artifact layout:

| Path | Purpose |
| --- | --- |
| `inputs/validators.json` | Exact frozen 55-validator corpus |
| `inputs/<network>/<validator>.json` | Minimal key/domain/list coordinates |
| `prompts/<network>/<validator>.txt` | Exact initial prompt |
| `packets/<network>/<validator>.md` | Generated identity packet; future H200 input |
| `logs/<network>/<validator>.jsonl` | Complete Corbanu/Codex event log |
| `logs/<network>/<validator>.stderr.log` | Captured stderr; required empty |
| `runs/<network>/<validator>.json` | Thread/session/usage/path/hash receipt |
| `index.md`, `index.json` | Human and structured corpus indexes |
| `verification.json` | Per-validator strict-verifier result |
| `manifest.json` | Corpus counts, execution contract, aggregate hashes |
| `build_prompts.py`, `run_all.py`, `finalize.py` | Frozen corpus tooling |

Packet contents include validator coordinates, claimed domain and official URLs,
public identity and aliases, X handle, incorporation and operating regions,
activities, estimated profile size, cited evidence, uncertainty/conflicts,
machine-readable JSON, and a neutral 90–160-word Dunn & Bradstreet-style
business summary.

Corpus findings are descriptive, not scores:

- identity established or likely: 45;
- identity not established: 10;
- public X handle established: 41;
- incorporation region established or qualified: 25;
- cited evidence URLs: 541.

Frozen hashes:

| Artifact set | SHA-256 |
| --- | --- |
| validator source corpus | `7687dcd9a23638dca4e0fbe50c2dd3782c6db89fa645802cd5dd9586feb87f27` |
| prompt template | `48a03cabd80cfd0f8fac6ef57cdc700ce1ea45c88a87b9fbdf7f9ee0f6d3769b` |
| 55 Markdown packets | `b198e232baa644731b38e2f6db3989c798156700ebc67856a193b32bb941d4bd` |
| 55 JSONL exec logs | `3a72e90a410df1c1ce0681f63d5d581ab70d724bda35362f8a03078a305493b1` |
| 55 run receipts | `94d53c7fd0d0a5e1b1149d5d0b51a0ef81645e90f1b43060da8c75e75955b541` |
| structured index | `52611131e415f6ca47b4365191b86eaf6dc55770d104b1005c71453cf3f8b9f4` |

Verify the frozen corpus without paid calls or web access:

```bash
ARTIFACT=benchmarks/ai-governance/validator-identity-packets-20260901
python3 "$ARTIFACT/finalize.py"
git diff --exit-code --   "$ARTIFACT/index.json"   "$ARTIFACT/index.md"   "$ARTIFACT/manifest.json"   "$ARTIFACT/verification.json"
```

The one-validator pilot at
`benchmarks/ai-governance/validator-identity-packet-pilot-20260901/` is
historical. Use the full 55-validator corpus, not the pilot.

Safety boundary:

- packets are `SHADOW_ONLY` public research;
- they are not consensus data;
- they do not prove current control of a validator key;
- they are not legitimacy, reputation, sanctions, association, credit, or risk
  scores;
- the complete JSONL logs are audit/publication evidence, not H200 scoring
  input;
- a claimed domain or WHOIS observation is not proof of legal identity or key
  control.

Machine verification is complete. Before any new public production use, confirm
the independent human publication review required by the runbook; do not infer
that review merely from `verification.json`.

### 4. Packet-based H200 replay — paused before execution

The next intended scoring revision is explicitly defined in
[Institution Legitimacy Scoring](../governance/institution-legitimacy-scoring.md)
and Section 10 of the
[identity-packet runbook](../runbooks/validator-identity-packets.md).

It has **not** been built or run.

Required input contract when resumed:

- exact Markdown bytes from `packets/<network>/<validator>.md`;
- network and validator key;
- per-packet SHA-256 from `index.json`;
- corpus packet-set SHA-256 from `manifest.json`;
- scoring request binds the packet hash;
- no live web search;
- no Corbanu rerun;
- no interpretation of JSONL logs by the H200;
- no mutable `index.md` display text or operator nickname as a substitute for
  the packet.

The packet-based replay must be a new dated successor package. It must not
overwrite:

- `institution-reputation-unl-20260901`;
- `validator-identity-packets-20260901`; or
- `reputation-h200-20260901`.

A safe resume sequence is:

1. rerun `finalize.py` and require no corpus diff;
2. document the independent human-review status;
3. build a new package from exact packet bytes and bind all packet/corpus hashes;
4. freeze the prompt, model revision, runtime image, deterministic settings,
   batch schedule, and padding;
5. run two times on each of two distinct-owner H200-class hosts;
6. compare raw response bytes per validator and padding slot;
7. publish scores, explanations, host identities, comparison, teardown receipt,
   and the identity-corpus manifest hash;
8. keep results `SHADOW_ONLY`; and
9. do not connect the result to consensus or validator weight without a separate
   operator decision and governed activation path.

### 5. Why identity work was paused

The identity-packet corpus completed successfully. Work stopped before its
downstream H200 packet replay because the Arc launch-partner grant became
time-critical ahead of the Arc mainnet schedule, and the user directed the
session to prioritize a current-devnet-qualified Arc proposal and a package for
Zellic technical review/co-sponsorship.

This was a priority interruption, not an identity-pipeline failure. No paid
identity session was left running and no H200 identity rental remains active.

## Arc/Zellic priority now

Controlling documents:

- [Arc Tier-4 specification](../specs/pfusdc-arc-tier4-spec-20260828.md);
- [Arc MVP testnet specification](../specs/pfusdc-arc-mvp-testnet-spec-20260828.md);
- [Arc grant proposal v3](../business/pfusdc-arc-grant-proposal-20260828-v3.md);
- [Current-devnet/Zellic readiness specification](../specs/pfusdc-arc-current-devnet-zellic-readiness-spec-20260902.md).

Current repository state:

- branch: `integrate/arc-tier4-current-v2-20260901`;
- Arc qualification-spec head before this handoff-only commit:
  `ecb59984d82395e2409accd55d9bead2b2587415`;
- PR #37 is draft;
- before this handoff-only commit, all reported CI jobs were green except the
  exhaustive Rust `test` and `open-reserve-proof-kit` jobs, which were still
  running;
- no PFTL validator was queried or mutated in this session.

Work completed for the Arc pivot:

- current `main` and the public Arc Tier-4/MVP specs were merged into the
  integration branch;
- grant proposal v3 was pulled from commit `3f393b76`;
- the incorrect generic proof-RPC/grant document was deleted;
- the current-devnet/Zellic qualification spec was committed at `ecb59984`;
- TIH full gate on that exact spec: GPT 94.0, Fable 90.6, GLM 90.8,
  overall **91.8**;
- a dedicated Arc-testnet-only EOA was created outside the repository and
  funded with 20 native Arc testnet units and 20 testnet USDC; no Arc
  transaction has been spent from it during this handoff work.

The Arc specification's first real blocker is current-v2 validator-registry
authentication. The public Arc testnet RPC returns `-32601` for
`eth_getProof`, while current-v2 requires an exact-block EIP-1186 registry
proof even for a no-change validator set. The guest must not be weakened.
TIH independently identified this unresolved proof source, missing owner, and
missing resolution deadline as the specification's main weakness and
recommended an execution sprint.

## Next decision or action

### P0 — Arc grant/Zellic

1. Let the remaining two PR #37 CI jobs finish; repair any Arc-related failure.
2. Assign an owner and deadline to the exact-block Arc registry-proof source.
   Qualify either a proof-capable Arc archive RPC, a locally operated archive
   node, or a narrow proof-generation implementation. Do not revert to v1 or
   accept `eth_call` as proof.
3. Execute the current-devnet qualification spec in order:
   reproducible current ELFs/vkeys, exact six-node clone, separately authorized
   live deployment, primitive matrix, fresh 1.000000-USDC round trip, and
   replay/conservation audit.
4. Build the Zellic review packet and classify every material claim in grant
   proposal v3. Do not claim review, audit, endorsement, or co-sponsorship until
   Zellic supplies it.
5. Keep PR #37 draft until its qualification gates close.

### Resume identity work only after the Arc deadline

1. Verify the frozen packet corpus and independent-review status.
2. Create a new dated packet-input H200 package; never overwrite frozen artifacts.
3. Run and publish the two-host/two-run byte replay bound to exact packet hashes.
4. Keep the result external and `SHADOW_ONLY` unless the operator separately
   authorizes a governance path.

## Boundaries

- No Task Node action.
- No live PFTL fleet probe, SSH, deployment, or chain mutation in this session.
- No identity packet or replay artifact was rewritten.
- No identity GPU rental remains active.
- The Arc testnet wallet secret is outside the repository and was never printed
  or committed.
- Historical replay evidence is not current authority.
- The Arc integration remains an undeployed draft PR.

## References

- [Institution legitimacy scoring](../governance/institution-legitimacy-scoring.md)
- [Institution reputation replay results](../governance/institution-reputation-unl-h200-results-20260901.md)
- [Validator identity packets](../governance/validator-identity-packets.md)
- [Identity packet operator runbook](../runbooks/validator-identity-packets.md)
- [Prior H200 Dravlic handoff](2026-09-01___dravlic__h200_reputation_v2_published_v3_in_flight.md)
- [Arc grant proposal v3](../business/pfusdc-arc-grant-proposal-20260828-v3.md)
- [Arc current-devnet/Zellic readiness specification](../specs/pfusdc-arc-current-devnet-zellic-readiness-spec-20260902.md)
- [Current PFTL state authority](../status/chain-state-current.md)

## End of session (Domagoj, 2026-09-02)

The sections above are the overnight agent session; this section is the day session.

### Delivered today

- **P0-1 — reproducible current-v2 identities:**
  [`65df4263`](https://github.com/postfiatorg/postfiatl1v2/commit/65df4263746566c90d0e46db8c90a46c23576265)
  completed the overnight five-file mid-fix without weakening a gate, and
  [`077b8637`](https://github.com/postfiatorg/postfiatl1v2/commit/077b863708ca741238425bdb15c6faff57bb5e0d)
  added manifest tracking. The
  [`reproduce-current-v2-identities` job](https://github.com/postfiatorg/postfiatl1v2/actions/runs/33616296837/job/100202792755)
  was green in 23m01s: its independent clean builds were byte-identical to each
  other and the checked-in ELFs. See the
  [workflow](https://github.com/postfiatorg/postfiatl1v2/blob/077b863708ca741238425bdb15c6faff57bb5e0d/.github/workflows/arc-proof-identities.yml)
  and [reproduction manifest](../evidence/arc-mvp-20260828/program-reproduction.current-v2-docker-20260902.json).
- **P0-2 — registry proof-source options:**
  [`85cb3c91`](https://github.com/postfiatorg/postfiatl1v2/commit/85cb3c9183ce5998e3e9a20a87106fadf7e1f8af)
  added the [options document](../specs/arc-registry-proof-source-options-20260902.md).
  Exact-historical `eth_getProof` probes failed on Circle, Blockdaemon, dRPC,
  and QuickNode. The recommendation is to qualify Option 2 on the account's
  `arc-archive-proof-20260902` instance; the operator must record owner,
  deadline, and chosen option.
- **P0-4 — Zellic review packet:**
  [`ebfcbdbb`](https://github.com/postfiatorg/postfiatl1v2/commit/ebfcbdbb41bda9b85e944bc603c84e0911428c23)
  classified all 101 grant-proposal-v3 claims in the
  [packet](../business/zellic-review-packet-20260902.md), then
  [`9008e749`](https://github.com/postfiatorg/postfiatl1v2/commit/9008e7490bfca06c4961862dedde99cbaf0c6a27)
  upgraded it with execution: 24 VERIFIED IN REPO / 15 devnet-demonstrated /
  12 implemented-unverified / 31 planned / 19 aspirational. One verification
  failure is recorded: the local egress reproducible build needs `cargo-prove`
  and Docker; CI covers the reproducible identity gate. No Zellic review or
  endorsement is claimed anywhere.
- **Archive-instance qualification — read-only:**
  [`75c28d72`](https://github.com/postfiatorg/postfiatl1v2/commit/75c28d72b6af22b73db8a9a3f9ceef950f0874d1)
  recorded the [exact probe](../specs/arc-registry-proof-source-options-20260902.md#qualification-check--2026-09-02).
  Instance `49602886` has Arc's Reth-based client installed but no node process
  started: `NOT SUITABLE` until provisioned, while billing about $0.28/hour for
  idle compute.
- **PR #37:** its [description](https://github.com/postfiatorg/postfiatl1v2/pull/37)
  is written and the
  [five-line status comment](https://github.com/postfiatorg/postfiatl1v2/pull/37#issuecomment-5509120470)
  is posted. The PR remains draft.
- **Qualification progress:** the
  [reproducible current ELFs/vkeys step](../specs/pfusdc-arc-current-devnet-zellic-readiness-spec-20260902.md#a2-rebuild-proof-identities)
  is satisfied by the green independent-rebuild CI job, byte-identical ELFs,
  and pinned vkeys.

### Boundaries

Archive-instance access was read-only: nothing was started, stopped, or
changed. No live Arc transaction occurred; the wallet secret was untouched;
the PFTL devnet was not contacted; Task Node was not used. Every push was to
this branch only, and `main` was untouched today.

### Decisions for the operator

1. Provision archive instance `49602886` as the proof source (recommended), or
   destroy the rental.
2. Record owner, deadline, and option for the registry proof source as required
   by the [options document](../specs/arc-registry-proof-source-options-20260902.md#recommendation).
3. The [pending-decisions sheet on `main`](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/governance/pending-operator-decisions.md)
   is still fully unanswered, including the model-authority row on which the
   latest message leans; one line per row closes it.

### References

- [Registry proof-source options](../specs/arc-registry-proof-source-options-20260902.md)
- [Zellic review packet](../business/zellic-review-packet-20260902.md)
- [PR #37](https://github.com/postfiatorg/postfiatl1v2/pull/37)
- [Current-devnet/Zellic qualification specification](../specs/pfusdc-arc-current-devnet-zellic-readiness-spec-20260902.md)
- [Pending operator decisions on `main`](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/governance/pending-operator-decisions.md)
