According to the May 2026 receipt-ratified synthesis, the right next move is still an execution plan, not another abstract whitepaper: the document says the composed Cobalt + deterministic inference + TensorCash/VeriLLM primitive has not yet been built or measured, and it names exactly the work we should operationalize first: PostFiat-shaped verification-cost measurement, rules-engine baselines, bundle-upgrade safety, receipt-ratified Cobalt amendments, and end-to-end testnet evidence.

I would keep the prior gate plan intact, but add one major new workstream:

> **Deterministic Governance Agent / Ruleset Genesis.**

This is the experiment you described: start with a high-level constitutional instruction like “choose a set of governance rules likely to optimize the value of a layer-one proof-of-authority cryptocurrency called PostFiat with the following architecture…,” run it under a pinned deterministic model stack, produce a typed ruleset, hash the output, replay it, and then eventually make that ruleset a Cobalt-ratified object.

You are not tripping. That is a valid experiment. The important distinction is:

> The deterministic agent can remove human judgment from routine governance **after humans ratify the bundle, objective statement, schema, and emergency rollback rules.**

That is the clean version.

---

# Updated target architecture

The execution layer should now have **two deterministic inference paths**:

```text
Path A — Governance Action Agent
Evidence snapshot → deterministic inference → validator registry delta / trust graph delta

Path B — Governance Rules Agent
Architecture statement + objective → deterministic inference → executable governance ruleset
```

Path A updates the chain.

Path B generates the rules that Path A must obey.

Both paths become Cobalt-governed artifacts.

```text
Cobalt-ratified GovernanceAgentBundle
  → deterministic ruleset generation
  → GovernanceRuleset hash
  → deterministic action generation
  → GovernanceAction hash
  → replay bundle
  → Cobalt shadow / dry-run / guarded-apply amendment
```

The PostFiat L1 already has the correct governance substrate: validator governance is explicit, signed, replayable, and part of chain state; Cobalt governs validator registries, trust graphs, amendments, and protocol transitions.  The receipt-ratified synthesis correctly identifies the old defect: the naive composition leaves the bundle, inputs, computation, output, and verification outside consensus, when they should be consensus objects.

---

# New object: `GovernanceAgentBundle`

This is the object that lets you deterministically generate rules without human judgment each round.

```rust
pub struct GovernanceAgentBundle {
    pub bundle_id: Hash,
    pub version: u32,

    // The “constitution prompt”
    pub architecture_statement_hash: Hash,
    pub objective_statement_hash: Hash,
    pub constitutional_constraints_hash: Hash,

    // Model/runtime
    pub model_weights_hash: Hash,
    pub tokenizer_hash: Hash,
    pub prompt_template_hash: Hash,
    pub output_schema_hash: Hash,
    pub runtime_image_hash: Hash,
    pub inference_engine_hash: Hash,
    pub deterministic_flags_hash: Hash,
    pub hardware_class_policy_hash: Hash,

    // Rule generation
    pub ruleset_schema_hash: Hash,
    pub ruleset_compiler_hash: Hash,
    pub ruleset_interpreter_hash: Hash,

    // Governance safety
    pub evidence_source_registry_root: Hash,
    pub rollback_policy_hash: Hash,
    pub activation_epoch: Epoch,
    pub expiry_epoch: Option<Epoch>,
}
```

The key addition is that the **architecture statement** and **objective statement** are hash-addressed governance objects.

Example objective:

```text
Choose a set of governance rules likely to optimize the long-term value,
security, credibility, institutional usefulness, and capture-resistance of
a layer-one proof-of-authority cryptocurrency called PostFiat.

PostFiat has:
- fixed 100B supply
- no native validator rewards
- fee burn
- Rust implementation
- ML-DSA-style post-quantum authorization
- Cobalt validator governance
- fast BFT ordering for ordinary transactions
- Orchard/Halo2-style confidential settlement
- natural-stakeholder validators
- explicit validator registry and trust graph state

Generate rules that minimize validator capture risk, maximize credible
settlement reliability, preserve low-cost validation, and avoid hidden
human discretion.
```

This is absolutely testable under SGLang deterministic inference. SGLang’s docs explicitly say even `temperature=0` can vary under normal serving because dynamic batching changes reduction order, and that SGLang deterministic inference addresses this with batch-invariant operations and the `--enable-deterministic-inference` flag.

---

# New output: `GovernanceRuleset`

The deterministic governance agent should not output prose. It should output a typed ruleset.

```rust
pub struct GovernanceRuleset {
    pub ruleset_id: Hash,
    pub generated_by_bundle: Hash,
    pub objective_hash: Hash,

    pub validator_scoring_rules: Vec<ScoringRule>,
    pub registry_mutation_rules: RegistryMutationRules,
    pub concentration_rules: ConcentrationRules,
    pub evidence_rules: EvidenceRules,
    pub trust_graph_rules: TrustGraphRules,
    pub model_disagreement_rules: DisagreementRules,
    pub rollback_rules: RollbackRules,
    pub no_op_rules: NoOpRules,
}
```

Example fields:

```json
{
  "ruleset_version": "postfiat.gov.ruleset.v1",
  "primary_objective": "maximize_long_run_postfiat_l1_value",
  "validator_scoring_rules": [
    {
      "name": "consensus_reliability",
      "weight": 0.30,
      "evidence_refs_required": ["vhs_24h", "vhs_30d"]
    },
    {
      "name": "operator_independence",
      "weight": 0.20,
      "evidence_refs_required": ["domain_attestation", "operator_cluster"]
    },
    {
      "name": "infrastructure_diversity",
      "weight": 0.15,
      "evidence_refs_required": ["asn", "country", "host"]
    }
  ],
  "registry_mutation_rules": {
    "max_adds_per_round": 1,
    "max_removes_per_round": 0,
    "hard_failure_remove_allowed": true,
    "minimum_score_for_add": 80,
    "churn_margin_delta": 8
  },
  "automatic_no_op_conditions": [
    "evidence_source_quorum_missing",
    "model_output_schema_invalid",
    "trust_graph_linkedness_fails",
    "receipt_verification_fails"
  ]
}
```

This should compile into a deterministic policy engine.

```text
GovernanceRuleset JSON
  → canonical hash
  → Rust policy/interpreter
  → golden fixture tests
  → Cobalt ratification
```

PAJAMA is directly relevant here. It argues for using LLMs to synthesize executable judging programs instead of relying only on LLM-as-judge scoring, reporting improved consistency and bias reduction versus a Qwen2.5-14B LLM-as-judge baseline.  That is almost exactly the design pattern we want: use the deterministic model to generate auditable executable rules, then run the rules deterministically.

---

# Add these gates to the existing plan

Keep Gates 0–12 from the prior plan. Add these.

## Gate 1.5 — Constitutional prompt bundle gate

**Goal:** Make the “choose rules likely to optimize PostFiat” instruction a governed object.

Inputs:

```text
architecture_statement.md
objective_statement.md
constitutional_constraints.md
ruleset_schema.json
```

Pass criteria:

```text
same statement bytes → same statement hash
any architecture/objective edit → new bundle hash
schema validates
forbidden scope expansions rejected
bundle is Cobalt-ratifiable in ShadowOnly mode
```

Output:

```text
reports/gov-inference-gate-1_5-constitutional-prompt-bundle.json
```

This lets you tell dev agents: “The agent’s values are not informal. They are hashed, versioned, and Cobalt-ratified.”

---

