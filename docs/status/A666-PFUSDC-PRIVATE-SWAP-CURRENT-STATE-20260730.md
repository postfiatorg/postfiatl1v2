# A666, pfUSDC, Private Swap, Bridge, and Uniswap Current State

**As of:** 2026-07-30 UTC
**Document role:** authoritative program status and documentation index
**Release determination:** functionally proven; limited availability only;
not production GA
**Governing economics:**
`../plans/A666-END-TO-END-MAINNET-PRIMARY-ISSUANCE-SPEC-20260727.md`
**Production release gate:**
`../plans/A666-PRIVATE-SWAP-PRODUCTION-HARDENING-SPEC-20260730.md`
**Latest PFTL qualification evidence:**
`../evidence/pftl-private-swap-p0-20260730/README.md`

This document answers four questions in one place:

1. What product are we building?
2. What has actually been implemented, deployed, and proven?
3. What is live right now?
4. What remains before this can be called production-ready?

When an older status report or handoff conflicts with this document about
what is currently deployed or proven, this document controls. The older
document remains authoritative for the historical run it records. Normative
economics and release requirements continue to be controlled by the governing
specifications linked above.

## 1. Executive summary

The product is a low-slippage primary market for a reserve-backed NAVCoin:

```text
Ethereum USDC
  -> PFTL pfUSDC
  -> newly created A666 at 1.005 x verified NAV
  -> optional private PFTL holding/transfer
  -> Ethereum wA666
  -> optional Uniswap secondary trading
```

The reverse path is:

```text
Ethereum wA666
  -> PFTL A666
  -> A666 retired at 0.9995 x verified NAV
  -> PFTL pfUSDC
  -> Ethereum USDC
```

The central business property is already implemented: a buyer does not need
to buy a large amount of A666 through a shallow Uniswap pool. The buyer's
USDC-backed reserve contribution permits new A666 supply to be created at the
governed issue price. Redemption retires A666 supply and releases the reserve
principal attributable to that supply.

The complete economic loop has functionally passed on Ethereum mainnet and the
six-validator PFTL fleet. Both transparent and private PFTL issue/redeem
transitions work. A666 and its proof-gated Ethereum representation are
deployed. The A666/USDC Uniswap v4 pool was deployed, seeded, and exercised by
third-party swaps.

This is not yet a public production product. The current resident private
service remains restricted to one controlled wallet, one active request,
loopback access, and a maximum request of `1.000000 A666`. The most recent
qualification completed four issue/redeem cycles and then correctly stopped
before publication when governed StakeHub NAV pricing became stale. The issue
latency gate also missed: four-sample p95 was `50.365 seconds` against the
current `42-second` qualification gate.

The correct release statement is therefore:

> The A666 primary issue/redeem, bridge, privacy, and Uniswap architecture is
> deployed and functionally proven. It is available only as a controlled
> limited-availability service while production automation, scale,
> non-custodial wallet boundaries, recovery, and qualification are completed.

## 2. Product and economic model

### 2.1 A666 is primary issuance, not an OTC inventory sale

Suppose A666 has a verified NAV of `$1.00`, existing supply of `100,000`, and
a buyer wants `1,000,000` new A666. The buyer supplies the governed amount of
pfUSDC, the reserve principal and A666 supply increase together, and the buyer
receives newly issued A666. The order does not traverse the Uniswap curve and
does not require an operator to own one million A666 in advance.

At the current issue multiplier:

```text
required pfUSDC = requested A666 x NAV x 1.005
```

At the current redemption multiplier:

```text
returned pfUSDC = retired A666 x NAV x 0.9995
```

The difference between the issue and redeem prices is the governed primary
market spread. All calculations are fixed-point/integer consensus
calculations, not floating-point execution.

### 2.2 There is no permanent maximum A666 supply

A666 v2 has no permanent asset maximum. New valid supply may be created when
all reserve, NAV, freshness, policy, capacity, and authorization rules pass.

The following values are operational risk controls, not a permanent maximum:

