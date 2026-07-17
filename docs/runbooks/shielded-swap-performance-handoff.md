# Shielded Swap Performance Handoff

Status: urgent handoff / performance triage
Date: 2026-06-21
Audience: protocol engineering, validator operators, next execution agent

## Summary

The live transparent NAV roundtrip and the private `ShieldedSwap` proof path are
different performance problems.

The transparent a651 <-> pfUSDC <-> Arbitrum USDC roundtrip has a measured
protocol runtime around `120.4s`. In that flow, the a651 NAV money-in checkpoint
is only one stage, measured around `8.5s`.

The current private a651 <-> pfUSDC shielded swap is much slower before it even
touches the WAN validators. The observed slow path is local:

- `asset-orchard-swap-create` spent roughly six minutes creating the local
  Halo2/Orchard swap action proof;
- the produced action had an Orchard anchor, two nullifiers, two output
  commitments, and a `6816` byte proof;
- after that, `shield-batch-swap` was still consuming CPU for minutes and had
  not yet emitted `batch.json`;
- the command had not reached `transport-peer-certified-batch-round` yet, so
  the bottleneck at that point was not WAN consensus, Arbitrum, or NAV
  checkpointing.

Do not compare the shielded local proof step to the `8.5s` a651 NAV money-in
checkpoint. They are different layers. The first is local ZK proof/batch
construction. The second is a transparent NAV accounting checkpoint inside the
already-automated roundtrip.

Important correction: the AssetOrchard prover was already optimized in the
prior ZK sprint. The optimized path exists, but the live one-shot CLI did not
actually experience the optimized hot path.

The prior optimization results are recorded in
`docs/status/zk-prover-optimization-results.md`:

```text
K=15 cached proving key
prove_ms      5,780
verify_ms        66
proof_bytes   6,816
```

The same report also records the remaining bad case:

```text
K=15 cold one-shot CLI path ~= 346.3s
```

That is the failure mode observed here. The live command used separate CLI
processes:

1. `asset-orchard-swap-create` paid the cold proving-key path, then created and
   verified the proof;
2. the process exited, so the in-process `OnceLock` proving-key cache was lost;
3. `shield-batch-swap` started a new process, rebuilt/loaded verifier state, and
   verified the same proof again before wrapping the batch;
4. only then did transport to the WAN validators begin.

So the system has an optimized hot prover, but the current live operator
workflow is not wired to use it. The fix is not "invent ZK optimization from
scratch"; it is to stop using cold one-shot commands for shielded swaps.

## What NAVCoins And a651 Are

NAVCoins are PFTL-issued assets whose accounting is tied to a NAV profile and a
reserve packet. The reserve packet publishes `verified_net_assets`, and PFTL
consensus enforces the NAV floor invariant:

```text
verified_net_assets >= circulating_supply * nav_per_unit
```

`nav_per_unit` is the floor of `verified_net_assets / circulating_supply`.
Remainders are over-collateralization, so the reserve does not need exact
equality to be safe.

`a651` is the real canonical NAVCoin used in the WAN devnet proof runs. It is
not a mock asset and must not be recreated or bootstrapped in a parallel local
chain. It was registered on the live `postfiat-wan-devnet` with an SP1-backed
floating NAV profile. At the relevant tested reserve/supply point, its NAV was
approximately USD `5.08` per unit.

Operationally:

- pfUSDC is the bridge-backed PFTL representation of Arbitrum USDC;
- subscribing pfUSDC into a651 primary-mints real a651;
- after that subscription, the a651 reserve packet must be recomputed so
  `verified_net_assets` increases by the subscribed value;
- exiting a651 back to pfUSDC reduces the a651 position and should be followed
  by the corresponding NAV money-out checkpoint;
- shielded swaps can rotate private notes representing a651 and pfUSDC, but the
  backing remains auditable at the transparent ingress/egress and reserve-packet
  boundaries.
- current Asset-Orchard egress is disclosed egress. It is a functional exit
  path, not private egress or private cash-out.

Canonical documentation:

- `docs/navcoin-sp1-verifier.md` explains the SP1-backed a651 NAV proof and the
  floating-NAV invariant.
- `docs/specs/otc-swaps-mvp-guide.md` defines the six-flow transparent MVP and
  states that real a651 on WAN devnet must be used.
- `docs/specs/private-otc-shielded-scope.md` defines the private NAV OTC layer:
  shield pfUSDC/a651, rotate them privately, keep reserves auditable.

## Current Live Context

Repo:

```text
$POSTFIAT_REPO
```

Release binary used by the run:

```text
./target/release/postfiat-node
```

