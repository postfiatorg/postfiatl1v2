# Full Cobalt Burndown

Status: active implementation burndown  
Date: 2026-05-19
Current implementation reference: [cobalt-implementation.md](../governance/cobalt-implementation.md)
Reference: [cobalt-bft-governance-in-open-networks.md](../references/cobalt-bft-governance-in-open-networks.md)
Adversarial hardening: [cobalt-adversarial-burndown.md](cobalt-adversarial-burndown.md)

## Target

Ship full Cobalt governance for PostFiat.

Done means:

- validators can run non-identical trust views;
- trust views are committed as governance state;
- essential subsets carry `t_S` and `q_S`;
- unsafe trust graphs fail before activation;
- governance certificates verify against local trust views;
- Cobalt reliable broadcast, binary agreement, multi-valued agreement, and
  democratic atomic broadcast exist for amendments;
- validator registry and trust graph transitions are ratified through Cobalt;
- remote evidence proves convergence with non-identical trust views;
- release gates fail closed without current full-Cobalt evidence.

## Current State

PostFiat has full-Cobalt governance mechanics with colocated/reused-machine
remote evidence on a credential-aligned 7-validator plan. The remaining gap is
evidence quality for independent physical operator topology, not the
7-validator Cobalt protocol path. The stale deploy-plan / credential mismatch
is cleared: a redacted realignment dry-run proved the current credential
inventory could back seven logical validators, a fresh ignored deploy plan was
generated from that inventory, the plan was deployed through the remote SSH
bootstrap path, the mutating full-Cobalt remote drill passed, and release plus
offline replay gates are green. The latest refreshed drill also requires and
proves post-suspend active-validator outage tolerance: after Cobalt suspends one
validator, one remaining active validator can be stopped, the remaining online
active quorum still orders the next block, and restart/replay returns the
active set to convergence. A redacted topology-diversity gate now makes the
remaining topology caveat machine-checkable: the current evidence has seven
logical validators across three host fingerprints, so independent seven-host
topology fails closed, while reused-machine evidence can be accepted only when
that mode is explicitly enabled. The topology gate can now also require a
Cobalt placement manifest with redacted host/operator/legal-domain/funding
labels; the current controlled placement manifest is five-validator evidence,
not seven-validator Cobalt placement evidence. A 7-validator Cobalt placement
capacity profile also shows the current credential inventory has three complete
machine/operator-host groups while the no-single-group-can-block-quorum profile
needs at least four independent groups. `scripts/testnet-cobalt-placement-preflight`
now composes the manifest verifier, placement-capacity profile, and topology
diversity gate into one local fail-closed report before any future Cobalt
placement attempt mutates remote validators. The mutating full-Cobalt remote
drill can now require that preflight with
`REQUIRE_COBALT_PLACEMENT_PREFLIGHT=1`; release and replay gates can enforce
the same requirement. Failed placement preflights now include a redacted
remediation section that spells out the exact machine, manifest, and diversity
deltas needed for minimum no-blocking and strict independent-topology modes.
The placement manifest verifier now also writes failed reports, so Cobalt
preflight evidence records manifest verification failure directly instead of a
missing verifier artifact. A redacted placement-manifest draft tool now turns
the current credential inventory into either a safe seven-validator Cobalt
manifest draft or an exact blocker report; the current inventory still blocks
because it has three complete credential slots/groups, not seven slots and four
minimum no-blocking groups. The Cobalt placement preflight now runs that draft
tool as a subreport, so release/drill gates carry the exact draft-readiness
blocker instead of relying on a separate operator command. The manifest draft
path can now also consume a sanitized public-diversity overlay keyed by
`machine_index`, so cloud, region, jurisdiction, legal-domain, and
funding-source groups can be merged into the generated manifest without
hand-editing credential-derived host/operator labels. The same command can now
emit a redacted overlay template for the currently credential-backed machine
indexes, giving operators an exact fill-in shape for the remaining public
diversity labels. The placement preflight now auto-selects a generated redacted
draft manifest when no explicit manifest is supplied and the draft is available,
then runs manifest verification, placement capacity, and topology gates against
that effective manifest. Placement preflight now also requests and records the
operator-fillable public-diversity overlay template as part of the same
preflight packet. Release and replay gates now lift the placement preflight's
draft-manifest source, emitted diversity-template path, missing placement
deltas, required operator inputs, and rerun commands into the top-level
fail-closed reports, so the controlled-launch gate explains the exact remaining
Cobalt placement action without opening nested subreports. Those gates also
record whether the placement preflight is bound to the remote drill evidence;
when placement preflight is required, a standalone preflight report can inform
diagnostics but cannot satisfy the launch gate unless the remote drill actually
ran with the preflight requirement enabled. A strict read-only Cobalt controlled
launch gate now composes release and replay with topology diversity required,
placement preflight required, reused-machine topology disallowed, and
remote-drill-bound placement evidence required. It currently fails closed on
the remaining topology and placement blockers. Release, replay, and the strict
controlled-launch gate now also expose the explicit Cobalt trust-view launch
requirements: minimum trust-view count and non-identical G1 trust views. Release
and replay now also require topology-diversity evidence to be bound to the same
remote drill packet when topology diversity is required. The strict launch gate
now pins the selected remote drill, generates a fresh topology-diversity
subreport from that packet, and feeds the same topology report into release and
replay, so stale topology evidence is no longer a strict-launch blocker. The
current strict launch report fails only on the real independent-topology and
placement-preflight requirements. The topology-diversity gate now emits a
redacted remediation object with strict independent-topology deltas, and the
strict launch gate lifts those deltas to the top-level report: current evidence
has seven validators, three host/operator-host fingerprints, four missing
independent host/operator-host fingerprints, and four validator slots that must
move off reused hosts for strict independent Cobalt topology. The standard
local check path now runs self-tests for this topology remediation and
strict-launch summary path. A dedicated Cobalt adversarial burndown tracks the
bad-actor conditions that matter before controlled testnet: collusion,
bribery/capture, stale replay, trust graph poisoning, RBC/ABBA equivocation,
MVBA/DABC invalid candidates, membership transition races, partitions,
crash/restart, and resource DoS. The first deterministic adversarial harness
now passes locally for seven logical validators and eleven scripted scenarios,
and the strict controlled-launch gate now generates that harness packet and
requires release plus replay to accept it. A follow-on collusion threshold
matrix now enumerates all 128 captured-validator sets against the current
G1-style graph and proves that capture sets inside every essential subset's
configured `t_S` do not create unsafe linkage, captured strong support, or
liveness loss; it also records explicit over-bound examples for liveness loss,
linkage break, and captured strong support. A correlated capture model now
evaluates host/operator/funding/jurisdiction group capture plus injected
capture sets. It records the current reused-group risk, proves strict
one-validator-per-group placement is safe for single-group capture under this
graph, and shows the expected failure if all validators share one funding
source. A trust-graph poison packet now proves unsafe linkage, invalid subset
parameters, duplicate validator scope, missing validator references, stale view
versions, malformed trust-view signatures, and tampered lifecycle records all
fail closed before activation. A stale replay packet now proves old G0
certificates, proposals, linkage reports, registry roots, trust-view ids, and
DABC replay bundles are rejected after G1 activation. An RBC Byzantine packet
now proves double-propose/conflicting-accept detection, conflicting
echo/ready/accept rejection, triggerless ready/accept denial, duplicate
message dedupe, invalid signature rejection, and withheld-ready non-acceptance.
An ABBA Byzantine packet now proves equivocation detection across all ABBA
message kinds, withheld-support nontermination, invalid signature and bad-round
rejection, conflicting finish evidence, live-mode deterministic coin rejection,
and single-sender nontermination.
An MVBA/DABC invalid-candidate packet now proves invalid RBC accepts,
conflicting candidate ids, duplicate raw candidates, bad output candidate ids,
conflicting parent hashes, skipped amendment slots, zero activation heights,
tampered activation evidence heights, and stale propose-id/payload mismatches
fail closed.
A membership-race packet now proves old-set blocks after activation, new-set
blocks before activation, mixed old/new block-membership metadata, stale
transaction-network ids, wrong graph roots, non-advancing activation heights,
stale governance epochs, and stale DABC membership payloads fail closed.
A partition simulation packet now proves 3/4 and 2/2/3 partitions are safe but
not live before heal, single-validator isolation preserves six-validator
progress, delay/reorder/duplicate delivery is deterministic, and healed
conflicting replay produces RBC conflict evidence instead of silent divergence.
A deterministic crash/restart persistence packet now proves serialized RBC,
ABBA, MVBA/DABC, graph activation, validator suspension, rollback, and stale
replay state either revalidates after restart or fails closed.
A live process-kill packet now starts seven actual local validator child
processes for one Cobalt RBC plus ABBA plus MVBA/DABC request, kills the
delayed validator before waiting for the round, proves the remaining six child
processes still accept the RBC payload, finish the ABBA value, select the same
MVBA candidate, ratify the same DABC amendment, and verify the same DABC replay
bundle under non-identical trust views, then respawns the killed validator and
proves the same DABC-aware path after restart.
A resource/verification DoS packet now bounds Cobalt signature hex length and
proves oversized signatures, malformed payloads, DABC pending-pair floods,
DABC checkpoint floods, RBC duplicate floods, and ABBA duplicate equivocations
fail closed or dedupe deterministically.
A governance-spam packet now bounds MVBA valid-input candidate sets and proves
many under-bound amendments select deterministically while amendment floods,
raw replay floods, duplicate slots, future pending slots, and invalid parent
chains fail closed.
An amendment replay bundle packet now proves governance amendment replay is
ordered and verifiable before a full-Cobalt gate can pass: activation,
supersession, and rollback records are ordered; tampered bundles are rejected;
and the node replay verifier accepts only the expected five amendments, five
activation records, two supersession records, and one rollback record. The
packet generator removes generated lifecycle-smoke node key artifacts before
retaining evidence, and the latest retained evidence scan has no key-shaped
fields.
A parser/canonical-payload fuzz packet now round-trips RBC, ABBA, DABC, trust
graph, replay-bundle, and trust-graph-transition artifacts and proves malformed
JSON, type confusion, tampered ids, and activation-binding mutations fail
closed.
A long adversarial soak packet now proves 32 sequential DABC governance
ratifications under scheduled offline validators, delayed/reordered/duplicated
messages, stale replay attempts, below-threshold ABBA equivocation, and
restart/replay checkpoints.
Release, replay, and strict controlled-launch gates now prefer the newest
mechanics-passing full-Cobalt remote drill by default, so later placement
preflight failure packets do not masquerade as failed Cobalt mechanics evidence.
Explicit `FULL_COBALT_REMOTE_DRILL_REPORT` overrides still win.
That resolver behavior is now covered by release, replay, controlled-launch,
and JSON-recorded gate-selection self-tests in the standard check path.
The full standard check path is green after clippy-cleaning the Cobalt
adversarial examples and the small workspace issues that blocked
`scripts/check`.
The standard check path now also self-tests the live process-kill predicate so
missing concurrency, MVBA, DABC ratification, or restart replay evidence cannot
silently satisfy the DABC-aware live-kill gate.
The standard check path now runs the actual DABC-aware live process-kill drill
after the predicate self-test, so a green local check proves the seven child
validator processes survive kill/respawn at RBC/ABBA/MVBA/DABC/replay rather
than only proving the JSON gate shape.
A separate read-only controlled-testnet Cobalt readiness gate now composes the
same selected full-Cobalt remote drill, topology evidence, release gate, replay
verifier, adversarial harness, and full adversarial packet set while explicitly
allowing reused-machine topology for controlled pre-testnet mechanics. It
passes on the current seven-logical-validator evidence and records the strict
independent-topology gap as a caveat instead of a controlled-testnet blocker.
The standard check path now runs that actual controlled-readiness gate, so a
green local check proves the selected remote drill, controlled topology,
adversarial packet set, release gate, and replay verifier still compose into a
passing full-Cobalt controlled-testnet mechanics packet.
The standard check path now also runs a strict-launch expected-fail wrapper. It
executes the strict independent-topology launch gate, requires it to fail
nonzero, and proves the failure is limited to the known topology/placement
blockers while non-identical trust views, adversarial packets, release/replay
subreports, and remote-drill mechanics remain green.
That wrapper now has a predicate self-test in the standard check path, so
unexpected blockers, missing expected blockers, missing mechanics checks, or an
accidentally successful strict launch cannot satisfy the expected-fail wrapper.
Release and replay now also require named packet-specific checks for every
Cobalt adversarial packet type, so an adversarial packet cannot satisfy the
full packet set by presenting only schema/status/ok/validator-count shape.
The gate-selection self-test now imports release and replay and proves their
adversarial packet specs and required-check contracts stay identical, so future
release/replay drift fails the standard Cobalt self-test path.
It now also compares the controlled launch/readiness packet generator contract
against the release/replay verifier contract, so generated Cobalt adversarial
packet names, report filenames, and environment bindings cannot drift from the
packets release/replay require.
Controlled readiness and strict launch now explicitly check that the generated
adversarial harness and full packet set are bound into replay as well as
release, so replay cannot silently verify a different adversarial packet set
than the one generated by the gate.
Those controlled gates now also require the release and replay adversarial
packet maps to exactly match the generated packet names, so extra stale or
unexpected packet entries cannot hide inside otherwise passing evidence.
The exact-map predicate is now factored into shared gate helpers and self-tested
against matching, extra, missing, and wrong-path packet maps; the strict
expected-fail wrapper also rejects a synthetic stale packet-map mechanics case.
Controlled readiness and strict launch reports now expose a compact
`adversarial_packet_set_binding` object with expected packet names, release and
replay packet counts, missing/extra packet names, and exact/bound booleans.
The strict expected-fail wrapper now copies and validates that same binding
summary at top level, so the wrapper packet proves the packet-map contract
without requiring reviewers to inspect the nested strict-launch report.
The binding summary now also carries a canonical SHA-256 digest of the sorted
expected packet names and matching release/replay packet-name digests. The
strict expected-fail wrapper recomputes the digest and rejects malformed or
stale packet-name summaries.
Controlled readiness and strict launch now expose digest equality as named
mechanics checks, so packet-name digest drift appears in the gate blocker list
instead of only in the binding summary.
The strict controlled-launch gate remains unchanged and continues to fail
closed on independent topology and placement.
The strict controlled-launch gate now also proves that release and replay are
both bound to the same selected full-Cobalt remote drill packet. The strict
expected-fail wrapper treats that release/replay remote-drill binding as a
required Cobalt mechanics check, and its self-test rejects stale replay remote
binding.
The gate-selection self-test now also imports the strict expected-fail wrapper,
runs its predicate self-test, and records that release, replay, and topology
selected-remote binding checks are required strict-launch mechanics.
The strict expected-fail predicate self-test now rejects stale selected-remote
binding independently for release, replay, and topology evidence.
The gate-selection self-test now also records the strict expected-fail blocker
contract: the expected blockers and required-false checks must exactly match
the known topology/placement blocker set, and strict mechanics true-checks may
not overlap that blocker set.
The strict expected-fail predicate self-test now also rejects a contradictory
packet where an expected topology/placement blocker remains listed but its
required-false check flips true.
It now repeats that contradiction test for every strict topology/placement
required-false check, so no single blocker check can silently drift to true.
The strict expected-fail predicate self-test now also flips every required-true
mechanics check false one at a time, proving each required mechanics check is
independently enforced by the wrapper.
The gate-selection self-test now loads the strict wrapper's JSON self-test
artifact and verifies that its recorded case coverage exactly matches the
imported required-true and required-false strict check sets.
That gate-selection coverage contract now has its own negative self-tests: it
rejects missing required-true coverage, missing required-false coverage, stale
extra coverage, false exhaustive flags, and missing strict self-test reports.
The top-level controlled launch/readiness gates now record git branch,
revision, and dirty-state, and can fail closed with `REQUIRE_CLEAN_GIT=1` when
generating candidate evidence from a clean committed checkout.
The standalone full-Cobalt release and replay reports now record the same git
provenance and clean-worktree requirement, so subreports cannot lose the
candidate revision binding when reviewed outside the top-level gates.
The controlled launch/readiness gates now also require release and replay
subreports to match the parent gate's git revision, dirty-state, and clean-git
requirement, and the strict expected-fail wrapper treats those checks as
required Cobalt mechanics.
Those parent gates now also require release/replay branch provenance to match
and require the child reports' own git checks to pass, so a hand-edited child
report cannot satisfy the top-level Cobalt gate by carrying only copied git
metadata.
They now also require the topology, release, and replay subreports to carry the
expected Cobalt report schemas before the parent controlled gate can pass, and
the strict expected-fail wrapper treats those schema checks as required
mechanics.
Standalone release and replay now also expose named schema checks for the
subreports they directly consume: local harness, remote drill, credential
preflight, topology, placement preflight, and adversarial harness. A stale or
wrong subreport schema now appears as a specific release/replay blocker instead
of being hidden inside a broader requirement failure.
The controlled readiness and strict launch gates now require those standalone
release/replay subreport schema checks to pass. That makes stale nested Cobalt
subreports fail at the parent gate even if the child report itself is present,
redacted, and otherwise shaped like a release or replay report.
Those parent gates now also include a `subreport_schema_checks` summary with
the exact release/replay child check names, observed booleans, and
missing-or-false entries, so a failed parent packet explains the stale nested
schema without requiring reviewers to open child reports first.
The gate-selection self-test now records and validates that schema-check
contract too: strict expected-fail must require the parent schema-check
booleans, release/replay child schema check names must be nonempty and unique,
and controlled readiness must inherit the same release/replay schema-check
name sets as controlled launch.
At pushed revision `06283a9d6af62fa27ec45cfdc1b0d25c3d641b67`, a clean
controlled-readiness refresh passes with `REQUIRE_CLEAN_GIT=1`, no blockers,
and release/replay/adversarial evidence bound to the selected full-Cobalt
remote drill. The strict expected-fail wrapper also passes at the same clean
revision and proves the strict launch gate still fails only on the known
topology/placement checks.

Existing:

- Cobalt domain binding.
- Governance amendments.
- Validator registry lifecycle operations.
- Canonical quorum certificates.
- Registry root transitions.
- Governance replay bundles.
- Local and remote canonical registry evidence.
- First-class trust graph, trust view, essential subset, linkedness, and
  connectivity checker types in `postfiat-consensus-cobalt`.
- Operator manifests can carry signed local trust view and trust graph metadata.
- Non-uniform governance certificate type and verifier exist in
  `postfiat-consensus-cobalt`; local 7-validator / 3-trust-view smoke passes.
- Consensus API has an explicit canonical vs non-uniform governance verifier
  mode; non-uniform mode rejects stale canonical-quorum governance amendments.
- Cobalt RBC message types exist with deterministic ids, canonical signing
  payloads, and ML-DSA signing/verification tests.
- RBC echo and ready support can be evaluated against a local trust view using
  strong/weak support rules.
- RBC conflicting accept evidence detects linked validators that accept
  different payloads for the same proposer and amendment slot.
- RBC local non-identical-trust-view drill now proves one payload is accepted
  across seven validators and at least three distinct trust views.
- RBC loopback TCP drill now moves serialized RBC propose/echo/ready/accept
  messages through seven validator worker processes on one machine, with each
  worker evaluating its own non-identical trust view.
- ABBA message and round-state substrate exists with deterministic ids,
  canonical signing payloads, and ML-DSA signing/verification tests.
- ABBA aux/conf/finish support checks evaluate local trust views; linked
  conflicting finish values produce deterministic evidence.
- ABBA common-random-source abstraction has a deterministic simulation path
  that fails closed in live mode.
- ABBA same-sender equivocation evidence now detects validators that send both
  `true` and `false` in the same init/aux/conf/finish round; the regression
  test proves the equivocal sender is excluded from local support evaluation.
- MVBA valid-input candidate substrate derives candidates from RBC accepts and
  deterministically selects the same output across linked local views.
- DABC ratified amendment substrate turns MVBA output candidates into a
  deterministic parent-hash chain with explicit sequence, amendment slot,
  parent ratification id, output candidate id, and activation height.
- DABC full-knowledge checkpoint substrate gates activation on covered
  activation intervals, local-view strong support, and ratified pending slots.
- DABC replay bundle substrate verifies ratified amendment order, activation
  evidence, full-knowledge checkpoints, and activation heights offline.
- Validator registry updates can now bind trust graph transitions by carrying
  old/new registry roots, old/new trust graph roots, and the deterministic
  transition id.
- Trust view and essential subset lifecycle records build the next trust graph,
  recompute linkage, and reject unsafe graph updates before activation.
- Trust graph rollback records now restore the previous authority trust views
  when a failed graph has unsafe linkage evidence, and bind the rollback payload
  to DABC ratification.
- DABC validator lifecycle ratification binds a DABC amendment payload hash to
  the exact validator registry lifecycle record, activation height, and
  previous registry root.
- Transaction-network membership now binds registry root, trust graph root,
  governance epoch, validators, quorum, activation height, and block metadata.
- Transaction-network transition validation rejects old-set block bindings after
  the Cobalt-ratified activation height and accepts the new set.
- Transaction-network replacement now has DABC payload binding and a simulated
  ordering-failure drill where the old set is rejected and the replacement set
  resumes block membership validation at activation.
- A local full-Cobalt harness now reruns the Cobalt crate tests, node compile,
  consolidates latest local Cobalt evidence packets into one JSON report, and
  fails closed without passing RBC loopback TCP transport evidence.
- A remote full-Cobalt drill wrapper now refreshes local Cobalt evidence,
  derives the current G1 trust graph root, requires RBC loopback TCP transport
  evidence from the local harness, runs a redacted credential/deploy-plan
  preflight with an optional redacted realignment candidate, checks the selected
  remote deploy plan, and fails closed until a 7-validator remote plan and
  matching credentials exist.
- A full-Cobalt release gate now fails closed without non-uniform mode evidence,
  current trust graph root, local linkedness/DABC evidence, RBC loopback TCP
  transport evidence, a passing remote drill, and post-change finality.
- An offline full-Cobalt replay verifier now checks the evidence chain for
  graph history, DABC order, registry lifecycle binding, trust-graph
  transitions, transaction-network transitions, non-uniform certificates, and
  post-change remote finality. It also replays the local RBC loopback TCP
  transport evidence referenced by the release gate.
- Governance verification now has explicit cutover mode flags. Canonical mode is
  still the default; non-uniform mode requires a trust graph root and rejects
  canonical governance amendment evidence.
- Canonical validator sets can now be expressed as trust graph `G0`: one trust
  view per validator, each with the same all-validator essential subset.
- The first non-identical trust graph `G1` can now be DABC-ratified from `G0`
  by binding the trust-graph lifecycle record payload into the ratified
  amendment chain.
- A 7-logical-validator remote full-Cobalt drill now passes on a reused-machine
  topology generated from the current credential inventory. The run proves a
  mutating validator-registry suspension, active-set post-change finality, and
  post-suspend active-validator outage tolerance with release/replay gates
  against current G1 root
  `7440f743c8a20de85990b6ce26aa081d253a447218e02d269769ffd7c489702a4ebe1d2a82f4d1b8d47087b5f8f924fc`.
- A redacted topology-diversity gate reads the full-Cobalt remote evidence and
  credential preflight, records only host/user fingerprints, and fails closed
  when independent topology is required but validator slots reuse machines.
- The topology-diversity gate can require a placement manifest and public
  diversity labels, recording only hashed group labels. This prevents a future
  full-Cobalt topology claim from passing on host fingerprints alone while
  operator, jurisdiction, legal-domain, or funding-domain labels are missing.
- A deterministic local adversarial harness exercises seven logical validators
  with non-identical trust views under honest, withhold, colluding-withhold,
  duplicate/reorder, invalid-signature, malformed-payload, stale-root,
  RBC-conflict, ABBA-equivocation, and crash/restart scenarios.