| Control | Current value | Meaning |
|---|---:|---|
| Issue capacity | `2,000,000 A666` per policy epoch | Bounded issuance window |
| Redeem capacity | `2,000,000 A666` per policy epoch | Bounded redemption window |
| Maximum primary order | `1,000,000 A666` | Per-order risk bound |
| Ethereum export packet cap | `250,000 A666` | Per-packet bridge bound |
| Net wrapped cap | `2,000,000 A666` | Ethereum route exposure bound |

Larger legitimate flows can be split across packets and policy epochs while
preserving the same economics and conservation rules.

### 2.3 Redemption does not require a prefunded two-million-pfUSDC bucket

The issue and redemption capacities do not mean an operator must inventory
two million pfUSDC before accepting issues. A buyer funds primary issuance.
That contribution becomes counted settlement reserve principal. Redemption
retires supply and releases the corresponding reserve value. The system must
prove exact reserve/supply conservation; it must not substitute an unrelated
operator inventory model.

### 2.4 Uniswap is secondary liquidity

Uniswap provides transferability, price discovery, and immediate secondary
trading for wA666. It is not:

- the A666 NAV oracle;
- the source of primary-market capacity;
- the backing ledger;
- the issuer; or
- the required route for a large acquisition.

A shallow pool can still create large secondary-market slippage. The primary
issue facility exists specifically so a large buyer can acquire newly issued
A666 near verified NAV without consuming that shallow pool.

## 3. Why the architecture changed

### 3.1 The Arbitrum pfUSDC route was rejected

The first proof-native pfUSDC design used Arbitrum One. Under the intended
trustless boundary, PFTL needed a Nitro assertion to pass its Ethereum-backed
confirmation path. The observed/product-planning wait was approximately
`6.4 days`.

That route was technically coherent but commercially incompatible with the
required user journey of `25 minutes or less`. Arbitrum is therefore
deprecated as a new pfUSDC ingress domain. Historical Arbitrum artifacts and
balances remain conservation history, not current product capacity.

### 3.2 Ethereum-mainnet pfUSDC replaced it

The replacement rail uses canonical Ethereum-mainnet USDC:

```text
USDC approval and vault deposit
  -> Ethereum evidence and SP1 ingress proof
  -> PFTL propose/finalize/claim
  -> spendable pfUSDC
```

The first `25 USDC` functional campaign proved the full pfUSDC round trip with
exact conservation and replay rejection, but took `2h45m48s`. The subsequent
`1 USDC` latency run completed deposit inclusion through withdrawal inclusion
in `20m12s`, passing the `25-minute` target with `4m48s` of margin.

Observed user-side gas for the ingress approval and deposit was:

| Mainnet run | Approval gas | Deposit gas | Total ETH | USD at the campaign-pinned ETH price |
|---|---:|---:|---:|---:|
| `25 USDC` functional run | 55,570 | 215,669 | `0.000015740899 ETH` | about `$0.03` |
| `1 USDC` latency run | 55,558 | 215,645 | `0.000081604399 ETH` | about `$0.15` |

Both paths used approximately `271,000` gas. The dollar difference came from
the live gas price. These figures cover ERC-20 approval and vault deposit;
they exclude deployment, off-chain proof compute, and later withdrawal gas.
The ingress gas class is substantially independent of deposit principal.

### 3.3 a651 was replaced by A666 v2

The legacy a651/USDC Uniswap v4 pool had zero pool-specific liquidity in the
last a651 inspection and was not a usable PFTL-to-Uniswap product path. a651
also embodied launch assumptions that do not describe the desired large-
capacity primary market.

A666 v2 replaced it with:

- no permanent maximum supply;
- verified NAV and reserve policy on PFTL;
- buyer-funded primary issue and primary redemption;
- proof-gated PFTL-to-Ethereum wrapping;
- a dedicated wA666/USDC Uniswap v4 venue; and
- an ownerless a651 burn migration used to fund the opening A666 pool.

The old a651 venue is historical. It is not the current A666 product and must
not be presented as live independent backing or primary-market capacity.

