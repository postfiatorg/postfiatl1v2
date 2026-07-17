# Research Response Synthesis

Date: 2026-05-13

This document integrates archived research-agent responses into the current CTO
direction for PostFiat L1 v2. It is an engineering synthesis, not a verbatim
endorsement of every recommendation in those files.

The raw inputs were archived on 2026-05-13 under
`work_archive/2026-05-13-superseded-markdown/` so they are not confused with
the active burndown.

## Input Hygiene

Reviewed response files:

- `work_archive/2026-05-13-superseded-markdown/docs/research-requests/claude_governance.md`
- `work_archive/2026-05-13-superseded-markdown/docs/research-requests/claude_cobalt.md`
- `work_archive/2026-05-13-superseded-markdown/docs/research-requests/claude_privacy.md`
- `work_archive/2026-05-13-superseded-markdown/docs/research-requests/gemini_cobalt.md`
- `work_archive/2026-05-13-superseded-markdown/docs/research-requests/gemini_privacy.md`
- `work_archive/2026-05-13-superseded-markdown/docs/research-requests/gemini_governance.md`

Notes:

- `gemini_governance.md` is byte-identical to `gemini_privacy.md`; treat it as
  a duplicate privacy response, not an independent PQ-XRP governance response.
- `work_archive/2026-05-13-superseded-markdown/docs/research-requests/gemini`
  is empty.
- Several response files contain concrete code anchors, but line numbers are
  advisory because this repo is moving quickly.
- Some zkVM version claims in the responses were stale. A source check on
  2026-05-13 found `risc0-zkvm` stable crate documentation at 3.0.5, with a
  5.0.0 release candidate visible, and SP1 public release material at v6.x.
  Pinning must be done from current upstream release notes and dependency
  review, not from the agent prose.

## CTO Read

The research responses converge on the right product thesis:

> quantum-resistant, XRP-like institutional settlement, with Cobalt-derived
> validator governance and regulated confidential settlement.

They also converge on the current implementation state:

- The transparent PQ settlement path is the most mature part of the chain.
- The Cobalt implementation is a strong canonical-UNL governance substrate, not
  yet full per-node-trust-view Cobalt.
- Privacy has useful semantics and RPC plumbing but is not production until the
  debug proof backend is replaced and note encryption/disclosure is real.

The main correction is scope language. Privacy is not an optional R&D appendix.
It is a first-class product pillar. The correct sequencing is not "privacy
later"; it is:

1. Prove the network substrate with strict remote controlled-testnet evidence.
2. In parallel, replace the privacy backend behind the existing adapter with a
   real zkVM proof path and ML-KEM note envelopes.
3. Do not make production privacy claims until the real backend, disclosure
   policy, remote evidence, benchmarks, and audit posture exist.

## Accepted Direction

### Product

The target product is a quantum-resistant private XRP-like settlement chain
with Cobalt-derived governance. The day-one buyer is an institution that
believes quantum migration risk is material, accepts proof-of-authority /
federated-validator settlement as commercially validated by XRPL, and needs
confidential settlement with regulated disclosure.

The 100B fixed-supply thesis, burned fees, and no validator rewards for the
controlled-testnet phase are consistent with the XRPL comparison and should
remain explicit in public-facing docs.

### PQ XRP-Like Chain

"XRP-like" should mean operational surface:

- known validators,
- fast deterministic finality,
- low fees,
- burned anti-spam fees,
- account reserves,
- public RPC,
- wallet/sign/submit flows,
- validated receipts,
- history and explorer surfaces,
- governance-visible validator membership.

It should not mean mechanically cloning XRPL LCP. The codebase has already
invested in HotStuff-family ordering with Cobalt-derived governance. Pivoting
to an XRPL-style close/UNL consensus loop now would burn time and muddy the
existing evidence. Keep the current architecture and adopt the useful XRP-like
acceptance criteria.

Immediate PQ-XRP completion work:

- signed finality artifacts that public RPC can return and third parties can
  verify;
- canonical transaction signing bytes with network id, genesis binding,
  sequence, fee, expiry, and operation fields;
- public RPC method completeness for `server_info`, fee, ledger, account,
  submit, tx, validators, and manifests;
- reliable wallet submission with expiry polling and validated receipts;
- fee/reserve/burn invariants exposed in receipts and metrics;
- history/archive/checkpoint path;
- reproducible benchmarks for ML-DSA verify/sign, certificate bytes, finality
  latency, RPC rejection behavior, memory, disk, and bandwidth.

### Cobalt Governance

The current Cobalt subsystem should be described as "Cobalt-derived governance
in canonical-UNL mode." That is honest and defensible. It is not yet full
MacBrough Cobalt with per-node `UNL_i`, `ES_i`, local quorum thresholds, and
linked-node safety analysis.

The controlled-testnet target is:

- lifecycle-governed validator membership;
- amendment lifecycle;
- registry-root binding in block/certificate verification;
- stale signer and stale vote rejection after membership/key changes;
- signed operator manifests;
- replayable governance evidence packages;
- remote drills after every live membership mutation.

The path to full Cobalt is not a big-bang rewrite. First, make the canonical
mode complete and replayable. Then introduce a `TrustView` abstraction with
`TrustView::Canonical` as today's behavior and `TrustView::Cobalt` as the
future per-node-trust-view mode. Only after non-identical trust views pass
remote drills should external language graduate beyond canonical mode.

### Privacy

The privacy workstream should ship as "Confidential Settlement v1":

- one-input / two-output spend proof first;
- production proof backend behind the existing `ProofSystem` boundary;
- note commitments, nullifiers, Merkle anchors, and value balance in the guest;
- ML-KEM-768 KEM/DEM note envelopes;
- separate recipient, sender-outgoing, auditor, and recovery envelopes;
- public journal bound to chain id, genesis hash, image id, action kind,
  nullifiers, anchor, commitments, fee, policy id, disclosure hash, envelope
  hashes, and epoch;
- regulated pool policy with mandatory auditor/disclosure hashes where
  required;
- scan/disclose/unshield flow through wallet/RPC;
- proof/ciphertext byte pricing.

RISC Zero should be the first implementation target because it fits the
existing Rust adapter direction and has a mature ecosystem. SP1 should be
benchmarked against the same guest semantics, not used to delay the first
working backend. The exact versions and crate features must be pinned from
current upstream release notes at implementation time.

Important claim boundary: Groth16 or other elliptic-curve wrappers can be useful
for proof compression or EVM-style verification, but they are not a
post-quantum privacy claim. If the product claim is "quantum-resistant private
settlement," the verification path relied on by PostFiat validators must retain
a hash/STARK-style post-quantum security story or explicitly disclose the
classical wrapper caveat.

## Rejected Or Deferred Direction

- Do not rewrite the chain into XRPL LCP before controlled testnet. Keep
  HotStuff-family ordering plus Cobalt-derived governance.
- Do not claim pure Cobalt until per-node trust views and essential subsets are
  modeled and exercised.
- Do not introduce Negative-UNL-style quorum adjustment before validator health
  telemetry, finality artifacts, and partition drills are stable. Negative UNL
  is an outage-management feature, not a substitute for Cobalt trust evolution.
- Do not build ZK operator-manifest attestations for v1. Use signed,
  machine-readable, PII-minimized operator manifests first; reserve ZK
  concentration proofs for a later governance hardening pass.
- Do not remove the debug proof path in a way that breaks tests before the real
  backend exists. Move it behind test/debug features and make production
  profiles fail closed.
- Do not call the current shielded path "production privacy." The right public
  phrase before audit is "regulated confidential settlement pilot" or
  equivalent.

## Next Execution Queue

The highest-ROI order from here:

1. **Strict remote network evidence.** Normalize the five-machine inventory,
   prove SSH/deploy/ports, run strict 5-validator P0, and publish a report with
   wallet submit, public RPC, finality receipt, restart, catch-up, and
   post-change governance evidence.
2. **Finality and receipt artifact.** Make the public RPC return a compact,
   signed, replayable finality artifact for submitted transparent transfers.
   This is the bridge between "toy" and "externally verifiable chain."
3. **Governance replay package v0.** Package genesis, validator manifests,
   registry updates, governance certificates, block certificates, and a verifier
   command into an offline replay bundle.
4. **Privacy v1 scaffolding.** Add production `PublicJournal`, `PrivateWitness`,
   note/envelope types, and a zkVM proof backend skeleton while preserving the
   existing shielded RPC semantics.
5. **ML-KEM envelopes.** Implement KEM/DEM note encryption and scan/disclosure
   flows with KATs and domain separation.
6. **Spend proof.** Implement the 1-in/2-out guest, prove/verify locally, then
   wire node verification and fee pricing.
7. **Benchmark and audit package.** Publish proof sizes, proving time,
   verifier time, ciphertext overhead, RPC payload sizes, and exact build
   hashes before stronger privacy claims.

## What This Means For The Whitepaper

The whitepaper should make these points explicit:

- The first user is an institutional buy-side or settlement actor that wants a
  quantum-resistant federated settlement rail and views XRPL's validator model
  as commercially validated.
- Privacy is core to the thesis because regulated institutions need
  confidential settlement, selective disclosure, and long-term data
  confidentiality.
- Cobalt is currently implemented in canonical-UNL mode; full per-node
  trust-view Cobalt is the governance hardening target.
- The chain is not an XRPL clone. It is XRP-like in settlement posture and
  operator model, with different ordering and governance internals.
- Production privacy has an executable path through zkVM proofs and ML-KEM
  envelopes; it is not impossible, but it is not represented by the current
  debug adapter.
