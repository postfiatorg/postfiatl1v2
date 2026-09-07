# PFTL-Native Deterministic SEC Index Protocol

Date: 2026-08-25

Status: implementation specification; controlled-testnet target; no live-value
authorization

Primary settlement and finality network: Post Fiat Ledger (PFTL)

External data boundary: SEC-hosted registry, filings, and filing exhibits only

Inference profile: pinned `Qwen/Qwen3.8-27B` through deterministic SGLang

Flare dependency: none

Change control: any change to the source policy, universe rule, text
normalization, slicing, prompt, model/runtime profile, parser, missing-data
policy, fundamental-size formula, narrative-selection formula, portfolio
formula, replay threshold, or transaction signing preimage creates a new
versioned series. A finalized epoch is immutable.

## Objective and claim boundary

The protocol turns a frozen SEC filing corpus into five evidence-backed
thematic issuer indices. Qwen performs the qualitative compression and scoring;
deterministic integer code constructs the size factor, selects dominant
narratives, and calculates index weights. PFTL records the methodology,
content roots, independent replay receipts, and final weights as native
consensus state.

The intended claim is:

> Given the committed SEC corpus, model/runtime profile, prompts, and
> transformation rules, independent replay operators produced the same result,
> and PFTL finalized exactly that result without an undisclosed constituent or
> weight override.

This is stronger than an operator publishing a signed CSV. It is not proof that
the SEC filing is true, that Qwen's interpretation is wise, that the index will
perform well, or that replay operators are independent merely because they use
different keys. The accurate label is **PFTL-finalized and independently
replayable**, not assumption-free or universally trustless.

The protocol creates issuer indices. An investable fund, perpetual, NAVCoin, or
tokenized security is a separate downstream product with separate price,
liquidity, custody, corporate-action, and legal requirements.

## 1. PFTL-only protocol

### 1.1 System boundary

PFTL is sufficient for registration, attestation, conflict handling, and
finalization:

```text
SEC source bytes
  -> canonical issuer and filing universe
  -> deterministic financial-fact extraction
  -> deterministic text normalization and slicing
  -> pinned Qwen compression and narrative generation
  -> pinned Qwen issuer/narrative scoring with exact SEC evidence spans
  -> integer-only size, dominance, and weight calculation
  -> independent byte-exact replay receipts
  -> PFTL proposal, challenge window, quorum, and finalization
  -> immutable index epoch root and constituent proofs
```

Qwen does not run inside PFTL consensus. A 27B GPU inference workload is too
large, slow, hardware-specific, and failure-prone for every validator to place
in the state-transition path. PFTL validators instead execute bounded
cryptographic and integer checks over signed replay statements. Anyone may run
the published compiler and report a disagreement; in v1, only an admitted
replay key can place a consensus-effective attestation or conflict on-chain.

This is the same broad pattern as PFTL's existing NAV attestor and epoch
finalization machinery, but index state must use separate types and signing
domains. Index evidence is not reserve evidence and must not be smuggled into a
`NavReservePacket`.

### 1.2 Repository ownership

The implementation should preserve the current separation of responsibilities:

| Repository/layer | Responsibility |
| --- | --- |
| `navstrategies` | SEC download, filing selection, XBRL extraction, text normalization, Qwen jobs, compiler artifacts, and replay CLI. |
| `postfiatl1v2/crates/types` | Canonical index protocol types, bounds, signing preimages, and SHA3-384 identities. |
| `postfiatl1v2/crates/execution` | Series registration, proposal, attestation, conflict, and finalization state transitions. |
| `postfiatl1v2/crates/storage` | Persistent series, epoch, receipt, and finalized-root indexes. |
| `postfiatl1v2/crates/rpc_sdk` | Read APIs, fee quotes, transaction submission, and Merkle-proof retrieval. |
| `postfiatl1v2/python/postfiat_rpc` | Python builders and verifier used by the compiler and demo. |

Proposed files:

