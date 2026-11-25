# Solana pfUSDC exit cutover

The postfiat login on this host belongs to the trusted administrator who orchestrates the cutover. It holds deployment keys and Docker access equivalent to host root. The network-facing payout worker runs as pf-sol-exit. Its separate account limits a compromised worker to its operational credentials, jobs and service permissions. The administrator can access those credentials and control the host. The machine owner runs the single privileged installation in step 4. Set REL, CHAIN_FIX and OPS_FIX to the approved release name and reviewed local commit IDs. The public gateway URL, its existing session cookie jar and six operator reviews are deployment inputs. Keep the public relay flag explicitly disabled through the real payout and its L1 settlement. Starting at step 8, direct L1 clients can burn. The immutable Solana binding supports only the revision-8 source bucket.

## 1. Qualify the release with burns closed

Hold release-sensitive governance, rotations and automated proposal producers from planning through six-way convergence. Coordinate transaction traffic while binaries are mixed. On 2026-11-23 from 04:10:14 to 04:11:21 UTC, seven polls of ports 39650–39655 all returned height 1753. The observed rate was 0 blocks/minute on every validator. Blocks require transactions. A quiet window is neither a throughput bound nor an ETA for reaching H.

Immediately before scheduling, sample six new ten-second intervals. Use the highest observed interval rate or one block/minute, whichever is higher. Budget both builds (179 s release build measured on this host plus 213 s archived interlock build), all three sequential replays (use at least 387 s each from the predecessor run) and six restarts. The six recorded host deployments took 75, 65, 26, 74, 65 and 27 seconds. Budget the slowest host plus 60 s for restore and convergence per remaining host. Substitute longer measured rehearsal times when applicable. The height adds twice the total time at the planning rate, another minute at that rate and two spare blocks, rounding upward. During the mixed-build interval, quiesce transaction producers or enforce a maximum arrival rate no higher than the rate used in this budget. If the operators cannot do either, defer scheduling. After all six converge, ordinary transactions can resume and advance the chain to H. Integrate the reviewed chain and ops fixes in a fresh release checkout. The two fixes must both be present. The checked-in development release height is unset.

```bash
set -euo pipefail
umask 077
export EXIT_WORK=/home/postfiat/var/sol-exit
export REL=${REL:?set a fresh sol-exit release name}
export CHAIN_FIX=${CHAIN_FIX:?set the reviewed chain commit}
export OPS_FIX=${OPS_FIX:?set the reviewed ops commit}
export CARGO_TARGET_DIR="$EXIT_WORK/target-release"
git -C "$EXIT_WORK/l1" fetch public refs/heads/main:refs/remotes/public/main
git -C "$EXIT_WORK/l1" worktree add -b "release/$REL" "$EXIT_WORK/$REL" public/main
cd "$EXIT_WORK/$REL"
git merge --no-ff --no-edit "$CHAIN_FIX"
git merge --no-ff --no-edit "$OPS_FIX"
export PYTHONPATH="$PWD/ops/sol-live"
export EXIT_ROLL_PLAN="$EXIT_WORK/$REL-timing.json"
python3 ops/sol-live/exit_roll.py plan --build-seconds "${BUILD_SECONDS:-392}" --replay-seconds "${REPLAY_SECONDS:-1161}" --output "$EXIT_ROLL_PLAN"
H=$(jq -er .height "$EXIT_ROLL_PLAN")
export H
cat "$EXIT_ROLL_PLAN"
python3 - <<'PY'
from pathlib import Path
from live import fleet
import os
h=int(os.environ["H"])
assert h>max(1754,fleet()["block_height"])
p=Path("crates/types/src/solana_exit.rs")
s=p.read_text()
marker="None, // Release sets the agreed future height here."
assert s.count(marker)==1
p.write_text(s.replace(marker,f"Some({h}), // Agreed WAN release height."))
PY
git add crates/types/src/solana_exit.rs
git commit -m "Schedule the closed Solana exit release at block $H"
cargo build --locked --release -p postfiat-node --bin postfiat-node
export B="$CARGO_TARGET_DIR/release/postfiat-node" KIT="$HOME/var/$REL"
REV=$(git rev-parse --short=8 HEAD)
export REV
install -d -m700 "$HOME/.config/postfiat"
oncall="$HOME/.config/postfiat/sol-exit-oncall.env"
tmp=$(mktemp "${oncall}.XXXXXXXX")
printf 'cd -- %q\nexport REV=%q H=%q KIT=%q\n' "$PWD" "$REV" "$H" "$KIT" > "$tmp"
mv -T "$tmp" "$oncall"
# Name the live release and the one it replaced; the kit has no defaults and refuses either missing.
EXIT_OLD_RELEASE=<live release id> EXIT_OLDER_RELEASE=<release it replaced> \
  python3 ops/sol-live/exit_release_kit.py --release "$REL" --binary "$B"
export ARCHIVE="$KIT/anchor-replay" INTEGRITY="$EXIT_WORK/replay/source/.integrity.key"
export FAUCET="$EXIT_WORK/replay/source/faucet_account.json"
mkdir -m700 "$ARCHIVE"
rsync -a --exclude='*key*' --exclude=faucet_account.json "$EXIT_WORK/replay/source/" "$ARCHIVE/"
replay() {
  docker run --rm --network none --cap-drop ALL --security-opt no-new-privileges \
    -u "$(id -u):$(id -g)" -v "$2:/var/lib/postfiat/validator-3" \
    -v "$4:/var/lib/postfiat/validator-3/faucet_account.json:ro" -v "$3:/var/lib/postfiat/validator-3/.integrity.key:ro" -v "$1:/opt/node:ro" ubuntu:24.04 \
    /opt/node verify-state --data-dir /var/lib/postfiat/validator-3
}
replay "$B" "$ARCHIVE" "$INTEGRITY" "$FAUCET" > "$KIT/evidence/anchor-replay.json"
jq -e --slurpfile prev "$EXIT_WORK/evidence/old-state.json" \
  '.verified and .block_log==$prev[0].block_log and .block_log.block_count==1753' "$KIT/evidence/anchor-replay.json"
```

Verify both the predecessor and the release against a private snapshot of the **current** committed generation before staging. At height 1753 the scratch archive above serves both purposes. At a later height set TIP_STORE and TIP_KEY to a consistent scratch snapshot and its matching integrity file. Take both binaries through the same generation. Preserve the original height-1753 archive for tag 3.

```bash
TIP_STORE=${TIP_STORE:-$ARCHIVE}
TIP_KEY=${TIP_KEY:-$INTEGRITY}
TIP_FAUCET=${TIP_FAUCET:-$FAUCET}
replay "$HOME/var/main-r64-20261121/bin/postfiat-node" "$TIP_STORE" "$TIP_KEY" "$TIP_FAUCET" > "$KIT/evidence/tip-old.json"
replay "$B" "$TIP_STORE" "$TIP_KEY" "$TIP_FAUCET" > "$KIT/evidence/tip-new.json"
python3 - <<'PY'
import os
from pathlib import Path
from live import fleet,read
p=Path(os.environ["KIT"])/"evidence"
a,b=[read(p/f"tip-{v}.json") for v in ("old","new")]
assert a["verified"] and b["verified"] and a["block_log"]==b["block_log"]
r=b["block_log"]; f=fleet()
assert (r["block_count"],r["tip_hash"],r["state_root"])==(f["block_height"],f["block_tip_hash"],f["state_root"])
PY
cmp "$KIT/evidence/tip-old.json" "$KIT/evidence/tip-new.json"
bash "$KIT/stage.sh"
```

The publisher credential stays at `~/.postfiat/trusted-deployment-publisher.private.json`. If the snapshot and the converged tip differ, refresh the snapshot and qualify both binaries again.

Exercise the integrated rotation interlock on the copied archive. The chain regression models activation at 1760. Build its test executable in a scratch tree with that fixture height. Keep the scheduled release binary for the exact release replay above.

```bash
git merge-base --is-ancestor "$CHAIN_FIX" HEAD
scratch="$KIT/interlock-source"
mkdir "$scratch"
git archive HEAD | tar -x -C "$scratch"
(
  cd "$scratch"
  python3 - <<'HEIGHT'
import os
from pathlib import Path
p=Path("crates/types/src/solana_exit.rs")
marker=f'Some({os.environ["H"]}), // Agreed WAN release height.'
s=p.read_text()
assert s.count(marker)==1
p.write_text(s.replace(marker,"Some(1760), // Archived interlock regression"))
HEIGHT
  python3 ops/sol-live/exit_review_gate.py packages "$scratch" > "$KIT/evidence/interlock-packages.txt"
  packages=()
  while read -r package; do packages+=(-p "$package"); done < "$KIT/evidence/interlock-packages.txt"
  CARGO_TARGET_DIR="$KIT/interlock-target" cargo test --locked --release "${packages[@]}" --lib --no-run --message-format=json > "$KIT/evidence/interlock-build.jsonl"
)
python3 ops/sol-live/exit_review_gate.py run "$scratch" "$KIT/evidence" "$ARCHIVE" "$INTEGRITY" "$FAUCET" | tee "$KIT/evidence/exit-review-count.log"
```

## 2. Roll validator 0, then the other five

```bash
python3 ops/sol-live/exit_roll.py roll 0 --dry-run
python3 ops/sol-live/exit_roll.py roll 0
v0="$KIT/evidence/deploy-v0"
test "$(cut -d' ' -f1 "$v0/binary-sha.txt")" = "$(sha256sum "$B" | cut -d' ' -f1)"
grep -Fxq plain-verify-ok "$v0/manifest-verify.txt"
grep -Fxq bound-verify-ok "$v0/manifest-verify.txt"
cat "$v0/processes.txt" "$v0/watchdog-before.txt" "$KIT/evidence/watchdog-restored-v0.log"
python3 -c 'from live import rpc; import os; assert rpc(0,"status")["build_git_revision"] == os.environ["REV"]'
```