## 4. Deployed production identifiers

| Component | Identifier |
|---|---|
| PFTL A666 v2 asset | `521c6c630bb48d4a37ab4a7bd4900dd2caa2d9e99499e452da3c7ce75b3d74b62d20e18555642bec32174498cbee5e2c` |
| PFTL production route | `pftl-a666-ethereum-wA666-usdc-v1` |
| Mainnet wA666 | `0xeE4C92eDB03efdD9B519339edc19ad70C69A9bE5` |
| A666 SP1 receipt verifier | `0xb79FF97EcC11574a8A78d0b5a9D7C8c2A94bF96A` |
| Proof-gated A666 controller | `0x9A0262C0572fb4DB08765408eB225E207F40c3d9` |
| Ownerless a651 migration | `0xFfBBae0eb8450D10F8C0D26C2952b089D56e517c` |
| wA666/USDC Uniswap v4 pool | `0xc5f1e4b5bb07c0718eddcc3d102dc751b8953ec25bb05cdc14d95419d4d16e98` |
| Uniswap v4 PoolManager | `0x000000000004444c5dc75cB358380D2e3dE08A90` |
| Mainnet pfUSDC Epoch-5 vault | `0x8583409ddbac984ec195dfa06a21103d92403c1e` |
| Mainnet pfUSDC Epoch-5 verifier | `0xa77d5af456ef212303e31727b6ca4888cd771e2c` |

The A666 controller is locked to proof-gated behavior. The deployed verifier
pins SP1 program vkey
`0x004e44aca326861252ee5ff7863b1174635b727759b75d46b28bb28d4a7b34f9`.

These addresses are deployment identity, not a current market quote. Operators
must re-read chain ID, bytecode hashes, controller bindings, caps, pool state,
balances, and finalized checkpoints before a new live campaign.

## 5. What has been proven end to end

### 5.1 Transparent buyer-funded primary issue

The first fresh mainnet buyer run executed:

```text
100.500000 Ethereum USDC
  -> 100.500000 PFTL pfUSDC
  -> 100.000000 newly issued A666
  -> 100.000000 Ethereum wA666
```

The A666 canonical supply and proof-gated wrapped supply each increased by
exactly `100,000,000` atoms. This was a primary subscription, not an OTC
transfer from opening inventory. The buyer's PFTL A666 balance was consumed by
the export and the buyer received wA666 on Ethereum.

### 5.2 Transparent redemption

The inverse transparent path was implemented and functionally exercised:

```text
wA666 returned from Ethereum
  -> proof-backed A666 return on PFTL
  -> governed transparent primary redemption
  -> pfUSDC
  -> proof-native mainnet USDC withdrawal
```

The original acceptance campaign exposed an immutable legacy pfUSDC egress
guest mismatch. The route failed closed. A replacement Epoch-5 verifier/vault
lane was deployed and the transparent round trip completed. Because recovery
required operator intervention, the historical run is a functional pass, not
a clean hands-off release pass.

### 5.3 Full private round trip

The private acceptance path completed:

```text
Ethereum USDC
  -> public PFTL pfUSDC boundary
  -> private primary A666 issue
  -> Ethereum wA666
  -> proof-backed PFTL return
  -> private primary A666 redemption
  -> private pfUSDC
  -> public Ethereum USDC boundary
```

The run proved:

- new A666 supply was created against the buyer's reserve contribution;
- the export minted wA666 exactly once;
- returned wA666 became PFTL A666 under proof/checkpoint rules;
- private redemption retired the A666;
- pfUSDC egress released USDC;
- private proofs verified;
- replay attempts were rejected;
- all six validators converged; and
- supply/reserve conservation held.

The first complete private run required operator repair and missed both
latency targets. The later optimized canonical issue completed in
`1,776 seconds` (`29m36s`), missing the `25-minute` issue target by `276
seconds`. Its redemption recovery completed in `888 seconds` (`14m48s`),
inside the `25-minute` redemption target.

