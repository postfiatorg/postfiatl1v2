# Publisher-key rotation rehearsal — 2026-09-22

Offline rehearsal for `combined-devnet-20260921`, based on release tip
`d75356f6cf1a1686cd129805ae9e3b1414341000`. This record does **not** approve a real
publisher rotation. The commands below become the operator sequence only if the
other lane approves the new key.

No Task Node action, fleet connection, remote backup export, installation,
service restart, or transaction was performed. The fleet's trusted public key
was not touched. Only disposable local copies were signed.

## Results

| Requested step | Result |
| --- | --- |
| 1. Throwaway keys and public exports | PASS: deployment key and separate snapshot key, private files mode 600 beneath the mode-700 rehearsal directory; key contents were never printed or recorded. |
| 2. Public-file replacement and signing | PASS: replaced the copied stage's public file, rebased local binding/report paths, updated the copied unsigned review inventory, and signed the complete manifest. |
| 3. Local verification | PASS: packet preflight and direct qualified-node verification for validator-0 through validator-5. Full rollout preflight is **not rehearsable offline**. |
| 4. Local snapshot signing and verification | PASS: small synthetic snapshot and exact signed finalized-checkpoint export/import, followed by checkpoint and identity verification. A fresh canary backup still requires the fleet. |
| 5. Destruction and original-stage comparison | PASS: 266 disposable files shredded and removed; only the unsigned record remains. Original 39-file stage digest unchanged. |
| 6. Documentation gates | PASS: strict MkDocs build, public documentation links, and tracked-tree public secret scan; all exit 0. |

The qualified executable used was
`~/.cache/qualify-fix-20260918/binaries/candidate-1`, SHA-256
`051ad12ce22c33f2370388259d754a9c1016a8edc1b52db7d2852fe78f6fbc7d`.
This is the successful build path recorded in
[the qualification packet](../release-repair-20260918/node-builds.json).
The similarly named file under `~/.cache/release-repair-20260918/binaries/`
belongs to a superseded build and was not executed.

The rehearsal approval window was `1790080508` through `1821616508`
(2026-09-22 12:35:08 UTC through 2027-09-22 12:35:08 UTC):
exactly 31,536,000 seconds, matching [SIGNING.md](SIGNING.md).

## Ordered real-run commands

Run on this server from the reviewed release checkout, only after approval.
Keep the entire later preflight/backup/apply session on this workstation:
rollout state freezes absolute paths. The real deployment private key is
`~/.postfiat/publishers/combined-devnet-20260921/deployment.private.json`,
mode 600. The separate snapshot private key is
`~/.postfiat/publishers/combined-devnet-20260921/snapshot-publisher.private.json`,
also mode 600. Neither belongs in Git, a stage, a snapshot, or a validator.

### 1. Create and export the approved deployment key

```bash
set -euo pipefail
umask 077
export PYTHONDONTWRITEBYTECODE=1
REPO="$PWD"
RELEASE=combined-devnet-20260921
NODE="$HOME/.cache/qualify-fix-20260918/binaries/candidate-1"
ORIGINAL="$HOME/.cache/deploy-prep-20260921/validator-stage"
WORK="$HOME/.cache/publisher-rotation-combined-devnet-20260921"
KEYDIR="$HOME/.postfiat/publishers/combined-devnet-20260921"
STAGE="$WORK/stage"
PACKET="$WORK/repo/deployments/$RELEASE"
CONFIG="$STAGE/rootfs/etc/postfiat/releases/$RELEASE"
test ! -e "$WORK"
test ! -e "$KEYDIR/deployment.private.json"
install -d -m 0700 "$KEYDIR" "$WORK" "$WORK/repo/deployments" "$WORK/tmp"
export TMPDIR="$WORK/tmp"
printf '%s  %s\n' \
  051ad12ce22c33f2370388259d754a9c1016a8edc1b52db7d2852fe78f6fbc7d \
  "$NODE" | sha256sum -c -
"$NODE" deployment-publisher-key-create \
  --publisher-key-file "$KEYDIR/deployment.private.json" > /dev/null
"$NODE" deployment-publisher-key-export \
  --publisher-key-file "$KEYDIR/deployment.private.json" \
  --public-key-file "$KEYDIR/deployment.public.json" > /dev/null
stat -c '%a %n' "$KEYDIR" "$KEYDIR/deployment.private.json"
```

