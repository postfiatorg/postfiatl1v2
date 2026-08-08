# A666 End-to-End Live Funds Runbook and Current Handoff

> **Canonical operational truth as of 2026-08-07 23:46 UTC.**
>
> This document supersedes the 2026-08-05 recovery handoffs for current
> operations. Historical evidence remains immutable. The end-to-end live loop is
> **incomplete**. Stage 0 and stage 1 are finalized. Legs 2 through 8 have not
> executed. Never infer completion from an Anvil receipt, a batch-only result, a
> generated packet, or a proof artifact.
>
> Every live mutation remains fail-closed: one held packet, one execution,
> receipt and conservation gates, then the next packet. Any deviation means
> **STOP-no-retry**.

## 0. One-screen handoff

### DONE

- Production code and dependencies were integrated into PFTL main.
- The end-to-end capability validation was published on PFTL main.
- The six-validator fleet was rolled to the orchard-aware release.
- The Ethereum deposit of 10,000,000 USDC atoms (10.000000 USDC) was mined.
- The corresponding PFTL claim finalized at height 779.
- The holder balance increased from 1,358,493 atoms (1.358493 pfUSDC) to
  11,358,493 atoms (11.358493 pfUSDC).
- The pfUSDC cap increased from 287,859,297 atoms (287.859297 pfUSDC) to
  297,859,297 atoms (297.859297 pfUSDC), epoch 45.
- StakeHub master now contains the additive, code-identity-bound Hyperliquid
  existing-reader mode.
- A fresh six-leg reserve witness was captured and a real CPU SP1 Groth16 proof
  completed in about 25 minutes. That proof is discard-only because its policy
  hash differs from the active PFTL profile.

### LIVE STATE

- PFTL fleet: 6/6 at height 779, common state root
  2a2a9bf6a7aca98b45e9daadd9b233045ffc225a26eda380233964a56c6e894ce598a198279a24bc52386fc597777b71.
- Fleet binary SHA-256:
  25e607595e581e7d435c6282f2db95aa473cf61877a7d75cea73505ea697c7f4.
- Fleet mempool: 0 on all six validators.
- Holder PFUSDC: 11,358,493 atoms (11.358493 pfUSDC).
- Holder transparent A666: 99,000,000 atoms (99.000000 A666), pre-existing
  campaign state.
- Ethereum wallet:
  - USDC: 74,161,443 atoms (74.161443 USDC)
  - wA666: 103,000,000 atoms (103.000000 wA666)
  - ETH: 289,735,632,642,339,914 wei (0.289735632642339914 ETH)
  - nonce: 304
- Active A666 route reservation count: 0.
- Active export entitlement count: 0.
- Pending return-import claims: 0.
- Pricing remains E5: NAV epoch 5, NAV 90,234,207 USD_1E8.
- StakeHub wallet agent service is active on merged code and locked.

### BLOCKED

Leg 2a correctly failed before broadcast with
stale_pftl_uniswap_pricing. PFTL height 779 is outside the route's 100-block
pricing freshness guard from the E5 mark. The next valid action is a fresh E6
NAV mark under the active E5-compatible proof policy. The wallet agent restart
needed to load merged Hyperliquid existing-reader support cleared the in-memory
vault unlock.

The only human-held dependency is:

~~~bash
stakehub agent unlock
~~~

After unlock, restore and verify the required agent policy before any custody
operation. The current post-restart defaults are per-transaction USD cap 2,000,
daily USD cap 5,000, empty contract whitelist, and empty verifier whitelist.
Never assume the pre-restart in-memory policy survived.

### NEXT

1. Unlock the running StakeHub wallet agent.
2. Verify singleton service, unlocked=true, and the exact required policy.
3. Land and test the NEAR BlockHeaderV6 threshold fix described in section 12.
4. Open the specialized governed existing-reader session.
5. Capture a fresh Hyperliquid receipt against the legacy reader pinned by the
   active policy.
