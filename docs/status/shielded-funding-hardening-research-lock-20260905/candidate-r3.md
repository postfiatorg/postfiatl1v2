# Private Hyperliquid Funding from Self-Custody: Hardening Spec

Date: 2026-09-05, revision 3 (operator-funded cover)

Status: revised research draft; not locked, implemented, or deployment-approved.

Objective: fund a fresh Hyperliquid account from an existing self-custody wallet,
without a centralized exchange, while breaking the deterministic public funding
link and measuring remaining amount, timing, and ownership inference.

For this Hyperliquid path, this revision supersedes conflicting privacy gates,
sponsorship, and Ethereum-to-Arbitrum assumptions in the
[MVP design](shielded-perp-venue-funding-mvp-spec.md). The
[implementation journal](shielded-perp-venue-funding-implementation-20260904.md)
records existing work, not completion of this revision. Lighter and venue return
flows are outside this revision's implementation scope. Research locking and
subsequent implementation milestones follow `AGENTS.md`.

## 0. Product contract

```text
Known self-custody wallet T on Ethereum
  -> shared WETH vault -> public pfETH recipient T'
  -> fixed-denomination encrypted Asset-Orchard note
       [private ownership; shared pool; overlapping holding periods]
  -> private egress to fresh PFTL account P_i
  -> pfETH burn -> proof-bound WETH release to fresh Ethereum EOA E_i
  -> unwrap -> canonical ETH bridge to E_i on Arbitrum
  -> sponsored ETH/USDC swap and Bridge2 deposit from E_i
  -> fresh Hyperliquid account E_i
```

The entrance `T -> vault -> T' -> note commitment` is public. The exit
`P_i -> E_i -> Hyperliquid` is public. Privacy is the difficulty of matching
the entrance to that exit. Hiding the initial depositor is not required. A
fresh ingress EOA funded by T adds no privacy boundary; no CEX or custodial
mixer is part of the route.

The user retains note and account spend keys. Existing PFTL, proof, vault,
Ethereum, Arbitrum, token, and venue trust assumptions remain. Trading and later
transfers remain observable under a known Hyperliquid address. This ETH-first
route introduces ETH price exposure while waiting; a stablecoin pool would be
a separate design.

The initial claim concerns a public-chain observer who knows the service uses
operator-funded cover but lacks the operator's private note-to-exit mappings.
Funded cover accounts are an explicit bootstrap mechanism, not independent users.
The operator can identify its own exits and may identify a user's exit by
elimination. Protection against that operator is a separate, stronger result.
Both results are displayed; only the selected claim governs the wallet gate.
Compromised wallets/provers, global network observation, and arbitrary collusion
are not covered by the public-observer claim.

## 1. Existing implementation and required changes

Paths under `stakehub/` are relative to the sibling `StakeHub` repository.
These findings come from source inspection, not new production attestations.

| Surface | Existing behavior | Required change/use |
| --- | --- | --- |
| `stakehub/shielded_exit_executor.py::vault_ingress_batch` | Wrap, approve, `depositV2`; correct pfETH/WETH units | Reuse calls for public ingress; no duplicate ingress builder |
| `crates/node/src/orchard_policy_actions.rs` | Ingress-v2 and private-egress construction | Keep note openings, witnesses, and private proving local |
| `crates/node/src/orchard_state_application.rs` | Private-egress nullifier check and full asset credit | Reuse; no new consensus anonymity gate or reserve deduction |
| `crates/types/src/shielded_bridge_governance.rs` | Commitments, nullifiers, encrypted outputs, aggregate balances | Does not expose per-source unspent-note inventory |
| `stakehub/shielded_exit_planner.py` | Integer ETH bands, jitter, encrypted mapping, historical count gate | Single launch band; common windows; source-aware gate |
| `stakehub/shielded_exit_observer.py` | Vault events and equal-size ingress/release matching | Add PFTL egress/anchors, partial histories, source groups, auxiliary knowledge |
| `stakehub/shielded_exit_pftl.py` | Shared sponsor activates fresh PFTL accounts | Reuse with uniform requests isolated from ingress identity |
| `stakehub/shielded_exit_jobs.py`, `shielded_exit_relayer.py` | Immutable jobs, hashes, restart state, preparation-time gate | Replace stale counts; ordinary signed Ethereum transactions; minimize exports |
| Proposed `stakehub/shielded_exit_cover.py` | Not implemented | Bounded operator-funded cover scheduler using existing note/job/state primitives; separate operator wallet and accounting |
| `crates/ethereum-contracts/src/ExitExecutorV1.sol` | EOA-authorized 7702 batches | Use on Arbitrum after arrival, not for default Ethereum `depositEth` |