Expected modes: 700 and 600. The creator refuses to overwrite a key. Have the
approved new public-file fingerprint reviewed before the real signing session.

### 2. Copy the stage and update the review inventory

`local-preflight.py` pins the old public-file hash in
`manifest-input.unsigned.json`. Merely replacing `deployment.public.json`
would fail that check. Use a separate packet copy with the approved new
publisher and hash; keep every binary, unit, environment, topology, and circuit
hash unchanged. The original prepared stage remains untouched.

```bash
cp -a --reflink=never "$ORIGINAL" "$STAGE"
cp -a "$REPO/deployments/$RELEASE" "$PACKET"
ln -s "$REPO/python" "$WORK/repo/python"
ln -s "$REPO/crates" "$WORK/repo/crates"
cp "$KEYDIR/deployment.public.json" "$CONFIG/deployment.public.json"
python3 - "$ORIGINAL" "$STAGE" "$PACKET" "$KEYDIR" <<'PY'
import hashlib, json, sys
from pathlib import Path
original, stage, packet, keydir = map(Path, sys.argv[1:])

def rebase(value):
    if isinstance(value, str):
        old = str(original)
        if value == old or value.startswith(old + "/"):
            return str(stage) + value[len(old):]
        return value
    if isinstance(value, list):
        return [rebase(v) for v in value]
    if isinstance(value, dict):
        return {k: rebase(v) for k, v in value.items()}
    return value

for name in ("stage-report.json", "validator-bindings.signing.json"):
    path = stage / name
    path.write_text(json.dumps(rebase(json.loads(path.read_text())), indent=2) + "\n")
public_path = keydir / "deployment.public.json"
public = json.loads(public_path.read_text())
path = packet / "manifest-input.unsigned.json"
inputs = json.loads(path.read_text())
inputs["trusted_publisher_file_sha256"] = hashlib.sha256(public_path.read_bytes()).hexdigest()
inputs["publisher"] = public["publisher"]
inputs["publisher_key_file"] = str(keydir / "deployment.private.json")
path.write_text(json.dumps(inputs, indent=2) + "\n")
PY
```

The per-validator runtime binding files keep their final host paths. They have
no separate signature: `deployment-manifest-create` signs all twelve service
unit/environment hash pairs in the single fleet manifest, as described in
[the deployment runbook](../../docs/runbooks/signed-deployment-manifest.md).
Only the local signing binding and stage report need path rebasing.

### 3. Sign and verify all six validators

```bash
VALID_FROM_UNIX="$(date -u +%s)"
VALID_UNTIL_UNIX="$((VALID_FROM_UNIX + 31536000))"
python3 "$PACKET/sign-manifest.py" \
  --stage "$STAGE" \
  --publisher-key-file "$KEYDIR/deployment.private.json" \
  --valid-from-unix "$VALID_FROM_UNIX" \
  --valid-until-unix "$VALID_UNTIL_UNIX"
python3 "$PACKET/local-preflight.py" --stage "$STAGE" --require-signed
```

The helper runs unsigned input checks, creates/signs the manifest once, and
verifies all six validators. Do not run it again against an already signed
stage: its first check deliberately expects no manifest.

For direct verification, translate each runtime binding into a local-only
copy, then verify with the retained qualified executable:

```bash
mkdir -m 0700 "$WORK/local-bindings"
python3 - "$STAGE" "$CONFIG" "$WORK/local-bindings" <<'PY'
import json, sys
from pathlib import Path
stage, config, output = map(Path, sys.argv[1:])
for i in range(6):
    name = f"validator-{i}.bindings.json"
    binding = json.loads((config / name).read_text())
    for row in binding["validators"]:
        for service in row["services"]:
            for field in ("service_unit_file", "environment_file"):
                target = Path(service[field])
                assert target.is_absolute() and ".." not in target.parts
                service[field] = str(stage / "rootfs" / target.relative_to("/"))
    (output / name).write_text(json.dumps(binding, indent=2) + "\n")
PY
for i in 0 1 2 3 4 5; do
  "$NODE" deployment-manifest-verify \
    --manifest-file "$CONFIG/deployment-manifest.json" \
    --trusted-publisher-key-file "$CONFIG/deployment.public.json" \
    --validator-id "validator-$i" \
    --validator-bindings-file "$WORK/local-bindings/validator-$i.bindings.json" \
    --runtime-binary-file "$NODE" \
    --runtime-topology-file "$CONFIG/topology.json" \
    --runtime-swap-circuit-metadata-file "$CONFIG/swap.metadata.json" \
    --runtime-private-egress-circuit-metadata-file "$CONFIG/private-egress.metadata.json" \
    > /dev/null
  printf 'validator-%s PASS\n' "$i"
done
```