### 5.4 PFTL-only resident private service

Once pfUSDC is already spendable on PFTL, the resident service can perform the
governed issue/redeem middle without waiting for Ethereum:

```text
transparent or private pfUSDC
  -> private or transparent A666
  -> private or transparent pfUSDC
```

Real certified transitions have exercised all relevant transparent/private
output combinations. Successful issue increases supply and consumes the exact
quoted pfUSDC. Successful redemption decreases supply and returns the exact
quoted pfUSDC.

The service has durable request state, fail-closed readiness, warm private
proving, authenticated validator peers, remote proposer routing, local apply
before certified send, exact consensus certificate verification, and recovery
after a restart following publication.

### 5.5 Variable-size issue, real StakeHub NAV, and redemption

A separate live-value campaign proved the requested `1x`/`100x` business
flow:

1. transparently issue and export `1.000000 A666`;
2. privately issue and export `100.000000 A666`;
3. build and finalize a new A666 NAV mark from the real StakeHub six-leg
   source path;
4. transparently return and redeem the `1.000000 A666`; and
5. privately return and redeem the `100.000000 A666`.

Both acquisitions created new supply and delivered wA666 without consuming
Uniswap liquidity. The new proof-backed NAV was `$0.90103113`; Uniswap price
and issue spread were excluded from the calculation. Both redemptions retired
the newly created supply. Supply, wrapped supply/claims, reserve, pfUSDC vault
obligations, and six-validator state reconciled at terminal PFTL height `440`.

That campaign was a **business-flow pass** and a **release fail**. The
`100 A666` private issue took `3,948 seconds`; the `100 A666` private
redemption took `1,776 seconds`; it included proof-worker recovery and other
operator intervention. It proves the economic behavior at two different
sizes, not million-A666 production capacity or unattended reliability.

## 6. Privacy and trust boundaries

“Private swap” describes the PFTL middle. It does not make Ethereum account
activity private.

### 6.1 What the private path hides

The Asset-Orchard path uses private notes, nullifiers, commitments, encrypted
outputs, proof verification, and spending authorization. The private-primary
issue/redeem action does not publish the user's note opening or spending key.
Restricted note and key material is kept outside repository evidence.

### 6.2 What remains public

The following boundaries are publicly observable:

- Ethereum USDC approval and vault deposit;
- Ethereum wA666 mint, transfer, return, or Uniswap swap;
- Ethereum USDC withdrawal;
- public PFTL ingress/egress amounts and destinations where the selected
  output is transparent;
- transaction timing; and
- public bridge packets and proof-verification results.

Direct private egress protects the spent note opening but can still reveal the
public destination, asset, amount, and timing. Fixed denominations, batching,
delays, and relayers are separate privacy improvements.

### 6.3 Verification assumptions by direction

| Direction | Current boundary |
|---|---|
| PFTL -> Ethereum A666/wA666 | SP1 proof of finalized PFTL execution under immutable contract/program bindings |
| PFTL -> Ethereum pfUSDC withdrawal | SP1 proof-native Epoch-5 verifier/vault lane |
| Ethereum -> PFTL | Disclosed PFTL BFT checkpoint plus Ethereum receipt inclusion under the registered route |

The Ethereum-to-PFTL boundary is not an Ethereum light client running inside
PFTL. It must be described exactly as deployed; “trustless” must not erase the
checkpoint assumption.

### 6.4 Current custody boundary

The deployed resident service is operator-controlled limited availability.
It uses one controlled wallet and restricted local private state. This is not
non-custodial GA. General availability requires user-held spending authority,
recoverable client wallet state, and a prover/relay interface that cannot
change the user's approved asset, amount, recipient, quote, or route.

## 7. Current live PFTL state

A read-only audit on 2026-07-30 found all six validators identical:

| Field | Value |
|---|---|
| Validator revision | `777faa0e` on all six |
| Finalized height | `528` |
| Finalized tip | `dc4aba8b9a43e8fcae3bd92cf7b16fb622bdcdb5dd356a3b918b454c286d437c1464d8fba9e0b090cad8bb71e4456d4d` |
| State root | `83a836dd56e8ed359fea2ca67a26fa8ef4da7fab78e7da02e526d7416315bcb7761b5851b56b4206a527baf05bb53916` |
| Mempool entries | `0` on all six |
| A666 authorized valid supply | `31,489,197,455` atoms |
| Settlement reserve | `112,995,855` pfUSDC atoms |
| Outstanding bridge claims | `31,489,197,455` A666 atoms |
| Ethereum-route native spendable A666 | `0` |
| Active reservations | `0` |
| Route invariant | `true` |

On validator 2:

- `postfiat-pftl-swapd.service` is active;
- `postfiat-pftl-round-driver.service` is active;
- `postfiat-asset-orchard-local.service` is active;
- readiness is green with zero active swaps;
- the resident asset mirror matches finalized height `528` and its state root;
- the round driver reports five authenticated peers;
- the durable journal is healthy;
- the publication outbox is empty; and
- the live `pftl_swapd` binary SHA-256 is
  `fd4d59ef50be6aa1ed62b83470204fcc4cd7cf82e95f8418754ea45e3a55a31f`.

This is an operational snapshot, not a promise that height, balances, pool
price, or service health will remain unchanged. A new value-moving campaign
must take a fresh preflight snapshot.

## 8. Latest resident-service qualification

Four complete controlled cycles committed exactly once. Each cycle performed
private issue, private redemption, and an ordinary certified egress of the
returned private pfUSDC for the next input.

| Operation | Accepted-to-commit samples | Four-sample p95 | Gate | Result |
|---|---|---:|---:|---|
| Private issue | `46.118`, `47.956`, `49.508`, `50.365` s | `50.365 s` | `42 s` | Fail |
| Private redeem | `32.650`, `37.235`, `34.913`, `38.862` s | `38.862 s` | `45 s` | Pass |

Additional measurements:

- issue proof-DAG maximum: `12.496 seconds`;
- redeem proof-DAG maximum: `12.526 seconds`; and
- finality observer: `292.794-407.842 milliseconds`.

Cycle 5 stopped before publication with terminal journal status
`FAILED_PREPUBLISH`. Its pending output is durably `discarded`; no publication
artifact exists and the outbox is empty. Offline simulation identified:

```text
stale_pftl_uniswap_pricing
PFTL-Uniswap finalized NAV pricing is older than the consensus freshness window
```

This was a correct safety result: stale governed pricing could not create a
valid swap. It also exposed missing production automation. The runner must
refresh StakeHub reserve/NAV inputs, wait for finalized six-validator
convergence, and reacquire a quote before the freshness boundary.

The ten-cycle campaign was stopped after four complete cycles. The decision is
`NO-GO` for the 100-issue/100-redeem production gate because:

1. only four complete cycles ran;
2. governed NAV refresh is not automated;
3. prover-mirror readiness races required explained retries; and
4. private issue p95 exceeded the gate.

## 9. Ethereum and Uniswap state: proven versus current

The following is the last repository-evidenced deployment state, not a
real-time market quote:

- the wA666 token, verifier, controller, migration contract, and pool were
  deployed on Ethereum mainnet;
- the opening PFTL inventory was source-debited and proof-minted exactly once;
- burning legacy a651 through the ownerless migration released `3,000 wA666`;
- `3,000 wA666` and `3,000 USDC` were added through the official Uniswap v4
  PositionManager;
- the position recorded `3,000,000,000` liquidity units;
- third-party swaps finalized and moved the pool price; and
- temporary token/Permit2 approvals were revoked.

Before stating a current price, depth, or executable quote, re-read the
finalized Ethereum pool state. The existence of the pool and historical
liquidity is proven; current liquidity and price are time-varying market
facts.

wA666 is a standard Ethereum asset that can be added to an Ethereum wallet
such as MetaMask by contract address. Wallet visibility does not itself prove
liquidity, NAV, backing, or an executable quote.

