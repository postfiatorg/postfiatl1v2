# Deployment day — combined-devnet-20260921

**Future operator procedure. No command below was run against the fleet during
preparation.** Execute on the trusted signing/deployment workstation, from a
checkout containing this packet. Deploy only the qualified node executable.

The supported rollout is
[postfiat-safe-rollout](../../scripts/postfiat-safe-rollout):
**fresh preflight → signed canary backup → apply-next**, one validator at a
time. Never deploy with manual fleet copies, rsync, or hand-edited rollout state.

## 1. Local setup and the key-holder action

Regenerate and sign the stage using [SIGNING.md](SIGNING.md). The other lane's
exclusive operation is signing with the existing deployment and snapshot
publisher keys. The exact manifest command is:

```bash
set -euo pipefail
export PYTHONDONTWRITEBYTECODE=1
PACKET="$PWD/deployments/combined-devnet-20260921"
CACHE="$HOME/.cache/deploy-prep-20260921"
STAGE="$CACHE/validator-stage"
NODE="$STAGE/rootfs/opt/postfiat/releases/combined-devnet-20260921/postfiat-node"
EVIDENCE="$CACHE/deploy-$(date -u +%Y%m%dT%H%M%SZ)"
mkdir -m 0700 "$EVIDENCE"
VALID_FROM_UNIX="$(date -u +%s)"
VALID_UNTIL_UNIX="$((VALID_FROM_UNIX + 31536000))"
python3 "$PACKET/sign-manifest.py" \
  --stage "$STAGE" \
  --publisher-key-file /secure/deployment.private.json \
  --valid-from-unix "$VALID_FROM_UNIX" \
  --valid-until-unix "$VALID_UNTIL_UNIX"
```

If already signed as in SIGNING.md, run only
`python3 "$PACKET/local-preflight.py" --stage "$STAGE" --require-signed`.
Keep all private keys and the Vultr API credential off validators and out of Git.
Do not rebuild the node from the later preparation commit.

## 2. Fresh read-only fleet observation

Use a quiet deployment window: suspend transaction/proposal producers through
their owning operator before capturing the baseline. No governance, committee,
storage, or replicated-state activation is part of this rollout. The baseline
must remain unchanged while preserving the pre-activation rollback option.

Establish these six local RPC tunnels only if equivalent listeners are not
already running. A port conflict must fail, not attach the check to an unknown
listener. No tunnel was established by the preparation task.

```bash
ssh -fNT -o BatchMode=yes -o ExitOnForwardFailure=yes -L 127.0.0.1:39650:127.0.0.1:27650 root@64.176.220.75
ssh -fNT -o BatchMode=yes -o ExitOnForwardFailure=yes -L 127.0.0.1:39651:127.0.0.1:27651 root@95.179.184.122
ssh -fNT -o BatchMode=yes -o ExitOnForwardFailure=yes -L 127.0.0.1:39652:127.0.0.1:27652 root@66.42.48.39
ssh -fNT -o BatchMode=yes -o ExitOnForwardFailure=yes -L 127.0.0.1:39653:127.0.0.1:27653 root@149.28.63.106
ssh -fNT -o BatchMode=yes -o ExitOnForwardFailure=yes -L 127.0.0.1:39654:127.0.0.1:27654 root@95.179.179.206
ssh -fNT -o BatchMode=yes -o ExitOnForwardFailure=yes -L 127.0.0.1:39655:127.0.0.1:27655 root@45.32.110.170
python3 "$PACKET/observe-fleet.py" \
  --stage "$STAGE" --applied "" --rpc-tunnel-base-port 39650 \
  --output "$EVIDENCE/before.json"
```

