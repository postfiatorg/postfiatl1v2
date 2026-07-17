# Receipt-Ratified Deterministic LLM Oracle Governance for XRPL-Style Settlement Networks

**A Research Synthesis Bridging Post Fiat Dynamic UNL Publication, the PostFiat Rust L1 with Cobalt Governance, and Verifiable Decentralized Inference Primitives**

**Working draft, May 2026**
**Status:** Internal research brief intended as input to follow-on agent-driven research

---

## Abstract

This paper synthesizes four previously disjoint research artifacts — the Post Fiat Dynamic Unique Node List (UNL) publication design, the PostFiat Rust Layer 1 architecture with Cobalt-derived governance, the TensorCash AI proof-of-work receipt structure, and the VeriLLM publicly verifiable decentralized inference protocol — into a single proposed primitive: **receipt-ratified deterministic LLM oracle governance**.

The core claim is structural rather than empirical. Cobalt governance provides a ratification mechanism for trust-graph and validator-registry transitions in a known-validator settlement chain. Deterministic LLM inference under a pinned model bundle (SGLang batch-invariant kernels, single-GPU greedy decoding, fixed quantization, pinned container image) provides a function from canonical inputs to canonical outputs that is reproducible across honest participants. TensorCash receipts provide a compact cryptographic commitment to that function's execution. VeriLLM's full-sequence prefill verification provides a way to verify such a receipt at approximately one percent of the cost of producing it. Composed together, these primitives admit a governance architecture in which (a) a small inferencer tier executes the pinned governance computation, (b) a larger verifier tier ratifies the resulting receipt at low cost using VeriLLM's commit-reveal-sample discipline, and (c) Cobalt amendment machinery anchors the ratified receipt as authoritative validator-registry or amendment state.

This composition is, to the authors' knowledge, novel. It has not been implemented as a live consensus primitive on any existing chain. The empirical evidence currently available — the 2,900-call zero-variance Qwen3.6/SGLang replay on the Post Fiat XRPL UNL credibility cohort, the five-run zero-variance PFT Ledger `scoring_v2` replay, and VeriLLM's prototype 1% verification overhead — supports the feasibility of each component in isolation but does not constitute production evidence for the composed system. This document is therefore a research brief, not a deployment proposal.

The paper proceeds in six parts. Sections 1–2 summarize the Post Fiat Dynamic UNL publication design and the PostFiat Rust L1 architecture. Section 3 analyzes the naive composition of the two and identifies its structural defects. Section 4 introduces the TensorCash and VeriLLM primitives and their relevant properties. Section 5 proposes the receipt-ratified synthesis and develops the necessary mathematics. Section 6 enumerates open design questions and proposes a structured research brief intended for use by follow-on research agents with full context of this document.

---

## 1. Background I: Post Fiat Dynamic UNL Publication

### 1.1 Problem and architecture

XRPL-style consensus protocols depend critically on the composition of each server's Unique Node List (UNL) — the set of validators a server trusts not to collude. Worst-case safety analyses establish that pairwise UNL overlap must remain above approximately 90% to prevent fork conditions [Chase & MacBrough, 2018; Amores-Sesar, Cachin, Mićić, 2021]. In practice this overlap is maintained by widely consumed signed recommended-validator lists published by foundation operators. List construction is opaque: participants observe the published set but not the criteria, evidence, or judgment underlying it.

The Post Fiat Dynamic UNL design replaces opaque publisher discretion with a six-stage auditable pipeline:

1. **Evidence collection.** Validator consensus performance (from a Validator History Service), network topology (from `/crawl`), Autonomous System Number enrichment (from pyasn over BGP tables), country-level geolocation (from DB-IP Lite), domain attestation (via `xrp-ledger.toml`), and software/governance signals.

2. **Deterministic normalization.** Raw evidence is canonicalized into a typed snapshot whose SHA-256 hash binds the scoring round's inputs.

3. **Model-assisted scoring.** A pinned open-weight model (currently `Qwen/Qwen3.6-27B-FP8`, served through Modal with SGLang deterministic inference) consumes the snapshot under a published scoring prompt (`prompts/scoring_v5.txt`) and emits an integer score in [0,100] per validator with a rationale.

4. **Deterministic selector.** A typed function $U_t = G(S_t, U_{t-1}; \theta, K, \delta)$ where $S_t$ is the score vector, $U_{t-1}$ is the prior round's UNL, $\theta$ is a minimum-score threshold, $K$ is the maximum list size, and $\delta$ is the churn-control margin. Challengers displace incumbents only on a score advantage exceeding $\delta$ or on hard-failure conditions.

5. **Artifact bundle.** A staged verifier-ready bundle is produced with the schema `bundle.json → inputs/{evidence, model_request, validator_map} → runtime/execution_manifest.json → outputs/{model_response, scores, selected_unl, signed_validator_list, verification_hashes}`.

6. **Publication.** The signed validator list is published at the canonical `postfiat.org/{env}_vl.json` path with a future `effective` activation timestamp; the audit bundle is pinned to IPFS; the bundle CID is anchored on-chain via a PFTL memo transaction.

### 1.2 Determinism evidence

Under the pinned execution stack (Qwen3.6-27B-FP8, single H100 with tensor parallelism TP=1, SGLang `--enable-deterministic-inference`, container image `lmsysorg/sglang:nightly-dev-cu13-20260430-e60c60ef@sha256:5d9ec71597...`, FlashInfer workspace 2 GiB, chunked prefill size 4096, max running requests 1, greedy decoding), the system has produced:

- **XRPL UNL credibility replay (2026-05-05).** 2,900 calls across 29 validator domains with two batches of 50 repeats each. Zero errors, zero parse failures, 100 complete score maps, exactly one unique score-map SHA-256 hash (`9f7f95a7be238e2b6bb1cc081986f8b5dffc07b9397578d723c6f6d7c77c81c8`), zero domains with score variance, zero domains with raw-output variance.

- **PFT Ledger `scoring_v2` replay.** 5 independent runs scoring all 42 testnet validators. 5/5 JSON-valid runs, 1 extracted-answer hash, 1 score-map hash, 1 top-35 hash, zero validators with score spread, 35/35 top-35 intersection across all runs.

These observations exceed the conventional rank-stability target (pairwise rank agreement > 95%, top-k overlap > 90%). Under a fully pinned execution environment, exact replayability is operationally achievable, not merely statistical stability.

### 1.3 Phased deployment as originally specified

The Dynamic UNL design proposes a four-phase deployment:

- **Phase 1:** Foundation operates the pipeline. Authority is foundation-controlled. Audit trails are public.
- **Phase 2:** Validators run independent shadow scoring via local sidecars. Commit-reveal protocols prevent output copying. Convergence is measured.
- **Phase 3A:** Authoritative list content transfers from foundation output to validator-converged output if convergence thresholds are met.
- **Phase 3B:** Publication infrastructure decentralizes — snapshot assembly, round announcement, list signing, artifact hosting.

### 1.4 Acknowledged limitations

The Dynamic UNL paper explicitly bounds its claim:

- Model scores are qualitative assessments, not mathematical proofs.
- Convergent model outputs do not by themselves imply social independence of validators.
- Exact reproducibility requires policy constraints on the execution environment.
- The Phase 0 evidence is sufficient for feasibility but not for production authority transfer.
- The model layer should be retained only if it outperforms a deterministic rules engine on the same evidence.

### 1.5 Structural defects of the design as specified

The design is correct in its architectural direction but inherits two limitations from its XRPL-publisher legacy:

