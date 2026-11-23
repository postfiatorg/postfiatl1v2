# Solana pfUSDC exit, orchestrator runbook

Run from the reviewed revision. The checkout is prepared with the WAN release height unset. The seven earlier Solana series need a separate migration. The current immutable light client covers revision 8 only.

## 1. Build and qualify the release

Choose H with enough lead for six sequential restarts. Release scheduling uses Post Fiat block height. The descriptor's earliest valid height is 1754. Keep H at or above that value. Use a fresh release id. In a bash shell on the orchestrator:

```bash
set -euo pipefail
export EXIT_WORK=/home/postfiat/var/sol-exit
export CARGO_TARGET_DIR=$EXIT_WORK/target
export H=${H:?set the agreed future height}
export REL=sol-exit-r1
git -C "$EXIT_WORK/l1" worktree add -b "release/$REL" "$EXIT_WORK/release" public/main
cd "$EXIT_WORK/release"
git merge --no-ff --no-edit sol/exit-tasknode
python3 - <<'PY'
import os,pathlib,sys
sys.path.insert(0,'ops/sol-live')
from live import fleet
h=int(os.environ['H'])
assert h>fleet()['block_height'] and h>=1754
p=pathlib.Path('crates/types/src/solana_exit.rs')
s=p.read_text()
old='None, // Release sets the agreed future height here.'
assert s.count(old)==1
p.write_text(s.replace(old,f'Some({h}), // Reviewed WAN release height.'))
PY
git commit -am "Activate the governed Solana exit at height $H"
cargo build --locked --release -p postfiat-node --bin postfiat-node
export B=$CARGO_TARGET_DIR/release/postfiat-node
export KIT=$HOME/var/$REL
python3 ops/sol-live/exit_release_kit.py --release "$REL" --binary "$B"
cp -a --reflink=auto "$EXIT_WORK/replay/source" "$KIT/replay"
bash "$HOME/var/main-r64-20261121/run-copy.sh" "$KIT/bin/postfiat-node" "$KIT/replay" verify-state > "$KIT/evidence/release-replay.json"
jq -e --slurpfile old "$EXIT_WORK/evidence/old-state.json" '.verified and .block_log==$old[0].block_log' "$KIT/evidence/release-replay.json"
bash "$KIT/stage.sh"
```

If the tip has advanced, qualify the fresh archive before installing any release units. This uses the same file allowlist and the active generation named by validator 3. The store integrity key stays in the private copy.

```bash
export SNAP=$(mktemp -d "$KIT/replay-refresh-XXXXXXXX")
export SOURCE_HOST=$(PYTHONPATH=ops/sol-live python3 -c 'from live import HOSTS;print(HOSTS[3])')
export VD=/var/lib/postfiat/validator-3
ssh -oBatchMode=yes "root@$SOURCE_HOST" "cat $VD/transactional_generation.json" > "$SNAP/pointer"
GEN=$(jq -er '.database_directory|split("/")|last|select(test("^transactional-generation-[0-9]+$"))' "$SNAP/pointer")
sed '/^transactional-generation-/d' "$EXIT_WORK/replay/files" > "$SNAP/files"
printf '%s\n' "$GEN/" .integrity.key >> "$SNAP/files"
mkdir -m700 "$SNAP/source"
rsync -azr --files-from="$SNAP/files" "root@$SOURCE_HOST:$VD/" "$SNAP/source/"
cmp "$SNAP/pointer" "$SNAP/source/transactional_generation.json"
cp -a --reflink=auto "$SNAP/source" "$SNAP/old"
cp -a --reflink=auto "$SNAP/source" "$SNAP/new"
for version in old new; do
  BIN="$KIT/bin/postfiat-node"
  if [ "$version" = old ]; then BIN="$HOME/var/main-r64-20261121/bin/postfiat-node"; fi
  bash "$HOME/var/main-r64-20261121/run-copy.sh" "$BIN" "$SNAP/$version" verify-state > "$SNAP/$version.json"
done
PYTHONPATH=ops/sol-live python3 - <<'PYTHON'
import os
from pathlib import Path
from live import fleet, read
p = Path(os.environ["SNAP"])
a, b = [read(p / (x + ".json")) for x in ("old", "new")]
f = fleet()
assert a["verified"] and b["verified"] and a["block_log"] == b["block_log"]
r = a["block_log"]
assert (r["block_count"], r["tip_hash"], r["state_root"]) == (f["block_height"], f["block_tip_hash"], f["state_root"])
PYTHON
```

A moving tip or unsuccessful verification requires a fresh copy. The publisher key used by stage.sh is `~/.postfiat/trusted-deployment-publisher.private.json`. Rollback before staging or deployment consists of discarding this isolated candidate.

## 2. Roll the six validators