Require all four services per host active; all twelve validator/RPC processes
on the exact old executable; six matching signed old deployment identities,
chain/genesis/protocol, height/tip/root, and empty mempools. The observer reads
`status`, `server_info`, and `mempool_status`, checks unit/environment/runtime
hashes, hashes actual running executables, and runs the real manifest verifier
against each host’s existing trust file. The runtime status field
\`deployment_manifest_verified\` is intentionally false in the candidate: status
hashing is not publisher authentication, so the observer does not use that flag
as a signature gate. A changed old manifest or
release is a stop requiring a refreshed packet, not permission to loosen the
observer. This script has been syntax-checked locally, not exercised against
the fleet in preparation.

## 3. Full supported preflight

```bash
scripts/postfiat-safe-rollout preflight \
  --stage-report "$STAGE/stage-report.json" \
  --inventory-file "$PACKET/inventory.txt" \
  --vultr-api-key-file /secure/vultr-api-key \
  --state-file "$EVIDENCE/rollout-state.json" \
  --canary-validator-id validator-1 \
  --rpc-tunnel-base-port 39650 \
  > "$EVIDENCE/preflight.json"
```

Inspect all six diffs, cloud identity reconciliation, registry/signer checks,
and convergence. Require `preflight.verified=true`, `deletion_count=0`, and
order `validator-1, validator-0, validator-2, validator-3, validator-4, validator-5`.
A changed inventory/stage requires a new preflight; never edit frozen state.

## 4. Mandatory signed backup and rollback-pair check

The key holder runs:

```bash
scripts/postfiat-safe-rollout backup \
  --state-file "$EVIDENCE/rollout-state.json" \
  --evidence-dir "$EVIDENCE/pre-rollout-backup" \
  --snapshot-publisher-key-file /secure/snapshot-publisher.private.json \
  --snapshot-publisher-public-key-file /secure/snapshot-publisher.public.json \
  > "$EVIDENCE/backup.json"