## 10. Failure and repair history

The program did not progress as one clean release. Important failures and the
resulting hardening work are part of the current state:

| Failure or limitation | Resulting action/current disposition |
|---|---|
| Arbitrum trustless confirmation took about 6.4 days | Replaced with direct Ethereum-mainnet pfUSDC; Arbitrum deprecated |
| First pfUSDC functional round trip took 2h45m48s | Optimized replacement run passed in 20m12s |
| First A666 transparent issue took 42.2 minutes | Subsequent chain/prover work reduced the canonical private issue to 29m36s, still above target |
| Legacy pfUSDC egress guest did not match immutable deployment | Failed closed; Epoch-5 verifier/vault lane deployed |
| Destination-consume transition was missing from the early workflow | Implemented proof-backed consume accounting and replay coverage |
| Validator software required rolling upgrades during acceptance | Added signed manifests, checkpoint backups, one-node-at-a-time apply order, convergence checks, and readiness gates |
| Cold private proving-key setup dominated one-shot workflows | Deployed long-lived resident prover/service and high-core proving path |
| Resident finality observer loaded unnecessary historical QC data | Bound verification to the committed proposal view while retaining exact certificate checks |
| Prover mount identity mapping could silently mismatch | Hardened identity-map preservation and fail-closed readiness |
| Campaign runner reached stale governed NAV in cycle 5 | Consensus rejected before publication; production NAV refresh worker remains required |
| Resident issue latency remains above gate | Continue proof/concurrency pipeline work without weakening consensus verification |

Failures that rejected before publication did not authorize manual ledger
edits. Recovery used certified chain actions, exact-once journals, or a new
verified deployment lane.

## 11. What remains

The binding checklist is in the production-hardening specification. At a high
level, remaining work is:

### P0: reconcile and automate

- finish exact reconciliation of the cycle-5 egressed pfUSDC against the
  controlled balance and global conservation report;
- freeze and independently restore a signed content-addressed checkpoint;
- automate authenticated StakeHub reserve/NAV refresh;
- make the resident orchestrator fully exact-once across every crash prefix;
- remove prover-mirror readiness races;
- complete safe rollback and redacted baseline manifests; and
- rerun the qualification campaign from a fresh governed NAV.

### P0: meet latency and reliability gates

- reduce resident private issue p95 below its release threshold;
- complete at least `100` consecutive issue and `100` consecutive redeem
  operations with zero unexplained intervention;
- prove invariant, replay, stale-quote, stale-NAV, duplicate-request,
  restart-after-publication, and corrupt-journal behavior;
- run amount and concurrency ladders under explicit risk limits; and
- measure full Ethereum-to-PFTL and PFTL-to-Ethereum latency separately from
  the PFTL-only service.

### P1: expand the supported product

- move from one controlled wallet to authenticated multi-wallet service
  boundaries;
- keep private spending authority outside the orchestration/prover service;
- implement recoverable client-side private wallet state and rescan;
- expose quote expiry, NAV epoch, proof state, bridge state, and failure reason
  in product surfaces;
- reproduce the already functional `1x`/`100x` variable-size campaign under
  the hardened, intervention-free service and expand it into a bounded amount
  ladder;
- re-read and monitor live Ethereum contract/pool state; and
- qualify bounded large orders and packet splitting without claiming
  unsupported million-dollar capacity.

### Production gates

The hardening specification defines:

- `P1`: controlled operator limited availability;
- `P2`: authenticated multi-wallet general availability; and
- `P3`: non-custodial general availability.

No historical canary or functional demonstration implicitly passes a later
gate.

## 12. What may and may not be claimed

### Supported statements

- A666 v2 has no permanent maximum supply.
- Buyer-funded primary issue and primary redemption are implemented.
- A large acquisition is designed to avoid Uniswap slippage by creating new
  supply against verified reserve value.
- Transparent issue/redeem works.
- Private PFTL issue/redeem works.
- The complete mainnet loop has functionally passed.
- wA666 and the A666/USDC Uniswap v4 pool are deployed.
- The resident PFTL service is materially faster than the original one-shot
  end-to-end workflow.