Before host 0, the wrapper saves the full replay comparison with six independent current status reads in `pre-roll.json`. It refuses a stale replay. Before each host and again immediately before each validator and RPC restart, it refreshes six-way height, hash and root and checks the time budget for all remaining hosts against H. It keeps the highest observed rate. Each host must restore both original watchdog states and converge before another host may start. Any nonzero restoration status stops the roll, including one swallowed by the inherited script.

If a margin or restoration check fails, stop further rolls and release-sensitive traffic. Keep the producer hold. Restore any affected watchdog from the saved pre-roll state, inspect binary and unit state on the partial host, and obtain six-way agreement. If H is still ahead and the predecessor replays the entire committed tip, roll back individually and schedule a new H with fresh replay evidence. If that cannot be proved, keep traffic quiescent and coordinate forward recovery. Never reuse a failed host's rollback-unit backup by blindly rerunning deployment.

Inspect both running validator-0 image digests and restored watchdogs against the saved states. Continue in a separate step only after validator 0 passes.

```bash
for n in 1 2 3 4 5; do
  python3 ops/sol-live/exit_roll.py roll "$n"
done
python3 - <<'PY'
import os
from live import fleet
f=fleet()
assert f["build_git_revision"]==os.environ["REV"]
print(f["block_height"],f["block_tip_hash"],f["state_root"])
PY
```

Inspect the canary's staged SHA, manifest verification, restored watchdog state and fleet convergence before rolling validator 1. Check the per-host evidence after each restart. Restore predecessor units only while the predecessor can replay every committed block. Use the saved units at `/etc/postfiat/releases/main-r64-20261121/rollback-units/` and the kit's `restore-watchdog.sh` on each affected host, then recheck root convergence. After a new decision or burn requires this release, repair forward.

### 2.1 Rebuild the B24 rehearsal tool from the release

As soon as all six report `$REV`, and before the next 17:55 UTC B24 reading, run on the main machine:

```bash
ops/b24-live/rehearsal_tool.sh "$REL" "$REV" "$(sha256sum "$B" | cut -d' ' -f1)"
```

It builds `tools/b23-nav-canary` at `$REV` with `--locked`, installs it as `b23-coord/artifacts/$REL/postfiat-b23-nav-canary` and prints the line for `ops/b24-live/known-releases.txt`. Add that line, copy the tool to validator-0 as `/tmp/b23-tee-nav-canary-<first 16 hex of its sha256>`, then run `python3 ops/b24-live/daily.py --check`: it must name `$REL`. Until then the daily job refuses to run, and says which of these is missing.

## 3. Register the descriptor with burns closed

When the next proposal height reaches H, review the registration batch and submit through the six resident validator signers. The prepare command needs a writable scratch store with its matching integrity file accessible to the node. Run it inside the same store and key mounts used above.

```bash
mkdir -m700 "$KIT/governance"
docker run --rm --network none -u "$(id -u):$(id -g)" --cap-drop ALL \
  -v "$ARCHIVE:/var/lib/postfiat/validator-3" -v "$FAUCET:/var/lib/postfiat/validator-3/faucet_account.json:ro" -v "$INTEGRITY:/var/lib/postfiat/validator-3/.integrity.key:ro" \
  -v "$B:/opt/node:ro" -v "$PWD/crates/types/src/solana_exit_profile.json:/profile:ro" -v "$KIT/governance:/draft" \
  ubuntu:24.04 /opt/node vault-bridge-route-profile-governance --data-dir /var/lib/postfiat/validator-3 \
  --validators validator-0,validator-1,validator-2,validator-3,validator-4,validator-5 \
  --profile-file /profile --amendment-file /draft/amendment.json --batch-file /draft/batch.json
cmp "$KIT/governance/batch.json" crates/node/tests/fixtures/solana-exit/governance-batch.json
python3 ops/sol-live/exit_govern.py submit --revision "$REV" --release-height "$H"
```

Require all six series RPCs to report the reviewed descriptor hash, `active=true` and `burns_open=false`. Confirm the Ethereum descriptor still matches epoch 11. On a lost governance response check the amendment ID and receipts on the six validators before proposing another round.

Registration starts the chain-side hold on incompatible changes. Keep all six signer holds in force through the six accepted open receipts. After registration each validator operator inventories their resident signer outputs, outstanding authorizations (including partial or externally held signatures), governance coordinator queues and certified but uncommitted rotations. Resolve outstanding rounds or prove their signed slot expired. The ops gate separately checks the filenames in each resident consensus-v2 safety journal and refuses any round beyond the agreed tip. The operator inventory covers governance authorizations that have not entered consensus.

On each authenticated validator host the operator publishes a root-owned `/etc/postfiat/sol-exit-rotation-clearance.json` recording `validator` (validator-N), `profile_hash` (the registered profile hash), `revision` (resident build), `parent` (block_height, block_tip_hash and state_root), `hold_through_open` (true) and `outstanding_signed_rotation_rounds` (the actual list). Missing evidence or any listed round blocks qualification. A changed parent requires a new inventory. Open reads each root-owned regular clearance file again for its candidate parent and rejects group or world write access.

## 4. Precheck the immutable binding

```bash
setup_args=(--binary "$B" --archive "$ARCHIVE" --integrity-file "$INTEGRITY" --faucet-file "$FAUCET")
mkdir -p "$KIT/evidence/solana"
roles="$KIT/evidence/solana/roles.json"
if [[ -e "$roles" ]]; then
  python3 -c 'from exit_setup import CONFIG,read,verify_role_record; import sys; verify_role_record(read(CONFIG), read(sys.argv[1]))' "$roles"
else
  python3 ops/sol-live/exit_setup.py prove-roles --output "$roles"
fi
python3 ops/sol-live/exit_setup.py "${setup_args[@]}" --qualify --output "$KIT/evidence/solana/unsigned.json"
jq -e '.guest.forward and .guest.start==.config.initial_committee and .simulation.value.err==null' "$KIT/evidence/solana/unsigned.json"
```

Setup re-verifies the archived certificate and compares the authenticated committee, accepted ingress record and bucket across six validators. It checks finalized Solana vault config, custody, and USDC mint at one slot, deployed programs and loader authorities, executes the frozen checkpoint guest on every selected segment through the six-way finalized tip, and checks both simulated PDA bodies. At a stationary 1753 tip the guest can prove ancestry ending there. Wait for a later finalized block before signing.

The tag-3 instruction is 317 bytes. Its SHA-256 is `7d02bbfe0bc6b43a52746ed5d13cd506ea0b7d4b1628d00c279fc9ed38cf19b6`. Its route binds the shared custody, so the verifier must first be upgraded to the reviewed custody-route build. The dedicated pause authority is `ow7wprz3MPEN15BGLyNvdJr5AUq4LNZK2ox1gY1QEJx`. Its signing file stays offline from the payout service. Review the instruction, ordered accounts and simulated bindings in `unsigned.json`.

### Install the reviewed Task Node gateway and relay before tag 3

The reviewed withdrawal route is draft PR #429, branch `wallet/solana-withdraw`, at `56dbebb4672172c9f336dc95b831deb8281e3fd7`. The old resident relay returns 404 for withdrawals. On the trusted host stage a fresh detached checkout for the relay. Set `L1_SOLANA_WITHDRAWALS_ENABLED=false` explicitly in the reviewed startup environments before either rollout. Install the reviewed gateway through the Task Node production release procedure in `docs/releasing.md` once its approved main release contains these changes. Pin the running gateway image digest and its build ID to that release record. Inspect every serving gateway instance. Retain its existing authenticated relay upstream. On the trusted host point the user unit's WorkingDirectory and its Node entry point at the staged relay release, preserve the existing relay credentials by path and restart that relay into the disabled configuration. Do this only in the authorized release window.

```bash
TN_REVIEW=56dbebb4672172c9f336dc95b831deb8281e3fd7
TN_RELAY="$KIT/tasknode-relay"
git -C "$EXIT_WORK/tasknode-withdraw" worktree add --detach "$TN_RELAY" "$TN_REVIEW"
RELAY_UNIT=${RELAY_UNIT:-navcoin-relay.service}
relay_pid=$(systemctl --user show "$RELAY_UNIT" -p MainPID --value)
test "$relay_pid" -gt 1
NODE=$(readlink -e "/proc/$relay_pid/exe")
"$NODE" -e 'const [major,minor]=process.versions.node.split(".").map(Number); if(major!==24||minor<21)process.exit(1)'
node_bin=$(dirname "$NODE")
export PATH="$node_bin:$PATH"
test "$(command -v node)" = "$NODE"
(cd "$TN_RELAY" && "$NODE" "$node_bin/npm" ci && "$NODE" --test server/l1/withdrawal-routes.test.mjs)
```

Use the resident Node executable for all three steps. The reviewed engine range is 24.21 through 24.x. The resident executable at review was 24.22.1. Deploy the approved gateway, confirm the running digest on every instance, and take its build ID from the deployment record. Set GATEWAY to the wallet page's HTTPS origin and RELAY_COOKIE_FILE to the path of the existing authenticated cookie jar. On the relay host install the control command and its configuration under the on-call login. Keep those paths and the release directory accessible across logins.

