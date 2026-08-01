# Wallet and end-to-end swap handoff

**Date:** 2026-08-01

**Repository:** `a666-eth-fast-lane-combined-20260724`

**Branch at handoff:** `feature/pnok-private-fix`
**Purpose:** one authoritative handoff for the browser wallet, Ethereum/pfUSDC
bridge, A666 primary market and MetaMask path, private PFTL swaps, and the pNOK
private-FX demonstration.

## 1. Executive status

The wallet is no longer a static account viewer. It now drives and recovers
the principal product journeys that have been built during the mainnet work:

1. canonical Ethereum-mainnet USDC into proof-native pfUSDC on PFTL;
2. pfUSDC into newly issued A666 at a governed NAV-based primary quote;
3. native A666 from PFTL into proof-bound wA666 in MetaMask;
4. wA666 back from MetaMask, restored as native A666, redeemed for pfUSDC,
   and ultimately withdrawable as Ethereum USDC;
5. transparent and Asset-Orchard-private A666 issue/redeem execution on PFTL;
   and
6. an exact private `pfUSDC <-> pNOK` FIX in the browser.

The critical economic result is proven: a buyer does not have to consume a
small Uniswap pool to acquire a large quantity of a NAVCoin. The buyer's
pfUSDC funds primary issuance, new NAVCoin supply is created at the governed
quote, and the resulting asset can be exported to Ethereum. Redemption retires
the NAVCoin supply and returns pfUSDC under the active policy. This is primary
creation/redemption, not an OTC transfer from a finite operator inventory.

The current release classification is deliberately split:

| Surface | Current determination |
|---|---|
| Ethereum USDC -> pfUSDC | Functionally proven on mainnet; current driver readiness passes |
| Transparent A666 issue/export | Proven end to end with live value |
| Transparent A666 return/redeem | Proven, but the historical qualification required recovery |
| Private A666 issue/redeem | Functionally proven; limited availability, not unattended GA |
| A666 Uniswap venue | Deployed and historically exercised; price and depth require a fresh chain read |
| Private pfUSDC/pNOK FIX | Controlled browser demo accepted; source bridge is not Tier 4 |
| Public unattended users | Not production-qualified |

This document describes what works and what remains. It does not upgrade a
controlled demo into a production claim.

## 2. Product model in plain English

### 2.1 Primary NAVCoin acquisition

Bob starts with USDC. The intended acquisition path is:

```text
Ethereum USDC
  -> governed Ethereum vault deposit
  -> proof-verified pfUSDC on PFTL
  -> governed primary NAVCoin issue at NAV plus issuance spread
  -> native NAVCoin on PFTL
  -> optional proof-bound export
  -> wrapped NAVCoin in Bob's MetaMask wallet
```

The primary issue changes supply. If Bob contributes the pfUSDC required to
buy 100,000 NAVCoin units, the protocol creates those units subject to the
live quote, reserve, freshness, order, and capacity rules. Bob is not buying
100,000 units from the shallow Uniswap pool and is not relying on the operator
to own 100,000 NAVCoin units in advance.

Policy capacities and per-order bounds are safety limits for a policy epoch.
They are not a permanent maximum supply. A successor governed epoch can add
capacity after reserve and NAV inputs are refreshed.

### 2.2 Primary redemption

The reverse path is:

```text
wrapped NAVCoin in MetaMask
  -> return/burn transaction on Ethereum
  -> finalized proof-bound import of native NAVCoin on PFTL
  -> governed primary redemption at NAV minus redemption spread
  -> pfUSDC on PFTL
  -> optional proof-native withdrawal to Ethereum USDC
```

Redemption retires the redeemed NAVCoin supply. A facility needs valid
settlement capacity at execution time, but it does not require a permanent
two-million-pfUSDC operator inventory merely because two million NAVCoin might
eventually be issued. Buyer-funded pfUSDC enters the reserve/accounting path
when issuance occurs.

### 2.3 Secondary liquidity

The wA666/USDC Uniswap pool is the secondary venue and Ethereum integration
surface. It is useful for continuous price discovery and smaller immediate
trades. It is not intended to absorb every large creation or redemption order.
The primary market prevents a thin secondary pool from being the only way to
acquire A666.

## 3. Wallet work completed

### 3.1 Self-custody and account surface

