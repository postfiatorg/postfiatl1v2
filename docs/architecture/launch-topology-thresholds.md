# Launch Topology and Independence Thresholds

**Status:** Proposal — awaiting the operator's thresholds
**Date:** 2026-09-01
**Author:** Domagoj Ravlic (dravlic)
**Decision owner:** Post Fiat
**Purpose:** D3 of the [testnet-path milestone](../plans/active/l1v2-public-testnet-path-milestone.md) — concrete numeric topology and independence thresholds for a public-testnet launch, reusing the fork's strict-gate machinery.
**Related:** the locked [testnet genesis and launch specification](l1v2-testnet-genesis-and-launch-spec.md) (§3.3 trust-graph arithmetic, §5 launch gates, §5.2 independence calculation), the [genesis-registry proposal path](genesis-registry-proposal-path.md), the [release-gate inventory](../status/release-gate-inventory.md) row 20, and the [public launch boundary](../security/public-launch-boundary.md)

This document decides nothing. Every number below is a proposal awaiting the
operator's confirmation; the operator may adopt, tighten, or replace any of
them. Fork rules are cited from the read-only clones `dynamic-unl-scoring`
and `validator-scoring-sidecar`.

## 1. Shared arithmetic

All thresholds are stated at the genesis specification's minimum registry
size and scale with it (§3.3, §5.1–5.2 of the launch specification):

```text
n_S ≥ 12                                  registry size (launch minimum)
q_S = ceil(4 * n_S / 5) = 10 at n_S = 12  ratification quorum
n_S - q_S = 2                             unavailability margin
n_S - q_S + 1 = 3                         blocking threshold (any 3 seats can stall)
t_S = min(3, 4, 7) = 3 at n_S = 12        tolerated-fault bound
```

A correlated group that reaches `q_S` can ratify alone; a group that reaches
`n_S - q_S + 1` can stall registry evolution. The caps below are set against
those two numbers.

## 2. Dimensions

### 2.1 Hosting provider / ASN concentration

- **Fork rule today:** no hard cap. Provider-family and country counts are
  precomputed (`dynamic-unl-scoring/scoring_service/services/provider_families.py`,
  `compute_concentration`) and injected into the scoring prompt as the sole
  concentration evidence (`dynamic-unl-scoring/prompts/scoring_v10.txt`,
  diversity rules); concentration only lowers the diversity sub-score, which
  carries weight 10 of 100 in the deterministic final score
  (`dynamic-unl-scoring/docs/DeterministicFinalScore.md`). A dominant provider
  is scored down, never excluded.
- **Proposed l1v2 launch threshold:** at most **2 of 12** registry seats per
  hosting-provider family (general form: family weight ≤ `n_S - q_S`), and
  the same **2-seat** cap per individual ASN, using the fork's family
  normalization so corporate variants of one operator count together.
- **Rationale:** a single provider-wide outage must leave at least `q_S`
  validators standing, so no family may reach the blocking threshold of 3.
- **Checked by:** built — `python/postfiat_rpc/placement_preflight.py`
  (tests `python/tests/test_placement_preflight.py`) enforces the family and
  ASN caps over declared fields using the fork's family normalization ported
  from `provider_families.py`, and fails closed on unresolved endpoints.
  Feeding it fork-side ASN resolution
  (`dynamic-unl-scoring/scoring_service/clients/asn.py`, public pyasn data)
  for real operators is community-facing work under gate L3.

### 2.2 Geographic concentration

- **Fork rule today:** no hard cap. Country counts come from DB-IP Lite
  resolution (`dynamic-unl-scoring/scoring_service/clients/geolocation.py`)
  and enter only the same 10-of-100 diversity sub-score
  (`dynamic-unl-scoring/prompts/scoring_v10.txt`, country-concentration
  rules).
- **Proposed l1v2 launch threshold:** at most **4 of 12** seats per country
  (one-third), and at least **4 distinct countries** across the registry.
- **Rationale:** jurisdictional compulsion moves more slowly than a provider
  outage, so a one-third cap keeps every country well below quorum reach
  while staying attainable for a 12-seat testnet.
- **Honest caveat:** 4 seats exceeds the blocking threshold of 3, so a fully
  compelled country group could stall (never fork) registry evolution; if
  the operator wants geography held to the same bar as providers, the cap is
  **2 of 12** and at least **6 countries**. This is the one dimension where
  the proposal deliberately diverges from the group arithmetic.
