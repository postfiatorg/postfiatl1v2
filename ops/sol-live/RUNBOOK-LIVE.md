# Solana pfUSDC exit cutover

Orchestrator only. Use one shell from the reviewed release checkout. Keep the public relay flag disabled through the real payout and its L1 settlement. Starting at step 8, direct L1 clients can burn. The immutable Solana binding supports only the revision-8 source bucket.

## 1. Qualify the release with burns closed

Choose an agreed future block height H greater than 1754, with time to roll six validators. Integrate the reviewed chain and ops fixes in a fresh release checkout. The two fixes must both be present. The checked-in development release height is unset.

```bash
set -euo pipefail
umask 077
export EXIT_WORK=/home/postfiat/var/sol-exit
export REL=${REL:?set a fresh sol-exit release name}
export H=${H:?set the agreed future height}
export CHAIN_FIX=${CHAIN_FIX:?set the reviewed chain commit}
export OPS_FIX=${OPS_FIX:?set the reviewed ops commit}
export CARGO_TARGET_DIR="$EXIT_WORK/target-release"
git -C "$EXIT_WORK/l1" fetch public refs/heads/main:refs/remotes/public/main
git -C "$EXIT_WORK/l1" worktree add -b "release/$REL" "$EXIT_WORK/$REL" public/main
cd "$EXIT_WORK/$REL"
git merge --no-ff --no-edit "$CHAIN_FIX" "$OPS_FIX"
export PYTHONPATH="$PWD/ops/sol-live"
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
export REV=$(git rev-parse --short=8 HEAD)
python3 ops/sol-live/exit_release_kit.py --release "$REL" --binary "$B"
export ARCHIVE="$KIT/anchor-replay" INTEGRITY="$EXIT_WORK/replay/source/.integrity.key"
mkdir -m700 "$ARCHIVE"
rsync -a --exclude='*key*' "$EXIT_WORK/replay/source/" "$ARCHIVE/"
replay() {
  docker run --rm --network none --cap-drop ALL --security-opt no-new-privileges \
    -u "$(id -u):$(id -g)" -v "$2:/var/lib/postfiat/validator-3" \
    -v "$3:/var/lib/postfiat/validator-3/.integrity.key:ro" -v "$1:/opt/node:ro" ubuntu:24.04 \
    /opt/node verify-state --data-dir /var/lib/postfiat/validator-3
}
replay "$B" "$ARCHIVE" "$INTEGRITY" > "$KIT/evidence/anchor-replay.json"
jq -e --slurpfile prev "$EXIT_WORK/evidence/old-state.json" \
  '.verified and .block_log==$prev[0].block_log and .block_log.block_count==1753' "$KIT/evidence/anchor-replay.json"
```

Verify both the predecessor and the release against a private snapshot of the **current** committed generation before staging. At height 1753 the scratch archive above serves both purposes. At a later height set TIP_STORE and TIP_KEY to a consistent scratch snapshot and its matching integrity file. Take both binaries through the same generation. Preserve the original height-1753 archive for tag 3.

```bash
TIP_STORE=${TIP_STORE:-$ARCHIVE}
TIP_KEY=${TIP_KEY:-$INTEGRITY}
replay "$HOME/var/main-r64-20261121/bin/postfiat-node" "$TIP_STORE" "$TIP_KEY" > "$KIT/evidence/tip-old.json"
replay "$B" "$TIP_STORE" "$TIP_KEY" > "$KIT/evidence/tip-new.json"
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
bash "$KIT/stage.sh"
```

The publisher credential stays at `~/.postfiat/trusted-deployment-publisher.private.json`. If the snapshot and the converged tip differ, refresh the snapshot and qualify both binaries again.

## 2. Roll validator 0, then the other five

