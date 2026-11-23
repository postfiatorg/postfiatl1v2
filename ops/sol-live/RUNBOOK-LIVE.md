# Solana pfUSDC exit cutover

Orchestrator only. Use one Bash shell on the orchestrator and trusted relay host. Set REL, CHAIN_FIX and OPS_FIX to the approved release name and reviewed local commit IDs. The public gateway URL, its existing session cookie jar and six operator reviews are deployment inputs. Keep the public relay flag explicitly disabled through the real payout and its L1 settlement. Starting at step 8, direct L1 clients can burn. The immutable Solana binding supports only the revision-8 source bucket.

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
  CARGO_TARGET_DIR="$KIT/interlock-target" cargo test --locked --release -p postfiat-node --lib --no-run --message-format=json > "$KIT/evidence/interlock-build.jsonl"
)
testbin=$(jq -r 'select(.target.name=="postfiat_node" and .executable)|.executable' "$KIT/evidence/interlock-build.jsonl")
for case in exit_review_open_checks_checkpoint_history exit_review_close_drain_rotate_and_reopen exit_review_registration_holds_rotation_until_open_or_retired; do
  docker run --rm --network none --cap-drop ALL --security-opt no-new-privileges -u "$(id -u):$(id -g)" \
    -v "$ARCHIVE:/var/lib/postfiat/validator-3" -v "$FAUCET:/var/lib/postfiat/validator-3/faucet_account.json:ro" -v "$INTEGRITY:/var/lib/postfiat/validator-3/.integrity.key:ro" \
    -v "$testbin:/opt/test:ro" ubuntu:24.04 /opt/test "tests::solana_exit::$case" --ignored --exact | tee "$KIT/evidence/$case.log"
  grep -Fq '1 passed; 0 failed' "$KIT/evidence/$case.log"
done
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

On each authenticated validator host the operator publishes a root-owned `/etc/postfiat/sol-exit-rotation-clearance.json` recording `validator` (validator-N), `profile_hash` (the registered profile hash), `revision` (resident build), `parent` (block_height, block_tip_hash and state_root), `hold_through_open` (true) and `outstanding_signed_rotation_rounds` (the actual list). Missing evidence or any listed round blocks qualification. A changed parent requires a new inventory.

## 4. Precheck the immutable binding

```bash
setup_args=(--binary "$B" --archive "$ARCHIVE" --integrity-file "$INTEGRITY" --faucet-file "$FAUCET")
mkdir -p "$KIT/evidence/solana"
python3 ops/sol-live/exit_setup.py prove-roles --output "$KIT/evidence/solana/roles.json"
python3 ops/sol-live/exit_setup.py "${setup_args[@]}" --qualify --output "$KIT/evidence/solana/unsigned.json"
jq -e '.guest.forward and .guest.start==.config.initial_committee and .simulation.value.err==null' "$KIT/evidence/solana/unsigned.json"
```

Setup re-verifies the archived certificate and compares the authenticated committee, accepted ingress record and bucket across six validators. It checks finalized Solana vault config, custody, and USDC mint at one slot, deployed programs and loader authorities, executes the frozen checkpoint guest on every selected segment through the six-way finalized tip, and checks both simulated PDA bodies. At a stationary 1753 tip the guest can prove ancestry ending there. Wait for a later finalized block before signing.

The tag-3 instruction is 317 bytes. Its SHA-256 is `bb32524804b64a9d7d40fec86c9735f8c1f5d5d03d7a8a6963e8c21f83903fe4`. The dedicated pause authority is `ow7wprz3MPEN15BGLyNvdJr5AUq4LNZK2ox1gY1QEJx`. Its signing file stays offline from the payout service. Review the instruction, ordered accounts and simulated bindings in `unsigned.json`.

Fund the dedicated fee payer once. Preserve the journal at `~/var/sol-exit/funding-attempt.json` through retries and releases. This command persists the signature, blockhash and expiry before sending 0.10 SOL. After a timeout rerun it with the same journal and an RPC serving finalized transaction history. A pending signature blocks a second transfer.