- Release, replay, and strict controlled-launch gates can require the
  adversarial harness packet with `REQUIRE_COBALT_ADVERSARIAL_HARNESS=1`; the
  strict gate now generates the packet and binds the same report into release
  and replay.
- A collusion threshold report enumerates every captured-validator set for the
  current seven-validator graph and separates inside-`t_S` behavior from
  over-bound liveness, linkage, and captured-support failures.
- A correlated capture report evaluates capture by host, operator, funding,
  jurisdiction, and injected capture sets against the same local trust views.
- A trust-graph poison report exercises unsafe and malformed graph updates and
  proves they fail before activation.
- A stale replay report exercises old governance evidence after activation and
  proves the active G1 verifier rejects it.
- An RBC Byzantine report exercises proposer and voter faults and proves the
  RBC support gates fail closed.
- An ABBA Byzantine report exercises sender faults and proves ABBA evidence,
  support, and common-coin guardrails fail closed.
- An MVBA/DABC invalid-candidate report exercises malformed candidate and
  ratification paths and proves they fail closed.
- A membership-race report exercises old/new transaction-network transition
  boundaries and proves stale or mixed membership evidence fails closed.
- A partition simulation report exercises 3/4, 2/2/3, single-validator
  isolation, delay, reorder, duplicate, and healed replay behavior.
- A deterministic crash/restart persistence report exercises RBC, ABBA,
  MVBA/DABC, graph activation, validator suspension, rollback, and stale replay
  after reload.
- A resource/verification DoS report exercises signature length, malformed
  payload, DABC count bounds, duplicate RBC messages, and duplicate ABBA
  equivocation handling.
- A governance-spam report exercises many amendments, MVBA candidate floods,
  raw replay floods, duplicate amendment slots, future pending slots, and
  invalid parent chains.
- A parser/canonical-payload fuzz report exercises JSON decoding and canonical
  signing/id recomputation for RBC, ABBA, DABC, trust graphs, replay bundles,
  and trust graph transitions.
- A long adversarial soak report exercises 32 repeated governance rounds,
  scheduled offline validators, duplicate/reordered delivery, restart replay,
  stale replay rejection, below-threshold equivocation, and final DABC replay.

Missing:

- independent 7-validator physical/operator topology evidence. Current passing
  remote evidence uses seven logical validator slots across three host
  fingerprints, with up to three validators sharing one host fingerprint.
- a seven-validator Cobalt placement manifest with operator, host,
  operator-host, cloud-provider, region, jurisdiction, legal-domain, and
  funding-source groups. The current controlled placement manifest covers five
  targets and does not carry public-diversity labels.
- one more independent machine/operator-host group for minimum capture-threshold
  placement capacity, and four more independent groups for strict one-host-per
  Cobalt-validator evidence.
- a passing `scripts/testnet-cobalt-placement-preflight` report for the proposed
  seven-validator Cobalt placement before another remote mutation.
- independent-topology and placement-preflight satisfaction in the strict
  launch gate. Stale topology reports no longer satisfy release/replay, and the
  strict gate now generates the topology report from its pinned remote packet.
  Current strict topology remediation says four validator slots must move onto
  four additional independent host/operator-host fingerprints.

## Burndown