The browser wallet creates or imports the user's PFTL account, stores the
encrypted wallet vault locally, signs with the user's ML-DSA key locally, and
locks/unlocks without sending the spending key to the proxy. The normal wallet
surface shows PFT and issued-asset balances, receiving details, send/process
actions, recent activity, and network/security settings.

The proxy is a transaction coordinator and read adapter. It is not supposed to
receive the user's raw wallet key. Mutation routes are authenticated and the
wallet uses the same-origin browser boundary expected by the proxy. Localhost
demo use no longer asks an end user to paste an internal bridge-service token.

### 3.2 Connectivity and honest status

The wallet/proxy work added TLS WebSocket support, exact-host authenticated
tunnels, live RPC feeds, route discovery, proposer routing, and bounded retry
for transient governed-state reads. The UI distinguishes connecting, ready,
blocked, and settled states instead of presenting a transaction button while
the mutation path is unavailable.

The original `rpc offline`, read-only RPC, and failed WebSocket experience was
a real integration defect. It was corrected at the wallet/proxy/service
boundary and covered by the wallet and proxy suites. Operators must check the
**user** service namespace:

```bash
systemctl --user is-active pft-wallet-proxy-8080.service
systemctl --user is-active postfiat-pftl-pnok-prover.service
```

Checking the system namespace without `--user` incorrectly reports these two
services as inactive.

### 3.3 Ethereum-mainnet pfUSDC bridge

The Bridge screen now uses canonical Ethereum-mainnet USDC and the governed
Ethereum vault. Arbitrum is retired for new deposits and is not presented as
the current path. The browser flow is:

1. connect MetaMask on Ethereum mainnet;
2. enter a USDC amount and approve only that amount;
3. deposit into the active governed vault;
4. wait for Ethereum finality, generate/verify the ingress proof, and relay
   the exact claim to the selected PFTL recipient; and
5. refresh the finalized pfUSDC balance.

The deposit script now accepts and validates the wallet-selected canonical
PFTL recipient instead of relying on one fixed test recipient. The current
bridge driver is pinned to the live validator release and verifies its binary
hash before value-moving work.

Bridge jobs are durable. Navigating away, refreshing, locking the wallet, or
restarting the proxy does not intentionally discard a known deposit. The UI
can recover from the Ethereum transaction hash and shows the active job on the
Process surface. Retry logic was added for temporary readiness, fleet-read,
and job-creation failures after the deposit transaction has already finalized.

The obsolete UX defects addressed here include:

- instructions to bridge USDC to Arbitrum;
- a route-genesis error on the retired Arbitrum path;
- requiring a user to paste a session-only proxy token for local demo use;
- losing bridge status after navigation;
- reporting a finalized deposit as abandoned after a transient relay error;
- artificial wallet-side pfUSDC amount limits; and
- ambiguous progress that did not say whether MetaMask, Ethereum finality,
  proof generation, PFTL claim, or balance refresh was pending.

Consensus policy can still impose explicit transaction/epoch risk bounds.
Removing an artificial browser maximum does not authorize the wallet to evade
governed consensus rules.

### 3.4 Route-driven NAV Markets

The navigation and market surfaces were generalized from a hard-coded A666
button into route-driven NAV Markets. The wallet discovers supported NAVCoins,
asset metadata, current route state, NAV epoch, quote, spreads, validity, and
available actions from governed state. A666 is the deployed live example, not
a permanent assumption in the component architecture.

For A666, the market shows:

- issue versus redeem direction;
- amount and calculated pfUSDC consideration;
- reserve value and issuance/redemption spread;
- quote/NAV freshness and capacity;
- PFTL versus MetaMask delivery/source choices; and
- the durable progress of each leg.

Issuance uses the buyer's pfUSDC and increases A666 supply. Redemption burns
A666 and decreases supply. The UI does not describe issue as an operator OTC
sale.

### 3.5 MetaMask export, return, and redemption

The export path can consume newly issued native A666 and proof-mint wA666
directly to the connected Ethereum recipient. Durable export jobs survive
navigation and process restarts and expose their transaction/proof state.

The inverse UX now has an explicit redemption source:

- **From PFTL** redeems native A666 already held by the PFTL wallet.
- **From MetaMask** first returns wA666 from Ethereum, proves finality,
  restores native A666 on PFTL, and then redeems it.