```

Require `backup.verified=true`, `source_validator=validator-1`, candidate binary
SHA-256 matching RELEASE-ID.txt, a nonempty signed-manifest hash, and the same
height/tip/root as `before.json`. Retain the complete backup locally. The tool
exports on validator-1, copies explicitly, signs locally, imports under the
trusted snapshot key, and verifies the finalized checkpoint. Its internal
signing command and supported resume command are in SIGNING.md.

**Before applying any validator, confirm its exact rollback pair.** The old
executable is
`/opt/postfiat/releases/a666-source-route-20260907/postfiat-node`, SHA-256
`57b0f4d1d42d66878d7dbb8c33919c7fa0f87c6cc1a4b9cc1a85d75b634eec83`.
The conditional recovery script below expects a separately retained,
node-specific compatible pre-upgrade data directory at:

```text
/var/lib/postfiat/rollback/combined-devnet-20260921/validator-N
```

It must restore the fresh baseline at the original
`/var/lib/postfiat/validator-N` path, preserve that node's own isolated signer
and anti-equivocation safety, and include any referenced storage generation.
Also retain that host's exact old signed units in its old release directory.
The September 18 local rollback rehearsal establishes compatibility, not the
existence or freshness of these six host-local anchors.

**Anchor creation/availability is unverified and remains a deploy-day
prerequisite.** The mandatory canary snapshot is not six stopped-directory
backups, and the tool has no per-host rollback-anchor operation. Have the
operator's existing backup/recovery procedure establish and verify these
anchors before proceeding. If it cannot, stop before canary; this packet does
not substitute a guessed data directory or the old executable alone. Do not
copy one validator's private signer or local signer-safety state to another.

The backup is also not permission to rewind committed history or discard
post-backup votes. If signing activity occurred since an anchor, preserve the
latest anti-equivocation state through the approved recovery procedure; do not
run the simple anchor restore below until that continuity is established.

## 5. Apply canary, inspect, then advance one node at a time

Define this shell function once. It checks the tool-selected next validator;
`apply-next` itself accepts no validator argument.

```bash
apply_checked() {
  local expected="$1" completed="$2"
  PYTHONPATH=python python3 - "$EVIDENCE/rollout-state.json" "$expected" <<'PY'
import json, sys
from postfiat_ops.safe_rollout import next_validator
with open(sys.argv[1]) as handle:
    assert next_validator(json.load(handle)) == sys.argv[2]
PY
  scripts/postfiat-safe-rollout apply-next \
    --state-file "$EVIDENCE/rollout-state.json" \
    > "$EVIDENCE/$expected.apply.json"
  python3 "$PACKET/observe-fleet.py" \
    --stage "$STAGE" --applied "$completed" --rpc-tunnel-base-port 39650 \
    --baseline "$EVIDENCE/before.json" \
    --output "$EVIDENCE/$expected.after.json"
}
```

Run **one line, inspect its receipts, then decide whether to run the next line**.
Do not run these as an unattended loop:

```bash
apply_checked validator-1 validator-1
apply_checked validator-0 validator-1,validator-0
apply_checked validator-2 validator-1,validator-0,validator-2
apply_checked validator-3 validator-1,validator-0,validator-2,validator-3
apply_checked validator-4 validator-1,validator-0,validator-2,validator-3,validator-4
apply_checked validator-5 validator-1,validator-0,validator-2,validator-3,validator-4,validator-5
```

Each apply rechecks all six registry rosters, checks the target diff, promotes
only allowlisted hash-verified files, verifies the signed manifest, stops the
target's two validator services, migrates legacy integrity metadata offline,
verifies its checkpoint, and restarts transport then RPC. It requires local
health and six-node convergence before advancing durable state.

After each apply, the observer additionally checks the exact signed release
identity and actual process hashes on every host. Upgraded nodes must report
build `03e422a7` and the new binary/manifest/artifact hashes; remaining nodes
must still report the old identity. Cobalt and archive services remain active
on their existing releases; their unit bytes and running executable hashes must
match the fresh baseline. Candidate transport and RPC readiness markers must
both be present and nonempty. Six-node convergence alone is not proof of a full
release identity match. If the additional observer fails after the tool already
advanced state, stop; do not edit the `applied` list.

## Stop conditions

Stop immediately on any nonzero command or missing evidence, signature/trust/
activation/expiry mismatch, artifact hash mismatch, wrong chain/genesis/protocol
or RPC schema, non-empty mempool, cloud/inventory or committee-roster mismatch,
failed copy/promotion/migration/checkpoint, service or readiness failure, mixed
identity outside the expected rolling prefix, state-root/tip/height divergence,
rejected expected receipt, failed conservation/security invariant, or loss of
the compatible rollback pair. This procedure also stops if the baseline advances.

Preserve incoming files, data, and logs. Stop an unhealthy affected validator's
RPC and transport services; do not advance the fleet. No governance activation,
receipt-producing probe, quorum override, deletion, state-file edit, or manual
release copy is a repair step. After replicated-state activation, follow
[forward recovery](../../docs/runbooks/replicated-state-v2-activation.md);
the old-binary rollback below is forbidden.

## Exact pre-activation rollback commands

There is **no `postfiat-safe-rollout rollback` subcommand**. These are conditional
emergency recovery commands restoring a retained exact data/binary pair, not an
alternative forward deployment. They implement the
[operator rollback boundary](../../docs/runbooks/public-operator-runbook.md)
and preserve the failed directory. They were not run during this task.

Use only after the anchor/signer-safety prerequisites above are verified and
the fleet is still at the original baseline. The script stops both target
services, preserves failed data, restores the retained data at its original
path, verifies the old checkpoint and exact baseline, reinstalls the retained
old signed units, verifies the old manifest, then starts transport and RPC.
It uses no candidate-written data with the old binary. Missing anchors,
missing retained units, or any verification error fail closed.

```bash
read -r BASE_HEIGHT BASE_TIP BASE_ROOT < <(
  python3 - "$EVIDENCE/before.json" <<'PY'
import json, sys
print(*json.load(open(sys.argv[1]))["ledger_identity"])
PY
)
# Choose only affected nodes, in reverse applied order. Each line is independent.
ssh root@45.32.110.170 bash -s -- validator-5 "$BASE_HEIGHT" "$BASE_TIP" "$BASE_ROOT" < "$PACKET/rollback-one.sh"
ssh root@95.179.179.206 bash -s -- validator-4 "$BASE_HEIGHT" "$BASE_TIP" "$BASE_ROOT" < "$PACKET/rollback-one.sh"
ssh root@149.28.63.106 bash -s -- validator-3 "$BASE_HEIGHT" "$BASE_TIP" "$BASE_ROOT" < "$PACKET/rollback-one.sh"
ssh root@66.42.48.39 bash -s -- validator-2 "$BASE_HEIGHT" "$BASE_TIP" "$BASE_ROOT" < "$PACKET/rollback-one.sh"
ssh root@64.176.220.75 bash -s -- validator-0 "$BASE_HEIGHT" "$BASE_TIP" "$BASE_ROOT" < "$PACKET/rollback-one.sh"
ssh root@95.179.184.122 bash -s -- validator-1 "$BASE_HEIGHT" "$BASE_TIP" "$BASE_ROOT" < "$PACKET/rollback-one.sh"
```

After **each** chosen rollback, run the observer before another recovery action,
setting `REMAINING` to the still-upgraded prefix (for canary rollback, empty):

```bash
REMAINING="" # replace with the exact still-upgraded prefix when applicable
python3 "$PACKET/observe-fleet.py" \
  --stage "$STAGE" --applied "$REMAINING" --rpc-tunnel-base-port 39650 \
  --baseline "$EVIDENCE/before.json" \
  --output "$EVIDENCE/rollback-$(date -u +%Y%m%dT%H%M%SZ).json"