```bash
bash "$KIT/roll-rest.sh" 0
bash "$KIT/roll-rest.sh" 1 2 3 4 5
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

At or after H, review the registration batch and submit through the six resident validator signers. The prepare command needs a writable scratch store with its matching integrity file accessible to the node. Run it inside the same store and key mounts used above.

```bash
mkdir -m700 "$KIT/governance"
docker run --rm --network none -u "$(id -u):$(id -g)" --cap-drop ALL \
  -v "$ARCHIVE:/var/lib/postfiat/validator-3" -v "$INTEGRITY:/var/lib/postfiat/validator-3/.integrity.key:ro" \
  -v "$B:/opt/node:ro" -v "$PWD/crates/types/src/solana_exit_profile.json:/profile:ro" -v "$KIT/governance:/draft" \
  ubuntu:24.04 /opt/node vault-bridge-route-profile-governance --data-dir /var/lib/postfiat/validator-3 \
  --validators validator-0,validator-1,validator-2,validator-3,validator-4,validator-5 \
  --profile-file /profile --amendment-file /draft/amendment.json --batch-file /draft/batch.json
cmp "$KIT/governance/batch.json" crates/node/tests/fixtures/solana-exit/governance-batch.json
python3 ops/sol-live/exit_govern.py submit --revision "$REV" --release-height "$H"
```

Require all six series RPCs to report the reviewed descriptor hash, `active=true` and `burns_open=false`. Confirm the Ethereum descriptor still matches epoch 11. On a lost governance response check the amendment ID and receipts on the six validators before proposing another round.

## 4. Precheck the immutable binding

```bash
setup_args=(--binary "$B" --archive "$ARCHIVE" --integrity-file "$INTEGRITY")
mkdir -p "$KIT/evidence/solana"
python3 ops/sol-live/exit_setup.py "${setup_args[@]}" --output "$KIT/evidence/solana/unsigned.json"
jq -e '.guest.forward and .guest.start==.config.initial_committee and .simulation.value.err==null' "$KIT/evidence/solana/unsigned.json"
```

Setup re-verifies the archived certificate and compares the authenticated committee, accepted ingress record and bucket across six validators. It checks finalized Solana vault config, custody, USDC mint and canary ATA at one slot, deployed programs and loader authorities, executes the frozen checkpoint guest and checks the simulated PDA bytes. At a stationary 1753 tip the guest can prove ancestry ending there. Wait for a later finalized block before signing.

The tag-3 instruction is 317 bytes. Its SHA-256 is `bb32524804b64a9d7d40fec86c9735f8c1f5d5d03d7a8a6963e8c21f83903fe4`. The dedicated pause authority is `ow7wprz3MPEN15BGLyNvdJr5AUq4LNZK2ox1gY1QEJx`. Its signing file stays offline from the payout service. Review the instruction, ordered accounts and simulated bindings in `unsigned.json`.

Fund the dedicated fee payer once. This orchestrator command sends **0.10 SOL**. Check the finalized balance first to recover a lost funding response. The checkout and precheck do not fund it.

```bash
python3 - <<'PY'
from exit_setup import DIRECTORY, ExitCluster, FEE_PAYER, MAINNET_PAYER, check_payout_authorities, require
from solders.system_program import transfer
c=ExitCluster(MAINNET_PAYER)
check_payout_authorities(c)
require(c.call("getBalance",str(FEE_PAYER),c.level)["value"]==0,"Fee payer already funded; reconcile first.")
c.unlock(DIRECTORY/"mainnet-payer.json")
ix=transfer(dict(from_pubkey=c.owner,to_pubkey=FEE_PAYER,lamports=100_000_000))
print(c.send("exit.fund",[ix]))
PY
```

The recipient is `8EiSLnZQv23HtA5adokE6xfKyCK6mf4HGkpxSPpop1fs`. Readiness quotes peak proof-buffer, checkpoint, marker and ATA rents and transaction fees above the 20,000,000-lamport floor. All Solana buckets sharing this custody must remain covered, including queued redemptions pending L1 settlement.

## 5. Initialize Solana

```bash
python3 ops/sol-live/exit_setup.py "${setup_args[@]}" --execute --output "$KIT/evidence/solana/initialized.json"
jq -e '.finalized_readback and .signature' "$KIT/evidence/solana/initialized.json"
```

Only this step opens the setup payer and verifier deployment signers at `~/.config/postfiat/sol-bridge/{mainnet-payer,egress-verifier}.json`. Empty system-owned prefunded PDAs are eligible for creation. If RPC times out, repeat the unsigned setup command first. It recognizes both exact finalized accounts and the exact historical tag-3 instruction. For deep history provide `--init-signature` and an archival mainnet RPC. Stop on a conflicting binding. A permanent binding error needs a reviewed forward repair.

## 6. Verify the initialized checker

```bash
python3 ops/sol-live/exit_setup.py "${setup_args[@]}" --output "$KIT/evidence/solana/readback.json"
jq -e '.finalized_readback and .guest.forward' "$KIT/evidence/solana/readback.json"
tag8=(--guest "$KIT/evidence/solana/guest" --work "$KIT/evidence/tag8")
python3 ops/sol-live/exit_tag8.py stage "${tag8[@]}"
python3 ops/sol-live/exit_tag8.py simulate "${tag8[@]}"
jq -e '.value.err==null' "$KIT/evidence/tag8/simulation.json"
python3 ops/sol-live/exit_tag8.py reclaim "${tag8[@]}"
```

Staging obtains the Groth16 proof and signs only the buffer allocation and uploads. Simulation sends an unsigned tag 8 to `simulateTransaction` and checks its returned light and checkpoint bytes. Reclaim the buffer rent after successful simulation. Keep burns closed if any check fails.

## 7. Install the isolated payout service

Install a **system** service under `pf-sol-exit`. Keep the code, pinned ELF and proof host root-owned. Provision a separate SSH identity for this service account on the existing prover and verify its host key out of band. Its home must permit batch SSH to the configured prover and the remote account must support the shared GPU lock. Keep the deployment account's SSH and Solana credentials out of the service home.

```bash
app=/opt/postfiat/sol-exit
svc=/var/lib/pf-sol-exit
id pf-sol-exit >/dev/null 2>&1 || sudo useradd --system --user-group --home-dir "$svc" --shell /usr/sbin/nologin pf-sol-exit
sudo install -d -m700 -o pf-sol-exit -g pf-sol-exit "$svc"
sudo install -d -m755 "$app" "$app/proof/bin" "$app/proof/historical"
git archive HEAD ops/sol-live solana/deploy solana/pfusdc-egress-verifier/bootstrap.py crates/types/src/solana_exit_profile.json | sudo tar -x -C "$app" --no-same-owner
sudo install -m755 "$HOME/repos/b23-coord/evidence/withdraw/bin/proof-host" "$app/proof/bin/proof-host"
sudo install -m644 "$HOME/repos/b23-coord/evidence/withdraw/historical/egress.elf" "$app/proof/historical/egress.elf"
sudo install -m644 "$KIT/evidence/solana/guest/witness.json" "$app/anchor-witness.json"
sudo python3 -m venv "$app/venv"
sudo "$app/venv/bin/pip" install solders==0.28.0 base58==2.1.1 requests==2.31.0
fee="$HOME/.config/postfiat/sol-bridge/exit-fee-payer.json"
test -f "$fee" && test ! -L "$fee" && test "$(stat -c %a "$fee")" = 600
test "$(stat -c %d "$fee")" = "$(stat -c %d "$svc")"
sudo test ! -e "$svc/fee-payer.json"
sudo mv -T "$fee" "$svc/fee-payer.json"
sudo chown pf-sol-exit:pf-sol-exit "$svc/fee-payer.json"
test -z "$(ss -H -ltn 'sport = :18089')"
sudo install -m644 ops/sol-live/pft-sol-mainnet-withdraw.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now pft-sol-mainnet-withdraw.service
export API=http://127.0.0.1:18089/solana/withdrawals
curl -fsS "$API/readiness" | tee "$KIT/evidence/closed-readiness.json" | jq -e '.payout_ready and (.burns_open==false) and (.ready==false)'
```

The same-filesystem move places the original fee-payer inode under the service account. On recovery use the already-provisioned key after checking its owner, mode and public address. `ProtectHome=yes` hides the operator's home from the unit. The withdrawal journal is under `/var/lib/pf-sol-exit/jobs`. The Ethereum service and faucet keep their existing ports.

## 8. Open burns in a separate unanimous decision

The frozen checker requires old-quorum V2 `signed_authorizations` in a supported single-update governance payload. Historical block 924 contains five Cobalt authorizations and zero V2 authorizations. That block predates the trusted anchor. A future Cobalt-only rotation after 1753 blocks payment across it. Readiness screens finalized batches, up to eight blocks per check. Let that scan finish. Coordinate future registry changes with the validator operators before opening burns. A rotation format the frozen guest cannot verify requires a reviewed verifier repair. An HTTP flag cannot stop direct L1 burns after opening.

```bash
python3 ops/sol-live/exit_govern.py open --revision "$REV" --release-height "$H"
curl -fsS "$API/readiness" | tee "$KIT/evidence/open-readiness.json" | jq -e '.ready and .burns_open'
```

The wrapper checks the closed descriptor on six validators and the installed payout service. It invokes `ratify-solana-exit-open --data-dir "$VD" --validators "$VALIDATORS" --height "$H_NEXT" --amendment-file open.json` on the resident node for the next proposal height. It obtains all six authorizations and submits a separate ordinary governance batch. Check six accepted receipts and open flags. At this point a direct L1 client can burn. Keep the public relay disabled.

## 9. Pay and settle one 0.02 pfUSDC canary

The source is StakeHub's encrypted b23-bob wallet under `~/.pft/wallets/4eca662d469907e4/` with passphrase at `~/.pft/b23-bob.passphrase`. The destination is `Ce5Yv528pKVrVq4JbQchZ8zasCAzxT3f9uCfatTZcfo8`. Preserve the same canary directory across retries.

```bash
python3 ops/sol-live/exit_canary.py prepare --output "$KIT/canary"
jq '{before,fields}' "$KIT/canary/plan.json"
python3 ops/sol-live/exit_canary.py burn --output "$KIT/canary"
python3 ops/sol-live/exit_canary.py check --output "$KIT/canary"
jq -e '.token_deltas=={"custody":-20000,"recipient":20000} and .withdrawal_packet_hash and .solana_tx' "$KIT/canary/paid.json"
```

The checker compares the signed envelope including fee and sequence, the six finalized burn confirmations, the permanent Solana marker, the transaction-local custody debit and ATA credit, and the L1 series debit. On timeout, recover the original signed burn and use `check` in the original directory. A paid marker with unavailable history proves payment; obtain the finalized transaction from an archival RPC before attesting settlement.

Each validator independently verifies the exact finalized tag-2 payout against the L1 packet, including program IDs, route, burn marker, full owner and ATA, mint, amount and slot. Use the zero-based ordinal among successful tag-2 payouts in execution order. The canary worker emits one tag 2, at ordinal zero. Confirm that in the actual transaction. On a validator in a fresh round directory, prepare the unsigned settlement decision at the next proposal height.

```bash
postfiat-node ratify-solana-exit-paid --data-dir "$VD" --validators "$VALIDATORS" --height "$H_NEXT" \
  --packet-hash "$PACKET" --solana-slot "$SLOT" --payout-index "$INDEX" --signature-hex "$SIG_HEX" --amendment-file paid.json