## Gate 3.5 — Deterministic ruleset generation gate

**Goal:** Prove the governance agent can deterministically generate the same ruleset from the same architecture/objective statement.

Run:

```text
provider = Modal H100!
model = Qwen/Qwen3.6-27B-FP8
runs = 50 initially, then 100, then 1000
input = architecture_statement + objective_statement + ruleset_schema
```

Pass criteria:

```text
50/50 valid JSON
50/50 same GovernanceRuleset hash
50/50 same compiled policy hash
50/50 same explanatory digest hash
```

Failure criteria:

```text
any schema drift
any policy hash drift
any malformed rule
any missing no-op condition
```

Output:

```text
reports/gov-inference-gate-3_5-deterministic-ruleset-generation.json
```

This is your core experiment.

---

## Gate 3.6 — Time-locked deterministic governance-agent replay

**Goal:** Test whether a future round can be deterministic but not precomputed.

Mechanism:

```text
T0: Cobalt ratifies GovernanceAgentBundle and evidence window.
T1: Cobalt emits round_seed = H(last_cobalt_certificate_hash, round_id, domain).
T2: model_request = H(bundle_id, evidence_root, round_seed, objective_hash).
T3: inferencer runs deterministic governance agent.
T4: everyone can replay the same output after seed is known.
```

Pass criteria:

```text
round output cannot be finalized before Cobalt seed
all honest runners get same ruleset/action hash after seed
stale seed rejected
wrong seed rejected
```

Output:

```text
reports/gov-inference-gate-3_6-timelocked-governance-agent.json
```

This addresses your “everyone has the same output so it can be time locked” intuition.

---

## Gate 7.5 — Ruleset compiler/interpreter gate

**Goal:** Make the generated rules executable, not just pretty JSON.

Pipeline:

```text
GovernanceRuleset JSON
  → canonicalize
  → compile/interpretable policy module
  → run on frozen evidence snapshots
  → produce deterministic RegistryDeltaCandidate
```

Pass criteria:

```text
same ruleset + same evidence → same registry delta
malformed rule rejected
non-terminating rule impossible
rule cannot access network
rule cannot call model
rule cannot mutate state directly
```

Output:

```text
reports/gov-inference-gate-7_5-ruleset-compiler.json
```

This is the first place where “human judgment removal” becomes concrete.

---

## Gate 7.6 — LLM-generated rules versus direct LLM judgment

**Goal:** Decide whether the governance-agent-generated ruleset is strong enough to govern routine deltas.

Run both:

```text
A. Direct LLM scorer/action generator
B. Deterministic generated ruleset
```

Same evidence snapshot.

Measure:

```text
top-k overlap
registry delta agreement
disagreement cases
churn behavior
concentration behavior
false add / false remove on gold fixtures
```

Pass criteria:

```text
ruleset agrees with direct scorer on high-confidence cases
ruleset rejects unsafe direct-scorer deltas
ruleset produces NoOp when evidence is ambiguous
manual review needed only for gate evaluation, not routine execution
```

Output:

```text
reports/gov-inference-gate-7_6-ruleset-vs-llm-judgment.json
```

This connects directly to the older Dynamic UNL limitation: the original paper already says model-assisted scoring should be retained only if it beats or complements deterministic rules on the same evidence.

---

## Gate 8.5 — Cobalt-ratified ruleset activation dry run

**Goal:** Put the generated ruleset into Cobalt governance state without letting it mutate the registry yet.

Mode:

```rust
ActionMode::DryRunValidate
```

Cobalt records:

```text
GovernanceAgentBundle hash
ArchitectureStatement hash
ObjectiveStatement hash
GovernanceRuleset hash
CompiledPolicy hash
ReplayBundle root
```

Pass criteria:

```text
Cobalt ratifies ruleset root
compiled policy hash stored
registry unchanged
ruleset replay bundle retrievable
stale ruleset rejected
ruleset generated from wrong bundle rejected
```

Output:

```text
reports/gov-inference-gate-8_5-cobalt-ruleset-dry-run.json
```

---

## Gate 9.5 — Guarded apply using generated ruleset

**Goal:** Let the generated ruleset, not a human, drive a tiny controlled registry mutation.

Flow:

```text
active GovernanceRuleset
active EvidenceSnapshot
ruleset interpreter produces RegistryDeltaCandidate
rules engine validates hard constraints
Cobalt ratifies GuardedApply
registry changes
rollback drill runs
```

Pass criteria:

```text
max one add
zero routine removals
all evidence refs valid
all concentration caps pass
trust graph linkedness passes
rollback available
no human approval after bundle/ruleset activation
```

Output:

```text
reports/gov-inference-gate-9_5-generated-ruleset-guarded-apply.json
```

This is the first milestone where you can honestly say:

> “A deterministic governance agent generated the rules, and those rules mutated the L1 registry through Cobalt.”

---

# Add a new dev-agent assignment

## Agent 8 — Deterministic Governance Agent / Ruleset Genesis Agent

Mission:

```text
Build the deterministic governance-agent experiment that generates executable
PostFiat governance rules from a Cobalt-ratified architecture/objective statement.
```

Tasks:

```text
1. Write architecture_statement.md for PostFiat L1.
2. Write objective_statement.md: optimize long-run value/security/credibility of PostFiat.
3. Write constitutional_constraints.md.
4. Define ruleset_schema.json.
5. Build model_request generator for ruleset generation.
6. Run ruleset generation through LocalDocker and Modal H100.
7. Canonicalize GovernanceRuleset JSON.
8. Hash GovernanceRuleset and compiled policy.
9. Build ruleset compiler/interpreter.
10. Build golden fixture suite.
11. Add Gate 3.5, 3.6, 7.5, 7.6, 8.5, 9.5 reports.
```

Output:

```text
PR: deterministic-governance-agent
CLI:
  postfiat-gov-agent generate-ruleset
  postfiat-gov-agent replay-ruleset
  postfiat-gov-agent compile-ruleset
  postfiat-gov-agent run-policy
```

---

# The agent prompt to hand them

```text
You are implementing the PostFiat Deterministic Governance Agent.

The goal is not to build a chatbot. The goal is to deterministically generate
a typed, executable governance ruleset from a Cobalt-ratified architecture
statement and objective statement.

The model must output only valid JSON under ruleset_schema.json. The output
must canonicalize to the same GovernanceRuleset hash across repeated runs under
the pinned inference bundle.

Initial objective:

"Choose a set of governance rules likely to optimize the long-term value,
security, credibility, institutional usefulness, and capture-resistance of a
layer-one proof-of-authority cryptocurrency called PostFiat."

Architecture constraints:

- Rust L1
- Cobalt validator governance
- fixed 100B supply
- no native validator rewards
- fee burn
- natural-stakeholder validators
- ML-DSA-style post-quantum authorization
- Orchard/Halo2-style private settlement
- fast BFT ordering for ordinary transactions
- validator registry and trust graph are chain state
- deterministic inference bundles are Cobalt-ratified

The generated ruleset must:
- minimize validator capture
- preserve liveness and settlement credibility
- minimize hidden human discretion
- define evidence-source requirements
- define registry mutation rules
- define concentration caps
- define no-op conditions
- define rollback triggers
- never allow direct model mutation of chain state
- never allow bundle self-upgrade
- never allow scope expansion without Cobalt ratification

Do not produce prose. Produce only GovernanceRuleset JSON.
```

---

# How the new research changes the plan