| ID | Priority | Status | Work | Exit Artifact |
| --- | --- | --- | --- | --- |
| COBALT-000 | P0 | Done | Put Cobalt paper PDF and Markdown extraction in repo. | `docs/references/cobalt-bft-governance-in-open-networks.md`, `docs/references/cobalt-bft-governance-in-open-networks.pdf`. |
| COBALT-001 | P0 | Done | Write full Cobalt shipping plan from the paper and current baseline. | `docs/governance/full-cobalt-shipping-plan.md`. |
| COBALT-010 | P0 | Done | Add `TrustViewId`, `EssentialSubsetId`, `TrustGraphRoot`, `EssentialSubset`, `TrustView`, `TrustGraph`, `TrustGraphTransition`, `LinkageReport`, and `ConnectivityReport`. | Implemented in `crates/consensus_cobalt/src/lib.rs`; evidence: `reports/testnet-cobalt-linkedness-checker/trust-graph-types-v0-20260518T155052Z/testnet-cobalt-linkedness-checker.json`. |
| COBALT-011 | P0 | Done | Define deterministic trust graph hashing and signing inputs. | Stable hash construction and mutation tests in `postfiat-consensus-cobalt`; evidence: `reports/testnet-cobalt-linkedness-checker/trust-graph-types-v0-20260518T155052Z/testnet-cobalt-linkedness-checker.json`. |
| COBALT-012 | P0 | Done | Derive UNL from essential subsets rather than treating UNL as the source of truth. | `derive_trust_view_unl` plus non-uniform view tests; evidence: `reports/testnet-cobalt-linkedness-checker/trust-graph-types-v0-20260518T155052Z/testnet-cobalt-linkedness-checker.json`. |
| COBALT-013 | P0 | Done | Add validator manifest fields for local trust view and trust graph version. | Signed operator manifest Cobalt metadata, manifest verify report binding, governance genesis bundle duplicate/stale view rejection; evidence: `reports/testnet-cobalt-trust-graph-smoke/operator-manifest-trust-view-v0-20260518T160012Z/testnet-cobalt-operator-manifest-trust-view.json`. |
| COBALT-020 | P0 | Done | Implement essential-subset parameter validation. | Rejects invalid `n_S`, `t_S`, `q_S`; accepts paper-valid configurations; evidence: `reports/testnet-cobalt-linkedness-checker/trust-graph-types-v0-20260518T155052Z/testnet-cobalt-linkedness-checker.json`. |
| COBALT-021 | P0 | Done | Implement strong support and weak support checks for a local trust view. | Unit tests cover `q_S` in every subset and `t_S + 1` in one subset; evidence: `reports/testnet-cobalt-linkedness-checker/trust-graph-types-v0-20260518T155052Z/testnet-cobalt-linkedness-checker.json`. |
| COBALT-022 | P0 | Done | Implement linked and fully linked pair checks. | Exact local checker returns linked / fully linked pair reports; evidence: `reports/testnet-cobalt-linkedness-checker/trust-graph-types-v0-20260518T155052Z/testnet-cobalt-linkedness-checker.json`. |
| COBALT-023 | P0 | Done | Implement weakly connected and strongly connected reports. | `LinkageReport` includes known-graph weak/strong connectivity fields; evidence: `reports/testnet-cobalt-linkedness-checker/trust-graph-types-v0-20260518T155052Z/testnet-cobalt-linkedness-checker.json`. |
| COBALT-024 | P0 | Done | Add exact small-graph counterexample generation. | Unsafe graph report includes conflicting pair reason; evidence: `reports/testnet-cobalt-linkedness-checker/trust-graph-types-v0-20260518T155052Z/testnet-cobalt-linkedness-checker.json`. |
| COBALT-025 | P0 | Done | Add property tests for linked contradictory-support rejection. | Exhaustive small-graph support enumeration proves linked strong supports share an honest validator under the configured fault bound; evidence: `reports/testnet-cobalt-linkedness-checker/trust-graph-types-v0-20260518T155052Z/testnet-cobalt-linkedness-checker.json`. |
| COBALT-030 | P0 | Done | Introduce non-uniform governance certificate shape. | `NonUniformGovernanceCertificate` binds registry root, trust graph root, trust view id, local validator, proposal, support, satisfied subsets, linkage report hash, and votes; evidence: `reports/testnet-cobalt-nonuniform-certificate/nonuniform-certificate-v0-20260518T160659Z/testnet-cobalt-nonuniform-certificate.json`. |
| COBALT-031 | P0 | Done | Implement trust-view certificate verifier. | Verifier checks active graph, exact linkage report, local trust view id, strong support under the local view, satisfied subsets, votes, and certificate id; support valid for one view but invalid for another fails locally; evidence: `reports/testnet-cobalt-nonuniform-certificate/nonuniform-certificate-v0-20260518T160659Z/testnet-cobalt-nonuniform-certificate.json`. |
| COBALT-032 | P0 | Done | Keep canonical certificate verifier only behind `cobalt_mode=canonical`. | `verify_governance_amendment_for_mode` accepts canonical amendments in canonical mode and rejects them in non-uniform mode; evidence: `reports/testnet-cobalt-nonuniform-certificate/nonuniform-mode-gate-v0-20260518T160934Z/testnet-cobalt-nonuniform-mode-gate.json`. |
| COBALT-033 | P0 | Done | Add local non-uniform governance certificate smoke. | Consensus test builds 7 validators, 3 non-identical trust views, and verifies one proposal under two local views while rejecting the third; evidence: `reports/testnet-cobalt-nonuniform-certificate/nonuniform-certificate-v0-20260518T160659Z/testnet-cobalt-nonuniform-certificate.json`. |
| COBALT-040 | P0 | Done | Implement Cobalt reliable broadcast message types. | `RbcPropose`, `RbcEcho`, `RbcReady`, `RbcAccept`, deterministic message ids, canonical signing payloads, ML-DSA signing/verification tests, and tampered binding rejection; evidence: `reports/testnet-cobalt-rbc-nonuniform/rbc-message-types-v0-20260518T161324Z/testnet-cobalt-rbc-message-types.json`. |
| COBALT-041 | P0 | Done | Implement RBC local support evaluation. | `evaluate_rbc_echo_support`, `evaluate_rbc_ready_support`, `rbc_ready_allowed_from_echo`, `rbc_ready_allowed_from_ready`, and `rbc_accept_allowed_from_ready`; local-view test covers echo strong support, ready weak support, ready strong support, and a different trust view rejecting the same weak sender set; evidence: `reports/testnet-cobalt-rbc-nonuniform/rbc-local-support-v0-20260518T161542Z/testnet-cobalt-rbc-local-support.json`. |
| COBALT-042 | P0 | Done | Implement RBC conflicting-payload evidence. | `RbcConflictingAcceptEvidence` and `detect_rbc_conflicting_accept`; linked validators accepting different payloads for the same proposer/slot produce deterministic evidence, while same-payload accepts do not; evidence: `reports/testnet-cobalt-rbc-nonuniform/rbc-conflict-evidence-v0-20260518T161814Z/testnet-cobalt-rbc-conflict-evidence.json`. |
| COBALT-043 | P1 | Done | Add remote RBC drill with non-identical trust views. | Local deterministic drill passes for seven validators and at least three distinct trust views: `reports/testnet-cobalt-rbc-nonuniform/rbc-nonidentical-local-drill-v0-20260518T205536Z/testnet-cobalt-rbc-nonidentical-local-drill.json`. Loopback TCP transport drill passes with seven validator workers and non-identical local trust views: `reports/testnet-cobalt-rbc-nonuniform/rbc-nonidentical-tcp-drill-v0-20260518T211459Z/testnet-cobalt-rbc-nonidentical-tcp-drill.json`. This proves serialized RBC transport on reused-machine loopback, not independent operator topology. |
| COBALT-050 | P0 | Done | Implement ABBA message types and round state. | `AbbaInit`, `AbbaAux`, `AbbaConf`, `AbbaFinish`, `AbbaRoundState`, deterministic ids, canonical signing payloads, ML-DSA signing/verification tests, bad-round rejection, and tampered-id rejection; evidence: `reports/testnet-cobalt-abba-nonuniform/abba-message-types-v0-20260518T162136Z/testnet-cobalt-abba-message-types.json`. |
| COBALT-051 | P0 | Done | Implement ABBA support checks and finish consistency. | `AbbaSupportEvaluation`, aux/conf/finish local support evaluators, weak/strong support helpers, `AbbaConflictingFinishEvidence`, and linked conflicting finish detection; evidence: `reports/testnet-cobalt-abba-nonuniform/abba-support-finish-v0-20260518T162413Z/testnet-cobalt-abba-support-finish.json`. |
| COBALT-052 | P0 | Done | Add deterministic test CRS and live CRS guardrail. | `AbbaCommonRandomSource`, `CobaltRuntimeMode`, and `abba_common_coin`; deterministic test CRS works in simulation and is rejected in live mode, signed beacon source remains live-allowed; evidence: `reports/testnet-cobalt-abba-nonuniform/abba-crs-guardrail-v0-20260518T162645Z/testnet-cobalt-abba-crs-guardrail.json`. |
| COBALT-053 | P1 | Done | Add Byzantine equivocation simulation for ABBA. | `AbbaEquivocationEvidence`, per-kind equivocation detectors, round-state equivocation scan, and regression test proving equivocal senders are excluded from local support; evidence: `reports/testnet-cobalt-abba-nonuniform/abba-equivocation-v0-20260518T205140Z/testnet-cobalt-abba-equivocation.json`. |
| COBALT-060 | P0 | Done | Implement MVBA valid-input set and output selection. | `MvbaCandidate`, `MvbaValidInputSet`, `mvba_candidate_from_rbc_accept`, deterministic candidate id, sorted/deduped valid input set, and deterministic output selection across linked local views; evidence: `reports/testnet-cobalt-dabc-nonuniform/mvba-valid-input-v0-20260518T162922Z/testnet-cobalt-mvba-valid-input.json`. |
| COBALT-061 | P0 | Done | Implement DABC amendment slots or parent-hash chain. | `DabcRatifiedAmendment`, `ratify_dabc_amendment`, `validate_dabc_ratified_amendment`, deterministic ratification ids, and a test proving multiple proposers produce one linear ratified amendment order; evidence: `reports/testnet-cobalt-dabc-nonuniform/dabc-ratified-chain-v0-20260518T163708Z/testnet-cobalt-dabc-ratified-chain.json`. |
| COBALT-062 | P0 | Done | Implement full-knowledge wait before activation. | `DabcFullKnowledgeCheck`, `DabcFullKnowledgeCheckpoint`, and `DabcActivationEvidence` gate activation on covered intervals, strong support, and ratified pending slots; evidence: `reports/testnet-cobalt-dabc-nonuniform/dabc-full-knowledge-v0-20260518T164429Z/testnet-cobalt-dabc-full-knowledge.json`. |
| COBALT-063 | P0 | Done | Add DABC replay bundle. | `DabcReplayBundle`, `DabcReplayReport`, and `verify_dabc_replay_bundle` verify amendment order, activation evidence, checkpoints, and activation heights offline; evidence: `reports/testnet-cobalt-dabc-nonuniform/dabc-replay-bundle-v0-20260518T164830Z/testnet-cobalt-dabc-replay-bundle.json`. |
| COBALT-070 | P0 | Done | Bind validator registry transitions to trust graph transitions. | `TrustGraphTransition`, `build_trust_graph_transition`, and `certify_validator_registry_update_with_trust_graph_transition` bind registry updates to old/new registry roots, old/new trust graph roots, and transition id; evidence: `reports/testnet-cobalt-trust-graph-transition/registry-trust-graph-binding-v0-20260518T165410Z/testnet-cobalt-registry-trust-graph-binding.json`. |
| COBALT-071 | P0 | Done | Add lifecycle operations for trust view update and essential subset update. | `TrustGraphLifecycleRecord`, `build_trust_view_update_transition`, and `build_essential_subset_update_transition` build safe graph updates and reject unsafe linkage before activation; evidence: `reports/testnet-cobalt-trust-graph-transition/trust-graph-lifecycle-v0-20260518T165806Z/testnet-cobalt-trust-graph-lifecycle.json`. |
| COBALT-072 | P0 | Done | Ratify admit/remove/suspend/reactivate/rotate through DABC. | `DabcValidatorLifecycleRatification`, `validator_registry_lifecycle_payload_hash`, and `bind_dabc_ratification_to_validator_registry_update` bind DABC payloads to validator lifecycle records; evidence: `reports/testnet-cobalt-trust-graph-transition/dabc-validator-lifecycle-v0-20260518T170135Z/testnet-cobalt-dabc-validator-lifecycle.json`. |
| COBALT-073 | P1 | Done | Add rollback path for bad trust graph activation. | `TrustGraphRollbackRecord`, `DabcTrustGraphRollbackRatification`, replay validation, and DABC payload binding; evidence: `reports/testnet-cobalt-trust-graph-transition/trust-graph-rollback-v0-20260518T210254Z/testnet-cobalt-trust-graph-rollback.json`. |
| COBALT-080 | P0 | Done | Bind transaction network membership to Cobalt amendments. | `TransactionNetworkMembership` and `CobaltBlockMembershipBinding` bind block metadata to active registry root, trust graph root, governance epoch, and transaction-network id; evidence: `reports/testnet-cobalt-trust-graph-transition/transaction-network-binding-v0-20260518T170552Z/testnet-cobalt-transaction-network-binding.json`. |
| COBALT-081 | P0 | Done | Reject blocks from inactive transaction validators after a Cobalt transition. | `validate_transaction_network_transition` and `validate_cobalt_block_against_transaction_network_transition` accept old-set blocks before activation, reject old-set blocks after activation, and accept new-set blocks; evidence: `reports/testnet-cobalt-trust-graph-transition/transaction-network-transition-v0-20260518T170758Z/testnet-cobalt-transaction-network-transition.json`. |
| COBALT-082 | P1 | Done | Add transaction-network replacement drill after simulated ordering failure. | `DabcTransactionNetworkRatification` binds replacement membership to DABC, old-set blocks fail after activation, and replacement-set blocks validate at activation; evidence: `reports/testnet-cobalt-trust-graph-transition/transaction-network-replacement-v0-20260518T210756Z/testnet-cobalt-transaction-network-replacement.json`. |
| COBALT-090 | P0 | Done | Add local full-Cobalt evidence harness. | `scripts/testnet-cobalt-full-local-harness` writes a consolidated local evidence report for linkedness, non-uniform certificates, RBC, ABBA, DABC, transition evidence, Cobalt tests, and node compile; it now requires passing RBC loopback TCP transport evidence. Latest evidence: `reports/testnet-cobalt-full-local-harness/full-cobalt-local-v0-20260518T211844Z/testnet-cobalt-full-local-harness.json`. |
| COBALT-091 | P0 | Done | Add remote full-Cobalt drill. | `scripts/testnet-cobalt-full-remote-drill` passed with seven logical validators on a reused-machine topology. Evidence: `reports/testnet-cobalt-full-nonuniform-remote-drill/full-cobalt-remote-v0-20260518T201528Z/testnet-cobalt-full-nonuniform-remote-drill.json`. |
| COBALT-092 | P0 | Done | Add full-Cobalt release gate. | `scripts/testnet-cobalt-full-release-gate` passes with `cobalt_mode=non_uniform`, current G1 trust graph root, local linkedness/DABC evidence, RBC loopback TCP transport evidence, passing remote drill, and post-change finality. Evidence: `reports/testnet-cobalt-full-release-gate/full-cobalt-release-gate-v0-20260518T211921Z/testnet-cobalt-full-release-gate.json`. |
| COBALT-093 | P0 | Done | Add full-Cobalt replay verifier command. | `scripts/testnet-cobalt-full-replay-verify` passes graph history, DABC order, registry transitions, transaction-network transitions, non-uniform certificates, remote drill pass, release gate pass, RBC TCP transport replay, and post-change finality. Evidence: `reports/testnet-cobalt-full-replay-verify/full-cobalt-replay-v0-20260518T212129Z/testnet-cobalt-full-replay-verify.json`. |
| COBALT-094 | P0 | Done | Make the remote full-Cobalt drill self-contained for current evidence. | The remote wrapper now derives current G1 root, requires RBC TCP-aware local harness evidence, and fails closed on credential/plan mismatch. Latest run blocked before remote mutation because credentials do not match the deploy-plan host fingerprint for `validator-0`: `reports/testnet-cobalt-full-nonuniform-remote-drill/full-cobalt-remote-v0-20260518T212613Z/testnet-cobalt-full-nonuniform-remote-drill.json`; fail-closed release/replay evidence: `reports/testnet-cobalt-full-release-gate/full-cobalt-release-gate-v0-20260518T212643Z/testnet-cobalt-full-release-gate.json`, `reports/testnet-cobalt-full-replay-verify/full-cobalt-replay-v0-20260518T212647Z/testnet-cobalt-full-replay-verify.json`. |
| COBALT-095 | P0 | Done | Add redacted credential/deploy-plan preflight to full-Cobalt remote evidence. | `scripts/testnet-cobalt-remote-credential-preflight` compares deploy-plan hosts with available credential hosts using only redacted fingerprints. The full remote drill now runs it before any mutation and the release/replay gates expose credential-preflight failure directly. Latest evidence: `reports/testnet-cobalt-remote-credential-preflight/preflight-v0-20260518T213347Z/testnet-cobalt-remote-credential-preflight.json`, `reports/testnet-cobalt-full-nonuniform-remote-drill/full-cobalt-remote-v0-20260518T213356Z/testnet-cobalt-full-nonuniform-remote-drill.json`, `reports/testnet-cobalt-full-release-gate/full-cobalt-release-gate-v0-20260518T213455Z/testnet-cobalt-full-release-gate.json`, `reports/testnet-cobalt-full-replay-verify/full-cobalt-replay-v0-20260518T213500Z/testnet-cobalt-full-replay-verify.json`. |
| COBALT-096 | P0 | Done | Add redacted credential-aligned realignment candidate. | The credential preflight now records a safe-to-commit 7-validator mapping over available credential host fingerprints when the current deploy plan is stale. Latest evidence proves a 7-validator reused-machine candidate exists across three credential hosts while the current plan remains unmatched: `reports/testnet-cobalt-remote-credential-preflight/preflight-v0-20260518T214027Z/testnet-cobalt-remote-credential-preflight.json`, `reports/testnet-cobalt-full-nonuniform-remote-drill/full-cobalt-remote-v0-20260518T214035Z/testnet-cobalt-full-nonuniform-remote-drill.json`, `reports/testnet-cobalt-full-release-gate/full-cobalt-release-gate-v0-20260518T214044Z/testnet-cobalt-full-release-gate.json`, `reports/testnet-cobalt-full-replay-verify/full-cobalt-replay-v0-20260518T214047Z/testnet-cobalt-full-replay-verify.json`. |
| COBALT-097 | P0 | Done | Add redacted remote plan realignment dry-run. | `scripts/testnet-cobalt-remote-plan-realignment-dry-run` proves the current credential inventory can back a 7-validator full-Cobalt deploy-plan shape without recording hosts, IPs, passwords, or keys. The full remote drill now attaches this report when credential preflight blocks on a stale plan. Evidence: `reports/testnet-cobalt-remote-plan-realignment/realignment-v0-20260518T214823Z/testnet-cobalt-remote-plan-realignment-dry-run.json`, `reports/testnet-cobalt-full-nonuniform-remote-drill/full-cobalt-remote-v0-20260518T214828Z/testnet-cobalt-full-nonuniform-remote-drill.json`. |
| COBALT-098 | P0 | Done | Deploy credential-aligned 7-validator plan and refresh full-Cobalt remote evidence. | A fresh ignored deploy plan was generated from the current credential inventory, deployed through the SSH bootstrap path, and exercised through the full-Cobalt mutating validator-registry drill. Remote drill, release gate, and replay are green. Evidence: `reports/testnet-cobalt-remote-bootstrap-smoke/bootstrap-cobalt-realigned-20260518T214913Z-pinned-creds.json`, `reports/testnet-cobalt-remote-bootstrap-smoke/bootstrap-cobalt-realigned-default-cred-preflight-20260518T2215.json`, `reports/testnet-cobalt-remote-credential-preflight/preflight-v0-20260518T214918Z/testnet-cobalt-remote-credential-preflight.json`, `reports/testnet-cobalt-full-nonuniform-remote-drill/full-cobalt-remote-v0-20260518T220506Z/testnet-cobalt-full-nonuniform-remote-drill.json`, `reports/testnet-cobalt-full-release-gate/full-cobalt-release-gate-v0-20260518T221428Z/testnet-cobalt-full-release-gate.json`, `reports/testnet-cobalt-full-replay-verify/full-cobalt-replay-v0-20260518T221428Z/testnet-cobalt-full-replay-verify.json`. |
| COBALT-099 | P0 | Done | Require and prove post-suspend active-validator outage tolerance. | The full-Cobalt remote drill now records whether post-suspend active-validator outage evidence is required and proven. Release and replay gates enforce the check when required. Latest evidence proves Cobalt suspension from 7 to 6 active validators, one additional active validator stopped, the remaining 5 online active validators ordering the next block, restart/replay, and final active-set convergence: `reports/testnet-cobalt-remote-bootstrap-smoke/bootstrap-cobalt-realigned-fault-prep-20260518T2220Z.json`, `reports/testnet-cobalt-full-nonuniform-remote-drill/full-cobalt-remote-v0-20260518T223730Z/testnet-cobalt-full-nonuniform-remote-drill.json`, `reports/testnet-cobalt-full-release-gate/full-cobalt-release-gate-v0-20260518T224824Z/testnet-cobalt-full-release-gate.json`, `reports/testnet-cobalt-full-replay-verify/full-cobalt-replay-v0-20260518T224824Z/testnet-cobalt-full-replay-verify.json`. |
| COBALT-100 | P0 | Done | Add cutover mode flags. | `postfiat-node verify-governance` now accepts `--cobalt-mode canonical|non-uniform` and `--trust-graph-root HASH`; non-uniform mode requires the trust graph root and rejects canonical governance evidence; evidence: `reports/testnet-cobalt-cutover-mode/cutover-mode-flags-v0-20260518T172756Z/testnet-cobalt-cutover-mode-flags.json`. |
| COBALT-101 | P0 | Done | Convert current canonical set into trust graph `G0`. | `build_canonical_unl_trust_graph` creates one trust view per canonical validator, each with the same all-validator essential subset; evidence: `reports/testnet-cobalt-g0/canonical-g0-v0-20260518T173052Z/testnet-cobalt-canonical-g0.json`. |
| COBALT-102 | P0 | Done | Ratify first non-identical trust graph `G1`. | `DabcTrustGraphLifecycleRatification`, `trust_graph_lifecycle_payload_hash`, and `bind_dabc_ratification_to_trust_graph_lifecycle_record` bind a non-identical G1 trust-view update to DABC ratification from G0; evidence: `reports/testnet-cobalt-g1/g1-dabc-ratification-v0-20260518T173435Z/testnet-cobalt-g1-dabc-ratification.json`. |
| COBALT-104 | P0 | Done | Add redacted topology-diversity gate for full-Cobalt remote evidence. | `scripts/testnet-cobalt-topology-diversity-gate` reports seven validators across three host fingerprints. Independent seven-host topology fails closed at `reports/testnet-cobalt-topology-diversity/topology-v0-20260518T225720Z/testnet-cobalt-topology-diversity-gate.json`; explicit reused-machine mode passes at `reports/testnet-cobalt-topology-diversity/topology-v0-20260518T225717Z/testnet-cobalt-topology-diversity-gate.json`. Release/replay can require this gate with `REQUIRE_COBALT_TOPOLOGY_DIVERSITY=1`; latest optional mechanics gate remains green at `reports/testnet-cobalt-full-release-gate/full-cobalt-release-gate-topology-optional-v0-20260518T2308Z/testnet-cobalt-full-release-gate.json` and `reports/testnet-cobalt-full-replay-verify/full-cobalt-replay-topology-optional-v0-20260518T2308Z/testnet-cobalt-full-replay-verify.json`. |
| COBALT-105 | P0 | Done | Add Cobalt placement-manifest diversity requirements to the topology gate. | `scripts/testnet-cobalt-topology-diversity-gate` can now require a placement manifest and public diversity labels without recording raw group labels. Reused-machine topology still passes only when explicitly allowed: `reports/testnet-cobalt-topology-diversity/topology-reused-v0-20260518T2307Z/testnet-cobalt-topology-diversity-gate.json`. Requiring the current controlled placement manifest fails closed because it covers five targets, not seven Cobalt validators: `reports/testnet-cobalt-topology-diversity/topology-reused-placement-required-fail-v0-20260518T2307Z/testnet-cobalt-topology-diversity-gate.json`. Release/replay expose the placement state: `reports/testnet-cobalt-full-release-gate/full-cobalt-release-gate-placement-required-fail-v0-20260518T2308Z/testnet-cobalt-full-release-gate.json`, `reports/testnet-cobalt-full-replay-verify/full-cobalt-replay-placement-required-fail-v0-20260518T2308Z/testnet-cobalt-full-replay-verify.json`. |
| COBALT-106 | P0 | Done | Record 7-validator Cobalt placement capacity profile from current credential inventory. | `reports/testnet-cobalt-placement-capacity/testnet-remote-placement-capacity-profile-20260518T231145Z.json` shows 7 validators need quorum 5, blocking threshold 3, and at least 4 independent machine/operator-host groups to avoid one group blocking quorum. Current credentials expose 3 complete groups, so minimum capture-threshold capacity is short by 1 group and strict independent-validator topology remains short by 4 groups. |
| COBALT-107 | P0 | Done | Add local Cobalt placement preflight for the next remote placement attempt. | `scripts/testnet-cobalt-placement-preflight` composes placement-manifest verification, credential/placement capacity, and topology-diversity requirements into one redacted local report before remote mutation. Current fail-closed evidence: `reports/testnet-cobalt-placement-preflight/preflight-v0-20260518T232134Z/testnet-cobalt-placement-preflight.json`; public-diversity-required variant: `reports/testnet-cobalt-placement-preflight/preflight-v0-20260518T232137Z/testnet-cobalt-placement-preflight.json`. |
| COBALT-108 | P0 | Done | Wire placement preflight into the mutating full-Cobalt remote drill. | `scripts/testnet-cobalt-full-remote-drill` now honors `REQUIRE_COBALT_PLACEMENT_PREFLIGHT=1`; when enabled, the remote mutation path requires a passing `scripts/testnet-cobalt-placement-preflight` report first. Non-mutating fail-closed evidence proves the current incomplete placement blocks before remote mutation: `reports/testnet-cobalt-full-nonuniform-remote-drill/full-cobalt-placement-preflight-required-fail-v0-20260518T2324Z/testnet-cobalt-full-nonuniform-remote-drill.json`. |
| COBALT-109 | P0 | Done | Enforce Cobalt placement preflight in release and replay gates. | `scripts/testnet-cobalt-full-release-gate` and `scripts/testnet-cobalt-full-replay-verify` now expose `placement_preflight` and enforce `REQUIRE_COBALT_PLACEMENT_PREFLIGHT=1`. Current fail-closed evidence: `reports/testnet-cobalt-full-release-gate/full-cobalt-placement-preflight-required-fail-v0-20260518T2330Z/testnet-cobalt-full-release-gate.json`, `reports/testnet-cobalt-full-replay-verify/full-cobalt-placement-preflight-required-fail-v0-20260518T2330Z/testnet-cobalt-full-replay-verify.json`. |
| COBALT-110 | P0 | Done | Add redacted remediation deltas to failed Cobalt placement preflight reports. | `scripts/testnet-cobalt-placement-preflight` now emits a `remediation` object with minimum no-blocking and strict independent-topology deltas, manifest target/bindable-target shortfalls, controlled/public diversity field shortfalls, required operator inputs, and rerun commands. Current evidence: `reports/testnet-cobalt-placement-preflight/preflight-v0-20260518T233348Z/testnet-cobalt-placement-preflight.json`; public-diversity evidence: `reports/testnet-cobalt-placement-preflight/preflight-v0-20260518T233351Z/testnet-cobalt-placement-preflight.json`; strict independent evidence: `reports/testnet-cobalt-placement-preflight/preflight-v0-20260518T233339Z/testnet-cobalt-placement-preflight.json`. |
| COBALT-111 | P0 | Done | Make placement manifest verification fail with evidence instead of a missing report. | `scripts/testnet-placement-manifest-verify` now writes a failed JSON report for ordinary manifest/diversity/source-evidence failures. The Cobalt preflight now records `placement_manifest_verify_report_not_ok` with the verifier report present and redacted, instead of `placement_manifest_verify_report_missing`. Evidence: `reports/testnet-cobalt-placement-preflight/preflight-v0-20260518T233742Z/testnet-cobalt-placement-preflight.json`; public-diversity evidence: `reports/testnet-cobalt-placement-preflight/preflight-v0-20260518T233747Z/testnet-cobalt-placement-preflight.json`; strict independent evidence: `reports/testnet-cobalt-placement-preflight/preflight-v0-20260518T233801Z/testnet-cobalt-placement-preflight.json`. |
| COBALT-112 | P0 | Done | Add redacted seven-validator Cobalt placement manifest draft tool. | `scripts/testnet-cobalt-placement-manifest-draft` reads the current credential inventory through the remote SSH parser, emits a redacted manifest draft when seven unique credential targets exist, and otherwise fails closed with exact slot/group deltas. Self-test covers the emitted-manifest and blocked paths. Current evidence blocks as expected with three complete slots/groups: `reports/testnet-cobalt-placement-manifest-draft/draft-v0-20260518T234940Z/testnet-cobalt-placement-manifest-draft.json`; public-diversity variant: `reports/testnet-cobalt-placement-manifest-draft/draft-public-v0-20260518T234940Z/testnet-cobalt-placement-manifest-draft.json`. |
| COBALT-113 | P0 | Done | Attach placement-manifest draft evidence to Cobalt placement preflight. | `scripts/testnet-cobalt-placement-preflight` now runs `scripts/testnet-cobalt-placement-manifest-draft` as a redacted subreport and exposes `placement_manifest_draft` in the top-level preflight JSON. Current evidence blocks as expected with `placement_manifest_draft_not_ready`: `reports/testnet-cobalt-placement-preflight/preflight-draft-integrated-v0-20260518T2350Z/testnet-cobalt-placement-preflight.json`; public-diversity variant: `reports/testnet-cobalt-placement-preflight/preflight-draft-integrated-public-v0-20260518T2350Z/testnet-cobalt-placement-preflight.json`. |
| COBALT-114 | P0 | Done | Add sanitized public-diversity overlay support to Cobalt placement manifests. | `scripts/testnet-cobalt-placement-manifest-draft` now accepts `COBALT_DIVERSITY_OVERLAY` with schema `postfiat-testnet-cobalt-placement-diversity-overlay-v1`, keyed by `machine_index`, and merges only public-diversity fields into the generated manifest. The overlay path rejects sensitive-shaped values and duplicate machine indexes; self-test covers a seven-machine public-diversity pass. `scripts/testnet-cobalt-placement-preflight` passes the overlay through to the draft subreport. Current no-overlay evidence still blocks on three complete slots/groups: `reports/testnet-cobalt-placement-manifest-draft/draft-overlay-ready-v0-20260519T0009Z/testnet-cobalt-placement-manifest-draft.json`; public-diversity-required evidence records the missing public labels: `reports/testnet-cobalt-placement-manifest-draft/draft-overlay-public-missing-v0-20260519T0009Z/testnet-cobalt-placement-manifest-draft.json`; integrated preflight evidence: `reports/testnet-cobalt-placement-preflight/preflight-overlay-ready-v0-20260519T0009Z/testnet-cobalt-placement-preflight.json` and `reports/testnet-cobalt-placement-preflight/preflight-overlay-public-missing-v0-20260519T0009Z/testnet-cobalt-placement-preflight.json`. |
| COBALT-115 | P0 | Done | Emit operator-fillable Cobalt public-diversity overlay template. | `scripts/testnet-cobalt-placement-manifest-draft` now supports `WRITE_DIVERSITY_OVERLAY_TEMPLATE=1` and writes `cobalt-placement-diversity-overlay-template.json` with schema `postfiat-testnet-cobalt-placement-diversity-overlay-v1`, public-diversity fields, and only credential machine indexes. Current evidence emits a redacted three-row template while still blocking on missing machines/groups: `reports/testnet-cobalt-placement-manifest-draft/diversity-template-v0-20260519T0016Z/testnet-cobalt-placement-manifest-draft.json`; template: `reports/testnet-cobalt-placement-manifest-draft/diversity-template-v0-20260519T0016Z/cobalt-placement-diversity-overlay-template.json`. |
| COBALT-116 | P0 | Done | Auto-use generated Cobalt placement manifest drafts in preflight. | `scripts/testnet-cobalt-placement-preflight` now runs the draft command before selecting the effective manifest. If no explicit `COBALT_PLACEMENT_MANIFEST` / `PLACEMENT_MANIFEST` is supplied and the generated draft exists and is redacted, preflight uses it for manifest verification, placement-capacity, and topology-diversity subreports. Current 7-validator evidence still blocks and falls back to the default manifest because no seven-target draft exists: `reports/testnet-cobalt-placement-preflight/preflight-auto-draft-v0-20260519T0019Z/testnet-cobalt-placement-preflight.json`. Functional evidence with `VALIDATORS=3` proves the generated-draft branch by selecting `placement_manifest_source=generated_draft` and passing manifest verification plus placement capacity before the remote topology gate blocks on unrelated 7-validator remote evidence: `reports/testnet-cobalt-placement-preflight/preflight-auto-draft-functional-v0-20260519T0019Z/testnet-cobalt-placement-preflight.json`. |
| COBALT-117 | P0 | Done | Include public-diversity overlay template in every Cobalt placement preflight. | `scripts/testnet-cobalt-placement-preflight` now sets `WRITE_DIVERSITY_OVERLAY_TEMPLATE=1` for the draft subreport by default, records the template path in `inputs`, and exposes template status in `placement_manifest_draft`. Current 7-validator evidence still blocks on missing machines/groups but includes a redacted three-row template: `reports/testnet-cobalt-placement-preflight/preflight-template-integrated-v0-20260519T0029Z/testnet-cobalt-placement-preflight.json`; template: `reports/testnet-cobalt-placement-preflight/preflight-template-integrated-v0-20260519T0029Z/cobalt-placement-diversity-overlay-template.json`. Functional `VALIDATORS=3` evidence proves generated-draft selection, manifest verification, placement capacity, and template emission together: `reports/testnet-cobalt-placement-preflight/preflight-template-integrated-functional-v0-20260519T0029Z/testnet-cobalt-placement-preflight.json`. |
| COBALT-118 | P0 | Done | Surface the actionable Cobalt placement packet in release and replay gates. | `scripts/testnet-cobalt-full-release-gate` and `scripts/testnet-cobalt-full-replay-verify` now expose placement-manifest source, generated-draft availability, diversity overlay template path, missing placement deltas, required operator inputs, and rerun commands from the placement preflight report. Refreshed non-mutating remote evidence blocks before mutation as expected at `reports/testnet-cobalt-full-nonuniform-remote-drill/full-cobalt-placement-packet-v0-20260519T0041Z/testnet-cobalt-full-nonuniform-remote-drill.json`; release and replay fail closed with the same placement packet at `reports/testnet-cobalt-full-release-gate/full-cobalt-placement-packet-v1-20260519T0041Z/testnet-cobalt-full-release-gate.json` and `reports/testnet-cobalt-full-replay-verify/full-cobalt-placement-packet-v1-20260519T0041Z/testnet-cobalt-full-replay-verify.json`. |
| COBALT-119 | P0 | Done | Bind placement-preflight launch satisfaction to the remote drill. | Release and replay now record `placement_preflight.source` plus `placement_preflight.bound_to_remote_drill`, prefer the remote drill's embedded placement preflight, and require that remote-bound evidence when `REQUIRE_COBALT_PLACEMENT_PREFLIGHT=1`. This prevents a standalone preflight report from satisfying a launch gate if the remote drill did not actually run with the placement preflight enabled. Evidence: `reports/testnet-cobalt-full-release-gate/full-cobalt-placement-bound-v0-20260519T0052Z/testnet-cobalt-full-release-gate.json` and `reports/testnet-cobalt-full-replay-verify/full-cobalt-placement-bound-v0-20260519T0052Z/testnet-cobalt-full-replay-verify.json`. |
| COBALT-120 | P0 | Done | Add Cobalt gate scripts to the standard local check path. | `scripts/check` now py-compiles the full-Cobalt remote drill, release gate, replay verifier, placement-manifest draft, placement preflight, and topology-diversity gate, and runs the Cobalt placement-manifest draft self-test. Focused checks passed: `python3 -m py_compile ...`, `scripts/testnet-cobalt-placement-manifest-draft --self-test`, `bash -n scripts/check`, and `git diff --check`. |
| COBALT-121 | P0 | Done | Add strict read-only Cobalt controlled-launch gate. | `scripts/testnet-cobalt-controlled-launch-gate` runs the full-Cobalt release and replay gates with topology diversity required, placement preflight required, reused-machine topology disallowed, and placement evidence required to be bound to the remote drill. It writes a single redacted launch report with top-level blockers plus the placement deltas and operator inputs. Current evidence fails closed at `reports/testnet-cobalt-controlled-launch-gate/controlled-launch-strict-v1-20260519T0112Z/testnet-cobalt-controlled-launch-gate.json` because topology diversity and placement preflight are not yet satisfied. |
| COBALT-122 | P0 | Done | Require redacted release/replay subreports inside the strict Cobalt launch gate. | `scripts/testnet-cobalt-controlled-launch-gate` now verifies the composed release and replay JSON reports are redacted before the strict launch gate can pass. Current evidence confirms both subreports are redacted while the gate still fails closed on topology and placement blockers: `reports/testnet-cobalt-controlled-launch-gate/controlled-launch-strict-redacted-v0-20260519T0117Z/testnet-cobalt-controlled-launch-gate.json`. |
| COBALT-123 | P0 | Done | Make trust-view count and non-identical G1 evidence first-class launch checks. | `scripts/testnet-cobalt-full-release-gate`, `scripts/testnet-cobalt-full-replay-verify`, and `scripts/testnet-cobalt-controlled-launch-gate` now expose explicit trust-view launch checks instead of relying on inference from RBC transport evidence. Current strict evidence shows minimum three trust views required, seven G1 trust views observed, non-identical G1 trust views true, and RBC distinct trust views seven while still failing closed on topology/placement: `reports/testnet-cobalt-controlled-launch-gate/controlled-launch-trustviews-v0-20260519T0126Z/testnet-cobalt-controlled-launch-gate.json`. |
| COBALT-124 | P0 | Done | Bind topology-diversity launch evidence to the same remote drill packet. | `scripts/testnet-cobalt-full-release-gate` and `scripts/testnet-cobalt-full-replay-verify` now require the topology-diversity report to reference the same full-Cobalt remote drill used by the release/replay run when topology diversity is required. `scripts/testnet-cobalt-controlled-launch-gate` lifts that check to the strict launch report. Current evidence fails closed with `release_topology_bound_to_remote_drill=false` and `replay_topology_bound_to_remote_drill=false` because the selected topology report points at an older remote packet: `reports/testnet-cobalt-controlled-launch-gate/controlled-launch-topology-bound-v0-20260519T0134Z/testnet-cobalt-controlled-launch-gate.json`. |
| COBALT-125 | P0 | Done | Generate strict-launch topology evidence from the pinned remote drill. | `scripts/testnet-cobalt-controlled-launch-gate` now selects one full-Cobalt remote drill packet, generates a topology-diversity subreport from it, and passes that exact topology report into release and replay. Current evidence has `release_topology_bound_to_remote_drill=true`, `replay_topology_bound_to_remote_drill=true`, and `topology_gate_bound_to_selected_remote=true`; it still fails closed on independent topology and placement preflight: `reports/testnet-cobalt-controlled-launch-gate/controlled-launch-autotopology-v0-20260519T0150Z/testnet-cobalt-controlled-launch-gate.json`. |
| COBALT-126 | P0 | Done | Surface strict independent-topology remediation in Cobalt launch evidence. | `scripts/testnet-cobalt-topology-diversity-gate` now emits redacted strict independent-topology deltas and required operator inputs. `scripts/testnet-cobalt-controlled-launch-gate` lifts those deltas to the top-level strict report. Current evidence states seven validators are present, three host/operator-host fingerprints are present, four more independent host/operator-host fingerprints are required, and four validator slots must move off reused hosts: `reports/testnet-cobalt-controlled-launch-gate/controlled-launch-topology-remediation-v0-20260519T0203Z/testnet-cobalt-controlled-launch-gate.json`. |
| COBALT-127 | P0 | Done | Add local self-tests for Cobalt topology remediation and strict launch summaries. | `scripts/testnet-cobalt-topology-diversity-gate --self-test` verifies the strict independent-topology delta math, and `scripts/testnet-cobalt-controlled-launch-gate --self-test` verifies the top-level strict-launch topology summary preserves those deltas. `scripts/check` now runs both self-tests. Refreshed evidence after wiring the self-tests: `reports/testnet-cobalt-controlled-launch-gate/controlled-launch-topology-remediation-selftest-v0-20260519T0210Z/testnet-cobalt-controlled-launch-gate.json`. |
| COBALT-128 | P0 | Done | Add deterministic Cobalt adversarial harness. | `crates/consensus_cobalt/examples/cobalt_adversarial_harness.rs` and `scripts/testnet-cobalt-adversarial-harness` run seven logical validators with non-identical trust views through eleven deterministic bad-condition scenarios. Evidence: `reports/testnet-cobalt-adversarial/adversarial-harness-v0-20260519T0228Z/testnet-cobalt-adversarial-harness.json`. |
| COBALT-129 | P0 | Done | Require the adversarial harness in strict Cobalt release/replay evidence. | `scripts/testnet-cobalt-full-release-gate`, `scripts/testnet-cobalt-full-replay-verify`, and `scripts/testnet-cobalt-controlled-launch-gate` now support and enforce `REQUIRE_COBALT_ADVERSARIAL_HARNESS=1`; the strict launch gate generates the harness packet and binds it into release plus replay. Evidence: `reports/testnet-cobalt-controlled-launch-gate/controlled-launch-adversarial-v0-20260519T0236Z/testnet-cobalt-controlled-launch-gate.json`. |
| COBALT-130 | P0 | Done | Add Cobalt collusion threshold matrix. | `crates/consensus_cobalt/examples/cobalt_collusion_threshold.rs` and `scripts/testnet-cobalt-collusion-threshold` enumerate all 128 captured-validator sets for the seven-validator graph. The report proves inside-fault-bound capture sets preserve safe linkage, cannot create captured strong support, and preserve liveness; it records first over-bound examples for liveness loss, linkage break, and captured strong support. Evidence: `reports/testnet-cobalt-adversarial/collusion-threshold-v0-20260519T0308Z/testnet-cobalt-collusion-threshold.json`. |
| COBALT-131 | P0 | Done | Add Cobalt correlated capture model. | `crates/consensus_cobalt/examples/cobalt_capture_model.rs` and `scripts/testnet-cobalt-capture-model` evaluate correlated capture by host, operator, funding, jurisdiction, and injected capture sets. The report detects current reused-group risk, proves strict independent single-group capture is safe, and detects single-funding-source captured support. Evidence: `reports/testnet-cobalt-adversarial/capture-model-v0-20260519T0320Z/testnet-cobalt-capture-model.json`. |
| COBALT-132 | P0 | Done | Add Cobalt trust graph poisoning packet. | `crates/consensus_cobalt/examples/cobalt_trust_graph_poison.rs` and `scripts/testnet-cobalt-trust-graph-poison` prove unsafe linkage updates, invalid subset parameters, duplicate validator scope, missing validator references, stale view versions, malformed trust-view signatures, and tampered lifecycle records all fail closed before activation. Evidence: `reports/testnet-cobalt-adversarial/trust-graph-poison-v0-20260519T0330Z/testnet-cobalt-trust-graph-poison.json`. |
| COBALT-133 | P0 | Done | Add Cobalt stale replay rejection packet. | `crates/consensus_cobalt/examples/cobalt_stale_replay.rs` and `scripts/testnet-cobalt-stale-replay` prove active G1 rejects old G0 non-uniform certificates, proposals, linkage reports, registry roots, trust-view ids, and DABC replay bundles. Evidence: `reports/testnet-cobalt-adversarial/stale-replay-v0-20260519T0345Z/testnet-cobalt-stale-replay.json`. |
| COBALT-134 | P0 | Done | Add Cobalt RBC Byzantine packet. | `crates/consensus_cobalt/examples/cobalt_rbc_byzantine.rs` and `scripts/testnet-cobalt-rbc-byzantine` prove double-propose/conflicting-accept detection, conflicting echo/ready/accept rejection, triggerless ready/accept denial, duplicate message dedupe, invalid signature rejection, and withheld-ready non-acceptance. Evidence: `reports/testnet-cobalt-adversarial/rbc-byzantine-v0-20260519T0358Z/testnet-cobalt-rbc-byzantine.json`. |
| COBALT-135 | P0 | Done | Add Cobalt ABBA Byzantine packet. | `crates/consensus_cobalt/examples/cobalt_abba_byzantine.rs` and `scripts/testnet-cobalt-abba-byzantine` prove init/aux/conf/finish equivocation detection, withheld-support nontermination, invalid signature and bad-round rejection, conflicting finish evidence, live-mode deterministic coin rejection, and single-sender nontermination. Evidence: `reports/testnet-cobalt-adversarial/abba-byzantine-v0-20260519T0410Z/testnet-cobalt-abba-byzantine.json`. |
| COBALT-136 | P0 | Done | Add Cobalt MVBA/DABC invalid-candidate packet. | `validate_mvba_candidate` now checks that `propose_message_id` matches the candidate root/proposer/slot/payload tuple, and `validate_dabc_ratified_amendment` rejects zero activation heights plus skipped amendment slots. `crates/consensus_cobalt/examples/cobalt_dabc_invalid_candidates.rs` and `scripts/testnet-cobalt-dabc-invalid-candidates` prove invalid RBC accepts, conflicting candidate ids, stale propose-id/payload mismatches, duplicate raw candidates, bad output ids, conflicting parent hashes, skipped slots, and wrong activation heights fail closed. Evidence: `reports/testnet-cobalt-adversarial/dabc-invalid-candidates-v0-20260519T0425Z/testnet-cobalt-dabc-invalid-candidates.json`. |
| COBALT-137 | P0 | Done | Add Cobalt membership-race packet. | `crates/consensus_cobalt/examples/cobalt_membership_race.rs` and `scripts/testnet-cobalt-membership-race` prove old-set blocks after activation, new-set blocks before activation, mixed old/new block-membership metadata, stale transaction-network ids, wrong graph roots, non-advancing activation heights, stale governance epochs, and stale DABC membership payloads fail closed. Evidence: `reports/testnet-cobalt-adversarial/membership-race-v0-20260519T0445Z/testnet-cobalt-membership-race.json`. |
| COBALT-138 | P0 | Done | Add Cobalt partition and message-disorder simulation. | `crates/consensus_cobalt/examples/cobalt_partition_simulation.rs` and `scripts/testnet-cobalt-partition-simulation` prove 3/4 and 2/2/3 partitions are safe but not live before heal, single-validator isolation preserves six-validator progress, delay/reorder/duplicate delivery is deterministic, and healed conflicting replay produces RBC conflict evidence. Evidence: `reports/testnet-cobalt-adversarial/partition-simulation-v0-20260519T0505Z/testnet-cobalt-partition-simulation.json`. |
| COBALT-139 | P0 | Done | Add Cobalt crash/restart persistence packet. | `crates/consensus_cobalt/examples/cobalt_crash_restart.rs` and `scripts/testnet-cobalt-crash-restart` prove serialized RBC replay is idempotent, ABBA equivocation evidence survives restart, MVBA/DABC replay verifies after reload, graph activation records revalidate, validator suspension DABC bindings survive reload, rollback restores the authority graph, and stale DABC replay is rejected after graph restart. Evidence: `reports/testnet-cobalt-adversarial/crash-restart-v0-20260519T0525Z/testnet-cobalt-crash-restart.json`. Live process-kill is now covered separately under `COBALT-151`. |
| COBALT-140 | P0 | Done | Add Cobalt resource/verification DoS packet. | `MAX_COBALT_SIGNATURE_HEX_LEN` bounds signature size before verification. `crates/consensus_cobalt/examples/cobalt_resource_dos.rs` and `scripts/testnet-cobalt-resource-dos` prove oversized RBC/ABBA/DABC signatures, malformed RBC payload hashes, DABC pending-pair floods, DABC checkpoint floods, RBC duplicate floods, and ABBA duplicate equivocations fail closed or dedupe deterministically. Evidence: `reports/testnet-cobalt-adversarial/resource-dos-v0-20260519T0545Z/testnet-cobalt-resource-dos.json`. |
| COBALT-141 | P0 | Done | Add Cobalt governance spam and amendment flood packet. | `MAX_MVBA_CANDIDATES_PER_SET` bounds MVBA valid-input candidate sets at 1024 before sorting/deduping and during replay validation. `crates/consensus_cobalt/examples/cobalt_governance_spam.rs` and `scripts/testnet-cobalt-governance-spam` prove many under-bound amendments select deterministically while candidate floods, raw replay floods, duplicate amendment slots, future pending slots, and invalid parent chains fail closed. Evidence: `reports/testnet-cobalt-adversarial/governance-spam-v0-20260519T0615Z/testnet-cobalt-governance-spam.json`. |
| COBALT-142 | P1 | Done | Add Cobalt parser/canonical-payload fuzz packet. | `crates/consensus_cobalt/examples/cobalt_parser_payload_fuzz.rs` and `scripts/testnet-cobalt-parser-payload-fuzz` exercise RBC, ABBA, DABC, trust graph, DABC replay bundle, and trust graph transition artifacts. The report proves valid corpus roundtrips preserve canonical signing payloads, truncated JSON fails parsing, protocol-version type mutations fail parsing, tampered ids/bindings fail validation, and replay/transition ids recompute from parsed payloads. Evidence: `reports/testnet-cobalt-adversarial/parser-payload-fuzz-v0-20260519T0645Z/testnet-cobalt-parser-payload-fuzz.json`. |
| COBALT-143 | P1 | Done | Add Cobalt long adversarial soak packet. | Fixed DABC long-chain extension by validating previous ratification core data without requiring the previous amendment to be genesis. `crates/consensus_cobalt/examples/cobalt_adversarial_soak.rs` and `scripts/testnet-cobalt-adversarial-soak` prove 32 sequential governance rounds with one scheduled offline validator per round, duplicate/reordered delivery, deterministic restart replay, stale replay rejection, below-threshold ABBA equivocation handling, and final DABC replay verification. Evidence: `reports/testnet-cobalt-adversarial/soak-v0-20260519T0715Z/testnet-cobalt-adversarial-soak.json`. |
| COBALT-144 | P1 | Done | Add Cobalt compromised-key recovery packet. | `rotate_key` now permits inactive-to-inactive replacement so a validator can be suspended, have compromised key material rotated while out of the active set, and then be reactivated. `crates/consensus_cobalt/examples/cobalt_key_compromise_recovery.rs` and `scripts/testnet-cobalt-key-compromise-recovery` prove DABC-bound suspension, inactive key rotation, reactivation, stale compromised support rejection, tampered compromised vote rejection, old-key reactivation rejection, and post-reactivation proposer acceptance. Evidence: `reports/testnet-cobalt-adversarial/key-compromise-recovery-v0-20260519T0730Z/testnet-cobalt-key-compromise-recovery.json`. |
| COBALT-145 | P1 | Done | Add Cobalt rollback recovery packet. | `crates/consensus_cobalt/examples/cobalt_rollback_recovery.rs` and `scripts/testnet-cobalt-rollback-recovery` prove unsafe trust-view updates fail before activation, a Byzantine-forced unsafe graph has explicit unsafe linkage, rollback restores authority trust views, rollback is DABC-ratified, replay verifies after JSON roundtrip, and tampered rollback/wrong DABC payloads fail closed. Evidence: `reports/testnet-cobalt-adversarial/rollback-recovery-v0-20260519T0750Z/testnet-cobalt-rollback-recovery.json`. |
| COBALT-146 | P1 | Done | Require the current Cobalt adversarial packet set in strict gates. | `scripts/testnet-cobalt-full-release-gate` and `scripts/testnet-cobalt-full-replay-verify` support `REQUIRE_COBALT_ADVERSARIAL_PACKET_SET=1` and validate current adversarial packet reports by schema/status/validator count/outside-operator flag. `scripts/testnet-cobalt-controlled-launch-gate` generates those packet reports, passes exact paths into release/replay, and verifies the packet set is present, redacted, required, bound to release, and accepted by replay. Evidence: `reports/testnet-cobalt-full-release-gate/full-cobalt-adversarial-packet-set-v0-20260519T0815Z/testnet-cobalt-full-release-gate.json`, `reports/testnet-cobalt-full-replay-verify/full-cobalt-adversarial-packet-set-v0-20260519T0815Z/testnet-cobalt-full-replay-verify.json`, `reports/testnet-cobalt-controlled-launch-gate/controlled-launch-adversarial-packet-set-v0-20260519T0815Z/testnet-cobalt-controlled-launch-gate.json`. The packet set now has 18 packets with live process-kill included by `COBALT-151`. |
| COBALT-147 | P1 | Done | Prefer mechanics-passing full-Cobalt remote drill evidence by default. | `scripts/testnet-cobalt-full-release-gate`, `scripts/testnet-cobalt-full-replay-verify`, and `scripts/testnet-cobalt-controlled-launch-gate` now select the newest mechanics-passing full-Cobalt remote drill when no explicit `FULL_COBALT_REMOTE_DRILL_REPORT` is provided, while keeping explicit overrides authoritative and replay bound to the release-selected drill. This prevents newer placement-preflight failure packets from creating false Cobalt mechanics blockers. Evidence: `reports/testnet-cobalt-full-release-gate/full-cobalt-mechanics-remote-selection-v0-20260519T0830Z/testnet-cobalt-full-release-gate.json`, `reports/testnet-cobalt-full-replay-verify/full-cobalt-mechanics-remote-selection-v0-20260519T0830Z/testnet-cobalt-full-replay-verify.json`, `reports/testnet-cobalt-controlled-launch-gate/controlled-launch-mechanics-remote-selection-v0-20260519T0830Z/testnet-cobalt-controlled-launch-gate.json`. Standalone release/replay pass; strict launch still fails closed only on topology and placement. |
| COBALT-148 | P1 | Done | Add standard self-test evidence for full-Cobalt gate selection. | `scripts/testnet-cobalt-full-release-gate --self-test`, `scripts/testnet-cobalt-full-replay-verify --self-test`, `scripts/testnet-cobalt-controlled-launch-gate --self-test`, and `scripts/testnet-cobalt-controlled-readiness-gate --self-test` assert default mechanics-passing selection, explicit override behavior, and controlled-vs-strict topology mode separation; replay also asserts release-bound report selection. `scripts/testnet-cobalt-gate-selection-self-test` records those checks as redacted JSON, and `scripts/check` runs the selector self-tests. Latest evidence: `reports/testnet-cobalt-gate-selection/gate-selection-live-process-kill-v0-20260519T0942Z/testnet-cobalt-gate-selection-self-test.json`. |
| COBALT-149 | P1 | Done | Make the standard full check path green after Cobalt gate hardening. | `scripts/check` now completes successfully. Cleanup included clippy-cleaning Cobalt adversarial examples, converting Cobalt rollback binding helpers to parameter structs, fixing mechanical node account-transaction clippy findings, and avoiding wire-format changes for shielded actions. Evidence: `reports/testnet-cobalt-check/full-check-clippy-clean-v0-20260519T0353Z/testnet-cobalt-check.json`. |
| COBALT-150 | P1 | Done | Add a controlled-testnet full-Cobalt readiness gate separate from the strict independent-topology launch gate. | `scripts/testnet-cobalt-controlled-readiness-gate` composes topology, release, replay, adversarial harness, and the full adversarial packet set with reused-machine topology explicitly allowed and placement preflight not required. It passes on current seven-logical-validator evidence while preserving `scripts/testnet-cobalt-controlled-launch-gate` as the strict independent-topology gate. Latest evidence with the 18-packet adversarial set: `reports/testnet-cobalt-controlled-readiness-gate/controlled-readiness-standard-check-v0-20260519T0600Z/testnet-cobalt-controlled-readiness-gate.json`. |
| COBALT-151 | P1 | Done | Add live process-kill and respawn evidence to the Cobalt adversarial packet set. | `crates/consensus_cobalt/examples/cobalt_live_process_kill.rs` and `scripts/testnet-cobalt-live-process-kill` now start seven actual local validator child processes for one Cobalt RBC plus ABBA plus MVBA/DABC request, kill the delayed validator before waiting for the round, prove the remaining six child processes accept/finish/ratify/replay under non-identical trust views, respawn the killed validator, and prove it repeats the DABC-aware path after restart. Release/replay now require the live packet's concurrent-worker, ABBA-finish, MVBA-selection, DABC-ratification, and DABC-replay checks before accepting the adversarial packet set. Evidence: `reports/testnet-cobalt-adversarial/live-process-kill-v7-20260519T1240Z/testnet-cobalt-live-process-kill.json`, `reports/testnet-cobalt-full-release-gate/full-cobalt-release-gate-v1-20260519T1205Z/testnet-cobalt-full-release-gate.json`, `reports/testnet-cobalt-full-replay-verify/full-cobalt-replay-v1-20260519T1205Z/testnet-cobalt-full-replay-verify.json`, and `reports/testnet-cobalt-controlled-readiness-gate/controlled-readiness-v6-20260519T1205Z/testnet-cobalt-controlled-readiness-gate.json`. |
| COBALT-152 | P1 | Done | Add live process-kill contract self-test to the standard check path. | `scripts/testnet-cobalt-live-process-kill --self-test` now validates the DABC-aware jq predicate without running a live drill: it accepts a complete packet and rejects missing concurrency, MVBA, DABC ratification, and restart DABC replay checks. `scripts/check` runs the self-test so future gate drift fails locally. Evidence: `reports/testnet-cobalt-adversarial/live-process-kill-self-test-v2-20260519T1240Z/testnet-cobalt-live-process-kill-self-test.json` and refreshed live packet `reports/testnet-cobalt-adversarial/live-process-kill-v7-20260519T1240Z/testnet-cobalt-live-process-kill.json`. |
| COBALT-153 | P1 | Done | Run the live process-kill drill in the standard check path. | `scripts/check` now runs `scripts/testnet-cobalt-live-process-kill` after the predicate self-test, so the default local gate exercises seven actual child validator processes through kill, survivor consensus, respawn, and DABC replay. Evidence: `reports/testnet-cobalt-adversarial/live-process-kill-standard-check-v0-20260519T1305Z/testnet-cobalt-live-process-kill.json` and `reports/testnet-cobalt-adversarial/live-process-kill-self-test-standard-v0-20260519T1305Z/testnet-cobalt-live-process-kill-self-test.json`. |
| COBALT-154 | P1 | Done | Run the controlled-readiness gate in the standard check path. | `scripts/check` now runs `scripts/testnet-cobalt-controlled-readiness-gate`, so local green checks prove the selected mechanics-good remote drill, controlled topology evidence, full adversarial packet set, release gate, and replay verifier still bind together for controlled-testnet Cobalt readiness. Evidence: `reports/testnet-cobalt-controlled-readiness-gate/controlled-readiness-standard-check-v0-20260519T0600Z/testnet-cobalt-controlled-readiness-gate.json`. |
| COBALT-155 | P1 | Done | Assert strict launch fails only on topology and placement. | `scripts/testnet-cobalt-strict-launch-expected-fail` runs the strict independent-topology launch gate, expects its nonzero fail-closed result, and rejects the packet unless all mechanics checks stay green and the only blockers are the known topology/placement requirements. `scripts/check` runs the wrapper. Evidence: `reports/testnet-cobalt-controlled-launch-gate/strict-expected-fail-standard-check-v0-20260519T0613Z/testnet-cobalt-strict-launch-expected-fail.json`. |
| COBALT-156 | P1 | Done | Self-test strict expected-fail wrapper predicates. | `scripts/testnet-cobalt-strict-launch-expected-fail --self-test` validates the wrapper without running the strict gate: it accepts the expected topology/placement fail-closed packet shape and rejects unexpected blockers, missing expected blockers, missing mechanics checks, and accidental strict-launch success. `scripts/check` runs the self-test before the live wrapper. Evidence: `reports/testnet-cobalt-controlled-launch-gate/strict-expected-fail-self-test-v0-20260519T0631Z/testnet-cobalt-strict-launch-expected-fail-self-test.json`. |
| COBALT-157 | P1 | Done | Require named checks for every Cobalt adversarial packet. | `scripts/testnet-cobalt-full-release-gate` and `scripts/testnet-cobalt-full-replay-verify` now maintain explicit required-check lists for every adversarial packet type and reject missing required checks or unexpected false checks. Evidence: `reports/testnet-cobalt-gate-selection/adversarial-packet-required-checks-v0-20260519T0645Z/testnet-cobalt-gate-selection-self-test.json` and `reports/testnet-cobalt-controlled-readiness-gate/adversarial-packet-required-checks-v0-20260519T0645Z/testnet-cobalt-controlled-readiness-gate.json`. |
| COBALT-158 | P1 | Done | Self-test release/replay adversarial packet contract drift. | `scripts/testnet-cobalt-gate-selection-self-test` now imports the release and replay gates and records that their adversarial packet specs, schemas, env bindings, filenames, and required-check lists match exactly. It also rejects missing per-packet required-check coverage, duplicate packet names, duplicate required-check names, and misplaced `outside_operators_required` packet checks. Evidence: `reports/testnet-cobalt-gate-selection/packet-contract-drift-v0-20260519T0715Z/testnet-cobalt-gate-selection-self-test.json`. |
| COBALT-159 | P1 | Done | Self-test Cobalt packet generator/verifier drift. | `scripts/testnet-cobalt-gate-selection-self-test` now imports controlled launch and controlled readiness, proves their adversarial packet command lists match, and proves generated packet names/report filenames/env bindings match the release/replay verifier contract. It also records that the separately generated adversarial harness is not duplicated as a subpacket. Evidence: `reports/testnet-cobalt-gate-selection/packet-generator-contract-v0-20260519T0730Z/testnet-cobalt-gate-selection-self-test.json`. |
| COBALT-160 | P1 | Done | Bind generated adversarial packet set into replay at controlled gates. | `scripts/testnet-cobalt-controlled-readiness-gate` and `scripts/testnet-cobalt-controlled-launch-gate` now require replay's `reports.adversarial_harness` and `reports.adversarial_packet_set` to match the freshly generated adversarial reports, not just release's paths. The strict expected-fail wrapper requires those replay-binding checks to stay true while topology/placement fail. Evidence: `reports/testnet-cobalt-controlled-readiness-gate/replay-packet-binding-v0-20260519T0745Z/testnet-cobalt-controlled-readiness-gate.json`, `reports/testnet-cobalt-controlled-launch-gate/replay-packet-binding-strict-expected-fail-v0-20260519T0745Z/testnet-cobalt-strict-launch-expected-fail.json`, and `reports/testnet-cobalt-controlled-launch-gate/replay-packet-binding-strict-self-test-v0-20260519T0745Z/testnet-cobalt-strict-launch-expected-fail-self-test.json`. |
| COBALT-161 | P1 | Done | Require exact adversarial packet maps in controlled gates. | `scripts/testnet-cobalt-controlled-readiness-gate` and `scripts/testnet-cobalt-controlled-launch-gate` now require release and replay `reports.adversarial_packet_set` maps to have exactly the generated packet names, not just matching paths for the expected subset. The strict expected-fail wrapper requires those exact-map checks to stay true. Evidence: `reports/testnet-cobalt-controlled-readiness-gate/exact-packet-map-v0-20260519T0825Z/testnet-cobalt-controlled-readiness-gate.json`, `reports/testnet-cobalt-controlled-launch-gate/exact-packet-map-strict-expected-fail-v0-20260519T0825Z/testnet-cobalt-strict-launch-expected-fail.json`, and `reports/testnet-cobalt-controlled-launch-gate/exact-packet-map-strict-self-test-v0-20260519T0825Z/testnet-cobalt-strict-launch-expected-fail-self-test.json`. |
| COBALT-162 | P1 | Done | Self-test exact adversarial packet-map predicates. | `scripts/testnet-cobalt-controlled-launch-gate` now exposes shared exact-map and path-binding helpers, both controlled gates self-test matching, extra, missing, and wrong-path packet maps, and `scripts/testnet-cobalt-strict-launch-expected-fail --self-test` rejects a stale extra packet-map mechanics case. Evidence: `reports/testnet-cobalt-controlled-readiness-gate/exact-packet-map-predicate-v0-20260519T0915Z/testnet-cobalt-controlled-readiness-gate.json`, `reports/testnet-cobalt-controlled-launch-gate/exact-packet-map-predicate-strict-expected-fail-v0-20260519T0915Z/testnet-cobalt-strict-launch-expected-fail.json`, and `reports/testnet-cobalt-controlled-launch-gate/exact-packet-map-predicate-self-test-v0-20260519T0915Z/testnet-cobalt-strict-launch-expected-fail-self-test.json`. |
| COBALT-163 | P1 | Done | Expose adversarial packet-map binding summary in controlled reports. | `scripts/testnet-cobalt-controlled-readiness-gate` and `scripts/testnet-cobalt-controlled-launch-gate` now emit `adversarial_packet_set_binding` with expected packet names, release/replay counts, missing/extra packet names, and exact/bound booleans. Evidence: `reports/testnet-cobalt-controlled-readiness-gate/packet-binding-summary-v0-20260519T0935Z/testnet-cobalt-controlled-readiness-gate.json` and `reports/testnet-cobalt-controlled-launch-gate/packet-binding-summary-strict-expected-fail-v0-20260519T0935Z/testnet-cobalt-strict-launch-expected-fail.json`. |
| COBALT-164 | P1 | Done | Validate packet binding summary in strict expected-fail wrapper. | `scripts/testnet-cobalt-strict-launch-expected-fail` now requires the nested strict-launch report's `adversarial_packet_set_binding` summary to be present, exact, bound, and empty of missing/extra packet names, then copies it to the wrapper report. Its self-test rejects stale and missing binding summaries. Evidence: `reports/testnet-cobalt-controlled-launch-gate/strict-wrapper-binding-self-test-v0-20260519T1010Z/testnet-cobalt-strict-launch-expected-fail-self-test.json` and `reports/testnet-cobalt-controlled-launch-gate/strict-wrapper-binding-summary-v0-20260519T1010Z/testnet-cobalt-strict-launch-expected-fail.json`. |
| COBALT-165 | P1 | Done | Add deterministic packet-name digest to Cobalt packet binding summaries. | `scripts/testnet-cobalt-controlled-launch-gate` now adds `expected_packet_names_sha256`, `release_packet_names_sha256`, and `replay_packet_names_sha256` to `adversarial_packet_set_binding`; controlled launch/readiness self-tests prove stale release/replay packet maps produce different digests, and `scripts/testnet-cobalt-strict-launch-expected-fail` recomputes the digest and rejects malformed summaries. Evidence: `reports/testnet-cobalt-controlled-readiness-gate/packet-binding-digest-v0-20260519T1045Z/testnet-cobalt-controlled-readiness-gate.json`, `reports/testnet-cobalt-controlled-launch-gate/packet-binding-digest-wrapper-self-test-v0-20260519T1045Z/testnet-cobalt-strict-launch-expected-fail-self-test.json`, and `reports/testnet-cobalt-controlled-launch-gate/packet-binding-digest-strict-wrapper-v0-20260519T1045Z/testnet-cobalt-strict-launch-expected-fail.json`. |
| COBALT-166 | P1 | Done | Promote packet-name digest binding into named Cobalt gate checks. | `scripts/testnet-cobalt-controlled-launch-gate` and `scripts/testnet-cobalt-controlled-readiness-gate` now emit `adversarial_packet_set_digest_bound_to_release` and `adversarial_packet_set_digest_bound_to_replay` checks, and `scripts/testnet-cobalt-strict-launch-expected-fail` requires those checks as mechanics that must remain true while strict topology/placement fail closed. Evidence: `reports/testnet-cobalt-controlled-readiness-gate/packet-digest-checks-v0-20260519T1115Z/testnet-cobalt-controlled-readiness-gate.json`, `reports/testnet-cobalt-controlled-launch-gate/packet-digest-checks-wrapper-self-test-v0-20260519T1115Z/testnet-cobalt-strict-launch-expected-fail-self-test.json`, and `reports/testnet-cobalt-controlled-launch-gate/packet-digest-checks-strict-wrapper-v0-20260519T1115Z/testnet-cobalt-strict-launch-expected-fail.json`. |
| COBALT-167 | P1 | Done | Expose deterministic adversarial packet identity in standalone release/replay reports. | `scripts/testnet-cobalt-full-release-gate` and `scripts/testnet-cobalt-full-replay-verify` now emit `adversarial_packet_set.packet_names` and `packet_names_sha256`, and their self-tests prove stale packet-name sets change the digest. `scripts/testnet-cobalt-gate-selection-self-test` now verifies release/replay packet identities match, counts match names, names match specs, and digests recompute. Evidence: `reports/testnet-cobalt-full-release-gate/packet-identity-v0-20260519T0924Z/testnet-cobalt-full-release-gate.json`, `reports/testnet-cobalt-full-replay-verify/packet-identity-v0-20260519T0924Z/testnet-cobalt-full-replay-verify.json`, and `reports/testnet-cobalt-gate-selection/packet-identity-contract-v0-20260519T0924Z/testnet-cobalt-gate-selection-self-test.json`. |
| COBALT-168 | P1 | Done | Make replay reject stale release packet identity. | `scripts/testnet-cobalt-full-replay-verify` now validates the release gate's embedded `adversarial_packet_set` identity against the replay verifier's current packet list and digest when the adversarial packet set is required. Its self-test rejects missing, stale, and malformed release packet identity, and refreshed replay evidence records `release_gate_adversarial_packet_identity_ok=true`. Evidence: `reports/testnet-cobalt-full-replay-verify/packet-identity-release-bound-v0-20260519T0936Z/testnet-cobalt-full-replay-verify.json`. |
| COBALT-169 | P1 | Done | Expose packet-identity binding in controlled Cobalt gates. | `scripts/testnet-cobalt-controlled-launch-gate` and `scripts/testnet-cobalt-controlled-readiness-gate` now emit `release_adversarial_packet_identity_bound_to_expected` and `replay_adversarial_packet_identity_bound_to_expected`, proving the standalone release/replay packet-name digests match the generated adversarial packet set. `scripts/testnet-cobalt-strict-launch-expected-fail` now requires those checks to stay true while topology/placement fail closed. Evidence: `reports/testnet-cobalt-controlled-readiness-gate/packet-identity-bound-v0-20260519T0948Z/testnet-cobalt-controlled-readiness-gate.json`, `reports/testnet-cobalt-controlled-launch-gate/packet-identity-bound-wrapper-self-test-v0-20260519T0947Z/testnet-cobalt-strict-launch-expected-fail-self-test.json`, and `reports/testnet-cobalt-controlled-launch-gate/packet-identity-bound-strict-expected-fail-v0-20260519T0948Z/testnet-cobalt-strict-launch-expected-fail.json`. |
| COBALT-170 | P1 | Done | Bind strict launch release/replay to the selected remote drill. | `scripts/testnet-cobalt-controlled-launch-gate` now emits `release_bound_to_selected_remote` and `replay_bound_to_selected_remote`, proving both standalone gates consumed the same selected full-Cobalt remote drill packet that the strict launch gate selected. `scripts/testnet-cobalt-strict-launch-expected-fail` now requires those checks as mechanics and its self-test rejects stale replay remote binding. Evidence: `reports/testnet-cobalt-controlled-launch-gate/selected-remote-binding-wrapper-self-test-v0-20260519T1006Z/testnet-cobalt-strict-launch-expected-fail-self-test.json` and `reports/testnet-cobalt-controlled-launch-gate/selected-remote-binding-strict-expected-fail-v0-20260519T1006Z/testnet-cobalt-strict-launch-expected-fail.json`. |
| COBALT-171 | P1 | Done | Add selected-remote binding to the gate-selection contract self-test. | `scripts/testnet-cobalt-gate-selection-self-test` now runs `scripts/testnet-cobalt-strict-launch-expected-fail --self-test` and imports its required-mechanics contract, proving `release_bound_to_selected_remote`, `replay_bound_to_selected_remote`, and `topology_gate_bound_to_selected_remote` stay required strict-launch mechanics. Evidence: `reports/testnet-cobalt-gate-selection/selected-remote-binding-contract-v0-20260519T1024Z/testnet-cobalt-gate-selection-self-test.json`. |
| COBALT-172 | P1 | Done | Exercise every selected-remote binding failure in the strict wrapper self-test. | `scripts/testnet-cobalt-strict-launch-expected-fail --self-test` now rejects stale selected-remote binding independently for release, replay, and topology evidence. `scripts/testnet-cobalt-gate-selection-self-test` also passes with that stricter wrapper predicate. Evidence: `reports/testnet-cobalt-controlled-launch-gate/selected-remote-binding-all-self-test-v0-20260519T1038Z/testnet-cobalt-strict-launch-expected-fail-self-test.json` and `reports/testnet-cobalt-gate-selection/selected-remote-binding-all-contract-v0-20260519T1038Z/testnet-cobalt-gate-selection-self-test.json`. |
| COBALT-173 | P1 | Done | Guard the strict expected-fail blocker contract. | `scripts/testnet-cobalt-gate-selection-self-test` now imports the strict expected-fail wrapper's `EXPECTED_BLOCKERS` and `REQUIRED_FALSE_CHECKS`, proves they exactly equal the known topology/placement blocker set, and proves strict mechanics true-checks do not overlap those blockers. Evidence: `reports/testnet-cobalt-gate-selection/strict-blocker-contract-v0-20260519T1052Z/testnet-cobalt-gate-selection-self-test.json`. |
| COBALT-174 | P1 | Done | Exercise strict blocker false-check rejection. | `scripts/testnet-cobalt-strict-launch-expected-fail --self-test` now rejects a synthetic packet where `release_gate_passed` flips true while the expected topology/placement blocker list is still present, proving `required_strict_blocker_checks_false` cannot be bypassed by blocker-list shape. The gate-selection self-test still passes with the stricter wrapper predicate. Evidence: `reports/testnet-cobalt-controlled-launch-gate/strict-blocker-false-check-self-test-v0-20260519T1103Z/testnet-cobalt-strict-launch-expected-fail-self-test.json` and `reports/testnet-cobalt-gate-selection/strict-blocker-false-check-contract-v0-20260519T1103Z/testnet-cobalt-gate-selection-self-test.json`. |
| COBALT-175 | P1 | Done | Exercise all strict blocker false-check contradictions. | `scripts/testnet-cobalt-strict-launch-expected-fail --self-test` now flips each `REQUIRED_FALSE_CHECKS` entry true one at a time and requires every case to fail via `required_strict_blocker_checks_false`. The gate-selection self-test passes with the expanded wrapper predicate. Evidence: `reports/testnet-cobalt-controlled-launch-gate/all-strict-blocker-false-checks-self-test-v0-20260519T1115Z/testnet-cobalt-strict-launch-expected-fail-self-test.json` and `reports/testnet-cobalt-gate-selection/all-strict-blocker-false-checks-contract-v0-20260519T1115Z/testnet-cobalt-gate-selection-self-test.json`. |
| COBALT-176 | P1 | Done | Exercise all strict required-true mechanics contradictions. | `scripts/testnet-cobalt-strict-launch-expected-fail --self-test` now flips each `REQUIRED_TRUE_CHECKS` entry false one at a time and requires every case to fail via `required_mechanics_checks_true`. The gate-selection self-test passes with the expanded wrapper predicate. Evidence: `reports/testnet-cobalt-controlled-launch-gate/all-required-true-checks-self-test-v0-20260519T1129Z/testnet-cobalt-strict-launch-expected-fail-self-test.json` and `reports/testnet-cobalt-gate-selection/all-required-true-checks-contract-v0-20260519T1129Z/testnet-cobalt-gate-selection-self-test.json`. |
| COBALT-177 | P1 | Done | Verify strict wrapper coverage from gate-selection JSON. | `scripts/testnet-cobalt-gate-selection-self-test` now parses the strict expected-fail self-test JSON report emitted during its run and proves the report loaded, the exhaustive coverage flags are true, the recorded required-true case names equal `REQUIRED_TRUE_CHECKS`, and the recorded required-false case names equal `REQUIRED_FALSE_CHECKS`. Evidence: `reports/testnet-cobalt-gate-selection/strict-self-json-coverage-contract-v0-20260519T1141Z/testnet-cobalt-gate-selection-self-test.json`. |
| COBALT-178 | P1 | Done | Self-test strict coverage contract negative cases. | `scripts/testnet-cobalt-gate-selection-self-test` now factors strict-wrapper coverage validation into a contract predicate and self-tests that the predicate accepts a complete synthetic report while rejecting missing required-true cases, missing required-false cases, stale extra required-true cases, false exhaustive flags, and missing reports. Evidence: `reports/testnet-cobalt-gate-selection/strict-coverage-negative-contract-v0-20260519T1156Z/testnet-cobalt-gate-selection-self-test.json`. |
| COBALT-179 | P1 | Done | Bind top-level Cobalt gate reports to git revision. | `scripts/testnet-cobalt-controlled-launch-gate` and `scripts/testnet-cobalt-controlled-readiness-gate` now emit git branch, revision, and dirty-state plus `git_revision_present` and `git_clean_if_required` checks. `REQUIRE_CLEAN_GIT=1` makes candidate evidence fail closed if the worktree is dirty. Clean controlled-readiness evidence records revision `986d731d79592a4570a3009ee0bbbf22874f72fa`, `dirty=false`, release passed, replay passed, and selected remote mechanics OK. Evidence: `reports/testnet-cobalt-controlled-readiness-gate/git-provenance-clean-v0-20260519T1219Z/testnet-cobalt-controlled-readiness-gate.json`. |
| COBALT-180 | P1 | Done | Bind standalone release/replay reports to git revision. | `scripts/testnet-cobalt-full-release-gate` and `scripts/testnet-cobalt-full-replay-verify` now emit git branch, revision, and dirty-state plus `git_revision_present` and `git_clean_if_required` checks. `REQUIRE_CLEAN_GIT=1` makes standalone release/replay evidence fail closed if the worktree is dirty. Clean evidence records revision `307c97954967bc44830b32253fb1facdd3b3aa10`, `dirty=false`, release passed, and replay passed. Evidence: `reports/testnet-cobalt-full-release-gate/git-provenance-clean-v0-20260519T1238Z/testnet-cobalt-full-release-gate.json` and `reports/testnet-cobalt-full-replay-verify/git-provenance-clean-v0-20260519T1238Z/testnet-cobalt-full-replay-verify.json`. |
| COBALT-181 | P1 | Done | Bind controlled gates to release/replay git provenance. | `scripts/testnet-cobalt-controlled-launch-gate` and `scripts/testnet-cobalt-controlled-readiness-gate` now require nested release/replay reports to match the parent gate's git revision, dirty-state, and `REQUIRE_CLEAN_GIT` setting. `scripts/testnet-cobalt-strict-launch-expected-fail` requires all six subreport provenance checks as mechanics, and the gate-selection self-test records strict coverage over the expanded required-true set. Clean controlled-readiness evidence records revision `a6e3d21575fbcd78e32809af10928387b976155d`, `dirty=false`, and all six subreport git-binding checks true. Evidence: `reports/testnet-cobalt-controlled-readiness-gate/subreport-git-binding-clean-v0-20260519T1256Z/testnet-cobalt-controlled-readiness-gate.json`, `reports/testnet-cobalt-controlled-launch-gate/subreport-git-binding-clean-strict-expected-fail-v0-20260519T1256Z/testnet-cobalt-strict-launch-expected-fail.json`, and `reports/testnet-cobalt-gate-selection/subreport-git-binding-contract-v1-20260519T1256Z/testnet-cobalt-gate-selection-self-test.json`. |
| COBALT-182 | P1 | Done | Require release/replay branch match and child git checks. | `scripts/testnet-cobalt-controlled-launch-gate` and `scripts/testnet-cobalt-controlled-readiness-gate` now require nested release/replay reports to match the parent gate's git branch and require the child reports' own `git_revision_present` and `git_clean_if_required` checks to pass. `scripts/testnet-cobalt-strict-launch-expected-fail` requires those four checks as mechanics, and the gate-selection self-test records strict coverage over all 52 required-true checks. Clean controlled-readiness evidence records revision `d3753deb8b1307307141fd02f67f51c448273521`, `dirty=false`, and all ten release/replay git provenance checks true. Evidence: `reports/testnet-cobalt-controlled-readiness-gate/subreport-git-checks-clean-v0-20260519T1311Z/testnet-cobalt-controlled-readiness-gate.json`, `reports/testnet-cobalt-controlled-launch-gate/subreport-git-checks-clean-strict-expected-fail-v0-20260519T1311Z/testnet-cobalt-strict-launch-expected-fail.json`, and `reports/testnet-cobalt-gate-selection/subreport-git-checks-contract-v1-20260519T1311Z/testnet-cobalt-gate-selection-self-test.json`. |
| COBALT-183 | P1 | Done | Require controlled-gate subreport schemas. | `scripts/testnet-cobalt-controlled-launch-gate` and `scripts/testnet-cobalt-controlled-readiness-gate` now require topology, release, and replay subreports to match the expected Cobalt schemas. `scripts/testnet-cobalt-strict-launch-expected-fail` requires those three schema checks as mechanics, and the gate-selection self-test records strict coverage over all 55 required-true checks. Clean controlled-readiness evidence records revision `ad91fb7be7b5a2ee7425e5c93d8ebb338263b084`, `dirty=false`, and all three subreport schema checks true. Evidence: `reports/testnet-cobalt-controlled-readiness-gate/subreport-schema-clean-v0-20260519T1329Z/testnet-cobalt-controlled-readiness-gate.json`, `reports/testnet-cobalt-controlled-launch-gate/subreport-schema-clean-strict-expected-fail-v0-20260519T1329Z/testnet-cobalt-strict-launch-expected-fail.json`, and `reports/testnet-cobalt-gate-selection/subreport-schema-contract-v1-20260519T1329Z/testnet-cobalt-gate-selection-self-test.json`. |
| COBALT-184 | P1 | Done | Expose standalone release/replay subreport schema checks. | `scripts/testnet-cobalt-full-release-gate` now exposes named schema checks for the local harness, remote drill, topology, placement preflight, and adversarial harness; `scripts/testnet-cobalt-full-replay-verify` now exposes named schema checks for remote drill, release gate, credential preflight, local harness, topology, placement preflight, and adversarial harness. Clean standalone evidence records revision `9d648c24544506a3cbade3c50b4e1dd2f6fe5a74`, `dirty=false`, and all new schema checks true. Evidence: `reports/testnet-cobalt-full-release-gate/standalone-subreport-schema-clean-v0-20260519T1351Z/testnet-cobalt-full-release-gate.json` and `reports/testnet-cobalt-full-replay-verify/standalone-subreport-schema-clean-v0-20260519T1351Z/testnet-cobalt-full-replay-verify.json`. |
| COBALT-185 | P1 | Done | Require standalone release/replay schema checks in parent gates. | `scripts/testnet-cobalt-controlled-launch-gate` and `scripts/testnet-cobalt-controlled-readiness-gate` now require `release_subreport_schema_checks_pass` and `replay_subreport_schema_checks_pass`; `scripts/testnet-cobalt-strict-launch-expected-fail` treats both checks as required mechanics. Clean controlled-readiness evidence records revision `20f61c230ee48f67b3b9da15f200756dfd8c5838`, `dirty=false`, release passed, replay passed, and both child schema-check aggregations true. Evidence: `reports/testnet-cobalt-controlled-readiness-gate/parent-subreport-schema-checks-clean-v0-20260519T1406Z/testnet-cobalt-controlled-readiness-gate.json`, `reports/testnet-cobalt-controlled-launch-gate/parent-subreport-schema-checks-clean-strict-expected-fail-v0-20260519T1406Z/testnet-cobalt-strict-launch-expected-fail.json`, and `reports/testnet-cobalt-gate-selection/parent-subreport-schema-checks-contract-v0-20260519T1406Z/testnet-cobalt-gate-selection-self-test.json`. |
| COBALT-186 | P1 | Done | Add parent-gate schema-check summaries. | `scripts/testnet-cobalt-controlled-launch-gate` and `scripts/testnet-cobalt-controlled-readiness-gate` now emit `subreport_schema_checks.release` and `subreport_schema_checks.replay` summaries listing expected child schema checks, observed booleans, and missing-or-false names. Clean controlled-readiness evidence records revision `72eb9d5af6a38ec11ba6751b7588fd78fbe6e51d`, `dirty=false`, both parent schema-check aggregations true, and zero missing release/replay child schema checks. Evidence: `reports/testnet-cobalt-controlled-readiness-gate/subreport-schema-summary-clean-v0-20260519T1422Z/testnet-cobalt-controlled-readiness-gate.json`, `reports/testnet-cobalt-controlled-launch-gate/subreport-schema-summary-clean-strict-expected-fail-v0-20260519T1422Z/testnet-cobalt-strict-launch-expected-fail.json`, and `reports/testnet-cobalt-gate-selection/subreport-schema-summary-contract-v0-20260519T1422Z/testnet-cobalt-gate-selection-self-test.json`. |
| COBALT-187 | P1 | Done | Guard schema-check contract names in gate selection. | `scripts/testnet-cobalt-gate-selection-self-test` now records and validates the parent subreport-schema contract: strict expected-fail requires `release_subreport_schema_checks_pass` and `replay_subreport_schema_checks_pass`, release and replay schema-check name sets are nonempty and unique, and controlled readiness inherits the same schema-check name sets from controlled launch. Evidence: `reports/testnet-cobalt-gate-selection/schema-check-contract-clean-v0-20260519T1435Z/testnet-cobalt-gate-selection-self-test.json`. |
| COBALT-188 | P1 | Done | Refresh clean head Cobalt readiness and strict expected-fail evidence. | At revision `06283a9d6af62fa27ec45cfdc1b0d25c3d641b67`, `scripts/testnet-cobalt-controlled-readiness-gate` passes with `REQUIRE_CLEAN_GIT=1`, `dirty=false`, and no blockers; `scripts/testnet-cobalt-strict-launch-expected-fail` passes and proves strict launch fails only on known topology/placement blockers. Evidence: `reports/testnet-cobalt-controlled-readiness-gate/readiness-clean-head-v0-20260519T1438Z/testnet-cobalt-controlled-readiness-gate.json` and `reports/testnet-cobalt-controlled-launch-gate/strict-expected-fail-clean-head-v0-20260519T1438Z/testnet-cobalt-strict-launch-expected-fail.json`. |
| COBALT-189 | P1 | Done | Require amendment replay bundle evidence in the full-Cobalt packet contract. | `scripts/testnet-cobalt-full-release-gate`, `scripts/testnet-cobalt-full-replay-verify`, and `scripts/testnet-cobalt-controlled-launch-gate` now include `amendment_replay_bundle` as a required generated packet. Release and replay validate packet schema, ordered activation/supersession/rollback counts, tampered-bundle rejection, and node replay-verifier counts. Clean controlled-readiness evidence records 19 expected packets at revision `2080c17436abcb1f0de0a7dc21a0646aa404c4bd`. Evidence: `reports/testnet-cobalt-controlled-readiness-gate/amendment-replay-contract-clean-v1-20260519T150324Z/testnet-cobalt-controlled-readiness-gate.json` and `reports/testnet-cobalt-gate-selection/amendment-replay-contract-clean-v1-20260519T150324Z/testnet-cobalt-gate-selection-self-test.json`. |
| COBALT-190 | P1 | Done | Remove generated lifecycle-smoke key artifacts from retained amendment replay evidence. | `scripts/testnet-cobalt-amendment-replay-bundle` now deletes the generated `source-amendment-lifecycle-smoke/node` directory after the smoke report is consumed, before retaining the replay bundle evidence. The clean standalone replay-bundle run and controlled-readiness run retained no node-key directory and no key-shaped fields under the selected evidence trees. Evidence: `reports/testnet-cobalt-amendment-replay-bundle/cleanup-clean-v1-20260519T150324Z/testnet-cobalt-amendment-replay-bundle.json` and `reports/testnet-cobalt-controlled-readiness-gate/amendment-replay-contract-clean-v1-20260519T150324Z/adversarial-packets/amendment_replay_bundle/testnet-cobalt-amendment-replay-bundle.json`. |
| COBALT-103 | P1 | Open | Update public docs after remote non-uniform evidence passes. | Whitepaper/current-state docs cite full-Cobalt evidence, not canonical evidence. |