```text
postfiatl1v2/
  crates/types/src/index_protocol.rs
  crates/execution/src/index_execution.rs
  python/postfiat_rpc/indexing.py
  docs/specs/pftl-native-deterministic-sec-index-protocol.md

navstrategies/
  navstrategies/indexing/sec_index_compiler.py
  navstrategies/indexing/canonical_text.py
  navstrategies/indexing/qwen_replay.py
  schemas/pftl-index-series-v1.schema.json
  schemas/pftl-index-epoch-v1.schema.json
  tests/indexing/
```

Do not introduce a second SEC financial-fact extractor. The compiler must call
the existing `sec_10q_three_metric_operator.py` and retain its accession-level
provenance and derivation formulas.

### 1.3 Immutable series definition

An `IndexSeriesV1` fixes all human choices before an epoch's results are known:

```text
series_id
series_version
publisher
source_policy_hash
universe_policy_hash
text_pipeline_hash
qwen_profile_hash
compression_prompt_hash
narrative_prompt_hash
applicability_prompt_hash
output_schema_hash
parser_hash
fundamental_size_policy_hash
dominance_policy_hash
weight_policy_hash
missing_data_policy_hash
replay_set_root
replay_threshold
challenge_window_blocks
maximum_artifact_bytes
```

`series_id` is descriptive, while `series_key` is consensus identity:

```text
series_key = SHA3-384(
  "postfiat.index.series.v1\n" ||
  canonical_series_signing_bytes
)
```

The canonical bytes use an explicit field order and length prefixes. JSON is an
RPC representation only and must not be a consensus hashing input. PFTL's
existing 96-character lowercase SHA3-384 convention applies.

No field in an active series is mutable. A methodology change registers a new
`series_version` and `series_key`; it cannot rewrite an earlier epoch.

### 1.4 SEC universe and security identity

The v1 universe uses the exact bytes of:

```text
https://www.sec.gov/files/company_tickers_exchange.json
```

Each epoch records the source URL, retrieval metadata, byte length, SHA-256
content hash for ordinary artifact tooling, and SHA3-384 PFTL content identity.
The raw bytes are retained in at least two content-addressed locations. IPFS is
permitted but not required and is never the only copy.

CIK is the issuer identity. The v1 canonical ticker rule is intentionally
mechanical:

1. group registry rows by CIK;
2. retain rows whose ticker and exchange fields are nonempty;
3. normalize ticker and exchange as trimmed Unicode NFC without changing case;
4. sort rows by `(ticker UTF-8 bytes, exchange UTF-8 bytes)`; and
5. select the first row as that issuer's display security.

This avoids an undisclosed liquidity or share-class judgment. It does not prove
that the selected class is the most investable one. Any later liquidity-aware
security selection requires a separately named market-data-enhanced series.

An issuer is eligible in v1 only when, at the epoch cutoff:

- its selected registry row exists in the frozen registry snapshot;
- a 10-Q accepted by the SEC is available;
- that 10-Q is newer than the issuer's filing used in the preceding epoch;
  otherwise the prior issuer packet carries forward unchanged;
- total assets and stockholders' equity can be extracted from that 10-Q; and
- four consecutive discrete fiscal-quarter revenue values can be constructed
  from the selected 10-Q, preceding 10-Qs, and intervening 10-K.

A true pre-revenue value of zero is valid. An unavailable or ambiguous value is
`MISSING`, not zero. Issuers missing any of the three v1 metrics are published
in the coverage report but excluded from v1 weights. There is no silent metric
substitution.

Epoch cutoff times and rebalance cadence are fixed in the series policy.
Company data changes only after a newly accepted 10-Q. An 8-K, headline, price
move, employee request, or model rerun cannot independently alter an issuer's
official packet.

### 1.5 Admissible SEC narrative text

The narrative corpus contains only SEC-hosted bytes available by the epoch
cutoff:

1. the issuer's selected 10-Q;
2. all plain-text or HTML earnings-release exhibits furnished with an 8-K Item
   2.02 after the issuer's preceding periodic report and no later than the
   selected 10-Q's acceptance time; and
3. the 10-Q MD&A text when no qualifying earnings-release exhibit is present.