The existing Hyperliquid Arbitrum probe begins with an Arbitrum-funded burner.
It does not establish delivery through the preceding Ethereum bridge. The former
draft's Lighter narrative is not evidence of this complete Hyperliquid path.
Remove unsupported same-day implementation promises and measured-effort claims.

## 2. Denomination and scheduling policy

### 2.1 Fixed-size first version

Start with one active denomination: **1 ETH per note and fresh Hyperliquid
account**, using the current integer-band planner. This avoids fragmenting a
small pool. Smaller bands require integer-atom support throughout planner,
observer, jobs, and CLI, plus their own observed cohort. Never use floats.

`D_atoms = 1_000_000_000` pfETH atoms;
`D_wei = D_atoms * 1_000_000_000`.

```text
vault deposit = shielded note = private-egress credit = bridge burn
              = vault WETH release = D
```

Native fees are funded separately; use the supported zero asset-fee egress.
Leave non-denomination remainder in the source wallet. No arbitrary whole-note
amount, custom USD target, public change refund, or consolidation of several
notes into one venue account is offered in the initial flow.

Use ingress-v2 with authenticated encrypted outputs. Verify deployed proof and
ingress versions before accepting funds. Verify Ethereum deposit, PFTL mint,
and shielding receipts individually; an Ethereum deposit alone is not a
shielded balance.

Multiple notes or addresses known to belong to T remain one source group.
Several venue accounts require collection-level analysis: note counts, timing,
and later consolidation can reveal common ownership. Initial automation allows
one pending user request per local source wallet. The cover scheduler may manage
many explicitly operator-owned notes within its separate budget. This reduces
accidental user patterns; it does not provide Sybil resistance.

### 2.2 Hold and release

Before entry show band activity, sponsor capacity, ETH exposure, waiting
conditions, and recovery. Historical activity is an admission hint, not a
promise of future privacy.

Initial policy: at least six hours after finalized shielding, then consider
common hourly PFTL submission windows. Select among eligible windows with local
cryptographic randomness independent of deposit order. Build a retained-anchor
proof and evaluate the prospective exit locally. There is no maximum note age
or automatic withdrawal deadline. After 48 hours, prompt for continued waiting
or explicit recovery; elapsed time never authorizes weaker privacy.

Prefer a common recent retained anchor per window. Only commitments covered by
that exact anchor are candidates. Do not unnecessarily identify users through
a note-specific creation root. A changed anchor invalidates the prior analysis.

Run the gate **before exporting the private-egress proof or any request that
reveals its timing/anchor**. Gating only Ethereum release is too late: PFTL egress
has already exposed the destination and amount. Include visible sponsor
activation, quote/proof requests, and failed attempts in the observer model.
Waiting after release adds no cryptographic separation to the public tail.

Windows are scheduling conventions, not atomic multi-user settlement. Analyze
partial participation, reordered/early broadcasts, and withdrawals that bypass
the wallet. If passing depends on atomic cohort execution, this version cannot
pass. Eight simultaneous requests alone are not a privacy guarantee.

## 3. Observer and wallet gate

### 3.1 Measure plausible source mappings

Delete the proposed on-chain `k_min` gate. Nullifiers do not publicly identify
the commitments they spend; the frontier cannot identify which ingress owners
still hold unspent notes. Distinct addresses, past exits, and pool balance do
not establish an individual withdrawal's anonymity.

Extend the existing observer to index finalized vault events, PFTL mint/shield
commitments and positions, private-egress anchors/nullifiers, public egresses,
burns, releases, sponsor transactions, bridge messages, and venue credits.
Record chain/block hashes, coverage, route/code versions, policy, and model.
Include public failed attempts and disclosures where observable.

Analyze the earliest public exit, then carry its candidates through the linked
tail. Enumerate feasible assignments consistent with asset, denomination,
anchor membership, chronology, public spends, and the stated behavior model.
Different nullifiers cannot consume the same note in an assignment. Allow
unspent deposits: use partial matchings, not compulsory complete bijections.
Never mark a particular deposit spent merely because a same-size exit occurred.