```bash
GATEWAY=${GATEWAY:?set the public wallet origin}
GATEWAY_BUILD=${GATEWAY_BUILD:?set the approved deployed gateway build ID}
RELAY_COOKIE_FILE=${RELAY_COOKIE_FILE:?set the absolute session cookie jar path}
test -r "$RELAY_COOKIE_FILE"
control="$HOME/.local/lib/pf-sol-exit-relay"
install -d -m700 "$control" "$HOME/.local/bin"
install -m644 ops/sol-live/{exit_cutover.py,exit-relay-probe.cjs} "$control/"
install -m755 ops/sol-live/exit_relay.py "$control/"
ln -sfn "$control/exit_relay.py" "$HOME/.local/bin/disable-sol-exit"
release_env="$KIT/withdraw-relay.env"
"$control/exit_relay.py" configure --unit "$RELAY_UNIT" --release "$TN_RELAY" --node "$NODE" \
  --probe-module "$control/exit-relay-probe.cjs" --flag-file "$release_env" \
  --gateway "$GATEWAY" --build "$GATEWAY_BUILD" --cookie-file "$RELAY_COOKIE_FILE"
dropin="$HOME/.config/systemd/user/$RELAY_UNIT.d"
install -d -m700 "$dropin"
printf '%s\n' '[Service]' "WorkingDirectory=$TN_RELAY" 'ExecStart=' \
  "ExecStart=\"$NODE\" --require \"$control/exit-relay-probe.cjs\" server/l1/relay.js" \
  "EnvironmentFile=$release_env" > "$dropin/99-solana-exit.conf"
systemctl --user daemon-reload
env -i HOME="$HOME" USER="$USER" XDG_RUNTIME_DIR="$XDG_RUNTIME_DIR" \
  DBUS_SESSION_BUS_ADDRESS="unix:path=$XDG_RUNTIME_DIR/bus" /bin/bash -lc '"$HOME/.local/bin/disable-sol-exit"' \
  | tee "$KIT/evidence/fresh-login-disable.log"
```

The drill restarts the one relay with admission disabled, verifies its actual executable, entry point, revision and flag, then checks an authenticated public job GET. A random 96-hex request ID must appear with a 503 in that unit's journal through the installed Node HTTP diagnostics hook. The hook logs only the probe ID. Keep the cookie jar current and repeat the drill before opening.

### Stage the dedicated account and prove the archive before funding or tag 3

Use the system installation on this host. The historical user-mode alternative in `install-exit-user.sh` failed here because the kernel denied the bubblewrap UID map. A successful host-side prover check does not qualify the service account.

The dedicated Ed25519 identity is at `~/.config/postfiat/sol-bridge/exit-prover-ssh`. Its approved fingerprint is `SHA256:VV5ewlk2iMf5KywKoCL19H/2r81A6UCB2fUt7pjgktw`. Keep its private file and public sidecar at mode 0600. Before the owner's install, the orchestrator obtains the prover **host** fingerprint independently from its owner or provider console and sets PROVER_HOST_FP. Select its matching line from the existing operator trust store. If that store lacks the host, obtain the host public-key record through the same trusted channel. The worker trust file must contain only the key for this host and port.

```bash
key="$HOME/.config/postfiat/sol-bridge/exit-prover-ssh"
trust="$HOME/.config/postfiat/sol-bridge/exit-prover-known_hosts"
fp=$(cat ops/sol-live/exit-prover.fingerprint)
bash ops/sol-live/check-prover-identity.sh "$key" "$fp"
test "$(ssh-keygen -lf "$key.pub" | awk '{print $2}')" = "$fp"
ssh-keygen -F '[ssh8.vast.ai]:20628' -f "$HOME/.ssh/known_hosts" > "$KIT/prover-host-candidates"
: > "$KIT/prover-host-approved"
while read -r line; do
  [[ -z $line || $line == \#* ]] && continue
  actual=$(ssh-keygen -lf - <<<"$line" | awk '{print $2}')
  if [[ $actual == "${PROVER_HOST_FP:?set the independently verified prover host fingerprint}" ]]; then
    printf '%s\n' "$line" >> "$KIT/prover-host-approved"
  fi
done < "$KIT/prover-host-candidates"
bash ops/sol-live/check-prover-host.sh "$KIT/prover-host-approved"
install -m600 "$KIT/prover-host-approved" "$trust"
```

From the reviewed checkout, run the following command with the operator's existing prover administration access. It connects only to `root@ssh8.vast.ai` on port 20628 to install a root-owned forced command and add the generated public key with shell, forwarding and PTY access restricted. A pre-existing authorization for the same public key with different options stops the command. The service uses only this dedicated private identity.

```bash
bash ops/sol-live/authorize-exit-prover.sh
```

On the orchestrator's trusted build account, use an independent, clean checkout of the approved full release commit. Keep this checkout, toolchain, build cache and GitHub publishing credential outside the worker account’s write access. Stage the reviewed public inputs under the builder's own KIT. Verify the role signatures, witness against the archived checkpoint and host fingerprint before packaging. Package the committed worker files, hash-pinned wheels, frozen proof host and ELF, verifier, public role record, witness and approved host trust. The two private credentials stay outside the tar. After publication, place a copy of the tar at an absolute path readable by `postfiat` on the relay host. Use a fresh output path on each build.

```bash
set -euo pipefail
wheels="$EXIT_WORK/target-ops/ops3/wheels"
test "$(git rev-parse HEAD)" = "${RELEASE_COMMIT:?approved full release commit}"
test -z "$(git status --porcelain)"
CARGO_TARGET_DIR=$(mktemp -d "$KIT/verifier-build.XXXXXXXX")
export CARGO_TARGET_DIR
bash ops/sol-live/build-exit-verifier.sh
BUNDLE="$KIT/sol-exit-install.tar"
python3 ops/sol-live/build-exit-bundle.py --kit "$KIT" --wheels "$wheels" \
  --proof "$HOME/repos/b23-coord/evidence/withdraw" --verifier "$CARGO_TARGET_DIR/release/exit-verify-proof" \
  --trust "$KIT/prover-host-approved" --output "$BUNDLE"
stat -c %s -- "$BUNDLE"
sha256sum "$BUNDLE"
```

Inspect the tar inventory and release inputs. In the **private GitHub repository**, the orchestrator commits the approved release revision, tar filename, exact byte count and SHA-256 and pushes that commit. The owner opens that exact commit through their own authenticated GitHub session, verifies its release identity, and supplies its 64 lowercase hex digits as `SHA256`, its decimal byte count as `BUNDLE_BYTES`, and the absolute operator-readable tar path as `BUNDLE`. Staging reads at most one byte past the approved size and allows 120 seconds for the copy. The following is the owner's single installation command.

```bash
sudo /usr/bin/env -i PATH=/usr/sbin:/usr/bin:/sbin:/bin HOME=/root /bin/bash -ceu 'cd /; [[ $1 =~ ^[0-9a-f]{64}$ && $2 == /* && $3 =~ ^[1-9][0-9]{0,17}$ ]]; umask 077; d=$(mktemp -d /var/lib/pf-sol-install.XXXXXXXX); trap '\''rm -rf -- "$d"'\'' EXIT; cd "$d"; timeout --kill-after=5s 120s runuser -u postfiat -- /usr/bin/head -c "$(($3 + 1))" -- "$2" > bundle.tar; [[ $(stat -c %s bundle.tar) == "$3" ]]; printf "%s  bundle.tar\n" "$1" | sha256sum --strict -c -; umask 022; tar -xf bundle.tar --no-same-owner --no-same-permissions; /bin/bash ./install-exit-system.sh' _ "${SHA256:?digest from private GitHub commit}" "${BUNDLE:?absolute tar path}" "${BUNDLE_BYTES:?byte count from private GitHub commit}"
```

The installer checks the installed revision, disabled/stopped withdrawal unit, inactive self-test and free port before invalidating earlier proof. A unit that is activating, deactivating or reloading counts as running. Both units are masked for the whole run and unmasked when it ends, whether it succeeds or fails. The new tree is built and checked under `/opt/postfiat/sol-exit.d/`; a failed build or check leaves everything as it was. A failure after the new unit files are loaded puts back the live tree's links and its own unit files; the self-test must then pass again. Its unit files and the self-test gate are loaded before the tree is switched in by one rename of the `/opt/postfiat/sol-exit` link. The previous tree stays as `sol-exit.previous`; appending `--rollback` puts it and its unit files back. Appending `--prune` removes older trees, and a re-run removes trees left by an interrupted install. A bundle older than the installed revision is refused unless `--allow-downgrade` is appended. The worker starts only on the tree the self-test unit last passed on. The worker runs as user and group ID 59900 (`account_id` in the installer); an install at ID 999 is moved there with both units stopped and masked: group first, then user, then every file under `/var/lib/pf-sol-exit` with the old IDs. A filesystem mounted under that directory stops the move. A same-revision reinstall keeps the installed payer inode. On first install it copies the two checked source descriptors into private staging, validates the staged identities under the worker UID, and publishes them. Retire the administrator’s original fee-payer copy under the owner’s key-custody procedure before funding. Any retained copy can still spend that key’s SOL.

**First orchestrator step after the owner returns.** Run the installed oneshot. Its root preflight checks the worker’s actual host identity and sudo policy. It refuses Docker group membership, write access to /run/docker.sock, any sudo grant, and permission to read or traverse the administrator’s home. The remaining proof runs unprivileged. Its systemd settings use the worker UID, dedicated prover identity, protected home, read-only runtime and writable jobs directory. It binds port 18089 briefly, checks the installed payer, six distinct validator tunnels and finalized Solana network, and verifies the returned proof and public values locally with the pinned Solana tag-7 verifier. Stop here on failure. Both funding and tag 3 independently verify the saved possession signatures and the root-owned self-test log before signing.