Every document record contains CIK, accession, form, acceptance timestamp,
SEC URL, exhibit filename, raw-byte hash, normalized-text hash, and normalized
text length. The compiler must retain raw source bytes and a byte/character
offset map so every cited normalized span can be traced back to raw SEC bytes.

Canonical text processing is pinned and versioned:

```text
decode using declared charset, otherwise UTF-8 with replacement
HTML parsing through one pinned parser version
remove script/style/navigation markup
decode entities
Unicode NFC normalization
line endings -> LF
horizontal whitespace runs -> one ASCII space
more than two blank lines -> two blank lines
retain paragraph order
```

After normalization, slice by the pinned Qwen tokenizer. The initial profile
uses 24,000-token payload slices, 1,024-token overlap, paragraph-first
boundaries, and a hard token boundary when a paragraph is too large. Slices are
ordered by `(CIK, accession, exhibit filename, start_token)` and identified by
their content hash. Context cutoff behavior is therefore data, not an operator
decision made during a run.

### 1.6 Financial-fact layer and fundamental size

For issuer `i`, the compiler extracts:

```text
revenue_i = sum of four consecutive discrete fiscal-quarter revenues
assets_i  = latest 10-Q consolidated total assets
equity_i  = latest 10-Q consolidated stockholders' equity
```

Values are normalized to signed `USD_E6` integers. Parsing must reject overflow
or more than six fractional decimal places; consensus-facing calculations use
`u128`/`i128` checked arithmetic and never `f32` or `f64`.

Raw assets would structurally dominate many industries, so v1 uses equal-weight
cross-sectional percentile ranks rather than adding raw dollars. For metric
`m` and eligible universe size `N > 1`:

```text
x(i,m) = max(metric(i,m), 0)

rank_ppm(i,m) = 0                                      if x(i,m) == 0
rank_ppm(i,m) = floor(
  1,000,000 * (2 * count(x(j,m) < x(i,m))
               + count(x(j,m) == x(i,m)) - 1)
  / (2 * (N - 1))
)                                                       otherwise

fundamental_size_ppm(i) = floor(
  (rank_ppm(i,revenue) + rank_ppm(i,assets) + rank_ppm(i,equity)) / 3
)
```

The midrank formula gives equal inputs equal ranks. CIK ascending is the
canonical iteration order but does not break economic ties. Negative revenue or
equity is clipped to zero and remains visibly negative in the evidence leaf.

This factor measures relative accounting scale, not market capitalization or
enterprise value. Banks and insurers will often rank highly on assets; that is
a disclosed property of the methodology, not an error to patch manually.

### 1.7 Pinned Qwen 3.8 execution profile

The initial profile starts from the replay-tested Bread-and-Circuses stack:

| Field | v1 value |
| --- | --- |
| Model | `Qwen/Qwen3.8-27B` |
| Revision | `1d4bf0f2ff6012fd82039f2fa52739d0dd7c60c0` |
| SGLang | `0.0.0.dev0+qwen38.27b.g561c8f3` |
| Container | `lmsysorg/sglang:qwen38-27b@sha256:febfb971c7352570fc445c466ebd6ffc9d896024958e544a60f2137fd85856b1` |
| Hardware class | NVIDIA H200 |
| Tensor parallelism | `1` |
| Context length | `32768` |
| Concurrent requests | `1` |
| Attention | FA3; Triton linear-attention prefill/decode |
| Determinism | `--enable-deterministic-inference` |
| Disabled paths | radix cache, overlap scheduling, prefill CUDA graphs, decode CUDA graphs |
| Sampling | greedy, `temperature=0`, `top_p=1`, seed `438916795` |

The model-file manifest, tokenizer, chat template, driver, CUDA, Torch,
Transformers, parser, launch command, environment allowlist, and complete prompt
bytes are also hashed. A model name and temperature alone are not a replay
profile.

Each inference stage has a fixed JSON schema, maximum input size, maximum output
tokens, and one precommitted fallback profile. Invalid or capped output after
that fallback becomes `UNRESOLVED`. Operators may not keep retrying until a
preferred answer appears.