```bash
bash "$KIT/roll-rest.sh" 0
bash "$KIT/roll-rest.sh" 1 2 3 4 5
PYTHONPATH=ops/sol-live python3 -c 'from live import fleet; import json; print(json.dumps(fleet()))'
test "$(PYTHONPATH=ops/sol-live python3 -c 'from live import fleet;print(fleet()["build_git_revision"])')" = "$(git rev-parse --short=8 HEAD)"
```

The r64 kit verifies each host's staged SHA and manifest, restores its watchdog settings, and checks six-way tip/root convergence after each restart. Read the per-host log before continuing after a failure. Before activation, restore a changed host using its saved predecessor units. Set N to the affected validator index.

```bash
export N=${N:?set the affected validator index}
HOST=$(PYTHONPATH=ops/sol-live python3 -c 'import os;from live import HOSTS;print(HOSTS[int(os.environ["N"])])')
ssh "root@$HOST" "set -e; cp -a /etc/postfiat/releases/main-r64-20261121/rollback-units/postfiat-validator-$N.service /etc/postfiat/releases/main-r64-20261121/rollback-units/postfiat-validator-$N-rpc.service /etc/systemd/system/; systemctl daemon-reload; systemctl restart postfiat-validator-$N.service; systemctl restart postfiat-validator-$N-rpc.service"
bash "$KIT/restore-watchdog.sh" "$N" "$HOST"
```

Recheck convergence after each restored host. Once a newly accepted governance or burn block exists, keep a release able to replay it and roll forward. An older validator can reject that history.

## 3. Register the exit descriptor

```bash
python3 ops/sol-live/exit_govern.py prepare --binary "$KIT/bin/postfiat-node" --copied-store "$KIT/replay" --output "$KIT/governance"
cmp "$KIT/governance/batch.json" crates/node/tests/fixtures/solana-exit/governance-batch.json
python3 ops/sol-live/exit_govern.py submit --revision "$(git rev-parse --short=8 HEAD)" --release-height "$H"
```

Submit when the pending governance block can be at or beyond H. The six resident validators sign the amendment for the current proposal slot with their own `/var/lib/postfiat/validator-N/validator_keys.json`. The wrapper assembles and submits through the existing governed round and checks both routes. A prepared unsigned batch can be discarded. If the submission response is lost, check the amendment by ID and the series RPC before attempting another round. A committed amendment is historical. A subsequent issue requires pausing payout and a forward governance/release correction.

## 4. Initialize the Solana checker

```bash
python3 ops/sol-live/exit_setup.py --output "$KIT/evidence/init-simulation.json"
python3 ops/sol-live/exit_setup.py --execute --output "$KIT/evidence/init-final-simulation.json"
```

The first command builds an unsigned transaction, simulates it on mainnet and verifies every byte of both returned accounts. The second repeats those checks, requires the exact governed descriptor on six validators, then signs with `~/.config/postfiat/sol-bridge/egress-verifier.json` and `~/.config/postfiat/sol-bridge/mainnet-payer.json`. The latter key also holds pause authority. The tag-3 transaction commits the checkpoint and committee recorded in ops/sol-live/fixtures/exit-setup.json. The instruction bytes and ordered accounts are in the simulation JSON. Discarding the transaction is sufficient before broadcast. Initialization is one-time. Tag 4 lets the pause authority suspend/resume withdrawals. Valid proofs can advance checkpoints. The program has no instruction to replace the route, bucket, trusted initial committee or pause key. Recovery from a mistaken binding requires a separately reviewed program change.

The historical anchor is the accepted transaction in block 1753. The builder reads its block ID and precommit QC committee root on all six validators and checks the exported, verified finalized checkpoint against that block. It obtains the family and ingress authorization from revision 8, checks the full bucket domain against the mainnet genesis, vault and USDC mint, and hashes the deployed verifier, vault and token program images. The descriptor's verifier policy hashes the raw 48-byte bucket ID with SHA-256. The two signers are checked against their public addresses. The payer covers 3,012,440 lamports in rent plus the simulated 10,000-lamport transaction fee.

| Tag 3 account, in instruction order | Address | Access |
| --- | --- | --- |
| Fee payer and pause authority | 4JJEuaYtJvgtMVixTbXfppHJPh6tcU5g7g7UtkvxK3sB | Writable signer |
| Verifier deployment key | 8Dxp8bfgEh6ankapfBdkTds5cebqrtPdFxpZSP6JsGAF | Read-only signer |
| Light PDA | FQe34dCQg9m6yEW6NjbjoR8LfWwKGLX2hBvTdY2Lu6s1 | Writable |
| Checkpoint PDA | BATyPAkKcpPBjFAttaHAVR9raxdNuWtQUNrdKgaWkait | Writable |
| System program | 11111111111111111111111111111111 | Read-only |