```bash
sudo -n /usr/bin/systemctl start pft-sol-exit-self-test.service
cp /var/log/pf-sol-exit/self-test.log "$KIT/evidence/service-proof.log"
python3 -c 'from exit_setup import CONFIG, read, require_installed_self_test; require_installed_self_test(read(CONFIG))'
```

The owner script leaves the withdrawal unit disabled. The service-specific sudo rule covers the self-test and start, stop or status for the withdrawal unit. Boot enablement and offline journal rebuild are owner procedures. The administrator retains host-wide access through Docker.

Fund the dedicated fee payer once. Preserve the journal at `~/var/sol-exit/funding-attempt.json` through retries and releases. This command persists the signature, blockhash and expiry before sending 0.10 SOL. After a timeout rerun it with the same journal and an RPC serving finalized transaction history. A pending signature blocks a second transfer.

```bash
python3 ops/sol-live/exit_fund.py --execute
```

The funding recipient is the operational fee payer `8EiSLnZQv23HtA5adokE6xfKyCK6mf4HGkpxSPpop1fs`. This low-value operational key receives 100,000,000 lamports (0.10 SOL) in the funding step. The worker reserves 20,000,000 lamports (0.02 SOL) before spending on rent and fees. A holder of the key can spend its SOL. It has no authority over the vault, verifier or pause flag. Vault release requires the pinned verifier’s signed PDA (solana/pfusdc-vault/src/lib.rs:30 and solana/pfusdc-egress-verifier/src/vault.rs:26). Tag 3 requires the deployment signer, stores the separate pause authority, and tag 4 checks that signer (solana/pfusdc-egress-verifier/src/lib.rs:152, :180 and :376). The reviewed tag-3 accounts and payload exclude the worker payer (ops/sol-live/exit_setup.py:211 and :247). The upgrade-authority check rejects either program naming the worker payer (ops/sol-live/exit_setup.py:122). Both role-specific signatures cover the reviewed instruction digest and a fresh random challenge. Readiness quotes peak proof-buffer, checkpoint, marker and ATA rents and transaction fees above the 20,000,000-lamport floor. All Solana buckets sharing this custody must remain covered, including queued redemptions pending L1 settlement.

## 5. Initialize Solana

```bash
python3 ops/sol-live/exit_setup.py "${setup_args[@]}" --execute --output "$KIT/evidence/solana/initialized.json"
jq -e '.finalized_readback and .signature' "$KIT/evidence/solana/initialized.json"
```

Initialization uses the setup payer and verifier deployment signers at `~/.config/postfiat/sol-bridge/{mainnet-payer,egress-verifier}.json`. Empty system-owned prefunded PDAs are eligible for creation. If RPC times out, repeat the unsigned setup command first. It checks the immutable light fields, original checkpoint PDA and exact finalized tag-3 transaction, then matches the current checkpoint PDA to the same finalized header on all six validators. For deep history provide `--init-signature` and an archival mainnet RPC. Stop on a conflicting binding. A permanent binding error needs a reviewed forward repair.

## 6. Verify the initialized checker

```bash
python3 ops/sol-live/exit_setup.py "${setup_args[@]}" --output "$KIT/evidence/solana/readback.json"
jq -e '.finalized_readback and (.paused|not) and .guest.forward and .guest.prior==.block and .guest.start==.committee' "$KIT/evidence/solana/readback.json"
tag8=(--setup-payer --guest "$KIT/evidence/solana/guest" --work "$KIT/evidence/tag8")
python3 ops/sol-live/exit_tag8.py stage "${tag8[@]}"
python3 ops/sol-live/exit_tag8.py simulate "${tag8[@]}"
jq -e '.value.err==null' "$KIT/evidence/tag8/simulation.json"
python3 ops/sol-live/exit_tag8.py reclaim "${tag8[@]}"
```

If the current checkpoint is already the agreed tip, wait for a newer finalized block and run readback again. The orchestrator uses the setup payer for this rehearsal, including the buffer rent, fees and reclaim. The worker fee payer remains under its service identity. Staging obtains the Groth16 proof and signs the buffer allocation and uploads. Simulation sends an unsigned tag 8 to `simulateTransaction` and checks its returned light and checkpoint bytes. Reclaim the buffer rent after successful simulation. Keep burns closed if any check fails.

## 7. Start the installed payout service with burns closed

After finalized initialization and tag-8 simulation, start the staged worker. The machine owner may separately enable it at boot after cutover. Check closed-route readiness on its loopback listener.

```bash
export API=http://127.0.0.1:18089/solana/withdrawals
sudo -n /usr/bin/systemctl start pft-sol-mainnet-withdraw.service
sudo -n /usr/bin/systemctl --no-pager status pft-sol-mainnet-withdraw.service
timeout 610s python3 ops/sol-live/exit_cutover.py wait-closed --api "$API" --seconds 600 > "$KIT/evidence/closed-readiness.json"
python3 ops/sol-live/exit_canary.py preflight --output "$KIT/canary"
```

### Must be true before the real minimum-amount canary

Repeat the disabled relay checks from step 4 against the same deployed gateway and relay. Freeze both deployments through settlement.

```bash
"$HOME/.local/lib/pf-sol-exit-relay/exit_relay.py" verify-disabled
```

Review the saved evidence for each completed step. Evaluate the final two checks at steps 8 and 9.

- [ ] The pre-roll six-validator snapshot and tip-new.json agree on height, hash and state root. Use the saved pre-roll.json after registration.
- [ ] deploy-v0 records both resident process hashes and manifest checks. All six rolled-vN.json files record restored watchdogs and convergence.
- [ ] Registration is confirmed closed on all six. The integrated registration hold regression passed and operators cleared outstanding signed rotations before final qualification.
- [ ] prove-roles verified both signatures before staging the fee key. The unsigned setup contains six current rotation clearances.
- [ ] Readback confirms the exact finalized tag 3. Tag-8 simulation.json confirms both account bodies.
- [ ] service-proof.log records a fresh verified proof under the installed identity. Require fresh closed payout readiness from the worker socket.
- [ ] The closed preflight confirms spendable fee reserve and 20,000 revision-8 atoms. The running flag is explicitly false and the authenticated public request returned the disabled response.
- [ ] open-accepted.json records six accepted receipts at the qualified parent's successor. Hold rotations through these receipts.
- [ ] The one original burn pays. settlement-before and settlement-check reconcile six receipts, the finalized payout reference and accounting before public enablement.

```bash
grep -Fxq 'service-proof verified' "$KIT/evidence/service-proof.log"
PYTHONPATH="$PWD/ops/sol-live" python3 -c 'import json; from exit_qualification_socket import qualify; print(json.dumps(qualify()))' | jq -e '.payout_ready and (.burns_open==false) and (.qualified_parent != null) and (.series_supply_atoms | type == "string") and (.series_supply_atoms == .bucket_outstanding_atoms) and all(.buckets[]; .series_supply_atoms == .outstanding_atoms) and (.relayer_lamports >= .relayer_floor_lamports + .payout_budget_lamports)'
```

### Staff and rehearse settlement before the first open

Name a settlement on-call, backup and independently reachable operator for each of the six resident signers. Record coverage, escalation contacts, expected payout volume and available custody margin in the cutover evidence. Each operator must complete a restore rehearsal on isolated state with test credentials. Exercise loss of that signer, fence the old instance, restore the authorized identity and its latest safety journals without duplicate signing, verify its committee identity and finalized parent, then complete a six-signature close and paid settlement. Record all six outcomes and who can perform each restore on call. Defer the first open if any signer, recovery path or settlement shift is unavailable.

## 8. Open burns in a separate decision (the first open is unanimous)

The frozen checker requires old-quorum V2 `signed_authorizations` in a supported single-update governance payload. Historical block 924 contains five Cobalt authorizations and zero V2 authorizations. That block predates the trusted anchor. A future Cobalt-only rotation after 1753 blocks payment across it. Readiness executes the frozen guest from the trusted committee, checks every segment endpoint and binds the resulting block hash, height and state root to six agreeing validators. It reuses previously verified finalized segments. An update at the tip requires a verified successor. Require the separately reviewed chain interlock in the exact scheduled build before opening. Drain pending burns before an incompatible change. A rotation format the frozen guest cannot verify requires a reviewed verifier repair. An HTTP flag cannot stop direct L1 burns after opening.

```bash
python3 ops/sol-live/exit_canary.py preflight --output "$KIT/canary"
python3 ops/sol-live/exit_govern.py open --revision "$REV" --release-height "$H" --output "$KIT/evidence/open-accepted.json"
jq -e '.height==(.qualified_parent.block_height+1) and (.validators|length)==6 and all(.validators[]; .receipt.accepted) and all(.routes[]; .burns_open)' "$KIT/evidence/open-accepted.json"
curl -fsS "$API/readiness/qualify" | tee "$KIT/evidence/open-readiness.json" | jq -e '.ready and .burns_open'
```

Qualification brackets the accounting reads with the same height, hash and root on six validators (five agreeing ones for a re-open). For every covered bucket of the shared custody, the route reports of its own series must show the full series supply equal to its bucket outstanding and the independently read source bucket. The separate custody test covers liabilities across all shared buckets, including queued redemptions. Missing full-supply reports keep opening closed. The wrapper checks the closed descriptor on six validators (on five agreeing ones for a re-open, when the route report shows `closed_since_registration`) and requests fresh qualification on /run/pf-sol-exit/qualify.sock. Systemd gives pf-sol-exit ownership of the directory. Open verifies the connected peer UID against the installed pf-sol-exit account before requesting a response. The loopback HTTP listener remains available to the relay. It restarts qualification when the agreed parent changes before submission. After an ambiguous submission inspect the receipts before another round. It invokes `ratify-solana-exit-open` on the resident node for the next proposal height. The first open obtains all six authorizations; a re-open after a close needs five, like the close. It submits a separate ordinary governance batch. Check the accepted receipts and open flags. At this point a direct L1 client can burn. Keep the public relay disabled.

