# Solana USDC deposit bridge cutover

These steps are for a future maintenance window after the port merges into public main. Run them as the bridge user on the local service host, in one Bash shell. They cover the local units for `solana-mainnet-usdc-v1` on `postfiat-wan-devnet-2`. The observer and GPU retain their current installations. The recorder writers also serve the local and devnet Solana clusters.

## 1. Stage the reviewed revision

Use a fresh directory. These unit files expect `%h/repos/postfiatl1v2-live`. Record the commit and compare it with the approved public merge. Stop if the directory already exists and is in service.

```bash
set -euo pipefail
new="$HOME/repos/postfiatl1v2-live"
old="$HOME/repos/solbridge-live"
rec="$HOME/repos/solbridge-recorder"
u="$HOME/.config/systemd/user"
state=/mnt/HC_Volume_106212907/stakehub-hyperliquid/sol-live
mainstate="$state/mainnet"
test ! -e "$new"
git clone --single-branch --branch main https://github.com/postfiatorg/postfiatl1v2.git "$new"
git -C "$new" rev-parse HEAD
cd "$new"
ops/sol-live/stage-runtime.sh "$old" "$rec" "$new"
test -x target/task-l/release/pfusdc-sol-capture
test -x target/task-l/host/release/pfusdc-sol-ingress-host
test -x target/task-l/warm-signer
/usr/bin/python3 -B -c 'import requests, solders'
test "$(sha256sum "$state/../sol-release6/bin/cdc98a49/postfiat-node" | cut -d" " -f1)" = de331e61d0c1606dd17ee336703a164028c6abb51ee51e8bbc4d838dbcec251c
```

The stager copies 18 allowlisted files and checks each SHA-256. It installs the local signer, host, capture tool, guest ELFs, SBF images and their checksums. It also accepts identical files staged by a release build. A mismatch blocks the switch. The user account, state mount, wallet API token file, signer and payer paths, RPC configuration, Vast CLI, SSH configuration and Unix socket locations stay at their existing paths.

## 2. Record the baseline and back up the installed definitions

```bash
backup=$(mktemp -d "$HOME/solana-unit-backup.XXXXXXXX")
chmod 700 "$backup"
units=(pft-sol-mainnet-{relay,wallet-api,health,reanchor,go}.service
       pft-sol-mainnet-{reanchor,go}.timer pft-sol-prover-tunnel.service
       pft-sol-recorder.service pft-sol-devnet-recorder.service pft-sol-recorder-close@.service)
writers=(pft-sol-recorder.service pft-sol-devnet-recorder.service
         pft-sol-recorder-close@local.service pft-sol-recorder-close@devnet.service)
overrides=()
for unit in pft-sol-recorder.service pft-sol-devnet-recorder.service; do
    overrides+=("$unit.d/ancestry-capture.conf" "$unit.d/recorder-close-image.conf")
done
go_want=timers.target.wants/pft-sol-mainnet-go.timer
tar -C "$u" -cf "$backup/definitions.tar" "${units[@]}" "${overrides[@]}" "$go_want"
systemctl --user show "${units[@]}" "${writers[@]}" pft-sol-recorder-close@mainnet.service \
    pft-sol-local.service pft-sol-devnet-local.service \
    -p Id -p FragmentPath -p DropInPaths -p UnitFileState -p ActiveState -p SubState \
    -p Result -p NRestarts -p MainPID > "$backup/baseline.txt"
printf '%s\n' "$backup"
```

The archive preserves the two go-unit symlinks and the direct timer wants link. Only known unit definitions are included. Review the baseline against the report. Resolve any new drop-ins or changed active instances before proceeding. Preserve disabled or deliberately stopped units. On November 21 the mainnet-go timer was enabled and stopped. The mainnet recorder-close instance was disabled. The four local/devnet recorder workers were retrying RPC connections. Record that fault separately. Both prerequisite cluster services must already be up before starting their writers.

Record the governed profile hash, route epoch, custody cursor, latest accepted job and pending signer marker by reference. Inspect the current wallet readiness using the GET in step 6. The existing data volume is shared across both code revisions. Preserve its normal recovery protection and retain the original checkouts for rollback.

## 3. Quiesce the local workers

Close new deposits through the wallet operator's existing maintenance control and confirm that the wallet refuses new sends. A direct Solana transfer remains possible. Schedule away from the epoch boundary. Pause if the relay has a stop marker, an unresolved receipt, or an old-epoch deposit awaiting settlement.

```bash
systemctl --user stop pft-sol-mainnet-reanchor.timer
while systemctl --user is-active --quiet pft-sol-mainnet-reanchor.service; do sleep 2; done
systemctl --user show pft-sol-mainnet-reanchor.service -p ActiveState -p SubState -p Result
```

Require the oneshot to be inactive with a successful last result. Confirm the go timer and service are inactive. Let the custody watcher finish its open jobs. Check the relay's pending submissions, its six-receipt waits and its cursor against the settled custody rows again. A stopped relay may have signed work pending. Resolve ambiguous submissions using the original envelope and receipt.