### 1.8 Semantic pipeline

The qualitative layer borrows the Bread-and-Circuses pattern—a public semantic
question, a pinned Qwen runtime, an integer score, and an explanatory brief—but
adds source-bounded evidence.

#### Compression layer

For each issuer slice, Qwen emits at most eight `ConceptBlockV1` objects:

```json
{
  "statement": "Demand for data-center power and cooling is expanding.",
  "materiality": 82,
  "evidence": [
    {
      "slice_id": "<sha3-384>",
      "start_char": 18422,
      "end_char": 18691
    }
  ]
}
```

Statements must be qualitative propositions that can be tested for
applicability to another company. A nonzero materiality score requires at least
one valid span wholly contained in the supplied slice. The parser rejects
uncited blocks, out-of-range offsets, unknown slice IDs, duplicate keys, extra
fields, non-integer scores, or invalid UTF-8.

#### Narrative synthesis

Concept blocks are ordered by `(CIK, slice_id, statement_hash)` and packed into
fixed-size synthesis shards. Each shard may emit at most eight candidate
narratives. Repeated deterministic reduction rounds use the same ordering and
packing rules until no more than 16 canonical candidates remain.

The model does not directly choose the winning five. It defines bounded
candidate statements; the later cross-sectional scoring and integer dominance
formula select the five winners. This prevents a hidden final editorial step.

Each candidate receives:

```text
narrative_id = SHA3-384(
  "postfiat.index.narrative.v1\n" ||
  epoch_source_root ||
  canonical_candidate_bytes
)
```

#### Company applicability scoring

Every eligible issuer is scored against every candidate using only that
issuer's canonical SEC evidence packet. Qwen emits:

```json
{
  "score": 0,
  "brief": "...",
  "supporting_spans": [],
  "counter_spans": []
}
```

`score` is an integer from 0 through 100. Scores above zero require a supporting
span; scores of 60 or higher require either two independent supporting spans or
one span containing a quantitative business fact. The model may use its frozen
weights for language understanding, but an issuer cannot enter an index solely
because the model associates its ticker with a theme from training data.

### 1.9 Dominant narratives and constituent weights

For issuer `i` and candidate narrative `k`:

```text
relevance_ppm(i,k) = 0                         if score(i,k) < 60
relevance_ppm(i,k) = score(i,k) * 10,000      otherwise

support(i,k) = fundamental_size_ppm(i) * relevance_ppm(i,k)

dominance(k) = sum_i support(i,k)
support_count(k) = count_i(score(i,k) >= 60)
```

A candidate qualifies only with at least ten supporting issuers and supporting
issuers from at least three SEC SIC two-digit groups. The five qualifying
candidates with greatest `(dominance, support_count)` become the epoch's
dominant narratives. Ties are resolved by ascending `narrative_id` bytes. If
fewer than five qualify, the epoch publishes the available count and never
invents filler narratives.

For each selected narrative:

```text
raw_weight(i,k) = support(i,k)
```

Issuers with zero raw weight are excluded. The remaining weights are normalized
to one billion parts using largest-remainder allocation:

1. compute each floor allocation with checked `u128` arithmetic;
2. distribute leftover parts by descending remainder;
3. break remainder ties by ascending CIK; and
4. assert that final integer weights sum to exactly `1,000,000,000`.

The v1 index has no discretionary caps, liquidity screen, turnover rule, or
price-based investability rule. Those features require market data and a new
series version. The output is therefore a deterministic issuer-weight index,
not yet a trade-ready benchmark.

### 1.10 Artifact tree and epoch identity

Large SEC documents and model outputs remain off-chain. PFTL stores roots,
counts, policies, and final state. The epoch bundle contains:

```text
registry snapshot
issuer/security identity leaves
raw SEC document manifest
normalized text and offset maps
financial-fact leaves and derivations
compression requests and raw responses
candidate-narrative requests and raw responses
applicability requests and raw responses
parsed score matrix
dominance table
five constituent/weight trees
coverage and unresolved reports
runtime and model-file manifests
compiler source commit and reproducible-build manifest
```

