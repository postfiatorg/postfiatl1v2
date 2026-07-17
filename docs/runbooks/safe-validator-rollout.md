# Safe validator fleet rollout

`scripts/postfiat-safe-rollout` is the only supported controlled-testnet fleet
deployment entrypoint. It consumes the canonical
`deployment-validator-units-stage` report and never accepts an operator-chosen
remote destination, an rsync option, or a deletion operation.

## Safety contract

- `--delete`, every `--delete*` spelling, `/`, `host:/`, destination flags,
  target-root flags, and rsync passthrough flags fail before argument parsing.
- Final destinations are derived from the signed release ID and validator ID.
  The only destinations are the two release directories and that validator's
  two named systemd units.
- Preflight verifies all six signed validator bindings locally, freezes hashes
  of the stage report and inventory, reconciles every instance ID/IP/region
  against the owning Vultr account, obtains six converged live RPC statuses,
  runs the active binary's exact full-committee key validation on every node,
  and records an exact create/update/unchanged diff. Deletions are not a valid
  diff action.
- Backup is a mandatory state transition. It exports from the canary without
  stopping services, explicitly copies the snapshot locally, signs it without
  moving the publisher private key to a validator, re-imports it under the
  trusted public key, and runs `verify-state`.
- `apply-next` accepts no validator argument. The state file selects the
  canary first and then a strict one-node-at-a-time order. It refuses to run
  without the verified preflight and signed backup, and rechecks all six
  mutable committee rosters immediately before each host mutation.
- Files are copied to deterministic `.incoming-safe-rollout` names inside
  their allowed filesystems. Every incoming SHA-256 is checked before atomic
  rename, signed-manifest verification, and a one-node service restart.
- A successful apply records local health and six-node RPC convergence. A
  failed copy, manifest check, restart, or convergence check does not advance
  rollout state.
- A release that changes replicated-state encoding additionally requires the
  versioned activation and pre/post-transition evidence in
  [Replicated State V2 Activation](replicated-state-v2-activation.md). The
  generic rolling deploy is not sufficient for a consensus migration.

## Commands

Create a fresh read-only preflight state:

```bash
scripts/postfiat-safe-rollout preflight \
  --stage-report /release/validator-stage/stage-report.json \
  --inventory-file /secure/wan-vultr-all-fleet.txt \
  --vultr-api-key-file /secure/vultr-api-key \
  --state-file /evidence/rollout-state.json \
  --canary-validator-id validator-1
```

Create and verify the mandatory signed backup:

```bash
scripts/postfiat-safe-rollout backup \
  --state-file /evidence/rollout-state.json \
  --evidence-dir /evidence/pre-rollout-backup \
  --snapshot-publisher-key-file /secure/snapshot-publisher.private.json \
  --snapshot-publisher-public-key-file /secure/snapshot-publisher.public.json
```

Apply exactly one validator. Repeat only after the command returns zero; the
tool derives the next validator from durable state:

```bash
scripts/postfiat-safe-rollout apply-next \
  --state-file /evidence/rollout-state.json
```

Never edit rollout state by hand. A changed stage report or inventory invalidates
the recorded hashes and requires a new preflight packet.