6. Rebuild the E6 witness using the E5-compatible policy topology.
7. Require exact policy hash, zero reconciliation residual, pinned vkey, and
   successful Groth16 verification.
8. Finalize E6 on PFTL.
9. Build a fresh S1f binding using E6 economics and a fresh swap deadline.
10. Resume at leg 2a. Never repeat stage 0 or leg 1.

## 1. Human objective

Complete one real-money loop using the already deployed literal A666 route:

~~~text
Ethereum USDC
  -> PFTL pfUSDC
  -> primary A666
  -> Ethereum wA666
  -> Uniswap wA666/USDC
  -> wA666
  -> PFTL A666
  -> pfUSDC
  -> Ethereum USDC
~~~

The route already exists. The wallet already holds 103.000000 wA666. No reset,
replacement chain, synthetic route, or mocked asset proves this objective.

Final acceptance requires a real Ethereum USDC credit after bridge-out, plus
exact PFTL and EVM conservation evidence. The current campaign has not reached
that acceptance condition.

## 2. Architecture and trust boundaries

### 2.1 Sources of truth

- **PFTL:** workflow state, economic state, NAV policy, issue and redemption
  receipts, supply caps, bridge state, and finality.
- **Ethereum mainnet:** USDC and wA666 balances, vault deposits, controller
  events, Uniswap transactions, and external bridge-out settlement.
- **StakeHub:** operator custody, policy-scoped signing, evidence capture, and
  reserve-proof host tooling. StakeHub does not decide PFTL economic state or
  replace PFTL receipts.
- **Repository evidence:** authorization bindings, packet hashes, command
  provenance, validation logs, and immutable history. Evidence describes state;
  it does not override live chain reads.

StakeHub remains a product. Decoupling means removing it as NAVCoin workflow or
economic authority. It does not mean deleting StakeHub, its custody vault, its
services, or its operator functions.

### 2.2 Solana is attested

The active Solana leg is **attested**, not a trustless Solana-finality proof.

The producer fetches finalized RPC account bytes, creates a signed RPC snapshot,
and requires a separate wallet-ownership signature. The SP1 guest verifies the
Ed25519 signatures, parses the stake-account bytes, and performs accounting. It
does not verify a Solana validator set, bank hash, consensus certificate, or
state-root proof. The implementation labels quantity and valuation
TrustTier::Attested and permits one SelfAttested source.

Code anchors:

- StakeHub/zk/script/src/bin/prepare_solana_attested_witness.rs:100-147
- StakeHub/zk/shared/src/solana_leg.rs:116-123
- StakeHub/zk/shared/src/solana_leg.rs:198-205
- StakeHub/zk/shared/src/solana_leg.rs:297-344

Any report describing the current Solana leg as trustless is wrong.

### 2.3 Hyperliquid reader identity is policy-bound

The active E5 policy commits to legacy reader:

~~~text
0xd5c4200b74929952dca4db70fdc65317c2705207
~~~

Expected runtime code:

- 9,006 bytes
- SHA-256
  2e49ae2b32f2598c8a77a3b234180101191396c77046e50498dda1df68bbe713

Deploying a new reader changes the aggregate policy preimage. StakeHub
master@63824784b4d3bbe8d086066919ec31cfe77e3dd8 adds an explicit
existing-reader mode with address, code-size, and code-hash gates. Ordinary
hl-snapshot behavior still deploys a new reader by default.

Two unintended readers were deployed while isolating this defect:

- 0x48317aa089a674506e92cc64734b721175dfef79
- 0xEbAcc5b43351F18ff605586Afc7dDAbc2ca09dFF

They are inert campaign artifacts. They have no PFTL consensus effect and must
never enter the E6 witness. Preserve their evidence; never reuse or represent
them as the policy-bound reader.

## 3. Repository and release state