The research you pasted is relevant, but I would classify it into three buckets.

## Adopt now

### 1. PAJAMA-style generated rules

Use this immediately for the deterministic governance-agent track.

Reason: it directly supports the “generate executable rules, then run them deterministically” path. PAJAMA’s premise is that LLMs can synthesize executable judging programs, lowering cost and improving auditability compared with direct LLM-as-judge scoring.

Add to plan:

```text
Gate 7.5: ruleset compiler
Gate 7.6: generated rules vs direct LLM judgment
Agent 8: deterministic governance-agent
```

### 2. TOPLOC as a future receipt layer

TOPLOC is relevant for compact verification commitments. The paper reports a locality-sensitive hashing method for intermediate activations, with 258 bytes per 32 tokens versus 262 KB for storing token embeddings directly in their Llama-3.1-8B example, and says it is robust across hardware configurations and algebraic reorderings.

Add to plan, but not as a blocker:

```text
Gate 10.5: TOPLOC receipt prototype
```

### 3. VeriLLM as verifier-tier target

VeriLLM is still the right verifier-tier target. Its abstract claims public verifiability, about 1% verification cost, one-honest-verifier security, peer-prediction incentives, and a formal game-theoretic analysis.

But do not hardcode the 1% as a PostFiat fact yet. The receipt-ratified synthesis itself says the PostFiat-specific verification-cost bound must be empirically characterized.

Add:

```text
Gate 10.1: PostFiat-shaped VeriLLM benchmark
```

---

## Research-gated, not immediate

### 1. TP-invariant kernels

The newer TP-invariance work is relevant. The arXiv paper on deterministic inference across tensor-parallel sizes says TP-induced inconsistency comes from inconsistent reduction orders across GPUs and proposes tree-based invariant kernels for bitwise identical results across TP sizes.

But do not make TP>1 part of the first canonical bundle.

Keep:

```text
canonical inferencer: H100, TP=1
future verifier expansion: TP=2/4/8 after benchmark gate
```

Add:

```text
Gate 14: TP-invariant verifier admission
```

### 2. TALUS / threshold ML-DSA

TALUS is relevant for later compact ratifier signatures. The paper says it produces standard ML-DSA signatures verifiable by unmodified ML-DSA verifiers and targets one-round online threshold ML-DSA signing.

But do not block the first implementation on TALUS.

Start with:

```text
registry-root-bound ML-DSA signatures
Merkle roots for receipt batches
ordinary Cobalt certificates
```

Then add:

```text
Gate 15: threshold ML-DSA / TALUS ratifier aggregation
```

### 3. Merkle Tree Certificates

MTC is relevant to reducing post-quantum signature footprint. The cited paper says ML-DSA-65 signatures are 3,309 bytes and public keys are 1,952 bytes, and proposes Merkle Tree Certificates to reduce repeated PQ signature overhead in PKI settings.

But again: later optimization, not first integration.

Add:

```text
Gate 15.5: MTC-style receipt batch compression
```

---

## Do not adopt yet

### 1. Apple Silicon / MLX verifier tier

Keep as future hardware-class research only. It is too much variance for the initial Cobalt-integrated path.

### 2. Full zkML verification

Still too heavy for first integration. Keep as dispute-tier research.

### 3. Automatically generated bundle upgrades

Do not let the generated ruleset upgrade the bundle that generated it. Bundle upgrade remains a separate Cobalt path.

---

# Updated gate list

Here is the new full gate list.

```text
G0    Existing Cobalt baseline stays green
G1    Bundle schema compiles and hashes deterministically
G1.5  Constitutional prompt / objective bundle gate
G2    Local Docker deterministic replay
G3    Modal H100 deterministic replay
G3.5  Deterministic ruleset generation
G3.6  Time-locked deterministic governance-agent replay
G4    Provider portability test
G5    Cobalt shadow receipt amendment
G6    Replay verifier gate
G7    Deterministic action rules engine
G7.5  Generated ruleset compiler/interpreter
G7.6  Generated ruleset vs direct LLM judgment
G8    Registry delta dry run
G8.5  Cobalt-ratified ruleset activation dry run
G9    Guarded registry apply
G9.5  Guarded apply using generated ruleset
G10   Verifier tier prototype
G10.1 PostFiat-shaped VeriLLM benchmark
G10.5 TOPLOC receipt prototype
G11   Adversarial governance tests
G12   Integrated L1 governance release candidate
G13   Autonomous routine-governance soak
G14   TP-invariant verifier admission
G15   Threshold ML-DSA aggregation research gate
G15.5 MTC-style receipt batch compression gate
```

The first meaningful milestone is still **G5**.

The first L1 state-mutation milestone is still **G9**.

The first deterministic-agent milestone is **G9.5**.

The first “we have the vision working” milestone is:

```text
G13: Autonomous routine-governance soak
```

---

# New experiment list

## Experiment A — “Can a model deterministically generate governance rules?”

Input:

```text
architecture_statement.md
objective_statement.md
constitutional_constraints.md
ruleset_schema.json
```

Run:

```text
Qwen3.6-27B-FP8
SGLang deterministic
H100
TP=1
50/100/1000 repeats
```

Measure:

```text
JSON validity
ruleset hash uniqueness
compiled policy hash uniqueness
semantic diff across runs
```

Pass:

```text
1 unique ruleset hash
1 unique compiled policy hash
0 invalid outputs
```

## Experiment B — “Can the generated ruleset run the validator registry?”

Input:

```text
frozen validator evidence snapshots
generated GovernanceRuleset
prior registry
prior trust graph
```

Measure:

```text
registry delta stability
NoOp frequency
unsafe proposal rejection
churn behavior
concentration behavior
```

Pass:

```text
same snapshot → same registry delta
unsafe fixtures rejected
ambiguous fixtures NoOp
```

## Experiment C — “Can Cobalt ratify the generated ruleset?”

Input:

```text
GovernanceRuleset hash
compiled policy hash
replay bundle root
```

Pass:

```text
Cobalt shadow amendment accepted
ruleset stored in governance state
registry unchanged
```

## Experiment D — “Can generated rules mutate registry under caps?”

Input:

```text
active Cobalt-ratified GovernanceRuleset
new evidence snapshot
```

Pass:

```text
one add or NoOp
no routine removals
rollback succeeds
all validators converge
```

## Experiment E — “Can we time-lock the governance agent?”

Input:

```text
future Cobalt seed
active bundle
evidence root
```

Pass:

```text
output unavailable before seed
output replayable after seed
all runners get same hash
stale seed rejected
```

## Experiment F — “Does verifier-tier cost match theory?”

Input:

```text
same PostFiat prompt shape
same output
VeriLLM-style prefill verifier prototype
```

Measure:

```text
inference wall clock
verification wall clock
GPU seconds
energy estimate
provider variance
```

Pass:

```text
actual ratio measured and published
no paper claim used without local evidence
```

---

# Updated research addendum

The pasted research changes the plan in these ways:

| Research item                     | Plan impact                                                        |
| --------------------------------- | ------------------------------------------------------------------ |
| PAJAMA / program-as-judge         | Add deterministic ruleset-generation path immediately              |
| VeriLLM                           | Keep as verifier-tier target, but benchmark PostFiat shape locally |
| TOPLOC                            | Add optional compact activation-commitment gate                    |
| TP-invariant kernels              | Add future TP verifier scaling gate                                |
| TALUS / threshold ML-DSA          | Add future ratifier aggregation gate                               |
| Merkle Tree Certificates          | Add future receipt-batch compression gate                          |
| Apple Silicon / MLX               | Keep as future hardware-class admission research                   |
| M1 verification-cost disagreement | Resolve by benchmark gate, not argument                            |
| Evidence-source compromise        | Keep evidence-source registry as Cobalt state                      |
| Bundle-upgrade math               | Keep as separate research; do not allow self-upgrade               |

