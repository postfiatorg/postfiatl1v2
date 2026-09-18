# Lighter self-custody funding extension

Date: 2026-09-05. Status: draft, research lock pending.
Task Node: `task_1b81da68342923f814770ea149d3f048`.

This extension serves the user's objective: fund a fresh Lighter account from an
existing Ethereum self-custody wallet without a CEX, with an explicitly measured
public-observer funding-privacy claim. It preserves the locked
[Hyperliquid specification](shielded-perp-venue-funding-privacy-hardening-20260905.md)
and its accepted implementation task. It inherits that specification's private
note boundary, operator-cover separation, consent, holding, recovery, and evidence
requirements except for the venue-specific differences below. Neither document
claims production privacy before funded qualification.

## Route and completion

The route is Ethereum source T -> shared WETH vault -> authenticated pfETH
source-series mint to T' -> encrypted 1-ETH Asset-Orchard ingress -> local proof
and privacy gate -> private egress to fresh PFTL P_i -> burn of the exact held
source-series asset -> proof-bound release of exactly 1 WETH to fresh Ethereum
E_i -> common-sponsored atomic WETH unwrap and Lighter ETH spot deposit to E_i.
The exit key stays on user-controlled hardware. P_i must be activated by the
common PFT sponsor, with any public activation included before privacy analysis.
Native fees are separate from the 1-ETH principal.