| Component | Canonical branch / commit | Role |
|---|---|---|
| PFTL source and campaign docs | origin/main@65c0e719e4e88f41f4b10d950f08acba90b44e82 | Current integration and S1e evidence |
| StakeHub source | origin/master@63824784b4d3bbe8d086066919ec31cfe77e3dd8 | Existing-reader support merged by PR 6 |
| Orchard fleet source | 540b2c1c739affd0f33da0be9fd5f9a92c3c8673 | Deployed PFTL lineage |
| Orchard fleet binary | 25e607595e581e7d435c6282f2db95aa473cf61877a7d75cea73505ea697c7f4 | Live on 6/6 validators |
| Main feature twin | 16621fa94a4a29c637574180800777fa4ed0e1b5 | Reachable from PFTL main |
| Production EVM client | e1f84b1f42dc901b22bacb16196f8ff09609ddcbc5af862d62a44c5db60bf9d8 | depositV2 and route binding |
| E5-compatible StakeHub source | 213618eed92641d9b8974e26d80fab60ce8f2ecb | Reproduces active aggregate topology |
| E6 ops builder source | 57ec4168c5472ee98bbe1c466a5ff9b1d30ec80f | Builds legacy-profile NAV operations |

Canonical clean worktrees:

- /home/postfiat/repos/pftl-validation-20260807
- /home/postfiat/repos/StakeHub-master-e6
- /home/postfiat/repos/StakeHub-e6-213618e

The original /home/postfiat/repos/StakeHub checkout is heavily dirty and is not
an execution or integration source. Never clean, reset, or bulk-stage it.

## 4. Live mutation ledger

| Stage | Result | Live mutation | Evidence / identifier |
|---|---|---|---|
| Ethereum deposit approval | FINAL | Allowance set for 10,000,000 USDC atoms | 0x5fa27d3a489f91ea5eed738386447b2717be8a4104cfc9d86482cc69242cb3b5 |
| Ethereum vault deposit | FINAL | 10,000,000 atoms (10.000000 USDC) deposited | 0x016f9c5f9b99fc951cea7c539f7f791c2d753d35793296b2e284f96512575924, block 25,698,310 |
| PFTL deposit propose | FINAL | Deposit entered PFTL at height 777 | e54713583c1bb46e908e8f01f1c996966dc5c82281365c91092a27ae0852d02b |
| PFTL deposit finalize | FINAL | Deposit finalized at height 778 | fire-20260806 evidence |
| Stage 0 rollout | FINAL | Six validators upgraded | live status binary SHA above |
| PFTL claim | FINAL | 10,000,000 pfUSDC atoms credited at height 779 | 46dde341b0a5eb6dc9359e40bfbf0cc7f7dd489b9fc2bb915c17a8948f675917e635cf05557a5a53944039c7771b9afd |
| Leg 2a attempts | NO MUTATION | Batch-only admission stopped before broadcast | stale_pftl_uniswap_pricing |
| E6 proof attempt 1 | HOST ONLY | Real proof generated; policy-mismatched and quarantined | run directory in section 9 |
| Hyperliquid readers | EVM GAS ONLY | Two unintended inert readers deployed | quarantined addresses in section 2.3 |
| Legs 2a through 5b | UNEXECUTED | No reservation, subscription, export, swap, return, redeem, or bridge-out | route counters remain zero |

Never report the complete live loop as successful. It is stopped after the
finalized claim and before the A666 reservation.

## 5. Current live state snapshot

Snapshot time: 2026-08-07 23:46 UTC.

### 5.1 PFTL fleet

All forwarded RPCs agreed:

| Validator port | Height | State root prefix | Mempool |
|---|---:|---|---:|
| 39660 | 779 | 2a2a9bf6a7aca98b | 0 |
| 39651 | 779 | 2a2a9bf6a7aca98b | 0 |
| 39652 | 779 | 2a2a9bf6a7aca98b | 0 |
| 39653 | 779 | 2a2a9bf6a7aca98b | 0 |
| 39654 | 779 | 2a2a9bf6a7aca98b | 0 |
| 39655 | 779 | 2a2a9bf6a7aca98b | 0 |

Other pins:

- chain: postfiat-wan-devnet-2
- genesis:
  ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad25521aff3ed334da07e150a7233a3e90a9
- build revision: 2246d257
- deployment manifest SHA-256:
  661b775c83dd86d2aabdb3f7f9bf6f99a79b5e08a649bb61a150da811e3c3a35

### 5.2 PFTL account and route

Holder: pfab9b9228942e5c529633a13aa271d5297bec6353

| Asset | Atoms | Human units | Campaign meaning |
|---|---:|---:|---|
| PFUSDC | 11,358,493 | 11.358493 pfUSDC | Includes finalized 10.000000 claim |
| A666 | 99,000,000 | 99.000000 A666 | Pre-existing; leg 2 has not changed it |
| a666 | 100,495 | 0.100495 a666 | Separate historical code; never conflate |

pfUSDC:

- circulating supply: 297,859,297 atoms (297.859297 pfUSDC)
- issued supply: 297,859,297 atoms (297.859297 pfUSDC)
- finalized epoch: 45
- supply equals cap

A666 route:

- route: pftl-a666-ethereum-wA666-usdc-v1
- policy epoch: 6
- pricing NAV epoch: 5
- pricing packet:
  78c9fce35e0dc1ad9a7bd5b25b34e432c606fc38889e1543d5242cff12fe637bb0a508aff3655d2e2e27ac872f88f289
- active reservation count: 0
- active reservation atoms: 0
- export entitlement count: 0
- pending return-import claim atoms: 0
- route paused: false

### 5.3 Ethereum wallet

Wallet: 0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0

| Item | Current value |
|---|---:|
| USDC | 74,161,443 atoms (74.161443 USDC) |
| wA666 | 103,000,000 atoms (103.000000 wA666) |
| ETH | 0.289735632642339914 ETH |
| nonce | 304 |

The 103.000000 wA666 baseline is protected. Campaign packets must use only
receipt-chained new output and must never debit this baseline by substitution.

### 5.4 StakeHub agent

- service: stakehub-pfusdc-wallet-agent.service
- state: active/running
- MainPID at snapshot: 1,975,132
- code worktree: /home/postfiat/repos/StakeHub-master-e6
- override:
  ~/.config/systemd/user/stakehub-pfusdc-wallet-agent.service.d/e6-override.conf
- unlock: false
- on-disk journal preserved across restart
- in-memory spent_today_usd reset to 0; that does not reset campaign accounting
- current policy: per-tx 2,000 USD, daily 5,000 USD, empty whitelists

## 6. What validation proved

Durable report:

docs/evidence/a666-public-reserve-product-20260803/live-demo/native-v1/validation-20260807/END-TO-END-VALIDATION-REPORT.md

Results:

- cargo check workspace: pass
- PFTL execution tests: 13/13
- PFTL node library tests: 267 pass, 0 fail, 2 ignored
- PFTL full tests: 424 pass, 0 fail across 13 binaries
- Python campaign tests: 84 pass, 1 skip
- packet hashes: 16/16
- claim batch-only on a fresh clone: pass
- local Ethereum fork legs 3c through 3h: receipt status 1
- evidence manifest: 103/103
- secret scan: clean

The verdict remains PARTIAL PASS. Validation did not execute PFTL legs 2a
through 5a, the constrained-signer burn, or final external withdrawal.

## 7. Binding history

| Stage | Binding SHA-256 | Status |
|---|---|---|
| S1 | cc1bd291543e45e59fa2ff89df7e5c041c8ed101d6f93fb7e0eac57dd134bf9c | Immutable source; never execute |
| S1b | 8a5ac848b43e0cfa5e52121f434c0ac322aabae467b078c86e517aaaa9ea9b52 | Expired/unsafe |
| S1c | d8f1ccdb12d055560bb7805cbb7573b023ad35fa1ae6a163c61f088b009d38ab | Leg 0 and leg 1 history |
| S1d | 50b6acc5ceccf67d784a88f0f6877012d2ae1f77534bb46b718d59a7d4dbea3c | Superseded ID repair |
| S1e | f29bcfa2f88c77e09854d3d64990496b11635022f4055f0fb689fc32280c8505 | Committed pre-E6 normalization; do not fire |
| S1f | absent | Build after E6 finalizes |

