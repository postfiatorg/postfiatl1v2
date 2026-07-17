# Controlled Testnet Review Packet

Status: external reviewer command sheet
Date: 2026-05-14
Controlled-launch candidate: `56db87a1f6f5be0dfe936c4619931aaefbbeffb5`
Current controlled-launch evidence pack:
`reports/testnet-controlled-launch-evidence-pack/head-56db87a-optimized-latency/testnet-controlled-launch-evidence-pack.json`.
Current live hardening evidence pack:
`reports/testnet-live-hardening-evidence-pack/current-20260514T-post-host-group-outage/testnet-live-hardening-evidence-pack.json`.

The launch candidate is the optimized `56db87a` package/release-gate control
point. Later docs, scripts, and live-hardening commits can wrap that candidate
without changing the release package revision. Confirm the repository head with
`git rev-parse HEAD` before running the commands below.

This packet is for consensus, cryptography, RPC, and operator reviewers. It
describes the controlled-testnet evidence surface, not a mainnet or public
decentralization claim.

## Scope To Review

The controlled-testnet launch claim is narrow:

- transparent post-quantum settlement with ML-DSA-style account and validator
  authorization;
- a known 5-validator controlled cohort;
- finality evidence through block certificates, receipts, wallet finality RPC,
  and release gates;
- Cobalt-derived validator governance in canonical-UNL mode;
- release package, operator launch packet, 8-hour soak, remote P0 gate, and
  redaction evidence;
- live post-launch hardening evidence: 100-round continuity, restart, one-node
  outage, below-quorum outage, host-group capture-threshold outage, RPC
  read-load, mixed read/write load, observability, and write-edge policy;
- honest exclusion of production privacy, external bridge custody, broad public
  decentralization, and TPS claims.

## Reproduce The Current Evidence

From the repository root:

```sh
git fetch origin
git checkout main
git pull --ff-only
git rev-parse HEAD
```

Run the full local code and smoke check:

```sh
scripts/check
```

Run the public-claims verifier:

```sh
REQUIRE_EVIDENCE=1 scripts/testnet-public-claims-check
```

Regenerate the controlled-launch evidence pack from committed head:

```sh
ROOT_DIR=reports/testnet-controlled-launch-evidence-pack/reviewer-current \
REPORT=reports/testnet-controlled-launch-evidence-pack/reviewer-current/testnet-controlled-launch-evidence-pack.json \
P0_NETWORK_GATE_REPORT=reports/testnet-p0-network-gate-remote-head-c18d590-sdk-signer/testnet-p0-network-gate-20260514T015505Z.json \
RUN_SUBPACKS=1 \
REQUIRE_CLEAN_GIT=1 \
scripts/testnet-controlled-launch-evidence-pack
```

Expected top-level checks:

```sh
jq '{status, ok: .controlled_launch_evidence_pack_ok, git, candidate, checks}' \
  reports/testnet-controlled-launch-evidence-pack/reviewer-current/testnet-controlled-launch-evidence-pack.json
```

The generated report should have `status=passed`,
`controlled_launch_evidence_pack_ok=true`, `git.dirty=false`, and all top-level
checks true. The selected P0 path should be the SDK-signer remote P0 report, and
`checks.sdk_signer_remote_p0_ok`, `checks.benchmark_uses_selected_p0`,
`checks.cobalt_audit_uses_selected_p0`, and
`checks.registry_audit_uses_selected_p0` should all be true. The launch
candidate inside the pack intentionally remains
`56db87a1f6f5be0dfe936c4619931aaefbbeffb5`; later evidence/docs commits can
wrap that candidate without changing the candidate revision.

Inspect the current live hardening pack:

```sh
jq '{status, ok: .live_hardening_evidence_pack_ok, git, config, summary, checks}' \
  reports/testnet-live-hardening-evidence-pack/current-20260514T-post-host-group-outage/testnet-live-hardening-evidence-pack.json
```

Expected: `status=passed`, `live_hardening_evidence_pack_ok=true`,
`git.dirty=false`, at least 17 evidence entries, minimum read requests `1200`,
minimum mixed read requests `600`, max continuity rounds at least `100`, and
max final height at least `165`.

## Evidence Map

Start with:

- `docs/status/chain-state-current.md`
- `docs/status/controlled-testnet-burndown.md`
- `docs/status/public-claims-checklist.md`
- `scripts/testnet-controlled-launch-evidence-pack`

Then inspect component reports from the evidence pack:

- final candidate report;
- operator launch packet;
- release status report;
- selected remote P0 report;
- benchmark evidence pack;
- Cobalt lifecycle audit;
- registry-root binding audit;
- Cobalt amendment lifecycle smoke;
- live hardening evidence pack;
- live host-group outage report;
- public claims check log.

The evidence pack records the exact path and SHA-256 for each item under its
`evidence` array.

## Review Questions

Consensus and finality:

- Do certificate ids, block hashes, receipt ids, and tx finality responses bind
  the intended chain id, genesis hash, protocol version, validator registry
  root, and quorum evidence?
- Are stale votes, split validator sets, failed leaders, partitions, restart,
  and catch-up handled without admitting conflicting finality?
- Does the host-group outage evidence correctly show that the current controlled
  topology has a two-validator operator-host group that can block quorum, cannot
  form quorum alone, and cannot advance state while below quorum?
- Are public RPC finality responses sufficient for a wallet or exchange to
  reject malformed or mismatched proofs?

Cobalt canonical governance:

- Is "Cobalt-derived governance in canonical-UNL mode" the correct claim for
  the code and evidence?
- Are operator manifests, genesis governance bundle, registry-root-bound
  certificates, lifecycle updates, emergency rotation, and replay artifacts
  enough for controlled testnet?
- What must be added before marketing can safely claim broader Cobalt trust
  evolution or non-uniform trust views?

Post-quantum cryptography:

- Are ML-DSA signature bytes, public key bytes, transaction envelopes,
  certificate votes, and domain labels represented without ambiguity?
- Which key lifecycle, KAT, dependency-inventory, and external audit prep gaps
  should block broader public testnet?
- Are certificate-size and RPC-payload costs described without implying lattice
  signature aggregation?

RPC, wallet, and operations:

- Are read-only RPC surfaces bounded and redaction-safe?
- Is write-edge submission sufficiently capped for controlled testnet?
- Is the minimum wallet flow enough for controlled testnet users to quote,
  sign, submit, and verify finality?
- Does the operator launch packet contain enough stop conditions for a launch
  captain?

Excluded scope:

- Production privacy is not in this launch evidence; current shielded flows use
  non-production proof plumbing.
- External bridge custody is not in this launch evidence.
- Public decentralization is not claimed from a controlled 5-validator cohort;
  current host-group evidence explicitly shows a topology concentration that
  can block quorum.
- TPS or latency claims require a separate benchmark packet with hardware,
  topology, workload, payload, and measurement-window details.

## Expected Reviewer Output

Useful review output should separate:

- must-fix blockers before controlled launch;
- should-fix issues before broader public testnet;
- wording changes needed for public claims;
- audit-prep items for cryptography and consensus;
- production privacy work that belongs in the Confidential Settlement v1 track.
