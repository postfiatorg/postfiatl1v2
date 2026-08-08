# A666 Live Funds Execution Handoff

- **Prepared:** 2026-08-08 UTC
- **Operational source of truth:** [A666 End-to-End Live Funds Runbook](../runbooks/A666-END-TO-END-LIVE-FUNDS-RUNBOOK-20260807.md)
- **PFTL main at audit:** 65c0e719e4e88f41f4b10d950f08acba90b44e82
- **StakeHub master at audit:** 63824784b4d3bbe8d086066919ec31cfe77e3dd8
- **Live-loop verdict:** incomplete; finalized after leg 1, stopped before leg 2a
- **Authorization effect:** this handoff documents state and recovery boundaries. It does not replace held-packet receipt gates.

## READ THIS FIRST

The actual objective is a real-money round trip:

~~~text
Ethereum USDC -> PFTL pfUSDC -> A666 -> wA666 -> Uniswap
-> wA666 -> PFTL A666 -> pfUSDC -> Ethereum USDC
~~~

Do not reset the fleet, recreate A666, or substitute a mocked route. Literal
A666 is deployed, the wallet holds 103.000000 wA666, and the live route is
pftl-a666-ethereum-wA666-usdc-v1.

The campaign has moved real money. The 10.000000 USDC deposit and PFTL claim are
final. The complete loop is still open. The next executor starts at E6 NAV
refresh, then leg 2a. Never repeat the deposit, claim, or rollout.

## DONE

1. PFTL production code, orchard accounting fix, depositV2 client, packets,
   runbook, and dependencies were integrated into main.
2. A durable capability validation passed:
   - full PFTL tests 424/424
   - node library tests 267 pass, 0 fail, 2 ignored
   - execution tests 13/13
   - Python campaign tests 84 pass, 1 skip
   - packet hashes 16/16
   - EVM fork legs 3c through 3h status 1
3. Six validators were upgraded to orchard-aware binary
   25e607595e581e7d435c6282f2db95aa473cf61877a7d75cea73505ea697c7f4.
4. Ethereum deposit transaction
   0x016f9c5f9b99fc951cea7c539f7f791c2d753d35793296b2e284f96512575924
   mined at block 25,698,310.
5. PFTL claim transaction
   46dde341b0a5eb6dc9359e40bfbf0cc7f7dd489b9fc2bb915c17a8948f675917e635cf05557a5a53944039c7771b9afd
   finalized at height 779.
6. StakeHub existing-reader support was merged through PR 6.
7. The SP1 aggregate prover was rebuilt and a real CPU Groth16 prove completed.
8. Solana trust semantics were corrected in the campaign record: the current
   leg is self-attested, not trustless.

## LIVE STATE

Read-only snapshot taken 2026-08-07 23:46 UTC:

| Item | Value |
|---|---|
| Fleet | 6/6, height 779, mempool 0 |
| Common state root | 2a2a9bf6a7aca98b45e9daadd9b233045ffc225a26eda380233964a56c6e894ce598a198279a24bc52386fc597777b71 |
| Holder PFUSDC | 11,358,493 atoms = 11.358493 pfUSDC |
| pfUSDC cap/supply | 297,859,297 atoms = 297.859297 pfUSDC, epoch 45 |
| Holder A666 | 99,000,000 atoms = 99.000000 A666, pre-existing |
| Ethereum USDC | 74,161,443 atoms = 74.161443 USDC |
| Ethereum wA666 | 103,000,000 atoms = 103.000000 wA666 |
| Ethereum nonce | 304 |
| A666 active reservations | 0 |
| A666 export entitlements | 0 |
| Return-import claims | 0 |
| StakeHub agent | active, patched, locked |

The 103.000000 wA666 balance is a protected baseline. It is proof that A666
export happened before this campaign. It must never be spent as a substitute
for receipt-chained campaign output.

## BLOCKED

Leg 2a batch-only admission returned stale_pftl_uniswap_pricing. No leg-2
transaction was broadcast. E5 NAV pricing is too old for the route's 100-block
freshness gate.

A fresh E6 capture and proof were created, but the aggregate policy hash did not
match the active PFTL profile. The cause is fully isolated:

- current capture used a newly deployed Hyperliquid reader instead of the
  policy-bound legacy reader;
- current capture used the newer XMR leg form instead of the E5-compatible
  sidecar topology.

The proof is valid for its own bytes and unusable for the active profile. Never
submit it.

The wallet agent was migrated to merged code so it can call the existing
Hyperliquid reader. Restart cleared its in-memory unlock and policy.

## ONE REQUIRED HUMAN ACTION

~~~bash
stakehub agent unlock
~~~