This fixes the misleading `wallet A666 balance is insufficient` error when the
user actually held wA666 in MetaMask. The return workflow shows the durable
stages: return wA666, confirm the Ethereum burn, prove finality, restore A666,
redeem A666, and verify finalized balances. Wrapped-asset redemption defaults
to the connected MetaMask source where appropriate.

### 3.6 Process recovery

Long-running bridge, export, return, and private-swap work is represented as a
durable job rather than an in-memory spinner. The Process screen and market
surfaces reload job state, reconcile it with finalized chain state, and render
a settled result after refresh. Duplicate submissions are idempotent or
rejected without additional state effect.

The recovery work covers:

- browser navigation and reload;
- wallet lock/unlock;
- proxy restart after job creation;
- worker/prover restart;
- transient RPC and governed-read failures;
- already-finalized Ethereum transactions; and
- a filled FIX market whose completed job must remain visibly settled.

### 3.7 Private execution on PFTL

The resident Asset-Orchard service supports private notes, nullifiers,
commitments, encrypted outputs, spending authorization, and proof-verified
atomic state transitions. Private primary A666 issue/redeem and private
pfUSDC/pNOK FIX execution reuse this path. The resident service keeps the
required circuits warm because a cold prover start is materially slower.

Privacy applies to the PFTL note layer. Ethereum approvals, deposits, wrapped
token actions, withdrawals, public destinations, amounts exposed by selected
boundaries, and timing are not magically hidden by a private PFTL swap.

### 3.8 Private FX screen and pNOK

The wallet now includes a Private FX surface that discovers the pfUSDC/pNOK
pair and exact FIX from finalized live state. The component does not hard-code
pNOK asset IDs. It renders the source trust label, execution privacy, FIX
packet, epoch, expiry, exact input/output, capacity, and final action IDs.

The accepted controlled quote is:

```text
20.000000 pfUSDC -> 210 pNOK
FIX: 10.500000 pNOK per pfUSDC
fee: 0
price impact: 0
execution: private on PFTL
source boundary: controlled sandbox checkpoint
```

The private transition consumes both parties' input notes and creates both
ownership-bound outputs atomically. It does not expose note openings or
spending keys in public evidence.

## 4. End-to-end results

### 4.1 Why Ethereum mainnet replaced Arbitrum

The original Arbitrum design depended on the intended Nitro assertion reaching
its Ethereum-backed confirmation boundary. The observed/planned wait was about
`6.4 days`, which could not meet the required user journey of 25 minutes or
less. Arbitrum remains historical conservation evidence, not current ingress.

The direct Ethereum-mainnet pfUSDC flow completed:

| Campaign | Result | Wall time | User ingress gas |
|---|---|---:|---:|
| `25 USDC` functional round trip | Conservation and replay checks passed | `2h45m48s` | about `$0.03` at the pinned gas/ETH price |
| `1 USDC` latency run | Deposit inclusion through withdrawal inclusion passed | `20m12s` | about `$0.15` at the pinned gas/ETH price |

Approval plus deposit used about 271,000 gas in each run. Dollar cost varied
with the live gas price. These values exclude deployments, off-chain proof
compute, and later withdrawal gas.

### 4.2 Transparent A666 issue and MetaMask delivery

The first fresh buyer-funded mainnet path executed:

```text
100.500000 Ethereum USDC
  -> 100.500000 PFTL pfUSDC
  -> 100.000000 newly issued A666
  -> 100.000000 Ethereum wA666
```

Canonical A666 supply and proof-gated wrapped supply each increased by exactly
`100,000,000` atoms. The buyer's PFTL A666 was consumed by export and the buyer
received wA666 on Ethereum. No Uniswap liquidity was consumed.

### 4.3 Transparent A666 return and redemption

The inverse path—wA666 return, proof-backed native restoration, A666
redemption, pfUSDC, and Ethereum USDC withdrawal—completed functionally. The
first acceptance attempt correctly failed closed on a legacy pfUSDC egress
guest mismatch. A replacement Epoch-5 verifier/vault lane completed the path.
Because that historical run required operator recovery, it is evidence of
functional correctness, not a clean unattended-release qualification.

### 4.4 Full private A666 round trip

The complete path was exercised with private primary issue and redemption in
the PFTL middle:

```text
Ethereum USDC
  -> public pfUSDC boundary
  -> private A666 issue
  -> Ethereum wA666
  -> proof-backed PFTL return
  -> private A666 redemption
  -> private pfUSDC
  -> public Ethereum USDC boundary
```