### 4. Prepare the separate snapshot publisher and sign the real backup

The [snapshot runbook](../../docs/runbooks/signed-validator-snapshot-recovery.md)
provides `snapshot-publisher-key-export`, which reads an existing key; there
is no `snapshot-publisher-key-create` command. The snapshot signer expects
the generic ML-DSA key format, not the dedicated deployment-private-key schema.
The rehearsal used `wallet-keygen` with an independent random seed solely to
produce this format. It was never funded or used to sign a transaction.

For an approved **new** snapshot publisher, the tested creation/export sequence
is below. If the operator supplies an existing approved snapshot publisher at
the named private path, run only its public export.

```bash
test ! -e "$KEYDIR/snapshot-publisher.private.json"
test ! -e "$KEYDIR/snapshot-publisher.seed"
test ! -e "$KEYDIR/snapshot-publisher.seed-backup.json"
openssl rand -hex -out "$KEYDIR/snapshot-publisher.seed" 32
"$NODE" wallet-keygen --chain-id postfiat-wan-devnet-2 \
  --master-seed-hex-file "$KEYDIR/snapshot-publisher.seed" \
  --key-file "$KEYDIR/snapshot-publisher.private.json" \
  --backup-file "$KEYDIR/snapshot-publisher.seed-backup.json" > /dev/null
"$NODE" snapshot-publisher-key-export \
  --publisher-key-file "$KEYDIR/snapshot-publisher.private.json" \
  --public-key-file "$KEYDIR/snapshot-publisher.public.json" > /dev/null
stat -c '%a %n' "$KEYDIR/snapshot-publisher.private.json"
shred -n 3 -z -u -- \
  "$KEYDIR/snapshot-publisher.seed" "$KEYDIR/snapshot-publisher.seed-backup.json"
```

The following commands require the deployment-day authorization and fleet
prerequisites in [DEPLOY-SHEET.md](DEPLOY-SHEET.md); they were **not executed**.
Use the new `STAGE` and `PACKET` paths above throughout that sheet, including
the fleet observer. After establishing its RPC tunnels and baseline:

```bash
EVIDENCE="$WORK/deploy-$(date -u +%Y%m%dT%H%M%SZ)"
mkdir -m 0700 "$EVIDENCE"
scripts/postfiat-safe-rollout preflight \
  --stage-report "$STAGE/stage-report.json" \
  --inventory-file "$PACKET/inventory.txt" \
  --vultr-api-key-file /secure/vultr-api-key \
  --state-file "$EVIDENCE/rollout-state.json" \
  --canary-validator-id validator-1 \
  --rpc-tunnel-base-port 39650 > "$EVIDENCE/preflight.json"
scripts/postfiat-safe-rollout backup \
  --state-file "$EVIDENCE/rollout-state.json" \
  --evidence-dir "$EVIDENCE/pre-rollout-backup" \
  --snapshot-publisher-key-file "$KEYDIR/snapshot-publisher.private.json" \
  --snapshot-publisher-public-key-file "$KEYDIR/snapshot-publisher.public.json" \
  > "$EVIDENCE/backup.json"
```

Require `preflight.verified=true`, then `backup.verified=true` and the
deployment sheet's fresh baseline checks. The backup command internally runs:

```bash
"$NODE" snapshot-export-signed-finalized-checkpoint \
  --data-dir "$EVIDENCE/pre-rollout-backup/backup-migrated-source" \
  --snapshot-dir "$EVIDENCE/pre-rollout-backup/backup-signed" \
  --publisher-key-file "$KEYDIR/snapshot-publisher.private.json"
```

Do not invoke that primitive a second time after `backup`. It re-imports with
`snapshot-import-signed-finalized-checkpoint` under the exported public key
and runs `verify-finalized-checkpoint`. If only verification was interrupted,
use the existing `resume-backup-verification` command in [SIGNING.md](SIGNING.md).

**The deploy-sheet change:** the approved new
`$STAGE/rootfs/etc/postfiat/releases/combined-devnet-20260921/deployment.public.json`
ships in that release directory through the existing allowlisted rollout,
alongside the newly signed manifest. No publisher-continuity signature is
required. The existing fresh preflight → signed backup → one-validator-at-a-time
apply sequence and rollback requirements still apply.