1. **Publisher-key infrastructure as the trust anchor.** Lists are signed by foundation publisher keys. The 2025 XRPL default-UNL migration demonstrated that publisher-key rotation is itself a high-stakes governance event requiring operator coordination.

2. **Phased authority transfer as a workaround for missing governance machinery.** Phases 1–3 exist because the design has no native mechanism to ratify validator-converged output as authoritative chain state. Authority transfer must be social and operational rather than protocol-level.

Both limitations dissolve in a chain with native validator-registry governance, which the PostFiat Rust L1 provides.

---

## 2. Background II: PostFiat Rust L1 and Cobalt-Derived Governance

### 2.1 Design pillars

The PostFiat Rust L1 is a greenfield rebuild of the XRP-style authority-validator settlement pattern. It preserves XRP's incentive model (fixed supply, no native validator subsidy, fee burn, natural-stakeholder validators) while replacing the implementation, governance, cryptography, and privacy layers. Six design decisions define the chain:

1. **Proof of authority.** Validators are known infrastructure operators and natural stakeholders.
2. **Privacy.** Confidential settlement via Orchard/Halo2 is a base workflow, not an add-on.
3. **Cobalt governance.** Validator trust evolution is handled through explicit Cobalt-derived governance mechanics with replayable certificates.
4. **Fixed supply.** 100 billion units at genesis, no native inflation.
5. **No native validator rewards.** Fees price resource use and are burned.
6. **Quantum-resistant authorization.** ML-DSA-style signatures on account and validator authorization paths.

### 2.2 Monetary invariant

The chain's monetary rule is:

$$
S_{t+1} = S_t - B_t, \quad I_t = 0, \quad R_t^{\text{native}} = 0, \quad S_0 = 10^{11}
$$

where $S_t$ is supply, $B_t \ge 0$ is fee burn, $I_t$ is native issuance, and $R_t^{\text{native}}$ is the protocol's native validator reward. This is structurally identical to XRP's monetary rule but rebuilt around modern cryptography.

### 2.3 Cobalt governance mechanics

Cobalt [MacBrough, 2018] models local trust through *essential subsets*. For an essential subset $S$ with cardinality $n_S$, quorum threshold $q_S$, and Byzantine tolerance $t_S$, Cobalt requires:

$$
0 \le t_S, q_S \le n_S
$$
$$
t_S < 2q_S - n_S
$$
$$
2t_S < q_S
$$

If $n_S > 3t_S + 1$ and $q_S = n_S - t_S$, these inequalities hold. The first nontrivial constraint enforces quorum intersection; the second supports forward progress.

The PostFiat L1 separates governance from fast transaction ordering:

- **Governance layer ($\mathcal{G}_{\text{Cobalt}}$):** validator registry transitions, amendments, emergency changes, trust-graph evolution.
- **Ordering layer ($\mathcal{O}_{\text{BFT}}$):** ordinary transaction batches via HotStuff-family certified BFT.

A governance transition is valid only if there exists a Cobalt certificate:

$$
\text{Cert}_G(\Delta G_t, h(G_t), h(G_{t+1}), \text{epoch}, \text{domain})
$$

where $G_t = (V_t, K_t, A_t, P_t, h_t)$ is the governance state at epoch $t$ — validator set $V_t$, validator key material $K_t$, active amendment set $A_t$, policy parameter vector $P_t$, and governance-state hash $h_t$.

### 2.4 Cobalt's overlap advantage

Where XRPL's worst-case safety requires pairwise UNL overlap $\alpha_{\text{XRPL}} \approx 0.90$ [Chase & MacBrough, 2018], Cobalt reduces the required overlap to $\alpha_{\text{Cobalt}} > 0.60$ [MacBrough, 2018]. The trust-list flexibility budget $\phi = 1 - \alpha$ is therefore $\phi_{\text{XRPL}} \approx 0.10$ vs $\phi_{\text{Cobalt}} < 0.40$. For a validator set of size $n=35$, the approximate number of validators by which two local views can safely differ rises from $D^{\text{XRPL}}_{\max} \approx 3.5$ to $D^{\text{Cobalt}}_{\max} \approx 14$.

### 2.5 Implementation state

The current PostFiat L1 controlled testnet has the following observed properties:

- Local 5-validator 20-round finality: `submit_to_finality` p50 1.563s, p95 1.709s, p99 1.753s.
- Local certified-round p50 0.921s, p95 1.043s.
- Local `tx` finality RPC p50 0.062s, p95 0.068s.
- Remote 5-validator peer-certified round p50 1.032s, p95 1.116s, p99 1.139s.

Cobalt mechanics implemented and gated:

- Non-identical trust views.
- Essential subsets with $t_S, q_S$ parameters.
- Linkedness checking before trust-graph activation.
- Fail-closed rejection of unsafe trust graphs.
- Non-uniform governance certificates.
- Reliable broadcast (RBC), binary agreement (ABBA), multi-valued agreement (MVBA), and democratic atomic broadcast (DABC) for amendments.
- Stale-replay rejection across activated trust graphs.
- Deterministic adversarial-packet test suite for collusion thresholds, captured groups, trust-graph poisoning, RBC/ABBA equivocation, MVBA/DABC invalid candidates, membership races, partitions, crash/restart, resource DoS, governance spam, amendment-replay bundles, parser/canonical-payload fuzzing, and long adversarial soak.

Authentication uses ML-DSA-style signatures. Per Open Quantum Safe parameter summaries, ML-DSA-65 has a 1,952-byte public key and a 3,309-byte signature versus 64 bytes for Ed25519, a 51.7× expansion in signature material. Certificate design therefore uses registry-root-bound compact evidence rather than per-vote public-key inclusion.

---

## 3. Naive Composition: What Goes Wrong

### 3.1 The obvious mapping

The simplest composition of the Dynamic UNL and the PostFiat Rust L1 is:

1. Dynamic UNL scoring pipeline runs as before.
2. The signed validator list it produces becomes the payload of a Cobalt amendment.
3. Cobalt validators vote on whether to ratify the amendment.
4. The amendment, if ratified, updates $V_t \to V_{t+1}$ in the chain's governance state $G_t$.

This is directionally correct but suffers four structural defects.

### 3.2 Defect 1: Authority remains foundation-centric

The Dynamic UNL pipeline as specified is foundation-operated. The Cobalt amendment is therefore a vote on *whether to accept the foundation's score map and selector output*, not on the governance decision itself. The Phase 1/2/3 phasing was designed to address this by introducing validator-side shadow scoring, but that machinery (sidecars, commit-reveal, convergence monitoring) is not currently live and is, in any case, an awkward retrofit on top of XRPL publisher infrastructure.

### 3.3 Defect 2: Hardware tier mismatch

If governance-tier validators are expected to *independently* compute the score map under the pinned bundle (Qwen3.6-27B-FP8 on H100, single GPU), the chain implicitly requires H100-class hardware for every governance participant. This replaces foundation centralization with hardware-tier centralization. The active scoring run consumes roughly 88 seconds and 12,428 total tokens per execution; doing this for every governance-tier validator on every round is operationally heavy.

### 3.4 Defect 3: Verification is not consensus

Cobalt ratifies what validators *vote on*. A validator votes "I accept this amendment" because it agrees with the proposed state transition. But in the model-assisted governance setting, the natural question is "is this score map the correct output of the bundle on the published inputs?" — a question about *verification*, not preference. A vote-based mechanism without an explicit verification step admits validators who simply rubber-stamp the foundation's submission.

### 3.5 Defect 4: Bundle as consensus-critical state is undeclared