The passphrase must enter interactively. Never place it in a command, file,
environment variable, report, or chat.

After unlock, the operator must restore and verify the exact required policy.
The post-restart process currently reports default caps and empty whitelists.

## NEXT, IN ORDER

1. Verify agent singleton, unlock state, policy, and empty active-session state.
2. Land the NEAR BlockHeaderV6 threshold fix with a failing-then-passing
   current-mainnet regression.
3. Use StakeHub master@63824784 for the existing-reader custody surface.
4. Use detached StakeHub source 213618e for E5-compatible aggregation.
5. Open one governed existing-reader session for
   0xd5c4200b74929952dca4db70fdc65317c2705207.
6. Require 9,006-byte runtime code and SHA-256
   2e49ae2b32f2598c8a77a3b234180101191396c77046e50498dda1df68bbe713.
7. Submit one fresh Hyperliquid snapshot at no more than 0.02 HYPE.
8. Close and reconcile the session.
9. Aggregate with XMR sidecar and the explicitly attested Solana witness.
10. Require policy
    076c071e44127158ef82350e7feeb64e0be0a06bf8ba4be5f0374ac36b992ac7.
11. Require zero reconciliation residual and exact vkey
    0x00580ee8c389192568a29dc23d54c22e73a3a45203b22e3d5a934801871e11a7.
12. Prove, verify, build E6 operations, batch-only, and finalize once.
13. Build S1f from final E6 values and a fresh four-hour swap deadline.
14. Resume at leg 2a, then continue receipt-by-receipt through bridge-out.
15. Finish only after external Ethereum USDC credit and conservation evidence.

## PROJECTED E6 ECONOMICS

Current fresh public values project NAV 90,365,059 USD_1E8.

| Mint | Required pfUSDC |
|---:|---:|
| old 11,027,135 atoms | 10,014,502 atoms; breaches bound |
| 11,011,167 atoms | 10,000,000 atoms; maximum compliant projection |
| 11,011,168 atoms | 10,000,001 atoms; breaches by one atom |

Recompute from the final policy-compatible E6 proof. S1f does not exist yet.

## NEVER DO

- Never repeat stage 0, the Ethereum deposit, or the PFTL claim.
- Never fire S1, S1b, S1c, S1d, or S1e.
- Never submit the policy-mismatched proof.
- Never use readers 0x48317aa089a674506e92cc64734b721175dfef79 or
  0xEbAcc5b43351F18ff605586Afc7dDAbc2ca09dFF.
- Never describe current Solana evidence as trustless finality.
- Never execute from the dirty original StakeHub checkout.
- Never reuse stale signatures, sessions, prices, nonces, or deadlines.
- Never use the old 11,027,135 mint without fresh arithmetic.
- Never touch the protected 103.000000 wA666 baseline.
- Never claim completion before bridge-out credits Ethereum USDC.

## REPOSITORY SAFETY

Use:

- /home/postfiat/repos/pftl-validation-20260807
- /home/postfiat/repos/StakeHub-master-e6
- /home/postfiat/repos/StakeHub-e6-213618e

The original StakeHub checkout contains unrelated tracked and untracked work.
Never bulk-add, clean, reset, or revert it.

The NEAR threshold change currently exists only in that dirty checkout. Treat it
as a patch source, not merged production code.

## EVIDENCE

- Canonical runbook:
  ../runbooks/A666-END-TO-END-LIVE-FUNDS-RUNBOOK-20260807.md
- Binding history:
  ../evidence/a666-public-reserve-product-20260803/live-demo/native-v1/fire-20260806/
- Validation:
  ../evidence/a666-public-reserve-product-20260803/live-demo/native-v1/validation-20260807/
- Fresh E6 run:
  /home/postfiat/repos/a666-eth-fast-lane-combined-20260724/docs/evidence/a666-public-reserve-product-20260803/nav-e6-fresh/20260807T195819Z/
- Stage-0/claim runtime:
  /tmp/krimp-exec-fire20260806/leg-1/ and /tmp/krimp-exec-s1c/leg1/
- E6 root-cause reports:
  /tmp/ghash-e6-recon/, /tmp/ghash-e6-policy/, /tmp/ghash-e6-econ/
- Agent migration:
  /tmp/krimp-e6-hl/restart-window.txt

Temporary evidence must be archived redaction-safe before reboot or cleanup.

## COMMUNICATION RULE

Report one of three things:

1. a finalized live receipt and exact before/after values;
2. an active long-running proof with PID/start/progress evidence;
3. a genuine fail-closed STOP with mutation reconciliation and the next
   engineering action.

Never report generated files as transactions, batch-only as broadcast, or a fork
as live money.