## Exact rehearsal settings and observations

The common commands above were exercised with these substitutions:

```bash
REPO=/tmp/rotation-rehearsal-20260922
RELEASE=combined-devnet-20260921
NODE="$HOME/.cache/qualify-fix-20260918/binaries/candidate-1"
ORIGINAL="$HOME/.cache/deploy-prep-20260921/validator-stage"
WORK="$HOME/.cache/key-rehearsal-20260922"
KEYDIR="$WORK"
STAGE="$WORK/stage"
PACKET="$WORK/repo/deployments/$RELEASE"
CONFIG="$STAGE/rootfs/etc/postfiat/releases/$RELEASE"
VALID_FROM_UNIX=1790080508
VALID_UNTIL_UNIX=1821616508
```

Setup ran `git fetch origin`, then
`git worktree add --detach /tmp/rotation-rehearsal-20260922 origin/release/combined-devnet-20260915`;
both exited 0. The worktree started at the expected tip. Rehearsal setup used
`umask 077` and `mkdir -m 0700 "$WORK"`. Disposable Python wrappers executed the
commands with stdout suppressed for key/public-file and signed-manifest JSON;
only paths, return codes, and selected verification results were recorded.

The copied `local-preflight.py` changed only its scratch directory literal
from `.cache/deploy-prep-20260921/tmp` to `.cache/key-rehearsal-20260922/tmp`.
All checks remained active. The unsigned review copy changed only
`trusted_publisher_file_sha256`, `publisher`, and `publisher_key_file`.
Neither adjustment was made to the release packet in Git.

| Command/check | Actual result |
| --- | --- |
| Directory creation; stage copy; packet copy; path rebasing; public-file copy; review-inventory update | PASS |
| `deployment-publisher-key-create`; `deployment-publisher-key-export` with the rehearsal paths above | Both exit 0; private file 600 |
| `sign-manifest.py` with the exact window above | Exit 0 |
| `local-preflight.py --stage "$STAGE" --require-signed` | Exit 0; `PASS_SIGNED_LOCAL_INPUTS_ONLY`; 33 canonical files, 4 circuit artifacts; network calls 0; rollout state not created |
| Direct `deployment-manifest-verify`, validator-0 | Exit 0, PASS |
| Direct `deployment-manifest-verify`, validator-1 | Exit 0, PASS |
| Direct `deployment-manifest-verify`, validator-2 | Exit 0, PASS |
| Direct `deployment-manifest-verify`, validator-3 | Exit 0, PASS |
| Direct `deployment-manifest-verify`, validator-4 | Exit 0, PASS |
| Direct `deployment-manifest-verify`, validator-5 | Exit 0, PASS |
| `scripts/postfiat-safe-rollout preflight --help` | Exit 0; no offline/dry-run mode |

The six direct commands used the local binding copies and the retained qualified
executable, exactly as in the loop above. The production
[preflight implementation](../../python/postfiat_ops/safe_rollout.py) queries
Vultr, fleet RPC, and SSH before completing its checks. It was not invoked.

### Local snapshot rehearsal

The snapshot key commands above ran with `KEYDIR="$WORK"` and key-derivation
chain ID `postfiat-key-rehearsal-20260922`; each exited 0. The key, seed, and
seed-backup remained inside the throwaway directory.

A small synthetic snapshot was exercised without starting a node service or
creating a transaction. These commands each exited 0:

```bash
"$NODE" init --data-dir "$WORK/small-snapshot-source" \
  --chain-id postfiat-key-rehearsal-20260922 --node-id validator-0 --validators 1 \
  > /dev/null
"$NODE" snapshot-export-signed --data-dir "$WORK/small-snapshot-source" \
  --snapshot-dir "$WORK/small-snapshot-signed" \
  --publisher-key-file "$WORK/snapshot-publisher.private.json" > /dev/null
"$NODE" snapshot-import-signed --data-dir "$WORK/small-snapshot-verified" \
  --snapshot-dir "$WORK/small-snapshot-signed" \
  --trusted-publisher-key-file "$WORK/snapshot-publisher.public.json" \
  --node-id validator-0 > /dev/null
```

PASS: height 0, 20 replicated files, 17,109 replicated bytes. Signed import
verified the signature and contents. It emitted legacy-integrity upgrade
warnings for its new disposable destination only.