The pinned model bundle (model weights + tokenizer + runtime envelope + hardware class + prompt + decoding parameters + container image hash + determinism flags) is, in the naive composition, an implementation detail of the foundation pipeline. But the bundle defines the governance function — different bundles produce different outputs on the same inputs. If validators are voting on outputs without explicitly ratifying the bundle, they are voting on the result of an unratified computation.

### 3.6 Summary

The naive composition treats LLM scoring as a side process whose results enter consensus only at the end. This is the wrong layering. A correct composition makes the bundle, the inputs, the computation, the output, and the verification all consensus objects.

---

## 4. Background III: Verifiable Inference Primitives

### 4.1 TensorCash: receipts as consensus objects

TensorCash is a proof-of-work monetary chain whose work function is canonical AI inference. The relevant innovation for our purposes is not its monetary model but its receipt structure.

A TensorCash work order at chain tip $b_{h-1}$, miner key $pk$, and nonce $n$ under epoch $e = (M_e, \mathcal{V}_e, \mathcal{R}_e, \mathcal{C}_e, \mathcal{Q}_e)$ is:

$$
x = \text{Prompt}\big(H(\text{TC/work/v1} \| b_{h-1} \| pk \| n \| e)\big)
$$

where $M_e$ is the admitted model artifact, $\mathcal{V}_e$ is the tokenizer/vocabulary, $\mathcal{R}_e$ is the runtime envelope, $\mathcal{C}_e$ is the set of admitted hardware classes, and $\mathcal{Q}_e$ is the canonical quantization rule. The model produces full next-token logits $z = M_e(x) \in \mathbb{R}^{|\mathcal{V}_e|}$. The commitment is over the full output distribution:

$$
p_j = \frac{\exp(z_j/\tau)}{\sum_k \exp(z_k/\tau)}, \quad \ell_j = \log p_j, \quad q_j = \mathcal{Q}_e(\ell_j)
$$

$$
L_j = H(\text{TC/logprob/v1} \| e \| c \| j \| q_j), \quad R_{\text{out}} = \text{MerkleRoot}(L_1, \ldots, L_{|\mathcal{V}_e|})
$$

with hardware class $c \in \mathcal{C}_e$. For Qwen-family v1 the vocabulary is 248,320 entries.

The receipt identifier is:

$$
\rho = H(\text{TC/receipt/v1} \| e \| c \| pk \| b_{h-1} \| H(x) \| R_{\text{out}} \| \text{ctu} \| m \| x_h)
$$

where $m$ is maturity height and $x_h$ is expiration height. Receipts are short-lived — they mature, expire, and can be consumed only once.

Two empirical observations from TensorCash are directly relevant:

1. **Greedy-token equality is insufficient.** A100 SXM4 and H100 PCIe runs emitted the same token text (`Here`) while producing different full-vector roots (`b8dca...` vs `871a9...`). Cross-hardware reproducibility requires class-bound roots, not output-level equality.

2. **Receipt validation can be replay-free.** The current `full-output-opening-validation` path validates a receipt by checking committed Merkle openings, prompt hash, descriptor hashes, inclusion proofs, and receipt-id recomputation — without re-executing the model. `receipt_replay_ms = 0; receipt_replay_performed = false`.

The takeaway: a receipt is a self-contained cryptographic object that proves a specific deterministic inference happened, and can be verified without re-executing the model. Full replay remains available as a dispute and audit path.

### 4.2 VeriLLM: 1% verification via structural asymmetry

VeriLLM [Wang et al., 2026] is a publicly verifiable decentralized inference protocol. Its central insight is that verifying an inference is structurally cheaper than producing it.

Autoregressive token generation requires $T_{\text{output}}$ sequential decode steps, each dependent on prior outputs. But once the output sequence is known, a verifier can concatenate prompt and output and run a single causal-masked forward pass — the *prefill* phase — which is fully parallelizable and saturates GPU tensor cores. Formally, if $\mathcal{F}_\theta$ is the model and $H = \mathcal{F}_\theta(X)$ is the hidden-state sequence:

$$
H' = \mathcal{F}_\theta(X_{\text{prompt}} \| X_{\text{output}})
$$

is sufficient to reconstruct all hidden states. The cost ratio in their reference implementation is:

$$
\frac{C_{\text{verify}}}{C_{\text{infer}}} = \frac{T_{\text{prompt}}}{T_{\text{prompt}} + T_{\text{output}}} \approx 0.01
$$

For workloads with short outputs relative to prompts, the ratio is even more favorable. VeriLLM's prototype measures 0.78% verification overhead on Qwen2.5-7B with 512-token prompt and 128-token output.

VeriLLM augments cheap verification with three protocol-level mechanisms:

**Commit-reveal with VRF sampling.** Each verifier $v_i$ commits to its hidden states by posting a Merkle root $r_i = \text{Merkle}(H_i)$. After all commitments are finalized, a smart contract derives challenge indices $\mathcal{S} = \{s_1, \ldots, s_k\}$ from a Verifiable Random Function. Verifiers open the corresponding leaves and submit inclusion proofs. Mismatches trigger slashing:

$$
\text{Slash}(v_i) = \mathbb{1}\big[\exists j : \delta(H_i[s_j], H^*[s_j]) > \varepsilon\big]
$$

where $\varepsilon$ is a calibrated floating-point tolerance threshold. This is *commit-then-sample*: the binding (Merkle commitment) precedes the unpredictable challenge (VRF), so a lazy verifier cannot adapt their response to the audit.

**Noise-tolerant comparison.** VeriLLM defines a bit-aware statistical test over hidden states, decomposing each floating-point element into sign, exponent, and mantissa. With tolerances $e_w, e_m$:

- $P_e$: proportion of exponent mismatches.
- $P_m$: fraction of large mantissa deviations among exponent mismatches.
- $P_w$: fraction of small mantissa deviations among exponent matches.
- $\bar{e} = \frac{1}{|\Delta|} \sum_{\delta \in \Delta} \delta$: mean discrepancy.

Empirically calibrated thresholds (off-chain: $P_e < 0.05$, $P_m > 0.75$, $P_w > 0.80$, $\bar{e} \in [-0.01, 0.01]$) achieve 99.85% attack detection with 1.9% false-positive rate. This is the mechanism that admits cross-hardware verification.

**Global honest majority through one-round re-verification.** VeriLLM's game-theoretic result: a single round of re-verification with expanded committee is sufficient under a global honest-majority assumption, even when local committees may be captured. The intuition is that an honest inferencer who is wrongly rejected has positive expected value from disputing, while a dishonest inferencer does not. Lemma E.1 of the VeriLLM paper establishes that for global honest fraction $r > \frac{1}{2(1-\mu)^2}$ (with $\mu$ a small noise parameter) and odd committee size $m$, honest inferences pass with probability $> 1/2$ and dishonest ones fail with probability $> 1/2$. Lemma E.2 establishes that for any failure bound $\xi$, the second-round committee size $m'$ satisfies:

$$
m' > \frac{\ln(1/\xi)}{2(r(1-\mu) - 1/2)^2}
$$

### 4.3 Composition properties

The TensorCash receipt and VeriLLM verification are structurally complementary:

- TensorCash defines *what* is committed (full distribution under hardware-class roots).
- VeriLLM defines *how to verify* a commitment cheaply (full-sequence prefill with commit-reveal-sample).

Neither alone is sufficient for governance: TensorCash gives a self-contained receipt but at PoW levels of overhead (full-vocabulary commitment, 248,320 Merkle leaves per token); VeriLLM gives a cheap verification protocol but without a portable receipt object that can be anchored on-chain independently of the scheduler. Together, they admit a primitive that is both portable and cheap to verify.

---