- **Checked by:** built — the same placement preflight, country axis
  (`--strict` selects this section's 2-of-12 / 6-country variant). It reads
  operator-declared country fields from the genesis evidence record
  (`genesis-registry-proposal-path.md` §2.3), re-digests each record against
  the registry's committed evidence digest, and treats a missing or
  mismatched record as unresolved, failing closed. Cross-checking declared
  countries against the fork's DB-IP resolution for real operators is
  community-facing work under gate L3.

### 2.3 Operator independence

- **Fork rule today:** no correlation rule exists. Each validator is scored
  individually; the only identity control is the verified-domain check
  inside the identity sub-score, weight 10 of 100
  (`dynamic-unl-scoring/prompts/scoring_v10.txt`,
  `dynamic-unl-scoring/docs/DeterministicFinalScore.md`). Nothing stops one
  operator from running several scored validators.
- **Proposed l1v2 launch threshold:** adopt launch gate L3 exactly — every
  connected component of the versioned correlation graph (common beneficial
  ownership, controlling organization, key custody, hosting account) holds
  **fewer than 3 of 12** seats, i.e. at most **2**, and an unresolved
  correlation record fails the gate rather than counting as independent
  (launch specification §5.2).
- **Rationale:** no single controller may be able to stall registry
  evolution, and removing any one group must still leave `q_S = 10` seats.
- **Checked by:** `new:` L3 independence verifier — reproduces connected
  components and both inequalities from the published correlation dataset
  (launch specification §7 names this check; no tool exists).

### 2.4 Client / software diversity

- **Fork rule today:** single implementation (the postfiatd fork); version
  currency is scored through the software sub-score, weight 10 of 100
  (`dynamic-unl-scoring/prompts/scoring_v10.txt`,
  `dynamic-unl-scoring/docs/DeterministicFinalScore.md`), and the sidecar's
  participation preflight requires a full reproduction of the latest round
  before declaring an operator READY
  (`validator-scoring-sidecar/src/validator_scoring_sidecar/preflight.py`).
- **Proposed l1v2 launch threshold:** **12 of 12** seats on the exact pinned
  qualified release binary at launch (zero version skew, matched by binary
  hash), with the single-implementation monoculture recorded as an accepted
  launch risk because no second client exists to diversify toward.
- **Rationale:** with one implementation, binary exactness is the only
  enforceable diversity control, and the fleet-receipt machinery to verify
  it already exists.
- **Checked by:** existing — fleet receipts against the pinned lineage
  ([chain-state-current](../status/chain-state-current.md), the B3
  mechanism) plus release-gate rows 6–8; per-seat readiness comes from the
  C4 ratification client extending the sidecar preflight's READY verdict.

### 2.5 Minimum validator count and quorum safety margin

- **Fork rule today:** UNL size is hard-capped at 35 with a score cutoff of
  40 and a displacement gap of 3
  (`dynamic-unl-scoring/scoring_service/config.py`, `unl_max_size`,
  `unl_score_cutoff`, `unl_min_score_gap`; enforced in
  `dynamic-unl-scoring/scoring_service/services/unl_selector.py` and
  reproduced independently in
  `validator-scoring-sidecar/src/validator_scoring_sidecar/scoring/selector.py`);
  the fork's safety convention is ≥90% round-to-round UNL overlap against
  the 80% quorum derivation (`dynamic-unl-scoring/docs/CurrentRoadmap.md`,
  churn-control analysis).
- **Proposed l1v2 launch threshold:** **`n_S ≥ 12`** seats with
  **`q_S = ceil(4·n_S/5)` = 10**, an unavailability margin of **2** seats,
  and **`t_S = 3`** at the minimum size — the launch specification's
  reviewed profile adopted unchanged, with every seat preflight-READY at
  launch.
- **Rationale:** 12 seats is the smallest size where the 4/5 quorum, the
  two-seat caps above, and six independent groups coexist (launch
  specification §5.1); shrinking it makes two-seat groups disproportionately
  powerful.
- **Checked by:** existing — the genesis-registry reference verifier
  validates the template trust-graph arithmetic
  (`python/postfiat_rpc/genesis_registry.py`), and the pinned Cobalt checker
  enforces the inequalities at ratification (`crates/consensus_cobalt/`).

## 3. Summary table

| Dimension | Fork rule and value | Proposed launch threshold | Checked by |
| --- | --- | --- | --- |
| Provider / ASN concentration | No cap; scored only, diversity weight 10/100 (`provider_families.py`, `scoring_v10.txt`) | ≤ 2 of 12 per provider family and per ASN | Built: `python/postfiat_rpc/placement_preflight.py` |
| Geographic concentration | No cap; scored only (`geolocation.py`, `scoring_v10.txt`) | ≤ 4 of 12 per country; ≥ 4 countries (strict variant: ≤ 2 and ≥ 6) | Built: the same placement preflight, country axis (`--strict`) |
| Operator independence | No correlation rule; verified-domain identity check only (weight 10/100) | Every correlation group ≤ 2 of 12; unresolved fails | `new:` L3 independence verifier |
| Client / software diversity | Single implementation; software weight 10/100; sidecar READY preflight (`preflight.py`) | 12 of 12 on the exact pinned binary; monoculture recorded as accepted risk | Existing: fleet receipts, release-gate rows 6–8, C4 preflight |
| Minimum count and quorum margin | UNL cap 35, cutoff 40, gap 3 (`config.py`); ≥90% overlap convention | `n_S ≥ 12`, `q_S = 10`, margin 2, `t_S = 3` | Existing: `genesis_registry.py` verifier, pinned Cobalt checker |

## 4. What must be confirmed and what must be built

Awaiting the operator's confirmation — none of these values is adopted by
this document:

1. the 2-seat provider-family and ASN cap (§2.1);
2. the country cap choice — 4-of-12 as proposed, or the strict 2-of-12
   variant (§2.2), the one open judgment call;
3. the L3 group cap adopted unchanged (§2.3);
4. the zero-skew binary rule and the accepted monoculture risk (§2.4); and
5. the `n_S ≥ 12` / `q_S = 10` launch profile (§2.5).

To be built (`new:`): the L3 independence verifier covering §2.3. The
placement preflight covering §2.1–2.2 is built
(`python/postfiat_rpc/placement_preflight.py`); everything in §2.4–2.5 is
checkable with machinery that already exists in this repository.

## 5. How this gates launch

These thresholds operationalize launch gate L3 and feed release-gate
inventory row 20. Nothing here authorizes any deployment: producing the
correlation dataset and running the preflights against real community
operators is community-facing work, blocked by Gate Zero (Z1–Z3) like all of
Phase D. Once the operator confirms the numbers, the built placement
preflight passes over the published dataset, and the still-`new:` L3
independence verifier exists and passes too, row 20 and gate L3 can close;
D4 — the public-testnet launch decision itself — remains an explicit
operator decision outside this plan's authority, and no threshold in this
document can substitute for it.