## 9. Pay and settle one canary at the governed minimum

The canary burns exactly the governed minimum withdrawal that readiness reports (0.30 USDC by default), from the covered series with the most redeemable pfUSDC. Fund the test account with at least that much Solana-backed pfUSDC in one series first. Do not lower the minimum for the canary.

The source is StakeHub's encrypted b23-bob wallet under `~/.pft/wallets/4eca662d469907e4/` with passphrase at `~/.pft/b23-bob.passphrase`. The destination is `Ce5Yv528pKVrVq4JbQchZ8zasCAzxT3f9uCfatTZcfo8`. Preserve the same canary directory across retries.

```bash
python3 ops/sol-live/exit_canary.py prepare --output "$KIT/canary"
jq '{before,fields}' "$KIT/canary/plan.json"
python3 ops/sol-live/exit_canary.py burn --output "$KIT/canary"
python3 ops/sol-live/exit_canary.py check --output "$KIT/canary"
jq -e '.amount_atoms as $a | $a > 0 and .token_deltas=={"custody":-$a,"recipient":$a} and .withdrawal_packet_hash and .solana_tx' "$KIT/canary/paid.json"
```

The checker compares the signed envelope including fee and sequence, the six finalized burn confirmations, the permanent Solana marker, the transaction-local custody debit and ATA credit, and the 20,000-atom debit recorded by that accepted L1 burn. Later wallet credits do not affect this check. If a finalized burn already exists, rerun `check` in its original directory. After a signing or submission interruption run `python3 ops/sol-live/exit_canary.py recover --output "$KIT/canary"`, then `check`. Recovery verifies the saved ML-DSA envelope offline and looks up its exact ID on all six. If absent everywhere it submits those same bytes against a fresh, matching quote. Preserve the directory if the signed file is missing or incomplete. A paid marker with unavailable history proves payment; obtain the finalized transaction from an archival RPC before attesting settlement.

Save the six-way accounting baseline before settlement.

```bash
python3 ops/sol-live/exit_canary.py settlement-before --output "$KIT/canary"
```

Each validator operator independently reviews the original paid.json, finalized Solana transaction and L1 packet using the reviewed checkout, their trusted mainnet RPC, and the validator tunnels. The review binds the one successful tag 2 (ordinal zero), marker, accounts, signature, amount, slot and packet. On each signer host install that operator's verified JSON result at `/etc/postfiat/sol-exit-paid-review.json` as a root-owned regular file with mode 0644 or 0600. The paid command checks all six opened files for root ownership and rejects symlinks and group or world write access before any signer starts. Re-review on a new parent. The review command prints only the parent and expected amendment kind.

```bash
python3 ops/sol-live/exit_canary.py settlement-review --output "$KIT/canary"
python3 ops/sol-live/exit_govern.py paid --revision "$REV" --release-height "$H" --paid "$KIT/canary/paid.json" --output "$KIT/canary/paid-round.json"
PAID_ID=$(jq -er .tx_id "$KIT/canary/paid-round.json")
PAID_HEIGHT=$(jq -er .height "$KIT/canary/paid-round.json")
python3 ops/sol-live/exit_canary.py settlement-check --output "$KIT/canary" --amendment "$PAID_ID" --height "$PAID_HEIGHT"
jq -e '(.validators|length)==6 and .state.settled==.state.amount and .reference.ordinal==0' "$KIT/canary/settled.json"
```

The paid command discovers the running validator binary, validates its data directory and uses `ratify-solana-exit-paid` to draft from paid.json. It checks the exact packet, finalized slot, raw 64-byte signature and ordinal in the draft before any signing. Each resident signer requires its independent approval for that parent and payout. The wrapper takes each decision's signature count from the route report (`decision_signatures`, see `exit_rules.py`); a chain on the first release reports none, and then every decision needs all six. From the second release, paid and close need the chain's normal quorum (5 of 6): the wrapper continues when one validator is unavailable or has not approved, names the signers in its output and saved evidence, and stops if fewer than five sign or the height's proposer is unavailable. For close and paid, every read (fleet tip, Ethereum route, closed descriptor, payout reference and the receipt survey) also needs only five validators that agree, and names the unreachable ones. `accepted.json` is written before the receipt survey, so a validator lost after submission does not lose the evidence of a committed action. The first open stays unanimous; a re-open after a close needs the same five of six as the close, so one lost validator can neither block nor lock holders' exit. On an ambiguous submission, locate the accepted amendment and height on all six by the saved draft ID before retrying. Recover paid-round.json from the verified receipt rather than issuing a second attestation.

Require six accepted settlement receipts. On each validator verify that the original redemption is settled for its full amount (the governed minimum) with `settlement_receipt_hash` equal to the accepted amendment ID. Reconcile its bucket's queue, counted reserves and supply allocations against the saved baseline. The check accounts for intervening burns, other settled packets, newly counted receipts and newly issued supply allocations from the same bucket. Keep the original baseline across retries. An outstanding signature or accounting discrepancy keeps the public relay disabled.

## 10. Enable the public relay

Assign the settlement duty to the bridge operations on-call and the six resident validator signers before enabling public admission. The on-call reconciles each paid marker and pending L1 packet, obtains six independent attestations and verifies the queue, reserves and supply after every settlement. Use the paid CLI above only for the pinned canary. For other holders validators must independently check the actual owner, ATA, amount and ordinal and draft the corresponding paid amendment on their resident binary. Keep public admission disabled unless this duty is staffed and exercised for the expected traffic. The archived excess custody was 60,003 atoms. Four unsettled 20,000-atom payouts exceed it. On a backlog or missing signer follow Recovery immediately.

After the finalized payout and its verified L1 settlement, enable admission using the saved configuration. This command sets L1_SOLANA_WITHDRAWALS_ENABLED=true, restarts the reviewed unit and verifies its process settings. An authenticated GET and a JSON POST from the wallet's browser origin must each return HTTP 200, the original paid burn ID and the original finalized signature. Failure triggers the standalone disable procedure. If that procedure fails, block the public withdrawal path at the gateway and verify denial.

```bash
"$HOME/.local/lib/pf-sol-exit-relay/exit_relay.py" enable "$KIT/canary/paid.json" | tee "$KIT/evidence/public-paid-job.log"
```

## Before the second release height is set

### The second Solana withdrawal release: what it changes, what is signed, in which order

**What a user gets, from the release height H on:**
- Every Solana deposit can be withdrawn, from any Solana-source bucket of the shared custody, not only the revision-8 batch of 23 November.
- A withdrawal that can never be paid (for example, the destination token account is frozen or closed) can be cancelled on Solana by its destination owner or by the pause key. It is then refunded on Post Fiat to the burner, in the same asset and bucket, by a decision of five of six validators. A burn is never both paid and refunded.
- A minimum of 0.30 USDC per withdrawal, a governed value.
- The payout address is checked before the burn: its owner must be a wallet key.
- Five of six validators can close, pay, re-open and refund.

**What the owner signs, and with which key:**
- **Upgrade authority** `4JJEuaYtJvgtMVixTbXfppHJPh6tcU5g7g7UtkvxK3sB` (today on the relay host): the three steps of "Upgrade the initialised Solana verifier": the extension by 10,240 bytes (about 0.052 SOL), the upgrade (buffer and upgrade), and the migration (tag 9).
- **Pause key** `ow7wprz3MPEN15BGLyNvdJr5AUq4LNZK2ox1gY1QEJx`: later, and only after H, a cancel of one unpayable burn. The transaction is prepared and checked with `exit_cancel.py` ("Cancel a burn with the pause key"); the tool never holds the key.
- **Optional:** after the migration's read-back, move the upgrade authority to an offline key or a multisig (step 11 of the upgrade). Never `--final`.

**What each of the six validator operators installs:**
- the release build of `postfiat-node` with the same H, before H;
- the per-host reviewer, with that host's own Solana endpoint ("Independent settlement reviews").

**The order from today's state to live:**

| Step | Must be true first | How to go back |
| --- | --- | --- |
| 1. Merge the release; build `postfiat-node` and the verifier from the merge; replay the live chain | The replay ends in the live tip hash and state root. | Nothing is deployed yet. |
| 2. Install the per-host reviewer on the six hosts, one at a time | The relay's settler runs with `--single-owner-fleet`. On each host the release binaries and `/etc/postfiat` are owned by root. | Stop and disable the reviewer's timer; the first release does not use it. |
| 3. Upgrade the verifier: extend, upgrade, migrate | The payout worker is stopped and admission disabled. The on-chain upgrade authority equals the signer. | Before the migration: upgrade back to the deployed build (`3b16fc30…`). After it: only the new build again. |
| 4. Pin `VERIFIER_SHA256` to the read-back hash and restart the worker | The pinned hash equals the chain's. | Stop the worker. |
| 5. Set H in source; build the release; install it on all six validators | Every validator reports the release build, and H is far enough ahead for all six. | Before H, return to the first release's build: nothing has changed on the chain. |
| 6. Reach H | Every paid burn is settled, and the settler is stopped just before H. | None after H: the rules are on the chain. Close withdrawals with five of six. |
| 7. Run from H | The relay runs `exit_govern.py paid` and `refund`; each host signs from its own review. | Close withdrawals with five of six. |