Retain history from a verified complete starting point. A 30-day activity view
must not delete older candidates or known disclosures. Missing history,
unmodeled private swaps/issuance, and unexplained balance transitions produce
`inconclusive`; do not ignore them. This wallet avoids split/merge, but must
conservatively handle other pool activity. Minimum holding time is a wallet
behavior assumption, not a consensus proof of every candidate's age.

Group candidates by known public source ownership, including linked Ethereum
and PFTL accounts. Unknown ownership stays unknown. Do not describe address
counts as verified independent people.

Measure the user's actual question: given known source T, which destination
account did T fund? For each publicly distinguishable destination cluster E,
compute `q(T,E) = Pr[T funds E | observer evidence, T completes a funding exit
in the evaluated cohort]` under each named model. Use the whole known source
group, not just one selected note. Group destinations with public common-owner
links; splitting one visible cluster into many addresses cannot improve the score.
For the initial one-note request, report candidate destination count and
`q_max = max_E q(T,E)`. Conditioning on a completed funding exit prevents
inflating privacy by assuming that T probably never withdrew. Do not condition
on the private actual destination, which the observer does not know.

Also report reverse source probabilities for each exit and operator ownership
concentration. These are diagnostics, not a requirement for eight independent
sources. With one user note and 99 foundation notes, an ideal symmetric public
model could give `q(T,E) = 1/100` while assigning 99/100 probability to foundation
ownership at every exit. That can hide T's destination even though there are
only two source groups. If all notes belong to T, destination ownership remains
T regardless of which particular note was spent, so this gate must fail.
Multiple user-note requests need a joint ownership model before being supported.

Evaluate public-chain evidence, wallet-known source grouping, operator-known
cover assignments, and sponsor/relay collusion as separate views. The public
view must know the cover-generation policy and public funding clusters; it must
not pretend that the cover wallets are independent people. Add published/leaked
cover mappings to that view as they become available. In the operator view,
condition on its complete known assignments; do not mistakenly pass it using
an unspent-note assumption or undisclosed operator-controlled exits.
Uniform assignment weights are not a worst-case bound: include timing, volume,
destination behavior, cover-policy fingerprints, and ownership sensitivity.