```bash
python3 ops/sol-live/exit_fund.py --execute
```

The funding recipient is the operational fee payer `8EiSLnZQv23HtA5adokE6xfKyCK6mf4HGkpxSPpop1fs`. Only the pause authority is encoded in tag 3. Both role-specific signatures cover the reviewed instruction digest and a fresh random challenge. Readiness quotes peak proof-buffer, checkpoint, marker and ATA rents and transaction fees above the 20,000,000-lamport floor. All Solana buckets sharing this custody must remain covered, including queued redemptions pending L1 settlement.

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
tag8=(--guest "$KIT/evidence/solana/guest" --work "$KIT/evidence/tag8")
python3 ops/sol-live/exit_tag8.py stage "${tag8[@]}"
python3 ops/sol-live/exit_tag8.py simulate "${tag8[@]}"
jq -e '.value.err==null' "$KIT/evidence/tag8/simulation.json"
python3 ops/sol-live/exit_tag8.py reclaim "${tag8[@]}"
```

If the current checkpoint is already the agreed tip, wait for a newer finalized block and run readback again. Staging obtains the Groth16 proof and signs only the buffer allocation and uploads. Simulation sends an unsigned tag 8 to `simulateTransaction` and checks its returned light and checkpoint bytes. Reclaim the buffer rent after successful simulation. Keep burns closed if any check fails.

## 7. Install the isolated payout service

Install a **system** service under `pf-sol-exit`. Keep the code, pinned ELF and proof host root-owned. Provision a separate SSH identity for this service account on the existing prover and verify its host key out of band. Its home must permit batch SSH to the configured prover and the remote account must support the shared GPU lock. Keep the deployment account's SSH and Solana credentials out of the service home.

```bash
app=/opt/postfiat/sol-exit
svc=/var/lib/pf-sol-exit
id pf-sol-exit >/dev/null 2>&1 || sudo useradd --system --user-group --home-dir "$svc" --shell /usr/sbin/nologin pf-sol-exit
sudo install -d -m700 -o pf-sol-exit -g pf-sol-exit "$svc"
git archive HEAD ops/sol-live solana/deploy solana/pfusdc-egress-verifier/bootstrap.py crates/types/src/solana_exit_profile.json |
  sudo bash ops/sol-live/install-exit-runtime.sh "$app" "$HOME/repos/b23-coord/evidence/withdraw" "$KIT/evidence/solana/anchor-witness.json"
sudo -u pf-sol-exit env HOME="$svc" PYTHONPATH="$app/ops/sol-live" EXIT_PROOF_HOME="$app/proof" \
  "$app/venv/bin/python" -c 'import exit_service; from exit_proof import verify_local_host; verify_local_host()'
fee="$HOME/.config/postfiat/sol-bridge/exit-fee-payer.json"
test -f "$fee"
test ! -L "$fee"
test "$(stat -c %a "$fee")" = 600
test "$(stat -c %d "$fee")" = "$(stat -c %d "$svc")"
sudo test ! -e "$svc/fee-payer.json"
sudo mv -T "$fee" "$svc/fee-payer.json"
sudo chown pf-sol-exit:pf-sol-exit "$svc/fee-payer.json"
test -z "$(ss -H -ltn 'sport = :18089')"
sudo install -m644 ops/sol-live/pft-sol-mainnet-withdraw.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now pft-sol-mainnet-withdraw.service
export API=http://127.0.0.1:18089/solana/withdrawals
timeout 610s python3 ops/sol-live/exit_cutover.py wait-closed --api "$API" --seconds 600 > "$KIT/evidence/closed-readiness.json"
```

A fresh installation uses umask 0022 inside the privileged installer. The service-user import check must pass before moving the funded key. The same-filesystem move places the original fee-payer inode under the service account. On recovery use the already-provisioned key after checking its owner, mode and public address. `ProtectHome=yes` hides the operator's home from the unit. The withdrawal journal is under `/var/lib/pf-sol-exit/jobs`. The Ethereum service and faucet keep their existing ports.

Run a fresh proof as the installed service user. The root-owned anchor witness came from the verified archive. An empty output directory forces GPU proving and verification.

```bash
sudo systemd-run --quiet --collect --wait --pipe -p User=pf-sol-exit -p Group=pf-sol-exit \
  -p ProtectHome=yes -p ProtectSystem=strict -p PrivateTmp=yes -p NoNewPrivileges=yes \
  --setenv=HOME="$svc" --setenv=EXIT_PROOF_HOME="$app/proof" --setenv=PYTHONPATH="$app/ops/sol-live" \
  "$app/venv/bin/python" -c 'from pathlib import Path; from tempfile import TemporaryDirectory; from exit_proof import prove; from live import read; import sys