Merkle leaves use domain-separated SHA3-384 over a fixed binary field order with
length prefixes. Children are ordered lexicographically before hashing so proof
verification never depends on map iteration order.

`IndexEpochBasisV1` commits:

```text
series_key
epoch
sec_cutoff_unix_seconds
registry_root
source_document_root
normalized_text_root
financial_fact_root
compression_output_root
narrative_candidate_root
score_matrix_root
dominance_root
selected_narrative_root
constituent_weight_root
runtime_manifest_root
compiler_manifest_root
coverage_counts
unresolved_counts
```

The epoch identity is:

```text
index_epoch_id = SHA3-384(
  "postfiat.index.epoch.v1\n" ||
  canonical_epoch_basis_bytes
)
```

### 1.11 Native PFTL transactions and state machine

Each replayer signs an `IndexReplayStatementV1` containing exactly:

```text
PFTL chain_id
PFTL genesis_hash
protocol_version
series_key
epoch
index_epoch_id
epoch_basis_hash
artifact_manifest_root
compiler_manifest_root
runtime_manifest_root
score_matrix_root
selected_narrative_root
constituent_weight_root
coverage_counts
unresolved_counts
replay_status = EXACT | CONFLICT
conflicting_epoch_id (present only for CONFLICT)
```

`EXACT` means the replayer fetched the committed source artifacts and rebuilt
the complete compiler result, including all non-inference arithmetic. PFTL
validates the statement's domain, signature, membership, bounds, and agreement;
it does not download the corpus or recompute the full Qwen and portfolio
workload inside consensus.

Add six versioned asset-operation transaction kinds:

| Transaction | Effect |
| --- | --- |
| `index_series_register` | Registers one immutable series and its replay policy. |
| `index_epoch_propose` | Opens one candidate epoch and commits all roots and artifact locations. |
| `index_replay_attest` | Records a registered replayer's exact-result statement. |
| `index_epoch_challenge` | Records a registered replayer's conflicting result root and freezes the epoch. |
| `index_epoch_finalize` | Finalizes after quorum and the challenge window; callable by anyone. |
| `index_series_halt` | Stops future proposals; never deletes finalized history. |

State transitions are:

```text
UNREGISTERED -> REGISTERED
REGISTERED   -> PROPOSED
PROPOSED     -> QUORUM_READY       when threshold distinct replay keys attest
PROPOSED     -> CONFLICTED         on a valid conflicting replay receipt
QUORUM_READY -> CONFLICTED         on a valid conflict before finalization
QUORUM_READY -> FINALIZED          after challenge_window_blocks
```

`CONFLICTED` is terminal for that epoch identity. Recovery requires a corrected
proposal with a new epoch identity or a new series version; governance cannot
silently select the preferred output. A replay key that signs two different
roots for the same `(series_key, epoch)` is recorded as equivocating and neither
receipt counts toward quorum.

Consensus validation must enforce:

- exact chain ID, genesis hash, protocol version, transaction kind, account
  sequence, fee, and ML-DSA signature binding;
- one monotonically increasing epoch number per series;
- one attestation per replay key and result root;
- replay membership frozen at series registration;
- distinct replay keys, not merely distinct receipt bytes;
- exact agreement on every epoch-basis root and count;
- block-height-based challenge windows, never wall-clock timers;
- bounded strings, vectors, artifact locators, and receipt counts;
- no floating point, wall-clock access, randomness, unordered state iteration,
  panic, or unbounded allocation in the transition path; and
- idempotent rejection of duplicate proposal, attestation, challenge, and
  finalization transactions.

Initial protocol bounds are:

```text
MAX_INDEX_REPLAYERS              = 16
MAX_INDEX_REPLAY_RECEIPTS        = 16 per proposal
MAX_INDEX_ARTIFACT_LOCATORS      = 4
MAX_INDEX_ARTIFACT_LOCATOR_BYTES = 512
MAX_INDEX_NARRATIVES             = 5 finalized per epoch
MAX_INDEX_PROTOCOL_STRING_BYTES  = 256 unless a narrower field bound applies
```