---

# Updated “vision-to-execution” statement

Use this internally:

> PostFiat will implement deterministic machine governance by Cobalt-ratifying the architecture statement, objective statement, inference bundle, evidence-source registry, generated ruleset, and replay bundle. Routine validator-registry changes will be generated by the active ruleset and active evidence snapshot, verified through deterministic replay and later verifier attestations, then applied only through Cobalt-certified state transitions. Human judgment is removed from routine governance after the bundle and ruleset are ratified; humans remain responsible only for genesis constraints, bundle upgrades, emergency rollback, and scope expansion.

That captures your vision without pretending the verification layer is already built.

---

# What I would tell the dev agents now

```text
Do not implement “AI governance” as a recommendation engine.

Implement:
1. GovernanceAgentBundle
2. ArchitectureStatement
3. ObjectiveStatement
4. GovernanceRuleset
5. Ruleset compiler/interpreter
6. Deterministic ruleset replay
7. Cobalt-ratified ruleset dry run
8. Guarded registry apply using generated ruleset

The initial experiment is:
“Given the PostFiat architecture and objective, deterministically generate the
rules likely to optimize the value/security/credibility of the L1.”

The output must be valid GovernanceRuleset JSON.
The output must hash identically across repeated deterministic runs.
The output must compile into a deterministic policy engine.
The policy engine must be able to produce registry deltas without human approval.
Cobalt must ratify the ruleset before it can affect state.
```

That is the correct addendum to the plan.

Based on the May 2026 PostFiat corpus, the right execution plan is **not** “write a bigger whitepaper.” It is:

> **Build a Cobalt-integrated deterministic inference control plane, prove it through gates, then let it mutate L1 governance state only after the gates pass.**

The plan below is what I would hand to dev agents.

---

# 1. Target architecture

## One-sentence architecture

**PostFiat L1 should treat deterministic inference as a Cobalt-ratified governance execution layer.**

The chain already has the right substrate: Cobalt governs validators, trust graphs, registry roots, amendments, and protocol transitions, while ordinary transactions stay on the fast certified ordering path.

The deterministic inference layer should therefore become a new Cobalt-governed subsystem:

```text
Evidence snapshot
  → deterministic inference bundle
  → generated governance output
  → receipt / replay bundle
  → Cobalt shadow amendment
  → Cobalt ratified amendment
  → guarded governance-state mutation
```

The existing Dynamic UNL work already gives the early off-chain shape: evidence collection, deterministic normalization, Qwen/Qwen3.6-27B-FP8 scoring, deterministic selector, staged artifact bundles, signed VL publication, and audit anchoring.  The missing work is to make those artifacts **native to Cobalt governance**, not merely published beside the chain.

---

# 2. The execution model

I would implement four components.

```text
postfiat-gov-orchestrator
postfiat-inference-runner
postfiat-governance-replay
postfiat-cobalt-adapter
```

## Component A — `postfiat-gov-orchestrator`

This coordinates a governance round.

Responsibilities:

```text
1. fetch active Cobalt governance state
2. fetch active InferenceBundle
3. freeze evidence snapshot
4. dispatch deterministic inference job to provider
5. collect model output
6. run deterministic rules engine
7. build receipt / replay bundle
8. submit Cobalt shadow amendment
9. optionally submit guarded apply amendment
```

It should not care whether the GPU came from Modal, Vast, or Runpod.

## Component B — `postfiat-inference-runner`

This is the GPU worker.

Responsibilities:

```text
1. load pinned model/runtime
2. accept canonical model_request.json
3. run SGLang deterministic inference
4. emit exact JSON according to schema
5. produce output hashes
6. return provider metadata
```

SGLang is the right starting point because its deterministic mode exists specifically to address temperature-zero nondeterminism caused by dynamic batching and floating-point reduction-order variation; the docs say deterministic inference uses batch-invariant operations and is enabled with `--enable-deterministic-inference`.

## Component C — `postfiat-governance-replay`

This is the deterministic verifier.

Responsibilities:

```text
1. recompute bundle hash
2. recompute evidence snapshot hash
3. recompute request hash
4. recompute model response hash
5. recompute parsed output hash
6. recompute selected registry delta
7. verify provider metadata
8. verify Cobalt amendment payload hash
```

At first this does **hash replay**, not full model replay.

Later it gets upgraded to verifier prefill / hidden-state sampling.

## Component D — `postfiat-cobalt-adapter`

This binds the output to the chain.

Responsibilities:

```text
1. define Cobalt amendment payload types
2. validate receipt-ratification amendments
3. store active bundle root in governance state
4. store evidence-source registry root
5. store receipt roots and replay bundle roots
6. apply generated registry deltas only after gates pass
```

This should live close to:

```text
crates/types
crates/consensus_cobalt
crates/execution
crates/node
crates/rpc_sdk
```

Those are already the relevant core crates in the Rust L1 architecture.

---

# 3. First implementation scope

Do **not** start with full VeriLLM. That is a later gate.

Start with:

> **Cobalt-ratified deterministic inference bundles with replayable receipts.**

Initial scope:

```text
routine validator-registry proposal generation
routine validator-registry delta dry runs
routine evidence-source registry hashing
routine policy/rules hash activation
Cobalt shadow amendments
```

Do not initially allow:

```text
automatic bundle upgrades
automatic evidence-source admission
automatic trust-graph activation
automatic emergency rollback
automatic production registry mutation
```

Those come after shadow and guarded gates.

---

# 4. Cobalt governance objects

## 4.1 `InferenceBundle`

This is the constitutional object for deterministic inference.

```rust
pub struct InferenceBundle {
    pub bundle_id: Hash,
    pub version: u32,

    pub model_id: String,
    pub model_weights_hash: Hash,
    pub tokenizer_hash: Hash,
    pub prompt_hash: Hash,
    pub output_schema_hash: Hash,

    pub normalization_rules_hash: Hash,
    pub deterministic_selector_hash: Hash,
    pub rules_engine_hash: Hash,

    pub runtime_image_hash: Hash,
    pub inference_engine_hash: Hash,
    pub deterministic_flags_hash: Hash,
    pub hardware_class_policy_hash: Hash,

    pub evidence_source_registry_root: Hash,
    pub verifier_policy_hash: Option<Hash>,
    pub rollback_policy_hash: Hash,

    pub activation_epoch: Epoch,
    pub expiry_epoch: Option<Epoch>,
}
```

The key principle:

> Changing the model, prompt, schema, runtime, provider container, deterministic flags, or selector is a Cobalt governance change.

## 4.2 `EvidenceSnapshot`

```rust
pub struct EvidenceSnapshot {
    pub round_id: GovernanceRound,
    pub registry_root_before: Hash,
    pub trust_graph_root_before: Hash,
    pub evidence_source_registry_root: Hash,
    pub canonical_json_hash: Hash,
    pub created_at_unix_ms: u64,
}
```

Evidence-source compromise is one of the biggest risks. A receipt can prove a model correctly computed the wrong answer from poisoned facts. The TensorCash/VeriLLM synthesis explicitly flags evidence-source manipulation as a first-class open issue.