with TemporaryDirectory() as d:
    values, proof = prove("checkpoint", read(sys.argv[1]), Path(d))
    assert values and len(proof)==356
    print("service-proof verified")' "$app/anchor-witness.json" | tee "$KIT/evidence/service-proof.log"
grep -Fxq 'service-proof verified' "$KIT/evidence/service-proof.log"
python3 ops/sol-live/exit_canary.py preflight --output "$KIT/canary"
```

### Must be true before the real 0.02 pfUSDC test

The resident navcoin-relay.service is in the postfiat user manager. Set an explicit false flag in its reviewed startup configuration and ensure it is effective before starting the canary. Set the authenticated public gateway origin and cookie-jar path from the reviewed deployment. Run on the trusted relay host. RELAY_UNIT must own the process behind that gateway. Freeze relay config and restarts through settlement. Repeat both checks if the process changes.

```bash
GATEWAY=${GATEWAY:?set the authenticated public gateway HTTPS origin}
RELAY_COOKIE_FILE=${RELAY_COOKIE_FILE:?set the existing session cookie jar path}
RELAY_UNIT=${RELAY_UNIT:-navcoin-relay.service}
test -r "$RELAY_COOKIE_FILE"
python3 ops/sol-live/exit_cutover.py relay-disabled --user --unit "$RELAY_UNIT"
curl -fsS --max-time 30 -b "$RELAY_COOKIE_FILE" "$GATEWAY/api/l1/bridge/solana/withdrawals/readiness" |
  jq -e '.ok and .ready==false and .message=="Solana withdrawals are not available yet"'