S1f is mandatory because fresh E6 economics change the mint and every
downstream amount, the previous deadlines expired, and S1e still binds E5.

## 8. Fail-closed rulings and defects

| Ruling | Defect or decision | Disposition |
|---|---|---|
| R1 | Resolver dropped post-resolution leg-1 hardening | Base-first overlay preserved committed leaves |
| R2 | Staged-field metadata dropped during refresh | Byte-identical exemptions carried |
| R3 | Pre-D1 metadata pointed to stale command layout | Vestigial entries dropped and logged |
| R4 | Orchard staging lacked public deployment record | Public-only record copied after hash verification |
| R5 | Manifest signed old-stage units and environments | Bound rewrite and manifest regeneration; 6/6 verify |
| R6 | Reservation ID was 64 hex; admission requires 96 | Fresh 96-hex ID, no live submission |
| R7 | EVM recipients used mixed case | S1e normalized active recipients |
| R8 | Overlay needed exact copied-leaf proof | Exact leaf equality recorded |
| R9 | Staged metadata accounting changed | Copy/drop ledger and coverage proof recorded |
| R10 | Legacy guest has one price source, no quorum schema | Preserve E1-E5 semantics; record provenance outside proof |
| R11 | Account seeds and custody binding missing from E5 session | Recovered from retained evidence |
| R12 | SP1 build lacked protoc and detached launch died | Static protoc installed; instrumented build passed |
| R13 | Tooling always deployed a new HL reader | Existing-reader support merged |
| R14 | Agent restart required to load R13 | Service migrated; expected locked state |

Declared open defects:

- First E6 proof policy hash is
  12ffba137fc636f73c29a848621bdda12d44b6e9d70bf79cb874f361362945a9,
  while active policy requires
  076c071e44127158ef82350e7feeb64e0be0a06bf8ba4be5f0374ac36b992ac7.
- Legacy NEAR verifier uses V6 only at protocol 150. Current mainnet protocol 86
  uses V6 encoding. Correct threshold 85 exists only as an uncommitted dirty
  checkout change and needs a regression test plus merge.
- Two leg-2a batch-only attempts caused no PFTL mutation.
- Two unintended HL deployments consumed gas but caused no PFTL mutation.

## 9. E6 reserve-proof path

Active profile:

- verifier: sp1-groth16
- vkey:
  0x00580ee8c389192568a29dc23d54c22e73a3a45203b22e3d5a934801871e11a7
- policy:
  076c071e44127158ef82350e7feeb64e0be0a06bf8ba4be5f0374ac36b992ac7
- finalized pricing epoch: 5

Durable run directory:

/home/postfiat/repos/a666-eth-fast-lane-combined-20260724/docs/evidence/a666-public-reserve-product-20260803/nav-e6-fresh/20260807T195819Z

The run contains price source records, six leg witnesses, aggregate witness,
reconciliation, execute public values, and a real Groth16 proof. The proof must
never be submitted because its policy hash mismatches.

Required rebuild topology:

- StakeHub source 213618e
- XMR sidecar
- policy-bound legacy HL reader
- Solana self-attested RPC snapshot
- corrected NEAR V6 hashing
- fresh source records
- zero reconciliation residual
- exact vkey and policy gates

Then use the 57ec4168 builder, batch-only validate, and submit/finalize one E6
mark. E5 proof-byte reuse and a retained stale HL receipt are forbidden.

## 10. Ordered resume procedure

### Gate A: custody

1. Require one agent listener owned by the user service.
2. Run stakehub agent unlock through getpass.
3. Require unlocked=true.
4. Restore and verify exact policy metadata.
5. Prove no stale active session exists.
6. STOP on process, whitelist, cap, or journal mismatch.

