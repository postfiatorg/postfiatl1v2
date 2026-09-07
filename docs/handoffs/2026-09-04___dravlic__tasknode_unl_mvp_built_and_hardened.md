# Task Node UNL MVP built and hardened

- **Operator:** Domagoj Ravlić (`dravlic`)
- **Date:** 2026-09-04 UTC

## BLUF

The operator's Telegram pointed to the published proposal,
[Derive the UNL from Task Node identity and ratify it through Cobalt](https://postfiat.org/research/deterministic-unl-task-node-cobalt/),
with the note that an MVP was next. This session produced the
[execution plan](../plans/active/tasknode-unl-mvp-plan.md) (Text Improvement
Harness: **89.80/100**), implemented the deterministic Python pipeline, ran its
first fixture-driven end-to-end shadow round and first frozen public-ledger
shadow round, and fixed 14 fail-closed defects found by a fresh-context
adversarial review. Everything remains additive and `SHADOW_ONLY`; none of
these outputs has live authority.

## Current state

- **Plan:** The [MVP plan](../plans/active/tasknode-unl-mvp-plan.md)
  ([`5ec21e0f`](https://github.com/postfiatorg/postfiatl1v2/commit/5ec21e0f4448ee53008928f68b586f9eaefef874))
  has steps A–G checked off with code references. The published proposal
  remains authoritative for formulas, walk parameters, edge and binding rules,
  and churn rules.
- **Pipeline:** The implementation covers
  [schema, accountability, and exact-rational trust walk](https://github.com/postfiatorg/postfiatl1v2/tree/3d0e5c012950399cad0ab4af967cfc1291077e40/python/postfiat_rpc)
  ([`a782c23a`](https://github.com/postfiatorg/postfiatl1v2/commit/a782c23a2122265d4bc72612f4205afc7c67f45c));
  the offline-only binding CLI
  ([`70651f83`](https://github.com/postfiatorg/postfiatl1v2/commit/70651f832d34364320cbed17ce39914b69bf6200));
  signed work-digest verification
  ([`ffc01e7d`](https://github.com/postfiatorg/postfiatl1v2/commit/ffc01e7d816d16b5d6527af0664296ec3887e10e));
  vouch, co-work, and funding edge extraction
  ([`1b68a2d4`](https://github.com/postfiatorg/postfiatl1v2/commit/1b68a2d4fe53b4df33e6ca27a31baee233a456cb));
  the churn/overlap guard, including one change per round until 39 validators,
  ([`09838ea9`](https://github.com/postfiatorg/postfiatl1v2/commit/09838ea9cf253bcdbc7c3dbd389c47bc5a89b8b9));
  and the shadow-derive CLI plus
  [golden fixtures](https://github.com/postfiatorg/postfiatl1v2/tree/3d0e5c012950399cad0ab4af967cfc1291077e40/python/tests/fixtures/tasknode_unl)
  ([`deaaa5a2`](https://github.com/postfiatorg/postfiatl1v2/commit/deaaa5a280765869af0d5a472921710711b9a37f)).
  Binding commands are limited to prepare, finalize, verify, and replay; there
  is no submission command. Identical fixture inputs produce byte-identical
  outputs.
- **Frozen real-data shadow:** The
  [run note](../governance/tasknode-unl-shadow-run-20260904.md) and
  [evidence directory](https://github.com/postfiatorg/postfiatl1v2/tree/bfec6bd9ac70e005027675530b925cf8181d2466/docs/governance/tasknode-unl-shadow-run-20260904)
  ([`bfec6bd9`](https://github.com/postfiatorg/postfiatl1v2/commit/bfec6bd9ac70e005027675530b925cf8181d2466))
  retain a bounded read-only testnet-ledger view: 49 transactions, three
  wallets, and 48 `pf.ptr/v4` memos. Missing required evidence caused an empty
  candidate set, zero funding edges, and named holds for every observed
  account. This is historical shadow evidence, not a fresh validator probe or
  live authority. The note names the missing Phase 0 binding and digest
  emissions, Phase 2 vouch memos, published funding exclusion list, and
  remaining Admission Policy V1 evidence.
- **Adversarial hardening:** The fresh-context review
  ([`3d0e5c01`](https://github.com/postfiatorg/postfiatl1v2/commit/3d0e5c012950399cad0ab4af967cfc1291077e40))
  fixed 14 verification/hold-path defects with
  [regression tests](https://github.com/postfiatorg/postfiatl1v2/tree/3d0e5c012950399cad0ab4af967cfc1291077e40/python/tests).
  Examples include fabricated old rewards inflating tenure, wallet-side
  validator-key replacement without an L1 rotation record, identity-removal
  relabeling bypassing its hold window, vouch/co-work records not bound to their
  ledger sender, and pointers after the digest anchor entering the frozen view.
  It found no defects in published constants, exact-rational walk arithmetic,
  or specified boundary values. Golden outputs and the frozen real-data report
  stayed byte-identical. The focused selection passes **103 tests and 34
  subtests**; the strict documentation build passed throughout the lineage and
  passes on this handoff.
- **Authority boundary:** The MVP source is merged on `main`, but it is not
  deployment evidence. No devnet, on-chain write, transaction preparation or
  submission, live validator probe, or Task Node action occurred. The ledger
  collection used unauthenticated read-only queries.
  [Current State](../status/chain-state-current.md) remains the authority for
  the last observed fleet and deployed lineage.

## Next decision or action

1. Review the [MVP plan](../plans/active/tasknode-unl-mvp-plan.md) against the
   proposal's intent; use the
   [real-data note](../governance/tasknode-unl-shadow-run-20260904.md) to see
   how the fail-closed pipeline treated the frozen ledger inputs. Redirect now
   if needed; the work is additive.
2. If the direction holds, begin Phase 0 emission: publish the funding
   exclusion list and decide where binding memos and Task Node signed work
   digests are emitted. The digest requires a Task Node publishing key; the
   operator binding CLI is ready.
3. Adopt or amend the
   [AI-governance direction](../governance/ai-governance-direction-20260903.md)
   through the [pending-decisions sheet](../governance/pending-operator-decisions.md).
4. Give or redirect the go at Gate G1 of the
   [Z3 NAVCoin round-trip plan](../plans/active/z3-navcoin-roundtrip-plan.md).
5. Re-login the `claude-plan` provider.

## References

- [Published proposal](https://postfiat.org/research/deterministic-unl-task-node-cobalt/)
  and its verified source commits
  [`85ee464`](https://github.com/postfiatorg/postfiatorg.github.io/commit/85ee4642288f3731456e120bd8e313d67be8c247) and
  [`2e2f25e`](https://github.com/postfiatorg/postfiatorg.github.io/commit/2e2f25ea66eca20b7f0b637f90cbb5151487bbeb)
- [Task Node UNL MVP execution plan](../plans/active/tasknode-unl-mvp-plan.md)
- [Task Node UNL shadow-run note](../governance/tasknode-unl-shadow-run-20260904.md)
  and [frozen evidence](https://github.com/postfiatorg/postfiatl1v2/tree/bfec6bd9ac70e005027675530b925cf8181d2466/docs/governance/tasknode-unl-shadow-run-20260904)
- [2026-09-03 operator handoff](2026-09-03___dravlic__agent_direction_decided_z3_planned.md)
