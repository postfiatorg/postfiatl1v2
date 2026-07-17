# PostFiat Urgent 72-Hour Plan

Status: active urgency plan
Date: 2026-05-28
Purpose: convert the current technical break from XRPL inheritance into a
fundraising-ready evidence package without diluting the core engineering path.

## Strategic Frame

PostFiat is not an XRPL fork. PostFiat is the clean authority-validator
settlement chain that XRP pointed toward but did not become.

The working thesis is:

1. Authority validation is a legitimate settlement model.
2. Native validator subsidies are unnecessary when validators are natural
   stakeholders.
3. XRPL's weak point is not authority validation; it is opaque UNL governance
   and inherited `rippled` state-machine complexity.
4. Cobalt-style trust evolution is the right governance answer, but XRPL
   appears to have left Cobalt in research rather than production.
5. PostFiat should use XRPL as evidence and prior art, not as inherited code.
6. The near-term job is to prove this with compact, inspectable artifacts.

## Non-Negotiables

- Do not restart a whip or cron injector unless explicitly requested.
- Do not touch the public XRPL audit article unless explicitly requested.
- Do not promote any XRPL finding without a local repro, proof log, hash,
  live-surface check, and remediation check.
- Do not let docs expand into another unreadable evidence dump.
- Do not claim Cobalt is deployed on XRPL. The claim is that XRPL researched it
  and PostFiat is implementing the governance slice that matters.
- Do not frame PostFiat as "XRPL but patched." Frame it as a clean successor.

## Priority Order

### 1. Finish The XRPL Inheritance-Risk Spine

Deadline: first 12 hours of the next work block.

Required outputs:

- Package the observed `1.5.0`
  `TRUSTLINE-POSITIVE-BALANCE-RESERVE-001` proof.
- Save patch, proof log, SHA-256, Docker/build details, tag commit, and tag
  date.
- Update packet triage and verifier only after the artifact is complete.
- Clean the temporary `rippled-1.5.0` worktree after artifacting.
- Continue the 8-hour hunt only for old-core, high-signal siblings:
  trustline/offer reserve accounting, owner-count drift, directory leaks,
  freeze/auth receive-path bypasses, and deterministic exceptions.

Success condition:

- At minimum, the `1.5.0` proof is committed as packet evidence.
- Best case, 1-5 additional old-core live/unfixed findings are packet-bound.

Stop condition:

- If the hunt devolves into policy-semantics clones or already-fixed
  next-release issues, stop and write negative inventory.

### 2. Write The Cobalt Autopsy

Deadline: 24 hours.

Required outputs:

- A short memo answering: what happened to Cobalt?
- Evidence from public `rippled`: no implementation traces, no public Cobalt
  branch, no Cobalt PR/issue history beyond unrelated Boost options.
- Evidence from public docs: Cobalt appears under consensus research while
  current XRPL docs still describe normal UNL consensus.
- Timeline: 2017 decentralization plan, February 2018 papers, subsequent
  non-deployment, current docs.
- Clear inference: Cobalt was published and promoted as research/future
  direction, but the public production chain stayed on existing consensus plus
  UNL/list evolution.
- PostFiat implication: use Cobalt-derived trust evolution for governance and
  registry transitions; do not use Cobalt as the high-throughput transaction
  ordering path.

Success condition:

- One readable memo with citations and local code-search evidence.

### 3. Make The Proof Surface Investor-Grade

Deadline: 48 hours.

Required outputs:

- One front-door status page with no more than eight links:
  whitepaper, architecture diagram, controlled testnet status, Cobalt witness,
  privacy demo, PQ authorization demo, XRPL inheritance-risk appendix, Qwen
  constitution demo if real.
- A short architecture diagram showing:
  users -> RPC -> transaction validation -> ordering -> state execution;
  Cobalt governs registry/amendments; privacy and PQ auth are baseline lanes.
- A one-page "why not fork XRPL" appendix:
  reserve/accounting proof, old-core risk, Cobalt non-deployment, governance
  opacity, and PostFiat clean-room design choices.

Success condition:

- A technical reviewer can understand the project in 10 minutes without reading
  100 docs pages.

### 4. Tighten The Whitepaper Around The Clean Successor Thesis

Deadline: 72 hours.

Required edits:

- Open with: authority validation is good; `rippled` inheritance is not.
- Treat XRPL as evidence that the category works, not as a codebase to inherit.
- Explain Cobalt as trust evolution for governance, not a claim that XRPL
  shipped Cobalt.
- Keep AI governance as accountability machinery: typed evidence, replay,
  challenge, and Cobalt gate. Do not present it as magic judgment.
- Move long implementation evidence into appendix links.
- Preserve crisp formulas only where they prove something or define a concrete
  admission/safety rule.

Success condition:

- Whitepaper reads like a thesis paper, not a burndown list.

## Fundraising Packet Shape

The packet should answer five questions:

1. Why authority validation?
2. Why not inherit XRPL/rippled?
3. What did PostFiat build that XRPL did not?
4. What evidence exists today?
5. What capital de-risks the next milestone?

Minimum evidence list:

- `TRUSTLINE-POSITIVE-BALANCE-RESERVE-001` current and old-tag repros.
- Cobalt safety witness demo and linkedness/checker output.
- Controlled testnet finality and state-root evidence.
- Orchard/Halo2 transaction demo.
- ML-DSA/PQ authorization demo.
- Qwen constitution/replay evidence only where the run is real and
  packet-bound.

## Operating Rule

Every task must either:

- strengthen the fundraising evidence spine;
- reduce inherited XRPL risk;
- make Cobalt/PostFiat governance more concrete;
- make the docs easier to understand; or
- harden a live demo.

Everything else waits.