Private proofs verified, replays were rejected, all six validators converged,
and reserve/supply conservation held. The first complete run required repair.
The later optimized issue took `29m36s`, missing the 25-minute issue target by
`4m36s`; redemption recovery took `14m48s`, inside the target.

### 4.5 Variable size and finalized reserve-aware NAV

A separate campaign proved the requested size variation and repricing flow:

1. transparent issue/export of `1.000000 A666`;
2. private issue/export of `100.000000 A666`;
3. a new six-leg reserve proof and finalized NAV mark produced by the
   historical internal operator path;
4. transparent return/redemption of `1.000000 A666`; and
5. private return/redemption of `100.000000 A666`.

The resulting proof-backed NAV was `$0.90103113`. Both acquisitions created
new supply without using Uniswap. Both redemptions retired that supply. Final
reconciliation passed at PFTL height 440.

This was a business-flow pass and a release fail: the 100-A666 private issue
took 3,948 seconds, the private redemption took 1,776 seconds, and the campaign
needed proof-worker/operator recovery. It does not prove million-unit capacity
or unattended production reliability.

### 4.6 PFTL-resident middle

When pfUSDC already exists on PFTL, transparent/private issue and redemption
do not need to wait for Ethereum. The resident service has exercised the
relevant transparent/private output combinations with certified state
transitions. The remaining latency is PFTL finality, proving, and service
coordination rather than Ethereum deposit finality.

### 4.7 Uniswap and a651/A666 migration

The old a651 pool is deprecated. The current Ethereum integration is A666 v2
and wA666. Repository evidence proves that:

- wA666, its verifier/controller, the ownerless migration, and the Uniswap v4
  pool were deployed on Ethereum mainnet;
- burning legacy a651 released `3,000 wA666`;
- `3,000 wA666` and `3,000 USDC` were added through the official Uniswap v4
  PositionManager;
- third-party swaps changed the pool price; and
- temporary approvals were revoked.

This is historical deployment evidence. Never quote current liquidity, depth,
or price without a fresh finalized Ethereum read.

### 4.8 pNOK private FIX qualification

The controlled pNOK demonstration passed:

- 10 consecutive browser acquisitions;
- 9 inverse private swaps restoring exact starting inventory;
- 19 unique finalized jobs in that repetition campaign;
- an additional fault-injected reset and acquisition;
- proxy restart, validator restart, resident prover restart/prewarm, browser
  reload, lock/unlock, and durable-job recovery;
- replay without effect; and
- 18 of 18 independent acceptance checks.

The source sandbox locks `500 WNOK`; PFTL has `500` pNOK issued and counted
exactly once. Acquisitions took about 148–156 seconds including replay
verification; inverse resets took about 114–138 seconds. A cold two-circuit
prewarm took about 323 seconds on 32 threads, so the demo prover must remain
resident and warm.

At the live read on 2026-08-01, all six PFTL validators agreed at height 776
with one state root and empty mempools. Epoch 5 was active and unpaused after
one inverse reset/acquisition cycle, with 19 of 20 exact fills remaining:

```text
committed: 20.000000 pfUSDC / 210 pNOK
remaining: 380.000000 pfUSDC / 3,990 pNOK
FIX packet: b768016c6ae186e6dd89f519ad119871c33d5037b5ee104973ca29c5e7982a9eacaefc31abb626f3775041ddc561ed5c
```

This is a controlled source checkpoint, not a Tier-4 pNOK bridge. The swap on
PFTL is exact, atomic, and private; the source-chain reserve assertion is the
weaker boundary. Live pNOK-to-WNOK release was not part of the accepted browser
qualification, although contract and integration tests cover the release and
replay rules.

## 5. Current runtime state

As checked on 2026-08-01:

- `pft-wallet-proxy-8080.service` is active in the user service namespace;
- `postfiat-pftl-pnok-prover.service` is active in the user service namespace;
- the wallet proxy listens on loopback port 8080;
- the resident prover listens on loopback port 8787;
- six authenticated validator tunnels listen on ports 28650–28655;
- all six validators agree at height 776, with no pending mempool entries;
- pNOK FIX epoch 5 is active with 19 fills remaining; and
- Ethereum bridge readiness reports `ready: true`, route active, vault
  unpaused, active proof program, healthy authenticated prover, and two
  reachable Ethereum execution RPCs.