```

PACKET is the 96-hex withdrawal packet hash. SIG_HEX is the raw 64-byte transaction signature decoded from base58. Distribute the identical unsigned amendment to all six resident signers. Each uses `governance-authorization-sign` at H_NEXT with its local `/var/lib/postfiat/validator-N/validator_keys.json`. Assemble all six authorizations with `governance-amendment-assemble`, build the ordinary `governance-batch`, then run `transport-peer-certified-batch-round` on the scheduled proposer. The commands and propagation flags follow the ordinary-amendment branch in `ops/sol-live/govern.py`. A changed proposal height requires a fresh draft and signatures.

Require six accepted settlement receipts. On each validator verify that the original redemption is settled for 20,000 atoms with `settlement_receipt_hash` equal to the accepted amendment ID. Reconcile the revision-8 queue, counted reserves and supply allocations against their pre-settlement values. An outstanding signature or accounting discrepancy keeps the public relay disabled.

## 10. Enable the public relay

After the canary's finalized payout and verified L1 settlement, set `L1_SOLANA_WITHDRAW_SERVICE_URL=http://127.0.0.1:18089` and `L1_SOLANA_WITHDRAWALS_ENABLED=true` in the reviewed relay deployment. Read readiness and the original paid job through the authenticated relay.

## Recovery

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