## 4.3 `InferenceReceipt`

This is the object the runner returns.

```rust
pub struct InferenceReceipt {
    pub receipt_id: Hash,
    pub bundle_id: Hash,
    pub evidence_snapshot_root: Hash,
    pub model_request_hash: Hash,
    pub model_response_hash: Hash,
    pub parsed_output_hash: Hash,
    pub generated_action_hash: Hash,

    pub provider: InferenceProvider,
    pub provider_run_id: String,
    pub hardware_class: HardwareClass,
    pub runtime_manifest_hash: Hash,

    pub signer: ValidatorId,
    pub signature: SignatureBytes,
}
```

For the first implementation, this is not yet a TensorCash full-distribution receipt. It is a deterministic governance receipt. Later gates can add hidden-state roots, top-k logits, or full output distribution commitments.

## 4.4 `GeneratedGovernanceOutput`

```rust
pub enum GeneratedGovernanceOutput {
    ValidatorRegistryDelta(RegistryDelta),
    TrustGraphDelta(TrustGraphDelta),
    EvidenceSourceDelta(EvidenceSourceDelta),
    PolicyParameterDelta(PolicyParameterDelta),
    RulesProgramUpdate(RulesProgramUpdate),
    NoOp(NoOpReason),
}
```

Initial production candidate:

```rust
GeneratedGovernanceOutput::ValidatorRegistryDelta
```

Everything else should be shadow-only until separately gated.

## 4.5 `ReceiptRatificationAmendment`

```rust
pub struct ReceiptRatificationAmendment {
    pub amendment_id: Hash,
    pub round_id: GovernanceRound,

    pub prior_governance_root: Hash,
    pub bundle_id: Hash,
    pub evidence_snapshot_root: Hash,

    pub receipt_root: Hash,
    pub replay_bundle_root: Hash,
    pub generated_action_hash: Hash,

    pub action_mode: ActionMode,
    pub cobalt_domain: DomainTag,
}
```

```rust
pub enum ActionMode {
    ShadowOnly,
    DryRunValidate,
    GuardedApply,
    FullApply,
}
```

This is critical. The same pipeline can run in four modes without changing code.

---

# 5. Artifact contract

Use the staged bundle contract as the canonical file layout because the addendum says the newer staged verifier-ready contract replaces older flat artifact names.

```text
round_<id>/
  bundle.json
  inputs/
    validator_evidence.json
    model_request.json
    validator_map.json
    prior_registry.json
    prior_trust_graph.json
  runtime/
    execution_manifest.json
    provider_manifest.json
    container_manifest.json
  outputs/
    model_response.json
    parsed_governance_output.json
    validator_scores.json
    registry_delta_candidate.json
    verification_hashes.json
    inference_receipt.json
    cobalt_amendment_payload.json
```

Hash every file.

Then hash the whole tree:

```text
replay_bundle_root = merkle_root(all_artifact_hashes)
```

---

# 6. Provider abstraction

The inference layer should be provider-agnostic from day one.

```rust
pub enum InferenceProvider {
    Modal,
    Runpod,
    Vast,
    LocalDocker,
}
```

```rust
pub trait InferenceProviderClient {
    fn submit(&self, job: InferenceJob) -> Result<InferenceRunId>;
    fn status(&self, run_id: &InferenceRunId) -> Result<InferenceStatus>;
    fn fetch_result(&self, run_id: &InferenceRunId) -> Result<InferenceResult>;
}
```

## Required environment variables

```text
# common
HF_TOKEN
POSTFIAT_RPC_URL
POSTFIAT_CHAIN_ID
POSTFIAT_GOV_SIGNER_KEY_REF
POSTFIAT_ARTIFACT_BUCKET
POSTFIAT_ARTIFACT_S3_ENDPOINT
POSTFIAT_ARTIFACT_S3_ACCESS_KEY
POSTFIAT_ARTIFACT_S3_SECRET_KEY

# Modal
MODAL_TOKEN_ID
MODAL_TOKEN_SECRET
MODAL_SECRET_NAME

# Runpod
RUNPOD_API_KEY
RUNPOD_ENDPOINT_ID

# Vast
VAST_API_KEY
VAST_TEMPLATE_HASH_ID
VAST_OFFER_QUERY
VAST_SSH_KEY_NAME
```

Do **not** put provider API keys on chain. Cobalt sees only:

```text
provider
provider_run_id_hash
provider_manifest_hash
runtime_manifest_hash
receipt_hash
```

## Modal execution path

Modal supports specifying GPU types in the `gpu` argument, including H100, H200, and B200; importantly, Modal may automatically upgrade an `H100` request to H200, while `H100!` avoids that automatic upgrade, which matters for deterministic benchmarking.  Modal secrets can be injected into functions and accessed as environment variables.

Modal worker sketch:

```python
# infra/modal_gov_runner.py
import modal

image = (
    modal.Image.from_registry(
        "lmsysorg/sglang:nightly-dev-cu13-20260430-e60c60ef"
    )
    .pip_install("postfiat-gov-runner")
)

app = modal.App("postfiat-gov-inference")

@app.function(
    image=image,
    gpu="H100!",          # force H100; no H200 auto-upgrade during determinism gates
    timeout=1800,
    secrets=[modal.Secret.from_name("postfiat-gov-secrets")],
)
def run_governance_inference(job: dict) -> dict:
    from postfiat_gov_runner.runner import run_job
    return run_job(job)
```

Modal is best for:

```text
G2/G3 deterministic replay
canonical controlled H100 runs
foundation-operated inference runner
```

## Runpod execution path

Runpod Serverless uses handler functions that receive a JSON job object with an `input` field, and their docs show `runpod.serverless.start({"handler": handler})` as the required entrypoint.  Runpod queue-based endpoints accept synchronous `/runsync` and asynchronous `/run` jobs with bearer-token authorization and an `input` payload.

Runpod handler sketch:

```python
# infra/runpod_handler.py
import runpod
from postfiat_gov_runner.runner import run_job

def handler(job):
    return run_job(job["input"])

runpod.serverless.start({"handler": handler})
```

Runpod is best for:

```text
provider redundancy
queue-based shadow scoring
verifier-side jobs
non-canonical comparison runs
```

## Vast execution path

Vast is useful for raw instance lifecycle control. Vast’s API can create an instance by accepting an ask contract, provides an Authorization bearer-token pattern, and supports request fields such as Docker image, disk, launch mode, environment variables, and startup commands.  Its CLI docs show the practical workflow: set API key, search offers, create instance with Docker image and startup command, poll until running, connect/copy data, and destroy the instance.

Vast launch sketch:

```bash
vastai set api-key "$VAST_API_KEY"

vastai search offers \
  'gpu_name=H100 num_gpus=1 verified=true rentable=true direct_port_count>=1' \
  -o 'dlperf_usd-'

vastai create instance "$OFFER_ID" \
  --image ghcr.io/postfiat/postfiat-gov-runner:sha-<digest> \
  --disk 200 \
  --env "-e HF_TOKEN=$HF_TOKEN -e POSTFIAT_JOB_ID=$JOB_ID" \
  --onstart-cmd "postfiat-gov-runner run --job s3://bucket/jobs/$JOB_ID.json" \
  --ssh \
  --direct
```

Vast is best for:

```text
cost comparison
fallback GPU capacity
hardware-class experiments
cross-provider reproducibility tests
```