**Numbers:**
- **Verifier after the upgrade:** 123,152 bytes, SHA-256 `4091ee16db29154e7aae642b637320c0a0d0cdee110da88d07416ab7b1c15b21`. It fits the deployed 116,928 bytes plus the 10,240-byte extension. The deployed program today: 116,928 bytes, `3b16fc309501814e0bba718ecadb80f02993f6f355153a13f37ae9d47d29354c`.
- **Node:** the replayed release build of `postfiat-node` has SHA-256 `b7015cda92dd94aa510e25a948c43c9700f10b80d0ea0b45ac815cdefb293ac8`. With H unset it reproduced the live chain block for block to the live tip and state root. No node code has changed since that build.

**Not in this release:**
- **Paying across a validator-set rotation.** A burn in the block after a rotation waits, with a named status, until a later checkpoint makes it payable.
- **A user-facing cancel.** Users have no button or command for it; only the destination owner's own Solana signature or the pause key, through `exit_cancel.py`.
- **A cancel made through another program.** Refund drafting reads only top-level instructions, so such a cancel is not found.

### Checklist

H (`SECOND_RELEASES` in `crates/types/src/solana_exit.rs`) is a source constant today, not a governed value. Every validator must run a build with the same H before H, or it diverges at the first block that uses the second release's rules.

- [ ] Every validator's status read reports the second-release build (`build_git_revision`).
- [ ] The Solana verifier is upgraded and its hash is pinned (`VERIFIER_SHA256` in `exit_setup.py`).
- [ ] The tools report the second release's rules from the chain's route report.
- [ ] A replay of the live chain with the build passes and matches the live state root.
- [ ] The per-host settlement reviewer runs on all six hosts and approved the latest payout on each (see "Independent settlement reviews").

A governed or interlocked activation would need validators to ratify H only from the second-release build, the code to read the stored activation instead of the constant, and ratification to be refused while any active validator reports another build.

### Independent settlement reviews

Under the second release, five of six signatures decide paid, close, re-open and refund. A signature is only as good as the review behind it. On each validator host, a review is independent when all of the following hold:
- **Where it runs:** on that host, as its own user `pf-sol-exit-review`.
- **What it reads:**
  - the burn and its redemption from that host's own node;
  - Solana through an endpoint configured on that host. Every verdict names that endpoint's host, never the full URL.
- **What it derives itself:** each address (marker, light, associated token account).
- **Which transaction it checks:** the one that created the marker by the verifier's own rule.
  - That is the successful transaction whose verifier instruction names the marker; the program creates a marker once.
  - The reviewer pages the marker's whole history to find it, up to 20,000 transactions.
  - Once a review approves, the reviewer remembers that transaction's signature (`/var/lib/postfiat-exit-review/writers/`) and later reviews check it again directly, so transactions that name the marker afterwards cannot push it out of reach.
  - Lamport transfers to the address and transactions that only name it are ignored, as the program ignores a prefund.
- **What it checks:**
  - the redemption is pending, unsettled and in a covered bucket;
  - the destination is canonical;
  - the marker's byte (1 paid, 2 cancelled) at finalized;
  - exactly one payout or cancel instruction of the verifier in that transaction;
  - the proof's public values, rebuilt from the buffer's own uploads before the payout, match the burn (burn, amount, destination, bucket, family, chain tag);
  - for a payout, the mint and the exact custody and recipient amounts.
- **How its verdict is kept:**
  - The verdict names the decision's SHA-256 and is sealed with the reviewer's Ed25519 key.
  - The keypair is readable only by `pf-sol-exit-review`.
  - The signing step reads only the public key, so it can check a verdict but not make one.
  - The signing step trusts the public key and the config only when they and `/etc/postfiat` are owned by root and writable by no one else; otherwise it refuses by name, because the node user could then replace the key or point the config at its own verdicts.
- **How it signs:** `exit_review.py sign --amendment-file F --proposal-slot H --expires-at-height E --authorization-file A` signs only a decision approved by a verdict at most 15 minutes old.
  - It builds the node's signing command itself: the resident binary, this host's data directory, validator name and key file.
  - The resident binary is the executable the kernel runs for the node user's `transport-validator-serve` process of this data directory (`/proc/PID/exe`), never a name taken from a command line. It must lie in `/opt/postfiat/releases/<release>/postfiat-node`, and the binary, its release directory and `/opt/postfiat/releases` must be owned by root and writable by no one else; otherwise, or when there is not exactly one such process, the step refuses. Check this on each host before the height: `stat -c '%U %a %n' /opt/postfiat/releases /opt/postfiat/releases/*/ /opt/postfiat/releases/*/postfiat-node` prints `root` and no group or other write bit.
  - It takes no command from its caller, and refuses any other argument.
  - It checks the amendment file's hash before and after the signing command, and removes the authorization if the file changed.
- **Exit codes:** 0 approved; 2 refused with a named reason; 3 an RPC error or a reply of an unexpected shape, named in one line; 4 a local file it cannot read. In `scan`, one failing burn stops neither the other burns' reviews nor the clearance. Verdicts older than a day are removed.

`govern.py` runs `exit_review.py sign` on each host for paid and refund under the second release. A host whose reviewer refused is printed with its reason as "refused by its own reviewer", apart from unreachable hosts, even when the other five reach the quorum.

`exit_review.py show-clearance` gives that host's own sealed rotation clearance for an opening or re-opening. A first-release chain keeps the go-live review files and six signatures. The go-live tools `operator/settle.py` (now only with `--single-owner-fleet`) and `operator/clearance.py` write those files from the relay host, and the reviewer replaces them. Update `settle.py` and `settler.sh` together: without the flag `settle.py` exits and prints the command to run, and `settler.sh` prints an `ALERT` line for any failed settle.

**Install, one host at a time.** Set `COMMIT` (the reviewed commit), `HOST`, `VALIDATOR` (`validator-0` to `validator-5`), `PORT` (that node's RPC port) and `URL` (that host's own Solana RPC URL). Copy the archive and the script to the host, then run the script there as root:

```bash
git archive -o exit-review.tar "$COMMIT" ops/sol-live
scp exit-review.tar ops/sol-live/install-exit-review.sh "root@$HOST:/root/"
ssh "root@$HOST" bash /root/install-exit-review.sh "$VALIDATOR" "$PORT" "$URL" /root/exit-review.tar
```

The script needs Python 3.10 or newer and `setfacl` (the `acl` package). Any failed step stops it with its reason. It installs:
- the reviewer and its venv, from the hash-pinned `exit-requirements.lock` with `--require-hashes`;
- the user `pf-sol-exit-review`, which alone owns the key (mode 0600), with `postfiat` in its group so the node user can read the verdicts and the config (mode 0640);
- the public key, readable by all;
- read access for the reviewer to the names in the node's `consensus-v2-safety` directory and nothing else of the node;
- the timer `pft-sol-exit-review.timer`, every minute.

The script runs the first review and stops if it ends in an RPC or access error.

**The settlement loop's restricted door.** Add `--settler-key FILE` (the loop's public key, printed by `install-exit-settler.sh`) to the install above. The script then writes one `restrict,command="…exit_review_door.py"` line into the reviewer user's `authorized_keys` and enables `pft-sol-exit-sign-door.path`. The door reads one JSON request on stdin. It accepts exactly two: hand in a drafted paid or refund decision for this host's own review, and return this host's result. The result is its signature, its refusal with the reason, or pending. It runs no command, names no file and gives no shell, transfer or forwarding. The node user signs a handed-in decision only from this host's own fresh verdict, as `exit_review.py sign` does (`exit_review.py sign-door`). What this changes: `exit_govern.py paid --restricted` collects every host's signature through the doors and never logs in as root to sign. With fewer than five signatures nothing is settled. Drafting, assembling and the proposer's batch submission still run through `govern.py`'s root login (see "What stays open").

**Check the six providers.** Use at least three different Solana providers across the six hosts. With `HOSTS` set to the six hosts, this prints each host's provider (host name only) and warns when fewer than three differ:

```bash
for h in $HOSTS; do ssh "root@$h" cat /etc/postfiat/sol-exit-review.json; echo; done | python3 ops/sol-live/exit_review.py endpoints
```

**What stays open.**
- **Root ssh from the relay.** The relay host still reaches every validator host as root over ssh (`govern.py`, `remote`). Root can run the node's signing command with the validator key directly, or replace the reviewer, so a compromised relay host can still sign on all six. The restricted door removes root from the signatures (`--restricted`). It does not remove root from three other steps:
  - the lead host drafts the decision with its node's data directory;
  - the lead host assembles it the same way;
  - the proposer submits the batch round with its validator key.

  Root ssh for settlement can go only once those three have a restricted way as well. Keep each validator key readable only by the node user.
- **Shared providers.** Independence also needs each host's Solana endpoint to be a different provider; the check above only warns.

**Order, so that no payout waits unsettled** (each payout blocks the next until it is settled):
1. While the chain runs the first release, install the reviewer on all six hosts, one at a time. The relay's settler keeps running with `--single-owner-fleet`; the reviewers only write verdicts.
2. For the next payout, confirm on each host that `/var/lib/postfiat-exit-review/<sha256 of the paid decision>.json` approves the same decision the settler signed.
3. Just before the second release height, let the settler settle every paid burn, then stop it.
4. From the height, the relay runs `exit_govern.py paid` (and `refund`). It only drafts and collects signatures; each host signs from its own review.

### Settlement loop as a service

Today the relay's settlement loop is the tmux session `settler`, running `oncall/settler.sh` every 90 s. That loop settles through `exit_govern.py paid --burn` and the root login. The installed loop is `pft-sol-exit-settler.timer`. It runs one pass of `exit_settler.py` every 90 s, as its own user `pft-sol-exit-settler`. A pass works like this:
- It holds a lock, so two passes never run at once.
- It finds burns to settle through any answering validator.
- It proposes each one with `exit_govern.py paid --burn B --restricted`, bounded to 900 s.
- A burn that keeps failing waits from 90 s, doubling up to an hour. It raises one `ALERT` line when it starts failing and one when the loop gives up after 12 attempts; after that, settle it by hand.
- Every failure is a journal line: `journalctl -u pft-sol-exit-settler`.