## Current Implementation Slice

The Cobalt path is now past local-only mechanics: the current slice has
credential-aligned reused-machine remote evidence, release gating, and offline
replay. The adversarial lane has started: a deterministic seven-validator
harness now proves the first scripted bad-condition packet, and strict
controlled launch now requires that packet through release and replay. The
collusion threshold matrix now proves the current G1-style graph's
inside-`t_S` safety/liveness behavior and records the over-bound failure
thresholds. The capture model now turns bribery/correlated control into
machine-checkable host/operator/funding/jurisdiction group capture evidence.
The trust-graph poison packet now proves malformed or unsafe graph updates fail
closed before activation. The stale replay packet now proves old G0 governance
evidence is rejected after G1 activation. The RBC Byzantine packet now proves
proposer/voter faults fail closed at validation, support, or conflict-evidence
boundaries. The ABBA Byzantine packet now proves sender faults fail closed at
equivocation-evidence, support, validation, or common-coin boundaries. The
MVBA/DABC invalid-candidate packet now proves malformed candidates,
ratification links, skipped slots, and activation-height tampering fail closed.
The membership-race packet now proves stale or mixed transaction-network
membership evidence fails closed on both sides of Cobalt-ratified activation.
The partition simulation packet now makes partition liveness explicit while
preserving safety and deterministic replay behavior.
The crash/restart packet now proves persisted Cobalt artifacts revalidate after
reload and stale replay still fails closed after graph restart.
The resource/verification DoS packet now adds an explicit signature-size bound
and proves core flood/malformed-input paths fail closed or dedupe.
The governance-spam packet now bounds MVBA candidate-set fanout and proves
amendment floods, duplicate slots, future pending slots, and invalid parent
chains fail closed.
The parser/canonical-payload fuzz packet now proves valid Cobalt artifacts
round-trip without canonical payload drift and malformed or tampered payloads
fail parsing or validation.
The long adversarial soak packet now proves repeated DABC ratification and
replay across 32 governance rounds under deterministic crash/offline, message
disorder, stale replay, and below-threshold equivocation pressure. It also
fixed a real DABC long-chain extension bug exposed by the soak.
Independent physical/operator topology remains a public-credibility evidence
gap, not a controlled-testnet code blocker.

