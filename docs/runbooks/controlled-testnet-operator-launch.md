# Controlled Testnet Operator Launch

Status: controlled-testnet runbook
Date: 2026-05-14
Audience: launch captain, validator operators, RPC operators

This runbook is the operational bridge between the release evidence and a live
controlled testnet. It assumes the launch candidate has already passed
`scripts/testnet-release-final-candidate`.

Current launch control point:

- Candidate revision:
  `d109751a7b3634812e026ddbdb3b72115432c6d3`.
- Final candidate report:
  `reports/testnet-release-final-candidate-current-head-d109751-sdk-p0/testnet-release-final-candidate-20260514T-current-head-d109751-sdk-p0.json`.
- Release package:
  `reports/testnet-release-packages/testnet-release-package-20260514T023818Z`.
- Operator launch packet:
  `reports/testnet-operator-launch-packet/current-d109751-sdk-p0/testnet-operator-launch-packet.json`.
- Controlled launch execution milestone:
  `docs/status/controlled-launch-execution-milestone.md`.
- Exact package remote-join rehearsal:
  `reports/testnet-release-final-candidate-current-head-d109751-sdk-p0/candidate/release-gate/release-gate/logs/exact-remote-join-dry-run.json`.

## Launch Packet

Before any live service start, generate an operator launch packet:

```bash
FINAL_CANDIDATE_REPORT=reports/testnet-release-final-candidate-current-head-d109751-sdk-p0/testnet-release-final-candidate-20260514T-current-head-d109751-sdk-p0.json \
  ROOT_DIR=reports/testnet-operator-launch-packet/current-d109751-sdk-p0 \
  REPORT=reports/testnet-operator-launch-packet/current-d109751-sdk-p0/testnet-operator-launch-packet.json \
  MARKDOWN=reports/testnet-operator-launch-packet/current-d109751-sdk-p0/operator-launch-packet.md \
  scripts/testnet-operator-launch-packet
```

The script validates:

- final candidate report status;
- release-candidate gate status;
- release gate status;
- package manifest verification;
- exact artifact remote-join evidence;
- completed soak checkpoint;
- P0 network gate linkage;
- host preflight linkage;
- private-material exclusion.

It writes a JSON report and a Markdown launch packet under
`reports/testnet-operator-launch-packet/`.

The current generated packet is:
`reports/testnet-operator-launch-packet/current-d109751-sdk-p0/operator-launch-packet.md`.

## Live Launch Sequence

1. Verify the received package with the package-local verifier.
2. Run host preflight from the release operator checkout.
3. Run exact remote-join dry run with the same operator artifact and private
   material in fake-root mode.
4. Stage validator-specific private files through the operator-controlled secure
   channel.
5. Install one validator slot at a time with the packaged provision script.
6. Start each validator and RPC service through systemd.
7. Collect status before moving to the next slot.
8. Confirm all validators report the same chain id, genesis hash, registry
   root, block tip, and state root.
9. Run read-only RPC checks and a certified transparent round.
10. Record post-launch release status and attach it to the launch packet.

The launch is not considered complete until the post-launch evidence listed in
`docs/status/controlled-launch-execution-milestone.md` has been recorded and
the canonical status docs point at it.

## RPC Policy

Validator RPC is read-only by default. Do not expose
`mempool_submit_signed_transfer` from validator RPC unless the launch captain
records a temporary exception. Use a separate public write edge for signed
transfer admission.

The RPC operating policy is
`docs/runbooks/public-rpc-operator-policy.md`.

## Validator History

Validators are partial-history nodes, not mandatory archive servers. The
operator must keep archive handoff and backfill evidence aligned with
`docs/runbooks/validator-history-retention.md` and
`docs/status/controlled-testnet-history-roles.json`.

Do not prune evidence that is still referenced by the active release packet.

## Governance Boundary

Controlled testnet uses Cobalt-derived validator governance in canonical-UNL
mode. The operator-facing governance source of truth is
`docs/governance/cobalt-controlled-testnet-plan.md`.

Do not describe the system as full open Cobalt consensus. The safe claim is:
validator-set evolution is explicit, signed, registry-root-bound, replayable,
and release-gated.

## Stop Conditions

Stop launch and collect evidence if any of these occur:

- package manifest verification fails;
- host preflight fails;
- exact remote-join rehearsal fails or leaves staging behind;
- install script accepts the wrong validator key;
- a validator reports a mismatched chain id, genesis hash, registry root, block
  tip, or state root;
- RPC read checks fail after service start;
- certified transparent round cannot form a quorum certificate;
- redaction scan finds private key, seed, mnemonic, SSH credential, password,
  or machine credential markers in public reports.

## Claim Boundaries

- This is a controlled testnet, not public decentralization.
- The launch candidate is transparent PQ settlement, not production privacy.
- Privacy remains a first-class product track, but debug proofs are not
  production privacy.
- Bridge simulation is not external asset custody.