Local validator mirror / runner data:

```text
$POSTFIAT_STATE/live-e2e-20260621T061254Z/runner-data-validator0
```

Current private swap run root:

```text
$POSTFIAT_STATE/private-a651-pfusdc-e2e-20260621T201931Z
```

Forward shielded swap artifact directory:

```text
$POSTFIAT_STATE/private-a651-pfusdc-e2e-20260621T201931Z/swap-forward
```

Topology:

```text
$POSTFIAT_STATE/live-e2e-20260621T061254Z/all-vultr-remote-topology.json
```

Input private notes for the forward swap:

```text
$RUN_ROOT/ingress-pfusdc/note.json
$RUN_ROOT/ingress-a651/note.json
```

Target certification height in the interrupted command:

```text
249
```

Proposal key used in the interrupted command:

```text
$POSTFIAT_STATE/otc-swaps-wan-20260619T011632Z/certifier-keys/validator-3.validator_keys.json
```

Do not print or copy private key material into docs, logs, or prompts.

## Transparent 120.4s Baseline

The transparent full Arbitrum roundtrip timing that matters for the NAV
performance plan is approximately:

| Stage | Observed time |
| --- | ---: |
| a651 NAV money-in checkpoint | `8.5s` |
| Exit a651 back to pfUSDC | `4.7s` |
| NAV money-out checkpoint | `9.1s` |
| Burn pfUSDC to redeem | `2.4s` |
| Withdrawal signature packet | `7.6s` |
| Arbitrum withdrawal proof/finalize/claim | `43.6s` |
| PFTL settle | `2.8s` |
| Final summary/verification | `4.1s` |
| Total full transparent roundtrip | `120.4s` |

This is a full economic path:

1. StakeHub wallet sends Arbitrum USDC into the bridge vault.
2. The deposit relays into PFTL and mints pfUSDC.
3. pfUSDC primary-mints real a651.
4. a651 verified net assets increase.
5. a651 exits back to pfUSDC.
6. a651 verified net assets decrease.
7. pfUSDC burns for redemption.
8. The EVM withdrawal proof/finalize/submit/finalize/claim path returns USDC.
9. PFTL settle closes the redemption.
10. Wallet/vault balances, NAV state, queue accounting, mempool, and validator
    convergence verify.

The a651 NAV money-in checkpoint is not the shielded swap. It is the transparent
NAV accounting verification after pfUSDC is subscribed into a651.

## Current Shielded Forward Command

The interrupted command chained three phases in one shell:

1. create the shielded swap action and proof;
2. wrap the action into a shielded batch;
3. certify the batch to the WAN devnet.

Command shape:

```bash
set -euo pipefail
BIN=./target/release/postfiat-node
DATA=$POSTFIAT_STATE/live-e2e-20260621T061254Z/runner-data-validator0
RUN_ROOT=$POSTFIAT_STATE/private-a651-pfusdc-e2e-20260621T201931Z
mkdir -p "$RUN_ROOT/swap-forward"

SEED_A=$(printf '%s' "$RUN_ROOT-swap-forward-output-a" | sha256sum | awk '{print $1}')
SEED_B=$(printf '%s' "$RUN_ROOT-swap-forward-output-b" | sha256sum | awk '{print $1}')

"$BIN" asset-orchard-swap-create \
  --data-dir "$DATA" \
  --input-note-file-a "$RUN_ROOT/ingress-pfusdc/note.json" \
  --input-note-file-b "$RUN_ROOT/ingress-a651/note.json" \
  --output-note-seed-hex-a "$SEED_A" \
  --output-note-seed-hex-b "$SEED_B" \
  --action-file "$RUN_ROOT/swap-forward/action.json" \
  --output-note-file-a "$RUN_ROOT/swap-forward/output-note-a.json" \
  --output-note-file-b "$RUN_ROOT/swap-forward/output-note-b.json" \
  --overwrite

"$BIN" shield-batch-swap \
  --data-dir "$DATA" \
  --swap-file "$RUN_ROOT/swap-forward/action.json" \
  --batch-file "$RUN_ROOT/swap-forward/batch.json"

"$BIN" transport-peer-certified-batch-round \
  --data-dir "$DATA" \
  --topology $POSTFIAT_STATE/live-e2e-20260621T061254Z/all-vultr-remote-topology.json \
  --batch-kind shielded \
  --batch-file "$RUN_ROOT/swap-forward/batch.json" \
  --key-file "$DATA/validator_keys.json" \
  --proposal-key-file $POSTFIAT_STATE/otc-swaps-wan-20260619T011632Z/certifier-keys/validator-3.validator_keys.json \
  --quorum-early-full-propagation \
  --artifact-dir "$RUN_ROOT/swap-forward/round" \
  --height 249 \
  --timeout-ms 180000 \
  --send-retries 3 \
  --retry-backoff-ms 1000
```