Latest completed slice:

- `COBALT-190`: removed generated lifecycle-smoke key artifacts from retained
  amendment replay evidence. `scripts/testnet-cobalt-amendment-replay-bundle`
  now deletes the generated node-key directory after consuming the smoke report,
  and the clean evidence trees for the standalone bundle and controlled
  readiness have no node-key directory or key-shaped fields.
  Evidence:
  `reports/testnet-cobalt-amendment-replay-bundle/cleanup-clean-v1-20260519T150324Z/testnet-cobalt-amendment-replay-bundle.json`
  and
  `reports/testnet-cobalt-controlled-readiness-gate/amendment-replay-contract-clean-v1-20260519T150324Z/adversarial-packets/amendment_replay_bundle/testnet-cobalt-amendment-replay-bundle.json`.

Previous completed slice:

- `COBALT-189`: required amendment replay bundle evidence in the full-Cobalt
  release/replay/controlled gate contract. Controlled readiness now expects 19
  packets, including `amendment_replay_bundle`; release and replay both accept
  the generated packet and reject stale shape in self-tests. Evidence:
  `reports/testnet-cobalt-controlled-readiness-gate/amendment-replay-contract-clean-v1-20260519T150324Z/testnet-cobalt-controlled-readiness-gate.json`
  and
  `reports/testnet-cobalt-gate-selection/amendment-replay-contract-clean-v1-20260519T150324Z/testnet-cobalt-gate-selection-self-test.json`.