Epoch bundles may be large, but no untrusted transaction may cause an
unbounded allocation or loop proportional to the off-chain corpus size.

For controlled testnet v1, use a static three-key replay set with a two-of-three
threshold. Public-network claims require a later admission, stake, rotation,
and penalty policy; lack of outside operators does not block controlled-testnet
protocol testing.

### 1.12 RPC and public verification

Required read methods:

```text
index_series(series_key)
index_epoch(series_key, epoch)
index_epochs(series_key, cursor, limit)
index_replay_receipts(series_key, epoch)
index_constituent_proof(series_key, epoch, narrative_id, cik)
index_conflicts(series_key, epoch)
```

Required write helpers:

```text
index_series_fee_quote
index_epoch_propose_fee_quote
index_replay_attest_fee_quote
index_epoch_challenge_fee_quote
index_epoch_finalize_fee_quote
```

The public verifier downloads the epoch bundle, checks every content hash,
rebuilds every Merkle root and integer transformation, verifies replay
signatures and PFTL finality, and optionally reruns Qwen. Verification without a
GPU can still prove artifact integrity, arithmetic, signatures, and finality;
it cannot independently prove that the committed model generated the raw
responses.

### 1.13 Failure policy

| Failure | Deterministic result |
| --- | --- |
| SEC fetch unavailable | No new proposal; previous finalized epoch remains current. |
| SEC source changes at the same URL | New byte hash; mismatched proposal rejected by replayers. |
| Ambiguous XBRL fact | Issuer excluded with reason code and cited candidates. |
| Missing TTM revenue | Issuer excluded; never substitute annual revenue or zero. |
| Invalid Qwen JSON | Run the one precommitted fallback; otherwise `UNRESOLVED`. |
| Invalid evidence span | Score output rejected. |
| Too few supporters | Narrative not selected. |
| Replay byte mismatch | Epoch becomes `CONFLICTED`; no majority-by-convenience. |
| Artifact unavailable | Replayers do not attest; epoch cannot reach quorum. |
| Arithmetic overflow | Compiler and consensus verifier fail closed. |
| PFTL reorganization before finality | Consumers wait for ordinary PFTL finality. |

### 1.14 Controlled-testnet implementation gates

- [x] Existing SEC operator extracts assets and stockholders' equity for 50/50
  frozen sample issuers.
- [x] Existing SEC operator constructs four-quarter revenue for 42/50 frozen
  sample issuers and reports unsupported cases explicitly.
- [x] Dow's `$41.319B` four-quarter revenue is a retained regression vector.
- [x] The Qwen 3.8/SGLang profile has prior byte-exact qualitative replay
  evidence, including the Bread-and-Circuses ratings.
- [x] PFTL already has native attestor, challenge, and epoch-finalization
  patterns that can be reused structurally.
- [ ] Lock the v1 source, universe, text, Qwen, size, dominance, and weight
  policy manifests and their exact hashes.
- [ ] Implement canonical SEC earnings-release selection and offset-preserving
  text normalization.
- [ ] Implement compression, narrative synthesis, applicability scoring, and
  fixed fallback schedules.
- [ ] Implement the integer percentile, dominance, selection, and
  largest-remainder weight compiler.
- [ ] Add the six PFTL index transaction kinds and persistent state indexes.
- [ ] Add deterministic signing/hash conformance vectors shared by Rust and
  Python.
- [ ] Add unit tests for every malformed operation and every arithmetic edge.
- [ ] Add property tests for weight conservation, replay idempotence, and
  order-independent input construction.
- [ ] Fuzz transaction decoding, Merkle proofs, JSON-to-protocol conversion,
  and hostile artifact bounds.
- [ ] Run a multi-node deterministic simulation covering duplicate receipts,
  equivocation, conflicting roots, delayed finalization, restart, and replay.
- [ ] Produce one 50-issuer end-to-end epoch with two byte-matching replay
  operators and five or fewer mechanically selected narratives.