```bash
systemctl --user stop pft-sol-mainnet-wallet-api.service pft-sol-mainnet-relay.service
systemctl --user stop pft-sol-mainnet-health.service pft-sol-prover-tunnel.service
systemctl --user stop "${writers[@]}"
```

## 4. Install the unit definitions

The mainnet scripts use the new working directory. The wallet API also gets the new Python path. The re-anchor and recorder units use their matching staged images. The go unit gets a new absolute launcher. The recorder writer starts the recorder-release wrapper with the ancestry directory as its working directory. The close template starts in the recorder release tree.

```bash
for unit in "${units[@]}"; do
    # Replace the directory entry atomically, including when the old entry is a symlink.
    install -m 0644 "ops/sol-live/$unit" "$u/$unit.solana-new"
    mv -Tf "$u/$unit.solana-new" "$u/$unit"
done
for file in "${overrides[@]}"; do
    mv "$u/$file" "$backup/${file//\//_}"
done
ln -sfn "$u/pft-sol-mainnet-go.timer" "$u/$go_want"
systemctl --user daemon-reload
systemctl --user show "${units[@]}" "${writers[@]}" \
    -p Id -p FragmentPath -p DropInPaths -p WorkingDirectory -p ExecStart
```

Check each effective command and working directory before starting it. All four old recorder overrides must be absent. The cluster services and the unrelated relay and tunnel units retain their definitions.

## 5. Start in dependency order

```bash
systemctl --user start pft-sol-prover-tunnel.service
for attempt in {1..30}; do
    [[ -S "${XDG_RUNTIME_DIR:?}/pft-sol-prover/prover.sock" ]] && break
    sleep 1
done
test -S "$XDG_RUNTIME_DIR/pft-sol-prover/prover.sock"
systemctl --user start pft-sol-mainnet-health.service
systemctl --user start pft-sol-mainnet-relay.service
systemctl --user start pft-sol-mainnet-wallet-api.service
```

If the socket does not appear in 30 seconds, inspect the tunnel before proceeding. Verify the four core services stay active. A relay exit status of 3 needs operator review. Resolve the recorded local/devnet recorder fault before claiming those writers healthy. Resume only the instances that were running or retrying in the approved baseline.

```bash
systemctl --user is-active pft-sol-local.service pft-sol-devnet-local.service
systemctl --user start pft-sol-recorder.service pft-sol-devnet-recorder.service
systemctl --user start pft-sol-recorder-close@local.service pft-sol-recorder-close@devnet.service
```

## 6. Verify readiness and resume the timer

```bash
for unit in pft-sol-{prover-tunnel,mainnet-health,mainnet-relay,mainnet-wallet-api}; do
    systemctl --user is-active --quiet "$unit"
    pid=$(systemctl --user show -p MainPID --value "$unit")
    test "$(readlink -f "/proc/$pid/cwd")" = "$new"
done
curl -fsS 'http://127.0.0.1:8187/api/bridge/readiness?route=solana-mainnet-usdc-v1' |
  jq -e --slurpfile active "$mainstate/activation.json" \
    '.ready == true and .ledger == "node" and .route_id == "solana-mainnet-usdc-v1"
     and .route_profile_hash == $active[0].profile_hash'
jq -e -s '.[0].profile_hash == .[1].profile_hash and .[0].profile_hash == .[2].profile_hash' \
    "$mainstate/activation.json" "$mainstate/relay-running.json" "$mainstate/wallet-relay.json"
systemctl --user start pft-sol-mainnet-reanchor.timer
systemctl --user list-timers pft-sol-mainnet-reanchor.timer
```

Allow a fresh health cycle. Confirm the next timer invocation finishes successfully and both services retain the governed profile. Verify an already accepted job remains readable, the cursor does not regress and no previously signed job receives a second credit. Check the recorder effective paths and restart counters independently. Reopen the wallet only when the mainnet checks pass. Observe a subsequent ordinary deposit through six accepted receipts. A maintenance test does not require sending funds.

## Rollback

Keep the wallet maintenance control in effect. Repeat step 3, including the oneshot and pending-submission checks. Recover the recorded unit definitions from the backup printed in step 2.

```bash
for file in "${units[@]}" "${overrides[@]}" "$go_want"; do
    rm -f -- "$u/$file"
done
tar -C "$u" -xf "$backup/definitions.tar"
systemctl --user daemon-reload
systemctl --user start pft-sol-prover-tunnel.service
systemctl --user start pft-sol-mainnet-health.service
systemctl --user start pft-sol-mainnet-relay.service
systemctl --user start pft-sol-mainnet-wallet-api.service
systemctl --user start "${writers[@]}"
systemctl --user start pft-sol-mainnet-reanchor.timer
```

Use the baseline to adjust this start list if an instance had been deliberately inactive. Repeat the readiness, cursor, profile and receipt checks using the old working directories. Keep the go timer stopped if that was its baseline. Both code revisions read the current activation, outbox, cursor, signer journal and wallet token. Retain any transactions or governance actions committed during the window. Rewinding the data volume could replay settled work.
