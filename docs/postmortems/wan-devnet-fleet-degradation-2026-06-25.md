# WAN Devnet Fleet Degradation - 2026-06-25

## Summary

On 2026-06-25 the `postfiat-wan-devnet` validator fleet degraded after the rolling binary update that deployed the cash-included NAV decoder across validators 0-5. The fleet reached a state where the shielded NAV swap bridge-in certification round collected only one block vote.

The failing StakeHub live E2E report was:

- `$STAKEHUB_STATE/shielded-nav-swap/e2e-script/stakehub-shielded-nav-swap-e2e-20260625T013941Z.json`

The bridge-in failure was:

```text
peer certified batch round certificate failed: insufficient block votes: got 1, need 5
```

The direct impact was that every shielded NAV swap attempt failed at step 5, `Bridge USDC into PFTL as pfUSDC`, while the fleet was below quorum. Those retries consumed operator time and made the demo look like a swap or note/nullifier failure even though the immediate blocker was L1 finality quorum.

## What Broke

The rolling binary update changed the validator fleet before the post-rollout quorum was proven. Validator 0 could still propose and vote, but validators 1-5 did not return usable block votes for the bridge-in certified round. With only 1/6 validators voting, the round could not meet the 5-vote quorum required by the demo topology.

This was not an Ethereum, USDC, or Orchard note issue. The bridge-in action failed before the private swap because the PFTL WAN devnet could not certify the PFTL bridge batch.

## Root Cause By Validator

The observed failure mode was missing vote responses from validators 1-5 after the rolling update. The operational root cause was rollout safety: the update did not stop after each validator to prove process restart, state convergence, and vote participation before continuing to the next validator.

| Validator | Failure mode during incident | Recovery action | Post-recovery state |
| --- | --- | --- | --- |
| validator-0 | Only validator producing a usable vote in the failed bridge-in round. | Kept as local proposer/reference, then included in final convergence check. | Voting and converged. |
| validator-1 | Did not return a usable vote for the failed bridge-in round after rollout. | Re-deployed/restarted validator services and rechecked status/root. | Voting and converged. |
| validator-2 | Did not return a usable vote for the failed bridge-in round after rollout. | Re-deployed/restarted validator services and rechecked status/root. | Voting and converged. |
| validator-3 | Did not return a usable vote for the failed bridge-in round after rollout. | Re-deployed/restarted validator services and rechecked status/root. | Voting and converged. |
| validator-4 | Did not return a usable vote for the failed bridge-in round after rollout. | Re-deployed/restarted validator services and rechecked status/root. | Voting and converged. |
| validator-5 | Did not return a usable vote for the failed bridge-in round after rollout. | Re-deployed/restarted validator services and rechecked status/root. | Voting and converged. |

The remediation evidence after restart/redeploy showed all six validators converged:

```text
validator-0 height 431 root 2bcaa5015332b2e7714c35df
validator-1 height 431 root 2bcaa5015332b2e7714c35df
validator-2 height 431 root 2bcaa5015332b2e7714c35df
validator-3 height 431 root 2bcaa5015332b2e7714c35df
validator-4 height 431 root 2bcaa5015332b2e7714c35df
validator-5 height 431 root 2bcaa5015332b2e7714c35df
```

## How It Was Fixed

The validator binaries and services were rechecked across all six WAN nodes, then validators that were not participating cleanly were restarted/re-synced. The fleet was not considered healthy again until all six validators reported the same height and state root.

After the fleet recovered, the shielded NAV swap was rerun twice in a row:

- `$STAKEHUB_STATE/shielded-nav-swap/e2e-script/stakehub-shielded-nav-swap-e2e-20260625T034439Z.json`
- `$STAKEHUB_STATE/shielded-nav-swap/e2e-script/stakehub-shielded-nav-swap-e2e-20260625T040738Z.json`

Both reports completed 12/12 steps with `ok=true`.

## Impact

The degraded fleet caused the shielded-swap demo to thrash. Each attempted live E2E hit the same bridge-in certification failure, which prevented pfUSDC from being minted into PFTL for the run. Because fresh bridge-in and fresh shielded notes depend on that certification, the rest of the demo could not be meaningfully tested until the L1 fleet was restored.

This also obscured a separate repeatability issue in the StakeHub flow: once the fleet was healthy, the demo still needed fresh bridge-in/shield/liquidity behavior so repeated runs would not reuse spent Orchard nullifiers. That application fix was validated only after WAN quorum was restored.

## Prevention

Future WAN devnet rolling updates need an explicit quorum gate:

1. Update only one validator at a time.
2. After each validator restart, verify the process is running, the node is synced, and the validator can produce a block vote.
3. Before proceeding to the next validator, confirm the fleet still has at least five voting validators.
4. After the rollout, run a post-rollout certified batch round and block release/demo activity if quorum is below five.
5. Keep a rollback path: retain the previous binary, service command, and validator data snapshot reference until the post-rollout quorum check passes.
6. Record per-validator height, state root, binary hash, service status, and vote result in the rollout log.

For shielded-swap demos, the release checklist should also include a simple pre-demo finality probe before any bridge or Orchard actions are attempted. If the probe cannot collect quorum, the demo should fail fast with an infrastructure error instead of starting the swap.