```

For a complete reverse rollback, the successive remaining prefixes are:
`1,0,2,3,4`; `1,0,2,3`; `1,0,2`; `1,0`; `1`; empty
(use full `validator-N` IDs). Include a failed target even if apply did not
advance durable state. Keep the old rollout state as evidence; a later forward
attempt requires a fresh preflight and backup, never manual state repair.
The old executable's historical full-replay defect remains known; checkpoint
verification is not represented as full replay.

## 6. Final observation and operational status record

After all six applies and all intervening checks succeed:

```bash
python3 "$PACKET/observe-fleet.py" \
  --stage "$STAGE" \
  --applied validator-1,validator-0,validator-2,validator-3,validator-4,validator-5 \
  --rpc-tunnel-base-port 39650 --baseline "$EVIDENCE/before.json" \
  --output "$EVIDENCE/final.json"
python3 - "$EVIDENCE/rollout-state.json" <<'PY'
import json, sys
s = json.load(open(sys.argv[1]))
assert s["preflight"]["verified"] and s["backup"]["verified"]
assert s["applied"] == s["order"] == [
    "validator-1", "validator-0", "validator-2", "validator-3", "validator-4", "validator-5"
]
PY
```

Update `docs/status/chain-state-current.md` and the operational summary in
`STATUS.md` only from this final evidence: UTC observation window, all six
hosts and 24 service states, release ID, twelve process hashes, embedded build
revision, signed-manifest hash, chain/genesis/protocol, height/tip/root, mempools,
and explicit unchanged sidecar/archive identities. Preserve prior observations
as history; do not declare activation or new accepted receipts.

```bash
"${EDITOR:-vi}" docs/status/chain-state-current.md STATUS.md
.venv-docs/bin/mkdocs build --strict
scripts/public-doc-links
scripts/public-secret-scan
git add docs/status/chain-state-current.md STATUS.md
git commit -m "Record the combined devnet deployment observation"
git push origin HEAD:release/combined-devnet-20260915
```

Reference redaction-safe observation and rollout receipts in that status update;
keep private backups, keys, and raw host data under the secure cache.
Today’s preparation does not modify the live operational status record.