- Safety checks rejected stale governed pricing before publication.

### Unsupported statements

- “The product is production GA.”
- “The private path makes Ethereum activity private.”
- “Any public user can use the resident service.”
- “The system is non-custodial.”
- “Million-dollar orders are presently qualified.”
- “The pool currently has 3,000/3,000 liquidity” without a fresh chain read.
- “All directions use the same trustless light-client boundary.”
- “The 25-minute issue SLO is satisfied.”
- “The 100/100 production campaign passed.”

## 13. Documentation authority and supersession map

| Document | Role after this update |
|---|---|
| `A666-PFUSDC-PRIVATE-SWAP-CURRENT-STATE-20260730.md` | Current program truth and index |
| `../plans/A666-END-TO-END-MAINNET-PRIMARY-ISSUANCE-SPEC-20260727.md` | Binding economics and primary-market behavior |
| `../plans/A666-PRIVATE-SWAP-PRODUCTION-HARDENING-SPEC-20260730.md` | Binding remaining work and release gates |
| `A666-MAINNET-DEPLOYMENT-20260727.md` | Historical deployment and transaction evidence |
| `A666-PRIVATE-ROUNDTRIP-HANDOFF-20260728.md` | Historical first full private round-trip handoff |
| `A666-CHAIN-OPTIMIZATION-RUN-REPORT-20260729.md` | Historical full-chain optimization result |
| `PFTL-RESIDENT-SWAP-LIMITED-AVAILABILITY-20260729.md` | Resident-service implementation history through limited availability |
| `../plans/PFTL-RESIDENT-PRIVATE-SWAP-SERVICE-SPEC-20260729.md` | Resident-service design and qualification history |
| `../plans/PFTL-PRIVATE-SWAP-INFRASTRUCTURE-AND-LATENCY-DIAGNOSTIC-20260730.md` | Diagnostic and action checklist |
| `../plans/PFUSDC-MAINNET-CAMPAIGN-HANDOFF-20260726.md` | pfUSDC/Arbitrum/Ethereum campaign history |
| `../plans/PFUSDC-TIER4-IMPLEMENTATION-PLAN-20260717.md` | Historical Arbitrum design; not the current route |
| `pfusdc-bridge-handoff-2026-06-19.md` | Historical generic bridge handoff |
| `../navcoins/uniswap-pool.md` | Historical a651 venue details |
| `../plans/pftl-uniswap-bridge-redeployment-spec.md` | Historical pre-A666 bridge/pool design |

## 14. Evidence map

| Claim | Primary repository evidence |
|---|---|
| Ethereum pfUSDC functional and latency runs | `../evidence/pfusdc-eth-campaign-20260725/` and `../evidence/pfusdc-eth-mainnet-latency-20260727-run2/` |
| A666 deployment and opening export | `A666-MAINNET-DEPLOYMENT-20260727.md` and `../../deployments/a666-mainnet-20260727/` |
| Transparent/private acceptance | `../evidence/a666-acceptance-20260728/` |
| Variable-size issue, real StakeHub NAV mark, and redemption | `../evidence/a666-variable-size-nav-roundtrip-20260728/README.md` |
| Optimized full private round trip | `../evidence/a666-optimization-run-20260729/` and `A666-CHAIN-OPTIMIZATION-RUN-REPORT-20260729.md` |
| Resident private service | `PFTL-RESIDENT-SWAP-LIMITED-AVAILABILITY-20260729.md` |
| Latest view-aware observer and partial qualification | `../evidence/pftl-private-swap-p0-20260730/README.md` |
| Remaining production work | `../plans/A666-PRIVATE-SWAP-PRODUCTION-HARDENING-SPEC-20260730.md` |

Evidence directories may contain restricted operational artifacts that are
intentionally untracked. Public documentation must not copy private note
openings, spend keys, nullifiers linked to a controlled identity, unrestricted
host paths, or signing material into the repository.