The bridge scripts and Monday runbook now pin:

```text
release: /opt/postfiat/releases/pnok-private-fix-2246d25/postfiat-node
node SHA-256: 05330fb20a40b8a4536000ec57da1862d879bcdc4a21bc8c0657f5c56aa8e0f5
topology: /etc/postfiat/releases/pnok-private-fix-2246d25/topology.json
revision gate: 2246d257
```

The previous `resident-local-commit-777faa0` runbook pin was stale and has
been removed from the current operational command sheet.

Runtime state is mutable. Rerun readiness and fleet convergence immediately
before moving value.

## 6. Verification performed

The accepted branch contains the following recorded qualification:

| Area | Result |
|---|---:|
| Browser wallet suite | 230/230 passing |
| Wallet proxy suite | 32/32 passing |
| pNOK Python tests | 8/8 passing |
| Fresh focused Rust `fx_fix` filter | 4/4 passing |
| Norges sandbox Forge suite | 407/407 passing |
| Targeted pNOK Forge tests | 24/24 passing |
| pNOK acceptance audit | 18/18 passing |
| Browser repetition | 10/10 acquisitions passing |

Immediately before this handoff, the three focused bridge tests passed, the
three modified Python scripts compiled, both content-addressed driver hashes
matched their configuration, and live Ethereum bridge readiness returned
ready.

These tests establish the controlled implementation state. They do not replace
an independent security review, high-volume capacity campaign, or unattended
public-user qualification.

## 7. Repository and worktree state

### 7.1 L1/wallet repository

Before the handoff commits, `feature/pnok-private-fix` was a strict linear
descendant of `origin/main`: zero commits behind and 16 commits ahead, with
merge base `6fb0106dc48b00d313bb2d04882abed55e26bea5`. There were no merge conflicts
to resolve. The branch was already pushed through
`a0b2fc7ab159368d6c767f6859731a7ca3915f80`.

The pending tracked runtime edits were reviewed and committed separately as
`7e03078` (`Align wallet bridge with live PFTL fleet`). They contain:

- the canonical PFTL recipient option and validation for pfUSDC deposits;
- the current deployed validator binary/topology pins in both bridge-stage
  scripts;
- the corresponding content-addressed driver and route hashes; and
- the corrected Monday runbook revision and binary hash gate.

The branch also contains 16 pNOK/private-FIX commits implementing consensus
state and RPC support, Asset-Orchard private execution, durable proxy jobs,
browser UX, fault campaigns, evidence curation, and acceptance documentation.
Because the branch is linear on top of main, the correct integration operation
is a fast-forward, not a conflict-producing merge commit.

### 7.2 Generated and private artifacts intentionally not committed

The worktree contains approximately 9,075 untracked files totaling about
10.8 GB. Most are generated execution/proof/fleet evidence under:

- `docs/evidence/`;
- `deployments/a666-mainnet-20260727/`;
- `deployments/pnok-private-fix-20260801/`; and
- `deployments/pnok-controlled-demo-20260801/`.

These trees include large generated state, proof artifacts, backups, and
private/restricted campaign material. They must **not** be bulk-added. Public
evidence needed for review has already been selected, sanitized, and committed.
Future evidence should be curated file by file after custody-material and size
checks.

Two unrelated small untracked development files also remain outside this
handoff:

- `crates/node/examples/diagnose_consensus_qcs.rs`; and
- `docs/plans/PFTERMINAL-EMPTY-MANAGER-TURN-DIAGNOSIS-20260726.md`.

They are preserved as user work and are not part of the wallet/pNOK merge.

### 7.3 Other repositories

The Norges sandbox worktree is clean on `feature/pnok-bridge` at
`7e293b4288279849bfe4810b25eea8d577c53bd7`, based on official upstream
`f1ad067e09fa3e4838be9605bd1fe450831e9244`. It is pushed to the PostFiat fork.
It should not be silently merged into the official Norges Bank upstream; that
requires its own review/PR decision.

The `postfiatorg.github.io` repository has separate uncommitted blog/research
edits and one untracked article. Those changes predate and are independent of
this L1/wallet handoff. They must be reviewed and committed in that repository,
not swept into the protocol merge.

## 8. Merge disposition: completed