## 5. Proposed Synthesis: Receipt-Ratified Cobalt Governance

### 5.1 The core construction

Define a **governance bundle** as the tuple:

$$
\mathcal{B} = (M, \mathcal{V}, \mathcal{R}, \mathcal{C}, \mathcal{Q}, P, \Sigma, \Pi)
$$

where $M$ is model weights, $\mathcal{V}$ is the tokenizer, $\mathcal{R}$ is the runtime envelope, $\mathcal{C}$ is the admitted hardware class set, $\mathcal{Q}$ is the quantization rule, $P$ is the scoring prompt, $\Sigma$ is the output schema (structured grammar), and $\Pi$ is the policy parameter vector (selector parameters $\theta, K, \delta$ and evidence-source whitelist). The bundle hash is $h(\mathcal{B})$.

Define an **evidence snapshot** $S_t$ at governance round $t$ as the canonical JSON serialization of evidence collected from the bundle's evidence-source whitelist, normalized per the bundle's normalization rules. Its hash is $h(S_t)$.

Define a **governance receipt** $\rho_t$ at round $t$ as:

$$
\rho_t = H\big(\text{PF/gov/v1} \| h(\mathcal{B}) \| h(S_t) \| c \| pk_I \| R_{\text{out}} \| \tau_t\big)
$$