The exact finalized-checkpoint signing primitive was also exercised using the
retained local height-1020 snapshot. Its 225,857,680 replicated bytes were
copied; the retained archive was never opened as a mutable data directory:

```bash
cp -a --reflink=never \
  "$HOME/.cache/combined-release-20260915/snapshot-checkpoint-1020" \
  "$WORK/snapshot-input"
"$NODE" snapshot-import-finalized-checkpoint \
  --data-dir "$WORK/snapshot-source" --snapshot-dir "$WORK/snapshot-input" \
  --node-id validator-0 > /dev/null
"$NODE" snapshot-export-signed-finalized-checkpoint \
  --data-dir "$WORK/snapshot-source" --snapshot-dir "$WORK/snapshot-signed" \
  --publisher-key-file "$WORK/snapshot-publisher.private.json" > /dev/null
"$NODE" snapshot-import-signed-finalized-checkpoint \
  --data-dir "$WORK/snapshot-verified" --snapshot-dir "$WORK/snapshot-signed" \
  --trusted-publisher-key-file "$WORK/snapshot-publisher.public.json" \
  --node-id validator-0 > /dev/null
"$NODE" verify-finalized-checkpoint --data-dir "$WORK/snapshot-verified"
"$NODE" status --data-dir "$WORK/snapshot-verified"
"$NODE" status --data-dir "$WORK/snapshot-source"
```

PASS: every command exited 0; `verified=true`, verification basis
`consensus-v2-finalized-checkpoint`, activation height 1, retained height 1020.
Source and signed-import status matched on chain ID, genesis, height, tip, and
state root. The signed snapshot contained 225,877,559 bytes. This proves the
local signing/import primitive against retained data, not backup freshness.

A fresh canary export, cloud inventory reconciliation, remote signer/registry
checks, live convergence, rollback-anchor availability, installation, service
startup, and rollout advancement were not rehearsable without the fleet.
No `postfiat-safe-rollout backup` or `apply-next` command was run.

### Destruction and unchanged-stage proof

The directory digest is SHA-256 of the sorted GNU `sha256sum` listing of all
regular files, using relative paths. The original stage contained 39 files
and no symlinks. The same command was run before the copy and after destruction:

```bash
(
  set -euo pipefail
  cd "$HOME/.cache/deploy-prep-20260921/validator-stage"
  LC_ALL=C find . -type f -print0 |
    LC_ALL=C sort -z |
    xargs -0 sha256sum |
    sha256sum
)
```

- Before: `b939071ea34d2e6579ac846067dea0928fbbbe6bb0d0940c8b0b8f54f0309a33`
- After: `b939071ea34d2e6579ac846067dea0928fbbbe6bb0d0940c8b0b8f54f0309a33`

After every signing/verifying process exited, the following commands exited 0.
Before shredding, every disposable regular file was checked to have one hard
link. Directory symlinks to repository code were not followed.

```bash
WORK="$HOME/.cache/key-rehearsal-20260922"
find "$WORK" -xdev -type f ! -path "$WORK/record.json" \
  -exec shred -n 3 -z -u -- {} +
find "$WORK" -mindepth 1 -maxdepth 1 ! -name record.json \
  -exec rm -rf -- {} +
ls -la "$WORK"
```

The final listing contained only `.`, `..`, and the mode-600 unsigned
`record.json`; the parent directory remained mode 700. All 266 disposable
regular files were shredded with three overwrite passes plus a zero pass and
unlinked, including both publisher private keys, the snapshot seed and seed
backup, generated local-test keys, every signed rehearsal artifact, local and
runtime bindings, the stage copy, snapshot copies/imports, and wrapper scripts.

**The rehearsal keys were destroyed. Nothing was installed.** The original
prepared stage is byte-identical by the before/after digest above. No rehearsal
public key, signature, private key, or signed artifact is included in this
commit; the only retained cache file is the unsigned command/result record.

### Documentation checks

Run from the detached release worktree:

```bash
.venv-docs/bin/mkdocs build --strict
scripts/public-doc-links
scripts/public-secret-scan
```

The worktree used a symlink to the existing repository's `.venv-docs`; no
dependencies were installed. Only this Markdown record is committed.
All three exited 0: MkDocs completed in 4.36 seconds;
`public_documentation_links=ok files=471`; `public secret scan passed mode=tracked-tree`.
No Rust, full workspace, or long Orchard suite was run for this documentation-only
commit. The snapshot checks above exercised the requested local backup boundary.
