# Whitepaper alignment audit specification

## Outcome and authority

Make the PostFiat protocol whitepaper a reliable entry point to the code that exists, the behavior tested, and the work still required. The documentation must let a reader answer three different questions: what the design argues, what this source implements, and what a dated deployment receipt actually observed.

Task Node task `task_e8a5201ae49adfd0e32b5496bb7f7188` governs this specification and its milestone. Accepted task `task_dae56e5e3650cf674495617e5547f747` governs the audit and documentation delivery. No protocol implementation or deployment is authorized by this specification.

The audit baseline is repository commit `d351353e57b295368450a57866ace17b5e1ce6ad`. Its authoritative protocol candidate is `docs/whitepaper.md`, Version 3, June 2026 with July implementation reconciliation, SHA-256 `83fa0951a27d8278b0fa6435d49931e406941a6860adf13f28d376fea600cc98`. `docs/business/whitepaper.md` explicitly declares itself non-normative. The downloadable `docs/assets/raw/whitepaper.md.txt` is a derivative to check for drift, not a second authority. Work occurs in an isolated branch; unrelated uncommitted changes in the main checkout are excluded. Record this baseline permanently even after correcting the paper.

## Questions and boundaries

Review the Abstract, §§1–12, every numbered subsection, Appendix A entries E1–E8, and the References. Inventory each material assertion about authorization, transition semantics, accounting, confidentiality, economic assumptions, limits, recovery, and measured behavior. Split a sentence when its clauses have different support. Cover the expanded ledger surface in §3: issued assets/NAV, bridge and proof profiles, account transactions, FastPay/FastSwap, and Asset-Orchard, without turning this into a new implementation campaign.

Use four evidence planes throughout:

- **Source:** behavior reachable in the pinned production path, including its activation, configuration, validation, execution, and persistence prerequisites.
- **Observed deployment:** a retained receipt tied to a chain, binary, source lineage, height and observation date. A committed receipt is historical evidence, not a fresh fleet probe or independent attestation by this audit.
- **Research target:** a proposed object, proof argument, fixture, or model without a demonstrated production consumer.
- **Unresolved:** absent evidence, conflicting documentation, or a claim requiring deeper verification than this audit establishes.

Classify each inventory row as implemented-and-tested, implemented-with-insufficient-verification, partial, planned, divergent, or unresolved. “Implemented-and-tested” means the named narrower behavior has relevant retained test coverage; separately record whether that test was run during this audit. It never means the entire protocol is proven. A theorem stays a conditional argument even when its checker passes tests. Pure economic premises and analogies receive explicit non-implementation treatment rather than artificial passing tests.

## Audit method

Read all whitepaper prose before editing. For each substantive implementation claim trace the owning type, validation, consuming node/execution path, persistence where relevant, and nearest meaningful test. Record repository-relative file names plus stable symbols or test names. Absence findings include search terms and directories checked; absence of a string alone cannot establish absence of equivalent behavior. Compare the nearest engineering page and historical evidence before proposing a correction.

The initial inspection identifies likely discrepancies to investigate, not conclusions to assume:

- The paper's July boundary calls the Cobalt pipeline research-only, while `crates/node/src/cobalt_handoff.rs` and `cobalt_authority_certificate.rs` contain a scoped authority path. Verify both scope exclusivity and the signed protocol certificate consumer; keep Consensus v2 as block finality.
- `README.md`, `STATUS.md`, and `docs/architecture/state-and-storage.md` retain undeployed-storage language, while `deployments/storage-lease-20260831/deploy-receipt.json` records height-930 activation and height-931 continuation. Reconcile timestamps and lineage without claiming current network health.
- §5.3 describes a full admission predicate. Compare `crates/consensus_cobalt/src/validator_admission_policy.rs` with the formula, input trust boundary, and newer Task Node proposal tooling. Evidence booleans are not independent observations.
- §§7.4 and 7.6 make strong containment and leakage statements. Compare action-specific ingress/egress disclosures, turnstile checks, and actual freeze/recovery consumers before claiming automatic containment or universal anonymity.
- Appendix A mixes numerical claims, hashes and archived report paths. Separate retained reproducible artifacts from missing original measurements; recompute arithmetic without presenting it as a new benchmark.