The L1/wallet branch passed the final documentation diff, wallet/proxy/build,
Python, focused Rust, live-fleet, and bridge-readiness gates. After a fresh
fetch, `origin/main` was still the exact ancestor: the feature branch was zero
commits behind and 18 commits ahead. The feature branch was pushed and
`origin/main` was fast-forwarded from `6fb0106` through the wallet/pNOK and
handoff commits without a merge conflict. Local `main` was then advanced to
the same remote state.

The generated/private evidence was not added. The tracked L1 worktree was
clean after integration; only the 9,075 intentional untracked files described
above remained. Future work in this tree must continue to avoid `git add .`.

## 9. What remains

### 9.1 Monday A666 demonstration

Use the corrected
[`A666-PFUSDC-MONDAY-DEMO-RUNBOOK-20260803.md`](../runbooks/A666-PFUSDC-MONDAY-DEMO-RUNBOOK-20260803.md).
The Monday audience flow is intentionally narrower than the total system:
pre-stage the Ethereum proof/funding, then demonstrate transparent issue,
fresh reserve-aware NAV/route advancement, and partial redemption. Do not add
private execution, bridge-out, Uniswap trading, another settlement asset, or a
validator upgrade during the live segment.

### 9.2 A666 production hardening

- migrate A666 to the provider-neutral proof ABI, then automate fresh reserve
  proof/NAV generation and policy advancement through the open proof kit;
- eliminate remaining prover-readiness races and manual recovery prefixes;
- repeat transparent and private issue/redeem at the target sizes without
  intervention;
- complete the 100-issue/100-redeem reliability gate;
- reduce private issue p95 below the declared latency gate;
- finish noncustodial multi-user authorization and custody boundaries;
- rehearse the exact browser journey from a clean session; and
- independently review contracts, circuits, consensus rules, bridge drivers,
  and operational key separation.

### 9.3 pNOK progression

- replace the controlled checkpoint with a Tier-4 source proof tied to
  continuous Besu/QBFT finality;
- execute and qualify live pNOK-to-WNOK release end to end;
- separate participant authorization for real independent users;
- run 100+ private swaps with capacity, restart, and latency gates;
- remove demo-only operator assumptions; and
- obtain independent security and Norges-sandbox integration review.

The accepted pNOK demo proves one exact atomic private bilateral FIX. The
broader research goal of an operator-blind or frequent-batch private FX market
is not implemented by this demo.

## 10. Authoritative references

- [`A666-PFUSDC-PRIVATE-SWAP-CURRENT-STATE-20260730.md`](A666-PFUSDC-PRIVATE-SWAP-CURRENT-STATE-20260730.md)
  — detailed A666/pfUSDC history, identifiers, failures, and evidence map.
- [`PNOK-PRIVATE-FIX-DEMO-ACCEPTANCE-20260801.md`](PNOK-PRIVATE-FIX-DEMO-ACCEPTANCE-20260801.md)
  — accepted controlled pNOK result and limitations.
- [`A666-PFUSDC-MONDAY-DEMO-RUNBOOK-20260803.md`](../runbooks/A666-PFUSDC-MONDAY-DEMO-RUNBOOK-20260803.md)
  — exact Monday operator sequence.
- [`A666-PRIVATE-SWAP-PRODUCTION-HARDENING-SPEC-20260730.md`](../plans/A666-PRIVATE-SWAP-PRODUCTION-HARDENING-SPEC-20260730.md)
  — production gates for the private A666 path.
- [`PFTL-PRIVATE-SWAP-INFRASTRUCTURE-AND-LATENCY-DIAGNOSTIC-20260730.md`](../plans/PFTL-PRIVATE-SWAP-INFRASTRUCTURE-AND-LATENCY-DIAGNOSTIC-20260730.md)
  — latency and reliability action list.
- [`PNOK-TIER4-PRIVATE-FIX-SWAP-DEMO-SPEC-20260801.md`](../plans/PNOK-TIER4-PRIVATE-FIX-SWAP-DEMO-SPEC-20260801.md)
  — intended pNOK architecture and the distinction between Tier-4 target and
  controlled first demo.
- [`wallet-web/README.md`](../../wallet-web/README.md)
  — wallet development and local-serving instructions.

Older status documents remain historical evidence. For current wallet/runtime
state and cross-flow product claims, this handoff takes precedence where an
older document conflicts with it.