where $c$ is the inferencer's declared hardware class, $pk_I$ is the inferencer's validator key, $R_{\text{out}}$ is the Merkle root over the deterministic output (per the bundle's output schema), and $\tau_t$ is the round identifier. The inferencer signs $\rho_t$ under ML-DSA.

Define a **verification attestation** $\alpha_{v,t}$ produced by verifier $v$ at round $t$ as a Merkle root over the verifier's reconstructed hidden states under VeriLLM's full-sequence prefill, signed by $v$ under ML-DSA. The verifier additionally commits to a verdict $b_{v,t} \in \{0,1\}$ where $b_{v,t} = 1$ if its noise-tolerant comparison against the inferencer's openings passes calibrated thresholds.

Define a **ratified amendment** as a Cobalt amendment whose payload is:

$$
\text{Amend}_t = \big(\rho_t, \{\alpha_{v,t}\}_{v \in V_{\text{ver}}}, \{b_{v,t}\}_{v \in V_{\text{ver}}}, \pi_{\text{VRF}}\big)
$$

where $V_{\text{ver}}$ is the verifier committee and $\pi_{\text{VRF}}$ is the VRF proof for sample-index derivation. The amendment is valid only if:

1. The bundle hash $h(\mathcal{B})$ matches the currently active bundle in $G_t$.
2. The evidence snapshot $S_t$ is derivable from the active evidence-source whitelist (replayable from raw sources).
3. The inferencer signature on $\rho_t$ verifies against an admitted inferencer key in $G_t$.
4. The verifier committee was selected by VRF from the admitted verifier pool in $G_t$.
5. The verifier verdict majority satisfies $\sum_v b_{v,t} \ge q_S$ where $q_S$ is the Cobalt quorum for the relevant essential subset.
6. VRF-sampled Merkle openings from the inferencer's receipt and at least $q_S$ verifier attestations agree within calibrated tolerance.

If valid, the amendment transitions $G_t \to G_{t+1}$ where the validator-registry change encoded in $R_{\text{out}}$ (parsed under schema $\Sigma$) becomes authoritative.

### 5.2 Three-tier architecture

The construction partitions validators into three roles:

- **Inferencer tier $V_I$**: small set of validators with hardware in $\mathcal{C}$. Produces receipts.
- **Verifier tier $V_{\text{ver}}$**: larger set of validators with verification-grade hardware (which may be a subset of $\mathcal{C}$ or, under VeriLLM's noise-tolerant comparison, hardware that produces statistically equivalent output). Verifies receipts.
- **Cobalt ratifier tier $V_R$**: all governance-participating validators. Ratifies amendments under standard Cobalt mechanics.

These tiers are not disjoint — inferencers and verifiers may also be ratifiers — but the inferencer tier should be sized small enough to keep hardware costs bounded and large enough to admit a diverse, non-collusive pool.

### 5.3 Mathematical properties

**Cost asymmetry.** Let $C_I$ be the cost per round of producing a receipt, $C_V$ the cost of verifying it, and $|V_I|, |V_{\text{ver}}|$ the tier sizes. The total per-round governance cost is:

$$
C_{\text{round}} = |V_I| \cdot C_I + |V_{\text{ver}}| \cdot C_V
$$

Under VeriLLM's measured ratio $C_V / C_I \approx 0.01$, even with $|V_{\text{ver}}| = 100 |V_I|$, verifier-tier cost equals inferencer-tier cost. Equivalently, the design admits very wide verifier pools at the same total cost as a single full inference. For PostFiat's prompt shape (7,654 prompt tokens, 4,774 completion tokens), the verify/infer ratio is approximately:

$$
\frac{C_V}{C_I} \approx \frac{T_{\text{prompt}}}{T_{\text{prompt}} + T_{\text{output}}} = \frac{7654}{12428} \approx 0.616
$$

This is materially higher than VeriLLM's 0.01 number, which assumed short-prompt, long-output workloads. This is one of the open empirical questions identified in Section 6.

**Safety under bundle pinning.** If the bundle $\mathcal{B}$ is fixed and SGLang determinism holds, the receipt $\rho_t$ is a function only of $h(S_t)$. Two honest inferencers in the same hardware class produce identical receipts. Verifier attestations under VeriLLM noise tolerance pass for honest inferencers with probability $1 - \mu$ for small $\mu$ (bounded numeric nondeterminism). A dishonest inferencer must produce a receipt that passes VRF-sampled openings; under TensorCash's full-distribution commitment and VeriLLM's binding Merkle commitment, this requires either compromising the underlying cryptographic primitives or controlling more than the quorum threshold of verifiers.

**Bundle as governance state.** The bundle $\mathcal{B}$ is no longer an implementation detail. It is part of $G_t$. Bundle upgrades are Cobalt amendments. The amendment payload includes the new bundle hash, the rationale, and (per the recursive-trust caveat) a dual-signoff: both an explicit validator vote *and* a receipt produced under the current bundle scoring the proposed bundle against published upgrade criteria. The latter prevents the current model from blessing its own replacement; the former prevents a captured validator quorum from accepting an unsafe bundle.

**Emergency revert.** Any validator may broadcast an emergency revert proposal that, with elevated quorum $q'_S > q_S$, freezes the current bundle and rolls back to the prior known-good. This handles the "convergent wrong output" failure mode where the bundle produces deterministic but objectively unsafe governance decisions.

### 5.4 Scope constraints

The bundle's policy vector $P$ defines what the model is *allowed* to decide. At genesis, the recommended scope is narrow:

- Validator additions (subject to bounded-fraction cap, e.g., $\le 10\%$ of the set per round).
- Validator removals (subject to hard-failure conditions documented in $P$, e.g., extended downtime, dangerously outdated software).
- Score-based ranking adjustments with explicit churn caps via $\delta$.

Out of scope at genesis:

- Bundle upgrades (require explicit validator vote, not model proposal).
- Fee parameter changes.
- Amendment scope expansion.
- Emergency protocol freeze.

Scope expansion to new decision classes is itself a Cobalt amendment requiring explicit validator vote.

### 5.5 Genesis bundle and constitutional layer

The genesis bundle is the network's constitutional layer. It encodes:

- Initial validator set $V_0$.
- Initial inferencer pool $V_I^0$ with declared hardware classes.
- Initial verifier pool $V_{\text{ver}}^0$.
- Initial governance scope $P_0$.
- Output schema $\Sigma_0$ specifying typed amendment grammar.
- Evidence-source whitelist with attestation requirements.
- Slashing economics for inferencers, verifiers, and ratifiers.
- Emergency revert quorum thresholds.
- Bundle upgrade criteria.

The genesis bundle is signed by genesis validators and anchored at the chain's first block.

### 5.6 What the synthesis does not claim

This design does not claim:

- That the model's *policy outputs* are correct (only that they are deterministic under the bundle).
- That decentralization is achieved at genesis (the inferencer pool is a centralization surface that must be diversified over time).
- That the architecture has been empirically validated end-to-end (it has not — only its components have been validated in isolation).
- That LLM oracle governance is preferable to a deterministic rules engine on every decision class (this comparison is an explicit open question).
- That cross-hardware-class verification under VeriLLM's noise tolerance is operationally robust for this specific prompt and model (open empirical question).

---

## 6. Open Design Questions and Research Brief

This section is intended as an actionable brief for follow-on LLM research agents with full conversational context. Each subsection identifies a specific research question, the relevant constraints, the empirical evidence that exists, and the deliverable that would advance the design.

### 6.1 SGLang determinism beyond single-GPU

**Question.** The current Post Fiat determinism evidence is restricted to single-GPU H100 with TP=1. What is the current state of the art on:

(a) Multi-GPU deterministic inference under SGLang and vLLM? Specifically, are tensor-parallel reduction operations now batch-invariant under any production-grade serving stack?

(b) Cross-hardware-class deterministic inference? Specifically, what is known about reproducibility between H100 / H200 / A100 / Blackwell / consumer GPUs?

(c) Apple Silicon / MLX determinism? Reuters reported Alibaba's Qwen3 launch for MLX in 2025. Has anyone characterized determinism properties under MLX for Qwen-family models?

**Constraints.** Active scoring profile is Qwen3.6-27B-FP8, SGLang nightly `dev-cu13`, container image pinned to `lmsysorg/sglang:nightly-dev-cu13-20260430-e60c60ef@sha256:5d9ec71597...`, FlashInfer workspace 2 GiB.

**Reference.** Thinking Machines Lab's "Defeating Nondeterminism in LLM Inference" identified batch-size variance as a major source of nondeterministic inference and proposed batch-invariant kernels as a structural fix. SGLang implements this in production. vLLM has a parallel implementation under `features/batch_invariance`. Yuan et al. (arXiv:2506.09501) propose LayerCast for numerical reproducibility under limited precision.

**Deliverable.** A characterization of the current best practice for cross-hardware deterministic LLM inference, with specific recommendations for whether the PostFiat synthesis should pin single-GPU, admit multiple hardware classes with class-bound roots (TensorCash-style), or rely on VeriLLM noise tolerance for cross-class verification.

### 6.2 VeriLLM verification cost for PostFiat-shaped workloads

**Question.** VeriLLM measures 0.78% verification overhead for 512-token prompt, 128-token output workloads on Qwen2.5-7B-Instruct. The PostFiat `scoring_v2` workload is 7,654 prompt tokens and 4,774 completion tokens — a fundamentally different shape. What is the actual verification cost ratio?

**Constraints.** The verifier already possesses the claimed output. The structural asymmetry argument depends on the verifier being able to skip autoregressive decoding. For PostFiat's prompt:output ratio of approximately 1.6:1 (versus VeriLLM's 4:1), the cost ratio is much higher than 1%.

**Math.** Naively, $C_V/C_I \approx T_p / (T_p + T_o)$. For PostFiat: $7654/12428 \approx 0.616$. But this naive ratio ignores (a) decoder amortization of KV-cache for long outputs and (b) GPU utilization differences between memory-bound autoregressive decoding and compute-bound parallel prefill. The actual ratio may be lower than 0.616 in practice.

**Deliverable.** Empirical benchmarks of VeriLLM-style prefill verification on Qwen3.6-27B-FP8 under SGLang for the PostFiat `scoring_v2` prompt shape. Specifically: median and p95 verification time on H100 vs A100 vs H200; comparison of `prefill(prompt || output)` against full inference at the same hardware tier; characterization of how the cost ratio scales with completion length.

### 6.3 Receipt commitment scope

**Question.** TensorCash commits to the full output distribution (248,320 Merkle leaves per token, ~7.9 MB of Merkle structure for a 1-token receipt assuming 32-byte leaf hashes). VeriLLM commits to hidden states with sample-based verification. For PostFiat governance receipts, what is the right commitment scope?

**Options.**

(a) Full output distribution (TensorCash-style). Maximum security against forged-output attacks but expensive in storage and bandwidth.

(b) Hidden states with VRF-sampled openings (VeriLLM-style). Cheap in steady state but adds a sampling round.

(c) Output tokens only with token-level hash (TOPLOC/Ambient-style). Cheapest but vulnerable to greedy-token collision attacks across hardware classes.

(d) Top-k logits (TOPLOC original, Ambient Proof-of-Logits). Middle ground.

(e) KV-cache sampling (TensorBlock, Proof-of-Cache). Different commitment surface.

**Constraints.** PostFiat receipts are anchored on-chain via Cobalt amendments. ML-DSA signatures are already 3,309 bytes. The receipt payload should be small enough that anchoring it in a Cobalt amendment does not significantly inflate the certificate. Full-output-opening validation (TensorCash) does not require model replay for steady-state validation.

**Deliverable.** A comparison matrix of commitment schemes for PostFiat-shaped governance workloads, with specific recommendations for receipt structure. Should include cost analysis (bytes per receipt, verification time, on-chain anchoring cost under ML-DSA signature weight) and security analysis (which attacks each scheme prevents).

### 6.4 Cobalt amendment payload schema for receipt-ratified governance

**Question.** Cobalt amendments in the current PostFiat L1 carry typed governance objects. What is the canonical schema for a receipt-ratified amendment, and how does it interact with Cobalt's existing RBC/ABBA/MVBA/DABC machinery?

**Constraints.** The current PostFiat Cobalt implementation supports non-uniform governance certificates, stale-replay rejection across activated trust graphs, and deterministic adversarial-packet handling. Amendment-replay bundles are part of the readiness gate. The crates involved are `crates/consensus_cobalt` and `crates/types`.

**Deliverable.** A schema specification for governance-receipt amendments compatible with the existing Cobalt machinery, plus integration notes for the `consensus_cobalt` crate. Should specify: amendment payload type, field encoding, Merkle structure for receipts/attestations, replay-bundle integration, fail-closed behavior for malformed receipts, and adversarial-packet test extensions.

### 6.5 Game theory of receipt-ratified governance

**Question.** VeriLLM's incentive math (Lemmas E.1, E.2, and the 6/−2/−10 example) applies to inference verification where there is an objective ground truth (the bundle's deterministic output). PostFiat governance has the same property in steady state, but introduces several novel incentive surfaces:

(a) **Inferencer rotation.** Inferencers are scarce (hardware constraint). How should the inferencer pool rotate? Random selection? Performance-weighted? Model-proposed (recursive)?

(b) **Slashing economics.** What is the slash for an inferencer producing an invalid receipt vs a verifier producing an invalid attestation vs a ratifier voting against a valid receipt? Under PostFiat's no-native-reward economic model, slashes can only be against bonded stake or operational reputation.

(c) **Dispute economics.** VeriLLM's dispute path requires the disputer to put up a cost that is refunded on success. Under PostFiat's bond model, what is the right disputer-bond size?

(d) **Bundle-upgrade incentives.** Who can propose bundle upgrades? What deposit is required? How are accepted upgrades rewarded vs rejected ones penalized?