Earlier completed slice:

- `COBALT-188`: refreshed clean head Cobalt readiness and strict expected-fail
  evidence at pushed revision `06283a9d6af62fa27ec45cfdc1b0d25c3d641b67`.
  Controlled readiness passes with `REQUIRE_CLEAN_GIT=1`, `dirty=false`, and no
  blockers. The strict expected-fail wrapper passes and confirms the nested
  strict launch gate fails only on the known topology/placement blockers.
  Evidence:
  `reports/testnet-cobalt-controlled-readiness-gate/readiness-clean-head-v0-20260519T1438Z/testnet-cobalt-controlled-readiness-gate.json`
  and
  `reports/testnet-cobalt-controlled-launch-gate/strict-expected-fail-clean-head-v0-20260519T1438Z/testnet-cobalt-strict-launch-expected-fail.json`.

Previous completed slice:

- `COBALT-187`: guarded schema-check contract names in gate selection.
  `scripts/testnet-cobalt-gate-selection-self-test` now records and validates
  that strict expected-fail requires `release_subreport_schema_checks_pass` and
  `replay_subreport_schema_checks_pass`, release and replay child schema-check
  names are nonempty and unique, and controlled readiness inherits the same
  schema-check name sets from controlled launch. Evidence:
  `reports/testnet-cobalt-gate-selection/schema-check-contract-clean-v0-20260519T1435Z/testnet-cobalt-gate-selection-self-test.json`.

Earlier completed slice:

- `COBALT-186`: added parent-gate schema-check summaries. The controlled
  launch/readiness gates now emit `subreport_schema_checks.release` and
  `subreport_schema_checks.replay`, with expected child check names, observed
  booleans, and missing-or-false lists. Clean controlled-readiness evidence
  records revision `72eb9d5af6a38ec11ba6751b7588fd78fbe6e51d`,
  `dirty=false`, both parent schema-check aggregations true, and zero missing
  release/replay child schema checks. Evidence:
  `reports/testnet-cobalt-controlled-readiness-gate/subreport-schema-summary-clean-v0-20260519T1422Z/testnet-cobalt-controlled-readiness-gate.json`,
  `reports/testnet-cobalt-controlled-launch-gate/subreport-schema-summary-clean-strict-expected-fail-v0-20260519T1422Z/testnet-cobalt-strict-launch-expected-fail.json`,
  `reports/testnet-cobalt-gate-selection/subreport-schema-summary-contract-v0-20260519T1422Z/testnet-cobalt-gate-selection-self-test.json`.

Previous completed slice:

- `COBALT-185`: required standalone release/replay schema checks in parent
  gates. `scripts/testnet-cobalt-controlled-launch-gate` and
  `scripts/testnet-cobalt-controlled-readiness-gate` now require
  `release_subreport_schema_checks_pass` and
  `replay_subreport_schema_checks_pass`; the strict expected-fail wrapper
  treats both checks as required mechanics. Clean controlled-readiness evidence
  records revision `20f61c230ee48f67b3b9da15f200756dfd8c5838`,
  `dirty=false`, release passed, replay passed, and both child schema-check
  aggregations true. Evidence:
  `reports/testnet-cobalt-controlled-readiness-gate/parent-subreport-schema-checks-clean-v0-20260519T1406Z/testnet-cobalt-controlled-readiness-gate.json`,
  `reports/testnet-cobalt-controlled-launch-gate/parent-subreport-schema-checks-clean-strict-expected-fail-v0-20260519T1406Z/testnet-cobalt-strict-launch-expected-fail.json`,
  `reports/testnet-cobalt-gate-selection/parent-subreport-schema-checks-contract-v0-20260519T1406Z/testnet-cobalt-gate-selection-self-test.json`.

Previous completed slice:

- `COBALT-184`: exposed standalone release/replay subreport schema checks.
  `scripts/testnet-cobalt-full-release-gate` now emits named schema checks for
  its local harness, remote drill, topology, placement preflight, and
  adversarial harness inputs. `scripts/testnet-cobalt-full-replay-verify` now
  emits named schema checks for remote drill, release gate, credential
  preflight, local harness, topology, placement preflight, and adversarial
  harness inputs. Clean standalone evidence records revision
  `9d648c24544506a3cbade3c50b4e1dd2f6fe5a74`, `dirty=false`, and all new
  schema checks true. Evidence:
  `reports/testnet-cobalt-full-release-gate/standalone-subreport-schema-clean-v0-20260519T1351Z/testnet-cobalt-full-release-gate.json`,
  `reports/testnet-cobalt-full-replay-verify/standalone-subreport-schema-clean-v0-20260519T1351Z/testnet-cobalt-full-replay-verify.json`.

Previous completed slice:

- `COBALT-183`: required controlled-gate subreport schemas. The controlled
  launch/readiness gates now require topology, release, and replay subreports
  to match the expected Cobalt schemas, and the strict expected-fail wrapper
  treats those three checks as required mechanics. Clean evidence records
  revision `ad91fb7be7b5a2ee7425e5c93d8ebb338263b084`, `dirty=false`, and all
  three schema checks true. Evidence:
  `reports/testnet-cobalt-controlled-readiness-gate/subreport-schema-clean-v0-20260519T1329Z/testnet-cobalt-controlled-readiness-gate.json`,
  `reports/testnet-cobalt-controlled-launch-gate/subreport-schema-clean-strict-expected-fail-v0-20260519T1329Z/testnet-cobalt-strict-launch-expected-fail.json`,
  `reports/testnet-cobalt-gate-selection/subreport-schema-contract-v1-20260519T1329Z/testnet-cobalt-gate-selection-self-test.json`.

Previous completed slice:

- `COBALT-182`: required release/replay branch match and child git checks.
  `scripts/testnet-cobalt-controlled-launch-gate` and
  `scripts/testnet-cobalt-controlled-readiness-gate` now require nested
  release/replay reports to match the parent gate's git branch, and require
  the child reports' own `git_revision_present` and `git_clean_if_required`
  checks to pass. `scripts/testnet-cobalt-strict-launch-expected-fail` now
  treats those four checks as required mechanics. Clean evidence records
  revision `d3753deb8b1307307141fd02f67f51c448273521`, `dirty=false`, and all
  ten release/replay git provenance checks true. Evidence:
  `reports/testnet-cobalt-controlled-readiness-gate/subreport-git-checks-clean-v0-20260519T1311Z/testnet-cobalt-controlled-readiness-gate.json`,
  `reports/testnet-cobalt-controlled-launch-gate/subreport-git-checks-clean-strict-expected-fail-v0-20260519T1311Z/testnet-cobalt-strict-launch-expected-fail.json`,
  `reports/testnet-cobalt-gate-selection/subreport-git-checks-contract-v1-20260519T1311Z/testnet-cobalt-gate-selection-self-test.json`.

Previous completed slice:

- `COBALT-181`: bound controlled Cobalt gates to release/replay git provenance.
  `scripts/testnet-cobalt-controlled-launch-gate` and
  `scripts/testnet-cobalt-controlled-readiness-gate` now reject release/replay
  subreports whose git revision, dirty-state, or clean-git requirement differs
  from the parent gate. `scripts/testnet-cobalt-strict-launch-expected-fail`
  requires those six checks as mechanics while the strict topology/placement
  checks remain expected blockers. Clean evidence records revision
  `a6e3d21575fbcd78e32809af10928387b976155d`, `dirty=false`, and all six
  subreport git-binding checks true. Evidence:
  `reports/testnet-cobalt-controlled-readiness-gate/subreport-git-binding-clean-v0-20260519T1256Z/testnet-cobalt-controlled-readiness-gate.json`,
  `reports/testnet-cobalt-controlled-launch-gate/subreport-git-binding-clean-strict-expected-fail-v0-20260519T1256Z/testnet-cobalt-strict-launch-expected-fail.json`,
  `reports/testnet-cobalt-gate-selection/subreport-git-binding-contract-v1-20260519T1256Z/testnet-cobalt-gate-selection-self-test.json`.

Previous completed slice:

- `COBALT-180`: bound standalone full-Cobalt release/replay reports to git
  provenance. `scripts/testnet-cobalt-full-release-gate` and
  `scripts/testnet-cobalt-full-replay-verify` now emit git branch, revision,
  and dirty-state; `REQUIRE_CLEAN_GIT=1` makes standalone evidence fail closed
  unless generated from a clean committed checkout. Clean evidence records
  revision `307c97954967bc44830b32253fb1facdd3b3aa10`, `dirty=false`, release
  passed, and replay passed. Evidence:
  `reports/testnet-cobalt-full-release-gate/git-provenance-clean-v0-20260519T1238Z/testnet-cobalt-full-release-gate.json`,
  `reports/testnet-cobalt-full-replay-verify/git-provenance-clean-v0-20260519T1238Z/testnet-cobalt-full-replay-verify.json`.

Previous completed slice:

- `COBALT-179`: bound top-level controlled Cobalt gate reports to git
  provenance. `scripts/testnet-cobalt-controlled-launch-gate` and
  `scripts/testnet-cobalt-controlled-readiness-gate` now emit git branch,
  revision, and dirty-state; `REQUIRE_CLEAN_GIT=1` makes candidate evidence
  fail closed unless generated from a clean committed checkout. Clean evidence
  records revision `986d731d79592a4570a3009ee0bbbf22874f72fa`, `dirty=false`,
  release passed, replay passed, and selected remote mechanics OK. Evidence:
  `reports/testnet-cobalt-controlled-readiness-gate/git-provenance-clean-v0-20260519T1219Z/testnet-cobalt-controlled-readiness-gate.json`.

Previous completed slice:

- `COBALT-178`: self-tested strict coverage contract negative cases. The
  gate-selection self-test now factors strict-wrapper coverage validation into
  a contract predicate and proves it accepts a complete synthetic report while
  rejecting missing required-true coverage, missing required-false coverage,
  stale extra required-true coverage, false exhaustive flags, and missing
  strict self-test reports. Evidence:
  `reports/testnet-cobalt-gate-selection/strict-coverage-negative-contract-v0-20260519T1156Z/testnet-cobalt-gate-selection-self-test.json`.

Previous completed slice:

- `COBALT-177`: verified strict wrapper coverage from the gate-selection JSON
  contract. `scripts/testnet-cobalt-gate-selection-self-test` now parses the
  strict expected-fail self-test JSON emitted during its run and proves the
  report loaded, the exhaustive coverage flags are true, the recorded
  required-true case names equal `REQUIRED_TRUE_CHECKS`, and the recorded
  required-false case names equal `REQUIRED_FALSE_CHECKS`. Evidence:
  `reports/testnet-cobalt-gate-selection/strict-self-json-coverage-contract-v0-20260519T1141Z/testnet-cobalt-gate-selection-self-test.json`.

Previous completed slice:

- `COBALT-176`: exercised all strict required-true mechanics contradictions.
  The strict expected-fail predicate self-test now flips each
  `REQUIRED_TRUE_CHECKS` entry false one at a time and requires every case to
  fail via `required_mechanics_checks_true`. The gate-selection self-test
  passes with the expanded wrapper predicate. Evidence:
  `reports/testnet-cobalt-controlled-launch-gate/all-required-true-checks-self-test-v0-20260519T1129Z/testnet-cobalt-strict-launch-expected-fail-self-test.json`,
  `reports/testnet-cobalt-gate-selection/all-required-true-checks-contract-v0-20260519T1129Z/testnet-cobalt-gate-selection-self-test.json`.

Previous completed slice:

- `COBALT-175`: exercised all strict blocker false-check contradictions. The
  strict expected-fail predicate self-test now flips each `REQUIRED_FALSE_CHECKS`
  entry true one at a time and requires every case to fail via
  `required_strict_blocker_checks_false`. The gate-selection self-test passes
  with the expanded wrapper predicate. Evidence:
  `reports/testnet-cobalt-controlled-launch-gate/all-strict-blocker-false-checks-self-test-v0-20260519T1115Z/testnet-cobalt-strict-launch-expected-fail-self-test.json`,
  `reports/testnet-cobalt-gate-selection/all-strict-blocker-false-checks-contract-v0-20260519T1115Z/testnet-cobalt-gate-selection-self-test.json`.

Previous completed slice:

- `COBALT-174`: exercised the strict blocker false-check rejection path. The
  strict expected-fail predicate self-test now rejects a packet where
  `release_gate_passed` flips true while the expected topology/placement
  blocker list remains present, proving `required_strict_blocker_checks_false`
  cannot be bypassed by blocker-list shape. The gate-selection self-test still
  passes with that stricter wrapper predicate. Evidence:
  `reports/testnet-cobalt-controlled-launch-gate/strict-blocker-false-check-self-test-v0-20260519T1103Z/testnet-cobalt-strict-launch-expected-fail-self-test.json`,
  `reports/testnet-cobalt-gate-selection/strict-blocker-false-check-contract-v0-20260519T1103Z/testnet-cobalt-gate-selection-self-test.json`.

Previous completed slice:

- `COBALT-173`: guarded the strict expected-fail blocker contract in the
  gate-selection self-test. The self-test now proves the wrapper's
  `EXPECTED_BLOCKERS` and `REQUIRED_FALSE_CHECKS` exactly equal the known
  topology/placement blocker set, and that strict mechanics true-checks do not
  overlap those blockers. Evidence:
  `reports/testnet-cobalt-gate-selection/strict-blocker-contract-v0-20260519T1052Z/testnet-cobalt-gate-selection-self-test.json`.

Previous completed slice:

- `COBALT-172`: expanded the strict expected-fail predicate self-test to
  reject stale selected-remote binding independently for release, replay, and
  topology evidence. The gate-selection self-test passes with the stricter
  wrapper predicate. Evidence:
  `reports/testnet-cobalt-controlled-launch-gate/selected-remote-binding-all-self-test-v0-20260519T1038Z/testnet-cobalt-strict-launch-expected-fail-self-test.json`,
  `reports/testnet-cobalt-gate-selection/selected-remote-binding-all-contract-v0-20260519T1038Z/testnet-cobalt-gate-selection-self-test.json`.

Previous completed slice:

- `COBALT-171`: added selected-remote binding to the Cobalt gate-selection
  contract self-test. `scripts/testnet-cobalt-gate-selection-self-test` now
  runs the strict expected-fail predicate self-test and imports its
  required-mechanics set, proving `release_bound_to_selected_remote`,
  `replay_bound_to_selected_remote`, and `topology_gate_bound_to_selected_remote`
  remain required strict-launch mechanics. Evidence:
  `reports/testnet-cobalt-gate-selection/selected-remote-binding-contract-v0-20260519T1024Z/testnet-cobalt-gate-selection-self-test.json`.

Previous completed slice:

- `COBALT-170`: bound strict launch release/replay evidence to the selected
  full-Cobalt remote drill. The strict launch report now emits
  `release_bound_to_selected_remote` and `replay_bound_to_selected_remote`,
  proving release and replay consumed the same selected full-Cobalt remote
  packet, and the strict expected-fail wrapper requires those mechanics to stay
  true while topology/placement fail closed. Its self-test rejects stale replay
  remote binding. Evidence:
  `reports/testnet-cobalt-controlled-launch-gate/selected-remote-binding-wrapper-self-test-v0-20260519T1006Z/testnet-cobalt-strict-launch-expected-fail-self-test.json`,
  `reports/testnet-cobalt-controlled-launch-gate/selected-remote-binding-strict-expected-fail-v0-20260519T1006Z/testnet-cobalt-strict-launch-expected-fail.json`.

Previous completed slice:

- `COBALT-169`: exposed packet-identity binding in controlled Cobalt gates.
  Controlled launch/readiness now emit
  `release_adversarial_packet_identity_bound_to_expected` and
  `replay_adversarial_packet_identity_bound_to_expected`, proving standalone
  release/replay packet identities match the freshly generated adversarial
  packet set. The strict expected-fail wrapper requires both checks to stay true
  while topology/placement fail closed. Evidence:
  `reports/testnet-cobalt-controlled-readiness-gate/packet-identity-bound-v0-20260519T0948Z/testnet-cobalt-controlled-readiness-gate.json`,
  `reports/testnet-cobalt-controlled-launch-gate/packet-identity-bound-wrapper-self-test-v0-20260519T0947Z/testnet-cobalt-strict-launch-expected-fail-self-test.json`,
  `reports/testnet-cobalt-controlled-launch-gate/packet-identity-bound-strict-expected-fail-v0-20260519T0948Z/testnet-cobalt-strict-launch-expected-fail.json`.

Previous completed slice:

- `COBALT-168`: made replay fail closed on stale release packet identity. When
  the adversarial packet set is required, `scripts/testnet-cobalt-full-replay-verify`
  now validates the release gate's embedded `adversarial_packet_set.packet_names`
  and `packet_names_sha256` against the replay verifier's current packet list.
  The replay self-test rejects missing, stale, and malformed release packet
  identity, and the refreshed replay packet records
  `release_gate_adversarial_packet_identity_ok=true`. Evidence:
  `reports/testnet-cobalt-full-replay-verify/packet-identity-release-bound-v0-20260519T0936Z/testnet-cobalt-full-replay-verify.json`.

Previous completed slice:

- `COBALT-167`: exposed deterministic adversarial packet identity in standalone
  release and replay reports. Both reports now carry the sorted
  `adversarial_packet_set.packet_names` list and
  `adversarial_packet_set.packet_names_sha256`, and their self-tests prove stale
  packet-name sets change the digest. The gate-selection self-test now proves
  release/replay packet identities match, counts match names, names match specs,
  and digests recompute. Current 18-packet digest:
  `ea4db10775a378cf04daeb5671b23e75fbe88b7b17e25266b7bf7926075369bc`.
  Evidence:
  `reports/testnet-cobalt-full-release-gate/packet-identity-v0-20260519T0924Z/testnet-cobalt-full-release-gate.json`,
  `reports/testnet-cobalt-full-replay-verify/packet-identity-v0-20260519T0924Z/testnet-cobalt-full-replay-verify.json`,
  `reports/testnet-cobalt-gate-selection/packet-identity-contract-v0-20260519T0924Z/testnet-cobalt-gate-selection-self-test.json`.

Previous completed slice:

- `COBALT-166`: promoted packet-name digest binding into named Cobalt gate
  mechanics checks. Controlled launch/readiness now emit
  `adversarial_packet_set_digest_bound_to_release` and
  `adversarial_packet_set_digest_bound_to_replay`, and the strict expected-fail
  wrapper requires those checks to remain true while topology/placement fail
  closed. Evidence:
  `reports/testnet-cobalt-controlled-readiness-gate/packet-digest-checks-v0-20260519T1115Z/testnet-cobalt-controlled-readiness-gate.json`,
  `reports/testnet-cobalt-controlled-launch-gate/packet-digest-checks-wrapper-self-test-v0-20260519T1115Z/testnet-cobalt-strict-launch-expected-fail-self-test.json`,
  `reports/testnet-cobalt-controlled-launch-gate/packet-digest-checks-strict-wrapper-v0-20260519T1115Z/testnet-cobalt-strict-launch-expected-fail.json`.

Previous completed slice:

- `COBALT-165`: added deterministic packet-name digests to the Cobalt
  adversarial packet binding summary. The controlled gates now emit
  `expected_packet_names_sha256`, `release_packet_names_sha256`, and
  `replay_packet_names_sha256` over the sorted packet-name lists. The strict
  expected-fail wrapper recomputes the expected digest and rejects malformed,
  stale, missing, or non-exact summaries before accepting the wrapper packet.
  Current 18-packet digest:
  `ea4db10775a378cf04daeb5671b23e75fbe88b7b17e25266b7bf7926075369bc`.
  Evidence:
  `reports/testnet-cobalt-controlled-readiness-gate/packet-binding-digest-v0-20260519T1045Z/testnet-cobalt-controlled-readiness-gate.json`,
  `reports/testnet-cobalt-controlled-launch-gate/packet-binding-digest-wrapper-self-test-v0-20260519T1045Z/testnet-cobalt-strict-launch-expected-fail-self-test.json`,
  `reports/testnet-cobalt-controlled-launch-gate/packet-binding-digest-strict-wrapper-v0-20260519T1045Z/testnet-cobalt-strict-launch-expected-fail.json`.

Earlier completed slice:

- `COBALT-164`: tightened the strict expected-fail wrapper around the Cobalt
  adversarial packet-map contract. The wrapper now validates that the nested
  strict-launch report exposes an exact and bound `adversarial_packet_set_binding`
  summary with no missing or extra packet names, copies that summary to the
  wrapper report, and self-tests stale/missing binding failures. Evidence:
  `reports/testnet-cobalt-controlled-launch-gate/strict-wrapper-binding-self-test-v0-20260519T1010Z/testnet-cobalt-strict-launch-expected-fail-self-test.json`,
  `reports/testnet-cobalt-controlled-launch-gate/strict-wrapper-binding-summary-v0-20260519T1010Z/testnet-cobalt-strict-launch-expected-fail.json`.

Earlier completed slice:

- `COBALT-163`: added an audit summary for adversarial packet-map binding to
  controlled Cobalt reports. The new `adversarial_packet_set_binding` object
  shows expected packet names, release/replay packet counts, missing/extra
  packet names, and exact/bound booleans at top level. Evidence:
  `reports/testnet-cobalt-controlled-readiness-gate/packet-binding-summary-v0-20260519T0935Z/testnet-cobalt-controlled-readiness-gate.json`,
  `reports/testnet-cobalt-controlled-launch-gate/packet-binding-summary-strict-expected-fail-v0-20260519T0935Z/testnet-cobalt-strict-launch-expected-fail.json`.

Earlier completed slice:

- `COBALT-162`: self-tested the exact adversarial packet-map predicate. The
  launch gate now exposes shared exact-map and path-binding helpers, controlled
  launch/readiness self-tests reject extra and missing packet names plus wrong
  paths, and the strict expected-fail wrapper rejects a synthetic stale packet
  map as a mechanics failure. Evidence:
  `reports/testnet-cobalt-controlled-readiness-gate/exact-packet-map-predicate-v0-20260519T0915Z/testnet-cobalt-controlled-readiness-gate.json`,
  `reports/testnet-cobalt-controlled-launch-gate/exact-packet-map-predicate-strict-expected-fail-v0-20260519T0915Z/testnet-cobalt-strict-launch-expected-fail.json`,
  `reports/testnet-cobalt-controlled-launch-gate/exact-packet-map-predicate-self-test-v0-20260519T0915Z/testnet-cobalt-strict-launch-expected-fail-self-test.json`.

