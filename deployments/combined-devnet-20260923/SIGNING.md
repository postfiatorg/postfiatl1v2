# Signing handoff — combined-devnet-20260923

**Nothing in this file was executed during preparation. No key was generated,
nothing was signed, and fleet trust was not touched.** Signing is the only
remaining preparation step. The current deployment publisher
(`pf70522c56aebb1ba9dcdc3e7b569a7357c345a85a`, public file SHA-256
`66304dfca0b5893b156eb78e10d65a7b788c262043e85f26eadc82ea86a0314a`) is not on
this server. Choose exactly one path:

- **(a)** the other lane signs with the existing publisher key on his machine; or
- **(b)** after explicit approval, rotate trust to a new publisher key generated
  on this server, as rehearsed offline in
  [ROTATION-REHEARSAL.md](../combined-devnet-20260921/ROTATION-REHEARSAL.md#ordered-real-run-commands).

Both paths sign the same reviewed inputs: executable
`e7bb1afa17b4c6322ac8eadabdab570595ad778a5173a5516c09ba966ed9e4b1`, runtime
revision `1a0989ad`, and every unit/environment/topology/metadata hash in
[manifest-input.unsigned.json](manifest-input.unsigned.json). A wrong key,
changed input or rejected window is a stop condition.

## Path (a): the existing publisher, on the key holder's machine

Use a checkout of `release/combined-devnet-20260915` containing this packet
**and** `deployments/combined-devnet-20260921/` (the helpers reuse its scripts).
The `/secure/` paths are the key holder's existing files, not repository files
or instructions to create keys. Copy the qualified executable from
`~/.cache/qualify-fix-20260922/binaries/candidate-1` on the preparation server.

```bash
set -euo pipefail
export PYTHONDONTWRITEBYTECODE=1
PACKET="$PWD/deployments/combined-devnet-20260923"
CACHE="$HOME/.cache/deploy-prep-20260923"
STAGE="$CACHE/validator-stage"
printf '%s  %s\n' e7bb1afa17b4c6322ac8eadabdab570595ad778a5173a5516c09ba966ed9e4b1 \
  /secure/combined-devnet-20260923/postfiat-node | sha256sum -c -
python3 "$PACKET/prepare-stage.py" \
  --binary /secure/combined-devnet-20260923/postfiat-node \
  --trusted-publisher-public-file /secure/deployment.public.json \
  --output-dir "$STAGE"
VALID_FROM_UNIX="$(date -u +%s)"
VALID_UNTIL_UNIX="$((VALID_FROM_UNIX + 31536000))"
python3 "$PACKET/sign-manifest.py" \
  --stage "$STAGE" \
  --publisher-key-file /secure/deployment.private.json \
  --valid-from-unix "$VALID_FROM_UNIX" \
  --valid-until-unix "$VALID_UNTIL_UNIX"
```

`$STAGE` must not already exist on that machine. `prepare-stage.py` prints
`"result": "PASS"` and refuses a wrong executable or public-file hash.
`sign-manifest.py` runs the offline checks (expecting no manifest), calls the
node's real signer once, then reruns them with `--require-signed`
(`PASS_SIGNED_LOCAL_INPUTS_ONLY`, all six validators). The one-year window
mirrors the existing release; the publisher must review it. Expected output:

```text
$STAGE/rootfs/etc/postfiat/releases/combined-devnet-20260923/deployment-manifest.json
```

The expanded node invocation, for review (run the helper or this, not both):

```bash
CONFIG="$STAGE/rootfs/etc/postfiat/releases/combined-devnet-20260923"
NODE="$STAGE/rootfs/opt/postfiat/releases/combined-devnet-20260923/postfiat-node"
"$NODE" deployment-manifest-create \
  --deployment-id combined-devnet-20260923 \
  --valid-from-unix "$VALID_FROM_UNIX" \
  --valid-until-unix "$VALID_UNTIL_UNIX" \
  --chain-id postfiat-wan-devnet-2 \
  --genesis-hash ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad25521aff3ed334da07e150a7233a3e90a9 \
  --git-revision 1a0989ad \
  --binary-file "$NODE" \
  --build-profile release \
  --build-features privacy,rpc,transport \
  --protocol-version 1 \
  --rpc-schema postfiat-local-rpc-v1 \
  --service-unit-file "$STAGE/operator/postfiat-release-operator.service" \
  --environment-file "$STAGE/operator/operator.env" \
  --validator-bindings-file "$STAGE/validator-bindings.signing.json" \
  --topology-file "$CONFIG/topology.json" \
  --swap-circuit-metadata-file "$CONFIG/swap.metadata.json" \
  --private-egress-circuit-metadata-file "$CONFIG/private-egress.metadata.json" \
  --publisher-key-file /secure/deployment.private.json \
  --manifest-file "$CONFIG/deployment-manifest.json"
python3 "$PACKET/local-preflight.py" --stage "$STAGE" --require-signed
```

Return the signed `deployment-manifest.json` (public, signed data) to the
preparation server's stage at the same path, then run
`python3 "$PACKET/local-preflight.py" --stage "$HOME/.cache/deploy-prep-20260923/validator-stage" --require-signed`
there. Alternatively run the whole deployment from the key holder's machine;
rollout state freezes absolute paths, so never move it between machines.

The deployment-day canary backup is signed with the existing **snapshot**
publisher key by the same key holder; the exact `backup` and
`resume-backup-verification` commands are unchanged from the
[20260921 handoff](../combined-devnet-20260921/SIGNING.md#exact-backup-signing-command-deployment-day-only)
and are repeated in [DEPLOY-SHEET.md](DEPLOY-SHEET.md) step 4.

## Path (b): publisher-key rotation on this server

Run only after the rotation is explicitly approved. These are the rehearsed
[ordered real-run commands](../combined-devnet-20260921/ROTATION-REHEARSAL.md#ordered-real-run-commands)
with the new release, executable and stage. Two adaptations: the key directory
is named for this release, and the packet copy also needs the 20260921
directory (symlinked) because the helpers reuse its scripts.

### 1. Create and export the new deployment key

```bash
set -euo pipefail
umask 077
export PYTHONDONTWRITEBYTECODE=1
REPO="$PWD"
RELEASE=combined-devnet-20260923
NODE="$HOME/.cache/qualify-fix-20260922/binaries/candidate-1"
ORIGINAL="$HOME/.cache/deploy-prep-20260923/validator-stage"
WORK="$HOME/.cache/publisher-rotation-combined-devnet-20260923"
KEYDIR="$HOME/.postfiat/publishers/combined-devnet-20260923"
STAGE="$WORK/stage"
PACKET="$WORK/repo/deployments/$RELEASE"
CONFIG="$STAGE/rootfs/etc/postfiat/releases/$RELEASE"
test ! -e "$WORK"
test ! -e "$KEYDIR/deployment.private.json"
install -d -m 0700 "$KEYDIR" "$WORK" "$WORK/repo/deployments" "$WORK/tmp"
export TMPDIR="$WORK/tmp"
printf '%s  %s\n' \
  e7bb1afa17b4c6322ac8eadabdab570595ad778a5173a5516c09ba966ed9e4b1 \
  "$NODE" | sha256sum -c -
"$NODE" deployment-publisher-key-create \
  --publisher-key-file "$KEYDIR/deployment.private.json" > /dev/null
"$NODE" deployment-publisher-key-export \
  --publisher-key-file "$KEYDIR/deployment.private.json" \
  --public-key-file "$KEYDIR/deployment.public.json" > /dev/null
stat -c '%a %n' "$KEYDIR" "$KEYDIR/deployment.private.json"
sha256sum "$KEYDIR/deployment.public.json"
```

Expected: modes `700` and `600`; outputs `$KEYDIR/deployment.private.json`
(never in Git, a stage, a snapshot or a validator) and
`$KEYDIR/deployment.public.json`. Have the new public-file SHA-256 reviewed
before signing.

### 2. Copy the stage and update the review inventory

```bash
cp -a --reflink=never "$ORIGINAL" "$STAGE"
cp -a "$REPO/deployments/$RELEASE" "$PACKET"
ln -s "$REPO/deployments/combined-devnet-20260921" "$WORK/repo/deployments/combined-devnet-20260921"
ln -s "$REPO/python" "$WORK/repo/python"
ln -s "$REPO/crates" "$WORK/repo/crates"
cp "$KEYDIR/deployment.public.json" "$CONFIG/deployment.public.json"
```

Then run the rehearsal's unchanged Python block
([step 2](../combined-devnet-20260921/ROTATION-REHEARSAL.md#2-copy-the-stage-and-update-the-review-inventory))
with arguments `"$ORIGINAL" "$STAGE" "$PACKET" "$KEYDIR"`. It rebases the two
stage JSON files and sets only `trusted_publisher_file_sha256`, `publisher`
and `publisher_key_file` in the packet copy. The committed packet and the
original stage stay unchanged.

### 3. Sign and verify all six validators

```bash
VALID_FROM_UNIX="$(date -u +%s)"
VALID_UNTIL_UNIX="$((VALID_FROM_UNIX + 31536000))"
python3 "$PACKET/sign-manifest.py" \
  --stage "$STAGE" \
  --publisher-key-file "$KEYDIR/deployment.private.json" \
  --valid-from-unix "$VALID_FROM_UNIX" \
  --valid-until-unix "$VALID_UNTIL_UNIX"
```

Expected: `Signed manifest: $STAGE/rootfs/etc/postfiat/releases/combined-devnet-20260923/deployment-manifest.json`,
after `PASS_SIGNED_LOCAL_INPUTS_ONLY` for all six validators. Do not rerun it on
a signed stage. The optional direct per-validator `deployment-manifest-verify`
loop is the rehearsal's
[step 3](../combined-devnet-20260921/ROTATION-REHEARSAL.md#3-sign-and-verify-all-six-validators)
with the variables above.

### 4. Snapshot publisher for the deployment-day backup

Follow rehearsal
[step 4](../combined-devnet-20260921/ROTATION-REHEARSAL.md#4-prepare-the-separate-snapshot-publisher-and-sign-the-real-backup)
with the variables above. Expected outputs:
`$KEYDIR/snapshot-publisher.private.json` (mode 600) and
`$KEYDIR/snapshot-publisher.public.json`; the seed files are shredded. On
deployment day use `STAGE="$WORK/stage"` and `PACKET="$WORK/repo/deployments/combined-devnet-20260923"`
throughout [DEPLOY-SHEET.md](DEPLOY-SHEET.md), and the `$KEYDIR` snapshot files
in its step 4.

The new `deployment.public.json` ships in
`/etc/postfiat/releases/combined-devnet-20260923/` through the allowlisted
rollout alongside the signed manifest; the rehearsal found no publisher-continuity
signature is required. Preflight → signed backup → one-at-a-time apply and the
rollback requirements are unchanged.
