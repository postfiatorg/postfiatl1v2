# Release 6 Solana route rehearsal

**Hold the rollout.** The local six-node network finalized a transfer with fresh rehearsal signers. The six keyless physical fleet copies each rejected release 5 and the candidate at the genesis integrity check. A certified restore from a portable checkpoint is recorded separately below. No live unit, route or account was changed.

## Release lineage

Release 5 is `b24-release5-20261117`, revision `ac91a1f4`. The release branch starts at `784dc6d01`, which adds the deployed measurements to the same binary source. The joined integration line was never merged. Use `git diff --stat ac91a1f4..HEAD` and `git log --oneline ac91a1f4..HEAD` to audit the port.

The change introduces the governed activation height, source-specific proof operation, bounded Groth16 verification, light client, atomic receipt credit and node admission paths. Existing ledgers omit the optional Solana fields when absent. A signed height amendment and a governed route profile are required. The release guest's ranked epoch handoff uses `PFSOLHP2`. The test profile and its Groth16 fixtures use separately derived guest keys. The release key hashes and artifact hashes are in [program-info.json](../../programs/pfusdc-sol-ingress/program-info.json).

## Release 5 precedent

The [deployed measurements](finality-latency-status-reads-20261117.md#deployed-and-measured-2026-11-17-1428-to-1522-utc) and the records under `/home/postfiat/repos/b23-coord/evidence/ops/release5/` show one pre-rollout validator-1 copy at height 1421. Both binaries verified its finalized checkpoint. Both historical verification commands rejected the same governance amendment. The operators rolled validator-1 first, then 0, 2, 3, 4 and 5. They checked the validator and RPC executable on each host, sent a transfer and confirmed six-node agreement between hosts. See `copy-verify-finalized-checkpoint-{old,new}.out`, `fleet-99-all-release5.json` and the six `rollout-v*.log` files.

## Commands and evidence

Run from this worktree. Host changes in the rollout checklist belong to the orchestrator. For the rehearsal, all writes are local. The retained evidence directory is access-controlled on the build machine.

```bash
E=/mnt/HC_Volume_106212907/stakehub-hyperliquid/sol-release6
LOG=$E/evidence
R5=/home/postfiat/var/b24-release5-20261117/bin/postfiat-node
export CARGO_TARGET_DIR=$E/target CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0
cargo build --locked --release -p postfiat-node --bin postfiat-node
cargo test --locked -p postfiat-execution --lib
cargo test --locked -p postfiat-node --lib solana_bridge
cargo test --locked -p postfiat-node --lib vault_bridge
cargo test --locked -p postfiat-node --bin postfiat-node transport
cargo test --locked -p postfiat-node --test b23_facility_offer_finality
cargo test --locked -p postfiat-node --lib vault_bridge -- --ignored --test-threads=1
```

The supplemental Foundry run needs installed `forge`, `cast` and `anvil`. With default Anvil finality the governed deposit/withdrawal test saw empty runtime code at the finalized block. With `POSTFIAT_ANVIL_BIN` pointing to a wrapper running `exec anvil --slots-in-an-epoch 0 "$@"`, the claim failed with `pfusdc_source_series_not_active` on the release line and on clean `ac91a1f4`. The relay attached a route epoch before the test activated source-series accounting. The other real-Anvil test passed. See `$LOG/anvil.log`, `anvil-finalized-r{5,6}.log`.

### Physical data and certified restoration

```bash
for n in {0..5}; do
  /usr/bin/time -p ops/release6/copy-state.sh "$E/copies" "$n"
  for binary in "$R5" "$R6"; do
    ops/release6/probe-copy.sh "$binary" "$E/copies/validator-$n" status
  done
done
```

The copy script derives the host and SSH identity path from the fleet tunnels. It pulls an allowlist of canonical files, the transactional database and its history, and public consensus artifacts. The probe mounts each copy read-only at its canonical path in a network-disabled container. The chain-tip JSON on disk was left at the height of the storage migration. The configured transactional generation is authoritative.

| Validator | Bytes pulled | Seconds |
| --- | ---: | ---: |
| 0 | 1,791,598,058 | 93.87 |
| 1 | 1,791,607,704 | 48.16 |
| 2 | 1,794,885,596 | 34.72 |
| 3 | 1,791,621,074 | 55.50 |
| 4 | 1,791,693,249 | 46.96 |
| 5 | 1,791,193,304 | 31.98 |

Each binary returned `integrity MAC mismatch in genesis` on all six copies. The six subsequent read-only SHA-256 checks of the live database files also differed from their earlier local copies. The sampled live logical height remained 1453. The two reads occurred at different times on active databases. The detailed records are `$LOG/copy-{0..5}.log`, `db-sha-{0..5}.log` and `keyless-*-r*.log`.

The newest portable checkpoint found in the fleet inventory was validator-1's October 30 backup at height 1161. The 21 public snapshot files occupy 256,834,190 bytes. Fetching them took 13.04 seconds. The allowlist used for this copy is `$E/portable-files`. For each validator, import the common checkpoint under its own identity and then replay its own peer's signed history. The read-only RPC tunnels listen at `127.0.0.1:39650+n`.

```bash
backup=/var/lib/postfiat/pre-rollout-snapshots/vote-records-20261030-validator-1-finalized-checkpoint
rsync -rzt --files-from="$E/portable-files" --no-links -e "ssh -i $identity -o BatchMode=yes -o StrictHostKeyChecking=yes" "$host:$backup/" "$E/portable/"
for n in {0..5}; do
  "$R5" snapshot-import-finalized-checkpoint --snapshot-dir "$E/portable" --data-dir "$E/restore/validator-$n" --node-id "validator-$n"
  python3 ops/release6/catch-up.py "$R5" "$E/restore/validator-$n" "$E/catchup-v$n" "$n" 1208
  ops/release6/continue-replay.sh "$E" "$n" 1453
done
```

The initial direct release 5 catch-up to 1453 stopped at 1208. It rejected the archived governance payload at 1209 with `pfusdc_beacon_finality.current_sync_committee_root must be 64 lowercase hex characters`. The operator archive identifies `r2-1-activation-bound-20261112` at that height. Fetching only that installed binary from validator-1 gave SHA-256 `35513b7ce319162e070fab7ef6db8b6c75bec99201d04307bb8ffb0a9a89299a`. The historical release verified the signed transition at height 1209. The continuation script requires that SHA, height 1208 and the expected tip/root returned by the source peer. It switches immediately back to release 5 after 1209. Individual read methods, window ends and exact certificate-replay commands with results are in `$LOG/certified-catchup*.log`, `era1209-v1.log` and `replay-after-v*.log`.

### Isolated transport rehearsal

```bash
/usr/bin/time -p docker run --rm --network none --cap-drop ALL --security-opt no-new-privileges \
  --user "$(id -u):$(id -g)" -v "$R6:/opt/postfiat-node:ro" \
  -v "$PWD/ops/release6/disposable-transfer.py:/run-check.py:ro" \
  python:3.12-slim python -B /run-check.py
```

The test creates a distinct chain ID with six fresh validator identities. It starts six real transport services, waits for their verifier readiness markers, signs an ordinary transfer, runs a certified round, then verifies the receipt and checkpoint on each node. The captured result is in `$LOG/disposable-transfer.log`. The restored live-chain registry requires its own quorum of registered signers. A fresh round on the copied chain remains a separate release gate. Replaying an existing certificate demonstrates history compatibility.

## Host-by-host rollout checklist for the orchestrator

- [ ] Close the keyless-copy comparison, copied-chain transfer and bridge test gates. Record the exact approved binary SHA, source revision, signed manifest and deployment ID. Keep the Solana activation height unset through the mixed fleet.
- [ ] Set `REL` to the approved release 6 ID, `OLD=b24-release5-20261117`, `SHA` to the full approved SHA-256, and `STAGE_ROOT` and `EVID` to fresh release 6 paths. Export all five. The b23 scripts have historical release defaults.
- [ ] Stage with `deployment-validator-units-stage` using release 5's topology and circuit metadata and the approved binary. Use the existing deployment publisher to create the release 6 manifest. Verify the trusted publisher, validity interval, runtime binary, topology and both circuit metadata files. Verify the bindings for all six validators. Compare rendered units and environment against release 5 and review every difference.
- [ ] Capture the six live RPC responses, both executable hashes per host, common genesis/height/tip/root, validator identities, mempool, readiness and watchdog state. Keep the release 5 binary, units and rollback path available. Use the ops scripts in `/home/postfiat/repos/postfiatl1v2-b23-ops`.
- [ ] **Validator 1, canary.** Run `bash ops/b23/rollout_one.sh 1 "$host"`. Check the signed manifest and both running process roles against the full new SHA. Require six distinct successful status responses with one height, tip and root. At the newly submitted transfer height, require six identical block hashes and an accepted receipt for that exact transaction on every node. Check balances, readiness and outbox before continuing. `fleet_check.py` alone can report convergence when all six reads fail. `check_block.py` alone can report agreement on a rejected receipt.
- [ ] **Validator 0.** Run the same command with index 0 and its host. Repeat all canary checks.
- [ ] **Validator 2.** Roll only this host and repeat all checks.
- [ ] **Validator 3.** Roll only this host and repeat all checks.
- [ ] **Validator 4.** Roll only this host and repeat all checks.
- [ ] **Validator 5.** Roll only this host and repeat all checks. Archive the final six-node status, all twelve observed executable hashes, and the per-host transfer IDs, heights, actual receipts and state roots.
- [ ] On failure, halt the sequence. Before any Solana activation, use `OLD=b24-release5-20261117 bash ops/b23/rollback_host.sh "$n" "$host"` for the affected host. Verify both process roles against the full release 5 SHA `69de5ce01bbda92ac5507565388d9e9655457be9bdd37582efd5d38f969bcd18`, original watchdog state, readiness and six-node checkpoint agreement. The rollback script's printed SHA hint belongs to an older release. After route activation, a return to release 5 requires a separately qualified recovery plan.