**Reference.** VeriLLM Appendix E provides a full game-theoretic proof under specific assumptions. The PostFiat technical paper (Section 3, Proposition 2) formalizes the validator incentive function $U_i(H) = x_i - c_i + r_i$ and the attack utility $U_i(A) = b_i - p_i - L_i$. The receipt-ratified design changes both: $c_i$ now varies by tier (inferencer vs verifier vs ratifier) and $L_i$ now scales with bundle-upgrade decisions as well as set-composition decisions.

**Deliverable.** A formal extension of VeriLLM's game-theoretic analysis to receipt-ratified governance, with explicit slashing constants and bond sizes for each role. Should include simulation results under realistic parameter choices and adversarial scenarios.

### 6.6 Evidence-source attestation and oracle attack surface

**Question.** The Dynamic UNL pipeline consumes VHS data, `/crawl` topology, ASN enrichment, DB-IP geolocation, and domain attestations. In a foundation-operated pipeline these are trusted inputs. In a receipt-ratified design, evidence sources become governance-critical: an attacker who can manipulate VHS data or DB-IP responses can produce a receipt that is *deterministically correct under the bundle* but factually wrong.

**Constraints.** Evidence sources have varying integrity properties: BGP-derived ASN data is publicly verifiable; VHS data is foundation-operated; DB-IP Lite is a third-party commercial provider; domain attestations are operator-controlled.

**Deliverable.** A taxonomy of evidence sources by trust class, with recommendations for attestation, reproducibility, and dispute mechanics. Should specifically address: how evidence sources are whitelisted in the bundle, how alternative attestations are admitted, what happens when sources disagree, and whether evidence collection should be inferencer-side, verifier-side, or oracle-attested.

### 6.7 Comparison literature: state of decentralized inference

**Question.** Several adjacent systems make overlapping claims. What is the current state of each, and what design lessons can be borrowed?

**Systems to survey:**

- **Atoma Network.** Combines TEE attestation with sampling-based consensus.
- **Bittensor.** Peer-ranked intelligence market with stake-weighted incentives.
- **Ambient.** Proof-of-Logits with Solana-style architecture.
- **Pearl.** Proof-of-useful-work via MatMul side-channeling.
- **Gradient (the VeriLLM authors).** What other primitives have they published?
- **Lagrange / deep-prove.** GPU-based ZKML.
- **EZKL / ZKonduit.** ONNX-to-circuit ZKML.
- **Mira.** Multi-model hallucination consensus.
- **Phala.** TEE-based confidential AI cloud.
- **nesa.ai.** Heterogeneous TEE orchestration.
- **TOPLOC.** Locality-sensitive hashing for trustless inference.
- **TensorBlock Proof-of-Cache.** KV-cache sampling.
- **VeriSplit.** Linear operator masking for private outsourcing.

**Deliverable.** A comparison matrix focused specifically on each system's applicability to PostFiat-style governance (not generic inference serving). For each, identify: what verification primitive it provides, what trust assumptions it makes, what governance integration would look like, and what (if anything) PostFiat should borrow.

### 6.8 Post-quantum signature implications for receipt aggregation

**Question.** ML-DSA-65 signatures are 3,309 bytes. A receipt-ratified amendment with $|V_{\text{ver}}| = q_S$ verifier attestations carries at least $|V_{\text{ver}}| \times 3309$ bytes of signature material. For $|V_{\text{ver}}| = 25$, this is 82,725 bytes per amendment.

What signature aggregation, batch verification, or commitment compression schemes are appropriate for ML-DSA in this setting?

**Constraints.** PostFiat amendment certificates are already designed with registry-root-bound compact evidence (per Section 9 of the formal technical paper). ML-DSA does not natively support aggregation in the way BLS does.

**Deliverable.** A specification for compact receipt-amendment certificates under ML-DSA, including: which signatures must be retained in full, which can be replaced with hash-based commitments to a registry root, and what batch-verification optimizations apply at the verifier-tier.

### 6.9 Bundle upgrade safety

**Question.** Bundle upgrades are the most dangerous governance class. A malicious or unsafe bundle could produce convergent wrong outputs across the entire verifier pool. What is the safe upgrade process?

**Components needed:**

(a) **Upgrade proposal format.** New bundle hash, rationale, compatibility statement, transition window, rollback criteria.

(b) **Dual-signoff mechanism.** Both an explicit validator vote *and* a receipt under the current bundle scoring the proposed bundle against published criteria. The latter prevents the new bundle from blessing itself; the former prevents a captured validator pool from accepting an unsafe bundle.

(c) **Transition window.** A period during which both old and new bundles are accepted, allowing inferencers and verifiers to migrate.

(d) **Rollback path.** If the new bundle produces anomalous output in the first $N$ rounds after activation, automatic rollback.

**Deliverable.** A bundle-upgrade specification with formal safety analysis. Should include adversarial scenarios (compromised bundle proposer, captured validator pool, model-blessed bundle that turns out unsafe) and mitigations for each.

### 6.10 Comparison against deterministic rules baseline

**Question.** The Post Fiat Dynamic UNL paper concedes that the model layer should be kept only if it outperforms a deterministic rules engine on the same evidence. This question must be answered concretely before any production transfer.

**Constraints.** A deterministic rules baseline can use any rule set expressible in code: agreement history, uptime, version freshness, identity continuity, ASN diversity, country diversity, concentration caps, hard-failure conditions.

**Deliverable.** A rigorous comparison of model-assisted scoring against deterministic rules on the same frozen snapshot data. Specifically: identical evidence-bundle inputs; both systems produce score maps; metrics include top-k overlap, cutoff stability, disagreement cases, and adversarial robustness. The deliverable should be honest about whether the model layer earns its place.

### 6.11 Apple Silicon and consumer-tier verifier participation

**Question.** Apple Silicon supports Qwen3 via MLX (per Reuters, June 2025). M-series chips have significant on-device inference capacity. Can a consumer-tier validator running an M-series MacBook participate as a verifier in this design?

**Constraints.** Verifier participation requires running VeriLLM-style prefill verification under either bit-exact reproducibility (same hardware class) or VeriLLM noise tolerance (different class). MLX's numerical behavior likely differs from CUDA SGLang.

**Deliverable.** A characterization of MLX inference for Qwen3.6-27B-FP8 (or equivalent), with specific recommendations on whether Apple Silicon can serve as a verifier tier under noise tolerance. Should include cost analysis (verifier-tier hardware accessibility broadens the validator base) and security analysis (does cross-architecture verification weaken the receipt model?).

### 6.12 Composability with privacy and PQ paths

**Question.** PostFiat L1 includes an Orchard/Halo2 privacy path and ML-DSA authentication. Can governance receipts coexist with privacy primitives? For instance, can a validator's identity attestation be a selective-disclosure packet rather than a public claim?

**Constraints.** Orchard/Halo2 currently supports shielded value, nullifiers, viewing keys, disclosure packets. PQ note encryption is roadmap. Governance receipts must be publicly verifiable; identity attestations need not be.

**Deliverable.** A design note on the interaction between receipt-ratified governance and privacy primitives. Should address: which fields in a governance receipt can be shielded, how disclosure packets feed into evidence snapshots, and what compliance considerations apply.

### 6.13 Formal safety proof

**Question.** Under what formal model is receipt-ratified Cobalt governance safe and live? VeriLLM's proof covers inference verification under global honest majority. Cobalt's safety covers governance amendments under essential-subset constraints. Their composition has not been proved.

**Required elements:**