The loop only proposes. Each host's reviewer signs through its door, and with fewer than five signatures nothing settles.

**Install on the relay machine, as root there:**

```bash
git archive -o exit-settler.tar "$COMMIT" ops/sol-live solana/deploy
bash ops/sol-live/install-exit-settler.sh exit-settler.tar /home/postfiat/.ssh/known_hosts
```

The script:
- creates the user and its SSH key (mode 0600, never printed) and `/etc/postfiat/sol-exit-settler.env`, where the Solana RPC URL goes;
- prints the public key and the line each validator's `install-exit-review.sh … --settler-key FILE` writes;
- then asks all six doors a read-only request.

It enables nothing while a host does not answer; it names that host and exits 3. It also enables nothing while drafting, assembling and submitting still need a root login; it says so and exits 4. Until it enables the timer, keep the tmux loop. When it does, stop the tmux loop first, then start the timer:

```bash
tmux send-keys -t settler C-c && tmux kill-session -t settler   # as the relay's on-call login
systemctl enable --now pft-sol-exit-settler.timer
```

### Upgrade the initialised Solana verifier

The deployed verifier (`0x3b16fc30…`, byte-identical to `solana/deploy/build.sh` of `main`) stores in its light account a route that also binds the revision-8 bucket. The new program computes the custody route without it, so the light's `route` (bytes 136–168 of 289) must change once. Instruction tag 9 does this. Its arguments must rebuild the stored route, so the new route is fixed by the account. It runs once and leaves the checkpoint, the pause state, the checkpoint accounts and every paid marker unchanged. `solana/pfusdc-egress-verifier/tests/upgrade_rehearsal.py` rehearses these steps on the dumped mainnet state. The same build adds tag 10, `cancel` ([refund design](../../docs/specs/solana-exit-refund-design-20261124.md)). It changes no stored data and needs no step of its own. At 122,976 bytes, the image still fits in the 10,240-byte extension.

The L1 needs no close. The first release keeps running and revision-8 burns stay open. The new build can cancel from the upgrade on, but the pause key signs no cancel until the second release height is active (see "Cancel a burn with the pause key"). From the upgrade until the new service runs, the verifier or the old service refuses every payout, and burns wait unpaid. Stop the payout worker and disable public admission for that window. Revision-9 exits start only at the second release height.

**Before each signing step (4, 5, 6, 8):** read ProgramData `D7YLTGrE…` and confirm that its upgrade authority (bytes 13–45, option byte 12 = 1) equals the key about to sign, `4JJEua…`. Record the transaction signature of every step in `$KIT/evidence/upgrade-signatures.txt`.

**No authority change:** do not run `solana program set-upgrade-authority`, and never `--final`, until step 8 has been read back. Tag 9 reads the current upgrade authority, so removing it before the migration locks withdrawals for good.

| Step | Signer | Check |
| --- | --- | --- |
| 1. Build the reviewed commit twice with `solana/deploy/build.sh` (platform-tools v1.58) | none | Both builds equal; record `pfusdc_egress_verifier.so` SHA-256. A second person rebuilds and compares. |
| 2. Read the deployed state | none | Program code `0x3b16fc30…`; light `FQe34dCQ…` holds route `84b17839…c767`, height and `paused` as recorded. |
| 3. Stop the payout worker, disable admission | operator | The disable procedure in Recovery. |
| 4. `solana program extend 8Dxp8bfgEh6ankapfBdkTds5cebqrtPdFxpZSP6JsGAF 10240` | upgrade authority `4JJEua…` (with `--payer`) | Read back the ProgramData length: 127,213 bytes (45 + 116,928 + 10,240). The loader refuses less than 10,240 bytes; about 0.052 SOL rent. |
| 5. `solana program write-buffer pfusdc_egress_verifier.so --buffer-authority <upgrade authority>` | payer, upgrade authority | `solana program dump <buffer>` equals the build. |
| 6. `solana program upgrade <buffer> 8Dxp8bfg…` | upgrade authority | The buffer's rent returns to the spill account. |
| 7. Read back the program | none | The first bytes equal the build and the rest of ProgramData is zero. Note the SHA-256 of the whole ProgramData tail (`data[45:]`); `profile_code` hashes exactly that. On a local Agave 4.4.1 validator the tail was exactly the image. |
| 8. Send tag 9 (below) | upgrade authority | Tag 9 takes exactly three accounts: the upgrade authority (signer), the light `FQe34dCQ…` (writable) and ProgramData `D7YLTGrE…` (read-only). Read back the light: only bytes 136–168 differ from step 2. They now hold `02b02d4b…558e` (`tests/route-vector.txt`); `height` and `paused` are unchanged. |
| 9. Set `VERIFIER_SHA256` in `exit_setup.py` to the step-7 hash, then install and self-test the service build | operator | Before the restart, the installed service's `VERIFIER_SHA256` equals the SHA-256 of the ProgramData tail read now from the chain. |
| 10. Start the worker, re-enable admission | operator | Pending revision-8 burns are paid. |
| 11. Optional, after the step-8 read-back: `solana program set-upgrade-authority 8Dxp8bfg… --new-upgrade-authority <offline key or multisig>` | upgrade authority | Never `--final`. The service pins the authority (`profile_code` in `exit_setup.py` requires `MAINNET_PAYER`), so that pin changes in the same reviewed release; until then payouts stop. |

```bash
python3 solana/pfusdc-egress-verifier/bootstrap.py --migrate "$KIT/setup.json" "$KIT/evidence/migrate-args.bin"
PYTHONPATH="$PWD/ops/sol-live" python3 - <<'PY'
import os
from pathlib import Path
from exit_setup import DIRECTORY, ExitCluster, Instruction, LIGHT, LOADER, MAINNET_PAYER, VERIFIER, pda, ro, rw
c = ExitCluster(MAINNET_PAYER)
c.unlock(DIRECTORY / "mainnet-payer.json")
data = Path(os.environ["KIT"], "evidence/migrate-args.bin").read_bytes()
programdata = pda(LOADER, bytes(VERIFIER))
print(c.send("exit.migrate", [Instruction(VERIFIER, b"\x09" + data, [ro(MAINNET_PAYER, True), rw(LIGHT), ro(programdata)])]))
PY
```

`$KIT/setup.json` is the configuration of the 23 November initialisation. Its route equals the one built from `ops/sol-live/fixtures/exit-setup.json`. With any other configuration, tag 9 refuses with `Binding`. A second run also refuses with `Binding`, and a signer other than the upgrade authority is refused.

If a step fails:
- **Step 4.** An extension is permanent. Read the ProgramData length first; repeat only if it is still 116,973 bytes, because a repeat extends and charges again.
- **Step 5.** A half-written buffer stays funded. Resume it with `solana program write-buffer … --buffer <buffer key>`, or close it with `solana program close <buffer key>` to recover the rent.
- **Step 6.** The upgrade is atomic: either the new code is in place (step 7 shows it) or nothing changed. Repeat with the same buffer.
- **Step 8.** The account keeps the old route, and every withdrawal refuses with `Binding`. Fix the arguments and repeat. To go back instead, upgrade to the `main` build (`0x3b16fc30…`), which works on the unmigrated account.
- **After a successful migration.** The old program refuses every withdrawal with `Binding`, which is safe but stops payouts. To recover, upgrade to the new build again. No instruction restores the old route.

## Recovery

On signer loss or a settlement backlog, disable public admission immediately by running `"$HOME/.local/bin/disable-sol-exit"` on the relay host. Confirm its effective false flag and the authenticated disabled-job response. If the relay cannot be controlled, block the public withdrawal path at the gateway and verify denial there. Preserve the original burn IDs, pending claims, journals and finalized payment references. Page the settlement on-call and all signer operators. Direct burns remain possible until a close is committed, because five validators can finalize them. Five can also close direct L1 admission with a separate decision, and the same five can re-open it after that close, so a lost sixth signer neither blocks nor locks holders' exit. Track new burns and outstanding custody continuously. Restore the sixth signer using its rehearsed recovery path, fence any old signer before resuming, and verify the restored committee identity, safety history and agreed parent. Pay and settle all pending claims using their original packets, and reconcile custody before re-opening. Preserve evidence and wait for accepted receipts and closed descriptors on at least five agreeing validators; the wrapper names any validator that did not answer.

```bash
source "$HOME/.config/postfiat/sol-exit-oncall.env"
python3 ops/sol-live/exit_govern.py close --revision "$REV" --release-height "$H" --output "$KIT/evidence/close-accepted.json"
```

Disabling public admission or pausing Solana leaves finalized L1 burns pending. Keep the replay-capable validator release, restore payout and reuse the original burn ID. The worker records the payout signature and its expiry before broadcast and reconciles the finalized marker on restart. To rebuild a damaged SQLite journal, stop the payout unit, use the matching installed runtime, and restart only after recovery succeeds.

```bash
sudo -n /usr/bin/systemctl stop pft-sol-mainnet-withdraw.service
# Machine owner, after the operator confirms the worker is stopped:
sudo -u pf-sol-exit /opt/postfiat/sol-exit/venv/bin/python -I /opt/postfiat/sol-exit/ops/sol-live/exit_rebuild.py
sudo -n /usr/bin/systemctl start pft-sol-mainnet-withdraw.service
```

A rebuild checks L1 redemptions and finalized Solana markers, retains the old database and its SQLite sidecars, and atomically selects the recovered database through `jobs.current`. Pending jobs wait for potentially in-flight blockhashes to expire before resuming. Keep buffer keys whose on-chain rent has yet to be reclaimed.