### Gate B: source

1. Use StakeHub-master-e6@63824784 for the live agent and existing-reader CLI.
2. Use detached 213618e source for E5-policy aggregation.
3. Port the NEAR threshold fix into a clean branch.
4. Add failing current-header regression, make it pass, merge it.
5. Never execute from dirty /home/postfiat/repos/StakeHub.

### Gate C: Hyperliquid

1. Open specialized existing-reader session.
2. Pin chain 999, legacy reader, code size, and code hash.
3. Verify identity before charge and before send.
4. Submit one snapshot under 0.02 HYPE.
5. Produce receipt witness.
6. Close session and reconcile zero USDC.
7. Reject any receipt naming an unintended reader.

### Gate D: E6

1. Assemble E5-compatible six-leg witness.
2. Require exact policy hash.
3. Require reconcile residual 0.
4. Execute guest and compare public values byte-for-byte.
5. Prove with pinned ELF/vkey.
6. Verify locally.
7. Build E6 operations.
8. Batch-only validate on a fresh clone.
9. Submit/finalize once.
10. Verify 6/6 convergence and pricing freshness.

### Gate E: S1f

Build after E6 with:

- proof-derived mint
- principal at or below 10,000,000 pfUSDC atoms
- fresh 96-hex reservation ID
- lowercase recipients
- E6 epoch and packet hash
- fresh gas/cap
- fresh four-hour swap deadline
- fork simulation for revised amount
- complete packet hashes and linter

If final E6 NAV remains 90,365,059, the maximum compliant mint is 11,011,167
atoms. Recompute rather than carrying this value by assumption.

### Gate F: live loop

Resume at leg 2a:

1. order reserve
2. subscribe
3. export debit
4. signer funding if required
5. Ethereum accept-and-mint
6. Permit2 approvals
7. forward swap
8. reverse swap
9. return burn/import
10. redeem
11. bridge-out
12. final conservation, PnL, and evidence

## 11. Never do

- Never repeat the Ethereum deposit, PFTL claim, or stage-0 rollout.
- Never fire S1, S1b, S1c, S1d, or S1e.
- Never submit the discard-only E6 proof.
- Never use either unintended HL reader.
- Never call Solana trustless.
- Never reuse expired sessions, signatures, or deadlines.
- Never use round_ok as sole finality evidence.
- Never create leg 2a before E6 freshness passes.
- Never use the old 11,027,135 mint.
- Never debit the protected 103.000000 wA666 baseline.
- Never weaken policy, replay, amount, finality, or cap gates.
- Never print or commit credential values.
- Never clean/reset the dirty source worktrees.
- Never claim E2E success before final Ethereum USDC credit.

## 12. Recovery and rollback

Stage-0 backup:

/tmp/krimp-exec-fire20260806/leg-1/upgrade/pre-rollout-backup/

It is the only full-fleet rollback point. The claim finalized at height 779, so
ledger rollback across the claim is forbidden. Recovery is forward-only through
new authorized packets.

Ethereum transactions are irreversible after mining. A failed batch-only check
has no chain effect and must never be described as a submitted transaction.

After agentd restart: verify singleton, unlock interactively, restore policy,
reconcile journal/counter, prove no session, and resume only from the first
unexecuted packet.

## 13. Economics and cap

All token values use six-decimal atoms unless stated otherwise.

- prior campaign spend: 501.024845 USDC
- live principal deposit: 10.000000 USDC
- booked principal total: 511.024845 USDC
- campaign cap: 530.000000 USDC
- pre-gas headroom: 18.975155 USDC

Agent restart reset its in-memory daily counter; it did not reset the campaign
ledger. Recompute gas at fire time and include Hyperliquid HYPE gas in final PnL.

Projected E6 arithmetic:

~~~text
fresh NAV              90,365,059 USD_1E8
old mint                11,027,135 atoms
old required principal 10,014,502 atoms  -> exceeds bound