Emergency pause after initialization, from the reviewed checkout:

```bash
PYTHONPATH=ops/sol-live python3 - <<'PYTHON'
from exit_payout import cluster
from exit_setup import Instruction, VERIFIER, LIGHT, ro, rw
c = cluster()
print(c.send("egress.pause", [Instruction(VERIFIER, bytes([4, 1]), [ro(c.owner, True), rw(LIGHT)])]))
assert c.account(LIGHT)["data"][-1] == 1
PYTHON
```

Resume after review with the same command using flag 0 in both places. A pause preserves all used burn markers and withdrawals already finalized.

## 5. Install the payout service

Use the reviewed release in the live checkout. Verify the existing GPU SSH connection, native proof host and frozen ELF are available to the systemd user. The relayer uses the mainnet payer key and has a 20,000,000-lamport balance floor.

```bash
export REV=$(git rev-parse HEAD)
cd ~/repos/postfiatl1v2-live
git diff --quiet
git diff --cached --quiet
git fetch "$EXIT_WORK/release" HEAD
test "$(git rev-parse FETCH_HEAD)" = "$REV"
git merge --ff-only "$REV"
test "$(git rev-parse HEAD)" = "$REV"
test -z "$(ss -H -ltn 'sport = :18089')"
install -Dm644 ops/sol-live/pft-sol-mainnet-withdraw.service ~/.config/systemd/user/pft-sol-mainnet-withdraw.service
systemctl --user daemon-reload
systemctl --user enable --now pft-sol-mainnet-withdraw.service
curl -fsS http://127.0.0.1:18089/solana/withdrawals/readiness | tee "$KIT/evidence/readiness.json" | jq -e '.ready'
```

If the checkout has diverged or contains runtime edits, reconcile it into the reviewed release first. Keep the ETH upstream on port 18084. For the reviewed Task Node withdrawal relay, set `L1_SOLANA_WITHDRAW_SERVICE_URL=http://127.0.0.1:18089` and `L1_SOLANA_WITHDRAWALS_ENABLED=true` in its deployment environment after readiness succeeds. The faucet uses port 18085. The gateway forwards through its existing authenticated relay. A service rollback stops the Solana unit with `systemctl --user disable --now pft-sol-mainnet-withdraw.service`. Preserve `~/.local/state/postfiat/sol-exit`; its journal and pending burn IDs support recovery. Disabling a relay does not reverse accepted burns.

## 6. Prove one real payout

The signing helper opens StakeHub's named wallet b23-bob with `~/.pft/b23-bob.passphrase` and the encrypted wallet under `~/.pft/wallets/4eca662d469907e4/`. Use the same output directory for this canary throughout. It binds the operation to revision 8, 20,000 atoms and Phantom owner Ce5Yv528pKVrVq4JbQchZ8zasCAzxT3f9uCfatTZcfo8.

```bash
python3 ops/sol-live/exit_canary.py prepare --output "$KIT/canary"
jq '{before,fields}' "$KIT/canary/plan.json"
python3 ops/sol-live/exit_canary.py burn --output "$KIT/canary"
python3 ops/sol-live/exit_canary.py check --output "$KIT/canary"
jq '{burn_tx_id,height,solana_tx,recipient,before,after,marker}' "$KIT/canary/paid.json"
```

A signed but unsubmitted intent can be withheld. After acceptance the burn is irreversible. If submission or payout times out, run `check` with the original directory. It finds the original sequence/recipient/amount, requires six confirmations, tracks the existing job, and verifies the finalized Solana signature, one-use PDA and exact 20,000-atom balance changes. Investigate ambiguous submission from the preserved signed transaction before taking any further action.

## Earlier and future series

At the rehearsal tip the seven earlier mainnet Solana buckets had 5,967,032 outstanding atoms between them. Their holders keep their balances. The validator refuses a burn from any of those buckets to this checker. The same rule applies to a new ingress revision after 8. The vault pins the verifier program ID and the verifier has one light PDA, so deploying an additional independent checker alone would not release from this vault.

The smallest change that preserves the deployed checker is a governed one-for-one migration into bucket 4c045e28. Authorize explicit source and target bucket IDs with the same family, full genesis, vault, mint and custody binding. Require both buckets healthy and redeemable at par. In one signed, atomic state transition, debit the holder's old series line, credit their revision-8 line, and move the same outstanding liability and its counted backing. Record the paired adjustments in the backing ledger so the source root can be independently reconstructed. Leave queued redemptions and their backing in the old bucket. Reject amounts above free, fully backed outstanding and any impaired or cross-domain balance. Preserve aggregate issued supply, counted reserves, NAV and outstanding plus queues. Cover transparent and shielded holdings through the existing series-specific unshield path before conversion. This is a separate governed change with new conservation and replay tests.