The distinction is a design inference to test, not an experimentally established
privacy result for this service. Research finds that mixer anonymity-set counts
can overstate protection and that additional transaction volume need not improve
it: [Wang et al.](https://arxiv.org/abs/2201.09035). No view promises protection
against an adversary controlling or learning every alternative assignment.

Version the model before implementation: specify assignment weights, timing
likelihoods, ownership clustering rules, and the auxiliary records available to
each view. Do not tune them on a particular user's desired passing result.
Validate inference against exhaustive small synthetic histories with known
mappings, then independent held-out histories including adversarial timing.
Any production approximation needs an error bound; otherwise return
`inconclusive` when exact analysis is too large. The probability threshold is
conditional on these models, not a cryptographic success probability.

### 3.2 Decision, freshness, and scope

Initial `k_target = 8` is a product policy. A selected view passes only with
at least eight plausible destination clusters, `q_max <= 1/8` under all required
models for that view, complete relevant coverage, and no deterministic T-to-exit
link. A raw eight-address count is never sufficient.

| Claim / UI label | Gate and limitation |
| --- | --- |
| `Public-observer funding privacy — operator cover` | Initial service mode. Public models, including cover-aware pattern attacks and local source grouping, pass. The operator may infer the user's destination. A failed operator-aware result does not block this explicitly selected mode. |
| `Operator-aware funding privacy — named operators` | Additional result only if the same target-destination bound survives conditioning on the named operators' cover mappings and specified colluding relay/sponsor records. It does not imply resistance to arbitrary collusion. |

Show both results before entry and before egress, with model, evidence time,
and scope. Require the user's selected mode in local policy and payload-bound
authorization. Never fall back from operator-aware to public-observer mode
without an explicit choice. Missing auxiliary records make the operator-aware
result `inconclusive`; they do not automatically invalidate complete public
evidence. Failed selected-view bounds are `insufficient funding privacy`;
missing selected-view coverage or solver truncation is `inconclusive`. Neither
permits automatic submission under that selected claim.

Public data supplies candidate history. Private inputs are local ownership and
authorized auxiliary disclosures. Compare the selected note to a hypothetical
egress locally; never insert its actual source mapping in a relay-visible report.

Bind the decision to exact payload, nullifier, anchor, destination, denomination,
selected claim, snapshot, cover-policy version, and model hashes. Revalidate
before first submission, retry, or changed window. Require current finalized
tips and snapshot age no more than
one window. A stale `passed_at_job_preparation` marker is insufficient.
Export only the approved exit packet and minimal payload-bound, expiring
authorization, not the ownership analysis.

The relay can enforce service authorization but cannot independently verify
private ownership inputs or govern all permissionless withdrawals. This is a
wallet gate, not an on-chain anonymity guarantee. Continue analysis after exit:
later activity/disclosure can weaken an earlier estimate. Preserve the original
evidence and append updates rather than implying permanent unlinkability.

### 3.3 Operator-funded cover is an implementation requirement

Implement the cover scheduler as a separate operator application using the same
ingress-v2, proof, egress, sponsorship, and settlement code as user requests.
It creates real D-sized notes from operator-owned capital and controls only its
own note/account keys. Empty wallets and unfunded requests contribute nothing.
The operator funding cluster may be publicly known; the necessary uncertainty
is which exits it owns, not whether its ingress addresses look independent.
The user's source must not fund these alternative owners or reimburse cover
in a transaction that links the user to the exit.

Use this bounded lifecycle:

1. Configure an isolated cover wallet, locked policy version, capital budget,
   native gas/proof/swap budget, daily spending ceiling, minimum note inventory,
   and venue holding horizon. Track capital separately from consumed fees.
2. Ingress standard notes on a precommitted statistical schedule independent
   of individual user arrivals. Randomness and actual private mappings remain
   private. Publish the policy/distribution, not a future list of cover exits.
3. Maintain overlapping note ages and public exit windows. Use the same proofs,
   anchors, sponsor policies, packet sizes where feasible, and quote/settlement
   path as user exits. Cover-only relayer keys, timing rules, or job identifiers
   are distinguishing features and must be tested as such.
4. For the bootstrap cohort, carry cover through actual Hyperliquid funding of
   fresh operator-owned accounts. A cover path that always returns straight to
   the foundation while user paths alone reach Hyperliquid is not sufficient.
   Trading is not required; no self-trading, fabricated trades, or claims of
   organic customers/volume are part of the cover design.
5. Preserve a common holding policy and analyze account behavior. Idle cover
   accounts cannot be assumed to hide an active user account once behavior
   distinguishes them. Report the funding-time result and later degradation;
   do not promise indistinguishability of future trading.
6. Stop new admissions if usable cover, fees, or holding capacity fall below
   policy. Resume existing user settlement/recovery normally. A scheduler
   outage must not trigger a mass sweep that silently unmasks earlier users.

Size by effective unresolved alternatives, not cumulative recycled volume.
As a capital illustration only, seven 1-ETH cover notes plus one user note
can support eight destinations in an ideal symmetric one-shot model. Seven
cover ETH does not guarantee a pass. Ninety-nine cover notes require 99 ETH
of principal before gas, proving, swap costs, and capital held at the venue.
One repeatedly recycled ETH is not 99 simultaneous independent alternatives;
the observer must account for every visible exit and re-entry.

Initial cover funds remain in their exit compartments for the configured pilot
horizon. There is no automatic recycling/treasury sweep in this release. Public
retirement or disclosure can shrink past candidate sets, so the observer must
test that continuation and update affected reports. Unbounded steady-state
operation requires a separately specified, tested return/replenishment policy;
do not assume instantaneous or privacy-free capital reuse. Venue return flows
remain outside this implementation except for modeling their public consequences.

Keep operator mappings encrypted and separate from user wallets, job exports,
and public diagnostics. An auditor may use them in an isolated operator-aware
test; the public-view test gets only what a public observer can learn. Log
disclosures as additional observer evidence. The service must disclose that it
generates cover, its capital/operating costs, and its ability to infer user exits.
Protection depends on those mappings not becoming public, not on calling the
operator's accounts unrelated users or assuming it forgets its own transactions.

Evaluate the prospective user exit only against actual public history plus
that exact prospective packet. Unbroadcast cover promises cannot count as
executed alternatives. After settlement rerun against actual receipts and later
behavior. Admit no public-privacy claim until cover-aware analysis passes on
real funded history; generating accounts alone is only a functional test.

## 4. Shared sponsorship and asset accounting

Keep the existing shared PFTL activation service. Its `sponsor -> P_i` edge
reveals service membership, already apparent from the exit, and need not
identify T. Removing every sponsor edge is not necessary for this objective.

The sponsor receives only an exit address and common funding request through
an isolated connection. No ingress transaction, reusable login/customer token,
source signature, or user-specific sponsor account. Use a common epoch funding
amount covering account reserve and burn fees. Verify current native-PFT fees:
the helper defaults to 1,000 atoms and rejects less than 100; the former draft's
20-atom assumption is not a sufficient budget. Verify the exact activation receipt.

An Ethereum sponsor provides a common, bounded native-ETH allowance to E_i for
the two direct transactions in §5. Use epoch-wide gas/top-up policies and common
broadcasters. T, T', a user-specific relayer, and ingress-linked reimbursements
must not fund an exit. Rotation schedules must not identify users.

Pilot sponsorship is an explicit subsidy. Budget and reserve capacity before
admission; stop new admissions on depletion. Rate limits may affect availability;
do not solve abuse with an identity-bearing ingress-to-exit credential.
Production fee collection needs a separate privacy design. Free unlimited
service and hidden principal deductions are not assumptions.

Every note retains D pfETH. Sponsors debit native PFT or ETH; account for fees
and dust in their own units. No cross-asset `reserve_atoms` subtraction, new
private-egress field, or consensus reserve pool is needed. Dust stays in its
exit compartment; automatic sweeps to T or across burners would reveal links.

## 5. Hyperliquid settlement that reaches the intended account

### 5.1 Ethereum bridge correction

Do not use existing `hyperliquid_l1_batch` for this path: it invokes
`depositEth()` from a sponsored 7702 account while assuming ordinary E_i gets
the Arbitrum credit. Arbitrum documents aliasing for delegated callers, and
upstream Inbox aliases callers with code or callers different from `tx.origin`.
See [Arbitrum messaging](https://docs.arbitrum.io/how-arbitrum-works/deep-dives/l1-to-l2-messaging)
and [Inbox source](https://github.com/OffchainLabs/nitro-contracts/blob/main/src/bridge/Inbox.sol).

Use this default:

1. Derive E_i locally with the existing domain-separated derivation and reuse
   registry. Verify no previous activity on Ethereum, Arbitrum, or Hyperliquid;
   require empty Ethereum code and no previous delegation.
2. Verify the PFTL burn and proof-bound release of D WETH to E_i. A permissionless
   release caller cannot change the proved recipient or amount.
3. Receive the common ETH gas allowance. E_i signs an ordinary Ethereum
   transaction to `WETH.withdraw(D_wei)`; a broadcaster submits signed bytes.
4. After success, E_i signs an ordinary transaction directly to the pinned
   canonical Inbox, `depositEth()`, with `value = D_wei`. Require empty code
   and E_i as direct transaction origin. Gas is funded separately.
5. Follow the exact L1 message to L2 execution and D ETH delivery to ordinary
   E_i, not its alias. A timer or unrelated balance increase is insufficient.

The EOA pays Ethereum gas from the shared allowance; the broadcaster does not
need its key. Two public transactions are acceptable on this already public
tail. An explicit-recipient retryable is deferred, not an unspecified fallback.
Pin and simulate the deployed Inbox implementation at an identified block;
upstream source alone is not deployment verification.

### 5.2 Arbitrum swap and venue deposit

After confirmed delivery, reuse `hyperliquid_arbitrum_batch`. E_i signs a
chain-specific 7702 authorization and EIP-712 exact-output swap/deposit batch;
a common Arbitrum relayer pays gas. Bind chain, EOA, reviewed implementation,
nonce, deadline, router, token, Bridge2, and amounts. Delegation persists and
remains controlled by the EOA key; a failed call may still leave delegation
applied. Reconcile both account and executor nonces. See
[EIP-7702](https://eips.ethereum.org/EIPS/eip-7702).

Choose USD output using a common denomination/window quote policy, not an
identifying personal target. Sign output, maximum ETH input, and deadline;
the maximum must fit D and a bounded slippage policy. If stale or unfillable,
retain ETH and obtain a new bounded signature. No silent slippage increase,
destination change, or spending beyond authorized balances.

The current builder uses native Arbitrum USDC
`0xaf88d065e77c8cC2239327C5EDb3A432268e5831`, Bridge2
`0x2Df1c51E09aECF9cacB7bc98cB1742757f163dF7`, and a 5-USDC minimum.
The [official Bridge2 documentation](https://hyperliquid.gitbook.io/hyperliquid-docs/for-developers/api/bridge2),
read 2026-09-05, confirms native USDC, sender-account credit, and the minimum.
Reverify contract/token bindings and current rules before release; this revision
does not attest the live deployed configuration.
Credit must reach Hyperliquid account E_i, never the relayer. Refund ETH stays
in E_i. Rounding USD amounts creates no additional privacy boundary.

Require swap/transfer receipts and the actual Hyperliquid account credit before
completion. An accepted Arbitrum transaction alone is insufficient. A timed-out
venue API read requires reconciliation, not another deposit. Cross-chain
settlement is staged and recoverable, not one atomic transaction.

## 6. Waiting and recovery

Persist an encrypted local state machine:

| State | Evidence / next action |
| --- | --- |
| `deposited` | Final vault deposit; resume exact mint/shield operations |
| `shielded_waiting` | Verified note/witness; wait or explicit recovery |
| `exit_authorized` | Fresh analysis and exact approved payload; expire if not submitted in time |
| `egress_submitted` | Reconcile receipt/nullifier before retry or competing authorization |
| `egress_finalized` | D pfETH at P_i; resume same burn even if privacy evidence later weakens |
| `burn_finalized` | E_i/D/route binding verified; obtain public finality proof |
| `weth_released` | Exact release; resume unwrap and direct bridge |
| `arbitrum_funded` | Exact bridge message credited E_i; quote/sign bounded batch |
| `venue_deposit_pending` | Bridge2 transfer succeeded; reconcile without duplicate sending |
| `complete` | Exact venue credit, balances, and privacy report verified |

Keep hashes, nonces, proof identifiers, and balances by stage; reconcile before
sending after a restart. A local authorization expiry cannot revoke a leaked
signed transaction/proof. Once a packet leaves the wallet, reconcile or invalidate
it through the protocol before authorizing conflicting recovery.

Low activity creates no new consensus withdrawal lock. An unspent-note holder
may wait or explicitly choose ordinary redemption with the appropriate weaker
privacy label. Export local recovery material for another compatible wallet,
prover, or relay. Sponsor refusal is a service failure. Source-linked gas is an
explicit privacy downgrade, never an automatic fallback or a required CEX hop.
Existing chain/vault/proof outages can still delay redemption.

After irreversible egress, finish the already authorized settlement or recovery
from P_i/E_i. Report weakened privacy without retroactively blocking these public
funds. Returning funds to T is possible if explicitly selected and reveals that
exit. The 48-hour reminder never automatically initiates recovery.

## 7. Private client and operator boundaries

Only the encrypted wallet needs `T / note opening / witness -> P_i -> E_i`.
Generate private proofs locally or on user-controlled hardware. Ordinary remote
GPU proving with a supplied witness exposes private information. Public burn/
finality proofs may be outsourced only after checking their inputs/artifacts
contain no source-note data.

Separate ingress and exit transport sessions. Do not reuse source-wallet logins,
API tokens, cookies, analytics identifiers, or identifying RPC connections for
sponsors, relays, and venue access. Use supported privacy-preserving transport
without silent direct-network fallback. Tor does not defeat global timing
analysis. Unsupported venue/RPC transport must be reported as an operator/network
privacy limitation, not silently treated as solved.

Export only public exit data, expiry, and destination-bound signatures. Keep
`owned_evm_addresses`, source clusters, complete observer reports, shared plan
IDs, local labels, note paths, and seeds local. Use unrelated random per-exit job
IDs. Relays may know the public exit tail; they must not receive the private
entrance mapping.

Inspect CLI/UI, exception text, HTTP logs, filenames, proof artifacts, and
recovery bundles with marked synthetic identifiers/secrets. Compromise or
compelled disclosure of local records can reveal mappings. Selective disclosure
is optional user action, not a claim that disclosure causes no privacy loss.

## 8. Implementation order and acceptance gates

After research locking: observer and distinct claim gates, minimized jobs/shared
sponsorship, corrected bridge and reconciliation, bounded cover scheduler,
recovery/CLI, then user-facing interface. The initial version keeps egress
circuits and accounting unchanged.
A document edit does not itself require a fleet roll or route amendment.

| Gate | Required evidence |
| --- | --- |
| Target inference | k=1 fails; all candidate exits owned by T fail even with many addresses; ideal one-user/seven-cover fixture can pass public view while failing operator-aware view; reverse operator dominance does not falsely fail target privacy |
| Assignment model | Partial withdrawals, unspent/old notes, exact anchors, duplicate-spend exclusion, known mappings, asymmetric timing/volume; eight destinations with excessive `q_max` fail; assume T actually exits rather than diluting probability with nonwithdrawal |
| Coverage | Missing selected-view blocks/data, truncated solver, unsupported actions, stale snapshots, reordered/partial windows cannot pass; missing private operator records affect only claims requiring them; later disclosures update results |
| Mode authorization | Public mode can proceed with operator-aware failure visibly disclosed; operator-aware selection cannot silently downgrade; mode and cover policy are bound to the exact packet |
| Cover behavior | Same route to real Hyperliquid credits; public cover-policy-aware attacks, idle/active behavior, cover-only keys, recycling/retirement, operator-map leaks, and source-funded puppets cannot receive misleading labels |
| Cover budget | Real principal, concurrent inventory, fee ceilings, venue holding capacity, restart, depletion, and orderly stop reconcile; no real-user funds are spent as operator cover |
| Liveness | A passing cohort can enable an exit; depletion leaves others waiting but ordinary redemption remains available; reminders do not auto-withdraw |
| Data boundary | No source/secret identifiers in relay, sponsor, prover, RPC, receipt, or UI exports; no user-specific public sponsor clusters |
| Conservation | Integer units; exact D at each shield/bridge stage; native fees separate; quote/refund accounting; no automatic dust consolidation |
| Ethereum bridge | Pinned deployed-code simulation exposes the old route's aliasing and demonstrates direct EOA delivery to ordinary E_i; exact message tracking |
| Recovery | Sponsor outage, pending/reverted transactions, nonce conflicts, proof retry, failed 7702 calls, stale quotes, venue-credit delay, and restart never duplicate deposits or require a CEX |
| Functional probe | Authorized bounded self-custody Ethereum-to-Hyperliquid run through every stage; direct Arbitrum prefunding cannot substitute for the bridge |
| Privacy launch | Real funded history passes all models for the selected public-observer claim, including cover-pattern attacks; operator-owned cover may supply alternatives. Operator-aware protection requires its separate passing evidence. Synthetic account counts do not qualify. |
| Release | Applicable contracts/proofs/routes reviewed and pinned; focused checks; recoverable backup; usable Python CLI then interface; evidence in implementation journal |

Run fixtures/simulation before authorized live probes. A small test denomination
can prove mechanics but cannot qualify the 1-ETH cohort. The interface must show
ETH exposure, source amount, fees, selected privacy claim, both observer results,
operator inference risk, cover capacity/holding horizon, waiting/recovery,
and credited account without sending the private mapping to telemetry.
This spec revision authorizes no deployment or fund movement.

Until real-history public-observer analysis passes, offer a clearly labeled
functional pilot or waitlist. Operator-funded cover is a valid bootstrap input
to that analysis, not an automatic guarantee. Independent users are not a
mandatory prerequisite for this narrower claim. Operator-aware protection
requires remaining uncertainty after operator knowledge is included and must
not be advertised merely because the public-observer claim passes.

## 9. Deferred capabilities

Private split/merge is for arbitrary inputs and private change, not a prerequisite
for this fixed-denomination version. Its later spec must define enabled/dummy
legs, note membership, spend authority, nullifiers, asset tags, range checks,
overflow-safe conservation, encryption, versioned keys, accounting, and observer
compatibility. Existing pair-permutation/nonzero constraints do not implement
`10 -> 5 + 5` or `1 + 1 -> 2`.

Atomic multi-user releases, private fee payment, stronger anonymous relay
admission, a stablecoin pool, and venue return/consolidation privacy require
separate evidence. Do not imply them in the initial funding claim or turn them
into unexplained prerequisites for an already funded user's redemption.