---

# 7. SGLang launch profile

Canonical launch profile for first gates:

```bash
python3 -m sglang.launch_server \
  --model-path Qwen/Qwen3.6-27B-FP8 \
  --attention-backend fa3 \
  --enable-deterministic-inference \
  --tp 1 \
  --host 0.0.0.0 \
  --port 30000
```

The execution manifest should pin:

```json
{
  "model": "Qwen/Qwen3.6-27B-FP8",
  "gpu": "H100",
  "tensor_parallelism": 1,
  "attention_backend": "fa3",
  "deterministic": true,
  "temperature": 0,
  "max_running_requests": 1,
  "container_digest": "...",
  "prompt_hash": "...",
  "schema_hash": "..."
}
```

The reason to keep `TP=1` at first is simple: multi-GPU deterministic inference remains an active research/implementation area. Recent research on tree-based invariant kernels targets bitwise reproducibility across tensor-parallel sizes, but that should be treated as a future expansion gate, not as the first implementation assumption.

---

# 8. Gate plan

This is the important part.

Do not let dev agents jump directly to “AI updates validator set.” Make them clear gates.

## Gate 0 — Existing Cobalt baseline stays green

**Goal:** Prove we did not break the chain.

Inputs:

```text
current controlled testnet
current consensus_cobalt tests
current amendment replay tests
current adversarial packet tests
```

Pass criteria:

```text
Cobalt readiness gate green
amendment replay green
unsafe trust graphs still fail closed
stale replay rejection still works
fast ordering unaffected
```

Why: the L1 paper says Cobalt already has non-identical trust views, essential subsets, linkedness checks, fail-closed unsafe graph rejection, non-uniform certificates, RBC/ABBA/MVBA/DABC, registry transitions, replay rejection, and adversarial packet gates.

Output:

```text
reports/gov-inference-gate-0-cobalt-baseline.json
```

No inference code touches governance before this passes.

---

## Gate 1 — Bundle schema compiles and hashes deterministically

**Goal:** Define the Cobalt-visible inference bundle without running GPUs.

Dev tasks:

```text
crates/governance_inference_types
  - InferenceBundle
  - EvidenceSnapshot
  - InferenceReceipt
  - GeneratedGovernanceOutput
  - ReceiptRatificationAmendment

crates/governance_artifacts
  - canonical JSON
  - Merkle tree
  - domain-separated hashes
```

Pass criteria:

```text
same bundle bytes → same bundle_id
different prompt → different bundle_id
different model hash → different bundle_id
different runtime flag → different bundle_id
malformed bundle rejected
```

Output:

```text
reports/gov-inference-gate-1-bundle-hashing.json
```

---

## Gate 2 — Local Docker deterministic replay

**Goal:** Prove the runner can execute the full job locally before cloud providers enter.

Run:

```bash
docker run --gpus all ghcr.io/postfiat/postfiat-gov-runner:sha-<digest> \
  postfiat-gov-runner run \
  --job fixtures/jobs/pft_testnet_round_001.json \
  --out artifacts/gate2/
```

Pass criteria:

```text
5/5 local runs parse
5/5 produce same parsed_governance_output_hash
5/5 produce same registry_delta_candidate_hash
5/5 artifact tree roots match
```

Output:

```text
reports/gov-inference-gate-2-local-docker-determinism.json
```

Failure action:

```text
do not involve Modal/Vast/Runpod
fix canonicalization or runner
```

---

## Gate 3 — Modal canonical H100 deterministic replay

**Goal:** Establish the canonical inference environment.

Run:

```text
provider = Modal
gpu = H100!
runs = 50
same input bundle
same evidence snapshot
```

Pass criteria:

```text
50/50 successful
50/50 valid JSON
1 unique parsed output hash
1 unique registry delta hash
1 unique replay bundle root
mean/p95 latency recorded
token counts recorded
provider metadata recorded
```

Why Modal first: it supports H100/H100!, and `H100!` avoids automatic upgrade to H200 during benchmark runs.

Output:

```text
reports/gov-inference-gate-3-modal-h100-determinism.json
```

---

## Gate 4 — Provider portability test

**Goal:** Discover what changes across Modal, Runpod, and Vast.

Run:

```text
Modal H100!
Runpod H100 or closest available H100-equivalent
Vast H100 verified instance
same container digest
same model hash
same prompt hash
same request JSON
```

Pass criteria:

```text
each provider independently deterministic within its provider/hardware class
cross-provider exact match recorded if it happens
cross-provider mismatch does not fail the gate if each class is internally stable
```

Important: do **not** require Modal/Vast/Runpod to be bit-identical at this stage. Treat each as a hardware/runtime class.

Output:

```text
reports/gov-inference-gate-4-provider-portability.json
```

Failure action:

```text
provider class with internal nondeterminism is excluded
cross-provider divergence becomes verifier-policy input
```

---

## Gate 5 — Cobalt shadow amendment

**Goal:** Put inference artifacts into Cobalt without mutating validator state.

Mode:

```rust
ActionMode::ShadowOnly
```

Cobalt amendment contains:

```text
bundle_id
evidence_snapshot_root
receipt_root
replay_bundle_root
generated_action_hash
```

Pass criteria:

```text
Cobalt ratifies shadow amendment
governance state records receipt root
registry root unchanged
trust graph root unchanged
replay bundle retrievable
stale receipt rejected
tampered receipt rejected
wrong bundle_id rejected
```

Output:

```text
reports/gov-inference-gate-5-cobalt-shadow-amendment.json
```

This is the first real integration point.

---

## Gate 6 — Replay verifier gate

**Goal:** Any node can verify the round without trusting the orchestrator.

Command:

```bash
postfiat-gov-replay verify \
  --bundle artifacts/round_001/ \
  --expected-root <replay_bundle_root>
```

Pass criteria:

```text
recomputes every artifact hash
recomputes receipt_id
recomputes generated_action_hash
recomputes Cobalt amendment payload hash
detects one-byte mutation in every artifact class
```

Output:

```text
reports/gov-inference-gate-6-replay-verifier.json
```

---

## Gate 7 — Deterministic rules engine

**Goal:** Separate model text from executable governance.

The model output must be parsed into a typed governance output. Then a deterministic rules engine checks it.

Rules engine rejects:

```text
schema invalid
evidence ref missing
score outside allowed range
churn cap violation
operator concentration violation
ASN concentration violation
country/jurisdiction concentration violation
unsafe trust graph
unlinked trust graph
missing receipt
wrong bundle
wrong prior registry root
```

Pass criteria:

```text
all valid generated outputs accepted
all malformed outputs rejected
all hard-rule violations rejected
golden fixtures stable across runs
```

Output:

```text
reports/gov-inference-gate-7-rules-engine.json
```

This is where “no human interference” becomes safe: the machine can generate, but the deterministic rules decide whether the output is executable.

---

## Gate 8 — Registry delta dry run

**Goal:** Prove the generated delta would apply cleanly, but do not apply it yet.

Mode:

```rust
ActionMode::DryRunValidate
```

Pass criteria:

```text
Cobalt ratifies dry-run amendment
execution layer simulates registry transition
new registry root computed
old registry root remains active
trust graph linkedness check passes
rollback root computed
```

Output:

```text
reports/gov-inference-gate-8-registry-delta-dry-run.json
```

---

## Gate 9 — Guarded registry apply on controlled testnet

**Goal:** Let inference-generated governance mutate the controlled testnet registry under strict caps.