Important: because `asset-orchard-swap-create` is run with `--overwrite`,
re-running the combined command will redo the expensive local proof even if
`action.json` already exists. Do not restart this whole command blindly.

## What Was Observed

Observed during the interrupted run:

- `action.json` appeared after roughly six minutes;
- the action was locally verified enough to report:
  - one Orchard anchor;
  - two nullifiers;
  - two output commitments;
  - one `6816` byte proof;
- after action creation, the shell was inside `shield-batch-swap`;
- `shield-batch-swap` was still using CPU after roughly nine minutes total;
- `batch.json` had not appeared yet;
- therefore, the run had not reached the WAN certified transport round.

Interpretation:

- this is not an Arbitrum bottleneck;
- this is not the a651 NAV money-in checkpoint;
- this is not validator network latency yet;
- this is local proof creation plus local shielded batch wrapping/verification.

## Immediate Triage Commands

Set environment:

```bash
cd $POSTFIAT_REPO

export BIN=./target/release/postfiat-node
export DATA=$POSTFIAT_STATE/live-e2e-20260621T061254Z/runner-data-validator0
export RUN_ROOT=$POSTFIAT_STATE/private-a651-pfusdc-e2e-20260621T201931Z
export TOPO=$POSTFIAT_STATE/live-e2e-20260621T061254Z/all-vultr-remote-topology.json
export PROP_KEY=$POSTFIAT_STATE/otc-swaps-wan-20260619T011632Z/certifier-keys/validator-3.validator_keys.json
```

Check whether a proof/batch/transport process is still running:

```bash
ps -o pid,ppid,etime,pcpu,pmem,stat,args -u postfiat \
  | rg 'asset-orchard-swap-create|shield-batch-swap|transport-peer-certified-batch-round|postfiat-node'
```

Check current artifacts:

```bash
ls -lh "$RUN_ROOT/swap-forward"
test -s "$RUN_ROOT/swap-forward/action.json" && echo "action exists"
test -s "$RUN_ROOT/swap-forward/batch.json" && echo "batch exists"
test -d "$RUN_ROOT/swap-forward/round" && find "$RUN_ROOT/swap-forward/round" -maxdepth 2 -type f | sort
```

Inspect the public shape of the action without revealing private note material:

```bash
jq '{
  schema,
  anchor,
  nullifier_count: (.nullifiers // [] | length),
  output_commitment_count: (.output_commitments // [] | length),
  proof_bytes: ((.proof // "") | length / 2)
}' "$RUN_ROOT/swap-forward/action.json"
```

If `batch.json` exists, do not rebuild the batch. Go straight to transport.

If `action.json` exists but `batch.json` does not, do not rerun
`asset-orchard-swap-create`. Run only:

```bash
/usr/bin/time -p "$BIN" shield-batch-swap \
  --data-dir "$DATA" \
  --swap-file "$RUN_ROOT/swap-forward/action.json" \
  --batch-file "$RUN_ROOT/swap-forward/batch.json"
```

If `batch.json` exists and the round has not landed, run only:

```bash
/usr/bin/time -p "$BIN" transport-peer-certified-batch-round \
  --data-dir "$DATA" \
  --topology "$TOPO" \
  --batch-kind shielded \
  --batch-file "$RUN_ROOT/swap-forward/batch.json" \
  --key-file "$DATA/validator_keys.json" \
  --proposal-key-file "$PROP_KEY" \
  --quorum-early-full-propagation \
  --artifact-dir "$RUN_ROOT/swap-forward/round" \
  --height 249 \
  --timeout-ms 180000 \
  --send-retries 3 \
  --retry-backoff-ms 1000
```

## Correct Timing Split For The Shielded Swap

Do not time the entire private path as one opaque shell command. Split it:

| Phase | Command | Expected artifact | Notes |
| --- | --- | --- | --- |
| Swap proof creation | `asset-orchard-swap-create` | `action.json`, output notes | Currently the first major local bottleneck. |
| Batch construction | `shield-batch-swap` | `batch.json` | Currently a second local CPU-heavy stage. |
| WAN certification | `transport-peer-certified-batch-round` | round artifacts/certificate | Only starts after `batch.json` exists. |
| Final state verification | status/archive queries | validator height/state root, action visible | Confirms the live chain accepted the action. |

Use `/usr/bin/time -p` around each command and write a tiny timing file next to
each artifact:

```bash
{ /usr/bin/time -p "$BIN" shield-batch-swap ... ; } \
  2> "$RUN_ROOT/swap-forward/timing.shield-batch-swap.txt"
```

The next performance report must not say "WAN was slow" unless the timed
`transport-peer-certified-batch-round` stage is actually the slow stage.

## What To Optimize First

Priority order:

1. Preserve the optimized hot path in live operation.
   - Run proof creation and batch wrapping in one long-lived process, or add a
     local prover daemon, so the cached K=15 proving/verifying keys survive
     across swaps.
   - The target is to expose the already-measured `~5.8s` hot proof path to the
     operator workflow instead of the documented `~346s` cold CLI path.

2. Avoid accidental reproving.
   - Never rerun the combined command with `--overwrite` when `action.json`
     already exists.
   - Add resume behavior around `action.json` and `batch.json`.

3. Remove duplicate proof verification.
   - `asset-orchard-swap-create` already verifies the action it writes.
   - `shield-batch-swap` currently verifies the same `AssetOrchardSwapAction`
     again before wrapping it.
   - For live tooling, either combine the two steps so verification is done once
     inside one process, or write an attested action report that lets the batch
     wrapper skip local duplicate verification while consensus still verifies.

4. Measure `shield-batch-swap`.
   - Determine whether it is proving again, verifying the swap proof, rebuilding
     expensive Orchard state, or doing redundant archive/state work.
   - This is the immediate unknown after the six-minute action proof.

5. Cache or precompute static proving material.
   - If the swap prover regenerates parameters, verifying keys, pinned metadata,
     circuit layouts, or tree witnesses per run, move those out of the hot path.
   - Any cache must be keyed by circuit id, verifying-key hash, pool domain,
     protocol version, and chain/genesis binding.

6. Parallelize independent local work.
   - Output-note preparation, encrypted-output construction, and non-consensus
     serialization work can be prepared before the blocking proof if the witness
     dependencies allow it.
   - Do not parallelize state-affecting consensus application.

7. Add a dedicated benchmark command.
   - The CLI should emit a JSON timing report for:
     `swap_proof_ms`, `batch_wrap_ms`, `transport_ms`, `final_verify_ms`.
   - The report must include binary hash, git commit, data-dir state root,
     anchor, proof byte length, batch byte length, target height, and validator
     convergence.

8. Only after local phases are measured, tune WAN transport.
   - If transport is slow, inspect quorum polling and propagation.
   - If transport is fast, leave WAN alone and focus on proof/batch CPU.

## What Not To Do

- Do not call the private shielded swap a `120s` flow until measured. Current
  evidence says it is already longer than that locally.
- Do not compare `asset-orchard-swap-create` to the `8.5s` a651 NAV money-in
  checkpoint.
- Do not rerun the combined command from the top if `action.json` exists.
- Do not use local/no-value proof timings as the live-value result.
- Do not use `sshpass`.
- Do not expose raw private keys.
- Do not claim WAN consensus is the bottleneck before `batch.json` exists.

## Handoff Checklist

Before continuing the live private swap, answer these from artifacts:

- Is a process still running?
- Does `swap-forward/action.json` exist and have nonzero size?
- Does `swap-forward/batch.json` exist and have nonzero size?
- Did `swap-forward/round` produce a certificate or accepted batch report?
- What was the measured wall time for:
  - `asset-orchard-swap-create`;
  - `shield-batch-swap`;
  - `transport-peer-certified-batch-round`;
  - final state verification?
- Did all validators converge at the target height after transport?
- Does the on-chain action reveal only nullifiers, commitments, proof material,
  and pool metadata?
- Are transparent NAV reserves unchanged except for the already-visible ingress
  and exit boundaries?

Only after those are answered should the reverse swap be started.

## Required Next Code/Tooling Fix

The immediate tooling gap is that the shielded swap live command is not
hot-path-preserving or resume-safe. Add a runner or wrapper that:

- refuses to overwrite `action.json` unless explicitly asked;
- skips proof creation when `action.json` already exists and passes structural
  validation;
- skips batch wrapping when `batch.json` already exists and passes structural
  validation;
- keeps AssetOrchard proving/verifying keys warm across proof creation and batch
  wrapping;
- avoids duplicate local proof verification while preserving consensus
  verification;
- records per-stage timings into a machine-readable report;
- records whether each stage was fresh or reused;
- refuses to certify if the local data-dir anchor is stale relative to the
  selected WAN height;
- verifies final height/state-root convergence after certification.

This is the fastest way to stop losing operator time while the real proof
performance work is investigated.