Core anchors include `crates/ordering_fast/src/consensus_v2.rs`, `crates/node/src/consensus_v2_store.rs`, `crates/types/src/market_nav_asset_types.rs`, `crates/storage/src/transactional.rs`, `crates/consensus_cobalt/src/trust_graph_governance.rs`, `crates/privacy_orchard/src/asset_orchard.rs`, and `crates/crypto_provider/src/lib.rs`. Verify the precise remaining anchors during the audit. Consult primary external standards or research only for their own claims; they cannot establish PostFiat implementation conformance.

## Deliverables and reader workflow

Publish a concise architecture and section-by-section overview, a claim inventory with evidence links, a prioritized gap backlog, and focused corrections in existing documentation. Preserve the protocol argument where possible and change only statements justified by the audit. Explain which source-backed discrepancies were corrected and which unresolved claims remain. Avoid replacing one broad future claim with an equally broad completion claim.

Maintain one structured claim inventory under the documentation tree. Provide a small Python CLI in `python/postfiat_rpc/whitepaper_audit.py` that reads this inventory, prints human-readable summaries and section filters, offers JSON output, and checks schema, baseline identity, section coverage, and local evidence references. It must work offline, make no signing or network calls, and fail with a clear nonzero exit on invalid inventory or missing anchors. Its check result establishes inventory integrity, not factual truth or protocol correctness. Use the same inventory to render the navigable MkDocs alignment table so the CLI and reader-facing view cannot silently disagree. The existing MkDocs interface satisfies the reader-facing requirement; a separate web application is unnecessary.

Document the CLI invocation and link the overview/matrix from the whitepaper, README, architecture overview and MkDocs navigation. Keep historical source pin and corrected paper identity distinct. Track work in a concise milestone under `docs/plans/active/`; retire that file to `docs/plans/completed/` only after the CLI, reader interface, and required validation work.

## Verification and completion

Run meaningful CLI tests for malformed schema, missing anchors, incomplete section coverage, and successful section-filter/JSON behavior. Run the inventory check, compare the rendered table with its source, verify copied whitepaper bytes, the existing whitepaper-boundary script, public-document link checks, redaction checks, and a strict MkDocs build. Review the rendered pages for navigation, readable tables, diagrams and links. Record pre-existing failures separately and fix touched-surface failures. Do not add protocol tests that merely mirror unchanged implementation.

Run a bounded set of existing tests that resolves the highest-impact audit uncertainty, especially Cobalt authority scope, consensus certificate rules and admission classification. Broad workspace or Orchard suites are unnecessary for this documentation/tooling patch. If build resources or dependencies prevent a selected test, record the command and failure and retain the affected row's verification limitation. Do not modify runtime semantics to make documentation true.

Completion requires coverage of every substantive section and material claim, explicit limitations for missing artifacts, a reviewable patch or commit, CLI and site validation, and a backlog with severity, evidence, concrete next action and closure criterion. Rank authorization/accounting/privacy misstatements first, operational-state drift second, evidence reproducibility and navigation next. For Task Node submit durable artifacts and exact validation outcomes, answer its verification request, and claim lifecycle completion only after its rewarded outcome. Retained evidence proves only the work it actually records.

## Specification lock

Run the text-improvement harness's full configured gate: five real scores each from `openai/gpt-5.6-sol-pro`, `anthropic/claude-fable-5`, and `z-ai/glm-5.3`, with the exact document hash recorded. Lock immediately on the first compliant average at or above 86/100 and preserve the scored bytes. Record scores and lock in a separate file. Only below 86 use the critiques for a direct OpenRouter `openai/gpt-5.6-sol-pro` rewrite and rescore. Missing or failed judge calls do not constitute a passing full gate. The text score assesses this specification, not protocol security.