(a) Threat model covering inferencer corruption, verifier collusion, ratifier capture, bundle compromise, and evidence-source manipulation.

(b) Safety property: no invalid amendment ratifies with non-negligible probability.

(c) Liveness property: valid amendments ratify in bounded time under honest-majority assumptions.

(d) Composition theorem: under what conditions do VeriLLM's verification guarantees and Cobalt's ratification guarantees compose?

**Deliverable.** A formal safety and liveness analysis of the composed system, ideally with machine-checked proofs of the key composition lemmas.

### 6.14 Critical math to verify or develop

Several mathematical claims in this synthesis require formal verification or extension:

(M1) **Verification cost bound for PostFiat workload shape.** Section 5.3 derives a naive ratio of 0.616. The actual measured ratio under realistic SGLang serving has not been characterized.

(M2) **Optimal verifier committee size.** Given target failure bound $\xi$ and assumed honest fraction $r$, VeriLLM's $m' > \ln(1/\xi)/(2(r(1-\mu) - 1/2)^2)$ gives the second-round committee. For first-round committee, what is the optimal size under PostFiat's specific bond economics and inferencer-pool size?

(M3) **VRF sampling rate.** VeriLLM gives $n \ge \alpha^{-1} \ln(1/\delta)$ for per-sample error rate $\alpha$ and target undetected-cheating bound $\delta$. For PostFiat's structured output schema (versus VeriLLM's free-form hidden states), the per-sample error rate may differ. What sampling rate gives the desired security margin?

(M4) **Bundle-upgrade convergence.** Under what conditions does a bundle upgrade preserve receipt-ratified consensus? Specifically, if some fraction of inferencers and verifiers have migrated to the new bundle and others have not, what fraction is required for the upgrade to ratify?

(M5) **Hardware-class admission criteria.** Under what statistical conditions should a new hardware class be admitted to $\mathcal{C}$? VeriLLM gives empirical thresholds ($P_e < 0.05$ etc.); these need to be calibrated specifically for Qwen3.6-27B-FP8 on the PostFiat prompt.

(M6) **Slashing equilibrium.** What slashing constants yield honest behavior as the unique Nash equilibrium across inferencer, verifier, and ratifier roles under PostFiat's no-native-reward economics?

---

## 7. Summary

This document proposes a synthesis: receipt-ratified deterministic LLM oracle governance for an XRPL-style settlement network. The synthesis composes (a) Post Fiat's deterministic LLM scoring under SGLang, (b) PostFiat L1's Cobalt-derived governance, (c) TensorCash's receipt-as-consensus-object structure, and (d) VeriLLM's 1%-cost prefill verification. The combination admits a primitive that, to the authors' knowledge, has not been built: a chain whose validator-registry and amendment state evolve through deterministic LLM oracle outputs, ratified by cheap commit-reveal-sample verification under Cobalt's existing amendment machinery.

The empirical evidence currently available — Post Fiat's 2,900-call zero-variance Qwen3.6/SGLang replay, the five-run zero-variance PFT Ledger replay, VeriLLM's prototype 1% verification overhead, and TensorCash's mined-and-spent block evidence under full-distribution receipts — supports each component in isolation. The composed system has not been built or measured.

The proposed next steps are not a whitepaper. They are: (a) a focused empirical campaign on the open questions in Section 6, especially M1 (verification cost for PostFiat workload shape), the rules-engine baseline, and bundle-upgrade safety; (b) a prototype implementation of receipt-ratified amendments in the existing `consensus_cobalt` crate; and (c) end-to-end testnet evidence that one inferencer produces a receipt, verifier-tier validators ratify it, and Cobalt machinery anchors the resulting governance transition. Only after these are demonstrated should the unified whitepaper claim the composed primitive as PostFiat's production governance mechanism.

---

## References

The reference list below is organized by topic, with full citation information where available. Where multiple papers are listed together, they should be read as a cluster.

**XRPL consensus and validator-list publication.**

- XRPL Documentation, *Unique Node List (UNL)*. https://xrpl.org/docs/concepts/consensus-protocol/unl
- XRPL Documentation, *Configure Validator List Threshold*. https://xrpl.org/docs/infrastructure/configuration/configure-validator-list-threshold
- XRPL Documentation, *Consensus Protections Against Attacks and Failure Modes*. https://xrpl.org/docs/concepts/consensus-protocol/consensus-protections
- XRPL Documentation, *xrp-ledger.toml File*. https://xrpl.org/docs/references/xrp-ledger-toml
- XRPL Blog, *Default UNL Migration* (2025). https://xrpl.org/blog/2025/default-unl-migration
- Chase, Brad, and Ethan MacBrough. *Analysis of the XRP Ledger Consensus Protocol*. arXiv:1802.07242, 2018.
- Amores-Sesar, Ignacio, Christian Cachin, and Jovana Mićić. *Security Analysis of Ripple Consensus*. OPODIS 2020 / LIPIcs 184, 2021.

**Cobalt governance.**

- MacBrough, Ethan. *Cobalt: BFT Governance in Open Networks*. arXiv:1802.07240, 2018.

**Deterministic LLM inference.**

- Thinking Machines Lab. *Defeating Nondeterminism in LLM Inference*. Connectionism, 2025.
- SGLang Documentation, *Deterministic Inference*. https://docs.sglang.io/docs/advanced_features/deterministic_inference
- vLLM Documentation, *Batch Invariance*. https://docs.vllm.ai/en/latest/features/batch_invariance/
- LMSYS / SGLang Team. *Towards Deterministic Inference in SGLang and Reproducible RL Training*. 2025. https://www.lmsys.org/blog/2025-09-22-sglang-deterministic/
- Yuan, Jiayi, et al. *Understanding and Mitigating Numerical Sources of Nondeterminism in LLM Inference*. arXiv:2506.09501, 2025.

**Verifiable decentralized inference.**

- Wang, Ke, Zishuo Zhao, Xinyuan Song, Zelin Li, Libin Xia, Chris Tong, Bill Shi, Wenjie Qu, Eric Yang, and Lynn Ai. *VeriLLM: A Lightweight Framework for Publicly Verifiable Decentralized Inference*. arXiv:2509.24257v4, 2026.
- Arun, Arasu, et al. *Verde: Verification via refereed delegation for machine learning programs*. 2025.
- Ong, Jack Min, et al. *TOPLOC: A locality sensitive hashing scheme for trustless verifiable inference*. 2025.
- TensorBlock. *Proof-of-Cache*. https://github.com/TensorBlock/Proof-of-Cache
- TensorCash. *Pure Proof-of-Work Money for the AI Compute Era*. 2026.
- Atoma Network. *atoma-infer*. https://github.com/atoma-network/atoma-infer
- Inference Labs. *sertn-avs*. https://github.com/inference-labs-inc/sertn-avs
- NESA. *nesa*. https://github.com/nesaorg/nesa
- Phala Network Documentation. https://docs.phala.com/
- Mira Network. https://mira.network/
- Ambient.ai. https://ambient.ai/

**Zero-knowledge ML.**

- Sun, Haochen, Jason Li, and Hongyang Zhang. *zkLLM: Zero knowledge proofs for large language models*. 2024.
- Qu, Wenjie, et al. *zkGPT: An Efficient Non-interactive Zero-knowledge Proof Framework for LLM Inference*. 2025.
- Liu, Tianyi, Xiang Xie, and Yupeng Zhang. *zkCNN: Zero knowledge proofs for convolutional neural network predictions and accuracy*. 2021.
- Lagrange Labs. *deep-prove*. https://github.com/Lagrange-Labs/deep-prove
- ZKonduit. *ezkl*. https://github.com/zkonduit/ezkl
- Chen, Yilun, et al. *zkML: Zero-Knowledge Machine Learning for Trustworthy Inference*. 2024.
- Roy, Bidhan, Peter Potash, and Marcos Villagra. *ZKLoRA: Efficient Zero-Knowledge Proofs for LoRA Verification*. 2025.