maximum compliant mint 11,011,167 atoms
required principal     10,000,000 atoms
next mint integer      11,011,168 atoms
next principal         10,000,001 atoms  -> exceeds bound
~~~

These are projections until policy-compatible E6 finalizes.

## 14. Credential locations

Locations only:

- /home/postfiat/.stakehub/vault.enc
- /home/postfiat/.stakehub/live-demo-holder-custody/
- /home/postfiat/tmp/navswap-ce22-venue-rebuild-20260719/private/
- /home/postfiat/.postfiat/deployment-bfinal-1a8c0cb6.private.json
- /home/postfiat/.postfiat/recovery-v3-snapshot-publisher.private.json
- /run/user/1000/postfiat-constrained-signer/a666-signer.sock
- /home/postfiat/repos/wan-vultr-all-fleet.txt
- /home/postfiat/repos/vultr.txt

Verify by sanctioned use, never by printing contents.

## 15. Evidence map

| Subject | Path |
|---|---|
| Binding history | docs/evidence/a666-public-reserve-product-20260803/live-demo/native-v1/fire-20260806/ |
| Authorization | fire-20260806/SAURON-AUTHORIZATION-RULING-20260807.md |
| Validation | docs/evidence/a666-public-reserve-product-20260803/live-demo/native-v1/validation-20260807/ |
| Live readiness | docs/evidence/a666-public-reserve-product-20260803/live-demo/native-v1/FIRE-REVIEW-20260807.md |
| E6 capture | /home/postfiat/repos/a666-eth-fast-lane-combined-20260724/docs/evidence/a666-public-reserve-product-20260803/nav-e6-fresh/20260807T195819Z/ |
| Stage 0 / claim | /tmp/krimp-exec-fire20260806/leg-1/ and /tmp/krimp-exec-s1c/leg1/ |
| E6 reports | /tmp/ghash-e6-recon/, /tmp/ghash-e6-policy/, /tmp/ghash-e6-econ/, /tmp/ghash-sol-rc/ |
| Agent migration | /tmp/krimp-e6-hl/restart-window.txt |
| Cold-start handoff | docs/handoffs/A666-LIVE-FUNDS-EXECUTION-HANDOFF-20260807.md |

Temporary evidence can disappear after reboot. Archive redaction-safe reports
before resuming. Never archive secret-bearing request bodies.

## 16. Final acceptance checklist

- [x] Production dependencies reachable from PFTL main.
- [x] Existing-reader support merged to StakeHub master.
- [x] Capability validation published.
- [x] Fleet 6/6 on orchard-aware binary.
- [x] Ethereum deposit mined.
- [x] PFTL claim finalized and conserved.
- [ ] Wallet agent unlocked and policy restored.
- [ ] NEAR V6 fix tested and merged.
- [ ] Fresh policy-bound HL receipt captured.
- [ ] E6 policy hash matches active profile.
- [ ] E6 reconciliation residual is zero.
- [ ] E6 proof verifies.
- [ ] E6 finalizes on PFTL.
- [ ] S1f passes packet, fork, gas, and cap gates.
- [ ] Leg 2a and subscription finalize.
- [ ] Export and accept-mint finalize.
- [ ] Forward/reverse swap receipts have status 1.
- [ ] Return/import and redemption finalize.
- [ ] External bridge-out credits Ethereum USDC.
- [ ] Final conservation and PnL reconcile.
- [ ] Evidence and production changes are committed and pushed.

## 17. Supersession

This runbook and
docs/handoffs/A666-LIVE-FUNDS-EXECUTION-HANDOFF-20260807.md control current
operations.

Historical only:

- docs/handoffs/A666-RECOVERY-AND-LIVE-DEMO-HANDOFF-20260805.md
- docs/handoffs/A666-LIVE-ROUNDTRIP-SUCCESSOR-HANDOFF-20260805.md
- S1 through S1e reviews
- StakeHub-control-plane runner plans

Historical documents remain useful for receipts and failed approaches. They do
not control the next live action.