Unlike the Hyperliquid tail, Lighter needs no Ethereum-to-Arbitrum bridge or USD
swap. Its Ethereum deposit accepts an explicit credited L1 address. Reuse the
existing reviewed `ExitExecutorV1` builder's chain-1 EIP-7702 authorization and
EIP-712 batch: `WETH.withdraw(10^18)`, then
`deposit(E_i, 1, 1, 10^18)` with value `10^18`. The second `1` selects spot;
non-USDC assets cannot be deposited directly to the perps route. Check current
asset metadata before admission; do not infer eligibility from the builder's
constants. The [official deposit documentation](https://apidocs.lighter.xyz/docs/deposits-transfers-and-withdrawals)
describes recipient, asset, route and base-unit amounts. Its stated mainnet
contract is `0x3B4D794a66304F130a4Db8F2551B0070dfCf5ca7`.

Require exact 1-WETH balance, zero native ETH, no code/delegation and nonce zero
in E_i before signing. Bind chain 1, executor, E_i, executor nonce zero, both
calls and values, and a deadline at most 900 seconds away. The sponsor signs its
own bounded type-4 transaction under a shared durable nonce lock. Never send
E_i's private key, source wallet, note, witness, or private ownership map to the
sponsor. Public exit signatures are sufficient. Ethereum delegation persists;
a failed batch may still apply the delegation or consume account nonces.

Before any spend, pin Ethereum chain ID, finalized block/hash, vault and proof
route, WETH, executor and Lighter proxy runtime, applicable implementation/beacon
slots and implementation bytecode. Record governance/upgrade trust separately.
Recheck pins at pending and finalized state; a changed implementation halts new
admissions and requires review. Simulate the exact sponsored transaction. A
successful simulation does not establish future execution or venue credit.

Completion requires an exact canonical finalized Ethereum receipt, matching
signed transaction, `BatchExecuted(E_i, 0, 2)`, WETH withdrawal, and the deployed
Lighter contract's actual deposit event fields for E_i/ETH/spot/amount. Verify the
ABI/event layout against deployed code rather than inventing an event signature.
Also require actual Lighter deposit credit. Account creation or an HTTP success
response alone is insufficient. Use the official SDK's deposit history fields:
`id`, `amount`, `timestamp`, `status`, `l1_tx_hash`, `asset_id`; require completed
status, the exact L1 transaction hash and exact ETH amount. Read complete pages,
reject repeated cursors/duplicate identities, and verify the returned account's
L1 address. The [official SDK schema](https://github.com/elliottech/lighter-python/blob/main/lighter/models/deposit_history_item.py)
is the source contract for this adapter. Verify amount units against live asset
metadata and a recorded deposit; use integer or exact decimal arithmetic.
Unavailable authenticated history is pending verification, never a reason to
repay. The venue API remains a disclosed trust boundary unless its L2 state is
independently reconstructed and verified; this extension does not claim that.

The funding milestone ends at credited ETH spot balance. Enabling ETH margin or
unified trading is an explicit subsequent user action through the existing
Lighter SDK helper; it may require local API-key creation and account settings.
Do not label credited spot ETH as already enabled perps collateral or place a
trade. Preserve source/destination separation when enabling API access.

## Privacy and real cover

The exact public exit on PFTL remains the first irreversible privacy boundary.
Before exporting its proof, require a fresh authenticated history and the same
four precisely versioned inference models as the Hyperliquid specification:
uniform, timing, ownership concentration, and cover/venue behavior. Keep both
public-observer and named-operator-aware results visible and bind the selected
claim and policy hashes to the exact proof payload. Operator-owned cover can
supply alternative destinations under the public model; it cannot establish
protection from an operator that knows all other assignments. Sockpuppet address
counts do not prove independent ownership.

Analyze venue choice explicitly. An observer who knows T is funding Lighter can
exclude Hyperliquid-only destinations. Do not combine eight destinations across
two venues to claim eight plausible Lighter destinations. Use a separate Lighter
cohort and include all publicly known destination clustering within it. Unknown
venue choice must be treated as uncertainty in a named model, not silently
assumed. A public source request or previously visible Lighter intent also
conditions that model. The code extension must version this conditioning rather
than silently changing the locked Hyperliquid policy's hash.

Cover uses actual operator-funded 1-ETH notes, the same vault/source series,
proof, activation, sponsor, Lighter deposit and venue behavior as users. Source
series with distinct public asset IDs remain distinct inference candidates;
family membership does not make them interchangeable. Schedules are finite,
cryptographically randomized independently of user arrivals, and committed
before admission. Minimum shielding hold is six hours; hourly release windows
are conventions, not an assumption of atomic cohort settlement. Cover may exit
with an explicit no-privacy operator label to bootstrap; users never inherit
that bypass. The selected user model needs at least eight plausible destination
clusters and q_max <= 1/8 in every model, conditioned on T completing an exit.
Cover-only traffic does not create a user privacy result.

One user plus seven operator notes is only a theoretical minimum. Actual timing,
source-series, destination behavior and later disclosure may require more
inventory or fail altogether. Reserve cover principal, sponsor ETH and PFT,
proof costs, concurrent capacity and venue holding horizon before admission.
Stop new admissions on depletion. Do not spend user principal on cover, recycle
cover during the pilot, sweep dust, consolidate venue accounts, or infer a
privacy pass from elapsed time. Visible later trading differences and leaked
cover mappings must update both models and may invalidate prior inference.

## Authenticated history and recovery

Reuse native public snapshot replay and preserve chain/genesis/checkpoint pins.
Index the base asset and every registered source series, but retain exact asset,
source bucket, amount and commitment position in each record. Never remove old
notes or failed attempts to improve a score. PFTL headers have no wall-clock
field: collect contemporaneous observations, retain uncertainty intervals and
fail an exact timing gate when those intervals cannot support it. Do not
retroactively fabricate block timestamps from current observation time.

The complete importer must link Ethereum deposits, PFTL mints/ingresses,
commitment membership, egress nullifiers, burns, releases, sponsor operations and
Lighter credits, with coverage boundaries and content hashes. Unknown private
swaps/issuance remain inconclusive unless conservative analysis proves they
cannot affect the selected series/cohort. A JSON coverage flag cannot create
verified production evidence. Local operator maps remain separate and are never
exported with relay jobs. Network/venue sessions must not reuse source-linked
credentials or silently fall back from the selected privacy transport.

Use a venue-specific state path after `weth_released`: `venue_deposit_pending`
then `complete`; do not manufacture an `arbitrum_funded` event for Lighter. Before
that, use the shared durable stages and exact operation IDs. Persist signed bytes
before broadcasting. Reconcile exact proof/nullifier, transaction, nonce and
receipt before retries. A timeout never produces a second deposit. A changed
anchor, destination, amount or privacy policy requires new local authorization
before first export. Once egress is public, finish its authorized public tail
even if subsequent privacy analysis worsens.

On revert, preserve the receipt, actual fee, EOA delegation and executor/account
nonces, and retained WETH/ETH balances. A new signature needs explicit bounded
recovery of that same compartment. If venue credit is delayed or ambiguous,
continue reconciling the original deposit. Withdrawal helpers may return funds
to E_i through Lighter's secure withdrawal path, subject to venue availability;
verify route and final credit before exposing an executable recovery action.
Never automatically withdraw or return to T, and disclose the link if the user
explicitly chooses T. Returns are operational recovery, not a new privacy claim.

## Implementation and release evidence

The accepted extension task governs research locking, a concise milestone,
Python CLI, then its working local interface. The CLI must execute the real
configured route; any unavailable stage returns a specific pending/blocked
state. Buttons invoke that same controller rather than print manual commands.
Show route, credited account, denomination, fees, ETH exposure, current stage,
selected claim, both model results, sponsor capacity and recovery choices.
Keep private source maps and keys out of browser responses, logs and telemetry.

Required checks include exact native ingress/private-egress/burn conformance;
series-aware history completeness and missing-evidence refusal; venue-conditioned
privacy fixtures; common nonce concurrency, lost responses and restart; exact
Ethereum deposit/event and separate Lighter credit reconciliation; reverted 7702
recovery; integer conservation; CLI/interface secret scans and focused regression
tests. Validate available test routes rather than assume a full testnet vault
exists. Local real proofs are primitive conformance; mocked receipts are
simulation; prefunded live deposit tails prove only that tail. None substitutes
for a self-custody-to-venue funded run or funded privacy qualification.

Before mainnet execution, prepare a concrete memo naming isolated wallet
addresses, source principal, cover principal, fee ceilings per chain/operation,
exact contract/proof pins and hashes, backup/recovery paths and the proposed
commands. Obtain approval for the specified capital and actions; this design
note grants no fund movement. A small approved mechanics probe cannot qualify
the 1-ETH privacy cohort. Keep end-to-end execution, real-history privacy and
operator-aware qualification separately open until each has its own evidence.