Mode:

```rust
ActionMode::GuardedApply
```

Constraints:

```text
max_adds_per_round = 1
max_removes_per_round = 0 unless hard-failure evidence
max_total_churn = 3%
manual emergency rollback available
one active bundle only
one provider class only
```

Pass criteria:

```text
registry root changes
Cobalt certificate binds old and new roots
replay bundle root stored
old state can be restored through rollback amendment
all validators accept new registry root
no fast-path transaction regression
```

Output:

```text
reports/gov-inference-gate-9-guarded-apply.json
```

This is the first true integrated L1 milestone.

---

## Gate 10 — Verifier tier prototype

**Goal:** Add verifier attestations after the receipt exists.

Initial verifier does not need full VeriLLM. Start with:

```text
hash replay verifier
schema verifier
selector verifier
optional local model replay verifier
```

Then graduate to:

```text
prefill verification
hidden-state root
VRF sampled openings
comparison metrics
verifier attestation root
```

Pass criteria for first verifier gate:

```text
at least 3 independent verifier jobs
verifier attestations stored
incorrect receipt rejected
correct receipt accepted
verifier disagreement recorded
```

Output:

```text
reports/gov-inference-gate-10-verifier-tier.json
```

The synthesis document itself says the composed TensorCash/VeriLLM/Cobalt system has not been built or measured and identifies verification cost, receipt schema, and Cobalt amendment integration as open work.

---

## Gate 11 — Adversarial governance tests

**Goal:** Try to break the machine governance path.

Test cases:

```text
wrong bundle_id
wrong prompt_hash
wrong evidence root
stale prior registry root
provider returns malformed JSON
provider returns valid JSON with fake evidence refs
model emits registry delta violating churn cap
model emits trust graph that fails linkedness
receipt copied from previous round
artifact tree root tampered
verifier quorum missing
Cobalt amendment replayed after activation
provider API failure mid-round
```

Pass criteria:

```text
every unsafe case fails closed
last known-good registry remains active
no partial governance transition
all failures produce machine-readable reason codes
```

Output:

```text
reports/gov-inference-gate-11-adversarial-governance.json
```

---

## Gate 12 — Integrated L1 governance release candidate

**Goal:** End-to-end controlled-testnet machine governance.

Pass criteria:

```text
one active InferenceBundle is Cobalt-ratified
one evidence-source registry is Cobalt-ratified
one deterministic inference round runs on provider
one receipt root is Cobalt-ratified
one generated validator-registry delta is guarded-applied
one rollback drill succeeds
one replay verifier independently validates artifacts
one verifier-tier prototype validates receipt
no ordering/finality regression
```

Output:

```text
reports/gov-inference-gate-12-integrated-l1-rc.json
```

This is the point where you can tell people:

> “Deterministic inference is integrated into the L1 governance path.”

Not before.

---

# 9. Research workstream

Run research alongside implementation, not instead of it.

## Research Track A — Determinism

Questions:

```text
Does Qwen3.6-27B-FP8 produce identical outputs across 50/100/1000 runs on H100?
Does provider class change output?
Does H100 vs H200 change output?
Does FA3 vs FlashInfer change output?
Does Runpod/Vast/Modal provider metadata correlate with divergence?
```

Deliverable:

```text
determinism-matrix.md
determinism-matrix.json
```

External context: SGLang’s docs explain the batching/reduction-order root cause and deterministic-inference solution.  Recent deterministic-inference research is also exploring verified speculation and TP-invariant kernels, but those are expansion paths, not the initial L1 integration dependency.

## Research Track B — Verification cost

Questions:

```text
What is actual wall-clock full inference time?
What is replay verification time?
What is prefill verification time if implemented?
What is energy/GPU-second cost?
What is p50/p95 across providers?
```

The existing synthesis already identifies the PostFiat workload shape issue: a naive verification-cost ratio of `7654 / 12428 ≈ 0.616` must be characterized empirically rather than assumed from generic VeriLLM claims.

Deliverable:

```text
verification-cost-benchmark.md
verification-cost-benchmark.json
```

## Research Track C — Receipt structure

Questions:

```text
Is hash replay enough for Gate 5-9?
When do we add hidden-state roots?
When do we add top-k logits?
When do we add full TensorCash-style distribution commitment?
How large are receipts under ML-DSA?
```

Deliverable:

```text
receipt-structure-spec.md
```

## Research Track D — Rules generation

Questions:

```text
Can the model produce valid typed deltas 100/100 times?
Can a deterministic rules engine reject unsafe deltas?
Can a generated rules program run beside the model scorer?
What divergence threshold triggers automatic no-op?
```

Deliverable:

```text
rules-engine-spec.md
rules-engine-golden-fixtures/
```

## Research Track E — Cobalt composition

Questions:

```text
Where exactly should receipt amendments enter RBC/ABBA/MVBA/DABC?
Can replay bundles be part of amendment replay history?
What stale-replay protections are needed?
What payload sizes break existing tests?
```

Deliverable:

```text
cobalt-receipt-amendment-spec.md
```

The receipt-ratified synthesis specifically calls for a Cobalt-compatible schema, replay-bundle integration, fail-closed malformed-receipt handling, and adversarial-packet test extensions.

---

# 10. Dev agent assignments

## Agent 1 — Cobalt schema agent

Mission:

```text
Add receipt-ratified amendment types to crates/types and crates/consensus_cobalt.
```

Tasks:

```text
1. define InferenceBundle
2. define EvidenceSnapshot
3. define InferenceReceipt
4. define ReceiptRatificationAmendment
5. add ActionMode enum
6. add canonical serialization
7. add domain-separated hashes
8. add malformed payload tests
9. add stale replay tests
10. add shadow amendment path
```

Output:

```text
PR: cobalt-receipt-amendments
tests: consensus_cobalt::receipt_amendment_*
```

## Agent 2 — Provider runner agent

Mission:

```text
Build provider-agnostic runner for Modal, Runpod, Vast, and LocalDocker.
```

Tasks:

```text
1. implement InferenceProviderClient trait
2. implement Modal client
3. implement Runpod client
4. implement Vast client
5. implement LocalDocker client
6. implement provider_manifest.json
7. implement retry/backoff
8. implement artifact upload
9. implement job status polling
10. implement failure reason codes
```

Output:

```text
PR: provider-inference-runner
CLI: postfiat-gov-runner submit --provider modal|runpod|vast|local
```

## Agent 3 — Determinism/replay agent

Mission:

```text
Make every round replayable from artifacts.
```

Tasks:

```text
1. canonical JSON serializer
2. Merkle artifact tree
3. replay verifier CLI
4. mutation tests
5. hash stability tests
6. model response parser
7. parsed output canonicalization
8. registry delta hash
9. full replay bundle root
```

Output:

```text
PR: governance-replay
CLI: postfiat-gov-replay verify --bundle <path>
```

## Agent 4 — Rules engine agent

Mission:

```text
Convert model output into executable, constrained governance deltas.
```

Tasks:

```text
1. define GeneratedGovernanceOutput schema
2. define RegistryDeltaCandidate
3. implement concentration checks
4. implement churn caps
5. implement evidence-ref checks
6. implement trust graph linkedness precheck
7. implement deterministic no-op fallback
8. implement golden fixtures
```

Output:

```text
PR: governance-rules-engine
tests: governance_policy::*
```

## Agent 5 — Orchestrator agent

Mission:

```text
End-to-end governance round orchestration.
```

Tasks:

```text
1. fetch active governance state
2. freeze evidence snapshot
3. build model_request.json
4. call provider client
5. run rules engine
6. build receipt
7. build Cobalt amendment payload
8. submit shadow amendment
9. poll Cobalt ratification
10. publish reports
```

Output:

```text
PR: gov-orchestrator
CLI: postfiat-gov round run --mode shadow|dry-run|guarded-apply
```

## Agent 6 — Verification research/prototype agent

Mission:

```text
Prototype verifier tier after shadow receipts work.
```

Tasks:

```text
1. hash replay verifier
2. local model replay verifier
3. provider replay verifier
4. prefill verifier research
5. hidden-state commitment prototype
6. VRF sample policy
7. verifier attestation schema
```

Output:

```text
PR: verifier-tier-prototype
report: verification-cost-benchmark.json
```

## Agent 7 — Gate harness agent

Mission:

```text
Turn the gates into scripts and CI artifacts.
```

Tasks:

```text
scripts/gov-gate-0-cobalt-baseline
scripts/gov-gate-1-bundle-hash
scripts/gov-gate-2-local-docker
scripts/gov-gate-3-modal-h100
scripts/gov-gate-4-provider-portability
scripts/gov-gate-5-cobalt-shadow
scripts/gov-gate-6-replay
scripts/gov-gate-7-rules
scripts/gov-gate-8-dry-run
scripts/gov-gate-9-guarded-apply
scripts/gov-gate-10-verifier
scripts/gov-gate-11-adversarial
scripts/gov-gate-12-integrated-l1
```

Output:

```text
reports/gov-inference-gates/<gate-id>/*.json
```

---

# 11. Minimal first sprint

This is what I would do first.

## Sprint 1 deliverables

```text
1. Rust governance inference types
2. canonical artifact hashing
3. LocalDocker runner
4. Modal runner
5. one fixture evidence snapshot
6. one fixture model_request.json
7. one shadow-only Cobalt amendment type
8. replay verifier CLI
9. Gate 1, Gate 2, Gate 3 scripts
```

## Sprint 1 success criteria

```text
same fixture can run locally and on Modal
Modal H100! produces 50/50 same parsed output hash
artifact tree root is stable
Cobalt accepts shadow amendment
registry does not change
```

That is the first real proof.

---

# 12. Minimal request/response schema

## Inference job request

```json
{
  "job_version": "postfiat.gov.inference_job.v1",
  "chain_id": "postfiat-testnet",
  "round_id": 1,
  "bundle_id": "hash...",
  "evidence_snapshot_root": "hash...",
  "model_request": {
    "messages": [],
    "response_schema": {},
    "temperature": 0,
    "max_tokens": 6000
  },
  "artifact_upload": {
    "bucket": "postfiat-gov-artifacts",
    "prefix": "round_1/"
  }
}
```

## Inference result

```json
{
  "result_version": "postfiat.gov.inference_result.v1",
  "round_id": 1,
  "provider": "modal",
  "provider_run_id": "abc123",
  "hardware_class": "H100",
  "runtime_manifest_hash": "hash...",
  "model_response_hash": "hash...",
  "parsed_output_hash": "hash...",
  "generated_action_hash": "hash...",
  "artifact_root": "hash...",
  "error": null
}
```

## Failure result

```json
{
  "result_version": "postfiat.gov.inference_result.v1",
  "round_id": 1,
  "provider": "runpod",
  "status": "failed",
  "error": {
    "code": "MODEL_OUTPUT_SCHEMA_INVALID",
    "message": "missing registry_delta_candidate.adds"
  }
}
```

Failure results are useful. They become part of the test harness.

---

# 13. What “fully integrated L1” means

Do not define integrated L1 as “AI made a recommendation.”

Define it as:

```text
1. active InferenceBundle root is in Cobalt governance state
2. active EvidenceSourceRegistry root is in Cobalt governance state
3. governance round freezes evidence and produces receipt
4. receipt root is Cobalt-ratified
5. replay bundle is independently verifiable
6. generated governance output is rules-checked
7. Cobalt certificate binds old governance root and new governance root
8. registry/trust/policy state mutates only after certificate validation
```

That is integrated.

---

# 14. A realistic progression from vision to L1

## Phase A — Deterministic inference outside chain

```text
LocalDocker + Modal
no Cobalt
no registry mutation
```

Exit:

```text
same input → same output
```

## Phase B — Cobalt shadow receipt

```text
receipt root enters Cobalt
no registry mutation
```

Exit:

```text
Cobalt ratifies receipt root
```

## Phase C — Cobalt dry-run generated governance

```text
generated registry delta simulated
no active state change
```

Exit:

```text
new registry root computed but not activated
```

## Phase D — Cobalt guarded apply

```text
small registry mutation allowed
rollback required
```

Exit:

```text
machine-generated registry delta changes L1 state
```

## Phase E — Verifier tier

```text
hash replay verifiers
model replay verifiers
prefill verifier research
```

Exit:

```text
receipt requires verifier quorum
```

## Phase F — Autonomous routine governance

```text
routine registry updates no human approval
humans only for bundle upgrade / rollback / scope change
```

Exit:

```text
deterministic machine governance under Cobalt constraints
```

---

# 15. Dev-agent prompt

Use this for the coding agents:

```text
You are implementing PostFiat Cobalt-integrated deterministic inference governance.

Do not write a whitepaper. Build the execution layer.

The chain already has Rust types, Cobalt governance, fast ordering, ML-DSA-style signatures, and replayable governance state. Your task is to add a deterministic inference governance layer that can run on LocalDocker, Modal, Runpod, and Vast, emit replayable artifacts, and submit Cobalt shadow amendments before any state mutation.

Implement only the assigned work package. Respect the gates. Do not let inference mutate validator state before Gate 9.

Canonical initial model profile:
- Qwen/Qwen3.6-27B-FP8
- SGLang deterministic inference
- H100
- TP=1
- attention backend fa3
- temperature=0
- pinned container digest
- staged verifier-ready artifacts

Required artifacts:
- bundle.json
- inputs/validator_evidence.json
- inputs/model_request.json
- inputs/validator_map.json
- runtime/execution_manifest.json
- runtime/provider_manifest.json
- outputs/model_response.json
- outputs/parsed_governance_output.json
- outputs/registry_delta_candidate.json
- outputs/verification_hashes.json
- outputs/inference_receipt.json
- outputs/cobalt_amendment_payload.json

Never store provider API keys on chain. Only store hashes, roots, signatures, and Cobalt amendment payloads.

Every gate must emit a JSON report under:
reports/gov-inference-gates/<gate-id>/
```

---

# 16. The core plan in one page

Build this in order:

```text
G0  keep existing Cobalt green
G1  define bundle/receipt/amendment schemas
G2  run deterministic inference locally
G3  run deterministic inference on Modal H100!
G4  compare Modal / Runpod / Vast provider classes
G5  submit Cobalt shadow receipt amendment
G6  replay verifier proves artifact root
G7  deterministic rules engine validates generated output
G8  Cobalt dry-runs registry delta
G9  Cobalt guarded-applies tiny registry delta
G10 add verifier attestations
G11 adversarial governance tests
G12 integrated L1 release candidate
```

The first meaningful milestone is **Gate 5**.

The first real L1 milestone is **Gate 9**.

The full integrated deterministic inference governance milestone is **Gate 12**.