For an emergency Solana pause the offline operator uses the dedicated pause signer. On the pause host set PAUSE_CHECKOUT to the absolute path of the clean reviewed release checkout and RELEASE_COMMIT to its approved full commit ID. Use flag 0 to resume only after repair and review.

```bash
set -euo pipefail
cd -- "${PAUSE_CHECKOUT:?path to reviewed checkout on the pause host}"
test "$(git rev-parse HEAD)" = "${RELEASE_COMMIT:?approved full release commit}"
git diff --quiet HEAD --
PYTHONPATH="$PWD/ops/sol-live" python3 - <<'PY'
from exit_setup import DIRECTORY, ExitCluster, Instruction, LIGHT, MAINNET_PAYER, PAUSE, VERIFIER, load, require, ro, rw
c=ExitCluster(MAINNET_PAYER)
c.unlock(DIRECTORY/"mainnet-payer.json")
admin=load(DIRECTORY/"exit-pause-authority.json")
require(admin.pubkey()==PAUSE,"Pause signer mismatch.")
flag=1
print(c.send("exit.pause",[Instruction(VERIFIER,bytes([4,flag]),[ro(PAUSE,True),rw(LIGHT)])],admin))
assert c.account(LIGHT)["data"][-1]==flag
PY
```

### Cancel a burn with the pause key

The verifier lets two parties cancel a proven, unpaid burn: its destination owner, or the pause authority. Nobody else can. A cancel moves no funds; it makes the burn refundable on Post Fiat instead of payable on Solana.

**When:**
- **Who asks:** the holder (through support) or the settlement on-call. Never on a stranger's request.
- **Which burns:** a burn whose destination can never receive, for example a frozen or closed token account, a program-owned owner, or a mint rule change.
- **Not before the second release height is active,** that is, before five agreeing validators report `exit_release` 2. Before that the chain has no refund, and a cancelled burn could be neither paid nor refunded. No user-facing cancel exists before then.

**Check first** (two people):
1. The burn is unpayable: its job shows a named refusal (`refused: destination not a wallet …`), or the destination check refuses it now.
2. The burn is not in flight: its job is not `paying`, the journal holds no unexpired attempt signature, and the marker account does not exist at finalized.
3. The worker is stopped for this burn until the cancel is final.

**Then,** with `ops/sol-live/exit_cancel.py`, which never holds or reads the pause key:
1. **Prepare.** On the relay, stop the worker for this burn, then run `prepare`. It refuses unless the redemption is pending and unsettled, the marker does not exist at finalized, and no payout is in flight: neither the job paying, nor a recorded attempt whose block height has not passed. It checks that the worker's proof files are for this burn. It writes `unsigned.hex`, plus `summary.txt` and `summary.json`: the burn, amount, destination owner, marker address, program, the signer that must sign (the pause authority), the values buffer, the message hash and the last valid block height.
2. **Sign.** On the offline pause host, the holder signs `unsigned.hex` with the pause key, before the last valid block height in the summary. Past that height, prepare again.
3. **Verify.** On the relay, `verify` checks that the signed transaction is the prepared message, signed by the pause authority alone.
4. **Send.** `send` refuses unless five agreeing validators report the second release. It re-checks the burn and the marker, uploads the burn's public values, and sends the transaction. Read back the marker: byte 2 at finalized. Record the signature, then follow "Refund a cancelled burn" below.

```bash
python3 ops/sol-live/exit_cancel.py prepare --burn "$BURN" --proof-dir "$EXIT_WORK/jobs/$BURN/<witness hash>" --journal "$EXIT_WORK/jobs.sqlite" --out "$KIT/cancel-$BURN"
python3 ops/sol-live/exit_cancel.py verify --out "$KIT/cancel-$BURN" --signed "$KIT/cancel-$BURN/signed.hex"
python3 ops/sol-live/exit_cancel.py send --out "$KIT/cancel-$BURN" --signed "$KIT/cancel-$BURN/signed.hex"
```

### Refund a cancelled burn

1. On every validator host, the reviewer's timer finds the burn and writes that host's refund verdict. The reviewer reads the marker through that host's own Solana endpoint at finalized and derives its address from the burn ("Independent settlement reviews"). Confirm each verdict in `/var/lib/postfiat-exit-review/`.
2. The burner must still hold a trust line for the burned series. If they removed it, ask them to re-create it; until then the refund is refused, and it can be repeated afterwards.
3. On the relay, run the command below. It drafts the decision from the finalized cancelled marker, and each host signs only from its own review; five of six are needed.
4. Read back on five agreeing validators: the redemption's state is `refunded`, and the burner's balance in the series rose by the amount.

```bash
python3 ops/sol-live/exit_govern.py refund --revision "$REV" --release-height "$H" --burn "$BURN" --output "$KIT/evidence/refund-$BURN.json"
```

The seven earlier Solana series retain their own liabilities. The exit pays burns from every bucket of this shared custody whose ID derives from its own revision's policy, and the custody must cover all of them; moving liabilities between series requires separate governance and conservation tests for outstanding amounts, queued redemptions, counted reserves and allocations.

## One proving machine

The withdrawal prover can share the four-GPU deposit machine instead of renting its own. None of this has been run on a GPU yet; check it on the machine before moving payouts.

- **Why it collided.** SP1 picks the device, not our code: without a device id, sp1-sdk 6.3.1 connects to device 0 and sp1-cuda 6.3.1 starts `~/.sp1/bin/sp1-gpu-server` with `CUDA_VISIBLE_DEVICES=<id>` behind `/tmp/sp1-cuda-<id>.sock`. The deposit prover takes devices 0 to 3, one server and socket each.
- **Split.** One settings file, `/etc/pf-sol-exit-prove.conf` (root's, mode 644), gives both: `DEPOSIT_CUDA_DEVICES=0,1,2` for the deposit prover and `EXIT_CUDA_DEVICE=3` for the withdrawal prover. Each start refuses an overlap, and the deposit start refuses a `SOL_DEPOSIT_GPUS` that contradicts the file. Each prover keeps its own lock, work tree and SP1 home; no port is involved.
- **Memory.** One RTX 5090 (32 GB) has proven both programs; neither peak is measured. A whole GPU each, so they never share GPU memory. Host memory and CPU (32 + 4 threads) are shared and not measured. Three-GPU deposit proofs are slower; not measured.
- **Proof host.** Build `ops/withdraw/proof_host.rs` with this change; `--cuda-device N` or `PROOF_HOST_CUDA_DEVICE=N` picks the device, default 0. It refuses a device whose socket answers. The forced command passes only the file's device.
- **Stale servers.** A GPU server outlives a killed proof host (timeout, out of memory). A server is a leftover when its parent is not a live proof host, that is neither `proof-host` (the withdrawal prover) nor `host-live` (a deposit prover): init or a subreaper took it. Holding its lock, the forced command stops a leftover on its device and refuses a server of a live proof host. The deposit start stops leftovers on any device and never a server of a live proof host: on one of its own devices it refuses and names the device and the process, for example the withdrawal prover's when the settings file is missing. Write device numbers without a leading zero (`3`, not `03`); both scripts refuse any other form.
- **One user.** Both provers run as the same user. Each reads the other's GPU servers' environment in `/proc` to find their device; a server of another user cannot be read, is skipped, and is neither stopped nor refused.

On the four-GPU machine, as root, with the staged tree from `copy-exit-prover.sh`:

```bash
printf '%s\n' DEPOSIT_CUDA_DEVICES=0,1,2 EXIT_CUDA_DEVICE=3 EXIT_WORK_DIR=/workspace/exit/b23-withdraw-20261116 \
    EXIT_SP1_HOME=/workspace/exit/home > /etc/pf-sol-exit-prove.conf
chown root:root /etc/pf-sol-exit-prove.conf; chmod 644 /etc/pf-sol-exit-prove.conf
bash ops/sol-live/prover-start.sh
mkdir -p /workspace/b23-gpu
install -m755 exit-prover-command.sh /usr/local/bin/pf-sol-exit-prove
```

The payout worker starts the withdrawal prover itself through `pf-sol-exit-prove`; by hand: `cd /workspace/exit/b23-withdraw-20261116 && HOME=/workspace/exit/home SP1_PROVER=cuda RAYON_NUM_THREADS=4 ./proof-host --cuda-device 3 withdrawal egress.elf exit-witness.json /tmp/w prove`.

Check: `stat -c '%U %a' /etc/pf-sol-exit-prove.conf; cat /etc/pf-sol-exit-prove.conf; ls -l /tmp/sp1-cuda-*.sock; ss -xlp | grep sp1-cuda; for p in $(pgrep -x sp1-gpu-server); do echo "$p parent $(awk '{print $4}' /proc/$p/stat) $(tr '\0' '\n' </proc/$p/environ | grep CUDA_VISIBLE)"; done; nvidia-smi --query-compute-apps=pid,gpu_bus_id,used_memory --format=csv; nvidia-smi --query-gpu=index,pci.bus_id --format=csv`. Sockets 0 to 2 belong to the deposit servers, socket 3 only exists during a withdrawal proof.

Moving to one machine also changes the payout worker's SSH target (`HOST`, `PORT` in `exit_proof.py`, `authorize-exit-prover.sh`, `exit-prover.fingerprint`) and, if the bundle ships the rebuilt binary, the pinned proof-host hash (`install-exit-user.sh`, `build-exit-bundle.py`). To go back to two machines: restore the SSH target, delete `/etc/pf-sol-exit-prove.conf` on the deposit machine, and restart the deposit prover; without the file it takes all four devices again.