```

Review the saved evidence for each completed step. Evaluate the final two checks at steps 8 and 9.

- [ ] The pre-roll six-validator snapshot and tip-new.json agree on height, hash and state root. Use the saved pre-roll.json after registration.
- [ ] deploy-v0 records both resident process hashes and manifest checks. All six rolled-vN.json files record restored watchdogs and convergence.
- [ ] Registration is confirmed closed on all six. The integrated registration hold regression passed and operators cleared outstanding signed rotations before final qualification.
- [ ] prove-roles verified both signatures before moving the fee key. The unsigned setup contains six current rotation clearances.
- [ ] Readback confirms the exact finalized tag 3. Tag-8 simulation.json confirms both account bodies.
- [ ] service-proof.log records a fresh verified proof under the installed identity. Require fresh closed payout readiness at /readiness/qualify.
- [ ] The closed preflight confirms spendable fee reserve and 20,000 revision-8 atoms. The running flag is explicitly false and the authenticated public request returned the disabled response.
- [ ] open-accepted.json records six accepted receipts at the qualified parent's successor. Hold rotations through these receipts.
- [ ] The one original burn pays. settlement-before and settlement-check reconcile six receipts, the finalized payout reference and accounting before public enablement.

```bash
grep -Fxq 'service-proof verified' "$KIT/evidence/service-proof.log"
curl -fsS --max-time 600 "$API/readiness/qualify" | jq -e '.payout_ready and (.burns_open==false) and .qualified_parent and (.relayer_lamports >= .relayer_floor_lamports + .payout_budget_lamports)'
```

## 8. Open burns in a separate unanimous decision

The frozen checker requires old-quorum V2 `signed_authorizations` in a supported single-update governance payload. Historical block 924 contains five Cobalt authorizations and zero V2 authorizations. That block predates the trusted anchor. A future Cobalt-only rotation after 1753 blocks payment across it. Readiness executes the frozen guest from the trusted committee, checks every segment endpoint and binds the resulting block hash, height and state root to six agreeing validators. It reuses previously verified finalized segments. An update at the tip requires a verified successor. Require the separately reviewed chain interlock in the exact scheduled build before opening. Drain pending burns before an incompatible change. A rotation format the frozen guest cannot verify requires a reviewed verifier repair. An HTTP flag cannot stop direct L1 burns after opening.

```bash
python3 ops/sol-live/exit_canary.py preflight --output "$KIT/canary"
python3 ops/sol-live/exit_govern.py open --revision "$REV" --release-height "$H" --output "$KIT/evidence/open-accepted.json"
jq -e '.height==(.qualified_parent.block_height+1) and (.validators|length)==6 and all(.validators[]; .receipt.accepted) and all(.routes[]; .burns_open)' "$KIT/evidence/open-accepted.json"
curl -fsS "$API/readiness/qualify" | tee "$KIT/evidence/open-readiness.json" | jq -e '.ready and .burns_open'
```

The wrapper checks the closed descriptor on six validators and requests fresh service qualification. It restarts qualification when the agreed parent changes before submission. After an ambiguous submission inspect the six receipts before another round. It invokes `ratify-solana-exit-open` on the resident node for the next proposal height. It obtains all six authorizations and submits a separate ordinary governance batch. Check six accepted receipts and open flags. At this point a direct L1 client can burn. Keep the public relay disabled.

## 9. Pay and settle one 0.02 pfUSDC canary

The source is StakeHub's encrypted b23-bob wallet under `~/.pft/wallets/4eca662d469907e4/` with passphrase at `~/.pft/b23-bob.passphrase`. The destination is `Ce5Yv528pKVrVq4JbQchZ8zasCAzxT3f9uCfatTZcfo8`. Preserve the same canary directory across retries.

```bash
python3 ops/sol-live/exit_canary.py prepare --output "$KIT/canary"
jq '{before,fields}' "$KIT/canary/plan.json"
python3 ops/sol-live/exit_canary.py burn --output "$KIT/canary"
python3 ops/sol-live/exit_canary.py check --output "$KIT/canary"
jq -e '.token_deltas=={"custody":-20000,"recipient":20000} and .withdrawal_packet_hash and .solana_tx' "$KIT/canary/paid.json"
```

The checker compares the signed envelope including fee and sequence, the six finalized burn confirmations, the permanent Solana marker, the transaction-local custody debit and ATA credit, and the L1 series debit. After a signing or submission interruption run `python3 ops/sol-live/exit_canary.py recover --output "$KIT/canary"`, then `check`. Recovery verifies the saved ML-DSA envelope offline and looks up its exact ID on all six. If absent everywhere it submits those same bytes against a fresh, matching quote. Preserve the directory if the signed file is missing or incomplete. A paid marker with unavailable history proves payment; obtain the finalized transaction from an archival RPC before attesting settlement.

Save the six-way accounting baseline before settlement.

```bash
python3 ops/sol-live/exit_canary.py settlement-before --output "$KIT/canary"
```

Each validator operator independently reviews the original paid.json, finalized Solana transaction and L1 packet using the reviewed checkout, their trusted mainnet RPC, and the validator tunnels. The review binds the one successful tag 2 (ordinal zero), marker, accounts, signature, amount, slot and packet. On each signer host install that operator's verified JSON result at `/etc/postfiat/sol-exit-paid-review.json` under root ownership. Re-review on a new parent. The review command prints only the parent and expected amendment kind.

```bash
python3 ops/sol-live/exit_canary.py settlement-review --output "$KIT/canary"
python3 ops/sol-live/exit_govern.py paid --revision "$REV" --release-height "$H" --paid "$KIT/canary/paid.json" --output "$KIT/canary/paid-round.json"
PAID_ID=$(jq -er .tx_id "$KIT/canary/paid-round.json")
PAID_HEIGHT=$(jq -er .height "$KIT/canary/paid-round.json")
python3 ops/sol-live/exit_canary.py settlement-check --output "$KIT/canary" --amendment "$PAID_ID" --height "$PAID_HEIGHT"
jq -e '(.validators|length)==6 and .state.settled==20000 and .reference.ordinal==0' "$KIT/canary/settled.json"
```

The paid command discovers the running validator binary, validates its data directory and uses `ratify-solana-exit-paid` to draft from paid.json. It checks the exact packet, finalized slot, raw 64-byte signature and ordinal in the draft before any signing. Each resident signer requires its independent approval for that parent and payout. On an ambiguous submission, locate the accepted amendment and height on all six by the saved draft ID before retrying. Recover paid-round.json from the verified receipt rather than issuing a second attestation.

Require six accepted settlement receipts. On each validator verify that the original redemption is settled for 20,000 atoms with `settlement_receipt_hash` equal to the accepted amendment ID. Reconcile the revision-8 queue, counted reserves and supply allocations against their pre-settlement values. An outstanding signature or accounting discrepancy keeps the public relay disabled.

## 10. Enable the public relay

Assign the settlement duty to the bridge operations on-call and the six resident validator signers before enabling public admission. The on-call reconciles each paid marker and pending L1 packet, obtains six independent attestations and verifies the queue, reserves and supply after every settlement. Use the paid CLI above only for the pinned canary. For other holders validators must independently check the actual owner, ATA, amount and ordinal and draft the corresponding paid amendment on their resident binary. Keep public admission disabled unless this duty is staffed and exercised for the expected traffic. The archived excess custody was 60,003 atoms. Four unsettled 20,000-atom payouts exceed it. On a backlog or missing signer close burns and restore settlement before resuming.

After the canary's finalized payout and verified L1 settlement, set `L1_SOLANA_WITHDRAW_SERVICE_URL=http://127.0.0.1:18089` and `L1_SOLANA_WITHDRAWALS_ENABLED=true` in the reviewed relay deployment. Read readiness and the original paid job through the authenticated relay.