- [ ] Verify the same finalized epoch root from a clean machine.
- [ ] Expand to the full eligible SEC 10-Q universe only after runtime,
  artifact-size, and coverage measurements pass.

The controlled-testnet demo is complete when two clean replay environments
produce identical epoch roots, PFTL finalizes the epoch once, negative tests
cannot mutate or double-finalize it, every constituent has an SEC evidence
proof, and a third party can reconstruct all non-inference transformations from
the public bundle.

## 2. Optional Flare integration

Flare is an adapter, not a dependency and not the canonical index ledger. A
PFTL epoch must be able to reach `FINALIZED` while every Flare component is
offline.

### 2.1 Optional roles

Flare may add either or both of these capabilities:

1. **Hardware-attested replay operator.** A Flare Confidential Compute
   extension runs the pinned compiler or validates a completed replay inside a
   measured TEE. Its attested result is translated into one ordinary
   `index_replay_attest` transaction from a replay key already admitted by the
   PFTL series.
2. **EVM distribution adapter.** A Flare contract accepts a proof or configured
   threshold attestation that PFTL finalized an epoch, records a compact mirror,
   and exposes it to Flare applications without recalculating the index.

Neither path may replace the PFTL replay threshold, relax a PFTL conflict, or
rewrite a finalized PFTL root.

### 2.2 Confidential Compute binding

An optional FCC extension must bind its signed response to:

```text
PFTL chain ID and genesis hash
series_key and epoch
index_epoch_id
all epoch-basis roots and counts
PFTL replay-key identity
FCC extension ID and code version
TEE platform and measurement
model/runtime manifest root
request nonce and expiry
```

The extension should not hold NAVCoin custody or portfolio execution keys. Its
only authority is to issue one bounded replay statement. Simulated TEE evidence
must be labeled `DEMO_SIMULATED_TEE`; only a verified hardware attestation may
be labeled `TESTNET_HARDWARE_ATTESTED_TEE`.

Flare proves that a measured program signed the stated result under its
attestation assumptions. It still does not prove that Qwen's narrative is
financially correct or that SEC management claims are true.

### 2.3 Hash and identity translation

PFTL identities are 48-byte SHA3-384 values and must never be truncated into an
EVM `bytes32`. The Flare contract retains the full PFTL digest as dynamically
encoded bytes and derives a local lookup key:

```text
flare_epoch_key = keccak256(abi.encode(
  "pftl.index.epoch.flare.v1",
  pftl_chain_id,
  pftl_genesis_hash_bytes48,
  series_key_bytes48,
  epoch,
  index_epoch_id_bytes48
))
```

The event and storage record include both `flare_epoch_key` and the complete
PFTL values. `abi.encodePacked` is forbidden for structured signing or identity
preimages.

### 2.4 Flare mirror state machine

The optional contract has only:

```text
UNKNOWN -> SUBMITTED -> PFTL_FINALIZED_MIRROR
UNKNOWN -> SUBMITTED -> REJECTED
```

It is non-upgradeable for v1, accepts one final result per
`flare_epoch_key`, rejects replayed signatures and wrong-domain responses, and
emits the PFTL finality evidence locator. It does not have a competing
`FINALIZED` state for an epoch that PFTL considers proposed or conflicted.

### 2.5 Optional Flare gates

- [ ] Finalize and verify the PFTL-only 50-issuer demo first.
- [ ] Register an FCC extension version whose input/output schema is limited to
  `IndexReplayStatementV1`.
- [ ] Run a simulated Coston2 relay test with explicit demo labeling.
- [ ] Run a hardware-attested replay and submit its receipt to PFTL as one
  replay operator.
- [ ] Deploy the non-upgradeable Flare mirror contract.
- [ ] Prove altered PFTL roots, truncated hashes, wrong chain domains, stale
  attestations, and duplicate mirror finalization are rejected.
- [ ] Demonstrate that PFTL finalization continues normally with the entire
  Flare path disabled.

The optional integration is successful only when it adds a separately visible
attestation or EVM-consumption path without becoming a hidden availability,
governance, or finality dependency for the canonical PFTL index.