**Game theory and verifier's dilemma.**

- Zhao, Zishuo, Xi Chen, and Yuan Zhou. *It Takes Two: A Peer-Prediction Solution for Blockchain Verifier's Dilemma*. 2024.
- Luu, Loi, et al. *Demystifying incentives in the consensus computer*. 2015.

**Cryptographic primitives.**

- Merkle, Ralph C. *A certified digital signature*. 1989.
- Micali, Silvio, Michael Rabin, and Salil Vadhan. *Verifiable random functions*. 1999.
- NIST. *NIST Releases First 3 Finalized Post-Quantum Encryption Standards*. 2024.
- NIST FIPS 204, *Module-Lattice-Based Digital Signature Standard*. https://csrc.nist.gov/pubs/fips/204/final
- Open Quantum Safe, *ML-DSA parameter set summary*. https://openquantumsafe.org/liboqs/algorithms/sig/ml-dsa.html
- Ben-Sasson, Eli, et al. *Scalable, transparent, and post-quantum secure computational integrity*. IACR ePrint 2018/046.

**BFT consensus and Byzantine ordering.**

- Yin, Maofan, et al. *HotStuff: BFT Consensus in the Lens of Blockchain*. arXiv:1803.05069.
- Castro, Miguel, and Barbara Liskov. *Practical Byzantine Fault Tolerance*. 1999.
- Lewis-Pye, Andrew, and Tim Roughgarden. *Byzantine Generals in the Permissionless Setting*. arXiv:2101.07095, 2023 revision.

**Apple Silicon / MLX for LLM inference.**

- Apple Machine Learning Research, *Exploring LLMs with MLX and the Neural Accelerators in M5*. 2025.
- QwenLM, *Qwen3 repository documentation*. https://github.com/QwenLM/qwen3
- Reuters, *Alibaba launches new Qwen3 AI models for Apple's MLX architecture*. 2025.

**Trusted execution and hardware attestation.**

- Atoma Network. *atoma-infer*. https://github.com/atoma-network/atoma-infer
- Exo Explore. *evML*. https://github.com/exo-explore/evML
- Phala Network. https://docs.phala.com/

**Post Fiat and PostFiat L1 internal references.**

- Post Fiat. *Auditable, Model-Assisted Validator-List Publication for XRPL-Derived Networks*. May 2026 revision.
- PostFiat L1 Whitepaper, May 19, 2026 canonical unified version.
- *Post Fiat: A Formal Technical Paper*. May 14, 2026.
- *What XRP Was, Why PostFiat Exists, What Exists Now*. May 18, 2026.
- `dynamic-unl-scoring` Qwen3.6 deployment research, May 5, 2026.
- `postfiatorg.github.io` XRPL UNL deterministic replay artifacts, May 5, 2026.
- `dynamic-unl-scoring` Whitepaper Implementation Review, May 2026.

---

## Appendix A: Notation Reference

| Symbol | Meaning |
|---|---|
| $S_t$ | Native-token supply at epoch $t$ |
| $B_t$ | Fee burn at epoch $t$ |
| $V_t$ | Active validator set at epoch $t$ |
| $G_t$ | Governance state $(V_t, K_t, A_t, P_t, h_t)$ at epoch $t$ |
| $\mathcal{B}$ | Governance bundle $(M, \mathcal{V}, \mathcal{R}, \mathcal{C}, \mathcal{Q}, P, \Sigma, \Pi)$ |
| $\rho_t$ | Governance receipt at round $t$ |
| $\alpha_{v,t}$ | Verifier $v$'s attestation at round $t$ |
| $b_{v,t}$ | Verifier $v$'s binary verdict at round $t$ |
| $V_I$ | Inferencer tier |
| $V_{\text{ver}}$ | Verifier tier |
| $V_R$ | Cobalt ratifier tier (all governance validators) |
| $S_t$ (overloaded) | Evidence snapshot at round $t$ |
| $n_S, q_S, t_S$ | Cobalt essential-subset cardinality, quorum, Byzantine tolerance |
| $\theta, K, \delta$ | Dynamic UNL selector parameters (threshold, max size, churn margin) |
| $C_I, C_V$ | Per-round inferencer cost, per-round verifier cost |
| $\varepsilon$ | Calibrated floating-point tolerance for noise-tolerant comparison |
| $h(\cdot)$ | Generic hash function (typically SHA-256) |
| $H(\cdot)$ | Domain-separated hash with typed tag |
| $R_{\text{out}}$ | Merkle root over receipt output commitment |
| $\pi_{\text{VRF}}$ | VRF proof for sampling-index derivation |

---

## Appendix B: How to Use This Document with Research Agents

This document is intended as a copy-paste-ready input for LLM-driven research agents. When spinning up a research agent with full context, the recommended prompt structure is:

```
You are a research agent investigating receipt-ratified deterministic LLM
oracle governance for XRPL-style settlement networks. The attached document
provides full context on the synthesis, including (a) summaries of the
underlying papers (Post Fiat Dynamic UNL, PostFiat Rust L1, TensorCash,
VeriLLM), (b) the proposed synthesis architecture, and (c) a structured
research brief in Section 6 with specific open questions.

Your task is to research [specific Section 6 subsection number(s)] in
depth. For each question, you should:

1. Survey the current state of the art with proper citations to peer-reviewed
   work, technical reports, and production implementations from 2024-2026.
2. Identify any prior work that bears on the question but is not cited in
   Section 7 of the document.
3. Apply rigorous math where applicable. The notation reference in
   Appendix A is the canonical notation set; extend it only when necessary.
4. Produce concrete recommendations with explicit trade-offs.
5. Flag any claim in the document that you believe is overstated, understated,
   or incorrect.

Constraints on your work:
- The pinned execution profile is Qwen3.6-27B-FP8 on H100 with SGLang
  deterministic inference. Do not propose changes to this without
  explicit justification.
- The chain is PostFiat L1 (Rust, Cobalt governance, no native validator
  rewards, fixed 100B supply, ML-DSA authentication). Do not propose
  changes to these properties.
- Do not propose moving to a different model family or quantization
  without explicit justification.
- Frame your output as a research note, not a whitepaper. Include open
  questions, alternative paths, and adversarial scenarios.

When citing, prefer:
- Primary literature (arXiv, peer-reviewed venues).
- Production implementation documentation (SGLang, vLLM, MLX).
- Direct measurements over claims.

Avoid:
- Marketing materials.
- Unverifiable benchmark claims.
- Speculative trend extrapolations not grounded in published data.

Deliverable: a research note of 3,000-8,000 words addressing the assigned
sections, with full citations and explicit identification of open questions
that require further investigation.
```

Sections 6.1 through 6.14 can each be researched independently and in parallel. The most natural groupings are:

- **Determinism cluster:** 6.1, 6.11, M1, M5.
- **Verification cluster:** 6.2, 6.3, 6.8, M2, M3.
- **Governance cluster:** 6.4, 6.9, M4.
- **Security and game theory cluster:** 6.5, 6.6, 6.13, M6.
- **Comparison and integration cluster:** 6.7, 6.10, 6.12.

Each cluster can be assigned to a separate research agent. Cross-cluster integration of findings should be performed by a coordinating agent or human reviewer after individual cluster outputs are produced.

---

**End of document.**