## Recovery

For an open route, close direct L1 admission with a separate unanimous decision. Preserve evidence and wait for six accepted receipts and six closed descriptors.

```bash
python3 ops/sol-live/exit_govern.py close --revision "$REV" --release-height "$H" --output "$KIT/evidence/close-accepted.json"
```

Disabling public admission or pausing Solana leaves finalized L1 burns pending. Keep the replay-capable validator release, restore payout and reuse the original burn ID. The worker records the payout signature and its expiry before broadcast and reconciles the finalized marker on restart. To rebuild a damaged SQLite journal, stop the payout unit, run the following as the service account, and restart only after recovery succeeds.

```bash
sudo systemctl stop pft-sol-mainnet-withdraw.service
sudo -u pf-sol-exit env EXIT_FEE_KEY=/var/lib/pf-sol-exit/fee-payer.json \
  /opt/postfiat/sol-exit/venv/bin/python /opt/postfiat/sol-exit/ops/sol-live/exit_service.py \
  --state /var/lib/pf-sol-exit/jobs --rebuild
sudo systemctl start pft-sol-mainnet-withdraw.service
```

A rebuild checks L1 redemptions and finalized Solana markers, retains the old database and its SQLite sidecars, and atomically selects the recovered database through `jobs.current`. Pending jobs wait for potentially in-flight blockhashes to expire before resuming. Keep buffer keys whose on-chain rent has yet to be reclaimed.

For an emergency Solana pause the offline operator uses the dedicated pause signer. Use flag 0 to resume only after repair and review.

```bash
python3 - <<'PY'
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

The seven earlier Solana series retain their own liabilities. Migration requires separate governance and conservation tests for outstanding amounts, queued redemptions, counted reserves and allocations. This release binds only revision 8.
