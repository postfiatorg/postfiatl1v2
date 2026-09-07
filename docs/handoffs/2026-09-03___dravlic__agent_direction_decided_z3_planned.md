# Agent direction decided; Z3 planned

- **Operator:** Domagoj Ravlic (`dravlic`)
- **Date:** 2026-09-03 UTC

## BLUF

The operator asked this session to "figure out the direction of the agent."
The [direction decision](../governance/ai-governance-direction-20260903.md) is
written, harness-gated, and backed by the completed
[identity-packet replay](../governance/identity-packet-replay-results-20260903.md).
The next large task, Gate Zero Z3 NAVCoin round trips, is
[planned](../plans/active/z3-navcoin-roundtrip-plan.md) and awaits his go.

## Current state

- **Direction decision:** The
  [decision](../governance/ai-governance-direction-20260903.md)
  ([`22ee7db4`](https://github.com/postfiatorg/postfiatl1v2/commit/22ee7db45546da11151469e02c0d3d2e0ae04bad))
  passed the Text Improvement Harness at **88.80/100** (GPT 86.80, Fable
  89.60, GLM 90.00). The formula holds all binding authority. The model keeps
  one advisory job: identity/fabrication flags may trigger stricter
  deterministic entry checks, never punishment. Every change is governed, and
  all model-derived output stays `SHADOW_ONLY` until the gates pass. The
  [pending-decisions sheet](../governance/pending-operator-decisions.md)
  model-authority row now points here; one line from the operator adopts it.
- **Packet-input identity replay:** The experiment paused in the
  [2026-09-02 handoff](https://github.com/postfiatorg/postfiatl1v2/blob/integrate/arc-tier4-current-v2-20260901/docs/handoffs/2026-09-02___dravlic__identity_replay_paused_for_arc_grant.md)
  is complete. The package landed in
  [`e5d6bfaf`](https://github.com/postfiatorg/postfiatl1v2/commit/e5d6bfaf41e9276cac077e227a11ad29c5a0183d)
  and
  [`40bb8c73`](https://github.com/postfiatorg/postfiatl1v2/commit/40bb8c736d1a284843a35a3008f1469159a7f58b);
  [results](../governance/identity-packet-replay-results-20260903.md) landed in
  [`f0773c0a`](https://github.com/postfiatorg/postfiatl1v2/commit/f0773c0afeee9fcd657dd00827985ccbf6e6b75f).
  Determinism was **192/192 byte-identical** over four runs on two
  distinct-owner H200 hosts; total spend was **$4.83**; teardown receipts
  record every rental destroyed and absent. Under the frozen recognition
  prompt, only 22/55 validators scored non-zero and every PostFiat community
  validator scored zero. Obscure-but-real operators fail a "do you recognize
  this institution" test, which is why model output remains advisory.
- **Z3 planning:** The 228-line
  [NAVCoin round-trip plan](../plans/active/z3-navcoin-roundtrip-plan.md)
  ([`331ed515`](https://github.com/postfiatorg/postfiatl1v2/commit/331ed51597abc54d4e9d8929ff8225c78aa074e0))
  passed the harness at **90.00/100**. It reconciles the 2026-09-02 evidence:
  bridge deposit, mint, and redemption were each demonstrated once; swap and
  repeatability were not. Its gap gates begin with G1, where the operator names
  the SHADOW envelope: release lineage, Arc pair, A666 route, value cap, wallet
  control, and seven-day window. The plan authorizes no live action; NRRS
  remains deferred.
- **Infrastructure:** This server's `claude-plan` provider authentication is
  expired because its refresh token is missing. Re-login is required before a
  session uses the Fable default. This session finished on the `openai`
  provider. Corbanu is version 0.1.37.
- **Boundaries:** No Task Node action, devnet contact, live probe, or live value
  movement occurred. The Arc branch and frozen 2026-09-01 corpora were
  untouched. All Vast rentals for this work were destroyed and verified
  absent.

## Next decision or action

1. Adopt or amend the direction decision with one line in the
   [pending-decisions sheet](../governance/pending-operator-decisions.md); the
   rest of that sheet also still waits.
2. Give the go, or redirect, for the
   [Z3 plan](../plans/active/z3-navcoin-roundtrip-plan.md) at its first gate.
3. Re-login the `claude-plan` provider.

## References

- [AI governance direction decision](../governance/ai-governance-direction-20260903.md)
- [Identity-packet replay results](../governance/identity-packet-replay-results-20260903.md)
- [Z3 NAVCoin round-trip plan](../plans/active/z3-navcoin-roundtrip-plan.md)
- [Pending operator decisions](../governance/pending-operator-decisions.md)
- [2026-09-02 Dravlic handoff (Arc branch)](https://github.com/postfiatorg/postfiatl1v2/blob/integrate/arc-tier4-current-v2-20260901/docs/handoffs/2026-09-02___dravlic__identity_replay_paused_for_arc_grant.md)
- [Testnet path](../status/testnet-path.md)