Earlier completed slice:

- `COBALT-161`: tightened the controlled Cobalt packet-map contract. Release
  and replay must now report exactly the generated adversarial packet names,
  and the strict expected-fail wrapper requires those exact-map checks to remain
  true while strict topology/placement fail closed. Evidence:
  `reports/testnet-cobalt-controlled-readiness-gate/exact-packet-map-v0-20260519T0825Z/testnet-cobalt-controlled-readiness-gate.json`,
  `reports/testnet-cobalt-controlled-launch-gate/exact-packet-map-strict-expected-fail-v0-20260519T0825Z/testnet-cobalt-strict-launch-expected-fail.json`,
  `reports/testnet-cobalt-controlled-launch-gate/exact-packet-map-strict-self-test-v0-20260519T0825Z/testnet-cobalt-strict-launch-expected-fail-self-test.json`.

Earlier completed slice:

- `COBALT-160`: tightened controlled-readiness and strict-launch adversarial
  packet binding. Both controlled gates now require replay to report the same
  freshly generated adversarial harness and packet-set paths as release. The
  strict expected-fail wrapper includes those replay-binding checks in the
  mechanics that must remain true while strict topology/placement still fail
  closed. Evidence:
  `reports/testnet-cobalt-controlled-readiness-gate/replay-packet-binding-v0-20260519T0745Z/testnet-cobalt-controlled-readiness-gate.json`,
  `reports/testnet-cobalt-controlled-launch-gate/replay-packet-binding-strict-expected-fail-v0-20260519T0745Z/testnet-cobalt-strict-launch-expected-fail.json`,
  `reports/testnet-cobalt-controlled-launch-gate/replay-packet-binding-strict-self-test-v0-20260519T0745Z/testnet-cobalt-strict-launch-expected-fail-self-test.json`.

Earlier completed slice:

- `COBALT-159`: extended `scripts/testnet-cobalt-gate-selection-self-test` to
  compare the Cobalt packet generator contract against the verifier contract.
  The report now proves controlled launch and controlled readiness generate the
  same 17 adversarial subpackets, that those packet names/report filenames/env
  bindings match release/replay's verifier specs, and that the adversarial
  harness stays separately generated rather than duplicated as a subpacket.
  Evidence:
  `reports/testnet-cobalt-gate-selection/packet-generator-contract-v0-20260519T0730Z/testnet-cobalt-gate-selection-self-test.json`.

Previous completed slice:

- `COBALT-158`: hardened Cobalt gate-selection self-tests against release/replay
  packet-contract drift. The self-test now imports
  `scripts/testnet-cobalt-full-release-gate` and
  `scripts/testnet-cobalt-full-replay-verify`, verifies their adversarial
  packet specs and required-check contracts match exactly, and records the
  packet-count/required-check-count contract as JSON evidence. Evidence:
  `reports/testnet-cobalt-gate-selection/packet-contract-drift-v0-20260519T0715Z/testnet-cobalt-gate-selection-self-test.json`.

Previous completed slice:

- `COBALT-157`: tightened release and replay adversarial-packet validation.
  Each full-Cobalt adversarial packet now has a named required-check list, and
  release/replay reject missing required checks, any unexpected false check
  other than `outside_operators_required=false`, and the existing stale
  live-process packet shapes. Evidence:
  `reports/testnet-cobalt-gate-selection/adversarial-packet-required-checks-v0-20260519T0645Z/testnet-cobalt-gate-selection-self-test.json`,
  `reports/testnet-cobalt-controlled-readiness-gate/adversarial-packet-required-checks-v0-20260519T0645Z/testnet-cobalt-controlled-readiness-gate.json`.

Previous completed slice:

- `COBALT-156`: added `--self-test` to
  `scripts/testnet-cobalt-strict-launch-expected-fail` and wired that self-test
  into `scripts/check`. The self-test accepts a synthetic strict-launch
  fail-closed packet with exactly the known topology/placement blockers, then
  rejects packets with an unexpected Cobalt mechanics blocker, a missing
  expected blocker, a missing non-identical-trust-view mechanics check, or an
  accidentally successful strict launch. Evidence:
  `reports/testnet-cobalt-controlled-launch-gate/strict-expected-fail-self-test-v0-20260519T0631Z/testnet-cobalt-strict-launch-expected-fail-self-test.json`.

Previous completed slice:

- `COBALT-155`: added `scripts/testnet-cobalt-strict-launch-expected-fail` and
  wired it into `scripts/check`. The wrapper runs the strict independent
  topology launch gate, expects its fail-closed nonzero result, and then proves
  the observed blockers are exactly the known topology/placement blockers while
  the remote-drill mechanics, non-identical trust-view checks, adversarial
  packet requirements, release/replay report redaction, and topology binding
  checks remain green. Evidence:
  `reports/testnet-cobalt-controlled-launch-gate/strict-expected-fail-standard-check-v0-20260519T0613Z/testnet-cobalt-strict-launch-expected-fail.json`.

Previous completed slice:

- `COBALT-154`: wired the actual controlled-readiness gate into the standard
  check path. `scripts/check` now runs
  `scripts/testnet-cobalt-controlled-readiness-gate`, which selects the current
  mechanics-good remote full-Cobalt drill, regenerates controlled topology,
  adversarial harness, full adversarial packet set, release, and replay
  subreports, and requires them to pass and bind together with reused-machine
  topology explicitly allowed for controlled pre-testnet mechanics. Evidence:
  `reports/testnet-cobalt-controlled-readiness-gate/controlled-readiness-standard-check-v0-20260519T0600Z/testnet-cobalt-controlled-readiness-gate.json`.

Previous completed slice:

- `COBALT-153`: wired the actual DABC-aware live process-kill drill into the
  standard check path. `scripts/check` now runs
  `scripts/testnet-cobalt-live-process-kill` immediately after the predicate
  self-test, so local green checks exercise seven child validator processes,
  kill one delayed validator, prove the six survivors complete
  RBC/ABBA/MVBA/DABC/replay, respawn the killed validator, and prove restart
  replay. Evidence:
  `reports/testnet-cobalt-adversarial/live-process-kill-standard-check-v0-20260519T1305Z/testnet-cobalt-live-process-kill.json`,
  `reports/testnet-cobalt-adversarial/live-process-kill-self-test-standard-v0-20260519T1305Z/testnet-cobalt-live-process-kill-self-test.json`.

Previous completed slice:

- `COBALT-152`: added a live process-kill contract self-test to the standard
  check path. `scripts/testnet-cobalt-live-process-kill --self-test` now
  accepts a complete DABC-aware packet shape and rejects stale or incomplete
  evidence missing concurrency, MVBA selection, DABC ratification, or restart
  DABC replay checks. `scripts/check` runs this self-test before clippy/tests.
  Evidence:
  `reports/testnet-cobalt-adversarial/live-process-kill-self-test-v2-20260519T1240Z/testnet-cobalt-live-process-kill-self-test.json`,
  `reports/testnet-cobalt-adversarial/live-process-kill-v7-20260519T1240Z/testnet-cobalt-live-process-kill.json`.

Previous completed slice:

- `COBALT-151`: strengthened live process-kill and respawn evidence again. The
  example now starts seven actual local validator child processes for one RBC
  plus ABBA plus MVBA/DABC request, kills delayed `validator-6` before waiting
  for the round, proves the remaining six validators still accept the same RBC
  payload, finish the same ABBA value, select the same MVBA candidate, ratify
  the same DABC amendment, and verify the same DABC replay bundle under
  non-identical trust views, then respawns `validator-6` and proves the same
  DABC-aware path after restart. Release and replay now require the packet's
  concurrent-worker, ABBA-finish, MVBA-selection, DABC-ratification, and
  DABC-replay checks before accepting the adversarial packet set; the latest
  controlled-readiness packet passes with 18 adversarial packets bound.
  Evidence:
  `reports/testnet-cobalt-adversarial/live-process-kill-v7-20260519T1240Z/testnet-cobalt-live-process-kill.json`,
  `reports/testnet-cobalt-full-release-gate/full-cobalt-release-gate-v1-20260519T1205Z/testnet-cobalt-full-release-gate.json`,
  `reports/testnet-cobalt-full-replay-verify/full-cobalt-replay-v1-20260519T1205Z/testnet-cobalt-full-replay-verify.json`,
  `reports/testnet-cobalt-controlled-readiness-gate/controlled-readiness-v6-20260519T1205Z/testnet-cobalt-controlled-readiness-gate.json`.

Previous completed slice:

- `COBALT-150`: added `scripts/testnet-cobalt-controlled-readiness-gate`, a
  read-only controlled-testnet Cobalt gate that passes on the current
  seven-logical-validator reused-machine evidence without weakening the strict
  independent-topology launch gate. It selects the latest mechanics-passing
  full-Cobalt remote drill, regenerates controlled topology evidence with
  reused machines allowed, regenerates the adversarial harness and full packet
  set, and requires release plus replay to pass against those exact reports.
  Evidence:
  `reports/testnet-cobalt-controlled-readiness-gate/controlled-readiness-v6-20260519T1205Z/testnet-cobalt-controlled-readiness-gate.json`.

Previous completed slice:

- `COBALT-149`: `scripts/check` is green. The cleanup clippy-cleaned Cobalt
  adversarial examples, converted Cobalt rollback binding helpers to parameter
  structs, fixed mechanical node account-transaction clippy findings, and kept
  shielded transaction wire formats stable. Evidence:
  `reports/testnet-cobalt-check/full-check-clippy-clean-v0-20260519T0353Z/testnet-cobalt-check.json`.

Previous completed slice:

- `COBALT-148`: added self-test coverage for the full-Cobalt remote-drill
  selection policy. Release, replay, and controlled-launch self-tests now
  assert default mechanics-passing selection and explicit override behavior;
  the controlled-readiness self-test asserts reused-machine controlled topology
  mode remains separate from strict independent topology; replay also asserts
  release-bound selection. `scripts/check` runs those self-tests, and
  `scripts/testnet-cobalt-gate-selection-self-test` records the result as
  redacted JSON:
  `reports/testnet-cobalt-gate-selection/gate-selection-live-process-kill-v0-20260519T0942Z/testnet-cobalt-gate-selection-self-test.json`.

Previous completed slice:

- `COBALT-147`: release, replay, and strict controlled-launch gates now prefer
  the newest mechanics-passing full-Cobalt remote drill when no explicit
  `FULL_COBALT_REMOTE_DRILL_REPORT` is provided. The resolver no longer treats
  newer placement-preflight failure packets as the current Cobalt mechanics
  state. Standalone release/replay pass and select
  `full-cobalt-remote-v0-20260518T223730Z`; strict launch selects the same
  packet and still fails closed only on topology/placement:
  `reports/testnet-cobalt-full-release-gate/full-cobalt-mechanics-remote-selection-v0-20260519T0830Z/testnet-cobalt-full-release-gate.json`,
  `reports/testnet-cobalt-full-replay-verify/full-cobalt-mechanics-remote-selection-v0-20260519T0830Z/testnet-cobalt-full-replay-verify.json`,
  `reports/testnet-cobalt-controlled-launch-gate/controlled-launch-mechanics-remote-selection-v0-20260519T0830Z/testnet-cobalt-controlled-launch-gate.json`.

Previous completed slice:

- `COBALT-143`: added the long adversarial soak packet and fixed DABC
  long-chain extension. `ratify_dabc_amendment` now validates previous
  ratification core data without falsely requiring the previous amendment to be
  the genesis entry. The report proves 32 sequential governance ratifications,
  scheduled offline validators, duplicate/reordered delivery, deterministic
  restart replay, stale replay rejection, below-threshold ABBA equivocation
  handling, and final DABC replay verification:
  `reports/testnet-cobalt-adversarial/soak-v0-20260519T0715Z/testnet-cobalt-adversarial-soak.json`.

Previous completed slice:

- `COBALT-142`: added the parser/canonical-payload fuzz packet. It exercises
  RBC, ABBA, DABC, trust graph, DABC replay bundle, and trust graph transition
  artifacts. The report proves valid corpus roundtrips preserve canonical
  signing payloads, truncated JSON fails parsing, protocol-version type
  mutations fail parsing, tampered ids/bindings fail validation, and replay
  bundle / trust graph transition ids recompute from parsed payloads:
  `reports/testnet-cobalt-adversarial/parser-payload-fuzz-v0-20260519T0645Z/testnet-cobalt-parser-payload-fuzz.json`.

Previous completed slice:

- `COBALT-141`: added the governance-spam and amendment-flood packet.
  `MAX_MVBA_CANDIDATES_PER_SET` caps MVBA candidate sets at 1024 before
  sorting/deduping and during replay validation. The report proves many
  under-bound amendments select deterministically, while candidate floods, raw
  replay floods, duplicate amendment slots, future pending slots, and invalid
  parent chains fail closed:
  `reports/testnet-cobalt-adversarial/governance-spam-v0-20260519T0615Z/testnet-cobalt-governance-spam.json`.

Previous completed slice:

- `COBALT-140`: added the resource/verification DoS packet and signature-size
  bound. It proves oversized RBC/ABBA/DABC signatures, malformed payloads, DABC
  pending-pair floods, DABC checkpoint floods, RBC duplicate floods, and ABBA
  duplicate equivocations fail closed or dedupe:
  `reports/testnet-cobalt-adversarial/resource-dos-v0-20260519T0545Z/testnet-cobalt-resource-dos.json`.

Previous completed slice:

- `COBALT-139`: added the crash/restart persistence packet. It proves RBC
  restart replay idempotence, ABBA equivocation evidence preservation,
  MVBA/DABC replay verification after reload, graph activation revalidation,
  validator suspension binding persistence, rollback restoration, and stale
  DABC replay rejection after graph restart:
  `reports/testnet-cobalt-adversarial/crash-restart-v0-20260519T0525Z/testnet-cobalt-crash-restart.json`.

Previous completed slice:

- `COBALT-138`: added the partition and message-disorder simulation. It proves
  3/4 and 2/2/3 partitions do not create conflicting acceptance, single-validator
  isolation still allows six-validator progress, delay/reorder/duplicate
  delivery preserves support decisions, healed single-payload replay converges,
  and healed conflicting replay emits RBC conflict evidence:
  `reports/testnet-cobalt-adversarial/partition-simulation-v0-20260519T0505Z/testnet-cobalt-partition-simulation.json`.

Previous completed slice:

- `COBALT-137`: added the membership-race packet. It proves old-set blocks after
  activation, new-set blocks before activation, mixed old/new metadata, stale
  transaction-network ids, wrong graph roots, non-advancing activation heights,
  stale governance epochs, and stale DABC membership payloads fail closed:
  `reports/testnet-cobalt-adversarial/membership-race-v0-20260519T0445Z/testnet-cobalt-membership-race.json`.

Previous completed slice:

- `COBALT-136`: added the MVBA/DABC invalid-candidate packet. It proves
  invalid RBC accepts, conflicting candidate ids, stale propose-id/payload
  mismatches, duplicate raw candidates, bad output ids, conflicting parent
  hashes, skipped amendment slots, zero activation heights, and tampered
  activation evidence heights fail closed:
  `reports/testnet-cobalt-adversarial/dabc-invalid-candidates-v0-20260519T0425Z/testnet-cobalt-dabc-invalid-candidates.json`.

Previous completed slice:

- `COBALT-135`: added the ABBA Byzantine packet. It proves
  init/aux/conf/finish equivocation detection, withheld-support nontermination,
  invalid signature and bad-round rejection, conflicting finish evidence,
  live-mode deterministic coin rejection, and single-sender nontermination:
  `reports/testnet-cobalt-adversarial/abba-byzantine-v0-20260519T0410Z/testnet-cobalt-abba-byzantine.json`.

Previous completed slice:

- `COBALT-134`: added the RBC Byzantine packet. It proves
  double-propose/conflicting-accept detection, conflicting echo/ready/accept
  rejection, triggerless ready/accept denial, duplicate message dedupe, invalid
  signature rejection, and withheld-ready non-acceptance:
  `reports/testnet-cobalt-adversarial/rbc-byzantine-v0-20260519T0358Z/testnet-cobalt-rbc-byzantine.json`.

Previous completed slice:

- `COBALT-133`: added the stale replay rejection packet. It proves active G1
  rejects old G0 non-uniform certificates, proposals, linkage reports, registry
  roots, trust-view ids, and DABC replay bundles:
  `reports/testnet-cobalt-adversarial/stale-replay-v0-20260519T0345Z/testnet-cobalt-stale-replay.json`.

Previous completed slice:

- `COBALT-132`: added the trust-graph poison packet. It proves unsafe linkage,
  invalid subset parameters, duplicate validator scope, missing validator
  references, stale view versions, malformed trust-view signatures, and
  tampered lifecycle records all fail before activation:
  `reports/testnet-cobalt-adversarial/trust-graph-poison-v0-20260519T0330Z/testnet-cobalt-trust-graph-poison.json`.

Previous completed slice:

- `COBALT-131`: added the correlated capture model. It evaluates capture by
  host, operator, funding, jurisdiction, and injected capture sets. The report
  detects current reused-group risk, proves strict one-validator-per-group
  capture is safe for a single captured group, and detects the single
  funding-source failure where all validators become captured support:
  `reports/testnet-cobalt-adversarial/capture-model-v0-20260519T0320Z/testnet-cobalt-capture-model.json`.

Previous completed slice:

- `COBALT-130`: added the collusion threshold matrix. The report enumerates all
  128 captured-validator sets. Twelve capture sets are inside every active
  essential subset's fault bound; none produce unsafe graph linkage, captured
  strong support, or liveness loss. The first over-bound liveness loss appears
  at captured validators `validator-0, validator-1`; the first linkage break at
  `validator-0, validator-1, validator-2`; and the first captured strong
  support at `validator-0` through `validator-4`:
  `reports/testnet-cobalt-adversarial/collusion-threshold-v0-20260519T0308Z/testnet-cobalt-collusion-threshold.json`.

Previous completed slice:

- `COBALT-129`: release, replay, and strict controlled-launch gates now require
  the Cobalt adversarial harness when
  `REQUIRE_COBALT_ADVERSARIAL_HARNESS=1`. The strict gate generates the
  harness packet, passes that same report into release and replay, and records
  that both accepted it. Current strict evidence has adversarial checks green
  while topology and placement remain the expected strict blockers:
  `reports/testnet-cobalt-controlled-launch-gate/controlled-launch-adversarial-v0-20260519T0236Z/testnet-cobalt-controlled-launch-gate.json`.

Previous completed slice:

- `COBALT-128`: added `crates/consensus_cobalt/examples/cobalt_adversarial_harness.rs`
  and `scripts/testnet-cobalt-adversarial-harness`. The local report passes
  seven logical validators, seven distinct trust views, and eleven scenarios:
  honest accept, single withhold, colluding withhold, duplicate/reorder,
  invalid signature, malformed payload, stale root, RBC conflicting accept,
  ABBA same-sender equivocation, ABBA equivocal-sender exclusion, and
  crash/restart replay idempotence:
  `reports/testnet-cobalt-adversarial/adversarial-harness-v0-20260519T0228Z/testnet-cobalt-adversarial-harness.json`.

Previous completed slice:

- `COBALT-127`: added local self-tests for the Cobalt topology-remediation math
  and the strict controlled-launch topology summary. `scripts/check` now runs
  both `scripts/testnet-cobalt-topology-diversity-gate --self-test` and
  `scripts/testnet-cobalt-controlled-launch-gate --self-test`. Refreshed
  evidence after wiring those checks:
  `reports/testnet-cobalt-controlled-launch-gate/controlled-launch-topology-remediation-selftest-v0-20260519T0210Z/testnet-cobalt-controlled-launch-gate.json`.

Previous completed slice:

- `COBALT-126`: topology-diversity now emits redacted strict
  independent-topology remediation, and the strict controlled-launch gate lifts
  it to the top-level report. Current evidence says seven validators are
  present, but only three host/operator-host fingerprints are present; four
  additional independent host/operator-host fingerprints are required, and four
  validator slots must move off reused hosts:
  `reports/testnet-cobalt-controlled-launch-gate/controlled-launch-topology-remediation-v0-20260519T0203Z/testnet-cobalt-controlled-launch-gate.json`.

Previous completed slice:

- `COBALT-125`: the strict controlled-launch gate now pins one full-Cobalt
  remote drill packet, generates a topology-diversity subreport from it, and
  feeds that exact topology report into release and replay. Current evidence
  has topology binding true at the strict, release, and replay levels, while
  still failing closed on independent topology and placement:
  `reports/testnet-cobalt-controlled-launch-gate/controlled-launch-autotopology-v0-20260519T0150Z/testnet-cobalt-controlled-launch-gate.json`.

Previous completed slice:

- `COBALT-124`: release and replay now require topology-diversity evidence to
  be bound to the same full-Cobalt remote drill packet when topology diversity
  is required. The strict controlled-launch gate exposes the same check at the
  top level. Current evidence fails closed because the selected topology report
  points at an older remote packet than the current release/replay evidence:
  `reports/testnet-cobalt-controlled-launch-gate/controlled-launch-topology-bound-v0-20260519T0134Z/testnet-cobalt-controlled-launch-gate.json`.

Previous completed slice:

- `COBALT-123`: release, replay, and strict controlled-launch gates now expose
  explicit Cobalt trust-view launch checks. The latest strict report shows
  minimum three trust views required, seven G1 trust views observed,
  non-identical G1 trust views true, and RBC distinct trust views seven, while
  still failing closed on topology and placement:
  `reports/testnet-cobalt-controlled-launch-gate/controlled-launch-trustviews-v0-20260519T0126Z/testnet-cobalt-controlled-launch-gate.json`.

Previous completed slice:

- `COBALT-122`: the strict Cobalt controlled-launch gate now verifies that the
  release and replay subreports it composes are redacted before the launch gate
  can pass. Current evidence has `release_gate_report_redacted = true` and
  `replay_report_redacted = true`, while still failing closed on topology and
  placement blockers:
  `reports/testnet-cobalt-controlled-launch-gate/controlled-launch-strict-redacted-v0-20260519T0117Z/testnet-cobalt-controlled-launch-gate.json`.

Previous completed slice:

- `COBALT-121`: added `scripts/testnet-cobalt-controlled-launch-gate`, a
  read-only strict launch gate for full Cobalt. It composes the release and
  replay gates with topology diversity required, placement preflight required,
  reused-machine topology disallowed, and placement evidence required to be
  bound to the remote drill. Current evidence fails closed with explicit
  topology and placement blockers:
  `reports/testnet-cobalt-controlled-launch-gate/controlled-launch-strict-v1-20260519T0112Z/testnet-cobalt-controlled-launch-gate.json`.

Previous completed slice:

- `COBALT-120`: `scripts/check` now covers the Cobalt Python gate path. It
  py-compiles the full-Cobalt remote drill, release gate, replay verifier,
  placement-manifest draft, placement preflight, and topology-diversity gate,
  and runs `scripts/testnet-cobalt-placement-manifest-draft --self-test`.
  Focused checks passed locally.

Previous completed slice:

- `COBALT-119`: release and replay gates now bind placement-preflight
  satisfaction to the remote drill packet. The reports expose
  `placement_preflight.source` and `placement_preflight.bound_to_remote_drill`;
  when placement preflight is required, standalone preflight reports remain
  diagnostic only and cannot satisfy the launch gate unless the remote drill ran
  with the requirement enabled. Evidence:
  `reports/testnet-cobalt-full-release-gate/full-cobalt-placement-bound-v0-20260519T0052Z/testnet-cobalt-full-release-gate.json`
  and
  `reports/testnet-cobalt-full-replay-verify/full-cobalt-placement-bound-v0-20260519T0052Z/testnet-cobalt-full-replay-verify.json`.

Previous completed slice:

- `COBALT-118`: release and replay gates now surface the actionable placement
  packet from the Cobalt placement preflight. The top-level reports include
  placement-manifest source, whether a generated draft was available, the
  emitted public-diversity overlay-template path, missing minimum/no-blocking
  and strict-independent deltas, required operator inputs, and rerun commands.
  A refreshed full-Cobalt remote drill blocks before mutation as expected:
  `reports/testnet-cobalt-full-nonuniform-remote-drill/full-cobalt-placement-packet-v0-20260519T0041Z/testnet-cobalt-full-nonuniform-remote-drill.json`.
  Release and replay carry the same placement packet:
  `reports/testnet-cobalt-full-release-gate/full-cobalt-placement-packet-v1-20260519T0041Z/testnet-cobalt-full-release-gate.json`
  and
  `reports/testnet-cobalt-full-replay-verify/full-cobalt-placement-packet-v1-20260519T0041Z/testnet-cobalt-full-replay-verify.json`.

Previous completed slice:

- `COBALT-117`: `scripts/testnet-cobalt-placement-preflight` now requests the
  public-diversity overlay template from the draft subreport by default. The
  preflight packet includes the template path in `inputs` and summarizes
  template status in `placement_manifest_draft`, so a blocked placement run
  carries the operator-fillable overlay JSON directly. Current 7-validator
  evidence still blocks on missing machines/groups but includes a redacted
  three-row template:
  `reports/testnet-cobalt-placement-preflight/preflight-template-integrated-v0-20260519T0029Z/testnet-cobalt-placement-preflight.json`
  and
  `reports/testnet-cobalt-placement-preflight/preflight-template-integrated-v0-20260519T0029Z/cobalt-placement-diversity-overlay-template.json`.
  Functional `VALIDATORS=3` evidence proves generated-draft selection, manifest
  verification, placement capacity, and template emission together:
  `reports/testnet-cobalt-placement-preflight/preflight-template-integrated-functional-v0-20260519T0029Z/testnet-cobalt-placement-preflight.json`.

Previous completed slice:

- `COBALT-116`: `scripts/testnet-cobalt-placement-preflight` now selects an
  effective placement manifest after running the draft subreport. Explicit
  manifests still win. Without an explicit manifest, a generated redacted draft
  is automatically used when available; otherwise the preflight falls back to
  the current default manifest. This removes the manual handoff between
  manifest drafting and the Cobalt placement gate once enough machines are
  present. Current 7-validator evidence still blocks and falls back to the
  default manifest because no seven-target draft exists:
  `reports/testnet-cobalt-placement-preflight/preflight-auto-draft-v0-20260519T0019Z/testnet-cobalt-placement-preflight.json`.
  Functional evidence with `VALIDATORS=3` proves `placement_manifest_source =
  generated_draft`, manifest verification, and placement capacity on a generated
  draft:
  `reports/testnet-cobalt-placement-preflight/preflight-auto-draft-functional-v0-20260519T0019Z/testnet-cobalt-placement-preflight.json`.

Previous completed slice:

- `COBALT-115`: `scripts/testnet-cobalt-placement-manifest-draft` can now emit
  an operator-fillable public-diversity overlay template with
  `WRITE_DIVERSITY_OVERLAY_TEMPLATE=1`. The template contains only credential
  `machine_index` values and null public-diversity fields, plus the overlay
  schema expected by `COBALT_DIVERSITY_OVERLAY`. Current evidence writes a
  redacted three-row template for the three complete credential entries and
  still blocks on the missing four machine slots and missing independent group.
  Evidence:
  `reports/testnet-cobalt-placement-manifest-draft/diversity-template-v0-20260519T0016Z/testnet-cobalt-placement-manifest-draft.json`
  and
  `reports/testnet-cobalt-placement-manifest-draft/diversity-template-v0-20260519T0016Z/cobalt-placement-diversity-overlay-template.json`.

Previous completed slice:

- `COBALT-114`: `scripts/testnet-cobalt-placement-manifest-draft` now supports
  a sanitized `COBALT_DIVERSITY_OVERLAY` JSON file with schema
  `postfiat-testnet-cobalt-placement-diversity-overlay-v1`. The overlay is
  keyed by `machine_index` and can only provide public-diversity fields:
  cloud provider, region, jurisdiction, legal domain, and funding source. The
  script rejects sensitive-shaped labels and duplicate machine indexes, while
  leaving host/operator/operator-host labels credential-derived. The draft
  self-test now includes a seven-machine public-diversity pass, and
  `scripts/testnet-cobalt-placement-preflight` passes the overlay through to
  the draft subreport. Current live evidence still blocks because the inventory
  has three complete credential entries/groups. Evidence:
  `reports/testnet-cobalt-placement-manifest-draft/draft-overlay-ready-v0-20260519T0009Z/testnet-cobalt-placement-manifest-draft.json`,
  `reports/testnet-cobalt-placement-manifest-draft/draft-overlay-public-missing-v0-20260519T0009Z/testnet-cobalt-placement-manifest-draft.json`,
  `reports/testnet-cobalt-placement-preflight/preflight-overlay-ready-v0-20260519T0009Z/testnet-cobalt-placement-preflight.json`,
  and
  `reports/testnet-cobalt-placement-preflight/preflight-overlay-public-missing-v0-20260519T0009Z/testnet-cobalt-placement-preflight.json`.

Previous completed slice:

- `COBALT-113`: `scripts/testnet-cobalt-placement-preflight` now invokes the
  placement-manifest draft tool as a subreport and includes a summarized
  `placement_manifest_draft` section in the top-level report. That means the
  release/drill placement gate now carries the exact draft readiness blocker:
  three complete credential entries/groups, four missing manifest slots, one
  missing independent group for the minimum no-blocking profile, and four
  missing groups for strict one-validator-per-independent-host evidence.
  Evidence:
  `reports/testnet-cobalt-placement-preflight/preflight-draft-integrated-v0-20260518T2350Z/testnet-cobalt-placement-preflight.json`
  and
  `reports/testnet-cobalt-placement-preflight/preflight-draft-integrated-public-v0-20260518T2350Z/testnet-cobalt-placement-preflight.json`.

Previous completed slice:

- `COBALT-112`: `scripts/testnet-cobalt-placement-manifest-draft` now gives the
  Cobalt lane a deterministic local handoff from credentials to a placement
  manifest. It records no host/IP/password/key material, uses abstract
  host/operator/operator-host labels, emits a manifest only when enough complete
  credential targets exist, and otherwise fails closed with exact deltas. The
  current evidence remains blocked as expected: three complete credential
  entries/groups, four missing manifest slots, one missing independent group
  for the minimum no-blocking profile, and four missing groups for strict
  one-validator-per-independent-host evidence. Evidence:
  `reports/testnet-cobalt-placement-manifest-draft/draft-v0-20260518T234940Z/testnet-cobalt-placement-manifest-draft.json`
  and
  `reports/testnet-cobalt-placement-manifest-draft/draft-public-v0-20260518T234940Z/testnet-cobalt-placement-manifest-draft.json`.

Previous completed slice:

- `COBALT-111`: the placement-manifest verifier now writes a failed report for
  ordinary manifest, diversity, and source-evidence failures. The Cobalt
  preflight now links that report and blocks on
  `placement_manifest_verify_report_not_ok`, rather than losing the diagnostic
  as a missing artifact. Evidence:
  `reports/testnet-cobalt-placement-preflight/preflight-v0-20260518T233742Z/testnet-cobalt-placement-preflight.json`,
  `reports/testnet-cobalt-placement-preflight/preflight-v0-20260518T233747Z/testnet-cobalt-placement-preflight.json`,
  and
  `reports/testnet-cobalt-placement-preflight/preflight-v0-20260518T233801Z/testnet-cobalt-placement-preflight.json`.

Previous completed slice:

- `COBALT-110`: failed placement preflight reports now carry the remediation
  delta the operator needs. Current default evidence says the minimum
  no-single-group-can-block path needs four total complete credential slots for
  a seven-target manifest, one additional independent host/operator/operator-host
  group, two more manifest targets, and controlled/public labels as required.
  Strict independent evidence separately records four missing independent
  host/operator/operator-host groups. Evidence:
  `reports/testnet-cobalt-placement-preflight/preflight-v0-20260518T233348Z/testnet-cobalt-placement-preflight.json`,
  `reports/testnet-cobalt-placement-preflight/preflight-v0-20260518T233351Z/testnet-cobalt-placement-preflight.json`,
  and
  `reports/testnet-cobalt-placement-preflight/preflight-v0-20260518T233339Z/testnet-cobalt-placement-preflight.json`.

Previous completed slice:

- `COBALT-109`: release and replay now enforce the placement-preflight
  requirement when `REQUIRE_COBALT_PLACEMENT_PREFLIGHT=1` is set. Both reports
  expose the placement preflight status, validator count, and blockers. Current
  fail-closed evidence:
  `reports/testnet-cobalt-full-release-gate/full-cobalt-placement-preflight-required-fail-v0-20260518T2330Z/testnet-cobalt-full-release-gate.json`
  and
  `reports/testnet-cobalt-full-replay-verify/full-cobalt-placement-preflight-required-fail-v0-20260518T2330Z/testnet-cobalt-full-replay-verify.json`.

Previous completed slice:

- `COBALT-108`: the mutating full-Cobalt remote drill can now require the
  placement preflight with `REQUIRE_COBALT_PLACEMENT_PREFLIGHT=1`. In that mode
  the drill checks placement before the remote validator-registry mutation path
  can run. Current non-mutating evidence blocks on the incomplete placement and
  records no remote validator-registry report:
  `reports/testnet-cobalt-full-nonuniform-remote-drill/full-cobalt-placement-preflight-required-fail-v0-20260518T2324Z/testnet-cobalt-full-nonuniform-remote-drill.json`.

Previous completed slice:

- `COBALT-107`: a single local Cobalt placement preflight now gates the next
  remote placement attempt. The command checks the proposed manifest, current
  credential capacity, and topology-diversity evidence without SSH or remote
  mutation. Current evidence blocks for the expected reasons: the manifest has
  five targets rather than seven, the credential inventory has three complete
  machine/operator-host groups while the threshold profile needs four, and the
  current remote topology has three host fingerprints. Evidence:
  `reports/testnet-cobalt-placement-preflight/preflight-v0-20260518T232134Z/testnet-cobalt-placement-preflight.json`.

Previous completed slice:

- `COBALT-106`: current credential capacity is now captured as Cobalt evidence.
  For seven validators, a 5-vote quorum means any group with three validators
  can block progress. The current credential inventory has three complete
  machine/operator-host groups; the minimum no-single-group-can-block profile
  needs four groups, and strict one-host-per-validator evidence needs seven.

Previous completed slice:

- `COBALT-105`: topology diversity now includes optional placement-manifest
  requirements. The Cobalt topology report can require host, operator,
  operator-host, cloud-provider, region, jurisdiction, legal-domain, and
  funding-source group labels while recording only hashed labels and counts.
  The current controlled placement manifest fails this Cobalt gate because it
  covers five targets rather than seven Cobalt validators and lacks
  public-diversity labels.

Previous completed slice:

- `COBALT-104`: topology diversity is now a first-class evidence gate. The
  independent topology run fails closed because the current seven validator
  slots map to three host fingerprints with a max of three validators per host
  fingerprint. Reused-machine evidence can still be accepted deliberately by
  running the gate with `COBALT_TOPOLOGY_ALLOW_REUSED_MACHINES=1` and an
  appropriate `COBALT_TOPOLOGY_MIN_HOSTS`. Release and replay expose the
  topology gate and can fail closed with `REQUIRE_COBALT_TOPOLOGY_DIVERSITY=1`.

Previous completed slice:

- `COBALT-099`: post-suspend active-validator outage evidence is now a
  first-class full-Cobalt gate when required. The latest run starts from a clean
  credential-aligned 7-validator deployment, ratifies a Cobalt suspension to 6
  active validators, stops one remaining active validator, proves the remaining
  5 online active validators order the next block, restarts/replays the stopped
  validator, and passes release plus offline replay with
  `post_suspend_fault_tolerance_required=true` and
  `post_suspend_fault_tolerance_ok=true`.

Previous remote refresh slice:

- `COBALT-098`: a fresh credential-aligned 7-validator reused-machine plan was
  generated from the current credential inventory, deployed, and exercised
  through the mutating full-Cobalt remote drill. The latest full remote drill,
  release gate, and offline replay all pass.

Previous realignment slice:

- `COBALT-097`: the new redacted realignment dry-run proves a
  credential-aligned 7-validator deploy-plan shape is available without
  committing raw hosts, IPs, passwords, keys, or the raw deploy plan. The full
  remote drill now links to this report when credential preflight blocks on a
  stale plan.

Previous credential candidate slice:

- `COBALT-096`: credential preflight now emits a redacted realignment candidate.
  The evidence showed a credential-aligned 7-validator reused-machine candidate
  was available across three credential host fingerprints while the then-latest
  deploy plan was stale.

Previous credential preflight slice:

- `COBALT-095`: full-Cobalt remote evidence now has a dedicated redacted
  credential/deploy-plan preflight. The first preflight parsed the credential
  inventory, verified redaction, and blocked because none of the 7 then-current
  deploy-plan validator hosts matched the available credential hosts. The full
  remote drill stops before mutation when this preflight fails; release and
  replay expose `remote_credential_preflight_ok` directly.

Previous remote evidence slice:

- `COBALT-094`: remote full-Cobalt evidence is now self-contained. The wrapper
  derives the current G1 trust graph root from the Cobalt crate, requires the
  RBC TCP-aware local harness, and records fail-closed credential blockers. The
  original refreshed run did not mutate validators because credentials did not
  match the deploy-plan host fingerprint for `validator-0`.

Previous replay slice:

- `COBALT-093`: offline replay now explicitly verifies the release gate's RBC
  TCP transport check, the local harness RBC TCP check, and the underlying RBC
  TCP evidence packet. Replay evidence now includes the local RBC TCP report
  path in `verified_evidence.local_rbc_tcp_transport`.

Previous gate slice:

- `COBALT-090` / `COBALT-092`: the local full-Cobalt harness and release gate
  now require passing RBC loopback TCP transport evidence. The release gate uses
  the freshest local harness by default, so an older remote drill cannot pin the
  gate to stale local RBC evidence.

Previous completed slice:

- `COBALT-043`: RBC loopback TCP transport drill is implemented. The example
  starts seven local TCP validator workers, serializes one RBC
  propose/echo/ready/accept bundle over sockets, has each worker evaluate its
  own non-identical trust view, proves all workers accept the same payload, and
  proves same-payload accepts do not create conflict evidence. This is network
  transport evidence on a reused machine, not independent operator topology.

Previous transaction-network slice:

- `COBALT-082`: transaction-network replacement drill is implemented. DABC now
  binds the replacement membership payload, old-network block membership fails
  after the replacement activation height, and replacement-network block
  membership validates at activation.

Previous rollback slice:

- `COBALT-073`: trust graph rollback path is implemented. A bad graph with
  unsafe linkage evidence can be rolled back to the prior authority graph's
  trust views through a deterministic rollback graph, rollback record, replay
  validation, and DABC payload binding.

Previous RBC simulation slice:

- `COBALT-043` local simulation: RBC now has a deterministic non-identical
  trust-view drill. The test builds seven validators, at least three distinct
  local trust views, drives echo/ready/accept for one payload, proves every
  local view can accept that payload, and proves same-payload accepts do not
  create conflict evidence. The local full-Cobalt harness includes this RBC
  packet.

Previous ABBA slice:

- `COBALT-053`: ABBA same-sender equivocation evidence is implemented. A
  validator sending both `true` and `false` for one ABBA round now creates
  deterministic evidence across init/aux/conf/finish messages, and the
  regression test proves the equivocal sender is excluded from local support.
  The local full-Cobalt harness includes this ABBA evidence packet.

Previous remote/release slice:

- `COBALT-091` through `COBALT-093`: seven logical validators were deployed
  across reused machines, the full-Cobalt remote drill passed, the release gate
  passed with current G1 root, and offline replay verification passed.

Current blocker:

- No P0 code blocker for full-Cobalt mechanics. The remaining strict topology
  caveat is machine-checkable: independent topology fails because the current
  seven logical validators use only three host fingerprints. This is a public
  topology evidence gap, not a controlled-testnet Cobalt code blocker.
- Strict launch evidence now generates the topology report from its selected
  remote drill packet, then release/replay consume that exact report. The
  current blockers are independent topology and placement, not stale topology
  evidence.
- Default release/replay/strict-launch selection now prefers the newest
  mechanics-passing full-Cobalt remote drill. The current mechanics-good remote
  packet is
  `reports/testnet-cobalt-full-nonuniform-remote-drill/full-cobalt-remote-v0-20260518T223730Z/testnet-cobalt-full-nonuniform-remote-drill.json`;
  later placement-preflight failure packets remain diagnostics, not the default
  mechanics source.
- The current placement manifest is not enough for seven-validator Cobalt
  topology evidence. It is five-validator controlled-testnet placement evidence
  and has no public-diversity labels.
- Current credentials are one independent machine/operator-host group short of
  the minimum seven-validator capture-threshold profile and four groups short
  of strict one-host-per-validator evidence.
- All listed adversarial Cobalt packets are now implemented through live
  process-kill and respawn. Strict release/replay/controlled-launch gates now
  consume the full packet set when `REQUIRE_COBALT_ADVERSARIAL_PACKET_SET=1` is
  enabled.
- Controlled-testnet Cobalt readiness is green through
  `scripts/testnet-cobalt-controlled-readiness-gate`. That gate permits reused
  machines for the current engineering phase, requires release/replay/topology
  and the adversarial packet set to bind to one mechanics-good remote drill,
  and keeps strict independent topology as a recorded caveat.

Next start:

1. Use `scripts/testnet-cobalt-controlled-readiness-gate` when the question is
   whether full-Cobalt mechanics are ready for controlled testnet on
   project-controlled machines.
2. Convert reused-machine Cobalt evidence into stronger placement evidence when
   more independent machines/operators are available. The current full-Cobalt
   mechanics are green on seven logical validators, but not seven independent
   operators.
3. Run `scripts/testnet-cobalt-placement-manifest-draft` after more credential
   entries are added. When it emits a draft, use that draft as
   `COBALT_PLACEMENT_MANIFEST` and run
   `COBALT_TOPOLOGY_REQUIRE_PLACEMENT_MANIFEST=1` and, for public-diversity
   evidence, `COBALT_TOPOLOGY_REQUIRE_PUBLIC_DIVERSITY=1`.
4. Keep `COBALT-103` out of the implementation lane until explicitly asked;
   it is public-doc language, not a full-Cobalt mechanics blocker.

## Evidence Names

Use these report directories:

```text
reports/testnet-cobalt-trust-graph-smoke/
reports/testnet-cobalt-linkedness-checker/
reports/testnet-cobalt-nonuniform-certificate/
reports/testnet-cobalt-rbc-nonuniform/
reports/testnet-cobalt-abba-nonuniform/
reports/testnet-cobalt-dabc-nonuniform/
reports/testnet-cobalt-trust-graph-transition/
reports/testnet-cobalt-cutover-mode/
reports/testnet-cobalt-g0/
reports/testnet-cobalt-g1/
reports/testnet-cobalt-remote-credential-preflight/
reports/testnet-cobalt-remote-plan-realignment/
reports/testnet-cobalt-remote-bootstrap-smoke/
reports/testnet-cobalt-full-nonuniform-remote-drill/
reports/testnet-cobalt-topology-diversity/
reports/testnet-cobalt-placement-capacity/
reports/testnet-cobalt-placement-manifest-draft/
reports/testnet-cobalt-placement-preflight/
reports/testnet-cobalt-adversarial/
reports/testnet-cobalt-full-release-gate/
reports/testnet-cobalt-full-replay-verify/
reports/testnet-cobalt-controlled-launch-gate/
reports/testnet-cobalt-controlled-readiness-gate/
```

## Launch Gate

Full Cobalt is launch-green only when the release status contains:

```text
cobalt_mode = non_uniform
trust_graph_root = <current-root>
trust_view_count >= 3
non_identical_trust_views = true
linkedness_report_ok = true
unsafe_graph_rejection_ok = true
nonuniform_certificate_ok = true
rbc_nonuniform_ok = true
rbc_tcp_transport_ok = true
abba_nonuniform_ok = true
dabc_replay_ok = true
trust_graph_transition_ok = true
remote_nonuniform_drill_ok = true
post_change_finality_ok = true
cobalt_adversarial_harness_requirement_satisfied = true
offline_replay_ok = true
```

The current green gate for full-Cobalt mechanics on reused-machine remote
evidence is
`reports/testnet-cobalt-full-release-gate/full-cobalt-mechanics-remote-selection-v0-20260519T0830Z/testnet-cobalt-full-release-gate.json`
with replay at
`reports/testnet-cobalt-full-replay-verify/full-cobalt-mechanics-remote-selection-v0-20260519T0830Z/testnet-cobalt-full-replay-verify.json`.

Those packets select the newest mechanics-passing full-Cobalt remote drill by
default:
`reports/testnet-cobalt-full-nonuniform-remote-drill/full-cobalt-remote-v0-20260518T223730Z/testnet-cobalt-full-nonuniform-remote-drill.json`.

The current strict controlled-launch expected-fail packet proves the Cobalt
adversarial harness is generated, redacted, bound into release, and accepted by
replay while the strict gate still fails closed only on topology and placement:
`reports/testnet-cobalt-controlled-launch-gate/strict-expected-fail-standard-check-v0-20260519T0613Z/testnet-cobalt-strict-launch-expected-fail.json`.

The current controlled-testnet readiness packet passes with reused-machine
topology explicitly allowed, release/replay green, and the full adversarial
packet set bound:
`reports/testnet-cobalt-controlled-readiness-gate/controlled-readiness-standard-check-v0-20260519T0600Z/testnet-cobalt-controlled-readiness-gate.json`.
It records the strict independent-topology caveat separately: current evidence
has seven validators across three host/operator-host fingerprints, so four
validator slots would need to move off reused hosts for the strict gate.

Independent topology is a separate launch-quality gate. The current independent
topology run fails closed at
`reports/testnet-cobalt-topology-diversity/topology-independent-v0-20260518T2307Z/testnet-cobalt-topology-diversity-gate.json`
because seven validators map to three host fingerprints. When
`REQUIRE_COBALT_TOPOLOGY_DIVERSITY=1`, release and replay fail closed unless
the topology report satisfies independent topology, as shown by
`reports/testnet-cobalt-full-release-gate/full-cobalt-release-gate-topology-required-fail-v0-20260518T2308Z/testnet-cobalt-full-release-gate.json`
and
`reports/testnet-cobalt-full-replay-verify/full-cobalt-replay-topology-required-fail-v0-20260518T2308Z/testnet-cobalt-full-replay-verify.json`.

Placement-manifest diversity is also available as a Cobalt topology gate. The
current controlled placement manifest fails the seven-validator Cobalt
placement requirement at
`reports/testnet-cobalt-topology-diversity/topology-reused-placement-required-fail-v0-20260518T2307Z/testnet-cobalt-topology-diversity-gate.json`;
release/replay surface that failure at
`reports/testnet-cobalt-full-release-gate/full-cobalt-release-gate-placement-required-fail-v0-20260518T2308Z/testnet-cobalt-full-release-gate.json`
and
`reports/testnet-cobalt-full-replay-verify/full-cobalt-replay-placement-required-fail-v0-20260518T2308Z/testnet-cobalt-full-replay-verify.json`.

The current credential capacity report is
`reports/testnet-cobalt-placement-capacity/testnet-remote-placement-capacity-profile-20260518T231145Z.json`.
It shows three complete machine/operator-host groups against a four-group
minimum for no-single-group-can-block-quorum placement at seven validators.
